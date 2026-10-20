use mlua::prelude::*;
use quark_core::{Bytes, Options};
use rmpv::Value;

use crate::data::{add_values, decode, encode};

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    add_values(lua, &module)?;

    module.set(
        "Encode",
        lua.create_function(|lua, arguments| {
            encode(lua, arguments, |value: Value, _: &Options| {
                Ok(Box::new(move || {
                    let mut output = Vec::new();
                    rmpv::encode::write_value(&mut output, &value)
                        .map_err(|error| format!("cannot encode MessagePack: {error}"))?;
                    Ok(Bytes(output))
                }))
            })
        })?,
    )?;

    module.set(
        "Decode",
        lua.create_function(|lua, arguments| {
            decode(lua, arguments, |bytes| {
                let mut reader = bytes;
                let value = rmpv::decode::read_value(&mut reader)
                    .map_err(|error| format!("invalid MessagePack: {error}"))?;
                if !reader.is_empty() {
                    return Err(format!(
                        "invalid MessagePack: {} unexpected trailing bytes",
                        reader.len()
                    ));
                }
                Ok(value)
            })
        })?,
    )?;

    Ok(module)
}
