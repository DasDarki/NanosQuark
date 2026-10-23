use std::time::Duration;

use mlua::prelude::*;
use quark_core::Options;
use redis::aio::{
    ConnectionLike, ConnectionManager, ConnectionManagerConfig, MultiplexedConnection,
};
use redis::cluster::ClusterClient;
use redis::cluster_async::ClusterConnection;
use redis::cluster_read_routing::RandomReplicaStrategy;
use std::sync::Arc;

use redis::sentinel::{SentinelClient, SentinelNodeConnectionInfo, SentinelServerType};
use redis::{
    AsyncConnectionConfig, Cmd, ErrorKind, Pipeline, RedisError, RedisFuture, RedisResult,
    ServerErrorKind, Value,
};
use tokio::sync::Mutex as AsyncMutex;

use crate::settings;

#[derive(Clone)]
pub enum Connection {
    Managed(ConnectionManager),
    Direct(MultiplexedConnection),
    Cluster(ClusterConnection),
    Sentinel(SentinelConnection),
}

pub struct SentinelShared {
    client: AsyncMutex<SentinelClient>,
    current: AsyncMutex<Option<MultiplexedConnection>>,
    timeout: Duration,
    retries: usize,
    database: i64,
    master: String,
    watching: std::sync::atomic::AtomicBool,
    switches: tokio::sync::watch::Sender<u64>,
}

pub async fn master_switch(switches: &mut Option<tokio::sync::watch::Receiver<u64>>) {
    match switches {
        Some(receiver) => {
            if receiver.changed().await.is_err() {
                std::future::pending::<()>().await;
            }
        }
        None => std::future::pending::<()>().await,
    }
}

async fn watch_switches(shared: std::sync::Weak<SentinelShared>) {
    use futures_util::StreamExt;
    loop {
        let Some(current) = shared.upgrade() else {
            return;
        };
        let sentinel = current
            .client
            .lock()
            .await
            .async_get_sentinel_client()
            .await;
        let master = current.master.clone();
        drop(current);

        if let Ok(sentinel) = sentinel
            && let Ok(mut pubsub) = sentinel.get_async_pubsub().await
            && pubsub.subscribe("+switch-master").await.is_ok()
        {
            let mut messages = pubsub.on_message();
            while let Some(message) = messages.next().await {
                let payload: String = message.get_payload().unwrap_or_default();
                if payload.split(' ').next() != Some(master.as_str()) {
                    continue;
                }
                match shared.upgrade() {
                    Some(current) => {
                        current.reset().await;
                        current.switches.send_modify(|count| *count += 1);
                    }
                    None => return,
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

#[derive(Clone)]
pub struct SentinelConnection(Arc<SentinelShared>);

fn should_reconnect(error: &RedisError) -> bool {
    error.is_io_error()
        || error.is_connection_dropped()
        || error.is_connection_refusal()
        || error.is_timeout()
        || error.kind() == ErrorKind::Server(ServerErrorKind::ReadOnly)
}

fn retry_delay(attempt: usize) -> Duration {
    Duration::from_millis((attempt as u64 * 200).min(1000))
}

impl SentinelShared {
    async fn master(&self) -> RedisResult<redis::Client> {
        let mut client = self.client.lock().await;
        match tokio::time::timeout(self.timeout, client.async_get_client()).await {
            Ok(result) => result,
            Err(_) => Err(RedisError::from((
                ErrorKind::Io,
                "asking the sentinels for the master timed out",
            ))),
        }
    }

    async fn open(&self) -> RedisResult<MultiplexedConnection> {
        let client = self.master().await?;
        let config = AsyncConnectionConfig::new()
            .set_connection_timeout(Some(self.timeout))
            .set_response_timeout(Some(self.timeout));
        client
            .get_multiplexed_async_connection_with_config(&config)
            .await
    }

    fn watch(self: &Arc<Self>) {
        if !self
            .watching
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            quark_core::runtime().spawn(watch_switches(Arc::downgrade(self)));
        }
    }

    async fn connection(self: &Arc<Self>) -> RedisResult<MultiplexedConnection> {
        self.watch();
        let mut current = self.current.lock().await;
        if let Some(connection) = current.as_ref() {
            return Ok(connection.clone());
        }
        let connection = self.open().await?;
        *current = Some(connection.clone());
        Ok(connection)
    }

    async fn reset(&self) {
        *self.current.lock().await = None;
    }
}

impl ConnectionLike for SentinelConnection {
    fn req_packed_command<'a>(&'a mut self, command: &'a Cmd) -> RedisFuture<'a, Value> {
        Box::pin(async move {
            let mut attempt = 0;
            loop {
                let result = match self.0.connection().await {
                    Ok(mut connection) => connection.req_packed_command(command).await,
                    Err(error) => Err(error),
                };
                match result {
                    Err(error) if should_reconnect(&error) && attempt < self.0.retries => {
                        self.0.reset().await;
                        attempt += 1;
                        tokio::time::sleep(retry_delay(attempt)).await;
                    }
                    other => return other,
                }
            }
        })
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        pipeline: &'a Pipeline,
        offset: usize,
        count: usize,
    ) -> RedisFuture<'a, Vec<Value>> {
        Box::pin(async move {
            let mut attempt = 0;
            loop {
                let result = match self.0.connection().await {
                    Ok(mut connection) => {
                        connection
                            .req_packed_commands(pipeline, offset, count)
                            .await
                    }
                    Err(error) => Err(error),
                };
                match result {
                    Err(error) if should_reconnect(&error) && attempt < self.0.retries => {
                        self.0.reset().await;
                        attempt += 1;
                        tokio::time::sleep(retry_delay(attempt)).await;
                    }
                    other => return other,
                }
            }
        })
    }

    fn get_db(&self) -> i64 {
        self.0.database
    }
}

impl ConnectionLike for Connection {
    fn req_packed_command<'a>(&'a mut self, command: &'a Cmd) -> RedisFuture<'a, Value> {
        match self {
            Connection::Managed(connection) => connection.req_packed_command(command),
            Connection::Direct(connection) => connection.req_packed_command(command),
            Connection::Cluster(connection) => connection.req_packed_command(command),
            Connection::Sentinel(connection) => connection.req_packed_command(command),
        }
    }

    fn req_packed_commands<'a>(
        &'a mut self,
        pipeline: &'a Pipeline,
        offset: usize,
        count: usize,
    ) -> RedisFuture<'a, Vec<Value>> {
        match self {
            Connection::Managed(connection) => {
                connection.req_packed_commands(pipeline, offset, count)
            }
            Connection::Direct(connection) => {
                connection.req_packed_commands(pipeline, offset, count)
            }
            Connection::Cluster(connection) => {
                connection.req_packed_commands(pipeline, offset, count)
            }
            Connection::Sentinel(connection) => {
                connection.req_packed_commands(pipeline, offset, count)
            }
        }
    }

    fn get_db(&self) -> i64 {
        match self {
            Connection::Managed(connection) => connection.get_db(),
            Connection::Direct(connection) => connection.get_db(),
            Connection::Cluster(connection) => connection.get_db(),
            Connection::Sentinel(connection) => connection.get_db(),
        }
    }
}

pub enum Target {
    Single {
        client: redis::Client,
        config: ConnectionManagerConfig,
    },
    Cluster {
        client: Box<ClusterClient>,
        pubsub: redis::Client,
    },
    Sentinel(Arc<SentinelShared>),
}

pub struct Endpoint {
    pub target: Target,
    pub timeout: Duration,
}

impl Endpoint {
    pub fn from_lua(target: LuaValue, options: &Options) -> LuaResult<Self> {
        match target {
            LuaNil => Self::single(None, options),
            LuaValue::String(url) => Self::single(Some(url.to_str()?.to_owned()), options),
            LuaValue::Table(table) if table.contains_key("Sentinels")? => {
                Self::sentinel(&table, options)
            }
            LuaValue::Table(nodes) => {
                let nodes = nodes
                    .sequence_values::<String>()
                    .collect::<LuaResult<Vec<_>>>()?;
                Self::cluster(nodes, options)
            }
            other => Err(LuaError::runtime(format!(
                "expected a Redis URL, a list of cluster node URLs or a sentinel table, got {}",
                other.type_name()
            ))),
        }
    }

    fn single(url: Option<String>, options: &Options) -> LuaResult<Self> {
        let settings = settings(url, options)?;
        let config = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(settings.timeout))
            .set_response_timeout(Some(settings.timeout))
            .set_number_of_retries(options.get_or("Retries", 6)?);
        Ok(Endpoint {
            target: Target::Single {
                client: settings.client,
                config,
            },
            timeout: settings.timeout,
        })
    }

    fn cluster(nodes: Vec<String>, options: &Options) -> LuaResult<Self> {
        let first = nodes
            .first()
            .cloned()
            .ok_or_else(|| LuaError::runtime("a cluster needs at least one node URL"))?;
        let pubsub = settings(Some(first), options)?;
        for node in &nodes {
            crate::check_url(node)?;
        }

        let mut builder = ClusterClient::builder(nodes.iter().map(String::as_str))
            .connection_timeout(pubsub.timeout)
            .response_timeout(pubsub.timeout)
            .retries(options.get_or("Retries", 6)?);
        if let Some(username) = options.get::<String>("Username")? {
            builder = builder.username(username);
        }
        if let Some(password) = options.get::<String>("Password")? {
            builder = builder.password(password);
        }
        if nodes.iter().any(|node| node.starts_with("rediss://")) {
            builder = builder.tls(redis::TlsMode::Secure);
        }
        if let Some(ca) = options.get::<String>("CA")? {
            builder = builder.certs(redis::TlsCertificates {
                client_tls: None,
                root_cert: Some(ca.into_bytes()),
            });
        }
        if options.get_or("ReadFromReplicas", false)? {
            builder = builder.read_routing_strategy(RandomReplicaStrategy);
        }
        let client = builder.build().map_err(|error| {
            LuaError::runtime(format!("invalid Redis cluster settings: {error}"))
        })?;

        Ok(Endpoint {
            target: Target::Cluster {
                client: Box::new(client),
                pubsub: pubsub.client,
            },
            timeout: pubsub.timeout,
        })
    }

    fn sentinel(table: &LuaTable, options: &Options) -> LuaResult<Self> {
        crate::install_crypto_provider();
        let sentinels: Vec<String> = table
            .get::<Vec<String>>("Sentinels")
            .map_err(|_| LuaError::runtime("Sentinels has to be a list of sentinel URLs"))?;
        if sentinels.is_empty() {
            return Err(LuaError::runtime("at least one sentinel URL is needed"));
        }
        for sentinel in &sentinels {
            crate::check_url(sentinel)?;
        }
        let master: String = table
            .get::<Option<String>>("MasterName")?
            .ok_or_else(|| LuaError::runtime("the MasterName of the sentinel setup is required"))?;

        let mut redis_settings = redis::RedisConnectionInfo::default();
        if let Some(username) = options.get::<String>("Username")? {
            redis_settings = redis_settings.set_username(username);
        }
        if let Some(password) = options.get::<String>("Password")? {
            redis_settings = redis_settings.set_password(password);
        }
        let database = options.get_or("Database", 0)?;
        redis_settings = redis_settings.set_db(database);
        let mut node =
            SentinelNodeConnectionInfo::default().set_redis_connection_info(redis_settings);
        if options.get_or("TLS", false)? {
            node = node.set_tls_mode(redis::TlsMode::Secure);
        }

        let master_name = master.clone();
        let client = SentinelClient::build(
            sentinels.iter().map(String::as_str).collect(),
            master,
            Some(node),
            SentinelServerType::Master,
        )
        .map_err(|error| LuaError::runtime(format!("invalid sentinel settings: {error}")))?;
        let timeout = Duration::from_millis(options.get_or("Timeout", crate::DEFAULT_TIMEOUT)?);
        Ok(Endpoint {
            target: Target::Sentinel(Arc::new(SentinelShared {
                client: AsyncMutex::new(client),
                current: AsyncMutex::new(None),
                timeout,
                retries: options.get_or("Retries", 6)?,
                database,
                master: master_name,
                watching: std::sync::atomic::AtomicBool::new(false),
                switches: tokio::sync::watch::channel(0).0,
            })),
            timeout,
        })
    }

    pub async fn managed(&self) -> Result<Connection, String> {
        match &self.target {
            Target::Single { client, config } => {
                ConnectionManager::new_with_config(client.clone(), config.clone())
                    .await
                    .map(Connection::Managed)
                    .map_err(|error| format!("cannot connect to Redis: {error}"))
            }
            Target::Cluster { client, .. } => self.cluster_connection(client).await,
            Target::Sentinel(shared) => {
                shared
                    .connection()
                    .await
                    .map_err(|error| format!("cannot connect to the Redis master: {error}"))?;
                Ok(Connection::Sentinel(SentinelConnection(Arc::clone(shared))))
            }
        }
    }

    pub async fn dedicated(&self) -> Result<Connection, String> {
        match &self.target {
            Target::Single { client, .. } => {
                tokio::time::timeout(self.timeout, client.get_multiplexed_async_connection())
                    .await
                    .map_err(|_| "connecting to Redis timed out".to_owned())?
                    .map(Connection::Direct)
                    .map_err(|error| format!("cannot connect to Redis: {error}"))
            }
            Target::Cluster { client, .. } => self.cluster_connection(client).await,
            Target::Sentinel(shared) => {
                shared.watch();
                shared.open().await
            }
            .map(Connection::Direct)
            .map_err(|error| format!("cannot connect to the Redis master: {error}")),
        }
    }

    async fn cluster_connection(&self, client: &ClusterClient) -> Result<Connection, String> {
        tokio::time::timeout(self.timeout, client.get_async_connection())
            .await
            .map_err(|_| "connecting to the Redis cluster timed out".to_owned())?
            .map(Connection::Cluster)
            .map_err(|error| format!("cannot connect to the Redis cluster: {error}"))
    }

    pub fn is_cluster(&self) -> bool {
        matches!(self.target, Target::Cluster { .. })
    }

    pub async fn pubsub_client(&self) -> Result<redis::Client, String> {
        match &self.target {
            Target::Single { client, .. } => Ok(client.clone()),
            Target::Cluster { pubsub, .. } => Ok(pubsub.clone()),
            Target::Sentinel(shared) => {
                shared.watch();
                shared
                    .master()
                    .await
                    .map_err(|error| format!("cannot find the Redis master: {error}"))
            }
        }
    }

    pub fn switches(&self) -> Option<tokio::sync::watch::Receiver<u64>> {
        match &self.target {
            Target::Sentinel(shared) => Some(shared.switches.subscribe()),
            _ => None,
        }
    }
}
