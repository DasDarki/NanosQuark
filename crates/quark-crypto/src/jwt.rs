use mlua::UserDataRef;
use mlua::prelude::*;
use quark_core::Options;
use serde_json::{Map, Value};
use subtle::ConstantTimeEq;

use crate::encoding::{Format, decode_base64};
use crate::hash::{HashAlgorithm, hmac};
use crate::keys::{KeyAlgorithm, Padding, PrivateKey, PublicKey};
use crate::now_seconds;

#[derive(Clone, Copy, PartialEq)]
enum Algorithm {
    Hs256,
    Hs384,
    Hs512,
    Rs256,
    Rs384,
    Rs512,
    Ps256,
    Ps384,
    Ps512,
    Es256,
    Es384,
    EdDsa,
}

const ALGORITHMS: &[(&str, Algorithm)] = &[
    ("HS256", Algorithm::Hs256),
    ("HS384", Algorithm::Hs384),
    ("HS512", Algorithm::Hs512),
    ("RS256", Algorithm::Rs256),
    ("RS384", Algorithm::Rs384),
    ("RS512", Algorithm::Rs512),
    ("PS256", Algorithm::Ps256),
    ("PS384", Algorithm::Ps384),
    ("PS512", Algorithm::Ps512),
    ("ES256", Algorithm::Es256),
    ("ES384", Algorithm::Es384),
    ("EdDSA", Algorithm::EdDsa),
];

impl Algorithm {
    fn parse(name: &str) -> Option<Self> {
        ALGORITHMS
            .iter()
            .find(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
            .map(|(_, algorithm)| *algorithm)
    }

    fn name(self) -> &'static str {
        ALGORITHMS
            .iter()
            .find(|(_, candidate)| *candidate == self)
            .map(|(name, _)| *name)
            .unwrap_or_default()
    }

    fn hash(self) -> HashAlgorithm {
        match self {
            Algorithm::Hs384 | Algorithm::Rs384 | Algorithm::Ps384 | Algorithm::Es384 => {
                HashAlgorithm::Sha384
            }
            Algorithm::Hs512 | Algorithm::Rs512 | Algorithm::Ps512 | Algorithm::EdDsa => {
                HashAlgorithm::Sha512
            }
            _ => HashAlgorithm::Sha256,
        }
    }

    fn key_algorithm(self) -> Option<KeyAlgorithm> {
        match self {
            Algorithm::Hs256 | Algorithm::Hs384 | Algorithm::Hs512 => None,
            Algorithm::Rs256
            | Algorithm::Rs384
            | Algorithm::Rs512
            | Algorithm::Ps256
            | Algorithm::Ps384
            | Algorithm::Ps512 => Some(KeyAlgorithm::Rsa),
            Algorithm::Es256 => Some(KeyAlgorithm::EcdsaP256),
            Algorithm::Es384 => Some(KeyAlgorithm::EcdsaP384),
            Algorithm::EdDsa => Some(KeyAlgorithm::Ed25519),
        }
    }

    fn padding(self) -> Padding {
        match self {
            Algorithm::Ps256 | Algorithm::Ps384 | Algorithm::Ps512 => Padding::Pss,
            _ => Padding::Pkcs1,
        }
    }

    fn defaults_for(key: Option<KeyAlgorithm>) -> Vec<Algorithm> {
        ALGORITHMS
            .iter()
            .map(|(_, algorithm)| *algorithm)
            .filter(|algorithm| algorithm.key_algorithm() == key)
            .collect()
    }
}

enum SigningKey {
    Secret(Vec<u8>),
    Private(UserDataRef<PrivateKey>),
}

enum VerifyingKey {
    Secret(Vec<u8>),
    Public(PublicKey),
}

impl SigningKey {
    fn from_lua(value: LuaValue) -> LuaResult<Self> {
        match value {
            LuaValue::String(secret) => Ok(SigningKey::Secret(secret.as_bytes().to_vec())),
            LuaValue::UserData(userdata) if userdata.is::<PrivateKey>() => {
                Ok(SigningKey::Private(userdata.borrow::<PrivateKey>()?))
            }
            other => Err(LuaError::runtime(format!(
                "expected a secret string or a private key, got {}",
                other.type_name()
            ))),
        }
    }

    fn key_algorithm(&self) -> Option<KeyAlgorithm> {
        match self {
            SigningKey::Secret(_) => None,
            SigningKey::Private(key) => Some(key.algorithm()),
        }
    }
}

impl VerifyingKey {
    fn from_lua(value: LuaValue) -> LuaResult<Self> {
        match value {
            LuaValue::String(secret) => Ok(VerifyingKey::Secret(secret.as_bytes().to_vec())),
            LuaValue::UserData(userdata) if userdata.is::<PublicKey>() => Ok(VerifyingKey::Public(
                userdata.borrow::<PublicKey>()?.clone(),
            )),
            LuaValue::UserData(userdata) if userdata.is::<PrivateKey>() => Ok(
                VerifyingKey::Public(userdata.borrow::<PrivateKey>()?.public_key()),
            ),
            other => Err(LuaError::runtime(format!(
                "expected a secret string, a public key or a private key, got {}",
                other.type_name()
            ))),
        }
    }

    fn key_algorithm(&self) -> Option<KeyAlgorithm> {
        match self {
            VerifyingKey::Secret(_) => None,
            VerifyingKey::Public(key) => Some(key.algorithm()),
        }
    }
}

pub fn register(lua: &Lua, crypto: &LuaTable) -> LuaResult<()> {
    crypto.set("JWTSign", lua.create_function(sign)?)?;
    crypto.set("JWTVerify", lua.create_function(verify)?)?;
    crypto.set(
        "JWTDecode",
        lua.create_function(|lua, token: String| match decode(&token) {
            Ok(decoded) => {
                (to_lua(lua, &decoded.claims)?, to_lua(lua, &decoded.header)?).into_lua_multi(lua)
            }
            Err(error) => (LuaNil, error).into_lua_multi(lua),
        })?,
    )?;
    Ok(())
}

fn sign(lua: &Lua, (claims, key, options): (LuaValue, LuaValue, Options)) -> LuaResult<String> {
    let key = SigningKey::from_lua(key)?;
    let algorithm = match options.get::<String>("Algorithm")? {
        Some(name) => Algorithm::parse(&name)
            .ok_or_else(|| LuaError::runtime(format!("unsupported JWT algorithm '{name}'")))?,
        None => *Algorithm::defaults_for(key.key_algorithm())
            .first()
            .ok_or_else(|| LuaError::runtime("this key cannot sign JWTs"))?,
    };
    if algorithm.key_algorithm() != key.key_algorithm() {
        return Err(LuaError::runtime(format!(
            "the JWT algorithm {} does not match the key",
            algorithm.name()
        )));
    }

    let mut header = Map::new();
    header.insert("alg".to_owned(), Value::from(algorithm.name()));
    header.insert("typ".to_owned(), Value::from("JWT"));
    if let Some(extra) = options.get::<LuaValue>("Header")? {
        let Value::Object(extra) = lua.from_value::<Value>(extra)? else {
            return Err(LuaError::runtime("the JWT header has to be a table"));
        };
        for (name, value) in extra {
            if name != "alg" {
                header.insert(name, value);
            }
        }
    }

    let Value::Object(mut claims) = lua.from_value::<Value>(claims)? else {
        return Err(LuaError::runtime("the JWT claims have to be a table"));
    };
    let now = now_seconds();
    if options.get_or("IssuedAt", true)? && !claims.contains_key("iat") {
        claims.insert("iat".to_owned(), Value::from(now));
    }
    if let Some(seconds) = options.get::<u64>("ExpiresIn")? {
        claims.insert("exp".to_owned(), Value::from(now + seconds));
    }

    let header = serde_json::to_vec(&Value::Object(header)).map_err(LuaError::external)?;
    let claims = serde_json::to_vec(&Value::Object(claims)).map_err(LuaError::external)?;
    let signing_input = format!("{}.{}", base64url(&header), base64url(&claims));

    let signature = match &key {
        SigningKey::Secret(secret) => hmac(algorithm.hash(), secret, signing_input.as_bytes())?,
        SigningKey::Private(private) => private.sign(
            signing_input.as_bytes(),
            algorithm.padding(),
            Some(algorithm.hash()),
        )?,
    };

    Ok(format!("{signing_input}.{}", base64url(&signature)))
}

fn verify(
    lua: &Lua,
    (token, key, options): (String, LuaValue, Options),
) -> LuaResult<LuaMultiValue> {
    let key = VerifyingKey::from_lua(key)?;
    let allowed = match options.get::<Vec<String>>("Algorithms")? {
        Some(names) => names
            .iter()
            .map(|name| {
                Algorithm::parse(name)
                    .ok_or_else(|| LuaError::runtime(format!("unsupported JWT algorithm '{name}'")))
            })
            .collect::<LuaResult<Vec<_>>>()?,
        None => Algorithm::defaults_for(key.key_algorithm()),
    };
    let leeway = options.get_or("Leeway", 0u64)?;
    let issuer = options.get::<String>("Issuer")?;
    let audience = options.get::<String>("Audience")?;

    let decoded = match decode(&token) {
        Ok(decoded) => decoded,
        Err(error) => return (LuaNil, error).into_lua_multi(lua),
    };

    let result = check(
        &decoded,
        &key,
        &allowed,
        leeway,
        issuer.as_deref(),
        audience.as_deref(),
    )?;
    match result {
        Ok(()) => {
            (to_lua(lua, &decoded.claims)?, to_lua(lua, &decoded.header)?).into_lua_multi(lua)
        }
        Err(error) => (LuaNil, error).into_lua_multi(lua),
    }
}

struct Decoded {
    header: Value,
    claims: Value,
    signing_input: String,
    signature: Vec<u8>,
}

fn decode(token: &str) -> Result<Decoded, String> {
    let mut parts = token.trim().split('.');
    let (Some(header), Some(claims), Some(signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err("malformed token, expected three parts".to_owned());
    };

    let parse = |part: &str, name: &str| -> Result<Value, String> {
        let bytes =
            decode_base64(part.as_bytes(), true).map_err(|_| format!("malformed token {name}"))?;
        let value: Value =
            serde_json::from_slice(&bytes).map_err(|_| format!("malformed token {name}"))?;
        if value.is_object() {
            Ok(value)
        } else {
            Err(format!("malformed token {name}"))
        }
    };

    Ok(Decoded {
        header: parse(header, "header")?,
        claims: parse(claims, "claims")?,
        signing_input: format!("{header}.{claims}"),
        signature: decode_base64(signature.as_bytes(), true)
            .map_err(|_| "malformed token signature".to_owned())?,
    })
}

fn check(
    decoded: &Decoded,
    key: &VerifyingKey,
    allowed: &[Algorithm],
    leeway: u64,
    issuer: Option<&str>,
    audience: Option<&str>,
) -> LuaResult<Result<(), String>> {
    let Some(name) = decoded.header.get("alg").and_then(Value::as_str) else {
        return Ok(Err("the token has no algorithm".to_owned()));
    };
    let Some(algorithm) = Algorithm::parse(name) else {
        return Ok(Err(format!(
            "the token algorithm '{name}' is not supported"
        )));
    };
    if !allowed.contains(&algorithm) {
        return Ok(Err(format!(
            "the token algorithm {} is not allowed",
            algorithm.name()
        )));
    }
    if algorithm.key_algorithm() != key.key_algorithm() {
        return Ok(Err(format!(
            "the token algorithm {} does not match the key",
            algorithm.name()
        )));
    }

    let input = decoded.signing_input.as_bytes();
    let valid = match key {
        VerifyingKey::Secret(secret) => {
            bool::from(hmac(algorithm.hash(), secret, input)?.ct_eq(&decoded.signature))
        }
        VerifyingKey::Public(public) => public.verify(
            input,
            &decoded.signature,
            algorithm.padding(),
            Some(algorithm.hash()),
        )?,
    };
    if !valid {
        return Ok(Err("invalid token signature".to_owned()));
    }

    let now = now_seconds() as f64;
    let claim = |name: &str| decoded.claims.get(name);
    if let Some(expires) = claim("exp").and_then(Value::as_f64)
        && now > expires + leeway as f64
    {
        return Ok(Err("the token has expired".to_owned()));
    }
    if let Some(not_before) = claim("nbf").and_then(Value::as_f64)
        && now + (leeway as f64) < not_before
    {
        return Ok(Err("the token is not valid yet".to_owned()));
    }
    if let Some(issuer) = issuer
        && claim("iss").and_then(Value::as_str) != Some(issuer)
    {
        return Ok(Err("the token issuer does not match".to_owned()));
    }
    if let Some(audience) = audience {
        let matches = match claim("aud") {
            Some(Value::String(value)) => value == audience,
            Some(Value::Array(values)) => {
                values.iter().any(|value| value.as_str() == Some(audience))
            }
            _ => false,
        };
        if !matches {
            return Ok(Err("the token audience does not match".to_owned()));
        }
    }

    Ok(Ok(()))
}

fn to_lua(lua: &Lua, value: &Value) -> LuaResult<LuaValue> {
    lua.to_value_with(
        value,
        LuaSerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false),
    )
}

fn base64url(bytes: &[u8]) -> String {
    String::from_utf8(Format::Base64Url.encode(bytes).0).unwrap_or_default()
}
