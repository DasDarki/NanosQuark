mod dice;
mod generator;
mod noise;
mod operations;
mod picker;
mod rng;

use mlua::prelude::*;

use generator::Generator;

const LOOT_SOURCE: &str = include_str!("loot.lua");

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    generator::register(lua, &module)?;
    picker::register(lua, &module)?;
    noise::register(lua, &module)?;

    module.set(
        "IsGenerator",
        lua.create_function(|_, value: LuaValue| {
            Ok(matches!(value, LuaValue::UserData(userdata) if userdata.is::<Generator>()))
        })?,
    )?;

    let loot_table: LuaFunction = lua
        .load(LOOT_SOURCE)
        .set_name("=quark/random/loot.lua")
        .call((
            module.get::<LuaFunction>("WeightedPicker")?,
            module.get::<LuaFunction>("Int")?,
            module.get::<LuaFunction>("Float")?,
            module.get::<LuaFunction>("IsGenerator")?,
        ))?;
    module.set("LootTable", loot_table)?;

    Ok(module)
}
