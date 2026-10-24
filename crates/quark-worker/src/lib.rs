use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use mlua::prelude::*;
use mlua::{HookTriggers, StdLib, VmState};
use quark_core::{Dispatcher, Options, Resource, create_userdata, options_and_callback};

const MAX_DEPTH: usize = 64;
const MAX_POOL_SIZE: usize = 64;
const DEFAULT_TIMEOUT: u64 = 10_000;
const DEFAULT_MEMORY: usize = 256 * 1024 * 1024;

#[derive(Clone, Debug)]
enum Value {
    Nil,
    Boolean(bool),
    Integer(i64),
    Number(f64),
    String(Vec<u8>),
    Table(Vec<(Value, Value)>),
}

fn capture(value: LuaValue, depth: usize) -> LuaResult<Value> {
    if depth > MAX_DEPTH {
        return Err(LuaError::runtime(
            "worker data is nested too deeply or contains a cycle",
        ));
    }
    Ok(match value {
        LuaNil => Value::Nil,
        LuaValue::Boolean(flag) => Value::Boolean(flag),
        LuaValue::Integer(integer) => Value::Integer(integer),
        LuaValue::Number(number) => Value::Number(number),
        LuaValue::String(text) => Value::String(text.as_bytes().to_vec()),
        LuaValue::Table(table) => {
            let mut entries = Vec::new();
            for pair in table.pairs::<LuaValue, LuaValue>() {
                let (key, value) = pair?;
                entries.push((capture(key, depth + 1)?, capture(value, depth + 1)?));
            }
            Value::Table(entries)
        }
        other => {
            return Err(LuaError::runtime(format!(
                "only nil, booleans, numbers, strings and tables can be passed to and from workers, got {}",
                other.type_name()
            )));
        }
    })
}

fn capture_all(values: LuaMultiValue) -> LuaResult<Vec<Value>> {
    values.into_iter().map(|value| capture(value, 0)).collect()
}

fn restore(lua: &Lua, value: Value) -> LuaResult<LuaValue> {
    Ok(match value {
        Value::Nil => LuaNil,
        Value::Boolean(flag) => LuaValue::Boolean(flag),
        Value::Integer(integer) => LuaValue::Integer(integer),
        Value::Number(number) => LuaValue::Number(number),
        Value::String(bytes) => LuaValue::String(lua.create_string(bytes)?),
        Value::Table(entries) => {
            let table = lua.create_table_with_capacity(0, entries.len())?;
            for (key, value) in entries {
                table.raw_set(restore(lua, key)?, restore(lua, value)?)?;
            }
            LuaValue::Table(table)
        }
    })
}

struct Values(Vec<Value>);

impl IntoLuaMulti for Values {
    fn into_lua_multi(self, lua: &Lua) -> LuaResult<LuaMultiValue> {
        self.0
            .into_iter()
            .map(|value| restore(lua, value))
            .collect::<LuaResult<Vec<_>>>()
            .map(LuaMultiValue::from_iter)
    }
}

#[derive(Clone, Copy)]
struct Limits {
    timeout: Duration,
    memory: usize,
}

fn sandbox(limits: Limits) -> LuaResult<Lua> {
    let lua = Lua::new_with(
        StdLib::STRING
            | StdLib::TABLE
            | StdLib::MATH
            | StdLib::UTF8
            | StdLib::COROUTINE
            | StdLib::OS,
        LuaOptions::default(),
    )?;
    lua.set_memory_limit(limits.memory)?;
    let globals = lua.globals();
    for name in ["dofile", "loadfile", "require", "collectgarbage"] {
        globals.raw_set(name, LuaNil)?;
    }
    let os: LuaTable = globals.get("os")?;
    let safe_os = lua.create_table()?;
    for name in ["time", "clock", "date", "difftime"] {
        safe_os.set(name, os.get::<LuaValue>(name)?)?;
    }
    globals.set("os", safe_os)?;
    globals.set(
        "print",
        lua.create_function(|_, values: LuaMultiValue| {
            let parts = values
                .iter()
                .map(|value| value.to_string().unwrap_or_default())
                .collect::<Vec<_>>();
            println!("[Quark Worker] {}", parts.join("\t"));
            Ok(())
        })?,
    )?;
    Ok(lua)
}

fn with_deadline(lua: &Lua, timeout: Duration, stop: Arc<AtomicBool>) -> LuaResult<()> {
    let deadline = Instant::now() + timeout;
    lua.set_hook(
        HookTriggers::new().every_nth_instruction(10_000),
        move |_, _| {
            if stop.load(Ordering::Relaxed) {
                return Err(LuaError::runtime("the worker was stopped"));
            }
            if Instant::now() > deadline {
                return Err(LuaError::runtime(format!(
                    "the worker took longer than {} ms",
                    timeout.as_millis()
                )));
            }
            Ok(VmState::Continue)
        },
    )
}

fn describe(error: LuaError) -> String {
    quark_core::describe(&error)
}

fn run_once(code: String, data: Value, limits: Limits) -> Result<Values, String> {
    let lua = sandbox(limits).map_err(describe)?;
    with_deadline(&lua, limits.timeout, Arc::new(AtomicBool::new(false))).map_err(describe)?;
    let chunk = lua
        .load(&code)
        .set_name("=worker")
        .into_function()
        .map_err(describe)?;
    let data = restore(&lua, data).map_err(describe)?;
    let results: LuaMultiValue = chunk.call(data).map_err(describe)?;
    capture_all(results).map(Values).map_err(describe)
}

fn read_limits(options: &Options) -> LuaResult<Limits> {
    let memory: usize = options.get_or("MemoryLimit", DEFAULT_MEMORY)?;
    if memory < 1024 * 1024 {
        return Err(LuaError::runtime("MemoryLimit has to be at least 1 MiB"));
    }
    Ok(Limits {
        timeout: options.duration_or("Timeout", DEFAULT_TIMEOUT)?,
        memory,
    })
}

type Reply = Box<dyn FnOnce(Result<Values, String>) + Send>;

struct Job {
    function: String,
    arguments: Vec<Value>,
    reply: Reply,
}

struct PoolShared {
    sender: Mutex<Option<Sender<Job>>>,
    stop: Arc<AtomicBool>,
    pending: AtomicUsize,
    size: usize,
}

impl Resource for PoolShared {
    fn close(&self) {
        self.stop.store(true, Ordering::Release);
        if let Ok(mut sender) = self.sender.lock() {
            sender.take();
        }
    }
}

pub struct Pool(Arc<PoolShared>);

fn pool_thread(
    code: String,
    limits: Limits,
    stop: Arc<AtomicBool>,
    jobs: Arc<Mutex<Receiver<Job>>>,
    ready: Sender<Result<(), String>>,
) {
    let prepared = (|| -> LuaResult<Lua> {
        let lua = sandbox(limits)?;
        with_deadline(&lua, limits.timeout, Arc::clone(&stop))?;
        lua.load(&code).set_name("=worker").exec()?;
        Ok(lua)
    })();
    let lua = match prepared {
        Ok(lua) => {
            let _ = ready.send(Ok(()));
            lua
        }
        Err(error) => {
            let _ = ready.send(Err(describe(error)));
            return;
        }
    };
    drop(ready);
    loop {
        let job = match jobs.lock() {
            Ok(receiver) => receiver.recv(),
            Err(_) => return,
        };
        let Ok(job) = job else {
            return;
        };
        let result = (|| -> LuaResult<Values> {
            with_deadline(&lua, limits.timeout, Arc::clone(&stop))?;
            let function: LuaFunction = lua
                .globals()
                .get::<Option<LuaFunction>>(job.function.as_str())?
                .ok_or_else(|| {
                    LuaError::runtime(format!(
                        "the worker has no global function '{}'",
                        job.function
                    ))
                })?;
            let arguments = job
                .arguments
                .into_iter()
                .map(|value| restore(&lua, value))
                .collect::<LuaResult<Vec<_>>>()?;
            let results: LuaMultiValue = function.call(LuaMultiValue::from_iter(arguments))?;
            capture_all(results).map(Values)
        })()
        .map_err(describe);
        (job.reply)(result);
        if stop.load(Ordering::Acquire) {
            return;
        }
    }
}

impl LuaUserData for Pool {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "Call",
            |lua, pool, (function, arguments): (String, LuaMultiValue)| {
                let mut arguments: Vec<LuaValue> = arguments.into_iter().collect();
                let callback = match arguments.last() {
                    Some(LuaValue::Function(_)) => match arguments.pop() {
                        Some(LuaValue::Function(callback)) => callback,
                        _ => unreachable!(),
                    },
                    _ => {
                        return Err(LuaError::runtime(
                            "Pool:Call needs a callback as last argument, or use Quark.Await",
                        ));
                    }
                };
                let arguments = arguments
                    .into_iter()
                    .map(|value| capture(value, 0))
                    .collect::<LuaResult<Vec<_>>>()?;
                let dispatcher = Dispatcher::get(lua)?;
                let callback = Arc::new(lua.create_registry_value(callback)?);
                let shared = Arc::clone(&pool.0);
                shared.pending.fetch_add(1, Ordering::AcqRel);
                let reply_shared = Arc::clone(&shared);
                let reply: Reply = Box::new(move |result| {
                    reply_shared.pending.fetch_sub(1, Ordering::AcqRel);
                    dispatcher.post(move |lua| {
                        let function: LuaFunction = lua.registry_value(&callback)?;
                        match result {
                            Ok(values) => function.call::<()>(values),
                            Err(message) => function.call::<()>((LuaNil, message)),
                        }
                    });
                });
                let sent = shared.sender.lock().ok().and_then(|sender| {
                    sender.as_ref().map(|sender| {
                        sender.send(Job {
                            function,
                            arguments,
                            reply,
                        })
                    })
                });
                match sent {
                    Some(Ok(())) => Ok(true),
                    _ => {
                        shared.pending.fetch_sub(1, Ordering::AcqRel);
                        Err(LuaError::runtime("the worker pool is closed"))
                    }
                }
            },
        );
        methods.add_method("GetSize", |_, pool, ()| Ok(pool.0.size));
        methods.add_method("GetPending", |_, pool, ()| {
            Ok(pool.0.pending.load(Ordering::Acquire))
        });
        methods.add_method("Close", |_, pool, ()| {
            pool.0.close();
            Ok(())
        });
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set(
        "Run",
        lua.create_function(
            |lua, (code, data, options, callback): (String, LuaValue, LuaValue, Option<LuaFunction>)| {
                let (data, options, callback) = match (data, options, callback) {
                    (LuaValue::Function(callback), LuaNil, None) => (LuaNil, LuaNil, Some(callback)),
                    (data, options, callback) => (data, options, callback),
                };
                let (options, callback) = options_and_callback(options, callback)?;
                let limits = read_limits(&options)?;
                let data = capture(data, 0)?;
                quark_core::run(lua, callback, move || run_once(code, data, limits))
            },
        )?,
    )?;
    module.set(
        "Pool",
        lua.create_function(
            |lua, (code, options, callback): (String, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                let _ = callback;
                let default_size = std::thread::available_parallelism()
                    .map(|count| count.get())
                    .unwrap_or(2)
                    .div_ceil(2);
                let size: usize = options.get_or("Size", default_size)?;
                if !(1..=MAX_POOL_SIZE).contains(&size) {
                    return Err(LuaError::runtime(format!(
                        "a pool has between 1 and {MAX_POOL_SIZE} workers"
                    )));
                }
                let limits = read_limits(&options)?;
                let stop = Arc::new(AtomicBool::new(false));
                let (sender, receiver) = mpsc::channel::<Job>();
                let receiver = Arc::new(Mutex::new(receiver));
                let (ready, readiness) = mpsc::channel();
                for index in 0..size {
                    let code = code.clone();
                    let stop = Arc::clone(&stop);
                    let receiver = Arc::clone(&receiver);
                    let ready = ready.clone();
                    thread::Builder::new()
                        .name(format!("quark-lua-{index}"))
                        .spawn(move || pool_thread(code, limits, stop, receiver, ready))
                        .map_err(|error| {
                            LuaError::runtime(format!("cannot start a worker thread: {error}"))
                        })?;
                }
                drop(ready);
                for _ in 0..size {
                    match readiness.recv() {
                        Ok(Ok(())) => {}
                        Ok(Err(message)) => {
                            stop.store(true, Ordering::Release);
                            return (LuaNil, message).into_lua_multi(lua);
                        }
                        Err(_) => {
                            return (LuaNil, "a worker thread failed to start").into_lua_multi(lua);
                        }
                    }
                }
                let shared = Arc::new(PoolShared {
                    sender: Mutex::new(Some(sender)),
                    stop,
                    pending: AtomicUsize::new(0),
                    size,
                });
                let weak: std::sync::Weak<dyn Resource> =
                    Arc::downgrade(&shared) as std::sync::Weak<dyn Resource>;
                Dispatcher::get(lua)?.track(lua, weak)?;
                create_userdata(lua, Pool(shared))?.into_lua_multi(lua)
            },
        )?,
    )?;
    Ok(module)
}
