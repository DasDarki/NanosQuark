use mlua::prelude::*;
use minijinja::syntax::SyntaxConfig;
use minijinja::value::Serde;
use minijinja::{AutoEscape, Environment, UndefinedBehavior, Value};
use quark_core::{Options, create_userdata};

const DEFAULT_FUEL: u64 = 10_000_000;

pub struct Templates(Environment<'static>);

fn configure(options: &Options) -> LuaResult<Environment<'static>> {
    let mut environment = Environment::new();

    let escape = match options.get::<String>("AutoEscape")?.as_deref() {
        None | Some("None") => AutoEscape::None,
        Some("Html") => AutoEscape::Html,
        Some("Json") => AutoEscape::Json,
        Some(other) => {
            return Err(LuaError::runtime(format!("unknown AutoEscape '{other}', expected None, Html or Json")));
        }
    };
    environment.set_auto_escape_callback(move |_| escape.clone());

    environment.set_undefined_behavior(match options.get::<String>("Undefined")?.as_deref() {
        None | Some("Lenient") => UndefinedBehavior::Lenient,
        Some("Chainable") => UndefinedBehavior::Chainable,
        Some("Strict") => UndefinedBehavior::Strict,
        Some(other) => {
            return Err(LuaError::runtime(format!(
                "unknown Undefined behavior '{other}', expected Lenient, Chainable or Strict"
            )));
        }
    });

    environment.set_syntax(
        SyntaxConfig::builder()
            .trim_blocks(options.get_or("TrimBlocks", false)?)
            .lstrip_blocks(options.get_or("LstripBlocks", false)?)
            .keep_trailing_newline(options.get_or("KeepTrailingNewline", false)?)
            .build()
            .map_err(|error| LuaError::runtime(error.to_string()))?,
    );
    environment.set_fuel(Some(options.get_or("Fuel", DEFAULT_FUEL)?));

    if let Some(globals) = options.get::<LuaTable>("Globals")? {
        for pair in globals.pairs::<String, LuaValue>() {
            let (name, value) = pair?;
            environment.add_global(name, convert(&globals, value)?);
        }
    }
    Ok(environment)
}

fn convert(owner: &LuaTable, value: LuaValue) -> LuaResult<Value> {
    let lua = owner.weak_lua().try_upgrade().ok_or_else(|| LuaError::runtime("the Lua state is gone"))?;
    to_value(&lua, value)
}

fn to_value(lua: &Lua, value: LuaValue) -> LuaResult<Value> {
    let json: serde_json::Value = lua.from_value_with(
        value,
        LuaDeserializeOptions::new().deny_unsupported_types(false).deny_recursive_tables(true),
    )?;
    Ok(Value::from(Serde(json)))
}

fn describe(error: &minijinja::Error) -> String {
    let mut message = error.to_string();
    let mut source = std::error::Error::source(error);
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}

fn context(lua: &Lua, value: Option<LuaValue>) -> LuaResult<Value> {
    match value {
        None | Some(LuaNil) => Ok(Value::from(())),
        Some(value @ LuaValue::Table(_)) => to_value(lua, value),
        Some(other) => Err(LuaError::runtime(format!(
            "the template context has to be a table, got {}",
            other.type_name()
        ))),
    }
}

fn result(lua: &Lua, rendered: Result<String, minijinja::Error>) -> LuaResult<LuaMultiValue> {
    match rendered {
        Ok(text) => text.into_lua_multi(lua),
        Err(error) => (LuaNil, describe(&error)).into_lua_multi(lua),
    }
}

impl LuaUserData for Templates {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("Add", |lua, templates, (name, source): (String, String)| {
            match templates.0.add_template_owned(name, source) {
                Ok(()) => true.into_lua_multi(lua),
                Err(error) => (LuaNil, describe(&error)).into_lua_multi(lua),
            }
        });
        methods.add_method_mut("Remove", |_, templates, name: String| {
            templates.0.remove_template(&name);
            Ok(())
        });
        methods.add_method("Has", |_, templates, name: String| Ok(templates.0.get_template(&name).is_ok()));
        methods.add_method("GetNames", |lua, templates, ()| {
            let mut names: Vec<String> = templates.0.templates().map(|(name, _)| name.to_owned()).collect();
            names.sort();
            lua.create_sequence_from(names)
        });
        methods.add_method("Render", |lua, templates, (name, values): (String, Option<LuaValue>)| {
            let values = context(lua, values)?;
            let rendered = templates.0.get_template(&name).and_then(|template| template.render(values));
            result(lua, rendered)
        });
        methods.add_method(
            "RenderString",
            |lua, templates, (source, values): (String, Option<LuaValue>)| {
                let values = context(lua, values)?;
                result(lua, templates.0.render_str(&source, values))
            },
        );
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "Render",
        lua.create_function(|lua, (source, values, options): (String, Option<LuaValue>, Options)| {
            let environment = configure(&options)?;
            let values = context(lua, values)?;
            result(lua, environment.render_str(&source, values))
        })?,
    )?;
    module.set(
        "New",
        lua.create_function(|lua, options: Options| {
            let mut environment = configure(&options)?;
            if let Some(sources) = options.get::<LuaTable>("Templates")? {
                for pair in sources.pairs::<String, String>() {
                    let (name, source) = pair?;
                    if let Err(error) = environment.add_template_owned(name, source) {
                        return (LuaNil, describe(&error)).into_lua_multi(lua);
                    }
                }
            }
            create_userdata(lua, Templates(environment))?.into_lua_multi(lua)
        })?,
    )?;

    Ok(module)
}
