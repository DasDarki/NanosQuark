use mlua::prelude::*;
use quark_core::Module;

const MODULES: &[Module] = &[
    Module {
        name: "Buffer",
        build: quark_core::buffer,
    },
    Module {
        name: "Promise",
        build: quark_core::promise,
    },
    #[cfg(feature = "crypto")]
    Module {
        name: "Crypto",
        build: quark_crypto::build,
    },
    #[cfg(feature = "formats")]
    Module {
        name: "JSON",
        build: quark_formats::json,
    },
    #[cfg(feature = "formats")]
    Module {
        name: "TOML",
        build: quark_formats::toml,
    },
    #[cfg(feature = "formats")]
    Module {
        name: "YAML",
        build: quark_formats::yaml,
    },
    #[cfg(feature = "formats")]
    Module {
        name: "MessagePack",
        build: quark_formats::message_pack,
    },
    #[cfg(feature = "formats")]
    Module {
        name: "CBOR",
        build: quark_formats::cbor,
    },
    #[cfg(feature = "formats")]
    Module {
        name: "CSV",
        build: quark_formats::csv,
    },
    #[cfg(feature = "formats")]
    Module {
        name: "UUID",
        build: quark_formats::uuid,
    },
    #[cfg(feature = "formats")]
    Module {
        name: "URL",
        build: quark_formats::url,
    },
    #[cfg(feature = "testing")]
    Module {
        name: "Testing",
        build: quark_testing::build,
    },
];

#[mlua::lua_module]
fn quark(lua: &Lua) -> LuaResult<LuaTable> {
    quark_core::pin_library();

    let root = quark_core::create_root(lua, env!("CARGO_PKG_VERSION"), MODULES)?;
    lua.globals().set("Quark", &root)?;
    Ok(root)
}
