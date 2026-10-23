use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mlua::prelude::*;
use quark_core::{Binary, Bytes, Emitter, Options, Resource, create_object, runtime};
use rumqttc::{
    AsyncClient, ConnectionError, Event, EventLoop, LastWill, MqttOptions, Packet, QoS,
    TlsConfiguration, Transport,
};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};
use url::Url;

const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSED: u8 = 2;
const RETRY_MIN: Duration = Duration::from_millis(500);
const RETRY_MAX: Duration = Duration::from_secs(10);
const QUEUE_SIZE: usize = 1024;

struct Shared {
    state: AtomicU8,
    client: AsyncClient,
    close: UnboundedSender<()>,
    topics: Mutex<BTreeMap<String, QoS>>,
    client_id: String,
}

impl Resource for Shared {
    fn close(&self) {
        let _ = self.close.send(());
    }
}

pub struct MqttClient {
    shared: Arc<Shared>,
}

fn quality(level: Option<u8>) -> LuaResult<QoS> {
    match level.unwrap_or(0) {
        0 => Ok(QoS::AtMostOnce),
        1 => Ok(QoS::AtLeastOnce),
        2 => Ok(QoS::ExactlyOnce),
        other => Err(LuaError::runtime(format!(
            "QoS has to be 0, 1 or 2, got {other}"
        ))),
    }
}

fn quality_level(qos: QoS) -> u8 {
    match qos {
        QoS::AtMostOnce => 0,
        QoS::AtLeastOnce => 1,
        QoS::ExactlyOnce => 2,
    }
}

impl LuaUserData for MqttClient {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        quark_core::add_event_methods(methods);

        methods.add_method("Listen", |_, client, (topic, qos): (String, Option<u8>)| {
            let qos = quality(qos)?;
            client
                .shared
                .topics
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .insert(topic.clone(), qos);
            client
                .shared
                .client
                .try_subscribe(topic, qos)
                .map_err(|error| LuaError::runtime(format!("cannot subscribe: {error}")))
        });
        methods.add_method("Unlisten", |_, client, topic: String| {
            client
                .shared
                .topics
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(&topic);
            client
                .shared
                .client
                .try_unsubscribe(topic)
                .map_err(|error| LuaError::runtime(format!("cannot unsubscribe: {error}")))
        });
        methods.add_method(
            "Publish",
            |lua, client, (topic, payload, options): (String, Binary, Options)| {
                let qos = quality(options.get("QoS")?)?;
                let retain = options.get_or("Retain", false)?;
                match client
                    .shared
                    .client
                    .try_publish(topic, qos, retain, payload.0)
                {
                    Ok(()) => true.into_lua_multi(lua),
                    Err(error) => (LuaNil, format!("cannot publish: {error}")).into_lua_multi(lua),
                }
            },
        );
        methods.add_method("GetTopics", |lua, client, ()| {
            let topics = client
                .shared
                .topics
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            lua.create_sequence_from(topics)
        });
        methods.add_method("GetClientID", |_, client, ()| {
            Ok(client.shared.client_id.clone())
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
        methods.add_meta_method(LuaMetaMethod::ToString, |_, client, ()| {
            Ok(format!("MQTTClient({})", client.shared.client_id))
        });
    }
}

fn describe(error: &ConnectionError) -> String {
    match error {
        ConnectionError::ConnectionRefused(code) => {
            format!("the broker refused the connection: {code:?}")
        }
        other => other.to_string(),
    }
}

fn options_for(url: &str, options: &Options) -> LuaResult<(MqttOptions, String)> {
    let parsed = Url::parse(url)
        .map_err(|error| LuaError::runtime(format!("invalid MQTT URL '{url}': {error}")))?;
    let secure = match parsed.scheme() {
        "mqtt" | "tcp" => options.get_or("TLS", false)?,
        "mqtts" | "ssl" => true,
        other => {
            return Err(LuaError::runtime(format!(
                "unsupported MQTT URL scheme '{other}', expected mqtt:// or mqtts://"
            )));
        }
    };
    let host = parsed
        .host_str()
        .ok_or_else(|| LuaError::runtime(format!("the MQTT URL '{url}' has no host")))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let port = parsed.port().unwrap_or(if secure { 8883 } else { 1883 });
    let client_id = options
        .get::<String>("ClientID")?
        .unwrap_or_else(|| format!("quark-{}", crate::random_suffix()));

    let mut mqtt = MqttOptions::new(client_id.clone(), host, port);
    mqtt.set_keep_alive(Duration::from_secs(options.get_or("KeepAlive", 30)?));
    mqtt.set_clean_session(options.get_or("CleanSession", true)?);
    let max_packet_size = options.get_or("MaxPacketSize", 1024 * 1024)?;
    mqtt.set_max_packet_size(max_packet_size, max_packet_size);

    let username = options
        .get::<String>("Username")?
        .or_else(|| (!parsed.username().is_empty()).then(|| parsed.username().to_owned()));
    let password = options
        .get::<String>("Password")?
        .or_else(|| parsed.password().map(str::to_owned));
    if let Some(username) = username {
        mqtt.set_credentials(username, password.unwrap_or_default());
    }

    if let Some(will) = options.get::<LuaTable>("Will")? {
        let topic: String = will
            .get::<Option<String>>("Topic")?
            .ok_or_else(|| LuaError::runtime("the Will option needs a Topic"))?;
        let payload = match will.get::<LuaValue>("Payload")? {
            LuaNil => Vec::new(),
            value => {
                Binary::from_lua(
                    value,
                    &will
                        .weak_lua()
                        .try_upgrade()
                        .ok_or_else(|| LuaError::runtime("the Lua state is gone"))?,
                )?
                .0
            }
        };
        mqtt.set_last_will(LastWill::new(
            topic,
            payload,
            quality(will.get("QoS")?)?,
            will.get::<Option<bool>>("Retain")?.unwrap_or(false),
        ));
    }

    if secure {
        let ca: Option<String> = options.get("CA")?;
        let identity = quark_net::identity(options)?;
        let config = quark_net::client_config(ca.as_deref(), identity.as_ref(), &[])
            .map_err(LuaError::runtime)?;
        mqtt.set_transport(Transport::Tls(TlsConfiguration::Rustls(config)));
    }
    Ok((mqtt, client_id))
}

async fn drive(
    mut eventloop: EventLoop,
    shared: Arc<Shared>,
    emitter: Emitter,
    mut close: tokio::sync::mpsc::UnboundedReceiver<()>,
) {
    let mut delay = RETRY_MIN;
    let reason = loop {
        tokio::select! {
            event = eventloop.poll() => match event {
                Ok(Event::Incoming(Packet::ConnAck(ack))) => {
                    delay = RETRY_MIN;
                    shared.state.store(OPEN, Ordering::Release);
                    let topics = shared
                        .topics
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .clone();
                    for (topic, qos) in topics {
                        let _ = shared.client.try_subscribe(topic, qos);
                    }
                    emitter.emit("Connect", ack.session_present);
                }
                Ok(Event::Incoming(Packet::Publish(publish))) => {
                    let topic = publish.topic.clone();
                    let qos = quality_level(publish.qos);
                    let retain = publish.retain;
                    let duplicate = publish.dup;
                    emitter.with(move |lua, object| {
                        let details = lua.create_table()?;
                        details.set("QoS", qos)?;
                        details.set("Retain", retain)?;
                        details.set("Duplicate", duplicate)?;
                        quark_core::emit_now(lua, object, "Message", (topic, Bytes(publish.payload.to_vec()), details))
                    });
                }
                Ok(_) => {}
                Err(error) => {
                    let message = describe(&error);
                    if shared.state.swap(CONNECTING, Ordering::AcqRel) == OPEN {
                        emitter.emit("Disconnect", message);
                    } else {
                        emitter.emit("Error", format!("cannot connect to the MQTT broker: {message}"));
                    }
                    tokio::select! {
                        _ = tokio::time::sleep(delay) => {}
                        _ = close.recv() => break "closed".to_owned(),
                    }
                    delay = (delay * 2).min(RETRY_MAX);
                }
            },
            _ = close.recv() => {
                if shared.state.load(Ordering::Acquire) == OPEN {
                    let _ = shared.client.try_disconnect();
                    let _ = tokio::time::timeout(Duration::from_secs(1), async {
                        while eventloop.poll().await.is_ok() {}
                    })
                    .await;
                }
                break "closed".to_owned();
            }
        }
    };
    shared.state.store(CLOSED, Ordering::Release);
    emitter.emit("Close", reason);
}

fn connect(lua: &Lua, (url, options): (String, Options)) -> LuaResult<LuaAnyUserData> {
    let (mqtt, client_id) = options_for(&url, &options)?;
    let (client, eventloop) = AsyncClient::new(mqtt, QUEUE_SIZE);
    let (close, close_receiver) = unbounded_channel();
    let shared = Arc::new(Shared {
        state: AtomicU8::new(CONNECTING),
        client,
        close,
        topics: Mutex::new(BTreeMap::new()),
        client_id,
    });

    let object = create_object(
        lua,
        MqttClient {
            shared: Arc::clone(&shared),
        },
    )?;
    let emitter = Emitter::new(lua, &object)?;
    let weak: std::sync::Weak<dyn Resource> =
        Arc::downgrade(&shared) as std::sync::Weak<dyn Resource>;
    emitter.dispatcher().track(lua, weak)?;
    runtime().spawn(drive(eventloop, shared, emitter, close_receiver));
    Ok(object)
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Connect", lua.create_function(connect)?)?;
    module.set(
        "TopicMatches",
        lua.create_function(|_, (filter, topic): (String, String)| {
            Ok(rumqttc::matches(&topic, &filter))
        })?,
    )?;
    Ok(module)
}
