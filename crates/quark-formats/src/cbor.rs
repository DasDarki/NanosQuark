use ciborium::Value;
use mlua::prelude::*;
use quark_core::{Bytes, Options};

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
                    ciborium::into_writer(&value, &mut output)
                        .map_err(|error| format!("cannot encode CBOR: {error}"))?;
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
                let value = ciborium::from_reader::<Value, _>(&mut reader)
                    .map_err(|error| format!("invalid CBOR: {error}"))?;
                if !reader.is_empty() {
                    return Err(format!(
                        "invalid CBOR: {} unexpected trailing bytes",
                        reader.len()
                    ));
                }
                Ok(value)
            })
        })?,
    )?;

    Ok(module)
}
