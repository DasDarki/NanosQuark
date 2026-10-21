use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use mlua::prelude::*;
use quark_core::{Bytes, Emitter, Options, Resource, create_object, runtime};
use tokio::sync::mpsc::{UnboundedSender, unbounded_channel};

use crate::connection::Endpoint;

const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSED: u8 = 2;
const RETRY_MIN: Duration = Duration::from_millis(500);
const RETRY_MAX: Duration = Duration::from_secs(10);

enum Command {
    Listen(Vec<String>, bool),
    Unlisten(Vec<String>, bool),
    Close,
}

struct Shared {
    state: AtomicU8,
    commands: UnboundedSender<Command>,
    channels: Mutex<BTreeSet<String>>,
    patterns: Mutex<BTreeSet<String>>,
}

impl Resource for Shared {
    fn close(&self) {
        let _ = self.commands.send(Command::Close);
    }
}

pub struct RedisSubscriber {
    shared: Arc<Shared>,
}

fn names(values: LuaMultiValue, lua: &Lua) -> LuaResult<Vec<String>> {
    let names = values
        .into_iter()
        .map(|value| String::from_lua(value, lua))
        .collect::<LuaResult<Vec<_>>>()?;
    if names.is_empty() {
        return Err(LuaError::runtime("at least one channel is needed"));
    }
    Ok(names)
}

impl LuaUserData for RedisSubscriber {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        quark_core::add_event_methods(methods);

        for (name, pattern, listen) in [
            ("Listen", false, true),
            ("Unlisten", false, false),
            ("ListenPattern", true, true),
            ("UnlistenPattern", true, false),
        ] {
            methods.add_method(name, move |lua, subscriber, values: LuaMultiValue| {
                let names = names(values, lua)?;
                let set = if pattern {
                    &subscriber.shared.patterns
                } else {
                    &subscriber.shared.channels
                };
                {
                    let mut set = set.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
                    for name in &names {
                        if listen {
                            set.insert(name.clone());
                        } else {
                            set.remove(name);
                        }
                    }
                }
                let command = if listen {
                    Command::Listen(names, pattern)
                } else {
                    Command::Unlisten(names, pattern)
                };
                let _ = subscriber.shared.commands.send(command);
                Ok(())
            });
        }

        methods.add_method("GetChannels", |lua, subscriber, ()| {
            let channels = subscriber
                .shared
                .channels
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            lua.create_sequence_from(channels)
        });
        methods.add_method("GetPatterns", |lua, subscriber, ()| {
            let patterns = subscriber
                .shared
                .patterns
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            lua.create_sequence_from(patterns)
        });
        methods.add_method("GetState", |_, subscriber, ()| {
            Ok(match subscriber.shared.state.load(Ordering::Acquire) {
                CONNECTING => "connecting",
                OPEN => "open",
                _ => "closed",
            })
        });
        methods.add_method("IsOpen", |_, subscriber, ()| {
            Ok(subscriber.shared.state.load(Ordering::Acquire) == OPEN)
        });
        methods.add_method("Close", |_, subscriber, ()| {
            subscriber.shared.close();
            Ok(())
        });
    }
}

fn snapshot(set: &Mutex<BTreeSet<String>>) -> Vec<String> {
    set.lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .iter()
        .cloned()
        .collect()
}

pub fn create(lua: &Lua, (target, options): (LuaValue, Options)) -> LuaResult<LuaAnyUserData> {
    let endpoint = Endpoint::from_lua(target, &options)?;
    let (commands, mut receiver) = unbounded_channel();
    let shared = Arc::new(Shared {
        state: AtomicU8::new(CONNECTING),
        commands,
        channels: Mutex::new(BTreeSet::new()),
        patterns: Mutex::new(BTreeSet::new()),
    });

    let object = create_object(
        lua,
        RedisSubscriber {
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
            match tokio::time::timeout(endpoint.timeout, endpoint.pubsub_client().get_async_pubsub()).await {
                Ok(Ok(pubsub)) => {
                    let (mut sink, mut stream) = pubsub.split();
                    let channels = snapshot(&shared.channels);
                    let patterns = snapshot(&shared.patterns);
                    let mut subscribed = Ok(());
                    if !channels.is_empty() {
                        subscribed = sink.subscribe(channels).await;
                    }
                    if subscribed.is_ok() && !patterns.is_empty() {
                        subscribed = sink.psubscribe(patterns).await;
                    }

                    match subscribed {
                        Err(error) => emitter.emit("Error", format!("cannot subscribe: {error}")),
                        Ok(()) => {
                            delay = RETRY_MIN;
                            shared.state.store(OPEN, Ordering::Release);
                            emitter.emit("Connect", ());

                            let ended = loop {
                                tokio::select! {
                                    message = stream.next() => match message {
                                        Some(message) => {
                                            let pattern = if message.from_pattern() {
                                                message.get_pattern::<String>().ok()
                                            } else {
                                                None
                                            };
                                            emitter.emit(
                                                "Message",
                                                (message.get_channel_name().to_owned(), Bytes(message.get_payload_bytes().to_vec()), pattern),
                                            );
                                        }
                                        None => break None,
                                    },
                                    command = receiver.recv() => {
                                        let result = match command {
                                            Some(Command::Listen(names, false)) => sink.subscribe(names).await,
                                            Some(Command::Listen(names, true)) => sink.psubscribe(names).await,
                                            Some(Command::Unlisten(names, false)) => sink.unsubscribe(names).await,
                                            Some(Command::Unlisten(names, true)) => sink.punsubscribe(names).await,
                                            Some(Command::Close) | None => break Some("closed".to_owned()),
                                        };
                                        if let Err(error) = result {
                                            emitter.emit("Error", error.to_string());
                                        }
                                    }
                                }
                            };

                            if let Some(reason) = ended {
                                break reason;
                            }
                            shared.state.store(CONNECTING, Ordering::Release);
                            emitter.emit("Disconnect", "the connection to Redis was lost".to_owned());
                        }
                    }
                }
                Ok(Err(error)) => emitter.emit("Error", format!("cannot connect to Redis: {error}")),
                Err(_) => emitter.emit("Error", "connecting to Redis timed out".to_owned()),
            }

            let deadline = tokio::time::Instant::now() + delay;
            let closed = loop {
                tokio::select! {
                    _ = tokio::time::sleep_until(deadline) => break false,
                    command = receiver.recv() => {
                        if matches!(command, Some(Command::Close) | None) {
                            break true;
                        }
                    }
                }
            };
            if closed {
                break "closed".to_owned();
            }
            delay = (delay * 2).min(RETRY_MAX);
        };

        shared.state.store(CLOSED, Ordering::Release);
        emitter.emit("Close", reason);
    });

    Ok(object)
}
