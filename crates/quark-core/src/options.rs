use mlua::prelude::*;

use crate::describe;

#[derive(Default)]
pub struct Options(Option<LuaTable>);

impl Options {
    pub fn get<T: FromLua>(&self, key: &str) -> LuaResult<Option<T>> {
        let Some(table) = &self.0 else {
            return Ok(None);
        };

        table.get::<Option<T>>(key).map_err(|error| {
            LuaError::runtime(format!("invalid option '{key}': {}", describe(&error)))
        })
    }

    pub fn get_or<T: FromLua>(&self, key: &str, default: T) -> LuaResult<T> {
        Ok(self.get(key)?.unwrap_or(default))
    }
}

impl FromLua for Options {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        match value {
            LuaNil => Ok(Options(None)),
            LuaValue::Table(table) => Ok(Options(Some(table))),
            other => Err(LuaError::runtime(format!(
                "expected an options table, got {}",
                other.type_name()
            ))),
        }
    }
}

pub fn options_and_callback(
    value: LuaValue,
    callback: Option<LuaFunction>,
) -> LuaResult<(Options, Option<LuaFunction>)> {
    match (value, callback) {
        (LuaValue::Function(callback), None) => Ok((Options(None), Some(callback))),
        (LuaValue::Table(table), callback) => Ok((Options(Some(table)), callback)),
        (LuaNil, callback) => Ok((Options(None), callback)),
        (other, _) => Err(LuaError::runtime(format!(
            "expected an options table or a callback, got {}",
            other.type_name()
        ))),
    }
}
