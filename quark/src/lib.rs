use mlua::prelude::*;
use quark_core::Module;

const MODULES: &[Module] = &[
    #[cfg(feature = "crypto")]
    Module {
        name: "Crypto",
        build: quark_crypto::build,
    },
];

#[mlua::lua_module]
fn quark(lua: &Lua) -> LuaResult<LuaTable> {
    quark_core::pin_library();

    let root = quark_core::create_root(lua, env!("CARGO_PKG_VERSION"), MODULES)?;
    lua.globals().set("Quark", &root)?;
    Ok(root)
}
