use std::time::Duration;

use mlua::prelude::*;

use crate::describe;

const UNITS: &[(&[&str], f64)] = &[
    (&["ms", "msec", "millisecond", "milliseconds"], 1.0),
    (&["s", "sec", "secs", "second", "seconds"], 1_000.0),
    (&["m", "min", "mins", "minute", "minutes"], 60_000.0),
    (&["h", "hr", "hrs", "hour", "hours"], 3_600_000.0),
    (&["d", "day", "days"], 86_400_000.0),
    (&["w", "week", "weeks"], 604_800_000.0),
];

pub fn parse_duration(text: &str) -> Result<Duration, String> {
    let invalid = || format!("invalid duration '{text}', expected milliseconds or a text like '5s' or '1m 30s'");
    let mut rest = text.trim();
    if rest.is_empty() {
        return Err(invalid());
    }
    let mut total = 0.0;
    while !rest.is_empty() {
        let number_end = rest
            .find(|character: char| !(character.is_ascii_digit() || character == '.'))
            .unwrap_or(rest.len());
        let amount: f64 = rest[..number_end].parse().map_err(|_| invalid())?;
        rest = rest[number_end..].trim_start();
        let unit_end = rest
            .find(|character: char| !character.is_ascii_alphabetic())
            .unwrap_or(rest.len());
        let unit = rest[..unit_end].to_ascii_lowercase();
        let factor = UNITS
            .iter()
            .find(|(names, _)| names.contains(&unit.as_str()))
            .map(|(_, factor)| *factor)
            .ok_or_else(invalid)?;
        total += amount * factor;
        rest = rest[unit_end..].trim_start_matches([' ', ',']);
    }
    if !total.is_finite() || total < 0.0 {
        return Err(invalid());
    }
    Ok(Duration::from_secs_f64(total / 1000.0))
}

fn duration_value(key: &str, value: LuaValue) -> LuaResult<Duration> {
    let invalid = |message: String| LuaError::runtime(format!("invalid option '{key}': {message}"));
    match value {
        LuaValue::Integer(milliseconds) if milliseconds >= 0 => Ok(Duration::from_millis(milliseconds as u64)),
        LuaValue::Number(milliseconds) if milliseconds.is_finite() && milliseconds >= 0.0 => {
            Ok(Duration::from_secs_f64(milliseconds / 1000.0))
        }
        LuaValue::String(text) => parse_duration(&text.to_str()?).map_err(invalid),
        other => Err(invalid(format!(
            "expected milliseconds or a text like '5s', got {}",
            other.type_name()
        ))),
    }
}

#[derive(Default)]
pub struct Options(Option<LuaTable>);

impl Options {
    pub fn from_table(table: LuaTable) -> Self {
        Options(Some(table))
    }

    pub fn get<T: FromLua>(&self, key: &str) -> LuaResult<Option<T>> {
        let Some(table) = &self.0 else {
            return Ok(None);
        };

        table.get::<Option<T>>(key).map_err(|error| {
            LuaError::runtime(format!("invalid option '{key}': {}", describe(&error)))
        })
    }

    pub fn get_or<T: FromLua>(&self, key: &str, default: T) -> LuaResult<T> {
        Ok(self.get(key)?.unwrap_or(default))
    }

    pub fn duration(&self, key: &str) -> LuaResult<Option<Duration>> {
        match self.get::<LuaValue>(key)? {
            None | Some(LuaNil) => Ok(None),
            Some(value) => duration_value(key, value).map(Some),
        }
    }

    pub fn duration_or(&self, key: &str, default_milliseconds: u64) -> LuaResult<Duration> {
        Ok(self
            .duration(key)?
            .unwrap_or(Duration::from_millis(default_milliseconds)))
    }
}

impl FromLua for Options {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        match value {
            LuaNil => Ok(Options(None)),
            LuaValue::Table(table) => Ok(Options(Some(table))),
            other => Err(LuaError::runtime(format!(
                "expected an options table, got {}",
                other.type_name()
            ))),
        }
    }
}

pub fn options_and_callback(
    value: LuaValue,
    callback: Option<LuaFunction>,
) -> LuaResult<(Options, Option<LuaFunction>)> {
    match (value, callback) {
        (LuaValue::Function(callback), None) => Ok((Options(None), Some(callback))),
        (LuaValue::Table(table), callback) => Ok((Options(Some(table)), callback)),
        (LuaNil, callback) => Ok((Options(None), callback)),
        (other, _) => Err(LuaError::runtime(format!(
            "expected an options table or a callback, got {}",
            other.type_name()
        ))),
    }
}
