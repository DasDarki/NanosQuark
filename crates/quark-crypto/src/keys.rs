use chacha20poly1305::XChaCha20Poly1305;
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use getrandom::SysRng;
use hkdf::SimpleHkdf;
use mlua::prelude::*;
use pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey,
    EncryptedPrivateKeyInfoRef, LineEnding, SecretDocument,
};
use quark_core::{Binary, Bytes, Object, Options, options_and_callback};
use rand_core::UnwrapErr;
use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey};
use rsa::{Oaep, Pkcs1v15Sign, Pss, RsaPrivateKey, RsaPublicKey};

use crate::encoding::Format;
use crate::hash::HashAlgorithm;
use crate::random::random_bytes;

const DEFAULT_RSA_BITS: usize = 2048;
const SEALED_BOX_INFO: &[u8] = b"quark sealed box v1";
const SEALED_BOX_KEY_LENGTH: usize = 32;
const SEALED_BOX_NONCE_LENGTH: usize = 24;
const SEALED_BOX_TAG_LENGTH: usize = 16;

#[derive(Clone, Copy, PartialEq)]
pub enum KeyAlgorithm {
    Ed25519,
    EcdsaP256,
    EcdsaP384,
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
            "ECDSAP384" | "P384" => Ok(KeyAlgorithm::EcdsaP384),
            "RSA" => Ok(KeyAlgorithm::Rsa),
            "X25519" => Ok(KeyAlgorithm::X25519),
            _ => Err(LuaError::runtime(format!(
                "unsupported key algorithm '{name}', expected Ed25519, ECDSA-P256, ECDSA-P384, RSA or X25519"
            ))),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            KeyAlgorithm::Ed25519 => "Ed25519",
            KeyAlgorithm::EcdsaP256 => "ECDSA-P256",
            KeyAlgorithm::EcdsaP384 => "ECDSA-P384",
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
    pub hash: Option<HashAlgorithm>,
}

impl SignOptions {
    fn parse(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        let mut options = SignOptions {
            format: Format::Hex,
            padding: Padding::Pkcs1,
            hash: None,
        };

        match value {
            LuaNil => {}
            LuaValue::String(_) => options.format = Format::from_lua(value, lua)?,
            LuaValue::Table(_) => {
                let table = Options::from_lua(value, lua)?;
                options.format = table.get_or("Format", options.format)?;
                options.hash = table.get("Hash")?;
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
    EcdsaP384(p384::ecdsa::SigningKey),
    Rsa(Box<RsaPrivateKey>),
    X25519(x25519_dalek::StaticSecret),
}

#[derive(Clone)]
pub enum PublicKey {
    Ed25519(ed25519_dalek::VerifyingKey),
    EcdsaP256(p256::ecdsa::VerifyingKey),
    EcdsaP384(p384::ecdsa::VerifyingKey),
    Rsa(Box<RsaPublicKey>),
    X25519(x25519_dalek::PublicKey),
}

impl PrivateKey {
    fn generate(algorithm: KeyAlgorithm, rsa_bits: usize) -> Result<Self, String> {
        match algorithm {
            KeyAlgorithm::Ed25519 => Ok(PrivateKey::Ed25519(
                ed25519_dalek::SigningKey::from_bytes(&random_array()?),
            )),
            KeyAlgorithm::X25519 => Ok(PrivateKey::X25519(x25519_dalek::StaticSecret::from(
                random_array()?,
            ))),
            KeyAlgorithm::EcdsaP256 => loop {
                if let Ok(key) = p256::ecdsa::SigningKey::from_slice(&random_array::<32>()?) {
                    return Ok(PrivateKey::EcdsaP256(key));
                }
            },
            KeyAlgorithm::EcdsaP384 => loop {
                if let Ok(key) = p384::ecdsa::SigningKey::from_slice(&random_array::<48>()?) {
                    return Ok(PrivateKey::EcdsaP384(key));
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
            PrivateKey::EcdsaP384(_) => KeyAlgorithm::EcdsaP384,
            PrivateKey::Rsa(_) => KeyAlgorithm::Rsa,
            PrivateKey::X25519(_) => KeyAlgorithm::X25519,
        }
    }

    pub fn public_key(&self) -> PublicKey {
        match self {
            PrivateKey::Ed25519(key) => PublicKey::Ed25519(key.verifying_key()),
            PrivateKey::EcdsaP256(key) => PublicKey::EcdsaP256(*key.verifying_key()),
            PrivateKey::EcdsaP384(key) => PublicKey::EcdsaP384(*key.verifying_key()),
            PrivateKey::Rsa(key) => PublicKey::Rsa(Box::new(key.to_public_key())),
            PrivateKey::X25519(key) => PublicKey::X25519(x25519_dalek::PublicKey::from(key)),
        }
    }

    pub fn sign(
        &self,
        data: &[u8],
        padding: Padding,
        hash: Option<HashAlgorithm>,
    ) -> LuaResult<Vec<u8>> {
        match self {
            PrivateKey::Ed25519(key) => {
                use ed25519_dalek::Signer;
                Ok(key.sign(data).to_bytes().to_vec())
            }
            PrivateKey::EcdsaP256(key) => {
                use p256::ecdsa::signature::Signer;
                require_hash(
                    hash,
                    HashAlgorithm::Sha256,
                    "ECDSA P-256 signatures always use SHA256",
                )?;
                let signature: p256::ecdsa::Signature = key.sign(data);
                Ok(signature.to_bytes().to_vec())
            }
            PrivateKey::EcdsaP384(key) => {
                use p384::ecdsa::signature::Signer;
                require_hash(
                    hash,
                    HashAlgorithm::Sha384,
                    "ECDSA P-384 signatures always use SHA384",
                )?;
                let signature: p384::ecdsa::Signature = key.sign(data);
                Ok(signature.to_bytes().to_vec())
            }
            PrivateKey::Rsa(key) => {
                let hash = hash.unwrap_or(HashAlgorithm::Sha256);
                let hashed = hash.digest(data);
                let result = match padding {
                    Padding::Pkcs1 => key.sign(rsa_pkcs1_scheme(hash)?, &hashed),
                    Padding::Pss => rsa_pss_sign(key, hash, &hashed)?,
                };
                result.map_err(LuaError::external)
            }
            PrivateKey::X25519(_) => Err(LuaError::runtime(
                "X25519 keys cannot sign, use DeriveSharedSecret or Open",
            )),
        }
    }

    fn shared_secret(&self, public: &PublicKey) -> LuaResult<Vec<u8>> {
        match (self, public) {
            (PrivateKey::X25519(secret), PublicKey::X25519(public)) => {
                let shared = secret.diffie_hellman(public);
                if !shared.was_contributory() {
                    return Err(LuaError::runtime(
                        "the public key is not valid for key exchange",
                    ));
                }
                Ok(shared.as_bytes().to_vec())
            }
            (PrivateKey::EcdsaP256(secret), PublicKey::EcdsaP256(public)) => Ok(
                p256::ecdh::diffie_hellman(secret.as_nonzero_scalar(), public.as_affine())
                    .raw_secret_bytes()
                    .to_vec(),
            ),
            (PrivateKey::EcdsaP384(secret), PublicKey::EcdsaP384(public)) => Ok(
                p384::ecdh::diffie_hellman(secret.as_nonzero_scalar(), public.as_affine())
                    .raw_secret_bytes()
                    .to_vec(),
            ),
            _ => Err(LuaError::runtime(
                "key exchange needs an X25519, ECDSA-P256 or ECDSA-P384 private key and a public key of the same algorithm",
            )),
        }
    }

    fn open(&self, sealed: &[u8], associated: &[u8]) -> LuaResult<Result<Bytes, String>> {
        let public = self.public_key();
        let ephemeral_length = sealed_box_public_length(self.algorithm())?;
        if sealed.len() < ephemeral_length + SEALED_BOX_TAG_LENGTH {
            return Ok(Err("the sealed box is too short".to_owned()));
        }

        let (ephemeral, ciphertext) = sealed.split_at(ephemeral_length);
        let Ok(ephemeral_key) = import_raw_public_key(ephemeral, self.algorithm()) else {
            return Ok(Err(
                "the sealed box contains an invalid public key".to_owned()
            ));
        };
        let Ok(shared) = self.shared_secret(&ephemeral_key) else {
            return Ok(Err(
                "the sealed box contains an invalid public key".to_owned()
            ));
        };

        let (cipher, nonce) = sealed_box_cipher(&shared, ephemeral, &public.export_raw()?)?;
        Ok(cipher
            .decrypt(
                &nonce,
                Payload {
                    msg: ciphertext,
                    aad: associated,
                },
            )
            .map(Bytes)
            .map_err(|_| {
                "the sealed box could not be opened, the key is wrong or the data was modified"
                    .to_owned()
            }))
    }

    fn export_pem(&self, password: Option<&[u8]>) -> LuaResult<String> {
        let pem = match (self, password) {
            (PrivateKey::Ed25519(key), None) => key.to_pkcs8_pem(LineEnding::LF),
            (PrivateKey::EcdsaP256(key), None) => key.to_pkcs8_pem(LineEnding::LF),
            (PrivateKey::EcdsaP384(key), None) => key.to_pkcs8_pem(LineEnding::LF),
            (PrivateKey::Rsa(key), None) => key.to_pkcs8_pem(LineEnding::LF),
            (PrivateKey::Ed25519(key), Some(password)) => {
                key.to_pkcs8_encrypted_pem(password, LineEnding::LF)
            }
            (PrivateKey::EcdsaP256(key), Some(password)) => {
                key.to_pkcs8_encrypted_pem(password, LineEnding::LF)
            }
            (PrivateKey::EcdsaP384(key), Some(password)) => {
                key.to_pkcs8_encrypted_pem(password, LineEnding::LF)
            }
            (PrivateKey::Rsa(key), Some(password)) => {
                key.to_pkcs8_encrypted_pem(password, LineEnding::LF)
            }
            (PrivateKey::X25519(_), _) => {
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
            PrivateKey::EcdsaP384(key) => Ok(key.to_bytes().to_vec()),
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
            PublicKey::EcdsaP384(_) => KeyAlgorithm::EcdsaP384,
            PublicKey::Rsa(_) => KeyAlgorithm::Rsa,
            PublicKey::X25519(_) => KeyAlgorithm::X25519,
        }
    }

    pub fn verify(
        &self,
        data: &[u8],
        signature: &[u8],
        padding: Padding,
        hash: Option<HashAlgorithm>,
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
                require_hash(
                    hash,
                    HashAlgorithm::Sha256,
                    "ECDSA P-256 signatures always use SHA256",
                )?;
                let Ok(signature) = p256::ecdsa::Signature::from_slice(signature) else {
                    return Ok(false);
                };
                Ok(key.verify(data, &signature).is_ok())
            }
            PublicKey::EcdsaP384(key) => {
                use p384::ecdsa::signature::Verifier;
                require_hash(
                    hash,
                    HashAlgorithm::Sha384,
                    "ECDSA P-384 signatures always use SHA384",
                )?;
                let Ok(signature) = p384::ecdsa::Signature::from_slice(signature) else {
                    return Ok(false);
                };
                Ok(key.verify(data, &signature).is_ok())
            }
            PublicKey::Rsa(key) => {
                let hash = hash.unwrap_or(HashAlgorithm::Sha256);
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

    fn encrypt(&self, data: &[u8], options: &Options) -> LuaResult<Vec<u8>> {
        let PublicKey::Rsa(key) = self else {
            return Err(LuaError::runtime(
                "Encrypt needs an RSA public key, use Seal for X25519 and ECDSA keys",
            ));
        };

        let hash = options.get_or("Hash", HashAlgorithm::Sha256)?;
        let label = options
            .get::<Binary>("Label")?
            .map(|label| label.as_bytes().to_vec());
        let mut rng = UnwrapErr(SysRng);
        let result = match (hash, label) {
            (HashAlgorithm::Sha1, None) => key.encrypt(&mut rng, Oaep::<sha1::Sha1>::new(), data),
            (HashAlgorithm::Sha256, None) => {
                key.encrypt(&mut rng, Oaep::<sha2::Sha256>::new(), data)
            }
            (HashAlgorithm::Sha384, None) => {
                key.encrypt(&mut rng, Oaep::<sha2::Sha384>::new(), data)
            }
            (HashAlgorithm::Sha512, None) => {
                key.encrypt(&mut rng, Oaep::<sha2::Sha512>::new(), data)
            }
            (HashAlgorithm::Sha1, Some(label)) => {
                key.encrypt(&mut rng, Oaep::<sha1::Sha1>::new_with_label(label), data)
            }
            (HashAlgorithm::Sha256, Some(label)) => {
                key.encrypt(&mut rng, Oaep::<sha2::Sha256>::new_with_label(label), data)
            }
            (HashAlgorithm::Sha384, Some(label)) => {
                key.encrypt(&mut rng, Oaep::<sha2::Sha384>::new_with_label(label), data)
            }
            (HashAlgorithm::Sha512, Some(label)) => {
                key.encrypt(&mut rng, Oaep::<sha2::Sha512>::new_with_label(label), data)
            }
            _ => {
                return Err(LuaError::runtime(
                    "RSA OAEP supports SHA1, SHA256, SHA384 and SHA512",
                ));
            }
        };
        result.map_err(|error| LuaError::runtime(format!("RSA encryption failed: {error}")))
    }

    fn seal(&self, data: &[u8], associated: &[u8]) -> LuaResult<Vec<u8>> {
        let algorithm = self.algorithm();
        sealed_box_public_length(algorithm)?;

        let ephemeral =
            PrivateKey::generate(algorithm, DEFAULT_RSA_BITS).map_err(LuaError::runtime)?;
        let shared = ephemeral.shared_secret(self)?;
        let mut sealed = ephemeral.public_key().export_raw()?;

        let (cipher, nonce) = sealed_box_cipher(&shared, &sealed, &self.export_raw()?)?;
        let ciphertext = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: data,
                    aad: associated,
                },
            )
            .map_err(|_| LuaError::runtime("sealing failed"))?;
        sealed.extend_from_slice(&ciphertext);
        Ok(sealed)
    }

    fn export_pem(&self) -> LuaResult<String> {
        let pem = match self {
            PublicKey::Ed25519(key) => key.to_public_key_pem(LineEnding::LF),
            PublicKey::EcdsaP256(key) => key.to_public_key_pem(LineEnding::LF),
            PublicKey::EcdsaP384(key) => key.to_public_key_pem(LineEnding::LF),
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
            PublicKey::EcdsaP256(key) => Ok(key.to_sec1_point(true).as_bytes().to_vec()),
            PublicKey::EcdsaP384(key) => Ok(key.to_sec1_point(true).as_bytes().to_vec()),
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
        methods.add_method("Sign", |lua, key, (data, options): (Binary, LuaValue)| {
            let options = SignOptions::parse(options, lua)?;
            let signature = key.sign(&data.as_bytes(), options.padding, options.hash)?;
            Ok(options.format.encode(&signature))
        });
        methods.add_method(
            "DeriveSharedSecret",
            |_, key, (public, format): (LuaAnyUserData, Option<Format>)| {
                let shared = key.shared_secret(&*public.borrow::<PublicKey>()?)?;
                Ok(format.unwrap_or(Format::Raw).encode(&shared))
            },
        );
        methods.add_method(
            "Open",
            |_, key, (sealed, associated): (Binary, Option<Binary>)| {
                let associated = associated
                    .map(|value| value.as_bytes().to_vec())
                    .unwrap_or_default();
                key.open(&sealed.as_bytes(), &associated)
            },
        );
        methods.add_method("ExportPEM", |_, key, password: Option<Binary>| {
            key.export_pem(
                password
                    .as_ref()
                    .map(|password| password.as_bytes())
                    .as_deref(),
            )
        });
        methods.add_method("ExportRaw", |_, key, format: Option<Format>| {
            Ok(format.unwrap_or(Format::Raw).encode(&key.export_raw()?))
        });
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
            |lua, key, (data, signature, options): (Binary, Binary, LuaValue)| {
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
        methods.add_method("Encrypt", |_, key, (data, options): (Binary, Options)| {
            Ok(Bytes(key.encrypt(&data.as_bytes(), &options)?))
        });
        methods.add_method(
            "Seal",
            |_, key, (data, associated): (Binary, Option<Binary>)| {
                let associated = associated
                    .map(|value| value.as_bytes().to_vec())
                    .unwrap_or_default();
                Ok(Bytes(key.seal(&data.as_bytes(), &associated)?))
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
        lua.create_function(|lua, (data, options): (Binary, LuaValue)| {
            let (algorithm, password) = import_options(lua, options)?;
            Ok(import_private_key(&data.as_bytes(), algorithm, password.as_deref()).map(Object))
        })?,
    )?;
    crypto.set(
        "ImportPublicKey",
        lua.create_function(|_, (data, algorithm): (Binary, Option<KeyAlgorithm>)| {
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

fn import_options(
    lua: &Lua,
    value: LuaValue,
) -> LuaResult<(Option<KeyAlgorithm>, Option<Vec<u8>>)> {
    match value {
        LuaNil => Ok((None, None)),
        LuaValue::String(_) => Ok((Some(KeyAlgorithm::from_lua(value, lua)?), None)),
        LuaValue::Table(_) => {
            let options = Options::from_lua(value, lua)?;
            let password = options
                .get::<Binary>("Password")?
                .map(|password| password.as_bytes().to_vec());
            Ok((options.get("Algorithm")?, password))
        }
        other => Err(LuaError::runtime(format!(
            "expected an algorithm or an options table, got {}",
            other.type_name()
        ))),
    }
}

fn import_private_key(
    data: &[u8],
    algorithm: Option<KeyAlgorithm>,
    password: Option<&[u8]>,
) -> Result<PrivateKey, String> {
    if let Some(pem) = pem_text(data) {
        if pem.starts_with("-----BEGIN ENCRYPTED PRIVATE KEY") {
            let password =
                password.ok_or("the private key is encrypted, pass a Password option")?;
            let (_, document) =
                SecretDocument::from_pem(pem).map_err(|error| format!("invalid PEM: {error}"))?;
            let decrypted = EncryptedPrivateKeyInfoRef::try_from(document.as_bytes())
                .and_then(|info| info.decrypt(password))
                .map_err(|_| "the private key could not be decrypted, the password is wrong or the key is damaged".to_owned())?;
            return private_key_from_pkcs8(decrypted.as_bytes());
        }
        if pem.starts_with("-----BEGIN RSA PRIVATE KEY") {
            return RsaPrivateKey::from_pkcs1_pem(pem)
                .map(|key| PrivateKey::Rsa(Box::new(key)))
                .map_err(|error| format!("invalid RSA private key: {error}"));
        }
        if pem.starts_with("-----BEGIN EC PRIVATE KEY") {
            if let Ok(key) = p256::SecretKey::from_sec1_pem(pem) {
                return Ok(PrivateKey::EcdsaP256(p256::ecdsa::SigningKey::from(key)));
            }
            return p384::SecretKey::from_sec1_pem(pem)
                .map(|key| PrivateKey::EcdsaP384(p384::ecdsa::SigningKey::from(key)))
                .map_err(|error| format!("invalid EC private key: {error}"));
        }
        if pem.starts_with("-----BEGIN PRIVATE KEY") {
            let (_, document) =
                SecretDocument::from_pem(pem).map_err(|error| format!("invalid PEM: {error}"))?;
            return private_key_from_pkcs8(document.as_bytes());
        }
        return Err("unsupported PEM block, expected a private key".to_owned());
    }

    let algorithm = algorithm.ok_or("raw private keys need the algorithm as second argument")?;
    match algorithm {
        KeyAlgorithm::Ed25519 => Ok(PrivateKey::Ed25519(ed25519_dalek::SigningKey::from_bytes(
            &array_from(data)?,
        ))),
        KeyAlgorithm::X25519 => Ok(PrivateKey::X25519(x25519_dalek::StaticSecret::from(
            array_from::<32>(data)?,
        ))),
        KeyAlgorithm::EcdsaP256 => p256::ecdsa::SigningKey::from_slice(data)
            .map(PrivateKey::EcdsaP256)
            .map_err(|_| "invalid ECDSA P-256 private key".to_owned()),
        KeyAlgorithm::EcdsaP384 => p384::ecdsa::SigningKey::from_slice(data)
            .map(PrivateKey::EcdsaP384)
            .map_err(|_| "invalid ECDSA P-384 private key".to_owned()),
        KeyAlgorithm::Rsa => Err("RSA private keys can only be imported from PEM".to_owned()),
    }
}

fn private_key_from_pkcs8(der: &[u8]) -> Result<PrivateKey, String> {
    if let Ok(key) = ed25519_dalek::SigningKey::from_pkcs8_der(der) {
        return Ok(PrivateKey::Ed25519(key));
    }
    if let Ok(key) = p256::ecdsa::SigningKey::from_pkcs8_der(der) {
        return Ok(PrivateKey::EcdsaP256(key));
    }
    if let Ok(key) = p384::ecdsa::SigningKey::from_pkcs8_der(der) {
        return Ok(PrivateKey::EcdsaP384(key));
    }
    if let Ok(key) = RsaPrivateKey::from_pkcs8_der(der) {
        return Ok(PrivateKey::Rsa(Box::new(key)));
    }
    Err(
        "unsupported PKCS#8 private key, expected Ed25519, ECDSA P-256, ECDSA P-384 or RSA"
            .to_owned(),
    )
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
            if let Ok(key) = p384::ecdsa::VerifyingKey::from_public_key_pem(pem) {
                return Ok(PublicKey::EcdsaP384(key));
            }
            if let Ok(key) = RsaPublicKey::from_public_key_pem(pem) {
                return Ok(PublicKey::Rsa(Box::new(key)));
            }
            return Err(
                "unsupported public key, expected Ed25519, ECDSA P-256, ECDSA P-384 or RSA"
                    .to_owned(),
            );
        }
        return Err("unsupported PEM block, expected a public key".to_owned());
    }

    let algorithm = algorithm.ok_or("raw public keys need the algorithm as second argument")?;
    import_raw_public_key(data, algorithm)
}

fn import_raw_public_key(data: &[u8], algorithm: KeyAlgorithm) -> Result<PublicKey, String> {
    match algorithm {
        KeyAlgorithm::Ed25519 => ed25519_dalek::VerifyingKey::from_bytes(&array_from(data)?)
            .map(PublicKey::Ed25519)
            .map_err(|_| "invalid Ed25519 public key".to_owned()),
        KeyAlgorithm::X25519 => Ok(PublicKey::X25519(x25519_dalek::PublicKey::from(
            array_from::<32>(data)?,
        ))),
        KeyAlgorithm::EcdsaP256 => p256::ecdsa::VerifyingKey::from_sec1_bytes(data)
            .map(PublicKey::EcdsaP256)
            .map_err(|_| "invalid ECDSA P-256 public key".to_owned()),
        KeyAlgorithm::EcdsaP384 => p384::ecdsa::VerifyingKey::from_sec1_bytes(data)
            .map(PublicKey::EcdsaP384)
            .map_err(|_| "invalid ECDSA P-384 public key".to_owned()),
        KeyAlgorithm::Rsa => Err("RSA public keys can only be imported from PEM".to_owned()),
    }
}

fn sealed_box_public_length(algorithm: KeyAlgorithm) -> LuaResult<usize> {
    match algorithm {
        KeyAlgorithm::X25519 => Ok(32),
        KeyAlgorithm::EcdsaP256 => Ok(33),
        KeyAlgorithm::EcdsaP384 => Ok(49),
        _ => Err(LuaError::runtime(
            "sealed boxes need an X25519, ECDSA-P256 or ECDSA-P384 key, use Encrypt for RSA",
        )),
    }
}

fn sealed_box_cipher(
    shared: &[u8],
    ephemeral: &[u8],
    recipient: &[u8],
) -> LuaResult<(XChaCha20Poly1305, chacha20poly1305::XNonce)> {
    let salt = [ephemeral, recipient].concat();
    let mut output = [0; SEALED_BOX_KEY_LENGTH + SEALED_BOX_NONCE_LENGTH];
    SimpleHkdf::<sha2::Sha256>::new(Some(&salt), shared)
        .expand(SEALED_BOX_INFO, &mut output)
        .map_err(|_| LuaError::runtime("failed to derive the sealed box key"))?;

    let (key, nonce) = output.split_at(SEALED_BOX_KEY_LENGTH);
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| LuaError::runtime("failed to create the sealed box cipher"))?;
    let nonce = nonce
        .try_into()
        .map_err(|_| LuaError::runtime("failed to derive the sealed box nonce"))?;
    Ok((cipher, nonce))
}

fn pem_text(data: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(data).ok()?.trim();
    text.starts_with("-----BEGIN").then_some(text)
}

fn random_array<const N: usize>() -> Result<[u8; N], String> {
    let bytes = random_bytes(N).map_err(|error| error.to_string())?;
    array_from(&bytes)
}

fn array_from<const N: usize>(data: &[u8]) -> Result<[u8; N], String> {
    data.try_into()
        .map_err(|_| format!("expected a {N} byte key, got {} bytes", data.len()))
}

fn require_hash(
    hash: Option<HashAlgorithm>,
    expected: HashAlgorithm,
    message: &str,
) -> LuaResult<()> {
    match hash {
        None => Ok(()),
        Some(hash) if hash == expected => Ok(()),
        Some(_) => Err(LuaError::runtime(message.to_owned())),
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
