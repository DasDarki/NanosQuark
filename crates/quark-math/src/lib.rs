mod bigint;
mod decimal;
mod expression;
mod geometry;
mod quaternion;
mod scalar;
mod statistics;
mod vector;

use mlua::prelude::*;

pub use self::bigint::build as bigint;
pub use self::decimal::build as decimal;
pub use self::geometry::build as geometry;

pub fn math(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    scalar::register(lua, &module)?;
    quaternion::register(lua, &module)?;
    statistics::register(lua, &module)?;
    expression::register(lua, &module)?;
    Ok(module)
}
