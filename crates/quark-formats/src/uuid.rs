use mlua::prelude::*;
use quark_core::{Binary, Bytes};
use uuid::Uuid;

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Nil", Uuid::nil().to_string())?;
    module.set("Max", Uuid::max().to_string())?;

    module.set(
        "V4",
        lua.create_function(|_, ()| Ok(Uuid::new_v4().to_string()))?,
    )?;
    module.set(
        "V7",
        lua.create_function(|_, ()| Ok(Uuid::now_v7().to_string()))?,
    )?;

    module.set(
        "Parse",
        lua.create_function(|_, text: String| Ok(parse(&text).map(|uuid| uuid.to_string())))?,
    )?;
    module.set(
        "IsValid",
        lua.create_function(|_, value: LuaValue| {
            Ok(match value {
                LuaValue::String(text) => text.to_str().is_ok_and(|text| parse(&text).is_ok()),
                _ => false,
            })
        })?,
    )?;
    module.set(
        "GetVersion",
        lua.create_function(|_, text: String| Ok(parse(&text).map(|uuid| uuid.get_version_num())))?,
    )?;
    module.set(
        "GetTimestamp",
        lua.create_function(|_, text: String| {
            Ok(parse(&text).and_then(|uuid| {
                let (seconds, nanoseconds) = uuid
                    .get_timestamp()
                    .ok_or("this UUID version has no timestamp")?
                    .to_unix();
                Ok(seconds as i64 * 1000 + i64::from(nanoseconds / 1_000_000))
            }))
        })?,
    )?;
    module.set(
        "ToBytes",
        lua.create_function(|_, text: String| {
            Ok(parse(&text).map(|uuid| Bytes(uuid.as_bytes().to_vec())))
        })?,
    )?;
    module.set(
        "FromBytes",
        lua.create_function(|_, bytes: Binary| {
            Ok(Uuid::from_slice(bytes.as_bytes())
                .map(|uuid| uuid.to_string())
                .map_err(|_| format!("expected 16 bytes, got {}", bytes.as_bytes().len())))
        })?,
    )?;

    Ok(module)
}

fn parse(text: &str) -> Result<Uuid, String> {
    Uuid::try_parse(text.trim()).map_err(|error| format!("invalid UUID: {error}"))
}
