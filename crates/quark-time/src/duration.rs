use jiff::{Span, SpanRelativeTo, Unit};
use mlua::prelude::*;
use quark_core::Options;

const UNITS: &[(i64, &str, &str)] = &[
    (86_400_000, "d", "day"),
    (3_600_000, "h", "hour"),
    (60_000, "m", "minute"),
    (1_000, "s", "second"),
    (1, "ms", "millisecond"),
];

pub fn parse_span(text: &str) -> Result<Span, String> {
    text.trim()
        .parse::<Span>()
        .map_err(|error| format!("invalid duration '{text}': {error}"))
}

pub fn parse_milliseconds(text: &str) -> Result<i64, String> {
    let span = parse_span(text)?;
    let total = span
        .total((Unit::Millisecond, SpanRelativeTo::days_are_24_hours()))
        .map_err(|_| {
            format!("the duration '{text}' uses months or years, which have no fixed length")
        })?;
    Ok(total.round() as i64)
}

pub fn read_milliseconds(value: LuaValue) -> LuaResult<i64> {
    match value {
        LuaValue::Integer(milliseconds) => Ok(milliseconds),
        LuaValue::Number(milliseconds) => Ok(milliseconds.round() as i64),
        LuaValue::String(text) => parse_milliseconds(&text.to_str()?).map_err(LuaError::runtime),
        other => Err(LuaError::runtime(format!(
            "a duration has to be milliseconds or a text like '1h 30m', got {}",
            other.type_name()
        ))),
    }
}

fn format(milliseconds: i64, options: &Options) -> LuaResult<String> {
    let style: String = options.get_or("Style", "Short".to_owned())?;
    let largest: usize = options.get_or("Largest", UNITS.len())?;
    let sign = if milliseconds < 0 { "-" } else { "" };
    let mut rest = milliseconds.unsigned_abs() as i64;

    if style == "Clock" {
        let hours = rest / 3_600_000;
        let minutes = rest / 60_000 % 60;
        let seconds = rest / 1_000 % 60;
        return Ok(if hours > 0 {
            format!("{sign}{hours}:{minutes:02}:{seconds:02}")
        } else {
            format!("{sign}{minutes}:{seconds:02}")
        });
    }
    let long = match style.as_str() {
        "Short" => false,
        "Long" => true,
        other => {
            return Err(LuaError::runtime(format!(
                "unknown style '{other}', expected Short, Long or Clock"
            )));
        }
    };

    let mut parts = Vec::new();
    for &(size, short, name) in UNITS {
        if parts.len() >= largest.max(1) {
            break;
        }
        let amount = rest / size;
        rest %= size;
        if amount == 0 || (size == 1 && !parts.is_empty()) {
            continue;
        }
        parts.push(if long {
            format!("{amount} {name}{}", if amount == 1 { "" } else { "s" })
        } else {
            format!("{amount}{short}")
        });
    }
    if parts.is_empty() {
        parts.push(if long {
            "0 seconds".to_owned()
        } else {
            "0s".to_owned()
        });
    }
    Ok(format!(
        "{sign}{}",
        parts.join(if long { ", " } else { " " })
    ))
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "ParseDuration",
        lua.create_function(|lua, text: String| match parse_milliseconds(&text) {
            Ok(milliseconds) => milliseconds.into_lua_multi(lua),
            Err(message) => (LuaNil, message).into_lua_multi(lua),
        })?,
    )?;
    module.set(
        "FormatDuration",
        lua.create_function(|_, (milliseconds, options): (LuaValue, Options)| {
            format(read_milliseconds(milliseconds)?, &options)
        })?,
    )?;
    Ok(())
}
