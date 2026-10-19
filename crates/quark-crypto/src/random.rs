use mlua::prelude::*;
use quark_core::{Binary, Bytes};
use subtle::ConstantTimeEq;

use crate::encoding::Format;

const MAX_RANDOM_LENGTH: usize = 1 << 20;
const DEFAULT_ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const DEFAULT_TOKEN_BYTES: usize = 32;

pub fn register(lua: &Lua, crypto: &LuaTable) -> LuaResult<()> {
    crypto.set(
        "RandomBytes",
        lua.create_function(|_, count: usize| {
            check_length(count)?;
            Ok(Bytes(random_bytes(count)?))
        })?,
    )?;

    crypto.set(
        "RandomInt",
        lua.create_function(|_, (minimum, maximum): (i64, i64)| {
            if minimum > maximum {
                return Err(LuaError::runtime(
                    "the minimum must not be greater than the maximum",
                ));
            }
            let span = (maximum as i128 - minimum as i128 + 1) as u128;
            Ok((minimum as i128 + random_below(span)? as i128) as i64)
        })?,
    )?;

    crypto.set(
        "RandomString",
        lua.create_function(|_, (length, alphabet): (usize, Option<String>)| {
            check_length(length)?;
            let alphabet = alphabet
                .unwrap_or_else(|| DEFAULT_ALPHABET.to_owned())
                .chars()
                .collect::<Vec<_>>();
            if alphabet.is_empty() {
                return Err(LuaError::runtime("the alphabet must not be empty"));
            }

            (0..length)
                .map(|_| Ok(alphabet[random_below(alphabet.len() as u128)? as usize]))
                .collect::<LuaResult<String>>()
        })?,
    )?;

    crypto.set(
        "RandomToken",
        lua.create_function(|_, count: Option<usize>| {
            let count = count.unwrap_or(DEFAULT_TOKEN_BYTES);
            check_length(count)?;
            Ok(Format::Base64Url.encode(&random_bytes(count)?))
        })?,
    )?;

    crypto.set(
        "ConstantTimeEquals",
        lua.create_function(|_, (left, right): (Binary, Binary)| {
            Ok(bool::from(left.as_bytes().ct_eq(right.as_bytes())))
        })?,
    )?;

    Ok(())
}

pub fn random_bytes(count: usize) -> LuaResult<Vec<u8>> {
    let mut bytes = vec![0; count];
    getrandom::fill(&mut bytes).map_err(LuaError::external)?;
    Ok(bytes)
}

fn random_below(bound: u128) -> LuaResult<u128> {
    let limit = u128::MAX - u128::MAX % bound;
    loop {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).map_err(LuaError::external)?;
        let value = u128::from_le_bytes(bytes);
        if value < limit {
            return Ok(value % bound);
        }
    }
}

fn check_length(length: usize) -> LuaResult<()> {
    if length > MAX_RANDOM_LENGTH {
        return Err(LuaError::runtime(format!(
            "cannot generate more than {MAX_RANDOM_LENGTH} random values at once"
        )));
    }
    Ok(())
}
