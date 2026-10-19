use hmac::{KeyInit, Mac, SimpleHmac};
use mlua::prelude::*;
use quark_core::{Binary, Object};
use sha2::digest::block_api::BlockSizeUser;
use sha2::digest::{Digest, DynDigest};
use subtle::ConstantTimeEq;

use crate::encoding::Format;

#[derive(Clone, Copy, PartialEq)]
pub enum HashAlgorithm {
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
    Sha3_256,
    Sha3_384,
    Sha3_512,
    Blake3,
}

const SHORTCUTS: &[(&str, HashAlgorithm)] = &[
    ("MD5", HashAlgorithm::Md5),
    ("SHA1", HashAlgorithm::Sha1),
    ("SHA256", HashAlgorithm::Sha256),
    ("SHA384", HashAlgorithm::Sha384),
    ("SHA512", HashAlgorithm::Sha512),
    ("SHA3_256", HashAlgorithm::Sha3_256),
    ("SHA3_512", HashAlgorithm::Sha3_512),
    ("BLAKE3", HashAlgorithm::Blake3),
];

impl HashAlgorithm {
    pub fn parse(name: &str) -> LuaResult<Self> {
        let normalized = name
            .chars()
            .filter(|character| *character != '-' && *character != '_')
            .collect::<String>()
            .to_ascii_uppercase();

        match normalized.as_str() {
            "MD5" => Ok(HashAlgorithm::Md5),
            "SHA1" => Ok(HashAlgorithm::Sha1),
            "SHA224" => Ok(HashAlgorithm::Sha224),
            "SHA256" => Ok(HashAlgorithm::Sha256),
            "SHA384" => Ok(HashAlgorithm::Sha384),
            "SHA512" => Ok(HashAlgorithm::Sha512),
            "SHA3256" => Ok(HashAlgorithm::Sha3_256),
            "SHA3384" => Ok(HashAlgorithm::Sha3_384),
            "SHA3512" => Ok(HashAlgorithm::Sha3_512),
            "BLAKE3" => Ok(HashAlgorithm::Blake3),
            _ => Err(LuaError::runtime(format!(
                "unsupported hash algorithm '{name}'"
            ))),
        }
    }

    pub fn digest(self, data: &[u8]) -> Vec<u8> {
        let mut state = HashState::new(self);
        state.update(data);
        state.finalize()
    }
}

impl FromLua for HashAlgorithm {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        HashAlgorithm::parse(&String::from_lua(value, lua)?)
    }
}

enum HashState {
    Digest(Box<dyn DynDigest + Send>),
    Blake3(Box<blake3::Hasher>),
}

impl HashState {
    fn new(algorithm: HashAlgorithm) -> Self {
        match algorithm {
            HashAlgorithm::Md5 => HashState::Digest(Box::new(md5::Md5::default())),
            HashAlgorithm::Sha1 => HashState::Digest(Box::new(sha1::Sha1::default())),
            HashAlgorithm::Sha224 => HashState::Digest(Box::new(sha2::Sha224::default())),
            HashAlgorithm::Sha256 => HashState::Digest(Box::new(sha2::Sha256::default())),
            HashAlgorithm::Sha384 => HashState::Digest(Box::new(sha2::Sha384::default())),
            HashAlgorithm::Sha512 => HashState::Digest(Box::new(sha2::Sha512::default())),
            HashAlgorithm::Sha3_256 => HashState::Digest(Box::new(sha3::Sha3_256::default())),
            HashAlgorithm::Sha3_384 => HashState::Digest(Box::new(sha3::Sha3_384::default())),
            HashAlgorithm::Sha3_512 => HashState::Digest(Box::new(sha3::Sha3_512::default())),
            HashAlgorithm::Blake3 => HashState::Blake3(Box::new(blake3::Hasher::new())),
        }
    }

    fn update(&mut self, data: &[u8]) {
        match self {
            HashState::Digest(state) => state.update(data),
            HashState::Blake3(state) => {
                state.update(data);
            }
        }
    }

    fn finalize(self) -> Vec<u8> {
        match self {
            HashState::Digest(state) => state.finalize().into_vec(),
            HashState::Blake3(state) => state.finalize().as_bytes().to_vec(),
        }
    }
}

struct Hasher {
    state: Option<HashState>,
}

impl LuaUserData for Hasher {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("Update", |_, (this, data): (LuaAnyUserData, Binary)| {
            match this.borrow_mut::<Hasher>()?.state.as_mut() {
                Some(state) => state.update(data.as_bytes()),
                None => return Err(LuaError::runtime("the hasher was already finalized")),
            }
            Ok(this)
        });

        methods.add_method_mut("Finalize", |_, hasher, format: Option<Format>| {
            let state = hasher
                .state
                .take()
                .ok_or_else(|| LuaError::runtime("the hasher was already finalized"))?;
            Ok(format.unwrap_or(Format::Hex).encode(&state.finalize()))
        });

        methods.add_meta_method(LuaMetaMethod::ToString, |_, _, ()| Ok("Hasher"));
    }
}

pub fn hmac(algorithm: HashAlgorithm, key: &[u8], data: &[u8]) -> LuaResult<Vec<u8>> {
    match algorithm {
        HashAlgorithm::Md5 => Ok(compute_hmac::<md5::Md5>(key, data)),
        HashAlgorithm::Sha1 => Ok(compute_hmac::<sha1::Sha1>(key, data)),
        HashAlgorithm::Sha224 => Ok(compute_hmac::<sha2::Sha224>(key, data)),
        HashAlgorithm::Sha256 => Ok(compute_hmac::<sha2::Sha256>(key, data)),
        HashAlgorithm::Sha384 => Ok(compute_hmac::<sha2::Sha384>(key, data)),
        HashAlgorithm::Sha512 => Ok(compute_hmac::<sha2::Sha512>(key, data)),
        HashAlgorithm::Sha3_256 => Ok(compute_hmac::<sha3::Sha3_256>(key, data)),
        HashAlgorithm::Sha3_384 => Ok(compute_hmac::<sha3::Sha3_384>(key, data)),
        HashAlgorithm::Sha3_512 => Ok(compute_hmac::<sha3::Sha3_512>(key, data)),
        HashAlgorithm::Blake3 => Err(LuaError::runtime(
            "HMAC does not support BLAKE3, it is a keyed hash on its own",
        )),
    }
}

fn compute_hmac<D: Digest + BlockSizeUser + Clone>(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac =
        <SimpleHmac<D> as KeyInit>::new_from_slice(key).expect("HMAC accepts keys of any length");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

pub fn register(lua: &Lua, crypto: &LuaTable) -> LuaResult<()> {
    crypto.set(
        "Hash",
        lua.create_function(
            |_, (algorithm, data, format): (HashAlgorithm, Binary, Option<Format>)| {
                Ok(format
                    .unwrap_or(Format::Hex)
                    .encode(&algorithm.digest(data.as_bytes())))
            },
        )?,
    )?;

    for (name, algorithm) in SHORTCUTS {
        let algorithm = *algorithm;
        crypto.set(
            *name,
            lua.create_function(move |_, (data, format): (Binary, Option<Format>)| {
                Ok(format
                    .unwrap_or(Format::Hex)
                    .encode(&algorithm.digest(data.as_bytes())))
            })?,
        )?;
    }

    crypto.set(
        "Hasher",
        lua.create_function(|_, algorithm: HashAlgorithm| {
            Ok(Object(Hasher {
                state: Some(HashState::new(algorithm)),
            }))
        })?,
    )?;

    crypto.set(
        "HMAC",
        lua.create_function(
            |_, (algorithm, key, data, format): (HashAlgorithm, Binary, Binary, Option<Format>)| {
                let mac = hmac(algorithm, key.as_bytes(), data.as_bytes())?;
                Ok(format.unwrap_or(Format::Hex).encode(&mac))
            },
        )?,
    )?;

    crypto.set(
        "HMACVerify",
        lua.create_function(
            |_,
             (algorithm, key, data, expected, format): (
                HashAlgorithm,
                Binary,
                Binary,
                Binary,
                Option<Format>,
            )| {
                let expected = match format.unwrap_or(Format::Hex).decode(expected.as_bytes()) {
                    Ok(expected) => expected,
                    Err(error) => return Ok(Err(error)),
                };
                let mac = hmac(algorithm, key.as_bytes(), data.as_bytes())?;
                Ok(Ok(bool::from(mac.ct_eq(&expected))))
            },
        )?,
    )?;

    Ok(())
}
