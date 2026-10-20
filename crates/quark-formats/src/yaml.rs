use mlua::prelude::*;
use quark_core::Options;
use serde_json::Value;

use crate::data::{add_values, decode, encode, text};

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    add_values(lua, &module)?;

    module.set(
        "Encode",
        lua.create_function(|lua, arguments| {
            encode(lua, arguments, |value: Value, _: &Options| {
                Ok(Box::new(move || {
                    serde_saphyr::to_string(&value)
                        .map_err(|error| format!("cannot encode YAML: {error}"))
                }))
            })
        })?,
    )?;

    module.set(
        "Decode",
        lua.create_function(|lua, arguments| {
            decode(lua, arguments, |bytes| {
                serde_saphyr::from_str::<Value>(text(bytes)?)
                    .map_err(|error| format!("invalid YAML: {error}"))
            })
        })?,
    )?;

    Ok(module)
}
