use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes128Gcm, Aes256Gcm};
use chacha20poly1305::XChaCha20Poly1305;
use mlua::prelude::*;
use quark_core::{Binary, Bytes};

const AES_NONCE_LENGTH: usize = 12;
const XCHACHA_NONCE_LENGTH: usize = 24;
const TAG_LENGTH: usize = 16;

pub fn register(lua: &Lua, crypto: &LuaTable) -> LuaResult<()> {
    crypto.set(
        "AESEncrypt",
        lua.create_function(
            |_, (key, data, associated): (Binary, Binary, Option<Binary>)| {
                let key = key.as_bytes();
                let associated = associated
                    .map(|value| value.as_bytes().to_vec())
                    .unwrap_or_default();
                match key.len() {
                    16 => {
                        encrypt::<Aes128Gcm>(&key, &data.as_bytes(), &associated, AES_NONCE_LENGTH)
                    }
                    32 => {
                        encrypt::<Aes256Gcm>(&key, &data.as_bytes(), &associated, AES_NONCE_LENGTH)
                    }
                    _ => Err(LuaError::runtime("AES keys have to be 16 or 32 bytes long")),
                }
            },
        )?,
    )?;

    crypto.set(
        "AESDecrypt",
        lua.create_function(
            |_, (key, data, associated): (Binary, Binary, Option<Binary>)| {
                let key = key.as_bytes();
                let associated = associated
                    .map(|value| value.as_bytes().to_vec())
                    .unwrap_or_default();
                match key.len() {
                    16 => {
                        decrypt::<Aes128Gcm>(&key, &data.as_bytes(), &associated, AES_NONCE_LENGTH)
                    }
                    32 => {
                        decrypt::<Aes256Gcm>(&key, &data.as_bytes(), &associated, AES_NONCE_LENGTH)
                    }
                    _ => Err(LuaError::runtime("AES keys have to be 16 or 32 bytes long")),
                }
            },
        )?,
    )?;

    crypto.set(
        "XChaCha20Encrypt",
        lua.create_function(
            |_, (key, data, associated): (Binary, Binary, Option<Binary>)| {
                let key = key.as_bytes();
                let associated = associated
                    .map(|value| value.as_bytes().to_vec())
                    .unwrap_or_default();
                if key.len() != 32 {
                    return Err(LuaError::runtime("XChaCha20 keys have to be 32 bytes long"));
                }
                encrypt::<XChaCha20Poly1305>(
                    &key,
                    &data.as_bytes(),
                    &associated,
                    XCHACHA_NONCE_LENGTH,
                )
            },
        )?,
    )?;

    crypto.set(
        "XChaCha20Decrypt",
        lua.create_function(
            |_, (key, data, associated): (Binary, Binary, Option<Binary>)| {
                let key = key.as_bytes();
                let associated = associated
                    .map(|value| value.as_bytes().to_vec())
                    .unwrap_or_default();
                if key.len() != 32 {
                    return Err(LuaError::runtime("XChaCha20 keys have to be 32 bytes long"));
                }
                decrypt::<XChaCha20Poly1305>(
                    &key,
                    &data.as_bytes(),
                    &associated,
                    XCHACHA_NONCE_LENGTH,
                )
            },
        )?,
    )?;

    Ok(())
}

fn encrypt<C: Aead + KeyInit>(
    key: &[u8],
    plaintext: &[u8],
    associated: &[u8],
    nonce_length: usize,
) -> LuaResult<Bytes> {
    let cipher = C::new_from_slice(key).map_err(|_| LuaError::runtime("invalid key length"))?;
    let mut nonce = vec![0; nonce_length];
    getrandom::fill(&mut nonce).map_err(LuaError::external)?;

    let nonce_array = nonce
        .as_slice()
        .try_into()
        .map_err(|_| LuaError::runtime("invalid nonce length"))?;
    let ciphertext = cipher
        .encrypt(
            &nonce_array,
            Payload {
                msg: plaintext,
                aad: associated,
            },
        )
        .map_err(|_| LuaError::runtime("encryption failed"))?;

    nonce.extend_from_slice(&ciphertext);
    Ok(Bytes(nonce))
}

fn decrypt<C: Aead + KeyInit>(
    key: &[u8],
    data: &[u8],
    associated: &[u8],
    nonce_length: usize,
) -> LuaResult<Result<Bytes, String>> {
    let cipher = C::new_from_slice(key).map_err(|_| LuaError::runtime("invalid key length"))?;
    if data.len() < nonce_length + TAG_LENGTH {
        return Ok(Err("the ciphertext is too short".to_owned()));
    }

    let (nonce, ciphertext) = data.split_at(nonce_length);
    let nonce_array = nonce
        .try_into()
        .map_err(|_| LuaError::runtime("invalid nonce length"))?;
    Ok(cipher
        .decrypt(
            &nonce_array,
            Payload {
                msg: ciphertext,
                aad: associated,
            },
        )
        .map(Bytes)
        .map_err(|_| "decryption failed, the key is wrong or the data was modified".to_owned()))
}
