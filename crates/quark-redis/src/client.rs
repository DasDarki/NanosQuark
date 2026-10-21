use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use mlua::prelude::*;
use quark_core::{Binary, Dispatcher, Options, Resource, create_object};
use redis::aio::{ConnectionManager, ConnectionManagerConfig};
use redis::{Cmd, Value};
use tokio::sync::Mutex;

use crate::reply::{Convert, Replies, Reply};
use crate::settings;

struct Shared {
    client: redis::Client,
    config: ConnectionManagerConfig,
    manager: Mutex<Option<ConnectionManager>>,
    closed: AtomicBool,
}

impl Resource for Shared {
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(mut manager) = self.manager.try_lock() {
            manager.take();
        }
    }
}

impl Shared {
    async fn connection(&self) -> Result<ConnectionManager, String> {
        if self.closed.load(Ordering::Acquire) {
            return Err("the Redis client is closed".to_owned());
        }
        let mut manager = self.manager.lock().await;
        if let Some(manager) = manager.as_ref() {
            return Ok(manager.clone());
        }
        let connected =
            ConnectionManager::new_with_config(self.client.clone(), self.config.clone())
                .await
                .map_err(|error| format!("cannot connect to Redis: {error}"))?;
        *manager = Some(connected.clone());
        Ok(connected)
    }
}

pub struct RedisClient {
    shared: Arc<Shared>,
}

fn argument(value: LuaValue, lua: &Lua) -> LuaResult<Vec<u8>> {
    match value {
        LuaValue::Boolean(_) | LuaNil => Err(LuaError::runtime(format!(
            "Redis arguments have to be strings, numbers or buffers, got {}",
            value.type_name()
        ))),
        LuaValue::Number(number) if number.fract() == 0.0 && number.abs() < 9.0e15 => {
            Ok((number as i64).to_string().into_bytes())
        }
        other => Binary::from_lua(other, lua).map(|binary| binary.0),
    }
}

fn split_callback(mut values: Vec<LuaValue>) -> (Vec<LuaValue>, Option<LuaFunction>) {
    match values.last() {
        Some(LuaValue::Function(_)) => {
            let Some(LuaValue::Function(callback)) = values.pop() else {
                unreachable!()
            };
            (values, Some(callback))
        }
        _ => (values, None),
    }
}

fn build_command(lua: &Lua, name: &str, arguments: Vec<LuaValue>) -> LuaResult<Cmd> {
    let mut command = redis::cmd(name);
    for value in arguments {
        command.arg(argument(value, lua)?);
    }
    Ok(command)
}

fn execute(
    lua: &Lua,
    client: &RedisClient,
    command: Cmd,
    convert: Convert,
    callback: Option<LuaFunction>,
) -> LuaResult<LuaMultiValue> {
    let shared = Arc::clone(&client.shared);
    quark_core::run_async(lua, callback, async move {
        let mut connection = shared.connection().await?;
        let value: Value = command
            .query_async(&mut connection)
            .await
            .map_err(|error| error.to_string())?;
        Ok::<_, String>(Reply { value, convert })
    })
}

struct Method {
    name: &'static str,
    command: &'static str,
    convert: Convert,
}

const METHODS: &[Method] = &[
    Method {
        name: "Get",
        command: "GET",
        convert: Convert::Raw,
    },
    Method {
        name: "Delete",
        command: "DEL",
        convert: Convert::Raw,
    },
    Method {
        name: "Exists",
        command: "EXISTS",
        convert: Convert::Boolean,
    },
    Method {
        name: "Expire",
        command: "EXPIRE",
        convert: Convert::Boolean,
    },
    Method {
        name: "TTL",
        command: "TTL",
        convert: Convert::Raw,
    },
    Method {
        name: "HashGet",
        command: "HGET",
        convert: Convert::Raw,
    },
    Method {
        name: "HashGetAll",
        command: "HGETALL",
        convert: Convert::Pairs,
    },
    Method {
        name: "HashDelete",
        command: "HDEL",
        convert: Convert::Raw,
    },
    Method {
        name: "HashExists",
        command: "HEXISTS",
        convert: Convert::Boolean,
    },
    Method {
        name: "ListPush",
        command: "RPUSH",
        convert: Convert::Raw,
    },
    Method {
        name: "ListPushFront",
        command: "LPUSH",
        convert: Convert::Raw,
    },
    Method {
        name: "ListPop",
        command: "LPOP",
        convert: Convert::Raw,
    },
    Method {
        name: "ListPopBack",
        command: "RPOP",
        convert: Convert::Raw,
    },
    Method {
        name: "ListRange",
        command: "LRANGE",
        convert: Convert::Raw,
    },
    Method {
        name: "ListLength",
        command: "LLEN",
        convert: Convert::Raw,
    },
    Method {
        name: "SetAdd",
        command: "SADD",
        convert: Convert::Raw,
    },
    Method {
        name: "SetRemove",
        command: "SREM",
        convert: Convert::Raw,
    },
    Method {
        name: "SetMembers",
        command: "SMEMBERS",
        convert: Convert::Raw,
    },
    Method {
        name: "SetContains",
        command: "SISMEMBER",
        convert: Convert::Boolean,
    },
    Method {
        name: "SortedSetScore",
        command: "ZSCORE",
        convert: Convert::Number,
    },
    Method {
        name: "SortedSetRemove",
        command: "ZREM",
        convert: Convert::Raw,
    },
    Method {
        name: "SortedSetCount",
        command: "ZCARD",
        convert: Convert::Raw,
    },
    Method {
        name: "Publish",
        command: "PUBLISH",
        convert: Convert::Raw,
    },
];

impl LuaUserData for RedisClient {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "Command",
            |lua, client, (name, arguments): (String, LuaMultiValue)| {
                let (arguments, callback) = split_callback(arguments.into_vec());
                let command = build_command(lua, &name, arguments)?;
                execute(lua, client, command, Convert::Raw, callback)
            },
        );

        for method in METHODS {
            methods.add_method(method.name, move |lua, client, arguments: LuaMultiValue| {
                let (arguments, callback) = split_callback(arguments.into_vec());
                let command = build_command(lua, method.command, arguments)?;
                execute(lua, client, command, method.convert, callback)
            });
        }

        methods.add_method(
            "Set",
            |lua,
             client,
             (key, value, options, callback): (
                LuaValue,
                LuaValue,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let mut command = redis::cmd("SET");
                command.arg(argument(key, lua)?).arg(argument(value, lua)?);
                if let Some(seconds) = options.get::<u64>("EX")? {
                    command.arg("EX").arg(seconds);
                }
                if let Some(milliseconds) = options.get::<u64>("PX")? {
                    command.arg("PX").arg(milliseconds);
                }
                if options.get_or("NX", false)? {
                    command.arg("NX");
                }
                if options.get_or("XX", false)? {
                    command.arg("XX");
                }
                if options.get_or("KeepTTL", false)? {
                    command.arg("KEEPTTL");
                }
                execute(lua, client, command, Convert::Boolean, callback)
            },
        );

        methods.add_method(
            "Increment",
            |lua, client, (key, by, callback): (LuaValue, LuaValue, Option<LuaFunction>)| {
                let (by, callback) = amount(by, callback)?;
                let mut command = redis::cmd("INCRBY");
                command.arg(argument(key, lua)?).arg(by);
                execute(lua, client, command, Convert::Raw, callback)
            },
        );

        methods.add_method(
            "Decrement",
            |lua, client, (key, by, callback): (LuaValue, LuaValue, Option<LuaFunction>)| {
                let (by, callback) = amount(by, callback)?;
                let mut command = redis::cmd("DECRBY");
                command.arg(argument(key, lua)?).arg(by);
                execute(lua, client, command, Convert::Raw, callback)
            },
        );

        methods.add_method("HashSet", |lua, client, arguments: LuaMultiValue| {
            let (arguments, callback) = split_callback(arguments.into_vec());
            let mut arguments = arguments.into_iter();
            let key = arguments
                .next()
                .ok_or_else(|| LuaError::runtime("HashSet needs a key"))?;
            let mut command = redis::cmd("HSET");
            command.arg(argument(key, lua)?);
            match (arguments.next(), arguments.next()) {
                (Some(LuaValue::Table(fields)), None) => {
                    let mut pairs = Vec::new();
                    for pair in fields.pairs::<LuaValue, LuaValue>() {
                        let (field, value) = pair?;
                        pairs.push((argument(field, lua)?, argument(value, lua)?));
                    }
                    if pairs.is_empty() {
                        return Err(LuaError::runtime("HashSet needs at least one field"));
                    }
                    pairs.sort();
                    for (field, value) in pairs {
                        command.arg(field).arg(value);
                    }
                }
                (Some(field), Some(value)) => {
                    command
                        .arg(argument(field, lua)?)
                        .arg(argument(value, lua)?);
                }
                _ => {
                    return Err(LuaError::runtime(
                        "HashSet needs a field and a value or a table of fields",
                    ));
                }
            }
            execute(lua, client, command, Convert::Raw, callback)
        });

        methods.add_method(
            "HashIncrement",
            |lua, client, (key, field, by, callback): (LuaValue, LuaValue, LuaValue, Option<LuaFunction>)| {
                let (by, callback) = amount(by, callback)?;
                let mut command = redis::cmd("HINCRBY");
                command.arg(argument(key, lua)?).arg(argument(field, lua)?).arg(by);
                execute(lua, client, command, Convert::Raw, callback)
            },
        );

        methods.add_method(
            "SortedSetAdd",
            |lua, client, (key, member, score, callback): (LuaValue, LuaValue, f64, Option<LuaFunction>)| {
                let mut command = redis::cmd("ZADD");
                command.arg(argument(key, lua)?).arg(score).arg(argument(member, lua)?);
                execute(lua, client, command, Convert::Raw, callback)
            },
        );

        methods.add_method(
            "SortedSetIncrement",
            |lua, client, (key, member, by, callback): (LuaValue, LuaValue, f64, Option<LuaFunction>)| {
                let mut command = redis::cmd("ZINCRBY");
                command.arg(argument(key, lua)?).arg(by).arg(argument(member, lua)?);
                execute(lua, client, command, Convert::Number, callback)
            },
        );

        methods.add_method(
            "SortedSetRank",
            |lua,
             client,
             (key, member, options, callback): (
                LuaValue,
                LuaValue,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let name = if options.get_or("Reverse", false)? {
                    "ZREVRANK"
                } else {
                    "ZRANK"
                };
                let mut command = redis::cmd(name);
                command.arg(argument(key, lua)?).arg(argument(member, lua)?);
                execute(lua, client, command, Convert::Raw, callback)
            },
        );

        methods.add_method(
            "SortedSetRange",
            |lua,
             client,
             (key, start, stop, options, callback): (
                LuaValue,
                i64,
                i64,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let mut command = redis::cmd("ZRANGE");
                command.arg(argument(key, lua)?).arg(start).arg(stop);
                if options.get_or("Reverse", false)? {
                    command.arg("REV");
                }
                let with_scores = options.get_or("WithScores", false)?;
                if with_scores {
                    command.arg("WITHSCORES");
                }
                let convert = if with_scores {
                    Convert::Scores
                } else {
                    Convert::Raw
                };
                execute(lua, client, command, convert, callback)
            },
        );

        methods.add_method(
            "Pipeline",
            |lua, client, (commands, options, callback): (Vec<LuaTable>, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let mut pipeline = redis::pipe();
                if options.get_or("Atomic", false)? {
                    pipeline.atomic();
                }
                for entry in commands {
                    let mut values = entry.sequence_values::<LuaValue>().collect::<LuaResult<Vec<_>>>()?.into_iter();
                    let name = match values.next() {
                        Some(LuaValue::String(name)) => name.to_str()?.to_owned(),
                        _ => return Err(LuaError::runtime("every pipeline entry has to start with a command name")),
                    };
                    pipeline.add_command(build_command(lua, &name, values.collect())?);
                }

                let shared = Arc::clone(&client.shared);
                quark_core::run_async(lua, callback, async move {
                    let mut connection = shared.connection().await?;
                    let values: Vec<Value> = pipeline
                        .query_async(&mut connection)
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok::<_, String>(Replies(values))
                })
            },
        );

        methods.add_method("Close", |_, client, ()| {
            client.shared.close();
            Ok(())
        });

        methods.add_method("IsClosed", |_, client, ()| {
            Ok(client.shared.closed.load(Ordering::Acquire))
        });

        methods.add_meta_method(LuaMetaMethod::ToString, |_, client, ()| {
            Ok(if client.shared.closed.load(Ordering::Acquire) {
                "RedisClient(closed)"
            } else {
                "RedisClient(open)"
            })
        });
    }
}

fn amount(by: LuaValue, callback: Option<LuaFunction>) -> LuaResult<(i64, Option<LuaFunction>)> {
    match (by, callback) {
        (LuaValue::Function(callback), None) => Ok((1, Some(callback))),
        (LuaNil, callback) => Ok((1, callback)),
        (LuaValue::Integer(by), callback) => Ok((by, callback)),
        (LuaValue::Number(by), callback) if by.fract() == 0.0 => Ok((by as i64, callback)),
        (other, _) => Err(LuaError::runtime(format!(
            "expected an integer amount or a callback, got {}",
            other.type_name()
        ))),
    }
}

pub fn connect(lua: &Lua, (url, options): (Option<String>, Options)) -> LuaResult<LuaAnyUserData> {
    let settings = settings(url, &options)?;
    let config = ConnectionManagerConfig::new()
        .set_connection_timeout(Some(settings.timeout))
        .set_response_timeout(Some(settings.timeout))
        .set_number_of_retries(options.get_or("Retries", 6)?);

    let shared = Arc::new(Shared {
        client: settings.client,
        config,
        manager: Mutex::new(None),
        closed: AtomicBool::new(false),
    });
    let dispatcher = Dispatcher::get(lua)?;
    let weak: std::sync::Weak<dyn Resource> =
        Arc::downgrade(&shared) as std::sync::Weak<dyn Resource>;
    dispatcher.track(lua, weak)?;

    create_object(lua, RedisClient { shared })
}
