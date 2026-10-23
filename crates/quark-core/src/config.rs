use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::Duration;

use ipnet::IpNet;
use mlua::prelude::*;

const CONFIG_FILE: &str = "quark.toml";
const DEFAULT_TICK_BUDGET: u64 = 4;

#[derive(Clone)]
struct PortRange(u16, u16);

#[derive(Clone)]
enum HostRule {
    Any,
    Name(String),
    Suffix(String),
    Network(IpNet),
}

#[derive(Clone)]
struct ConnectRule {
    host: HostRule,
    ports: Option<PortRange>,
    text: String,
}

#[derive(Clone, Default)]
struct NetworkPolicy {
    connect: Option<Vec<ConnectRule>>,
    listen: Option<Vec<(PortRange, String)>>,
}

struct Config {
    tick_budget: AtomicU64,
    worker_threads: OnceLock<usize>,
    blocking_threads: OnceLock<usize>,
    network: RwLock<NetworkPolicy>,
    locked: AtomicBool,
    source: OnceLock<String>,
}

fn config() -> &'static Config {
    static CONFIG: OnceLock<Config> = OnceLock::new();
    CONFIG.get_or_init(|| {
        let config = Config {
            tick_budget: AtomicU64::new(DEFAULT_TICK_BUDGET),
            worker_threads: OnceLock::new(),
            blocking_threads: OnceLock::new(),
            network: RwLock::new(NetworkPolicy::default()),
            locked: AtomicBool::new(false),
            source: OnceLock::new(),
        };
        if let Ok(text) = std::fs::read_to_string(CONFIG_FILE) {
            match load_file(&config, &text) {
                Ok(()) => {
                    config.locked.store(true, Ordering::Release);
                    let _ = config.source.set(CONFIG_FILE.to_owned());
                }
                Err(error) => eprintln!("[Quark] ignoring {CONFIG_FILE}: {error}"),
            }
        }
        config
    })
}

fn parse_ports(text: &str) -> Result<PortRange, String> {
    let parse = |part: &str| {
        part.trim()
            .parse::<u16>()
            .map_err(|_| format!("'{part}' is not a port"))
    };
    match text.split_once('-') {
        Some((start, end)) => {
            let (start, end) = (parse(start)?, parse(end)?);
            if start > end {
                return Err(format!("the port range '{text}' is reversed"));
            }
            Ok(PortRange(start, end))
        }
        None => {
            let port = parse(text)?;
            Ok(PortRange(port, port))
        }
    }
}

fn parse_connect_rule(text: &str) -> Result<ConnectRule, String> {
    let trimmed = text.trim();
    let (host, ports) = if let Some(rest) = trimmed.strip_prefix('[') {
        let (host, rest) = rest
            .split_once(']')
            .ok_or_else(|| format!("'{text}' has an unclosed ["))?;
        (host, rest.strip_prefix(':'))
    } else if trimmed.matches(':').count() > 1 {
        (trimmed, None)
    } else {
        match trimmed.rsplit_once(':') {
            Some((host, ports)) => (host, Some(ports)),
            None => (trimmed, None),
        }
    };
    let host = host.trim().to_ascii_lowercase();
    let rule = if host == "*" {
        HostRule::Any
    } else if let Some(suffix) = host.strip_prefix("*.") {
        HostRule::Suffix(format!(".{suffix}"))
    } else if let Ok(network) = host.parse::<IpNet>() {
        HostRule::Network(network)
    } else if let Ok(address) = host.parse::<IpAddr>() {
        HostRule::Network(IpNet::from(address))
    } else if !host.is_empty() {
        HostRule::Name(host)
    } else {
        return Err(format!("'{text}' has no host"));
    };
    Ok(ConnectRule {
        host: rule,
        ports: ports.map(parse_ports).transpose()?,
        text: trimmed.to_owned(),
    })
}

fn listen_rule(value: &toml::Value) -> Result<(PortRange, String), String> {
    match value {
        toml::Value::Integer(port) => {
            let port = u16::try_from(*port).map_err(|_| format!("{port} is not a port"))?;
            Ok((PortRange(port, port), port.to_string()))
        }
        toml::Value::String(text) => Ok((parse_ports(text)?, text.clone())),
        other => Err(format!("listen rules are ports or ranges, got {other}")),
    }
}

fn load_file(config: &Config, text: &str) -> Result<(), String> {
    let document: toml::Table = text.parse().map_err(|error| format!("{error}"))?;
    if let Some(runtime) = document.get("runtime").and_then(toml::Value::as_table) {
        if let Some(budget) = runtime.get("tick_budget").and_then(toml::Value::as_integer) {
            set_tick_budget(config, budget)?;
        }
        if let Some(threads) = runtime
            .get("worker_threads")
            .and_then(toml::Value::as_integer)
        {
            set_threads(&config.worker_threads, threads, "worker_threads")?;
        }
        if let Some(threads) = runtime
            .get("blocking_threads")
            .and_then(toml::Value::as_integer)
        {
            set_threads(&config.blocking_threads, threads, "blocking_threads")?;
        }
    }
    if let Some(network) = document.get("network").and_then(toml::Value::as_table) {
        let mut policy = NetworkPolicy::default();
        if let Some(rules) = network.get("allow_connect").and_then(toml::Value::as_array) {
            policy.connect = Some(
                rules
                    .iter()
                    .map(|rule| {
                        rule.as_str()
                            .ok_or_else(|| "allow_connect entries have to be strings".to_owned())
                            .and_then(parse_connect_rule)
                    })
                    .collect::<Result<_, _>>()?,
            );
        }
        if let Some(rules) = network.get("allow_listen").and_then(toml::Value::as_array) {
            policy.listen = Some(rules.iter().map(listen_rule).collect::<Result<_, _>>()?);
        }
        *config
            .network
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = policy;
    }
    Ok(())
}

fn set_tick_budget(config: &Config, budget: i64) -> Result<(), String> {
    if !(1..=100).contains(&budget) {
        return Err("the tick budget has to be between 1 and 100 milliseconds".to_owned());
    }
    config.tick_budget.store(budget as u64, Ordering::Release);
    Ok(())
}

fn set_threads(slot: &OnceLock<usize>, threads: i64, name: &str) -> Result<(), String> {
    if !(1..=512).contains(&threads) {
        return Err(format!("{name} has to be between 1 and 512"));
    }
    slot.set(threads as usize)
        .map_err(|_| format!("{name} can only be set before Quark starts its worker threads"))
}

pub fn tick_budget() -> Duration {
    Duration::from_millis(config().tick_budget.load(Ordering::Acquire))
}

pub fn worker_threads() -> usize {
    *config().worker_threads.get_or_init(|| {
        let cores = std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(2);
        (cores / 2).clamp(2, 8)
    })
}

pub fn blocking_threads() -> usize {
    *config().blocking_threads.get_or_init(|| 64)
}

fn host_matches(rule: &HostRule, host: &str, addresses: &[IpAddr]) -> bool {
    match rule {
        HostRule::Any => true,
        HostRule::Name(name) => host == name,
        HostRule::Suffix(suffix) => host.ends_with(suffix.as_str()),
        HostRule::Network(network) => {
            let literal = host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .parse::<IpAddr>()
                .ok();
            match literal {
                Some(address) => network.contains(&address),
                None => {
                    !addresses.is_empty()
                        && addresses.iter().all(|address| network.contains(address))
                }
            }
        }
    }
}

pub fn has_connect_rules() -> bool {
    config()
        .network
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .connect
        .is_some()
}

pub fn check_connect(host: &str, port: u16, addresses: &[IpAddr]) -> Result<(), String> {
    let policy = config()
        .network
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(rules) = &policy.connect else {
        return Ok(());
    };
    let host = host.to_ascii_lowercase();
    let allowed = rules.iter().any(|rule| {
        rule.ports
            .as_ref()
            .is_none_or(|PortRange(start, end)| (*start..=*end).contains(&port))
            && host_matches(&rule.host, &host, addresses)
    });
    if allowed {
        Ok(())
    } else {
        Err(format!(
            "connecting to {host}:{port} is not allowed by the Quark network configuration"
        ))
    }
}

pub fn check_connect_blocking(host: &str, port: u16) -> Result<(), String> {
    if !has_connect_rules() {
        return Ok(());
    }
    if check_connect(host, port, &[]).is_ok() {
        return Ok(());
    }
    use std::net::ToSocketAddrs;
    let addresses: Vec<IpAddr> = (host, port)
        .to_socket_addrs()
        .map(|addresses| addresses.map(|address| address.ip()).collect())
        .unwrap_or_default();
    check_connect(host, port, &addresses)
}

pub fn check_listen(port: u16) -> Result<(), String> {
    let policy = config()
        .network
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(rules) = &policy.listen else {
        return Ok(());
    };
    if port != 0
        && rules
            .iter()
            .any(|(PortRange(start, end), _)| (*start..=*end).contains(&port))
    {
        return Ok(());
    }
    Err(format!(
        "listening on port {port} is not allowed by the Quark network configuration"
    ))
}

fn read_strings(value: LuaValue, name: &str) -> LuaResult<Vec<String>> {
    match value {
        LuaValue::Table(table) => table
            .sequence_values::<LuaValue>()
            .map(|entry| match entry? {
                LuaValue::String(text) => Ok(text.to_str()?.to_owned()),
                LuaValue::Integer(number) => Ok(number.to_string()),
                other => Err(LuaError::runtime(format!(
                    "{name} entries have to be strings or numbers, got {}",
                    other.type_name()
                ))),
            })
            .collect(),
        other => Err(LuaError::runtime(format!(
            "{name} has to be a list, got {}",
            other.type_name()
        ))),
    }
}

pub fn configure(_: &Lua, options: LuaTable) -> LuaResult<()> {
    let config = config();
    if config.locked.swap(true, Ordering::AcqRel) {
        return Err(LuaError::runtime(match config.source.get() {
            Some(file) if file == CONFIG_FILE => {
                format!("Quark is configured by {file}, Quark.Configure is disabled")
            }
            _ => "Quark.Configure can only be called once".to_owned(),
        }));
    }
    let _ = config.source.set("Quark.Configure".to_owned());

    if let Some(budget) = options.get::<Option<i64>>("TickBudget")? {
        set_tick_budget(config, budget).map_err(LuaError::runtime)?;
    }
    if let Some(threads) = options.get::<Option<i64>>("WorkerThreads")? {
        set_threads(&config.worker_threads, threads, "WorkerThreads").map_err(LuaError::runtime)?;
    }
    if let Some(threads) = options.get::<Option<i64>>("BlockingThreads")? {
        set_threads(&config.blocking_threads, threads, "BlockingThreads")
            .map_err(LuaError::runtime)?;
    }
    if let Some(network) = options.get::<Option<LuaTable>>("Network")? {
        let mut policy = NetworkPolicy::default();
        if let Some(rules) = network.get::<Option<LuaValue>>("AllowConnect")? {
            policy.connect = Some(
                read_strings(rules, "AllowConnect")?
                    .iter()
                    .map(|rule| parse_connect_rule(rule))
                    .collect::<Result<_, _>>()
                    .map_err(LuaError::runtime)?,
            );
        }
        if let Some(rules) = network.get::<Option<LuaValue>>("AllowListen")? {
            policy.listen = Some(
                read_strings(rules, "AllowListen")?
                    .iter()
                    .map(|rule| parse_ports(rule).map(|range| (range, rule.clone())))
                    .collect::<Result<_, _>>()
                    .map_err(LuaError::runtime)?,
            );
        }
        *config
            .network
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = policy;
    }
    Ok(())
}

pub fn configuration(lua: &Lua, _: ()) -> LuaResult<LuaTable> {
    let config = config();
    let table = lua.create_table()?;
    table.set("TickBudget", config.tick_budget.load(Ordering::Acquire))?;
    table.set("WorkerThreads", worker_threads())?;
    table.set("BlockingThreads", blocking_threads())?;
    table.set("Locked", config.locked.load(Ordering::Acquire))?;
    table.set("Source", config.source.get().cloned())?;
    let policy = config
        .network
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone();
    let network = lua.create_table()?;
    if let Some(rules) = policy.connect {
        network.set(
            "AllowConnect",
            lua.create_sequence_from(rules.into_iter().map(|rule| rule.text))?,
        )?;
    }
    if let Some(rules) = policy.listen {
        network.set(
            "AllowListen",
            lua.create_sequence_from(rules.into_iter().map(|(_, text)| text))?,
        )?;
    }
    table.set("Network", network)?;
    Ok(table)
}
