use std::cell::RefCell;
use std::collections::HashMap;

use mlua::prelude::*;
use quark_core::{Object, Options};
use regex::bytes::{Captures, Regex as Pattern, RegexBuilder};

const CACHE_SIZE: usize = 128;
const SIZE_LIMIT: usize = 4 * 1024 * 1024;

pub struct Regex(Pattern);

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
struct Flags {
    case_insensitive: bool,
    multiline: bool,
    dot_all: bool,
    extended: bool,
    ascii: bool,
}

#[derive(Default)]
struct Cache(RefCell<HashMap<(String, Flags), Pattern>>);

impl Flags {
    fn read(options: &Options) -> LuaResult<Self> {
        Ok(Flags {
            case_insensitive: options.get_or("CaseInsensitive", false)?,
            multiline: options.get_or("Multiline", false)?,
            dot_all: options.get_or("DotAll", false)?,
            extended: options.get_or("Extended", false)?,
            ascii: options.get_or("Ascii", false)?,
        })
    }
}

fn compile(pattern: &str, flags: Flags) -> Result<Pattern, String> {
    RegexBuilder::new(pattern)
        .case_insensitive(flags.case_insensitive)
        .multi_line(flags.multiline)
        .dot_matches_new_line(flags.dot_all)
        .ignore_whitespace(flags.extended)
        .unicode(!flags.ascii)
        .size_limit(SIZE_LIMIT)
        .build()
        .map_err(|error| match error {
            regex::Error::Syntax(message) => format!("invalid regular expression: {message}"),
            regex::Error::CompiledTooBig(_) => "the regular expression is too large".to_owned(),
            other => other.to_string(),
        })
}

fn cached(lua: &Lua, pattern: &str) -> LuaResult<Pattern> {
    if lua.app_data_ref::<Cache>().is_none() {
        lua.set_app_data(Cache::default());
    }
    let cache = lua
        .app_data_ref::<Cache>()
        .ok_or_else(|| LuaError::runtime("failed to access the regular expression cache"))?;
    let key = (pattern.to_owned(), Flags::default());
    if let Some(compiled) = cache.0.borrow().get(&key) {
        return Ok(compiled.clone());
    }
    let compiled = compile(pattern, Flags::default()).map_err(LuaError::runtime)?;
    let mut entries = cache.0.borrow_mut();
    if entries.len() >= CACHE_SIZE {
        entries.clear();
    }
    entries.insert(key, compiled.clone());
    Ok(compiled)
}

fn start_offset(text: &[u8], start: Option<i64>) -> LuaResult<Option<usize>> {
    let length = text.len() as i64;
    let start = match start.unwrap_or(1) {
        0 => 1,
        start if start < 0 => (length + start + 1).max(1),
        start => start,
    };
    if start > length + 1 {
        return Ok(None);
    }
    Ok(Some(start as usize - 1))
}

fn match_table(lua: &Lua, pattern: &Pattern, captures: &Captures) -> LuaResult<LuaTable> {
    let whole = captures.get(0).ok_or_else(|| LuaError::runtime("a match without text"))?;
    let table = lua.create_table()?;
    table.set("Text", lua.create_string(whole.as_bytes())?)?;
    table.set("Start", whole.start() + 1)?;
    table.set("End", whole.end())?;

    let groups = lua.create_table()?;
    for index in 1..captures.len() {
        match captures.get(index) {
            Some(group) => groups.raw_set(index, lua.create_string(group.as_bytes())?)?,
            None => groups.raw_set(index, false)?,
        }
    }
    table.set("Groups", groups)?;

    let named = lua.create_table()?;
    for name in pattern.capture_names().flatten() {
        if let Some(group) = captures.name(name) {
            named.set(name, lua.create_string(group.as_bytes())?)?;
        }
    }
    table.set("Named", named)?;
    Ok(table)
}

fn is_match(pattern: &Pattern, text: &[u8], start: Option<i64>) -> LuaResult<bool> {
    Ok(match start_offset(text, start)? {
        Some(offset) => pattern.is_match_at(text, offset),
        None => false,
    })
}

fn find(lua: &Lua, pattern: &Pattern, text: &[u8], start: Option<i64>) -> LuaResult<LuaMultiValue> {
    let found = start_offset(text, start)?.and_then(|offset| pattern.find_at(text, offset));
    match found {
        Some(found) => (found.start() + 1, found.end(), lua.create_string(found.as_bytes())?).into_lua_multi(lua),
        None => LuaNil.into_lua_multi(lua),
    }
}

fn first_match(lua: &Lua, pattern: &Pattern, text: &[u8], start: Option<i64>) -> LuaResult<Option<LuaTable>> {
    let Some(offset) = start_offset(text, start)? else {
        return Ok(None);
    };
    pattern
        .captures_at(text, offset)
        .map(|captures| match_table(lua, pattern, &captures))
        .transpose()
}

fn match_all(lua: &Lua, pattern: &Pattern, text: &[u8], limit: Option<usize>) -> LuaResult<LuaTable> {
    let result = lua.create_table()?;
    for captures in pattern.captures_iter(text).take(limit.unwrap_or(usize::MAX)) {
        result.raw_push(match_table(lua, pattern, &captures)?)?;
    }
    Ok(result)
}

fn replace(
    lua: &Lua,
    pattern: &Pattern,
    text: &[u8],
    replacement: LuaValue,
    limit: Option<usize>,
) -> LuaResult<(LuaString, usize)> {
    let limit = limit.unwrap_or(usize::MAX);
    let mut output = Vec::with_capacity(text.len());
    let mut last = 0;
    let mut count = 0;

    for captures in pattern.captures_iter(text).take(limit) {
        let whole = captures.get(0).ok_or_else(|| LuaError::runtime("a match without text"))?;
        output.extend_from_slice(&text[last..whole.start()]);
        match &replacement {
            LuaValue::String(template) => captures.expand(&template.as_bytes(), &mut output),
            LuaValue::Integer(_) | LuaValue::Number(_) => {
                output.extend_from_slice(replacement.to_string()?.as_bytes())
            }
            LuaValue::Function(function) => {
                match function.call::<LuaValue>(match_table(lua, pattern, &captures)?)? {
                    LuaValue::String(value) => output.extend_from_slice(&value.as_bytes()),
                    LuaValue::Integer(value) => output.extend_from_slice(value.to_string().as_bytes()),
                    LuaValue::Number(value) => output.extend_from_slice(value.to_string().as_bytes()),
                    LuaNil | LuaValue::Boolean(false) => output.extend_from_slice(whole.as_bytes()),
                    other => {
                        return Err(LuaError::runtime(format!(
                            "the replacement function has to return a string or nil, got {}",
                            other.type_name()
                        )));
                    }
                }
            }
            LuaValue::Table(lookup) => match lookup.get::<LuaValue>(lua.create_string(whole.as_bytes())?)? {
                LuaValue::String(value) => output.extend_from_slice(&value.as_bytes()),
                LuaNil | LuaValue::Boolean(false) => output.extend_from_slice(whole.as_bytes()),
                other => output.extend_from_slice(other.to_string()?.as_bytes()),
            },
            other => {
                return Err(LuaError::runtime(format!(
                    "the replacement has to be a string, a table or a function, got {}",
                    other.type_name()
                )));
            }
        }
        last = whole.end();
        count += 1;
    }
    output.extend_from_slice(&text[last..]);
    Ok((lua.create_string(output)?, count))
}

fn split(lua: &Lua, pattern: &Pattern, text: &[u8], limit: Option<usize>) -> LuaResult<LuaTable> {
    let result = lua.create_table()?;
    let parts: Box<dyn Iterator<Item = &[u8]>> = match limit {
        Some(limit) => Box::new(pattern.splitn(text, limit.max(1))),
        None => Box::new(pattern.split(text)),
    };
    for part in parts {
        result.raw_push(lua.create_string(part)?)?;
    }
    Ok(result)
}

impl LuaUserData for Regex {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("IsMatch", |_, regex, (text, start): (LuaString, Option<i64>)| {
            is_match(&regex.0, &text.as_bytes(), start)
        });
        methods.add_method("Find", |lua, regex, (text, start): (LuaString, Option<i64>)| {
            find(lua, &regex.0, &text.as_bytes(), start)
        });
        methods.add_method("Match", |lua, regex, (text, start): (LuaString, Option<i64>)| {
            first_match(lua, &regex.0, &text.as_bytes(), start)
        });
        methods.add_method("MatchAll", |lua, regex, (text, limit): (LuaString, Option<usize>)| {
            match_all(lua, &regex.0, &text.as_bytes(), limit)
        });
        methods.add_method(
            "Replace",
            |lua, regex, (text, replacement, limit): (LuaString, LuaValue, Option<usize>)| {
                replace(lua, &regex.0, &text.as_bytes(), replacement, limit)
            },
        );
        methods.add_method("Split", |lua, regex, (text, limit): (LuaString, Option<usize>)| {
            split(lua, &regex.0, &text.as_bytes(), limit)
        });
        methods.add_method("Count", |_, regex, text: LuaString| Ok(regex.0.find_iter(&text.as_bytes()).count()));
        methods.add_method("GetPattern", |_, regex, ()| Ok(regex.0.as_str().to_owned()));
        methods.add_method("GetGroupCount", |_, regex, ()| Ok(regex.0.captures_len() - 1));
        methods.add_method("GetGroupNames", |lua, regex, ()| {
            lua.create_sequence_from(regex.0.capture_names().flatten().map(str::to_owned))
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, regex, ()| Ok(format!("Regex({})", regex.0.as_str())));
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "New",
        lua.create_function(|lua, (pattern, options): (String, Options)| {
            let flags = Flags::read(&options)?;
            match compile(&pattern, flags) {
                Ok(compiled) => Object(Regex(compiled)).into_lua_multi(lua),
                Err(message) => (LuaNil, message).into_lua_multi(lua),
            }
        })?,
    )?;
    module.set(
        "IsValid",
        lua.create_function(|_, pattern: String| Ok(compile(&pattern, Flags::default()).is_ok()))?,
    )?;
    module.set("Escape", lua.create_function(|_, text: String| Ok(regex::escape(&text)))?)?;

    module.set(
        "IsMatch",
        lua.create_function(|lua, (pattern, text, start): (String, LuaString, Option<i64>)| {
            is_match(&cached(lua, &pattern)?, &text.as_bytes(), start)
        })?,
    )?;
    module.set(
        "Find",
        lua.create_function(|lua, (pattern, text, start): (String, LuaString, Option<i64>)| {
            find(lua, &cached(lua, &pattern)?, &text.as_bytes(), start)
        })?,
    )?;
    module.set(
        "Match",
        lua.create_function(|lua, (pattern, text, start): (String, LuaString, Option<i64>)| {
            first_match(lua, &cached(lua, &pattern)?, &text.as_bytes(), start)
        })?,
    )?;
    module.set(
        "MatchAll",
        lua.create_function(|lua, (pattern, text, limit): (String, LuaString, Option<usize>)| {
            match_all(lua, &cached(lua, &pattern)?, &text.as_bytes(), limit)
        })?,
    )?;
    module.set(
        "Replace",
        lua.create_function(
            |lua, (pattern, text, replacement, limit): (String, LuaString, LuaValue, Option<usize>)| {
                replace(lua, &cached(lua, &pattern)?, &text.as_bytes(), replacement, limit)
            },
        )?,
    )?;
    module.set(
        "Split",
        lua.create_function(|lua, (pattern, text, limit): (String, LuaString, Option<usize>)| {
            split(lua, &cached(lua, &pattern)?, &text.as_bytes(), limit)
        })?,
    )?;

    Ok(module)
}
