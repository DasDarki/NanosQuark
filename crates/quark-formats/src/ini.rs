use ini::Ini;
use mlua::prelude::*;
use quark_core::{Binary, options_and_callback};

use crate::data::text;

pub struct Document(Ini);

impl IntoLua for Document {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let root = lua.create_table()?;
        for (section, properties) in self.0.iter() {
            let target = match section {
                None => root.clone(),
                Some(name) => match root.raw_get::<LuaValue>(name)? {
                    LuaValue::Table(existing) => existing,
                    _ => {
                        let table = lua.create_table()?;
                        root.raw_set(name, &table)?;
                        table
                    }
                },
            };
            for (key, value) in properties.iter() {
                target.raw_set(key, value)?;
            }
        }
        Ok(LuaValue::Table(root))
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "Decode",
        lua.create_function(
            |lua, (input, options, callback): (Binary, LuaValue, Option<LuaFunction>)| {
                let (_, callback) = options_and_callback(options, callback)?;
                quark_core::run(lua, callback, move || {
                    Ini::load_from_str(text(input.as_bytes())?)
                        .map(Document)
                        .map_err(|error| format!("invalid INI: {error}"))
                })
            },
        )?,
    )?;

    module.set(
        "Encode",
        lua.create_function(|_, document: LuaTable| {
            let mut ini = Ini::new();
            let mut sections = Vec::new();

            for pair in document.pairs::<String, LuaValue>() {
                let (key, value) = pair?;
                match value {
                    LuaValue::Table(section) => sections.push((key, section)),
                    value => {
                        ini.with_general_section().set(key, scalar(value)?);
                    }
                }
            }

            sections.sort_by(|left, right| left.0.cmp(&right.0));
            for (name, section) in sections {
                let mut entries = Vec::new();
                for pair in section.pairs::<String, LuaValue>() {
                    let (key, value) = pair?;
                    if let LuaValue::Table(_) = value {
                        return Err(LuaError::runtime(format!(
                            "INI sections cannot be nested, '{name}.{key}' is a table"
                        )));
                    }
                    entries.push((key, scalar(value)?));
                }
                entries.sort();
                let mut setter = ini.with_section(Some(name));
                for (key, value) in entries {
                    setter.set(key, value);
                }
            }

            let mut output = Vec::new();
            ini.write_to(&mut output).map_err(LuaError::external)?;
            String::from_utf8(output).map_err(LuaError::external)
        })?,
    )?;

    Ok(module)
}

fn scalar(value: LuaValue) -> LuaResult<String> {
    match value {
        LuaValue::String(string) => Ok(string.to_string_lossy()),
        LuaValue::Boolean(_) | LuaValue::Integer(_) | LuaValue::Number(_) => value.to_string(),
        other => Err(LuaError::runtime(format!(
            "INI values have to be strings, numbers or booleans, got {}",
            other.type_name()
        ))),
    }
}
