use mlua::prelude::*;

use crate::{clock, promise, protect_table};

pub struct Module {
    pub name: &'static str,
    pub build: fn(&Lua) -> LuaResult<LuaTable>,
}

pub fn create_root(lua: &Lua, version: &str, modules: &'static [Module]) -> LuaResult<LuaTable> {
    let root = lua.create_table()?;
    root.set("Version", version)?;

    let import_root = root.clone();
    root.set(
        "Import",
        lua.create_function(move |lua, name: String| {
            let module = find(modules, &name).ok_or_else(|| {
                let available = modules
                    .iter()
                    .map(|module| module.name)
                    .collect::<Vec<_>>()
                    .join(", ");
                LuaError::runtime(format!(
                    "Quark has no module '{name}'. Available modules: {available}"
                ))
            })?;
            load(lua, &import_root, module)
        })?,
    )?;

    root.set(
        "Modules",
        lua.create_function(move |lua, ()| {
            lua.create_sequence_from(modules.iter().map(|module| module.name))
        })?,
    )?;

    let promise = promise(lua)?;
    for name in ["Async", "Await", "Delay"] {
        root.set(name, promise.get::<LuaFunction>(name)?)?;
    }
    root.set("Clock", lua.create_function(|_, ()| Ok(clock()))?)?;
    root.set("Configure", lua.create_function(crate::config::configure)?)?;
    root.set(
        "GetConfiguration",
        lua.create_function(crate::config::configuration)?,
    )?;

    protect_table(lua, &root)?;

    let metatable = lua.create_table()?;
    metatable.set(
        "__index",
        lua.create_function(move |lua, (root, key): (LuaTable, LuaValue)| {
            let LuaValue::String(key) = key else {
                return Ok(LuaNil);
            };

            match find(modules, &key.to_str()?) {
                Some(module) => load(lua, &root, module).map(LuaValue::Table),
                None => Ok(LuaNil),
            }
        })?,
    )?;
    root.set_metatable(Some(metatable))?;

    Ok(root)
}

fn find(modules: &'static [Module], name: &str) -> Option<&'static Module> {
    modules.iter().find(|module| module.name == name)
}

fn load(lua: &Lua, root: &LuaTable, module: &Module) -> LuaResult<LuaTable> {
    if let Some(table) = root.raw_get::<Option<LuaTable>>(module.name)? {
        return Ok(table);
    }

    let table = (module.build)(lua)?;
    protect_table(lua, &table)?;
    root.raw_set(module.name, &table)?;
    Ok(table)
}
