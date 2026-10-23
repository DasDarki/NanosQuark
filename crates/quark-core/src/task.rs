use std::fmt::Display;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use mlua::prelude::*;
use tokio::task::{AbortHandle, JoinHandle};

use crate::{Dispatcher, create_userdata, runtime};

const PENDING: u8 = 0;
const DONE: u8 = 1;
const CANCELLED: u8 = 2;

pub struct Task {
    state: Arc<AtomicU8>,
    abort: Option<AbortHandle>,
}

impl LuaUserData for Task {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Cancel", |_, task, ()| {
            let cancelled = task
                .state
                .compare_exchange(PENDING, CANCELLED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok();
            if cancelled && let Some(abort) = &task.abort {
                abort.abort();
            }
            Ok(cancelled)
        });
        methods.add_method("IsDone", |_, task, ()| {
            Ok(task.state.load(Ordering::Acquire) != PENDING)
        });
        methods.add_method("IsCancelled", |_, task, ()| {
            Ok(task.state.load(Ordering::Acquire) == CANCELLED)
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, task, ()| {
            Ok(match task.state.load(Ordering::Acquire) {
                PENDING => "Task(pending)",
                DONE => "Task(done)",
                _ => "Task(cancelled)",
            })
        });
    }
}

pub fn spawn_blocking<T, E, F>(
    lua: &Lua,
    callback: LuaFunction,
    work: F,
) -> LuaResult<LuaAnyUserData>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: IntoLuaMulti + Send + 'static,
    E: Display + Send + 'static,
{
    let completion = Completion::new(lua, callback)?;
    let state = Arc::clone(&completion.state);
    let handle = runtime().spawn_blocking(work);
    runtime().spawn(completion.complete_with(handle));
    create_userdata(lua, Task { state, abort: None })
}

pub fn run<T, E, F>(lua: &Lua, callback: Option<LuaFunction>, work: F) -> LuaResult<LuaMultiValue>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: IntoLuaMulti + Send + 'static,
    E: Display + Send + 'static,
{
    match callback {
        Some(callback) => spawn_blocking(lua, callback, work)?.into_lua_multi(lua),
        None => match work() {
            Ok(values) => values.into_lua_multi(lua),
            Err(error) => (LuaNil, error.to_string()).into_lua_multi(lua),
        },
    }
}

pub fn run_async<T, E, F>(
    lua: &Lua,
    callback: Option<LuaFunction>,
    future: F,
) -> LuaResult<LuaMultiValue>
where
    F: Future<Output = Result<T, E>> + Send + 'static,
    T: IntoLuaMulti + Send + 'static,
    E: Display + Send + 'static,
{
    match callback {
        Some(callback) => spawn_future(lua, callback, future)?.into_lua_multi(lua),
        None => match runtime().block_on(future) {
            Ok(values) => values.into_lua_multi(lua),
            Err(error) => (LuaNil, error.to_string()).into_lua_multi(lua),
        },
    }
}

pub fn spawn_future<T, E, F>(
    lua: &Lua,
    callback: LuaFunction,
    future: F,
) -> LuaResult<LuaAnyUserData>
where
    F: Future<Output = Result<T, E>> + Send + 'static,
    T: IntoLuaMulti + Send + 'static,
    E: Display + Send + 'static,
{
    let completion = Completion::new(lua, callback)?;
    let state = Arc::clone(&completion.state);
    let handle = runtime().spawn(future);
    let abort = handle.abort_handle();
    runtime().spawn(completion.complete_with(handle));
    create_userdata(
        lua,
        Task {
            state,
            abort: Some(abort),
        },
    )
}

struct Completion {
    dispatcher: Dispatcher,
    callback: LuaRegistryKey,
    state: Arc<AtomicU8>,
}

impl Completion {
    fn new(lua: &Lua, callback: LuaFunction) -> LuaResult<Self> {
        Ok(Self {
            dispatcher: Dispatcher::get(lua)?,
            callback: lua.create_registry_value(callback)?,
            state: Arc::new(AtomicU8::new(PENDING)),
        })
    }

    async fn complete_with<T, E>(self, handle: JoinHandle<Result<T, E>>)
    where
        T: IntoLuaMulti + Send + 'static,
        E: Display + Send + 'static,
    {
        let result = match handle.await {
            Ok(result) => result.map_err(|error| error.to_string()),
            Err(error) if error.is_panic() => Err("the native task panicked".to_owned()),
            Err(_) => Err("the native task was cancelled".to_owned()),
        };

        let Completion {
            dispatcher,
            callback,
            state,
        } = self;

        dispatcher.post(move |lua| {
            let function: LuaFunction = lua.registry_value(&callback)?;
            lua.remove_registry_value(callback)?;
            if state
                .compare_exchange(PENDING, DONE, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return Ok(());
            }

            match result {
                Ok(values) => function.call(values),
                Err(message) => function.call((LuaNil, message)),
            }
        });
    }
}
