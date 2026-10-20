use mlua::prelude::*;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode, utf8_percent_encode};
use quark_core::{Binary, Bytes};
use url::Url;
use url::form_urlencoded;

const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "Encode",
        lua.create_function(|_, (text, form): (String, Option<bool>)| {
            Ok(if form.unwrap_or(false) {
                form_urlencoded::byte_serialize(text.as_bytes()).collect::<String>()
            } else {
                utf8_percent_encode(&text, COMPONENT).to_string()
            })
        })?,
    )?;

    module.set(
        "Decode",
        lua.create_function(|_, (text, form): (Binary, Option<bool>)| {
            let mut bytes = text.0;
            if form.unwrap_or(false) {
                for byte in &mut bytes {
                    if *byte == b'+' {
                        *byte = b' ';
                    }
                }
            }
            Ok(Bytes(percent_decode(&bytes).collect()))
        })?,
    )?;

    module.set("EncodeQuery", lua.create_function(encode_query)?)?;
    module.set("DecodeQuery", lua.create_function(decode_query)?)?;
    module.set("Parse", lua.create_function(parse)?)?;

    module.set(
        "Join",
        lua.create_function(|_, (base, relative): (String, String)| {
            Ok(Url::parse(&base)
                .and_then(|base| base.join(&relative))
                .map(String::from)
                .map_err(|error| format!("invalid URL: {error}")))
        })?,
    )?;

    Ok(module)
}

fn encode_query(_: &Lua, parameters: LuaTable) -> LuaResult<String> {
    let mut pairs = Vec::new();
    for pair in parameters.pairs::<String, LuaValue>() {
        let (key, value) = pair?;
        match value {
            LuaValue::Table(values) => {
                for value in values.sequence_values::<LuaValue>() {
                    pairs.push((key.clone(), query_value(value?)?));
                }
            }
            value => pairs.push((key, query_value(value)?)),
        }
    }
    pairs.sort_by(|left, right| left.0.cmp(&right.0));

    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (key, value) in &pairs {
        serializer.append_pair(key, value);
    }
    Ok(serializer.finish())
}

fn query_value(value: LuaValue) -> LuaResult<String> {
    match value {
        LuaValue::String(string) => Ok(string.to_string_lossy()),
        LuaValue::Boolean(_) | LuaValue::Integer(_) | LuaValue::Number(_) => value.to_string(),
        other => Err(LuaError::runtime(format!(
            "query values have to be strings, numbers, booleans or lists of them, got {}",
            other.type_name()
        ))),
    }
}

fn decode_query(lua: &Lua, query: String) -> LuaResult<LuaTable> {
    let query = query.strip_prefix('?').unwrap_or(&query);
    let parameters = lua.create_table()?;
    for (key, value) in form_urlencoded::parse(query.as_bytes()) {
        let key = key.as_ref();
        match parameters.raw_get::<LuaValue>(key)? {
            LuaNil => parameters.raw_set(key, value.as_ref())?,
            LuaValue::Table(values) => values.raw_push(value.as_ref())?,
            existing => {
                let values = lua.create_sequence_from([
                    existing,
                    LuaValue::String(lua.create_string(value.as_ref())?),
                ])?;
                parameters.raw_set(key, values)?;
            }
        }
    }
    Ok(parameters)
}

fn parse(lua: &Lua, text: String) -> LuaResult<Result<LuaTable, String>> {
    let url = match Url::parse(&text) {
        Ok(url) => url,
        Err(error) => return Ok(Err(format!("invalid URL: {error}"))),
    };

    let parts = lua.create_table()?;
    parts.set("Scheme", url.scheme())?;
    if !url.username().is_empty() {
        parts.set("Username", url.username())?;
    }
    parts.set("Password", url.password())?;
    parts.set("Host", url.host_str())?;
    parts.set("Port", url.port_or_known_default())?;
    parts.set("Path", url.path())?;
    parts.set("Query", url.query())?;
    parts.set("Fragment", url.fragment())?;
    parts.set("Origin", url.origin().ascii_serialization())?;
    Ok(Ok(parts))
}
