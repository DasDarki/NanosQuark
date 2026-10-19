use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params, Version};
use mlua::prelude::*;
use quark_core::{Binary, options_and_callback};
use scrypt::Scrypt;

const DEFAULT_BCRYPT_COST: u32 = 12;

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Argon2,
    Bcrypt,
    Scrypt,
}

impl Kind {
    fn detect(hash: &str) -> Option<Self> {
        if hash.starts_with("$argon2") {
            Some(Kind::Argon2)
        } else if hash.starts_with("$scrypt$") {
            Some(Kind::Scrypt)
        } else if ["$2a$", "$2b$", "$2x$", "$2y$"]
            .iter()
            .any(|prefix| hash.starts_with(prefix))
        {
            Some(Kind::Bcrypt)
        } else {
            None
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Argon2 => "Argon2",
            Kind::Bcrypt => "bcrypt",
            Kind::Scrypt => "scrypt",
        }
    }
}

pub fn register(lua: &Lua, crypto: &LuaTable) -> LuaResult<()> {
    crypto.set("Argon2Hash", lua.create_function(argon2_hash)?)?;
    crypto.set("BcryptHash", lua.create_function(bcrypt_hash)?)?;
    crypto.set("ScryptHash", lua.create_function(scrypt_hash)?)?;

    crypto.set(
        "PasswordVerify",
        lua.create_function(|lua, arguments| verify(lua, None, arguments))?,
    )?;
    crypto.set(
        "Argon2Verify",
        lua.create_function(|lua, arguments| verify(lua, Some(Kind::Argon2), arguments))?,
    )?;
    crypto.set(
        "BcryptVerify",
        lua.create_function(|lua, arguments| verify(lua, Some(Kind::Bcrypt), arguments))?,
    )?;
    crypto.set(
        "ScryptVerify",
        lua.create_function(|lua, arguments| verify(lua, Some(Kind::Scrypt), arguments))?,
    )?;
    Ok(())
}

fn argon2_hash(
    lua: &Lua,
    (password, options, callback): (Binary, LuaValue, Option<LuaFunction>),
) -> LuaResult<LuaMultiValue> {
    let (options, callback) = options_and_callback(options, callback)?;
    let variant = match options
        .get_or("Variant", "argon2id".to_owned())?
        .to_ascii_lowercase()
        .as_str()
    {
        "argon2id" => Algorithm::Argon2id,
        "argon2i" => Algorithm::Argon2i,
        "argon2d" => Algorithm::Argon2d,
        other => {
            return Err(LuaError::runtime(format!(
                "unknown Argon2 variant '{other}', expected argon2id, argon2i or argon2d"
            )));
        }
    };
    let params = Params::new(
        options.get_or("MemoryCost", Params::DEFAULT_M_COST)?,
        options.get_or("TimeCost", Params::DEFAULT_T_COST)?,
        options.get_or("Parallelism", Params::DEFAULT_P_COST)?,
        None,
    )
    .map_err(|error| LuaError::runtime(format!("invalid Argon2 parameters: {error}")))?;

    let password = password.as_bytes().to_vec();
    quark_core::run(lua, callback, move || {
        Argon2::new(variant, Version::V0x13, params)
            .hash_password(&password)
            .map(|hash| hash.to_string())
            .map_err(|error| error.to_string())
    })
}

fn bcrypt_hash(
    lua: &Lua,
    (password, options, callback): (Binary, LuaValue, Option<LuaFunction>),
) -> LuaResult<LuaMultiValue> {
    let (options, callback) = options_and_callback(options, callback)?;
    let cost = options.get_or("Cost", DEFAULT_BCRYPT_COST)?;
    if !(4..=31).contains(&cost) {
        return Err(LuaError::runtime(
            "the bcrypt cost has to be between 4 and 31",
        ));
    }

    let password = password.as_bytes().to_vec();
    quark_core::run(lua, callback, move || {
        bcrypt::non_truncating_hash(&password, cost).map_err(|error| error.to_string())
    })
}

fn scrypt_hash(
    lua: &Lua,
    (password, options, callback): (Binary, LuaValue, Option<LuaFunction>),
) -> LuaResult<LuaMultiValue> {
    let (options, callback) = options_and_callback(options, callback)?;
    let params = scrypt::Params::new(
        options.get_or("LogN", scrypt::Params::RECOMMENDED_LOG_N)?,
        options.get_or("R", scrypt::Params::RECOMMENDED_R)?,
        options.get_or("P", scrypt::Params::RECOMMENDED_P)?,
    )
    .map_err(|error| LuaError::runtime(format!("invalid scrypt parameters: {error}")))?;

    let password = password.as_bytes().to_vec();
    quark_core::run(lua, callback, move || {
        Scrypt::from(params)
            .hash_password(&password)
            .map(|hash| hash.to_string())
            .map_err(|error| error.to_string())
    })
}

fn verify(
    lua: &Lua,
    expected: Option<Kind>,
    (password, hash, callback): (Binary, String, Option<LuaFunction>),
) -> LuaResult<LuaMultiValue> {
    let password = password.as_bytes().to_vec();
    quark_core::run(lua, callback, move || {
        verify_hash(&password, &hash, expected)
    })
}

fn verify_hash(password: &[u8], hash: &str, expected: Option<Kind>) -> Result<bool, String> {
    let kind = Kind::detect(hash).ok_or("unknown password hash format")?;
    if let Some(expected) = expected
        && expected != kind
    {
        return Err(format!(
            "expected a {} hash, got a {} hash",
            expected.name(),
            kind.name()
        ));
    }

    match kind {
        Kind::Argon2 => {
            let hash = PasswordHash::new(hash).map_err(|error| error.to_string())?;
            Ok(Argon2::default().verify_password(password, &hash).is_ok())
        }
        Kind::Scrypt => {
            let hash = PasswordHash::new(hash).map_err(|error| error.to_string())?;
            Ok(Scrypt::default().verify_password(password, &hash).is_ok())
        }
        Kind::Bcrypt => bcrypt::verify(password, hash).map_err(|error| error.to_string()),
    }
}
