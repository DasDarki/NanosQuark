use mlua::prelude::*;
use quark_core::Options;
use serde_json::{Map, Number, Value};

use crate::data::{decode, encode, text};

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "Encode",
        lua.create_function(|lua, arguments| {
            encode(lua, arguments, |value: Value, options: &Options| {
                if !value.is_object() {
                    return Err(LuaError::runtime(
                        "TOML documents have to be tables with string keys",
                    ));
                }
                let pretty = options.get_or("Pretty", false)?;
                Ok(Box::new(move || {
                    let result = if pretty {
                        ::toml::to_string_pretty(&value)
                    } else {
                        ::toml::to_string(&value)
                    };
                    result.map_err(|error| format!("cannot encode TOML: {error}"))
                }))
            })
        })?,
    )?;

    module.set(
        "Decode",
        lua.create_function(|lua, arguments| {
            decode(lua, arguments, |bytes| {
                let table = text(bytes)?
                    .parse::<::toml::Table>()
                    .map_err(|error| format!("invalid TOML: {}", error.to_string().trim()))?;
                Ok(convert_table(table))
            })
        })?,
    )?;

    Ok(module)
}

fn convert_table(table: ::toml::Table) -> Value {
    Value::Object(
        table
            .into_iter()
            .map(|(key, value)| (key, convert(value)))
            .collect::<Map<_, _>>(),
    )
}

fn convert(value: ::toml::Value) -> Value {
    match value {
        ::toml::Value::String(string) => Value::String(string),
        ::toml::Value::Integer(integer) => Value::Number(integer.into()),
        ::toml::Value::Float(float) => Number::from_f64(float)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        ::toml::Value::Boolean(boolean) => Value::Bool(boolean),
        ::toml::Value::Datetime(datetime) => Value::String(datetime.to_string()),
        ::toml::Value::Array(array) => Value::Array(array.into_iter().map(convert).collect()),
        ::toml::Value::Table(table) => convert_table(table),
    }
}
