use mlua::prelude::*;
use quark_core::Options;
use serde::Serialize;
use serde_json::Value;
use serde_json::ser::PrettyFormatter;

use crate::data::{add_values, decode, encode, read};

const DEFAULT_INDENT: usize = 2;

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    add_values(lua, &module)?;

    module.set(
        "Encode",
        lua.create_function(|lua, arguments| {
            encode(lua, arguments, |value: Value, options: &Options| {
                let indent = match (
                    options.get_or("Pretty", false)?,
                    options.get::<usize>("Indent")?,
                ) {
                    (_, Some(indent)) => Some(indent),
                    (true, None) => Some(DEFAULT_INDENT),
                    (false, None) => None,
                };
                Ok(Box::new(move || to_string(&value, indent)))
            })
        })?,
    )?;

    module.set(
        "Decode",
        lua.create_function(|lua, arguments| {
            decode(lua, arguments, |bytes| {
                serde_json::from_slice::<Value>(bytes)
                    .map_err(|error| format!("invalid JSON: {error}"))
            })
        })?,
    )?;

    module.set(
        "Validate",
        lua.create_function(|lua, (value, schema): (LuaValue, LuaValue)| {
            let schema = Schema::compile(lua, schema)?;
            schema.validate(lua, value)
        })?,
    )?;

    module.set(
        "Schema",
        lua.create_function(|lua, schema: LuaValue| {
            Ok(quark_core::Object(Schema::compile(lua, schema)?))
        })?,
    )?;

    Ok(module)
}

struct Schema(jsonschema::Validator);

impl Schema {
    fn compile(lua: &Lua, schema: LuaValue) -> LuaResult<Self> {
        let schema: Value = match schema {
            LuaValue::String(text) => serde_json::from_slice(&text.as_bytes())
                .map_err(|error| LuaError::runtime(format!("invalid JSON schema: {error}")))?,
            other => read(lua, other, &Options::default())?,
        };
        jsonschema::validator_for(&schema)
            .map(Schema)
            .map_err(|error| LuaError::runtime(format!("invalid JSON schema: {error}")))
    }

    fn validate(&self, lua: &Lua, value: LuaValue) -> LuaResult<LuaMultiValue> {
        let instance: Value = read(lua, value, &Options::default())?;
        let errors = lua.create_table()?;
        for error in self.0.iter_errors(&instance) {
            let entry = lua.create_table()?;
            entry.set("Path", error.instance_path().to_string())?;
            entry.set("Message", error.to_string())?;
            errors.raw_push(entry)?;
        }

        if errors.raw_len() == 0 {
            true.into_lua_multi(lua)
        } else {
            (false, errors).into_lua_multi(lua)
        }
    }
}

impl LuaUserData for Schema {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Validate", |lua, schema, value: LuaValue| {
            schema.validate(lua, value)
        });
        methods.add_method("IsValid", |lua, schema, value: LuaValue| {
            let instance: Value = read(lua, value, &Options::default())?;
            Ok(schema.0.is_valid(&instance))
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, _, ()| Ok("JSONSchema"));
    }
}

fn to_string(value: &Value, indent: Option<usize>) -> Result<String, String> {
    let Some(indent) = indent else {
        return serde_json::to_string(value).map_err(|error| error.to_string());
    };

    let indentation = " ".repeat(indent);
    let mut output = Vec::new();
    let mut serializer = serde_json::Serializer::with_formatter(
        &mut output,
        PrettyFormatter::with_indent(indentation.as_bytes()),
    );
    value
        .serialize(&mut serializer)
        .map_err(|error| error.to_string())?;
    String::from_utf8(output).map_err(|error| error.to_string())
}
