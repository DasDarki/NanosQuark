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
    Entries,
    Streams,
    Pending,
    AutoClaim,
}

pub struct StreamEntry {
    pub id: String,
    pub fields: Vec<(Vec<u8>, Vec<u8>)>,
}

impl IntoLua for StreamEntry {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let entry = lua.create_table()?;
        entry.set("ID", self.id)?;
        entry.set("Fields", fields_table(lua, self.fields)?)?;
        Ok(LuaValue::Table(entry))
    }
}

pub fn fields_table(lua: &Lua, fields: Vec<(Vec<u8>, Vec<u8>)>) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    for (field, value) in fields {
        table.raw_set(lua.create_string(field)?, lua.create_string(value)?)?;
    }
    Ok(table)
}

fn bytes_of(value: Value) -> Vec<u8> {
    match value {
        Value::BulkString(bytes) => bytes,
        Value::SimpleString(text) => text.into_bytes(),
        Value::VerbatimString { text, .. } => text.into_bytes(),
        Value::Int(number) => number.to_string().into_bytes(),
        Value::Okay => b"OK".to_vec(),
        _ => Vec::new(),
    }
}

fn items(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) | Value::Set(items) => items,
        Value::Map(pairs) => pairs
            .into_iter()
            .flat_map(|(key, value)| [key, value])
            .collect(),
        Value::Nil => Vec::new(),
        other => vec![other],
    }
}

pub fn parse_entries(value: Value) -> Vec<StreamEntry> {
    items(value)
        .into_iter()
        .filter_map(|entry| {
            let mut parts = items(entry).into_iter();
            let id = String::from_utf8_lossy(&bytes_of(parts.next()?)).into_owned();
            let mut flat = items(parts.next().unwrap_or(Value::Nil)).into_iter();
            let mut fields = Vec::new();
            while let (Some(field), Some(value)) = (flat.next(), flat.next()) {
                fields.push((bytes_of(field), bytes_of(value)));
            }
            Some(StreamEntry { id, fields })
        })
        .collect()
}

pub fn parse_streams(value: Value) -> Vec<(String, Vec<StreamEntry>)> {
    match value {
        Value::Map(pairs) => pairs
            .into_iter()
            .map(|(name, entries)| {
                (
                    String::from_utf8_lossy(&bytes_of(name)).into_owned(),
                    parse_entries(entries),
                )
            })
            .collect(),
        other => items(other)
            .into_iter()
            .filter_map(|stream| {
                let mut parts = items(stream).into_iter();
                let name = String::from_utf8_lossy(&bytes_of(parts.next()?)).into_owned();
                Some((name, parse_entries(parts.next().unwrap_or(Value::Nil))))
            })
            .collect(),
    }
}

fn entries_table(lua: &Lua, entries: Vec<StreamEntry>) -> LuaResult<LuaTable> {
    let list = lua.create_table()?;
    for entry in entries {
        list.raw_push(entry)?;
    }
    Ok(list)
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
            Convert::Entries => Ok(LuaValue::Table(entries_table(
                lua,
                parse_entries(self.value),
            )?)),
            Convert::Streams => {
                if matches!(self.value, Value::Nil) {
                    return Ok(LuaNil);
                }
                let table = lua.create_table()?;
                for (name, entries) in parse_streams(self.value) {
                    table.raw_set(name, entries_table(lua, entries)?)?;
                }
                Ok(LuaValue::Table(table))
            }
            Convert::Pending => {
                let mut parts = items(self.value).into_iter();
                let summary = lua.create_table()?;
                summary.set(
                    "Count",
                    match parts.next() {
                        Some(Value::Int(count)) => count,
                        _ => 0,
                    },
                )?;
                let id = |value: Option<Value>| match value {
                    Some(Value::Nil) | None => None,
                    Some(value) => Some(String::from_utf8_lossy(&bytes_of(value)).into_owned()),
                };
                summary.set("First", id(parts.next()))?;
                summary.set("Last", id(parts.next()))?;
                let consumers = lua.create_table()?;
                for consumer in items(parts.next().unwrap_or(Value::Nil)) {
                    let mut pair = items(consumer).into_iter();
                    if let (Some(name), Some(count)) = (pair.next(), pair.next()) {
                        let count = String::from_utf8_lossy(&bytes_of(count))
                            .parse::<i64>()
                            .unwrap_or_default();
                        consumers.raw_set(lua.create_string(bytes_of(name))?, count)?;
                    }
                }
                summary.set("Consumers", consumers)?;
                Ok(LuaValue::Table(summary))
            }
            Convert::AutoClaim => {
                let mut parts = items(self.value).into_iter();
                let next = parts
                    .next()
                    .map(|value| String::from_utf8_lossy(&bytes_of(value)).into_owned());
                let result = lua.create_table()?;
                result.set(
                    "Entries",
                    entries_table(lua, parse_entries(parts.next().unwrap_or(Value::Nil)))?,
                )?;
                result.set("Next", next)?;
                Ok(LuaValue::Table(result))
            }
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
