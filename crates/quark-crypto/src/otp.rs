use mlua::prelude::*;
use quark_core::Options;
use subtle::ConstantTimeEq;

use crate::encoding::decode_base32;
use crate::hash::{HashAlgorithm, hmac};
use crate::now_seconds;
use crate::random::random_bytes;

const DEFAULT_SECRET_BYTES: usize = 20;
const DEFAULT_DIGITS: u32 = 6;
const DEFAULT_PERIOD: u64 = 30;
const DEFAULT_WINDOW: u64 = 1;

struct Settings {
    algorithm: HashAlgorithm,
    digits: u32,
    period: u64,
}

impl Settings {
    fn parse(options: &Options) -> LuaResult<Self> {
        let algorithm = options.get_or("Algorithm", HashAlgorithm::Sha1)?;
        if !matches!(
            algorithm,
            HashAlgorithm::Sha1 | HashAlgorithm::Sha256 | HashAlgorithm::Sha512
        ) {
            return Err(LuaError::runtime(
                "one time passwords support SHA1, SHA256 and SHA512",
            ));
        }

        let digits = options.get_or("Digits", DEFAULT_DIGITS)?;
        if !(6..=10).contains(&digits) {
            return Err(LuaError::runtime(
                "one time passwords need between 6 and 10 digits",
            ));
        }

        let period = options.get_or("Period", DEFAULT_PERIOD)?;
        if period == 0 {
            return Err(LuaError::runtime(
                "the period has to be at least one second",
            ));
        }

        Ok(Settings {
            algorithm,
            digits,
            period,
        })
    }

    fn algorithm_name(&self) -> &'static str {
        match self.algorithm {
            HashAlgorithm::Sha256 => "SHA256",
            HashAlgorithm::Sha512 => "SHA512",
            _ => "SHA1",
        }
    }
}

pub fn register(lua: &Lua, crypto: &LuaTable) -> LuaResult<()> {
    crypto.set(
        "TOTPGenerateSecret",
        lua.create_function(|_, count: Option<usize>| {
            let count = count.unwrap_or(DEFAULT_SECRET_BYTES);
            if !(10..=64).contains(&count) {
                return Err(LuaError::runtime(
                    "TOTP secrets have to be between 10 and 64 bytes",
                ));
            }
            Ok(data_encoding::BASE32_NOPAD.encode(&random_bytes(count)?))
        })?,
    )?;

    crypto.set(
        "TOTP",
        lua.create_function(|_, (secret, options): (LuaString, Options)| {
            let settings = Settings::parse(&options)?;
            let time = options.get_or("Time", now_seconds())?;
            hotp(&secret_bytes(&secret)?, time / settings.period, &settings)
        })?,
    )?;

    crypto.set(
        "TOTPVerify",
        lua.create_function(
            |lua, (secret, code, options): (LuaString, LuaValue, Options)| {
                let settings = Settings::parse(&options)?;
                let time = options.get_or("Time", now_seconds())?;
                let window = options.get_or("Window", DEFAULT_WINDOW)?;
                let code = code_text(lua, code, settings.digits)?;
                let secret = secret_bytes(&secret)?;

                let counter = time / settings.period;
                let mut valid = false;
                for candidate in counter.saturating_sub(window)..=counter.saturating_add(window) {
                    let expected = hotp(&secret, candidate, &settings)?;
                    valid |= bool::from(expected.as_bytes().ct_eq(code.as_bytes()));
                }
                Ok(valid)
            },
        )?,
    )?;

    crypto.set(
        "HOTP",
        lua.create_function(|_, (secret, counter, options): (LuaString, u64, Options)| {
            let settings = Settings::parse(&options)?;
            hotp(&secret_bytes(&secret)?, counter, &settings)
        })?,
    )?;

    crypto.set(
        "TOTPUri",
        lua.create_function(|_, (secret, account, options): (String, String, Options)| {
            let settings = Settings::parse(&options)?;
            let issuer = options.get::<String>("Issuer")?;
            decode_base32(secret.as_bytes()).map_err(LuaError::runtime)?;

            let label = match &issuer {
                Some(issuer) => format!("{}:{}", percent_encode(issuer), percent_encode(&account)),
                None => percent_encode(&account),
            };
            let mut uri = format!(
                "otpauth://totp/{label}?secret={}&algorithm={}&digits={}&period={}",
                secret.replace([' ', '='], "").to_ascii_uppercase(),
                settings.algorithm_name(),
                settings.digits,
                settings.period
            );
            if let Some(issuer) = issuer {
                uri.push_str("&issuer=");
                uri.push_str(&percent_encode(&issuer));
            }
            Ok(uri)
        })?,
    )?;

    Ok(())
}

fn hotp(secret: &[u8], counter: u64, settings: &Settings) -> LuaResult<String> {
    let mac = hmac(settings.algorithm, secret, &counter.to_be_bytes())?;
    let offset = (mac[mac.len() - 1] & 0x0f) as usize;
    let binary = u32::from_be_bytes([
        mac[offset] & 0x7f,
        mac[offset + 1],
        mac[offset + 2],
        mac[offset + 3],
    ]);
    let code = u64::from(binary) % 10u64.pow(settings.digits);
    Ok(format!("{code:0width$}", width = settings.digits as usize))
}

fn secret_bytes(secret: &LuaString) -> LuaResult<Vec<u8>> {
    decode_base32(&secret.as_bytes()).map_err(LuaError::runtime)
}

fn code_text(lua: &Lua, code: LuaValue, digits: u32) -> LuaResult<String> {
    match code {
        LuaValue::Integer(number) => Ok(format!("{number:0width$}", width = digits as usize)),
        LuaValue::Number(number) if number.fract() == 0.0 => Ok(format!(
            "{:0width$}",
            number as i64,
            width = digits as usize
        )),
        other => Ok(String::from_lua(other, lua)?.trim().replace(' ', "")),
    }
}

fn percent_encode(text: &str) -> String {
    text.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}
