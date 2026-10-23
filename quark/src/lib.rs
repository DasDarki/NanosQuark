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
    #[cfg(feature = "compression")]
    Module {
        name: "Compression",
        build: quark_compression::compression,
    },
    #[cfg(feature = "compression")]
    Module {
        name: "Zip",
        build: quark_compression::zip,
    },
    #[cfg(feature = "compression")]
    Module {
        name: "Tar",
        build: quark_compression::tar,
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
    #[cfg(feature = "formats")]
    Module {
        name: "XML",
        build: quark_formats::xml,
    },
    #[cfg(feature = "formats")]
    Module {
        name: "INI",
        build: quark_formats::ini,
    },
    #[cfg(feature = "net")]
    Module {
        name: "Net",
        build: quark_net::net,
    },
    #[cfg(feature = "net")]
    Module {
        name: "TCP",
        build: quark_net::tcp,
    },
    #[cfg(feature = "net")]
    Module {
        name: "UDP",
        build: quark_net::udp,
    },
    #[cfg(feature = "net")]
    Module {
        name: "WebSocket",
        build: quark_net::websocket,
    },
    #[cfg(feature = "net")]
    Module {
        name: "HTTP",
        build: quark_net::http,
    },
    #[cfg(feature = "random")]
    Module {
        name: "Random",
        build: quark_random::build,
    },
    #[cfg(feature = "text")]
    Module {
        name: "Regex",
        build: quark_text::regex,
    },
    #[cfg(feature = "text")]
    Module {
        name: "Text",
        build: quark_text::text,
    },
    #[cfg(feature = "text")]
    Module {
        name: "Template",
        build: quark_text::template,
    },
    #[cfg(feature = "time")]
    Module {
        name: "Time",
        build: quark_time::build,
    },
    #[cfg(feature = "locale")]
    Module {
        name: "Locale",
        build: quark_locale::build,
    },
    #[cfg(feature = "utilities")]
    Module {
        name: "RateLimit",
        build: quark_utilities::rate_limit,
    },
    #[cfg(feature = "utilities")]
    Module {
        name: "Cache",
        build: quark_utilities::cache,
    },
    #[cfg(feature = "utilities")]
    Module {
        name: "PriorityQueue",
        build: quark_utilities::priority_queue,
    },
    #[cfg(feature = "utilities")]
    Module {
        name: "RingBuffer",
        build: quark_utilities::ring_buffer,
    },
    #[cfg(feature = "utilities")]
    Module {
        name: "Bitset",
        build: quark_utilities::bitset,
    },
    #[cfg(feature = "utilities")]
    Module {
        name: "Metrics",
        build: quark_utilities::metrics,
    },
    #[cfg(feature = "utilities")]
    Module {
        name: "Log",
        build: quark_utilities::log,
    },
    #[cfg(feature = "messaging")]
    Module {
        name: "MQTT",
        build: quark_messaging::mqtt,
    },
    #[cfg(feature = "messaging")]
    Module {
        name: "NATS",
        build: quark_messaging::nats,
    },
    #[cfg(feature = "redis")]
    Module {
        name: "Redis",
        build: quark_redis::build,
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
