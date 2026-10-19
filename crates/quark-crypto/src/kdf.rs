use hkdf::SimpleHkdf;
use mlua::prelude::*;
use quark_core::{Binary, Options, options_and_callback};
use sha2::digest::Digest;
use sha2::digest::block_api::BlockSizeUser;

use crate::encoding::Format;
use crate::hash::HashAlgorithm;

const DEFAULT_PBKDF2_ITERATIONS: u32 = 600_000;
const DEFAULT_KEY_LENGTH: usize = 32;
const MAX_KEY_LENGTH: usize = 1024;

pub fn register(lua: &Lua, crypto: &LuaTable) -> LuaResult<()> {
    crypto.set("HKDF", lua.create_function(hkdf)?)?;
    crypto.set("PBKDF2", lua.create_function(pbkdf2)?)?;
    Ok(())
}

fn hkdf(_: &Lua, (key, options): (Binary, Options)) -> LuaResult<quark_core::Bytes> {
    let algorithm = options.get_or("Algorithm", HashAlgorithm::Sha256)?;
    let salt = options.get::<Binary>("Salt")?;
    let info = options.get::<Binary>("Info")?;
    let length = key_length(&options)?;
    let format = options.get_or("Format", Format::Raw)?;

    let salt = salt.as_ref().map(|salt| salt.as_bytes().to_vec());
    let info = info
        .map(|info| info.as_bytes().to_vec())
        .unwrap_or_default();
    let key = key.as_bytes();

    let mut output = vec![0; length];
    match algorithm {
        HashAlgorithm::Sha1 => expand::<sha1::Sha1>(salt.as_deref(), key, &info, &mut output)?,
        HashAlgorithm::Sha256 => expand::<sha2::Sha256>(salt.as_deref(), key, &info, &mut output)?,
        HashAlgorithm::Sha384 => expand::<sha2::Sha384>(salt.as_deref(), key, &info, &mut output)?,
        HashAlgorithm::Sha512 => expand::<sha2::Sha512>(salt.as_deref(), key, &info, &mut output)?,
        _ => {
            return Err(LuaError::runtime(
                "HKDF supports the algorithms SHA1, SHA256, SHA384 and SHA512",
            ));
        }
    }

    Ok(format.encode(&output))
}

fn expand<D: Digest + BlockSizeUser + Clone>(
    salt: Option<&[u8]>,
    key: &[u8],
    info: &[u8],
    output: &mut [u8],
) -> LuaResult<()> {
    SimpleHkdf::<D>::new(salt, key)
        .expand(info, output)
        .map_err(|_| LuaError::runtime("the requested HKDF length is too large for this algorithm"))
}

fn pbkdf2(
    lua: &Lua,
    (password, salt, options, callback): (Binary, Binary, LuaValue, Option<LuaFunction>),
) -> LuaResult<LuaMultiValue> {
    let (options, callback) = options_and_callback(options, callback)?;
    let algorithm = options.get_or("Algorithm", HashAlgorithm::Sha256)?;
    let iterations = options.get_or("Iterations", DEFAULT_PBKDF2_ITERATIONS)?;
    let length = key_length(&options)?;
    let format = options.get_or("Format", Format::Raw)?;

    if iterations == 0 {
        return Err(LuaError::runtime("PBKDF2 needs at least one iteration"));
    }

    let derive: fn(&[u8], &[u8], u32, &mut [u8]) = match algorithm {
        HashAlgorithm::Sha1 => pbkdf2::pbkdf2_hmac::<sha1::Sha1>,
        HashAlgorithm::Sha256 => pbkdf2::pbkdf2_hmac::<sha2::Sha256>,
        HashAlgorithm::Sha384 => pbkdf2::pbkdf2_hmac::<sha2::Sha384>,
        HashAlgorithm::Sha512 => pbkdf2::pbkdf2_hmac::<sha2::Sha512>,
        _ => {
            return Err(LuaError::runtime(
                "PBKDF2 supports the algorithms SHA1, SHA256, SHA384 and SHA512",
            ));
        }
    };

    let password = password.as_bytes().to_vec();
    let salt = salt.as_bytes().to_vec();
    quark_core::run(lua, callback, move || {
        let mut output = vec![0; length];
        derive(&password, &salt, iterations, &mut output);
        Ok::<_, String>(format.encode(&output))
    })
}

fn key_length(options: &Options) -> LuaResult<usize> {
    let length = options.get_or("Length", DEFAULT_KEY_LENGTH)?;
    if length == 0 || length > MAX_KEY_LENGTH {
        return Err(LuaError::runtime(format!(
            "the key length has to be between 1 and {MAX_KEY_LENGTH} bytes"
        )));
    }
    Ok(length)
}
