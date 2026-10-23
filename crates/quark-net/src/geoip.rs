use std::net::IpAddr;
use std::sync::Arc;

use maxminddb::Reader;
use mlua::prelude::*;
use quark_core::{Object, Options, options_and_callback};
use serde_json::Value;

pub struct GeoIP {
    reader: Arc<Reader<Vec<u8>>>,
    language: String,
}

fn name(value: &Value, language: &str) -> Option<String> {
    let names = value.get("names")?;
    names
        .get(language)
        .or_else(|| names.get("en"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn place(lua: &Lua, value: Option<&Value>, language: &str) -> LuaResult<Option<LuaTable>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let table = lua.create_table()?;
    if let Some(code) = value
        .get("iso_code")
        .or_else(|| value.get("code"))
        .and_then(Value::as_str)
    {
        table.set("Code", code)?;
    }
    table.set("Name", name(value, language))?;
    if let Some(id) = value.get("geoname_id").and_then(Value::as_u64) {
        table.set("GeoNameID", id)?;
    }
    if let Some(member) = value.get("is_in_european_union").and_then(Value::as_bool) {
        table.set("IsInEuropeanUnion", member)?;
    }
    Ok(Some(table))
}

fn record(lua: &Lua, data: &Value, network: String, language: &str) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    table.set("Network", network)?;
    table.set("Country", place(lua, data.get("country"), language)?)?;
    table.set(
        "RegisteredCountry",
        place(lua, data.get("registered_country"), language)?,
    )?;
    table.set("Continent", place(lua, data.get("continent"), language)?)?;
    if let Some(city) = data.get("city") {
        table.set("City", name(city, language))?;
    }
    if let Some(subdivisions) = data.get("subdivisions").and_then(Value::as_array) {
        let list = lua.create_table()?;
        for subdivision in subdivisions {
            list.raw_push(place(lua, Some(subdivision), language)?)?;
        }
        table.set("Subdivisions", list)?;
    }
    if let Some(location) = data.get("location") {
        let entry = lua.create_table()?;
        entry.set("Latitude", location.get("latitude").and_then(Value::as_f64))?;
        entry.set(
            "Longitude",
            location.get("longitude").and_then(Value::as_f64),
        )?;
        entry.set(
            "AccuracyRadius",
            location.get("accuracy_radius").and_then(Value::as_u64),
        )?;
        entry.set(
            "TimeZone",
            location.get("time_zone").and_then(Value::as_str),
        )?;
        table.set("Location", entry)?;
    }
    if let Some(postal) = data
        .get("postal")
        .and_then(|postal| postal.get("code"))
        .and_then(Value::as_str)
    {
        table.set("PostalCode", postal)?;
    }
    if let Some(number) = data.get("autonomous_system_number").and_then(Value::as_u64) {
        table.set("ASN", number)?;
    }
    if let Some(organization) = data
        .get("autonomous_system_organization")
        .and_then(Value::as_str)
    {
        table.set("Organization", organization)?;
    }
    if let Some(traits) = data.get("traits") {
        for (key, field) in [
            ("is_anonymous_proxy", "IsAnonymousProxy"),
            ("is_satellite_provider", "IsSatelliteProvider"),
        ] {
            if let Some(flag) = traits.get(key).and_then(Value::as_bool) {
                table.set(field, flag)?;
            }
        }
    }
    Ok(table)
}

fn parse_address(text: &str) -> Result<IpAddr, String> {
    text.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .parse()
        .map_err(|_| format!("'{text}' is not an IP address"))
}

impl GeoIP {
    fn find(&self, address: &str) -> Result<Option<(Value, String)>, String> {
        let address = parse_address(address)?;
        let result = self
            .reader
            .lookup(address)
            .map_err(|error| format!("the lookup failed: {error}"))?;
        let Some(data) = result
            .decode::<Value>()
            .map_err(|error| format!("the database entry is invalid: {error}"))?
        else {
            return Ok(None);
        };
        let network = result
            .network()
            .map(|network| network.to_string())
            .unwrap_or_default();
        Ok(Some((data, network)))
    }
}

impl LuaUserData for GeoIP {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Lookup", |lua, geoip, address: String| {
            match geoip.find(&address) {
                Ok(Some((data, network))) => {
                    record(lua, &data, network, &geoip.language)?.into_lua_multi(lua)
                }
                Ok(None) => LuaNil.into_lua_multi(lua),
                Err(message) => (LuaNil, message).into_lua_multi(lua),
            }
        });
        methods.add_method("LookupRaw", |lua, geoip, address: String| {
            match geoip.find(&address) {
                Ok(Some((data, _))) => lua.to_value(&data)?.into_lua_multi(lua),
                Ok(None) => LuaNil.into_lua_multi(lua),
                Err(message) => (LuaNil, message).into_lua_multi(lua),
            }
        });
        methods.add_method("GetCountry", |_, geoip, address: String| {
            Ok(geoip.find(&address).ok().flatten().and_then(|(data, _)| {
                data.get("country")
                    .or_else(|| data.get("registered_country"))
                    .and_then(|country| country.get("iso_code"))
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            }))
        });
        methods.add_method("GetMetadata", |lua, geoip, ()| {
            let metadata = geoip.reader.metadata();
            let table = lua.create_table()?;
            table.set("DatabaseType", metadata.database_type.as_str())?;
            table.set("BuildEpoch", metadata.build_epoch)?;
            table.set("IpVersion", metadata.ip_version)?;
            table.set("NodeCount", metadata.node_count)?;
            table.set(
                "Languages",
                lua.create_sequence_from(metadata.languages.iter().map(String::as_str))?,
            )?;
            Ok(table)
        });
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "GeoIP",
        lua.create_function(
            |lua, (path, options, callback): (String, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                let language: String = Options::get_or(&options, "Language", "en".to_owned())?;
                quark_core::run(lua, callback, move || {
                    Reader::open_readfile(&path)
                        .map(|reader| {
                            Object(GeoIP {
                                reader: Arc::new(reader),
                                language,
                            })
                        })
                        .map_err(|error| {
                            format!("cannot open the GeoIP database '{path}': {error}")
                        })
                })
            },
        )?,
    )
}
