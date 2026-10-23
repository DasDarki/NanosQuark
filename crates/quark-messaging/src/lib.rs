mod mqtt;
mod nats;

pub use self::mqtt::build as mqtt;
pub use self::nats::build as nats;

fn random_suffix() -> String {
    let value = getrandom::u64().unwrap_or_else(|_| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_nanos() as u64)
            .unwrap_or_default()
    });
    format!("{value:016x}")
}
