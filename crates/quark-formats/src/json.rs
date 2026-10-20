use mlua::prelude::*;
use quark_core::Options;
use serde::Serialize;
use serde_json::Value;
use serde_json::ser::PrettyFormatter;

use crate::data::{add_values, decode, encode};

const DEFAULT_INDENT: usize = 2;

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    add_values(lua, &module)?;

    module.set(
        "Encode",
        lua.create_function(|lua, arguments| {
            encode(lua, arguments, |value: Value, options: &Options| {
                let indent = match (
                    options.get_or("Pretty", false)?,
                    options.get::<usize>("Indent")?,
                ) {
                    (_, Some(indent)) => Some(indent),
                    (true, None) => Some(DEFAULT_INDENT),
                    (false, None) => None,
                };
                Ok(Box::new(move || to_string(&value, indent)))
            })
        })?,
    )?;

    module.set(
        "Decode",
        lua.create_function(|lua, arguments| {
            decode(lua, arguments, |bytes| {
                serde_json::from_slice::<Value>(bytes)
                    .map_err(|error| format!("invalid JSON: {error}"))
            })
        })?,
    )?;

    Ok(module)
}

fn to_string(value: &Value, indent: Option<usize>) -> Result<String, String> {
    let Some(indent) = indent else {
        return serde_json::to_string(value).map_err(|error| error.to_string());
    };

    let indentation = " ".repeat(indent);
    let mut output = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(
        &mut output,
        PrettyFormatter::with_indent(indentation.as_bytes()),
    );
    value
        .serialize(&mut serializer)
        .map_err(|error| error.to_string())?;
    String::from_utf8(output).map_err(|error| error.to_string())
}
