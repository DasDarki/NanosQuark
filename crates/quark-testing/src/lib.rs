use mlua::prelude::*;

const SOURCE: &str = include_str!("testing.lua");

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let environment = lua.create_function(|lua, ()| Ok(quark_core::package_environment(lua)))?;
    let log = lua.create_function(|lua, message: String| console(lua, "Log", &message))?;
    let log_error = lua.create_function(|lua, message: String| console(lua, "Error", &message))?;
    let clock = lua.create_function(|_, ()| Ok(quark_core::clock()))?;

    lua.load(SOURCE).set_name("=quark/testing").call((
        quark_core::promise(lua)?,
        environment,
        log,
        log_error,
        clock,
    ))
}

fn console(lua: &Lua, level: &str, message: &str) -> LuaResult<()> {
    let line = format!("[Testing] {message}");
    let logged = lua
        .globals()
        .get::<LuaTable>("Console")
        .and_then(|console| console.get::<LuaFunction>(level))
        .and_then(|function| function.call::<()>(line.as_str()));

    if logged.is_err() {
        println!("{line}");
    }
    Ok(())
}
