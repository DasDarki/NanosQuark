use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use mlua::prelude::*;
use sha2::{Digest, Sha256, Sha512};

const MAX_RANDOM_BYTES: usize = 1 << 20;

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let crypto = lua.create_table()?;
    crypto.set("SHA256", lua.create_function(sha256)?)?;
    crypto.set("SHA512", lua.create_function(sha512)?)?;
    crypto.set("Hasher", lua.create_function(hasher)?)?;
    crypto.set("RandomBytes", lua.create_function(random_bytes)?)?;
    crypto.set("Base64Encode", lua.create_function(base64_encode)?)?;
    crypto.set("Base64Decode", lua.create_function(base64_decode)?)?;
    crypto.set("Argon2Hash", lua.create_function(argon2_hash)?)?;
    crypto.set("Argon2Verify", lua.create_function(argon2_verify)?)?;
    Ok(crypto)
}

fn sha256(_: &Lua, data: LuaString) -> LuaResult<String> {
    Ok(hex::encode(Sha256::digest(&*data.as_bytes())))
}

fn sha512(_: &Lua, data: LuaString) -> LuaResult<String> {
    Ok(hex::encode(Sha512::digest(&*data.as_bytes())))
}

enum HashState {
    Sha256(Sha256),
    Sha512(Sha512),
}

struct Hasher {
    state: Option<HashState>,
}

impl LuaUserData for Hasher {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("Update", |_, hasher, data: LuaString| {
            let bytes = data.as_bytes();
            match hasher.state.as_mut() {
                Some(HashState::Sha256(state)) => state.update(&*bytes),
                Some(HashState::Sha512(state)) => state.update(&*bytes),
                None => return Err(LuaError::runtime("the hasher was already finalized")),
            }
            Ok(())
        });

        methods.add_method_mut("Finalize", |_, hasher, ()| match hasher.state.take() {
            Some(HashState::Sha256(state)) => Ok(hex::encode(state.finalize())),
            Some(HashState::Sha512(state)) => Ok(hex::encode(state.finalize())),
            None => Err(LuaError::runtime("the hasher was already finalized")),
        });
    }
}

fn hasher(lua: &Lua, algorithm: String) -> LuaResult<LuaAnyUserData> {
    let state = match algorithm.to_ascii_uppercase().as_str() {
        "SHA256" => HashState::Sha256(Sha256::new()),
        "SHA512" => HashState::Sha512(Sha512::new()),
        _ => {
            return Err(LuaError::runtime(format!(
                "unsupported hash algorithm '{algorithm}', expected SHA256 or SHA512"
            )));
        }
    };

    quark_core::create_userdata(lua, Hasher { state: Some(state) })
}

fn random_bytes(lua: &Lua, count: usize) -> LuaResult<LuaString> {
    if count > MAX_RANDOM_BYTES {
        return Err(LuaError::runtime(format!(
            "cannot generate more than {MAX_RANDOM_BYTES} random bytes at once"
        )));
    }

    let mut bytes = vec![0; count];
    getrandom::fill(&mut bytes).map_err(LuaError::external)?;
    lua.create_string(bytes)
}

fn base64_encode(_: &Lua, (data, url_safe): (LuaString, Option<bool>)) -> LuaResult<String> {
    let engine = if url_safe.unwrap_or(false) {
        &URL_SAFE_NO_PAD
    } else {
        &STANDARD
    };
    Ok(engine.encode(&*data.as_bytes()))
}

fn base64_decode(lua: &Lua, (text, url_safe): (LuaString, Option<bool>)) -> LuaResult<LuaString> {
    let engine = if url_safe.unwrap_or(false) {
        &URL_SAFE_NO_PAD
    } else {
        &STANDARD
    };
    let bytes = engine
        .decode(&*text.as_bytes())
        .map_err(LuaError::external)?;
    lua.create_string(bytes)
}

fn argon2_hash(lua: &Lua, (password, callback): (LuaString, LuaFunction)) -> LuaResult<()> {
    let password = password.as_bytes().to_vec();
    quark_core::spawn_blocking(lua, callback, move || {
        Argon2::default()
            .hash_password(&password)
            .map(|hash| hash.to_string())
    })
}

fn argon2_verify(
    lua: &Lua,
    (password, hash, callback): (LuaString, String, LuaFunction),
) -> LuaResult<()> {
    let password = password.as_bytes().to_vec();
    quark_core::spawn_blocking(lua, callback, move || {
        let hash = PasswordHash::new(&hash)?;
        Ok::<_, argon2::password_hash::Error>(
            Argon2::default().verify_password(&password, &hash).is_ok(),
        )
    })
}
