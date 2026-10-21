use std::time::Duration;

use mlua::prelude::*;
use quark_core::Options;
use redis::aio::{
    ConnectionLike, ConnectionManager, ConnectionManagerConfig, MultiplexedConnection,
};
use redis::cluster::ClusterClient;
use redis::cluster_async::ClusterConnection;
use redis::cluster_read_routing::RandomReplicaStrategy;
use redis::{Cmd, Pipeline, RedisFuture, Value};

use crate::settings;

#[derive(Clone)]
pub enum Connection {
    Managed(ConnectionManager),
    Direct(MultiplexedConnection),
    Cluster(ClusterConnection),
}

impl ConnectionLike for Connection {
    fn req_packed_command<'a>(&'a mut self, command: &'a Cmd) -> RedisFuture<'a, Value> {
        match self {
            Connection::Managed(connection) => connection.req_packed_command(command),
            Connection::Direct(connection) => connection.req_packed_command(command),
            Connection::Cluster(connection) => connection.req_packed_command(command),
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
        }
    }

    fn get_db(&self) -> i64 {
        match self {
            Connection::Managed(connection) => connection.get_db(),
            Connection::Direct(connection) => connection.get_db(),
            Connection::Cluster(connection) => connection.get_db(),
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
            LuaValue::Table(nodes) => {
                let nodes = nodes
                    .sequence_values::<String>()
                    .collect::<LuaResult<Vec<_>>>()?;
                Self::cluster(nodes, options)
            }
            other => Err(LuaError::runtime(format!(
                "expected a Redis URL or a list of cluster node URLs, got {}",
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

    pub async fn managed(&self) -> Result<Connection, String> {
        match &self.target {
            Target::Single { client, config } => {
                ConnectionManager::new_with_config(client.clone(), config.clone())
                    .await
                    .map(Connection::Managed)
                    .map_err(|error| format!("cannot connect to Redis: {error}"))
            }
            Target::Cluster { client, .. } => self.cluster_connection(client).await,
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

    pub fn pubsub_client(&self) -> &redis::Client {
        match &self.target {
            Target::Single { client, .. } => client,
            Target::Cluster { pubsub, .. } => pubsub,
        }
    }
}
