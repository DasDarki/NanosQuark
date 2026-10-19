use std::sync::OnceLock;
use std::time::{Duration, Instant};

use mlua::prelude::*;

use crate::dispatcher::report_message;
use crate::{Dispatcher, spawn_future};

const SOURCE: &str = include_str!("promise.lua");

static START: OnceLock<Instant> = OnceLock::new();

struct PromiseModule(LuaTable);

pub fn promise(lua: &Lua) -> LuaResult<LuaTable> {
    if let Some(module) = lua.app_data_ref::<PromiseModule>() {
        return Ok(module.0.clone());
    }

    let defer = lua.create_function(|lua, callback: LuaFunction| {
        let dispatcher = Dispatcher::get(lua)?;
        let callback = lua.create_registry_value(callback)?;
        dispatcher.post(move |lua| {
            let function: LuaFunction = lua.registry_value(&callback)?;
            lua.remove_registry_value(callback)?;
            function.call(())
        });
        Ok(())
    })?;

    let report = lua.create_function(|lua, message: String| {
        report_message(lua, &message);
        Ok(())
    })?;

    let delay = lua.create_function(|lua, (milliseconds, callback): (u64, LuaFunction)| {
        spawn_future(lua, callback, async move {
            tokio::time::sleep(Duration::from_millis(milliseconds)).await;
            Ok::<_, String>(())
        })
    })?;

    let module: LuaTable = lua
        .load(SOURCE)
        .set_name("=quark/promise")
        .call((defer, report, delay))?;
    lua.set_app_data(PromiseModule(module.clone()));
    Ok(module)
}

pub fn clock() -> f64 {
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}
