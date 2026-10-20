use mlua::prelude::*;
use quark_core::{Binary, options_and_callback};
use serde_json::{Map, Value};

use crate::data::decode_with;

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Decode", lua.create_function(decode)?)?;
    module.set("Encode", lua.create_function(encode)?)?;
    Ok(module)
}

fn delimiter(value: Option<String>) -> LuaResult<u8> {
    match value.as_deref().map(str::as_bytes) {
        None => Ok(b','),
        Some([byte]) if byte.is_ascii() => Ok(*byte),
        Some(_) => Err(LuaError::runtime(
            "the delimiter has to be a single ASCII character",
        )),
    }
}

fn decode(
    lua: &Lua,
    (input, options, callback): (Binary, LuaValue, Option<LuaFunction>),
) -> LuaResult<LuaMultiValue> {
    let (options, callback) = options_and_callback(options, callback)?;
    let header = options.get_or("Header", true)?;
    let delimiter = delimiter(options.get("Delimiter")?)?;
    let trim = options.get_or("Trim", false)?;

    decode_with(lua, input, &options, callback, move |bytes| {
        let mut reader = ::csv::ReaderBuilder::new()
            .has_headers(header)
            .delimiter(delimiter)
            .flexible(true)
            .trim(if trim {
                ::csv::Trim::All
            } else {
                ::csv::Trim::None
            })
            .from_reader(bytes);

        let columns = if header {
            reader
                .headers()
                .map_err(|error| format!("invalid CSV: {error}"))?
                .iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let mut rows = Vec::new();
        for record in reader.records() {
            let record = record.map_err(|error| format!("invalid CSV: {error}"))?;
            let row = if header {
                Value::Object(
                    columns
                        .iter()
                        .zip(record.iter())
                        .map(|(column, field)| (column.clone(), Value::String(field.to_owned())))
                        .collect::<Map<_, _>>(),
                )
            } else {
                Value::Array(
                    record
                        .iter()
                        .map(|field| Value::String(field.to_owned()))
                        .collect(),
                )
            };
            rows.push(row);
        }
        Ok(Value::Array(rows))
    })
}

fn encode(
    lua: &Lua,
    (rows, options, callback): (Vec<LuaTable>, LuaValue, Option<LuaFunction>),
) -> LuaResult<LuaMultiValue> {
    let (options, callback) = options_and_callback(options, callback)?;
    let delimiter = delimiter(options.get("Delimiter")?)?;
    let keyed = rows.first().is_some_and(|row| row.raw_len() == 0);
    let header = options.get_or("Header", keyed)?;

    let columns = match options.get::<Vec<String>>("Columns")? {
        Some(columns) => columns,
        None if keyed => {
            let mut columns = Vec::new();
            for row in &rows {
                for pair in row.pairs::<String, LuaValue>() {
                    let (key, _) = pair?;
                    if !columns.contains(&key) {
                        columns.push(key);
                    }
                }
            }
            columns.sort();
            columns
        }
        None => Vec::new(),
    };

    let mut records = Vec::with_capacity(rows.len() + 1);
    if header && !columns.is_empty() {
        records.push(columns.clone());
    }
    for row in &rows {
        let record = if keyed || !columns.is_empty() && row.raw_len() == 0 {
            columns
                .iter()
                .map(|column| field(row.raw_get(column.as_str())?))
                .collect::<LuaResult<Vec<_>>>()?
        } else {
            row.sequence_values::<LuaValue>()
                .map(|value| field(value?))
                .collect::<LuaResult<Vec<_>>>()?
        };
        records.push(record);
    }

    quark_core::run(lua, callback, move || {
        let mut writer = ::csv::WriterBuilder::new()
            .delimiter(delimiter)
            .flexible(true)
            .from_writer(Vec::new());
        for record in &records {
            writer
                .write_record(record)
                .map_err(|error| format!("cannot encode CSV: {error}"))?;
        }
        let output = writer
            .into_inner()
            .map_err(|error| format!("cannot encode CSV: {error}"))?;
        String::from_utf8(output).map_err(|error| error.to_string())
    })
}

fn field(value: LuaValue) -> LuaResult<String> {
    match value {
        LuaNil => Ok(String::new()),
        value if value == LuaValue::NULL => Ok(String::new()),
        LuaValue::String(string) => Ok(string.to_string_lossy()),
        LuaValue::Boolean(_) | LuaValue::Integer(_) | LuaValue::Number(_) => value.to_string(),
        other => Err(LuaError::runtime(format!(
            "CSV fields have to be strings, numbers or booleans, got {}",
            other.type_name()
        ))),
    }
}
