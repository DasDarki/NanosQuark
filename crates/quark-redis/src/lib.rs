mod client;
mod connection;
mod reader;
mod reply;
mod subscriber;

use std::sync::Once;
use std::time::Duration;

use mlua::prelude::*;
use quark_core::Options;
use redis::IntoConnectionInfo;

const DEFAULT_URL: &str = "redis://127.0.0.1:6379";
pub const DEFAULT_TIMEOUT: u64 = 5_000;

pub struct Settings {
    pub client: redis::Client,
    pub timeout: Duration,
}

pub fn install_crypto_provider() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        let _ = tokio_rustls::rustls::crypto::ring::default_provider().install_default();
    });
}

pub fn settings(url: Option<String>, options: &Options) -> LuaResult<Settings> {
    install_crypto_provider();
    let url = url.unwrap_or_else(|| DEFAULT_URL.to_owned());
    let info = url
        .as_str()
        .into_connection_info()
        .map_err(|error| LuaError::runtime(format!("invalid Redis URL: {error}")))?;

    let mut redis_settings = info.redis_settings().clone();
    if let Some(username) = options.get::<String>("Username")? {
        redis_settings = redis_settings.set_username(username);
    }
    if let Some(password) = options.get::<String>("Password")? {
        redis_settings = redis_settings.set_password(password);
    }
    if let Some(database) = options.get::<i64>("Database")? {
        redis_settings = redis_settings.set_db(database);
    }
    let info = info.set_redis_settings(redis_settings);

    let client = match options.get::<String>("CA")? {
        Some(ca) => redis::Client::build_with_tls(
            info,
            redis::TlsCertificates {
                client_tls: None,
                root_cert: Some(ca.into_bytes()),
            },
        ),
        None => redis::Client::open(info),
    }
    .map_err(|error| LuaError::runtime(format!("invalid Redis settings: {error}")))?;

    Ok(Settings {
        client,
        timeout: Duration::from_millis(options.get_or("Timeout", DEFAULT_TIMEOUT)?),
    })
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Connect", lua.create_function(client::connect)?)?;
    module.set("Subscriber", lua.create_function(subscriber::create)?)?;
    module.set("StreamReader", lua.create_function(reader::create)?)?;
    Ok(module)
}
