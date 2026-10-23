use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_nats::{Client, ConnectOptions, Event, HeaderMap, Message, Request};
use bytes::Bytes as NatsBytes;
use futures_util::StreamExt;
use mlua::prelude::*;
use quark_core::{
    Binary, Bytes, Emitter, Options, Resource, create_object, emit_now, object_state,
    options_and_callback, runtime,
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSED: u8 = 2;
const DEFAULT_TIMEOUT: u64 = 5_000;

struct Shared {
    state: Arc<AtomicU8>,
    client: Client,
    emitter: OnceLock<Emitter>,
}

impl Shared {
    fn report(&self, message: String) {
        if let Some(emitter) = self.emitter.get() {
            emitter.emit("Error", message);
        }
    }

    fn close(self: &Arc<Self>) {
        if self.state.swap(CLOSED, Ordering::AcqRel) == CLOSED {
            return;
        }
        let shared = Arc::clone(self);
        runtime().spawn(async move {
            let _ = shared.client.drain().await;
            if let Some(emitter) = shared.emitter.get() {
                emitter.emit("Close", "closed".to_owned());
            }
        });
    }
}

struct Handle(Arc<Shared>);

impl Resource for Handle {
    fn close(&self) {
        self.0.close();
    }
}

pub struct NatsClient {
    shared: Arc<Shared>,
    _handle: Arc<Handle>,
}

struct SubscriptionShared {
    stop: UnboundedSender<()>,
    subject: String,
    queue: Option<String>,
}

impl Resource for SubscriptionShared {
    fn close(&self) {
        let _ = self.stop.send(());
    }
}

pub struct NatsSubscription {
    shared: Arc<SubscriptionShared>,
}

impl LuaUserData for NatsSubscription {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        quark_core::add_event_methods(methods);
        methods.add_method("Unlisten", |_, subscription, ()| {
            subscription.shared.close();
            Ok(())
        });
        methods.add_method("GetSubject", |_, subscription, ()| {
            Ok(subscription.shared.subject.clone())
        });
        methods.add_method("GetQueue", |_, subscription, ()| {
            Ok(subscription.shared.queue.clone())
        });
    }
}

struct Received {
    message: Message,
    client: Client,
}

fn read_headers(table: Option<LuaTable>) -> LuaResult<Option<HeaderMap>> {
    let Some(table) = table else {
        return Ok(None);
    };
    let mut headers = HeaderMap::new();
    for pair in table.pairs::<String, String>() {
        let (name, value) = pair?;
        headers.insert(name.as_str(), value.as_str());
    }
    Ok(Some(headers))
}

impl IntoLua for Received {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let Received { message, client } = self;
        let table = lua.create_table()?;
        table.set("Subject", message.subject.as_str())?;
        table.set("Payload", Bytes(message.payload.to_vec()))?;
        table.set(
            "Reply",
            message
                .reply
                .as_ref()
                .map(|reply| reply.as_str().to_owned()),
        )?;
        if let Some(headers) = &message.headers {
            let list = lua.create_table()?;
            for (name, values) in headers.iter() {
                let joined = values
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                list.set(AsRef::<str>::as_ref(name), joined)?;
            }
            table.set("Headers", list)?;
        }
        let reply = message.reply.clone();
        table.set(
            "Respond",
            lua.create_function(
                move |_, (_, payload, headers): (LuaValue, Binary, Option<LuaTable>)| {
                    let Some(reply) = reply.clone() else {
                        return Err(LuaError::runtime("the message has no reply subject"));
                    };
                    let headers = read_headers(headers)?;
                    let client = client.clone();
                    let payload = NatsBytes::from(payload.0);
                    runtime().spawn(async move {
                        let _ = match headers {
                            Some(headers) => {
                                client.publish_with_headers(reply, headers, payload).await
                            }
                            None => client.publish(reply, payload).await,
                        };
                    });
                    Ok(true)
                },
            )?,
        )?;
        Ok(LuaValue::Table(table))
    }
}

fn subject(text: &str) -> LuaResult<String> {
    if text.is_empty() || text.contains([' ', '\t', '\r', '\n']) {
        return Err(LuaError::runtime(format!(
            "'{text}' is not a valid NATS subject"
        )));
    }
    Ok(text.to_owned())
}

impl LuaUserData for NatsClient {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        quark_core::add_event_methods(methods);

        methods.add_method(
            "Publish",
            |_, client, (name, payload, options): (String, Binary, Options)| {
                let name = subject(&name)?;
                let reply: Option<String> = options.get("Reply")?;
                let headers = read_headers(options.get("Headers")?)?;
                let shared = Arc::clone(&client.shared);
                let payload = NatsBytes::from(payload.0);
                runtime().spawn(async move {
                    let result = match (reply, headers) {
                        (None, None) => shared.client.publish(name, payload).await,
                        (Some(reply), None) => {
                            shared.client.publish_with_reply(name, reply, payload).await
                        }
                        (None, Some(headers)) => {
                            shared
                                .client
                                .publish_with_headers(name, headers, payload)
                                .await
                        }
                        (Some(reply), Some(headers)) => {
                            shared
                                .client
                                .publish_with_reply_and_headers(name, reply, headers, payload)
                                .await
                        }
                    };
                    if let Err(error) = result {
                        shared.report(format!("cannot publish: {error}"));
                    }
                });
                Ok(true)
            },
        );

        methods.add_method(
            "Listen",
            |lua, client, (name, handler, options): (String, LuaValue, LuaValue)| {
                let name = subject(&name)?;
                let (handler, options) = match (handler, options) {
                    (LuaValue::Function(handler), options) => (Some(handler), Options::from_lua(options, lua)?),
                    (LuaValue::Table(table), LuaNil) => (None, Options::from_table(table)),
                    (LuaNil, options) => (None, Options::from_lua(options, lua)?),
                    (other, _) => {
                        return Err(LuaError::runtime(format!(
                            "expected a message handler or an options table, got {}",
                            other.type_name()
                        )));
                    }
                };
                let queue: Option<String> = options.get("Queue")?;
                let (stop, mut stopped) = unbounded_channel();
                let shared = Arc::new(SubscriptionShared {
                    stop,
                    subject: name.clone(),
                    queue: queue.clone(),
                });
                let object = create_object(lua, NatsSubscription { shared: Arc::clone(&shared) })?;
                if let Some(handler) = handler {
                    object_state(&object)?.raw_set("handler", handler)?;
                }
                let emitter = Emitter::new(lua, &object)?;
                let weak: std::sync::Weak<dyn Resource> = Arc::downgrade(&shared) as std::sync::Weak<dyn Resource>;
                emitter.dispatcher().track(lua, weak)?;

                let client = client.shared.client.clone();
                let keep = Arc::clone(&shared);
                runtime().spawn(async move {
                    let _keep = keep;
                    let subscribed = match &queue {
                        Some(queue) => client.queue_subscribe(name.clone(), queue.clone()).await,
                        None => client.subscribe(name.clone()).await,
                    };
                    let mut subscriber = match subscribed {
                        Ok(subscriber) => subscriber,
                        Err(error) => {
                            emitter.emit("Error", format!("cannot subscribe to {name}: {error}"));
                            return;
                        }
                    };
                    loop {
                        tokio::select! {
                            message = subscriber.next() => match message {
                                Some(message) => {
                                    let received = Received { message, client: client.clone() };
                                    emitter.with(move |lua, object| {
                                        let state = object_state(object)?;
                                        if let Some(handler) = state.raw_get::<Option<LuaFunction>>("handler")? {
                                            handler.call::<()>(received)
                                        } else {
                                            emit_now(lua, object, "Message", received)
                                        }
                                    });
                                }
                                None => break,
                            },
                            _ = stopped.recv() => {
                                let _ = subscriber.unsubscribe().await;
                                break;
                            }
                        }
                    }
                });
                Ok(object)
            },
        );

        methods.add_method(
            "Request",
            |lua,
             client,
             (name, payload, options, callback): (
                String,
                Binary,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = options_and_callback(options, callback)?;
                let name = subject(&name)?;
                let timeout = Duration::from_millis(options.get_or("Timeout", DEFAULT_TIMEOUT)?);
                let mut request = Request::new()
                    .payload(NatsBytes::from(payload.0))
                    .timeout(Some(timeout));
                if let Some(headers) = read_headers(options.get("Headers")?)? {
                    request = request.headers(headers);
                }
                let client = client.shared.client.clone();
                quark_core::run_async(lua, callback, async move {
                    client
                        .send_request(name.clone(), request)
                        .await
                        .map(|message| Received {
                            message,
                            client: client.clone(),
                        })
                        .map_err(|error| format!("the request to {name} failed: {error}"))
                })
            },
        );

        methods.add_method("Flush", |lua, client, callback: Option<LuaFunction>| {
            let client = client.shared.client.clone();
            quark_core::run_async(lua, callback, async move {
                client
                    .flush()
                    .await
                    .map(|_| true)
                    .map_err(|error| format!("cannot flush: {error}"))
            })
        });
        methods.add_method("GetState", |_, client, ()| {
            Ok(match client.shared.state.load(Ordering::Acquire) {
                CONNECTING => "connecting",
                OPEN => "open",
                _ => "closed",
            })
        });
        methods.add_method("IsOpen", |_, client, ()| {
            Ok(client.shared.state.load(Ordering::Acquire) == OPEN)
        });
        methods.add_method("Close", |_, client, ()| {
            client.shared.close();
            Ok(())
        });
    }
}

fn connect(lua: &Lua, (servers, options): (LuaValue, Options)) -> LuaResult<LuaMultiValue> {
    let servers: Vec<String> = match servers {
        LuaNil => vec!["nats://127.0.0.1:4222".to_owned()],
        LuaValue::String(url) => vec![url.to_str()?.to_owned()],
        LuaValue::Table(list) => list.sequence_values::<String>().collect::<LuaResult<_>>()?,
        other => {
            return Err(LuaError::runtime(format!(
                "expected a NATS URL or a list of URLs, got {}",
                other.type_name()
            )));
        }
    };
    let addresses = servers
        .iter()
        .map(|server| {
            server
                .parse::<async_nats::ServerAddr>()
                .map_err(|error| LuaError::runtime(format!("invalid NATS URL '{server}': {error}")))
        })
        .collect::<LuaResult<Vec<_>>>()?;

    let emitter_slot: Arc<OnceLock<Emitter>> = Arc::new(OnceLock::new());
    let state = Arc::new(AtomicU8::new(CONNECTING));
    let mut connect_options = ConnectOptions::new()
        .name(options.get_or("Name", "quark".to_owned())?)
        .retry_on_initial_connect()
        .connection_timeout(Duration::from_millis(
            options.get_or("Timeout", DEFAULT_TIMEOUT)?,
        ));
    if let Some(token) = options.get::<String>("Token")? {
        connect_options = connect_options.token(token);
    }
    if let (Some(username), Some(password)) = (
        options.get::<String>("Username")?,
        options.get::<String>("Password")?,
    ) {
        connect_options = connect_options.user_and_password(username, password);
    }
    let ca: Option<String> = options.get("CA")?;
    let identity = quark_net::identity(&options)?;
    if options.get_or("TLS", false)? || ca.is_some() || identity.is_some() {
        let config = quark_net::client_config(ca.as_deref(), identity.as_ref(), &[])
            .map_err(LuaError::runtime)?;
        connect_options = connect_options
            .require_tls(true)
            .tls_client_config((*config).clone());
    }

    let callback_slot = Arc::clone(&emitter_slot);
    let callback_state = Arc::clone(&state);
    connect_options = connect_options.event_callback(move |event| {
        let slot = Arc::clone(&callback_slot);
        let state = Arc::clone(&callback_state);
        async move {
            let Some(emitter) = slot.get() else {
                return;
            };
            match event {
                Event::Connected => {
                    if state
                        .compare_exchange(CONNECTING, OPEN, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        emitter.emit("Connect", ());
                    }
                }
                Event::Disconnected => {
                    if state
                        .compare_exchange(OPEN, CONNECTING, Ordering::AcqRel, Ordering::Acquire)
                        .is_ok()
                    {
                        emitter.emit("Disconnect", "the connection to NATS was lost".to_owned());
                    }
                }
                Event::ServerError(error) => {
                    emitter.emit("Error", format!("server error: {error}"))
                }
                Event::ClientError(error) => {
                    emitter.emit("Error", format!("client error: {error}"))
                }
                Event::SlowConsumer(id) => emitter.emit(
                    "Error",
                    format!("subscription {id} is too slow, messages were dropped"),
                ),
                Event::LameDuckMode | Event::Draining | Event::Closed => {}
            }
        }
    });

    let client = match runtime().block_on(connect_options.connect(addresses)) {
        Ok(client) => client,
        Err(error) => {
            return (LuaNil, format!("cannot connect to NATS: {error}")).into_lua_multi(lua);
        }
    };
    let shared = Arc::new(Shared {
        state: Arc::clone(&state),
        client,
        emitter: OnceLock::new(),
    });
    let handle = Arc::new(Handle(Arc::clone(&shared)));
    let object = create_object(
        lua,
        NatsClient {
            shared: Arc::clone(&shared),
            _handle: Arc::clone(&handle),
        },
    )?;
    let emitter = Emitter::new(lua, &object)?;
    let _ = shared.emitter.set(emitter.clone());
    let _ = emitter_slot.set(emitter.clone());
    let weak: std::sync::Weak<dyn Resource> =
        Arc::downgrade(&handle) as std::sync::Weak<dyn Resource>;
    emitter.dispatcher().track(lua, weak)?;

    if shared.client.connection_state() == async_nats::connection::State::Connected
        && state
            .compare_exchange(CONNECTING, OPEN, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        emitter.emit("Connect", ());
    }
    object.into_lua_multi(lua)
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Connect", lua.create_function(connect)?)?;
    Ok(module)
}
