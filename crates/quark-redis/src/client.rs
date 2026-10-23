use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use mlua::prelude::*;
use quark_core::{Binary, Dispatcher, Options, Resource, create_object};
use redis::{Cmd, Value};
use tokio::sync::Mutex;

use crate::connection::{Connection, Endpoint};
use crate::reply::{Convert, Replies, Reply};

struct Shared {
    endpoint: Endpoint,
    connection: Mutex<Option<Connection>>,
    closed: AtomicBool,
}

impl Resource for Shared {
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        if let Ok(mut connection) = self.connection.try_lock() {
            connection.take();
        }
    }
}

impl Shared {
    async fn connection(&self) -> Result<Connection, String> {
        if self.closed.load(Ordering::Acquire) {
            return Err("the Redis client is closed".to_owned());
        }
        let mut connection = self.connection.lock().await;
        if let Some(connection) = connection.as_ref() {
            return Ok(connection.clone());
        }
        let connected = self.endpoint.managed().await?;
        *connection = Some(connected.clone());
        Ok(connected)
    }
}

pub struct RedisClient {
    shared: Arc<Shared>,
}

pub fn argument(value: LuaValue, lua: &Lua) -> LuaResult<Vec<u8>> {
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
    Method {
        name: "StreamLength",
        command: "XLEN",
        convert: Convert::Raw,
    },
    Method {
        name: "StreamDelete",
        command: "XDEL",
        convert: Convert::Raw,
    },
    Method {
        name: "StreamAck",
        command: "XACK",
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
            "StreamAdd",
            |lua,
             client,
             (key, fields, options, callback): (
                LuaValue,
                LuaTable,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let mut command = redis::cmd("XADD");
                command.arg(argument(key, lua)?);
                if let Some(max_length) = options.get::<u64>("MaxLength")? {
                    command.arg("MAXLEN");
                    if options.get_or("Approximate", true)? {
                        command.arg("~");
                    }
                    command.arg(max_length);
                }
                command.arg(options.get_or("ID", "*".to_owned())?);
                for (field, value) in stream_fields(lua, fields)? {
                    command.arg(field).arg(value);
                }
                execute(lua, client, command, Convert::Raw, callback)
            },
        );

        methods.add_method(
            "StreamRange",
            |lua,
             client,
             (key, start, stop, options, callback): (
                LuaValue,
                Option<String>,
                Option<String>,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let reverse = options.get_or("Reverse", false)?;
                let mut command = redis::cmd(if reverse { "XREVRANGE" } else { "XRANGE" });
                let (low, high) = (
                    start.unwrap_or_else(|| "-".to_owned()),
                    stop.unwrap_or_else(|| "+".to_owned()),
                );
                command.arg(argument(key, lua)?);
                if reverse {
                    command.arg(high).arg(low);
                } else {
                    command.arg(low).arg(high);
                }
                if let Some(count) = options.get::<u64>("Count")? {
                    command.arg("COUNT").arg(count);
                }
                execute(lua, client, command, Convert::Entries, callback)
            },
        );

        methods.add_method(
            "StreamRead",
            |lua,
             client,
             (streams, options, callback): (LuaValue, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let mut command = redis::cmd("XREAD");
                read_options(&mut command, &options)?;
                stream_arguments(lua, &mut command, streams, "$")?;
                execute(lua, client, command, Convert::Streams, callback)
            },
        );

        methods.add_method(
            "StreamReadGroup",
            |lua,
             client,
             (group, consumer, streams, options, callback): (
                String,
                String,
                LuaValue,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let mut command = redis::cmd("XREADGROUP");
                command.arg("GROUP").arg(group).arg(consumer);
                read_options(&mut command, &options)?;
                if options.get_or("NoAck", false)? {
                    command.arg("NOACK");
                }
                stream_arguments(lua, &mut command, streams, ">")?;
                execute(lua, client, command, Convert::Streams, callback)
            },
        );

        methods.add_method(
            "StreamCreateGroup",
            |lua, client, (key, group, options, callback): (LuaValue, String, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let mut command = redis::cmd("XGROUP");
                command
                    .arg("CREATE")
                    .arg(argument(key, lua)?)
                    .arg(group)
                    .arg(options.get_or("ID", "$".to_owned())?);
                if options.get_or("MakeStream", true)? {
                    command.arg("MKSTREAM");
                }
                let shared = Arc::clone(&client.shared);
                quark_core::run_async(lua, callback, async move {
                    let mut connection = shared.connection().await?;
                    match command.query_async::<Value>(&mut connection).await {
                        Ok(_) => Ok(Reply { value: Value::Boolean(true), convert: Convert::Raw }),
                        Err(error) if error.to_string().contains("BUSYGROUP") => {
                            Ok(Reply { value: Value::Boolean(false), convert: Convert::Raw })
                        }
                        Err(error) => Err(error.to_string()),
                    }
                })
            },
        );

        methods.add_method(
            "StreamPending",
            |lua, client, (key, group, callback): (LuaValue, String, Option<LuaFunction>)| {
                let mut command = redis::cmd("XPENDING");
                command.arg(argument(key, lua)?).arg(group);
                execute(lua, client, command, Convert::Pending, callback)
            },
        );

        methods.add_method(
            "StreamClaim",
            |lua,
             client,
             (key, group, consumer, min_idle, options, callback): (
                LuaValue,
                String,
                String,
                u64,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let mut command = redis::cmd("XAUTOCLAIM");
                command
                    .arg(argument(key, lua)?)
                    .arg(group)
                    .arg(consumer)
                    .arg(min_idle)
                    .arg(options.get_or("Start", "0-0".to_owned())?);
                if let Some(count) = options.get::<u64>("Count")? {
                    command.arg("COUNT").arg(count);
                }
                execute(lua, client, command, Convert::AutoClaim, callback)
            },
        );

        methods.add_method(
            "StreamTrim",
            |lua,
             client,
             (key, max_length, options, callback): (
                LuaValue,
                u64,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let mut command = redis::cmd("XTRIM");
                command.arg(argument(key, lua)?).arg("MAXLEN");
                if options.get_or("Approximate", false)? {
                    command.arg("~");
                }
                command.arg(max_length);
                execute(lua, client, command, Convert::Raw, callback)
            },
        );

        methods.add_method(
            "Pipeline",
            |lua, client, (commands, options, callback): (Vec<LuaTable>, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = quark_core::options_and_callback(options, callback)?;
                let atomic = options.get_or("Atomic", false)?;
                let mut built = Vec::with_capacity(commands.len());
                for entry in commands {
                    let mut values = entry.sequence_values::<LuaValue>().collect::<LuaResult<Vec<_>>>()?.into_iter();
                    let name = match values.next() {
                        Some(LuaValue::String(name)) => name.to_str()?.to_owned(),
                        _ => return Err(LuaError::runtime("every pipeline entry has to start with a command name")),
                    };
                    built.push(build_command(lua, &name, values.collect())?);
                }

                let shared = Arc::clone(&client.shared);
                let separate = shared.endpoint.is_cluster() && !atomic;
                quark_core::run_async(lua, callback, async move {
                    let connection = shared.connection().await?;
                    if separate {
                        let replies = futures_util::future::join_all(built.into_iter().map(|command| {
                            let mut connection = connection.clone();
                            async move { command.query_async::<Value>(&mut connection).await }
                        }))
                        .await;
                        let values = replies
                            .into_iter()
                            .collect::<Result<Vec<_>, _>>()
                            .map_err(|error| error.to_string())?;
                        return Ok::<_, String>(Replies(values));
                    }

                    let mut pipeline = redis::pipe();
                    if atomic {
                        pipeline.atomic();
                    }
                    for command in built {
                        pipeline.add_command(command);
                    }
                    let mut connection = connection;
                    let values: Vec<Value> = pipeline
                        .query_async(&mut connection)
                        .await
                        .map_err(|error| error.to_string())?;
                    Ok(Replies(values))
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

pub fn stream_fields(lua: &Lua, fields: LuaTable) -> LuaResult<Vec<(Vec<u8>, Vec<u8>)>> {
    let mut pairs = Vec::new();
    for pair in fields.pairs::<LuaValue, LuaValue>() {
        let (field, value) = pair?;
        pairs.push((argument(field, lua)?, argument(value, lua)?));
    }
    if pairs.is_empty() {
        return Err(LuaError::runtime("stream entries need at least one field"));
    }
    pairs.sort();
    Ok(pairs)
}

pub fn stream_list(
    lua: &Lua,
    streams: LuaValue,
    default_id: &str,
) -> LuaResult<Vec<(Vec<u8>, String)>> {
    let mut list = Vec::new();
    match streams {
        LuaValue::Table(table) if table.raw_len() > 0 => {
            for key in table.sequence_values::<LuaValue>() {
                list.push((argument(key?, lua)?, default_id.to_owned()));
            }
        }
        LuaValue::Table(table) => {
            for pair in table.pairs::<LuaValue, String>() {
                let (key, id) = pair?;
                list.push((argument(key, lua)?, id));
            }
        }
        other => list.push((argument(other, lua)?, default_id.to_owned())),
    }
    if list.is_empty() {
        return Err(LuaError::runtime("at least one stream is needed"));
    }
    list.sort();
    Ok(list)
}

fn stream_arguments(
    lua: &Lua,
    command: &mut Cmd,
    streams: LuaValue,
    default_id: &str,
) -> LuaResult<()> {
    let list = stream_list(lua, streams, default_id)?;
    command.arg("STREAMS");
    for (key, _) in &list {
        command.arg(key);
    }
    for (_, id) in &list {
        command.arg(id);
    }
    Ok(())
}

fn read_options(command: &mut Cmd, options: &Options) -> LuaResult<()> {
    if let Some(count) = options.get::<u64>("Count")? {
        command.arg("COUNT").arg(count);
    }
    if let Some(block) = options.duration("Block")?.map(|block| block.as_millis() as u64) {
        command.arg("BLOCK").arg(block);
    }
    Ok(())
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

pub fn connect(lua: &Lua, (target, options): (LuaValue, Options)) -> LuaResult<LuaAnyUserData> {
    let shared = Arc::new(Shared {
        endpoint: Endpoint::from_lua(target, &options)?,
        connection: Mutex::new(None),
        closed: AtomicBool::new(false),
    });
    let dispatcher = Dispatcher::get(lua)?;
    let weak: std::sync::Weak<dyn Resource> =
        Arc::downgrade(&shared) as std::sync::Weak<dyn Resource>;
    dispatcher.track(lua, weak)?;

    create_object(lua, RedisClient { shared })
}
