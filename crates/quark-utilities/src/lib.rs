mod bitset;
mod log;

use jiff::{Span, SpanRelativeTo, Unit};
use mlua::prelude::*;
use quark_core::{clock, report_error};

pub use self::bitset::build as bitset;
pub use self::log::build as log;

const RATE_LIMIT_SOURCE: &str = include_str!("rate_limit.lua");
const CACHE_SOURCE: &str = include_str!("cache.lua");
const PRIORITY_QUEUE_SOURCE: &str = include_str!("priority_queue.lua");
const RING_BUFFER_SOURCE: &str = include_str!("ring_buffer.lua");
const METRICS_SOURCE: &str = include_str!("metrics.lua");

fn parse_duration(text: &str) -> Result<f64, String> {
    let span: Span = text
        .trim()
        .parse()
        .map_err(|error| format!("invalid duration '{text}': {error}"))?;
    span.total((Unit::Millisecond, SpanRelativeTo::days_are_24_hours()))
        .map_err(|_| {
            format!("the duration '{text}' uses months or years, which have no fixed length")
        })
}

fn helpers(lua: &Lua) -> LuaResult<(LuaFunction, LuaFunction, LuaFunction)> {
    let clock = lua.create_function(|_, ()| Ok(clock()))?;
    let duration = lua.create_function(|_, value: LuaValue| match value {
        LuaValue::Integer(milliseconds) => Ok(milliseconds as f64),
        LuaValue::Number(milliseconds) => Ok(milliseconds),
        LuaValue::String(text) => parse_duration(&text.to_str()?).map_err(LuaError::runtime),
        other => Err(LuaError::runtime(format!(
            "a duration has to be milliseconds or a text like '10s', got {}",
            other.type_name()
        ))),
    })?;
    let report = lua.create_function(|lua, message: String| {
        report_error(lua, &LuaError::runtime(message));
        Ok(())
    })?;
    Ok((clock, duration, report))
}

fn load(lua: &Lua, source: &str, name: &str) -> LuaResult<LuaTable> {
    let (clock, duration, report) = helpers(lua)?;
    lua.load(source)
        .set_name(format!("=quark/{name}.lua"))
        .call((clock, duration, report))
}

pub fn rate_limit(lua: &Lua) -> LuaResult<LuaTable> {
    load(lua, RATE_LIMIT_SOURCE, "rate_limit")
}

pub fn cache(lua: &Lua) -> LuaResult<LuaTable> {
    load(lua, CACHE_SOURCE, "cache")
}

pub fn priority_queue(lua: &Lua) -> LuaResult<LuaTable> {
    load(lua, PRIORITY_QUEUE_SOURCE, "priority_queue")
}

pub fn ring_buffer(lua: &Lua) -> LuaResult<LuaTable> {
    load(lua, RING_BUFFER_SOURCE, "ring_buffer")
}

pub fn metrics(lua: &Lua) -> LuaResult<LuaTable> {
    let (clock, _, report) = helpers(lua)?;
    lua.load(METRICS_SOURCE)
        .set_name("=quark/metrics.lua")
        .call((clock, report))
}
