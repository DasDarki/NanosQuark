mod bulk;
mod graph;
mod grid;
mod kdtree;
mod spatial;

use mlua::prelude::*;

pub fn pathfinding(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Grid", lua.create_function(grid::create)?)?;
    module.set("Graph", lua.create_function(graph::create)?)?;
    Ok(module)
}

pub fn spatial(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Hash", lua.create_function(spatial::create_hash)?)?;
    module.set("KDTree", lua.create_function(kdtree::create)?)?;
    bulk::register(lua, &module)?;
    Ok(module)
}
