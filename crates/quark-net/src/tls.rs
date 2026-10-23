use std::sync::{Arc, OnceLock};

use mlua::prelude::*;
use quark_core::Options;
use sha2::{Digest, Sha256};
use tokio_rustls::rustls::crypto::CryptoProvider;
use tokio_rustls::rustls::pki_types::pem::PemObject;
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use tokio_rustls::rustls::server::WebPkiClientVerifier;
use tokio_rustls::rustls::{ClientConfig, RootCertStore, ServerConfig, ServerConnection};
use tokio_rustls::{TlsAcceptor, TlsConnector};

pub const ALPN_HTTP1: &[u8] = b"http/1.1";
pub const ALPN_HTTP2: &[u8] = b"h2";

static DEFAULT_CONNECTOR: OnceLock<Result<TlsConnector, String>> = OnceLock::new();
static DEFAULT_HTTP_CONNECTOR: OnceLock<Result<TlsConnector, String>> = OnceLock::new();

#[derive(Clone)]
pub struct ClientTls {
    pub server_name: ServerName<'static>,
    pub connector: TlsConnector,
}

pub struct Identity {
    certificates: Vec<CertificateDer<'static>>,
    key: PrivateKeyDer<'static>,
}

impl Clone for Identity {
    fn clone(&self) -> Self {
        Identity {
            certificates: self.certificates.clone(),
            key: self.key.clone_key(),
        }
    }
}

#[derive(Clone)]
pub struct PeerCertificate {
    pem: String,
    fingerprint: String,
    subject: Option<String>,
    issuer: Option<String>,
    not_after: Option<i64>,
}

impl IntoLua for PeerCertificate {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        table.set("PEM", self.pem)?;
        table.set("Fingerprint", self.fingerprint)?;
        table.set("Subject", self.subject)?;
        table.set("Issuer", self.issuer)?;
        table.set("NotAfter", self.not_after)?;
        Ok(LuaValue::Table(table))
    }
}

fn common_name(name: &x509_parser::x509::X509Name) -> Option<String> {
    name.iter_common_name()
        .next()
        .and_then(|attribute| attribute.as_str().ok())
        .map(str::to_owned)
}

fn provider() -> Arc<CryptoProvider> {
    Arc::new(tokio_rustls::rustls::crypto::ring::default_provider())
}

fn pem_encode(der: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = String::with_capacity(der.len() * 4 / 3 + 4);
    for chunk in der.chunks(3) {
        let bytes = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let value = u32::from(bytes[0]) << 16 | u32::from(bytes[1]) << 8 | u32::from(bytes[2]);
        for index in 0..4 {
            if index <= chunk.len() {
                encoded.push(ALPHABET[(value >> (18 - index * 6)) as usize & 63] as char);
            } else {
                encoded.push('=');
            }
        }
    }
    let mut pem = String::from("-----BEGIN CERTIFICATE-----\n");
    for line in encoded.as_bytes().chunks(64) {
        pem.push_str(std::str::from_utf8(line).unwrap_or_default());
        pem.push('\n');
    }
    pem.push_str("-----END CERTIFICATE-----\n");
    pem
}

pub fn peer_certificate(connection: &ServerConnection) -> Option<PeerCertificate> {
    let certificate = connection.peer_certificates()?.first()?;
    let fingerprint = Sha256::digest(certificate.as_ref())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let parsed = x509_parser::parse_x509_certificate(certificate.as_ref())
        .ok()
        .map(|(_, parsed)| parsed);
    Some(PeerCertificate {
        pem: pem_encode(certificate.as_ref()),
        fingerprint,
        subject: parsed
            .as_ref()
            .and_then(|parsed| common_name(parsed.subject())),
        issuer: parsed
            .as_ref()
            .and_then(|parsed| common_name(parsed.issuer())),
        not_after: parsed.map(|parsed| parsed.validity().not_after.timestamp()),
    })
}

fn roots(extra: Option<&str>, include_defaults: bool) -> Result<RootCertStore, String> {
    let mut roots = RootCertStore::empty();
    if include_defaults {
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    }
    if let Some(extra) = extra {
        for certificate in parse_certificates(extra)? {
            roots
                .add(certificate)
                .map_err(|error| format!("invalid CA certificate: {error}"))?;
        }
    }
    Ok(roots)
}

pub fn client_config(
    extra_roots: Option<&str>,
    identity: Option<&Identity>,
    alpn: &[&[u8]],
) -> Result<Arc<ClientConfig>, String> {
    let builder = ClientConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|error| format!("cannot set up TLS: {error}"))?
        .with_root_certificates(roots(extra_roots, true)?);
    let mut config = match identity {
        Some(identity) => builder
            .with_client_auth_cert(identity.certificates.clone(), identity.key.clone_key())
            .map_err(|error| format!("invalid client certificate: {error}"))?,
        None => builder.with_no_client_auth(),
    };
    config.alpn_protocols = alpn.iter().map(|protocol| protocol.to_vec()).collect();
    Ok(Arc::new(config))
}

fn connector(
    extra_roots: Option<&str>,
    identity: Option<&Identity>,
    alpn: &[&[u8]],
) -> Result<TlsConnector, String> {
    let build = || client_config(extra_roots, identity, alpn).map(TlsConnector::from);
    match (extra_roots, identity, alpn.is_empty()) {
        (None, None, true) => DEFAULT_CONNECTOR.get_or_init(build).clone(),
        (None, None, false) if alpn == [ALPN_HTTP2, ALPN_HTTP1] => {
            DEFAULT_HTTP_CONNECTOR.get_or_init(build).clone()
        }
        _ => build(),
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

fn parse_identity(certificate: &str, private_key: &str) -> Result<Identity, String> {
    Ok(Identity {
        certificates: parse_certificates(certificate)?,
        key: PrivateKeyDer::from_pem_slice(private_key.as_bytes())
            .map_err(|error| format!("invalid private key PEM: {error}"))?,
    })
}

pub fn identity(options: &Options) -> LuaResult<Option<Identity>> {
    match (
        options.get::<String>("Certificate")?,
        options.get::<String>("PrivateKey")?,
    ) {
        (None, None) => Ok(None),
        (Some(certificate), Some(private_key)) => parse_identity(&certificate, &private_key)
            .map(Some)
            .map_err(LuaError::runtime),
        _ => Err(LuaError::runtime(
            "a client certificate needs both Certificate and PrivateKey",
        )),
    }
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
    let identity = identity(options)?;
    client_for(&name, ca.as_deref(), identity.as_ref(), &[])
        .map(Some)
        .map_err(LuaError::runtime)
}

pub fn client_for(
    name: &str,
    ca: Option<&str>,
    identity: Option<&Identity>,
    alpn: &[&[u8]],
) -> Result<ClientTls, String> {
    let server_name = ServerName::try_from(name.to_owned())
        .map_err(|_| format!("'{name}' is not a valid TLS server name"))?;
    Ok(ClientTls {
        server_name,
        connector: connector(ca, identity, alpn)?,
    })
}

pub fn server(options: &Options, alpn: &[&[u8]]) -> LuaResult<Option<TlsAcceptor>> {
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
    let identity = parse_identity(&certificate, &private_key).map_err(LuaError::runtime)?;

    let builder = ServerConfig::builder_with_provider(provider())
        .with_safe_default_protocol_versions()
        .map_err(|error| LuaError::runtime(format!("cannot set up TLS: {error}")))?;
    let builder = match table.get::<Option<String>>("ClientCA")? {
        None => builder.with_no_client_auth(),
        Some(client_ca) => {
            let roots = Arc::new(roots(Some(&client_ca), false).map_err(LuaError::runtime)?);
            let verifier = WebPkiClientVerifier::builder_with_provider(roots, provider());
            let verifier = if table
                .get::<Option<bool>>("RequireClientCertificate")?
                .unwrap_or(true)
            {
                verifier
            } else {
                verifier.allow_unauthenticated()
            };
            builder.with_client_cert_verifier(verifier.build().map_err(|error| {
                LuaError::runtime(format!("invalid ClientCA certificates: {error}"))
            })?)
        }
    };
    let mut config = builder
        .with_single_cert(identity.certificates, identity.key)
        .map_err(|error| {
            LuaError::runtime(format!(
                "the certificate does not match the private key: {error}"
            ))
        })?;
    config.alpn_protocols = alpn.iter().map(|protocol| protocol.to_vec()).collect();
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

pub struct Authority {
    pub certificate: String,
    pub private_key: String,
}

fn validity(params: &mut rcgen::CertificateParams, days: u32) {
    let now = rcgen::date_time_ymd(1970, 1, 1)
        + std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default();
    params.not_before = now - std::time::Duration::from_secs(3600);
    params.not_after = now + std::time::Duration::from_secs(u64::from(days) * 86_400);
}

pub fn create_authority(name: &str, days: u32) -> Result<(String, String), String> {
    let failed = |error: rcgen::Error| format!("cannot create the certificate authority: {error}");
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).map_err(failed)?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, name);
    params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    params.key_usages = vec![
        rcgen::KeyUsagePurpose::KeyCertSign,
        rcgen::KeyUsagePurpose::CrlSign,
        rcgen::KeyUsagePurpose::DigitalSignature,
    ];
    validity(&mut params, days);
    let key = rcgen::KeyPair::generate().map_err(failed)?;
    let certificate = params.self_signed(&key).map_err(failed)?;
    Ok((certificate.pem(), key.serialize_pem()))
}

pub fn create_certificate(
    names: Vec<String>,
    authority: Option<&Authority>,
    client: bool,
    days: u32,
) -> Result<(String, String), String> {
    let failed = |error: rcgen::Error| format!("cannot create the certificate: {error}");
    let Some(first) = names.first().cloned() else {
        return Err("at least one name is needed".to_owned());
    };
    let mut params = rcgen::CertificateParams::new(names).map_err(failed)?;
    params
        .distinguished_name
        .push(rcgen::DnType::CommonName, first);
    params.extended_key_usages = vec![if client {
        rcgen::ExtendedKeyUsagePurpose::ClientAuth
    } else {
        rcgen::ExtendedKeyUsagePurpose::ServerAuth
    }];
    validity(&mut params, days);
    let key = rcgen::KeyPair::generate().map_err(failed)?;
    let certificate = match authority {
        None => params.self_signed(&key).map_err(failed)?,
        Some(authority) => {
            let authority_key = rcgen::KeyPair::from_pem(&authority.private_key)
                .map_err(|error| format!("invalid CA private key: {error}"))?;
            let issuer = rcgen::Issuer::from_ca_cert_pem(&authority.certificate, authority_key)
                .map_err(|error| format!("invalid CA certificate: {error}"))?;
            params.signed_by(&key, &issuer).map_err(failed)?
        }
    };
    Ok((certificate.pem(), key.serialize_pem()))
}
