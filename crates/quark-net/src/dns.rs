use std::net::{IpAddr, ToSocketAddrs};
use std::str::FromStr;

use hickory_resolver::TokioResolver;
use hickory_resolver::proto::rr::{RData, RecordType};
use ipnet::IpNet;
use mlua::prelude::*;
use tokio::sync::OnceCell;

static RESOLVER: OnceCell<Result<TokioResolver, String>> = OnceCell::const_new();

pub enum Record {
    Text(String),
    Mail {
        priority: u16,
        exchange: String,
    },
    Service {
        priority: u16,
        weight: u16,
        port: u16,
        target: String,
    },
}

pub struct Records(Vec<Record>);

impl IntoLua for Records {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let list = lua.create_table()?;
        for record in self.0 {
            match record {
                Record::Text(text) => list.raw_push(text)?,
                Record::Mail { priority, exchange } => {
                    let table = lua.create_table()?;
                    table.set("Priority", priority)?;
                    table.set("Exchange", exchange)?;
                    list.raw_push(table)?;
                }
                Record::Service {
                    priority,
                    weight,
                    port,
                    target,
                } => {
                    let table = lua.create_table()?;
                    table.set("Priority", priority)?;
                    table.set("Weight", weight)?;
                    table.set("Port", port)?;
                    table.set("Target", target)?;
                    list.raw_push(table)?;
                }
            }
        }
        Ok(LuaValue::Table(list))
    }
}

async fn resolver() -> Result<&'static TokioResolver, String> {
    RESOLVER
        .get_or_init(|| async {
            TokioResolver::builder_tokio()
                .map_err(|error| format!("cannot read the system DNS configuration: {error}"))
                .and_then(|builder| {
                    builder
                        .build()
                        .map_err(|error| format!("cannot create the DNS resolver: {error}"))
                })
        })
        .await
        .as_ref()
        .map_err(Clone::clone)
}

fn trim_name(name: String) -> String {
    name.strip_suffix('.').map(str::to_owned).unwrap_or(name)
}

async fn lookup(name: String, kind: RecordType) -> Result<Records, String> {
    let resolver = resolver().await?;
    let lookup = resolver
        .lookup(name.as_str(), kind)
        .await
        .map_err(|error| format!("cannot look up {kind} records for '{name}': {error}"))?;

    let mut records = Vec::new();
    for record in lookup.answers() {
        let record = match &record.data {
            RData::A(address) => Record::Text(address.0.to_string()),
            RData::AAAA(address) => Record::Text(address.0.to_string()),
            RData::MX(mail) => Record::Mail {
                priority: mail.preference,
                exchange: trim_name(mail.exchange.to_string()),
            },
            RData::SRV(service) => Record::Service {
                priority: service.priority,
                weight: service.weight,
                port: service.port,
                target: trim_name(service.target.to_string()),
            },
            RData::TXT(text) => Record::Text(
                text.txt_data
                    .iter()
                    .map(|part| String::from_utf8_lossy(part).into_owned())
                    .collect::<String>(),
            ),
            RData::CNAME(target) => Record::Text(trim_name(target.0.to_string())),
            RData::NS(target) => Record::Text(trim_name(target.0.to_string())),
            RData::PTR(target) => Record::Text(trim_name(target.0.to_string())),
            _ => continue,
        };
        records.push(record);
    }
    Ok(Records(records))
}

fn record_type(name: &str) -> LuaResult<RecordType> {
    match name.to_ascii_uppercase().as_str() {
        "A" => Ok(RecordType::A),
        "AAAA" => Ok(RecordType::AAAA),
        "MX" => Ok(RecordType::MX),
        "TXT" => Ok(RecordType::TXT),
        "SRV" => Ok(RecordType::SRV),
        "CNAME" => Ok(RecordType::CNAME),
        "NS" => Ok(RecordType::NS),
        "PTR" => Ok(RecordType::PTR),
        _ => Err(LuaError::runtime(format!(
            "unsupported record type '{name}', expected A, AAAA, MX, TXT, SRV, CNAME, NS or PTR"
        ))),
    }
}

fn parse_ip(text: &str) -> Option<IpAddr> {
    IpAddr::from_str(text.trim().trim_start_matches('[').trim_end_matches(']')).ok()
}

fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_private()
                || ip.is_link_local()
                || ip.octets()[0] == 100 && (ip.octets()[1] & 0xc0) == 64
        }
        IpAddr::V6(ip) => {
            (ip.segments()[0] & 0xfe00) == 0xfc00 || (ip.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "Resolve",
        lua.create_function(|lua, (host, callback): (String, Option<LuaFunction>)| {
            quark_core::run(lua, callback, move || {
                let mut addresses = Vec::new();
                for address in (host.as_str(), 0)
                    .to_socket_addrs()
                    .map_err(|error| format!("cannot resolve '{host}': {error}"))?
                {
                    let ip = address.ip().to_string();
                    if !addresses.contains(&ip) {
                        addresses.push(ip);
                    }
                }
                Ok::<_, String>(Records(addresses.into_iter().map(Record::Text).collect()))
            })
        })?,
    )?;

    module.set(
        "Lookup",
        lua.create_function(
            |lua, (name, kind, callback): (String, LuaValue, Option<LuaFunction>)| {
                let (kind, callback) = match (kind, callback) {
                    (LuaValue::Function(callback), None) => ("A".to_owned(), Some(callback)),
                    (LuaNil, callback) => ("A".to_owned(), callback),
                    (LuaValue::String(kind), callback) => (kind.to_str()?.to_owned(), callback),
                    (other, _) => {
                        return Err(LuaError::runtime(format!(
                            "expected a record type or a callback, got {}",
                            other.type_name()
                        )));
                    }
                };
                quark_core::run_async(lua, callback, lookup(name, record_type(&kind)?))
            },
        )?,
    )?;

    module.set(
        "ReverseLookup",
        lua.create_function(|lua, (ip, callback): (String, Option<LuaFunction>)| {
            let ip = parse_ip(&ip)
                .ok_or_else(|| LuaError::runtime(format!("'{ip}' is not an IP address")))?;
            quark_core::run_async(lua, callback, async move {
                let resolver = resolver().await?;
                let names = resolver
                    .reverse_lookup(ip)
                    .await
                    .map_err(|error| format!("cannot look up the name of {ip}: {error}"))?;
                Ok::<_, String>(Records(
                    names
                        .answers()
                        .iter()
                        .filter_map(|record| match &record.data {
                            RData::PTR(target) => {
                                Some(Record::Text(trim_name(target.0.to_string())))
                            }
                            _ => None,
                        })
                        .collect(),
                ))
            })
        })?,
    )?;

    module.set(
        "CreateSelfSignedCertificate",
        lua.create_function(|_, names: LuaValue| {
            crate::tls::self_signed(read_names(names)?).map_err(LuaError::runtime)
        })?,
    )?;
    module.set(
        "CreateCertificateAuthority",
        lua.create_function(
            |_, (name, options): (Option<String>, quark_core::Options)| {
                crate::tls::create_authority(
                    name.as_deref().unwrap_or("Quark Certificate Authority"),
                    options.get_or("Days", 3650)?,
                )
                .map_err(LuaError::runtime)
            },
        )?,
    )?;
    module.set(
        "CreateCertificate",
        lua.create_function(|_, (names, options): (LuaValue, quark_core::Options)| {
            let authority = match options.get::<LuaTable>("CA")? {
                Some(table) => Some(crate::tls::Authority {
                    certificate: table.get::<Option<String>>("Certificate")?.ok_or_else(|| {
                        LuaError::runtime("the CA option needs Certificate and PrivateKey")
                    })?,
                    private_key: table.get::<Option<String>>("PrivateKey")?.ok_or_else(|| {
                        LuaError::runtime("the CA option needs Certificate and PrivateKey")
                    })?,
                }),
                None => None,
            };
            crate::tls::create_certificate(
                read_names(names)?,
                authority.as_ref(),
                options.get_or("Client", false)?,
                options.get_or("Days", 365)?,
            )
            .map_err(LuaError::runtime)
        })?,
    )?;

    module.set(
        "IsIP",
        lua.create_function(|_, value: LuaValue| {
            Ok(matches!(value, LuaValue::String(text) if text.to_str().is_ok_and(|text| parse_ip(&text).is_some())))
        })?,
    )?;
    module.set(
        "IsIPv4",
        lua.create_function(|_, text: String| Ok(matches!(parse_ip(&text), Some(IpAddr::V4(_)))))?,
    )?;
    module.set(
        "IsIPv6",
        lua.create_function(|_, text: String| Ok(matches!(parse_ip(&text), Some(IpAddr::V6(_)))))?,
    )?;
    module.set(
        "NormalizeIP",
        lua.create_function(|_, text: String| {
            Ok(parse_ip(&text)
                .map(|ip| ip.to_canonical().to_string())
                .ok_or_else(|| format!("'{text}' is not an IP address")))
        })?,
    )?;
    module.set(
        "IsInRange",
        lua.create_function(|_, (ip, range): (String, String)| {
            let ip = parse_ip(&ip)
                .ok_or_else(|| LuaError::runtime(format!("'{ip}' is not an IP address")))?;
            let network = IpNet::from_str(range.trim())
                .or_else(|_| parse_ip(&range).map(IpNet::from).ok_or(()))
                .map_err(|_| LuaError::runtime(format!("'{range}' is not a CIDR range")))?;
            Ok(network.contains(&ip.to_canonical()))
        })?,
    )?;
    module.set(
        "IsPrivate",
        lua.create_function(|_, text: String| {
            let ip = parse_ip(&text)
                .ok_or_else(|| LuaError::runtime(format!("'{text}' is not an IP address")))?;
            Ok(is_private(ip.to_canonical()))
        })?,
    )?;
    module.set(
        "IsLoopback",
        lua.create_function(|_, text: String| {
            let ip = parse_ip(&text)
                .ok_or_else(|| LuaError::runtime(format!("'{text}' is not an IP address")))?;
            Ok(ip.to_canonical().is_loopback())
        })?,
    )?;

    Ok(module)
}

fn read_names(names: LuaValue) -> LuaResult<Vec<String>> {
    match names {
        LuaValue::String(name) => Ok(vec![name.to_str()?.to_owned()]),
        LuaValue::Table(list) => list.sequence_values::<String>().collect(),
        other => Err(LuaError::runtime(format!(
            "expected a host name or a list of host names, got {}",
            other.type_name()
        ))),
    }
}
