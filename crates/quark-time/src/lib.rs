mod cron;
mod datetime;
mod duration;
mod names;
mod schedule;

use mlua::prelude::*;

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    datetime::register(lua, &module)?;
    duration::register(lua, &module)?;
    cron::register(lua, &module)?;
    schedule::register(lua, &module)?;
    Ok(module)
}
