use mlua::prelude::*;
use quark_core::Bytes;
use redis::Value;

#[derive(Clone, Copy)]
pub enum Convert {
    Raw,
    Boolean,
    Number,
    Pairs,
    Scores,
}

pub struct Reply {
    pub value: Value,
    pub convert: Convert,
}

pub struct Replies(pub Vec<Value>);

impl IntoLua for Replies {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let list = lua.create_table()?;
        for value in self.0 {
            list.raw_push(convert(lua, value)?)?;
        }
        Ok(LuaValue::Table(list))
    }
}

impl IntoLua for Reply {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self.convert {
            Convert::Raw => convert(lua, self.value),
            Convert::Boolean => Ok(LuaValue::Boolean(match self.value {
                Value::Nil => false,
                Value::Int(number) => number != 0,
                Value::Boolean(flag) => flag,
                _ => true,
            })),
            Convert::Number => match self.value {
                Value::Nil => Ok(LuaNil),
                Value::Int(number) => Ok(LuaValue::Integer(number)),
                Value::Double(number) => Ok(LuaValue::Number(number)),
                Value::BulkString(bytes) => number(&bytes),
                other => convert(lua, other),
            },
            Convert::Pairs => match self.value {
                Value::Array(items) => {
                    let table = lua.create_table()?;
                    let mut items = items.into_iter();
                    while let (Some(key), Some(value)) = (items.next(), items.next()) {
                        table.raw_set(convert(lua, key)?, convert(lua, value)?)?;
                    }
                    Ok(LuaValue::Table(table))
                }
                other => convert(lua, other),
            },
            Convert::Scores => match self.value {
                Value::Array(items) => {
                    let list = lua.create_table()?;
                    let mut items = items.into_iter().peekable();
                    while let Some(item) = items.next() {
                        let entry = lua.create_table()?;
                        match item {
                            Value::Array(mut pair) if pair.len() == 2 => {
                                let score =
                                    pair.pop().map(score_value).transpose()?.unwrap_or(LuaNil);
                                let member = pair
                                    .pop()
                                    .map(|member| convert(lua, member))
                                    .transpose()?
                                    .unwrap_or(LuaNil);
                                entry.set("Member", member)?;
                                entry.set("Score", score)?;
                            }
                            member => {
                                let score =
                                    items.next().map(score_value).transpose()?.unwrap_or(LuaNil);
                                entry.set("Member", convert(lua, member)?)?;
                                entry.set("Score", score)?;
                            }
                        }
                        list.raw_push(entry)?;
                    }
                    Ok(LuaValue::Table(list))
                }
                other => convert(lua, other),
            },
        }
    }
}

fn number(bytes: &[u8]) -> LuaResult<LuaValue> {
    let text = String::from_utf8_lossy(bytes);
    if let Ok(integer) = text.parse::<i64>() {
        return Ok(LuaValue::Integer(integer));
    }
    match text.as_ref() {
        "inf" | "+inf" => Ok(LuaValue::Number(f64::INFINITY)),
        "-inf" => Ok(LuaValue::Number(f64::NEG_INFINITY)),
        _ => text
            .parse::<f64>()
            .map(LuaValue::Number)
            .map_err(|_| LuaError::runtime(format!("expected a number, got '{text}'"))),
    }
}

fn score_value(value: Value) -> LuaResult<LuaValue> {
    match value {
        Value::BulkString(bytes) => number(&bytes),
        Value::Double(number) => Ok(LuaValue::Number(number)),
        Value::Int(number) => Ok(LuaValue::Integer(number)),
        _ => Ok(LuaNil),
    }
}

pub fn convert(lua: &Lua, value: Value) -> LuaResult<LuaValue> {
    Ok(match value {
        Value::Nil => LuaNil,
        Value::Int(number) => LuaValue::Integer(number),
        Value::BulkString(bytes) => Bytes(bytes).into_lua(lua)?,
        Value::SimpleString(text) => LuaValue::String(lua.create_string(text)?),
        Value::Okay => LuaValue::String(lua.create_string("OK")?),
        Value::Double(number) => LuaValue::Number(number),
        Value::Boolean(flag) => LuaValue::Boolean(flag),
        Value::Array(items) | Value::Set(items) | Value::Push { data: items, .. } => {
            let list = lua.create_table()?;
            for item in items {
                list.raw_push(convert(lua, item)?)?;
            }
            LuaValue::Table(list)
        }
        Value::Map(pairs) => {
            let table = lua.create_table()?;
            for (key, value) in pairs {
                let key = convert(lua, key)?;
                if !key.is_nil() {
                    table.raw_set(key, convert(lua, value)?)?;
                }
            }
            LuaValue::Table(table)
        }
        Value::Attribute { data, .. } => convert(lua, *data)?,
        Value::VerbatimString { text, .. } => LuaValue::String(lua.create_string(text)?),
        Value::ServerError(error) => return Err(LuaError::runtime(format!("{error:?}"))),
        other => LuaValue::String(lua.create_string(format!("{other:?}"))?),
    })
}
