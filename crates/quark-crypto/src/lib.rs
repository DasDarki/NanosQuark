mod cipher;
mod encoding;
mod hash;
mod jwt;
mod kdf;
mod keys;
mod otp;
mod password;
mod random;

use mlua::prelude::*;

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let crypto = lua.create_table()?;
    encoding::register(lua, &crypto)?;
    hash::register(lua, &crypto)?;
    kdf::register(lua, &crypto)?;
    password::register(lua, &crypto)?;
    cipher::register(lua, &crypto)?;
    keys::register(lua, &crypto)?;
    jwt::register(lua, &crypto)?;
    otp::register(lua, &crypto)?;
    random::register(lua, &crypto)?;
    Ok(crypto)
}

pub(crate) fn now_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}
