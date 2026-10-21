use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use mlua::prelude::*;
use quark_core::{Emitter, Options, Resource, create_object, runtime};
use redis::{Cmd, Value};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::client::stream_list;
use crate::connection::Endpoint;
use crate::reply::{fields_table, parse_streams};

const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSED: u8 = 2;
const RETRY_MIN: Duration = Duration::from_millis(500);
const RETRY_MAX: Duration = Duration::from_secs(10);
const DEFAULT_COUNT: u64 = 100;
const DEFAULT_BLOCK: u64 = 2_000;

struct Shared {
    state: AtomicU8,
    commands: UnboundedSender<()>,
}

impl Resource for Shared {
    fn close(&self) {
        let _ = self.commands.send(());
    }
}

pub struct StreamReader {
    shared: Arc<Shared>,
}

struct Fields(Vec<(Vec<u8>, Vec<u8>)>);

impl IntoLua for Fields {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        fields_table(lua, self.0).map(LuaValue::Table)
    }
}

struct Group {
    name: String,
    consumer: String,
    create: bool,
    start: String,
    auto_ack: bool,
}

impl LuaUserData for StreamReader {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        quark_core::add_event_methods(methods);
        methods.add_method("GetState", |_, reader, ()| {
            Ok(match reader.shared.state.load(Ordering::Acquire) {
                CONNECTING => "connecting",
                OPEN => "open",
                _ => "closed",
            })
        });
        methods.add_method("IsOpen", |_, reader, ()| {
            Ok(reader.shared.state.load(Ordering::Acquire) == OPEN)
        });
        methods.add_method("Close", |_, reader, ()| {
            reader.shared.close();
            Ok(())
        });
    }
}

pub fn create(lua: &Lua, (target, options): (LuaValue, Options)) -> LuaResult<LuaAnyUserData> {
    let endpoint = Endpoint::from_lua(target, &options)?;
    let streams = options
        .get::<LuaValue>("Streams")?
        .ok_or_else(|| LuaError::runtime("the Streams option is required"))?;

    let group = match options.get::<String>("Group")? {
        Some(name) => Some(Group {
            name,
            consumer: options
                .get::<String>("Consumer")?
                .ok_or_else(|| LuaError::runtime("readers with a Group need a Consumer name"))?,
            create: options.get_or("CreateGroup", true)?,
            start: options.get_or("Start", "$".to_owned())?,
            auto_ack: options.get_or("AutoAck", false)?,
        }),
        None => None,
    };
    let default_id = match &group {
        Some(_) => ">".to_owned(),
        None => options.get_or("Start", "$".to_owned())?,
    };
    let mut streams = stream_list(lua, streams, &default_id)?;
    if group.is_some() {
        for (_, id) in &mut streams {
            *id = ">".to_owned();
        }
    }
    let count = options.get_or("Count", DEFAULT_COUNT)?;
    let block = options.get_or("Block", DEFAULT_BLOCK)?;

    let (commands, mut receiver) = unbounded_channel();
    let shared = Arc::new(Shared {
        state: AtomicU8::new(CONNECTING),
        commands,
    });
    let object = create_object(
        lua,
        StreamReader {
            shared: Arc::clone(&shared),
        },
    )?;
    let emitter = Emitter::new(lua, &object)?;
    let weak: std::sync::Weak<dyn Resource> =
        Arc::downgrade(&shared) as std::sync::Weak<dyn Resource>;
    emitter.dispatcher().track(lua, weak)?;

    runtime().spawn(async move {
        let mut delay = RETRY_MIN;
        let reason = loop {
            match endpoint.dedicated().await {
                Ok(mut connection) => {
                    if let Some(group) = group.as_ref().filter(|group| group.create) {
                        for (key, _) in &streams {
                            let mut command = redis::cmd("XGROUP");
                            command.arg("CREATE").arg(key).arg(&group.name).arg(&group.start).arg("MKSTREAM");
                            if let Err(error) = command.query_async::<Value>(&mut connection).await
                                && !error.to_string().contains("BUSYGROUP")
                            {
                                emitter.emit("Error", format!("cannot create the consumer group: {error}"));
                            }
                        }
                    }

                    delay = RETRY_MIN;
                    shared.state.store(OPEN, Ordering::Release);
                    emitter.emit("Connect", ());

                    let closed = loop {
                        let mut command = match &group {
                            Some(group) => {
                                let mut command = redis::cmd("XREADGROUP");
                                command.arg("GROUP").arg(&group.name).arg(&group.consumer);
                                command
                            }
                            None => redis::cmd("XREAD"),
                        };
                        command.arg("COUNT").arg(count).arg("BLOCK").arg(block).arg("STREAMS");
                        for (key, _) in &streams {
                            command.arg(key);
                        }
                        for (_, id) in &streams {
                            command.arg(id);
                        }

                        tokio::select! {
                            result = command.query_async::<Value>(&mut connection) => match result {
                                Ok(value) => {
                                    for (name, entries) in parse_streams(value) {
                                        let mut acknowledge = Cmd::new();
                                        let mut pending = 0;
                                        for entry in entries {
                                            if group.is_none()
                                                && let Some((_, id)) = streams.iter_mut().find(|(key, _)| *key == name.as_bytes())
                                            {
                                                *id = entry.id.clone();
                                            }
                                            if group.as_ref().is_some_and(|group| group.auto_ack) {
                                                if pending == 0 {
                                                    acknowledge = redis::cmd("XACK");
                                                    acknowledge.arg(&name).arg(group.as_ref().map(|group| group.name.as_str()).unwrap_or_default());
                                                }
                                                acknowledge.arg(&entry.id);
                                                pending += 1;
                                            }
                                            emitter.emit("Entry", (name.clone(), entry.id, Fields(entry.fields)));
                                        }
                                        if pending > 0
                                            && let Err(error) = acknowledge.query_async::<Value>(&mut connection).await
                                        {
                                            emitter.emit("Error", format!("cannot acknowledge entries: {error}"));
                                        }
                                    }
                                }
                                Err(error) => {
                                    emitter.emit("Error", error.to_string());
                                    break false;
                                }
                            },
                            _ = receiver.recv() => break true,
                        }
                    };

                    if closed {
                        break "closed".to_owned();
                    }
                    shared.state.store(CONNECTING, Ordering::Release);
                    emitter.emit("Disconnect", "reading the stream failed".to_owned());
                }
                Err(error) => emitter.emit("Error", error),
            }

            let deadline = tokio::time::Instant::now() + delay;
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => {}
                _ = receiver.recv() => break "closed".to_owned(),
            }
            delay = (delay * 2).min(RETRY_MAX);
        };

        shared.state.store(CLOSED, Ordering::Release);
        emitter.emit("Close", reason);
    });

    Ok(object)
}
