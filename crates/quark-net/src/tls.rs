use std::sync::{Arc, OnceLock};

use mlua::prelude::*;
use quark_core::Options;
use tokio_rustls::rustls::crypto::CryptoProvider;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio_rustls::rustls::{ClientConfig, RootCertStore, ServerConfig};
use tokio_rustls::{TlsAcceptor, TlsConnector};

static DEFAULT_CONNECTOR: OnceLock<Result<TlsConnector, String>> = OnceLock::new();

#[derive(Clone)]
pub struct ClientTls {
    pub server_name: ServerName<'static>,
    pub connector: TlsConnector,
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(tokio_rustls::rustls::crypto::ring::default_provider())
}

fn connector(extra_roots: Option<&str>) -> Result<TlsConnector, String> {
    let build = |extra: Option<&str>| -> Result<TlsConnector, String> {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        if let Some(extra) = extra {
            for certificate in parse_certificates(extra)? {
                roots
                    .add(certificate)
                    .map_err(|error| format!("invalid CA certificate: {error}"))?;
            }
        }
        let config = ClientConfig::builder_with_provider(provider())
            .with_safe_default_protocol_versions()
            .map_err(|error| format!("cannot set up TLS: {error}"))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        Ok(TlsConnector::from(Arc::new(config)))
    };

    match extra_roots {
        None => DEFAULT_CONNECTOR.get_or_init(|| build(None)).clone(),
        Some(extra) => build(Some(extra)),
    }
}

pub fn parse_certificates(pem: &str) -> Result<Vec<CertificateDer<'static>>, String> {
    let certificates = CertificateDer::pem_slice_iter(pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("invalid certificate PEM: {error}"))?;
    if certificates.is_empty() {
        return Err("the PEM data contains no certificate".to_owned());
    }
    Ok(certificates)
}

pub fn client(options: &Options, host: &str, required: bool) -> LuaResult<Option<ClientTls>> {
    let enabled = match options.get::<LuaValue>("TLS")? {
        None | Some(LuaValue::Boolean(false)) => required,
        Some(LuaValue::Boolean(true)) => true,
        Some(other) => {
            return Err(LuaError::runtime(format!(
                "the TLS option has to be a boolean, got {}",
                other.type_name()
            )));
        }
    };
    if !enabled {
        return Ok(None);
    }

    let name = options.get_or("ServerName", host.to_owned())?;
    let ca = options.get::<String>("CA")?;
    client_for(&name, ca.as_deref())
        .map(Some)
        .map_err(LuaError::runtime)
}

pub fn client_for(name: &str, ca: Option<&str>) -> Result<ClientTls, String> {
    let server_name = ServerName::try_from(name.to_owned())
        .map_err(|_| format!("'{name}' is not a valid TLS server name"))?;
    Ok(ClientTls {
        server_name,
        connector: connector(ca)?,
    })
}

pub fn server(options: &Options) -> LuaResult<Option<TlsAcceptor>> {
    let Some(table) = options.get::<LuaValue>("TLS")? else {
        return Ok(None);
    };
    let LuaValue::Table(table) = table else {
        return Err(LuaError::runtime(
            "the TLS option of servers has to be a table with Certificate and PrivateKey",
        ));
    };

    let certificate: String = table
        .get::<Option<String>>("Certificate")?
        .ok_or_else(|| LuaError::runtime("the TLS option needs a Certificate (PEM)"))?;
    let private_key: String = table
        .get::<Option<String>>("PrivateKey")?
        .ok_or_else(|| LuaError::runtime("the TLS option needs a PrivateKey (PEM)"))?;

    let certificates = parse_certificates(&certificate).map_err(LuaError::runtime)?;
    let key = PrivateKeyDer::from_pem_slice(private_key.as_bytes())
        .map_err(|error| LuaError::runtime(format!("invalid private key PEM: {error}")))?;
    let config = ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|error| LuaError::runtime(format!("cannot set up TLS: {error}")))?
        .with_no_client_auth()
        .with_single_cert(certificates, key)
        .map_err(|error| {
            LuaError::runtime(format!(
                "the certificate does not match the private key: {error}"
            ))
        })?;
    Ok(Some(TlsAcceptor::from(Arc::new(config))))
}

pub fn self_signed(names: Vec<String>) -> Result<(String, String), String> {
    if names.is_empty() {
        return Err("at least one host name is needed".to_owned());
    }
    let certified = rcgen::generate_simple_self_signed(names)
        .map_err(|error| format!("cannot create the certificate: {error}"))?;
    Ok((certified.cert.pem(), certified.signing_key.serialize_pem()))
}
