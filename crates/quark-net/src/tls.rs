use std::sync::{Arc, OnceLock};

use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::{ClientConfig, RootCertStore};

static CONNECTOR: OnceLock<Result<TlsConnector, String>> = OnceLock::new();

pub fn connector() -> Result<TlsConnector, String> {
    CONNECTOR
        .get_or_init(|| {
            let mut roots = RootCertStore::empty();
            roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let provider = Arc::new(tokio_rustls::rustls::crypto::ring::default_provider());
            let config = ClientConfig::builder_with_provider(provider)
                .with_safe_default_protocol_versions()
                .map_err(|error| format!("cannot set up TLS: {error}"))?
                .with_root_certificates(roots)
                .with_no_client_auth();
            Ok(TlsConnector::from(Arc::new(config)))
        })
        .clone()
}
