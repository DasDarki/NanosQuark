use getrandom::SysRng;
use mlua::prelude::*;
use p256::pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding,
};
use quark_core::{Object, Options, options_and_callback};
use rand_core::UnwrapErr;
use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey};
use rsa::{Pkcs1v15Sign, Pss, RsaPrivateKey, RsaPublicKey};

use crate::encoding::Format;
use crate::hash::HashAlgorithm;
use crate::random::random_bytes;

const DEFAULT_RSA_BITS: usize = 2048;

#[derive(Clone, Copy, PartialEq)]
pub enum KeyAlgorithm {
    Ed25519,
    EcdsaP256,
    Rsa,
    X25519,
}

impl KeyAlgorithm {
    pub fn parse(name: &str) -> LuaResult<Self> {
        let normalized = name
            .chars()
            .filter(|character| *character != '-' && *character != '_')
            .collect::<String>()
            .to_ascii_uppercase();

        match normalized.as_str() {
            "ED25519" => Ok(KeyAlgorithm::Ed25519),
            "ECDSAP256" | "P256" | "ECDSA" => Ok(KeyAlgorithm::EcdsaP256),
            "RSA" => Ok(KeyAlgorithm::Rsa),
            "X25519" => Ok(KeyAlgorithm::X25519),
            _ => Err(LuaError::runtime(format!(
                "unsupported key algorithm '{name}', expected Ed25519, ECDSA-P256, RSA or X25519"
            ))),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            KeyAlgorithm::Ed25519 => "Ed25519",
            KeyAlgorithm::EcdsaP256 => "ECDSA-P256",
            KeyAlgorithm::Rsa => "RSA",
            KeyAlgorithm::X25519 => "X25519",
        }
    }
}

impl FromLua for KeyAlgorithm {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        KeyAlgorithm::parse(&String::from_lua(value, lua)?)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum Padding {
    Pkcs1,
    Pss,
}

pub struct SignOptions {
    pub format: Format,
    pub padding: Padding,
    pub hash: HashAlgorithm,
}

impl SignOptions {
    fn parse(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        let mut options = SignOptions {
            format: Format::Hex,
            padding: Padding::Pkcs1,
            hash: HashAlgorithm::Sha256,
        };

        match value {
            LuaNil => {}
            LuaValue::String(_) => options.format = Format::from_lua(value, lua)?,
            LuaValue::Table(_) => {
                let table = Options::from_lua(value, lua)?;
                options.format = table.get_or("Format", options.format)?;
                options.hash = table.get_or("Hash", options.hash)?;
                if let Some(padding) = table.get::<String>("Padding")? {
                    options.padding = match padding.to_ascii_uppercase().as_str() {
                        "PKCS1" | "PKCS1V15" => Padding::Pkcs1,
                        "PSS" => Padding::Pss,
                        other => {
                            return Err(LuaError::runtime(format!(
                                "unknown padding '{other}', expected PKCS1 or PSS"
                            )));
                        }
                    };
                }
            }
            other => {
                return Err(LuaError::runtime(format!(
                    "expected a format or an options table, got {}",
                    other.type_name()
                )));
            }
        }

        Ok(options)
    }
}

pub enum PrivateKey {
    Ed25519(ed25519_dalek::SigningKey),
    EcdsaP256(p256::ecdsa::SigningKey),
    Rsa(Box<RsaPrivateKey>),
    X25519(x25519_dalek::StaticSecret),
}

#[derive(Clone)]
pub enum PublicKey {
    Ed25519(ed25519_dalek::VerifyingKey),
    EcdsaP256(p256::ecdsa::VerifyingKey),
    Rsa(Box<RsaPublicKey>),
    X25519(x25519_dalek::PublicKey),
}

impl PrivateKey {
    fn generate(algorithm: KeyAlgorithm, rsa_bits: usize) -> Result<Self, String> {
        match algorithm {
            KeyAlgorithm::Ed25519 => Ok(PrivateKey::Ed25519(
                ed25519_dalek::SigningKey::from_bytes(&random_seed()?),
            )),
            KeyAlgorithm::X25519 => Ok(PrivateKey::X25519(x25519_dalek::StaticSecret::from(
                random_seed()?,
            ))),
            KeyAlgorithm::EcdsaP256 => loop {
                if let Ok(key) = p256::ecdsa::SigningKey::from_slice(&random_seed()?) {
                    return Ok(PrivateKey::EcdsaP256(key));
                }
            },
            KeyAlgorithm::Rsa => RsaPrivateKey::new(&mut UnwrapErr(SysRng), rsa_bits)
                .map(|key| PrivateKey::Rsa(Box::new(key)))
                .map_err(|error| error.to_string()),
        }
    }

    pub fn algorithm(&self) -> KeyAlgorithm {
        match self {
            PrivateKey::Ed25519(_) => KeyAlgorithm::Ed25519,
            PrivateKey::EcdsaP256(_) => KeyAlgorithm::EcdsaP256,
            PrivateKey::Rsa(_) => KeyAlgorithm::Rsa,
            PrivateKey::X25519(_) => KeyAlgorithm::X25519,
        }
    }

    pub fn public_key(&self) -> PublicKey {
        match self {
            PrivateKey::Ed25519(key) => PublicKey::Ed25519(key.verifying_key()),
            PrivateKey::EcdsaP256(key) => PublicKey::EcdsaP256(*key.verifying_key()),
            PrivateKey::Rsa(key) => PublicKey::Rsa(Box::new(key.to_public_key())),
            PrivateKey::X25519(key) => PublicKey::X25519(x25519_dalek::PublicKey::from(key)),
        }
    }

    pub fn sign(&self, data: &[u8], padding: Padding, hash: HashAlgorithm) -> LuaResult<Vec<u8>> {
        match self {
            PrivateKey::Ed25519(key) => {
                use ed25519_dalek::Signer;
                Ok(key.sign(data).to_bytes().to_vec())
            }
            PrivateKey::EcdsaP256(key) => {
                use p256::ecdsa::signature::Signer;
                require_sha256(hash)?;
                let signature: p256::ecdsa::Signature = key.sign(data);
                Ok(signature.to_bytes().to_vec())
            }
            PrivateKey::Rsa(key) => {
                let hashed = hash.digest(data);
                let result = match padding {
                    Padding::Pkcs1 => key.sign(rsa_pkcs1_scheme(hash)?, &hashed),
                    Padding::Pss => rsa_pss_sign(key, hash, &hashed)?,
                };
                result.map_err(LuaError::external)
            }
            PrivateKey::X25519(_) => Err(LuaError::runtime(
                "X25519 keys cannot sign, use DeriveSharedSecret for key exchange",
            )),
        }
    }

    fn export_pem(&self) -> LuaResult<String> {
        let pem = match self {
            PrivateKey::Ed25519(key) => key.to_pkcs8_pem(LineEnding::LF),
            PrivateKey::EcdsaP256(key) => key.to_pkcs8_pem(LineEnding::LF),
            PrivateKey::Rsa(key) => key.to_pkcs8_pem(LineEnding::LF),
            PrivateKey::X25519(_) => {
                return Err(LuaError::runtime(
                    "X25519 keys can only be exported with ExportRaw",
                ));
            }
        };
        pem.map(|pem| pem.to_string()).map_err(LuaError::external)
    }

    fn export_raw(&self) -> LuaResult<Vec<u8>> {
        match self {
            PrivateKey::Ed25519(key) => Ok(key.to_bytes().to_vec()),
            PrivateKey::EcdsaP256(key) => Ok(key.to_bytes().to_vec()),
            PrivateKey::X25519(key) => Ok(key.to_bytes().to_vec()),
            PrivateKey::Rsa(_) => Err(LuaError::runtime(
                "RSA keys can only be exported with ExportPEM",
            )),
        }
    }
}

impl PublicKey {
    pub fn algorithm(&self) -> KeyAlgorithm {
        match self {
            PublicKey::Ed25519(_) => KeyAlgorithm::Ed25519,
            PublicKey::EcdsaP256(_) => KeyAlgorithm::EcdsaP256,
            PublicKey::Rsa(_) => KeyAlgorithm::Rsa,
            PublicKey::X25519(_) => KeyAlgorithm::X25519,
        }
    }

    pub fn verify(
        &self,
        data: &[u8],
        signature: &[u8],
        padding: Padding,
        hash: HashAlgorithm,
    ) -> LuaResult<bool> {
        match self {
            PublicKey::Ed25519(key) => {
                use ed25519_dalek::Verifier;
                let Ok(signature) = ed25519_dalek::Signature::from_slice(signature) else {
                    return Ok(false);
                };
                Ok(key.verify(data, &signature).is_ok())
            }
            PublicKey::EcdsaP256(key) => {
                use p256::ecdsa::signature::Verifier;
                require_sha256(hash)?;
                let Ok(signature) = p256::ecdsa::Signature::from_slice(signature) else {
                    return Ok(false);
                };
                Ok(key.verify(data, &signature).is_ok())
            }
            PublicKey::Rsa(key) => {
                let hashed = hash.digest(data);
                let result = match padding {
                    Padding::Pkcs1 => key.verify(rsa_pkcs1_scheme(hash)?, &hashed, signature),
                    Padding::Pss => rsa_pss_verify(key, hash, &hashed, signature)?,
                };
                Ok(result.is_ok())
            }
            PublicKey::X25519(_) => Err(LuaError::runtime("X25519 keys cannot verify signatures")),
        }
    }

    fn export_pem(&self) -> LuaResult<String> {
        let pem = match self {
            PublicKey::Ed25519(key) => key.to_public_key_pem(LineEnding::LF),
            PublicKey::EcdsaP256(key) => key.to_public_key_pem(LineEnding::LF),
            PublicKey::Rsa(key) => key.to_public_key_pem(LineEnding::LF),
            PublicKey::X25519(_) => {
                return Err(LuaError::runtime(
                    "X25519 keys can only be exported with ExportRaw",
                ));
            }
        };
        pem.map_err(LuaError::external)
    }

    fn export_raw(&self) -> LuaResult<Vec<u8>> {
        match self {
            PublicKey::Ed25519(key) => Ok(key.to_bytes().to_vec()),
            PublicKey::EcdsaP256(key) => Ok(key.to_sec1_bytes().to_vec()),
            PublicKey::X25519(key) => Ok(key.as_bytes().to_vec()),
            PublicKey::Rsa(_) => Err(LuaError::runtime(
                "RSA keys can only be exported with ExportPEM",
            )),
        }
    }
}

impl LuaUserData for PrivateKey {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetAlgorithm", |_, key, ()| Ok(key.algorithm().name()));
        methods.add_method("GetPublicKey", |_, key, ()| Ok(Object(key.public_key())));
        methods.add_method(
            "Sign",
            |lua, key, (data, options): (LuaString, LuaValue)| {
                let options = SignOptions::parse(options, lua)?;
                let signature = key.sign(&data.as_bytes(), options.padding, options.hash)?;
                Ok(options.format.encode(&signature))
            },
        );
        methods.add_method("ExportPEM", |_, key, ()| key.export_pem());
        methods.add_method("ExportRaw", |_, key, format: Option<Format>| {
            Ok(format.unwrap_or(Format::Raw).encode(&key.export_raw()?))
        });
        methods.add_method(
            "DeriveSharedSecret",
            |_, key, (public, format): (LuaAnyUserData, Option<Format>)| {
                let public = public.borrow::<PublicKey>()?;
                match (key, &*public) {
                    (PrivateKey::X25519(secret), PublicKey::X25519(public)) => {
                        let shared = secret.diffie_hellman(public);
                        if !shared.was_contributory() {
                            return Err(LuaError::runtime(
                                "the public key is not valid for key exchange",
                            ));
                        }
                        Ok(format.unwrap_or(Format::Raw).encode(shared.as_bytes()))
                    }
                    _ => Err(LuaError::runtime(
                        "DeriveSharedSecret needs an X25519 private key and an X25519 public key",
                    )),
                }
            },
        );
        methods.add_meta_method(LuaMetaMethod::ToString, |_, key, ()| {
            Ok(format!("PrivateKey({})", key.algorithm().name()))
        });
    }
}

impl LuaUserData for PublicKey {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetAlgorithm", |_, key, ()| Ok(key.algorithm().name()));
        methods.add_method(
            "Verify",
            |lua, key, (data, signature, options): (LuaString, LuaString, LuaValue)| {
                let options = SignOptions::parse(options, lua)?;
                let signature = match options.format.decode(&signature.as_bytes()) {
                    Ok(signature) => signature,
                    Err(error) => return Ok(Err(error)),
                };
                Ok(Ok(key.verify(
                    &data.as_bytes(),
                    &signature,
                    options.padding,
                    options.hash,
                )?))
            },
        );
        methods.add_method("ExportPEM", |_, key, ()| key.export_pem());
        methods.add_method("ExportRaw", |_, key, format: Option<Format>| {
            Ok(format.unwrap_or(Format::Raw).encode(&key.export_raw()?))
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, key, ()| {
            Ok(format!("PublicKey({})", key.algorithm().name()))
        });
    }
}

pub fn register(lua: &Lua, crypto: &LuaTable) -> LuaResult<()> {
    crypto.set("GenerateKeyPair", lua.create_function(generate_key_pair)?)?;
    crypto.set(
        "ImportPrivateKey",
        lua.create_function(|_, (data, algorithm): (LuaString, Option<KeyAlgorithm>)| {
            Ok(import_private_key(&data.as_bytes(), algorithm).map(Object))
        })?,
    )?;
    crypto.set(
        "ImportPublicKey",
        lua.create_function(|_, (data, algorithm): (LuaString, Option<KeyAlgorithm>)| {
            Ok(import_public_key(&data.as_bytes(), algorithm).map(Object))
        })?,
    )?;
    Ok(())
}

fn generate_key_pair(
    lua: &Lua,
    (algorithm, options, callback): (KeyAlgorithm, LuaValue, Option<LuaFunction>),
) -> LuaResult<LuaMultiValue> {
    let (options, callback) = options_and_callback(options, callback)?;
    let bits = options.get_or("Bits", DEFAULT_RSA_BITS)?;
    if algorithm == KeyAlgorithm::Rsa && !(2048..=8192).contains(&bits) {
        return Err(LuaError::runtime(
            "RSA keys have to be between 2048 and 8192 bits",
        ));
    }

    quark_core::run(lua, callback, move || {
        let private = PrivateKey::generate(algorithm, bits)?;
        let public = private.public_key();
        Ok::<_, String>((Object(private), Object(public)))
    })
}

fn import_private_key(data: &[u8], algorithm: Option<KeyAlgorithm>) -> Result<PrivateKey, String> {
    if let Some(pem) = pem_text(data) {
        if pem.starts_with("-----BEGIN RSA PRIVATE KEY") {
            return RsaPrivateKey::from_pkcs1_pem(pem)
                .map(|key| PrivateKey::Rsa(Box::new(key)))
                .map_err(|error| format!("invalid RSA private key: {error}"));
        }
        if pem.starts_with("-----BEGIN EC PRIVATE KEY") {
            return p256::SecretKey::from_sec1_pem(pem)
                .map(|key| PrivateKey::EcdsaP256(p256::ecdsa::SigningKey::from(key)))
                .map_err(|error| format!("invalid EC private key: {error}"));
        }
        if pem.starts_with("-----BEGIN PRIVATE KEY") {
            if let Ok(key) = ed25519_dalek::SigningKey::from_pkcs8_pem(pem) {
                return Ok(PrivateKey::Ed25519(key));
            }
            if let Ok(key) = p256::ecdsa::SigningKey::from_pkcs8_pem(pem) {
                return Ok(PrivateKey::EcdsaP256(key));
            }
            if let Ok(key) = RsaPrivateKey::from_pkcs8_pem(pem) {
                return Ok(PrivateKey::Rsa(Box::new(key)));
            }
            return Err(
                "unsupported PKCS#8 private key, expected Ed25519, ECDSA P-256 or RSA".to_owned(),
            );
        }
        return Err("unsupported PEM block, expected a private key".to_owned());
    }

    let algorithm = algorithm.ok_or("raw private keys need the algorithm as second argument")?;
    match algorithm {
        KeyAlgorithm::Ed25519 => Ok(PrivateKey::Ed25519(ed25519_dalek::SigningKey::from_bytes(
            &seed_from(data)?,
        ))),
        KeyAlgorithm::X25519 => Ok(PrivateKey::X25519(x25519_dalek::StaticSecret::from(
            seed_from(data)?,
        ))),
        KeyAlgorithm::EcdsaP256 => p256::ecdsa::SigningKey::from_slice(data)
            .map(PrivateKey::EcdsaP256)
            .map_err(|_| "invalid ECDSA P-256 private key".to_owned()),
        KeyAlgorithm::Rsa => Err("RSA private keys can only be imported from PEM".to_owned()),
    }
}

fn import_public_key(data: &[u8], algorithm: Option<KeyAlgorithm>) -> Result<PublicKey, String> {
    if let Some(pem) = pem_text(data) {
        if pem.starts_with("-----BEGIN RSA PUBLIC KEY") {
            return RsaPublicKey::from_pkcs1_pem(pem)
                .map(|key| PublicKey::Rsa(Box::new(key)))
                .map_err(|error| format!("invalid RSA public key: {error}"));
        }
        if pem.starts_with("-----BEGIN PUBLIC KEY") {
            if let Ok(key) = ed25519_dalek::VerifyingKey::from_public_key_pem(pem) {
                return Ok(PublicKey::Ed25519(key));
            }
            if let Ok(key) = p256::ecdsa::VerifyingKey::from_public_key_pem(pem) {
                return Ok(PublicKey::EcdsaP256(key));
            }
            if let Ok(key) = RsaPublicKey::from_public_key_pem(pem) {
                return Ok(PublicKey::Rsa(Box::new(key)));
            }
            return Err("unsupported public key, expected Ed25519, ECDSA P-256 or RSA".to_owned());
        }
        return Err("unsupported PEM block, expected a public key".to_owned());
    }

    let algorithm = algorithm.ok_or("raw public keys need the algorithm as second argument")?;
    match algorithm {
        KeyAlgorithm::Ed25519 => ed25519_dalek::VerifyingKey::from_bytes(&seed_from(data)?)
            .map(PublicKey::Ed25519)
            .map_err(|_| "invalid Ed25519 public key".to_owned()),
        KeyAlgorithm::X25519 => Ok(PublicKey::X25519(x25519_dalek::PublicKey::from(seed_from(
            data,
        )?))),
        KeyAlgorithm::EcdsaP256 => p256::ecdsa::VerifyingKey::from_sec1_bytes(data)
            .map(PublicKey::EcdsaP256)
            .map_err(|_| "invalid ECDSA P-256 public key".to_owned()),
        KeyAlgorithm::Rsa => Err("RSA public keys can only be imported from PEM".to_owned()),
    }
}

fn pem_text(data: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(data).ok()?.trim();
    text.starts_with("-----BEGIN").then_some(text)
}

fn random_seed() -> Result<[u8; 32], String> {
    let bytes = random_bytes(32).map_err(|error| error.to_string())?;
    seed_from(&bytes)
}

fn seed_from(data: &[u8]) -> Result<[u8; 32], String> {
    data.try_into()
        .map_err(|_| format!("expected a 32 byte key, got {} bytes", data.len()))
}

fn require_sha256(hash: HashAlgorithm) -> LuaResult<()> {
    match hash {
        HashAlgorithm::Sha256 => Ok(()),
        _ => Err(LuaError::runtime(
            "ECDSA P-256 signatures always use SHA256",
        )),
    }
}

fn rsa_pkcs1_scheme(hash: HashAlgorithm) -> LuaResult<Pkcs1v15Sign> {
    match hash {
        HashAlgorithm::Sha256 => Ok(Pkcs1v15Sign::new::<sha2::Sha256>()),
        HashAlgorithm::Sha384 => Ok(Pkcs1v15Sign::new::<sha2::Sha384>()),
        HashAlgorithm::Sha512 => Ok(Pkcs1v15Sign::new::<sha2::Sha512>()),
        _ => Err(LuaError::runtime(
            "RSA signatures support SHA256, SHA384 and SHA512",
        )),
    }
}

fn rsa_pss_sign(
    key: &RsaPrivateKey,
    hash: HashAlgorithm,
    hashed: &[u8],
) -> LuaResult<rsa::Result<Vec<u8>>> {
    let mut rng = UnwrapErr(SysRng);
    match hash {
        HashAlgorithm::Sha256 => {
            Ok(key.sign_with_rng(&mut rng, Pss::<sha2::Sha256>::new(), hashed))
        }
        HashAlgorithm::Sha384 => {
            Ok(key.sign_with_rng(&mut rng, Pss::<sha2::Sha384>::new(), hashed))
        }
        HashAlgorithm::Sha512 => {
            Ok(key.sign_with_rng(&mut rng, Pss::<sha2::Sha512>::new(), hashed))
        }
        _ => Err(LuaError::runtime(
            "RSA signatures support SHA256, SHA384 and SHA512",
        )),
    }
}

fn rsa_pss_verify(
    key: &RsaPublicKey,
    hash: HashAlgorithm,
    hashed: &[u8],
    signature: &[u8],
) -> LuaResult<rsa::Result<()>> {
    match hash {
        HashAlgorithm::Sha256 => Ok(key.verify(Pss::<sha2::Sha256>::new(), hashed, signature)),
        HashAlgorithm::Sha384 => Ok(key.verify(Pss::<sha2::Sha384>::new(), hashed, signature)),
        HashAlgorithm::Sha512 => Ok(key.verify(Pss::<sha2::Sha512>::new(), hashed, signature)),
        _ => Err(LuaError::runtime(
            "RSA signatures support SHA256, SHA384 and SHA512",
        )),
    }
}
