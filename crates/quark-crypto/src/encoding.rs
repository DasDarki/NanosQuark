use data_encoding::{
    BASE32_NOPAD, BASE64, BASE64_NOPAD, BASE64URL_NOPAD, HEXLOWER, HEXLOWER_PERMISSIVE,
};
use mlua::prelude::*;
use quark_core::{Binary, Bytes};

#[derive(Clone, Copy)]
pub enum Format {
    Hex,
    Base64,
    Base64Url,
    Raw,
}

impl Format {
    pub fn encode(self, bytes: &[u8]) -> Bytes {
        match self {
            Format::Hex => Bytes(HEXLOWER.encode(bytes).into_bytes()),
            Format::Base64 => Bytes(BASE64.encode(bytes).into_bytes()),
            Format::Base64Url => Bytes(BASE64URL_NOPAD.encode(bytes).into_bytes()),
            Format::Raw => Bytes(bytes.to_vec()),
        }
    }

    pub fn decode(self, text: &[u8]) -> Result<Vec<u8>, String> {
        match self {
            Format::Hex => decode_hex(text),
            Format::Base64 => decode_base64(text, false),
            Format::Base64Url => decode_base64(text, true),
            Format::Raw => Ok(text.to_vec()),
        }
    }
}

impl FromLua for Format {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        let LuaValue::String(name) = &value else {
            return Err(LuaError::runtime(format!(
                "expected a format ('hex', 'base64', 'base64url' or 'raw'), got {}",
                value.type_name()
            )));
        };

        match name.to_str()?.to_ascii_lowercase().as_str() {
            "hex" => Ok(Format::Hex),
            "base64" => Ok(Format::Base64),
            "base64url" => Ok(Format::Base64Url),
            "raw" | "binary" => Ok(Format::Raw),
            other => Err(LuaError::runtime(format!(
                "unknown format '{other}', expected 'hex', 'base64', 'base64url' or 'raw'"
            ))),
        }
    }
}

pub fn register(lua: &Lua, crypto: &LuaTable) -> LuaResult<()> {
    crypto.set(
        "HexEncode",
        lua.create_function(|_, data: Binary| Ok(HEXLOWER.encode(data.as_bytes())))?,
    )?;
    crypto.set(
        "HexDecode",
        lua.create_function(|_, text: Binary| Ok(decode_hex(text.as_bytes()).map(Bytes)))?,
    )?;
    crypto.set(
        "Base64Encode",
        lua.create_function(|_, (data, url_safe): (Binary, Option<bool>)| {
            let format = if url_safe.unwrap_or(false) {
                Format::Base64Url
            } else {
                Format::Base64
            };
            Ok(format.encode(data.as_bytes()))
        })?,
    )?;
    crypto.set(
        "Base64Decode",
        lua.create_function(|_, (text, url_safe): (Binary, Option<bool>)| {
            Ok(decode_base64(text.as_bytes(), url_safe.unwrap_or(false)).map(Bytes))
        })?,
    )?;
    crypto.set(
        "Base58Encode",
        lua.create_function(|_, data: Binary| Ok(bs58::encode(data.as_bytes()).into_string()))?,
    )?;
    crypto.set(
        "Base58Decode",
        lua.create_function(|_, text: Binary| {
            Ok(bs58::decode(text.as_bytes().trim_ascii())
                .into_vec()
                .map(Bytes)
                .map_err(|error| format!("invalid Base58: {error}")))
        })?,
    )?;
    crypto.set(
        "Base32Encode",
        lua.create_function(|_, data: Binary| Ok(BASE32_NOPAD.encode(data.as_bytes())))?,
    )?;
    crypto.set(
        "Base32Decode",
        lua.create_function(|_, text: Binary| Ok(decode_base32(text.as_bytes()).map(Bytes)))?,
    )?;
    Ok(())
}

pub fn decode_hex(text: &[u8]) -> Result<Vec<u8>, String> {
    HEXLOWER_PERMISSIVE
        .decode(text.trim_ascii())
        .map_err(|error| format!("invalid hex: {error}"))
}

pub fn decode_base64(text: &[u8], url_safe: bool) -> Result<Vec<u8>, String> {
    let trimmed = text.trim_ascii();
    let unpadded = trimmed
        .strip_suffix(b"==")
        .or_else(|| trimmed.strip_suffix(b"="))
        .unwrap_or(trimmed);
    let engine = if url_safe {
        &BASE64URL_NOPAD
    } else {
        &BASE64_NOPAD
    };
    engine
        .decode(unpadded)
        .map_err(|error| format!("invalid Base64: {error}"))
}

pub fn decode_base32(text: &[u8]) -> Result<Vec<u8>, String> {
    let normalized = text
        .iter()
        .filter(|byte| !byte.is_ascii_whitespace() && **byte != b'=' && **byte != b'-')
        .map(u8::to_ascii_uppercase)
        .collect::<Vec<_>>();
    BASE32_NOPAD
        .decode(&normalized)
        .map_err(|error| format!("invalid Base32: {error}"))
}
