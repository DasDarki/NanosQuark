use std::fmt::Display;

use mlua::prelude::*;
use tokio::task::JoinHandle;

use crate::{Dispatcher, runtime};

pub fn spawn_blocking<T, E, F>(lua: &Lua, callback: LuaFunction, work: F) -> LuaResult<()>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: IntoLuaMulti + Send + 'static,
    E: Display + Send + 'static,
{
    let completion = Completion::new(lua, callback)?;
    let handle = runtime().spawn_blocking(work);
    runtime().spawn(completion.complete_with(handle));
    Ok(())
}

pub fn run<T, E, F>(lua: &Lua, callback: Option<LuaFunction>, work: F) -> LuaResult<LuaMultiValue>
where
    F: FnOnce() -> Result<T, E> + Send + 'static,
    T: IntoLuaMulti + Send + 'static,
    E: Display + Send + 'static,
{
    match callback {
        Some(callback) => {
            spawn_blocking(lua, callback, work)?;
            Ok(LuaMultiValue::new())
        }
        None => match work() {
            Ok(values) => values.into_lua_multi(lua),
            Err(error) => (LuaNil, error.to_string()).into_lua_multi(lua),
        },
    }
}

pub fn spawn_future<T, E, F>(lua: &Lua, callback: LuaFunction, future: F) -> LuaResult<()>
where
    F: Future<Output = Result<T, E>> + Send + 'static,
    T: IntoLuaMulti + Send + 'static,
    E: Display + Send + 'static,
{
    let completion = Completion::new(lua, callback)?;
    let handle = runtime().spawn(future);
    runtime().spawn(completion.complete_with(handle));
    Ok(())
}

struct Completion {
    dispatcher: Dispatcher,
    callback: LuaRegistryKey,
}

impl Completion {
    fn new(lua: &Lua, callback: LuaFunction) -> LuaResult<Self> {
        Ok(Self {
            dispatcher: Dispatcher::get(lua)?,
            callback: lua.create_registry_value(callback)?,
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
        } = self;

        dispatcher.post(move |lua| {
            let function: LuaFunction = lua.registry_value(&callback)?;
            lua.remove_registry_value(callback)?;

            match result {
                Ok(values) => function.call(values),
                Err(message) => function.call((LuaNil, message)),
            }
        });
    }
}
