# Crypto

```lua
local Crypto = Quark.Import("Crypto")
```

## Conventions

**Binary data** is passed and returned as Lua strings. Keys, salts and ciphertexts can contain any byte.

**Formats.** Functions returning binary data accept a format: `"hex"`, `"base64"`, `"base64url"` or `"raw"`. Hashes, MACs and signatures default to `"hex"`. Key material and ciphertexts (`RandomBytes`, `HKDF`, `PBKDF2`, `ExportRaw`, `DeriveSharedSecret`, encryption) default to `"raw"`, because they usually feed into other functions.

**Slow functions** (password hashing, PBKDF2, key generation) take an optional callback as last argument. With a callback the work runs on a worker thread and the callback receives the result on the game thread. Without a callback the function runs synchronously and blocks the server for its duration.

```lua
local hash = Crypto.Argon2Hash(password)                -- blocks
Crypto.Argon2Hash(password, function(hash, error) end)  -- does not block
```

**Errors.** Failures caused by the data, such as a wrong password, a modified ciphertext or a malformed token, return `nil, message` (or call the callback with `nil, message`). Programming errors, such as a wrong argument type or an invalid key length, raise a Lua error.

## Hashing

| Function | Returns |
| --- | --- |
| `Crypto.Hash(algorithm, data, format?)` | digest |
| `Crypto.MD5`, `SHA1`, `SHA256`, `SHA384`, `SHA512`, `SHA3_256`, `SHA3_512`, `BLAKE3` `(data, format?)` | digest |
| `Crypto.Hasher(algorithm)` | incremental hasher with `:Update(data)` (chainable) and `:Finalize(format?)` |
| `Crypto.HMAC(algorithm, key, data, format?)` | MAC |
| `Crypto.HMACVerify(algorithm, key, data, mac, format?)` | `true` / `false`, constant time |

Algorithms: `MD5`, `SHA1`, `SHA224`, `SHA256`, `SHA384`, `SHA512`, `SHA3-256`, `SHA3-384`, `SHA3-512`, `BLAKE3`. Names are case insensitive, `-` and `_` are ignored. MD5 and SHA-1 are only meant for interoperability.

```lua
local hasher = Crypto.Hasher("SHA256")
hasher:Update(chunk_one):Update(chunk_two)
local digest = hasher:Finalize()
```

## Key derivation

| Function | Options (defaults) |
| --- | --- |
| `Crypto.HKDF(key, options?)` | `Algorithm` (`SHA256`), `Salt`, `Info`, `Length` (32), `Format` (`raw`) |
| `Crypto.PBKDF2(password, salt, options?, callback?)` | `Algorithm` (`SHA256`), `Iterations` (600000), `Length` (32), `Format` (`raw`) |

## Passwords

| Function | Options (defaults) |
| --- | --- |
| `Crypto.Argon2Hash(password, options?, callback?)` | `Variant` (`argon2id`), `MemoryCost` in KiB (19456), `TimeCost` (2), `Parallelism` (1) |
| `Crypto.BcryptHash(password, options?, callback?)` | `Cost` (12), passwords longer than 72 bytes are rejected instead of truncated |
| `Crypto.ScryptHash(password, options?, callback?)` | `LogN` (17), `R` (8), `P` (1) |
| `Crypto.PasswordVerify(password, hash, callback?)` | detects Argon2, bcrypt and scrypt hashes |
| `Crypto.Argon2Verify`, `BcryptVerify`, `ScryptVerify` `(password, hash, callback?)` | like `PasswordVerify`, but only accept their own format |

Hashes are self describing strings (PHC format for Argon2 and scrypt, `$2b$` for bcrypt), so changing the options later keeps existing hashes valid. Verification returns `true` or `false`, a malformed hash returns `nil, message`.

```lua
Crypto.PasswordVerify(input, stored_hash, function(valid, error)
    if valid then Console.Log("welcome") end
end)
```

## Encryption

| Function | Key |
| --- | --- |
| `Crypto.AESEncrypt(key, plaintext, associated_data?)` / `AESDecrypt(key, ciphertext, associated_data?)` | 16 or 32 bytes (AES-128-GCM / AES-256-GCM) |
| `Crypto.XChaCha20Encrypt(key, plaintext, associated_data?)` / `XChaCha20Decrypt(...)` | 32 bytes |

Encryption generates a random nonce and returns `nonce .. ciphertext .. tag` as raw bytes. Decryption returns the plaintext or `nil, message` when the key is wrong or the data or associated data was modified. Use `Crypto.RandomBytes(32)` for keys and `Crypto.Base64Encode` to store the result as text.

```lua
local key = Crypto.RandomBytes(32)
local sealed = Crypto.AESEncrypt(key, "secret", "player:42")
local plaintext = Crypto.AESDecrypt(key, sealed, "player:42")
```

## Key pairs and signatures

`Crypto.GenerateKeyPair(algorithm, options?, callback?)` returns a private and a public key.

| Algorithm | Signatures | Key exchange and sealed boxes | Encryption |
| --- | --- | --- | --- |
| `Ed25519` | yes (recommended) | | |
| `ECDSA-P256` | yes, SHA-256 | yes | |
| `ECDSA-P384` | yes, SHA-384 | yes | |
| `RSA` (option `Bits`, 2048 to 8192, default 2048) | yes, PKCS#1 v1.5 or PSS | | OAEP, encrypt only |
| `X25519` | | yes (recommended) | |

RSA key generation takes noticeable time, pass a callback.

| Private key method | |
| --- | --- |
| `:Sign(data, format_or_options?)` | signature, options `Format`, `Padding` (`PKCS1` or `PSS`, RSA only), `Hash` (RSA: `SHA256`, `SHA384` or `SHA512`, ECDSA keys always use their curve's hash) |
| `:GetPublicKey()` | public key |
| `:DeriveSharedSecret(public_key, format?)` | shared secret for X25519, P-256 and P-384, run it through `HKDF` before using it as a key |
| `:Open(sealed, associated_data?)` | plaintext of a sealed box, or `nil, message` |
| `:ExportPEM(password?)` | PKCS#8 PEM, encrypted with the password if given (not for X25519) |
| `:ExportRaw(format?)` | raw private key: 32 bytes, 48 for P-384 (not for RSA) |
| `:GetAlgorithm()` | algorithm name |

| Public key method | |
| --- | --- |
| `:Verify(data, signature, format_or_options?)` | `true` / `false`, same options as `Sign` |
| `:Seal(data, associated_data?)` | sealed box only the private key can open (X25519, P-256, P-384) |
| `:Encrypt(data, options?)` | RSA-OAEP ciphertext, options `Hash` (`SHA256`, also `SHA1`, `SHA384`, `SHA512`) and `Label` |
| `:ExportPEM()` | SPKI PEM (not for X25519) |
| `:ExportRaw(format?)` | 32 bytes for Ed25519 and X25519, compressed SEC1 point for P-256 and P-384 (not for RSA) |
| `:GetAlgorithm()` | algorithm name |

`Crypto.ImportPrivateKey(data, algorithm_or_options?)` and `Crypto.ImportPublicKey(data, algorithm?)` read PEM (PKCS#8, encrypted PKCS#8, SPKI, PKCS#1 for RSA, SEC1 for P-256 and P-384) and detect the algorithm. Raw keys need the algorithm, encrypted keys the password: `Crypto.ImportPrivateKey(pem, { Password = "..." })`. Invalid input or a wrong password returns `nil, message`.

```lua
local private, public = Crypto.GenerateKeyPair("Ed25519")
local signature = private:Sign("payload")
public:Verify("payload", signature) -- true
```

### Sealed boxes

A sealed box encrypts data for the owner of a public key, without a shared secret beforehand. Use it to encrypt data for another server or to store data that only an offline key can read.

```lua
local private, public = Crypto.GenerateKeyPair("X25519")
local sealed = public:Seal("secret", "context")
local plaintext = private:Open(sealed, "context")
```

Construction: an ephemeral key pair of the recipient's algorithm performs a key exchange with the recipient's public key. HKDF-SHA256 (salt: ephemeral public key followed by recipient public key, info: `quark sealed box v1`) derives a 32 byte key and a 24 byte nonce for XChaCha20-Poly1305. The result is `ephemeral public key .. ciphertext .. tag`, the public key being raw (X25519) or a compressed SEC1 point (P-256, P-384).

### Encrypted private keys

`ExportPEM(password)` writes standard `ENCRYPTED PRIVATE KEY` PEM (PBES2 with scrypt and AES-256-CBC) that OpenSSL and other tools read. Encrypting and decrypting takes a moment because of scrypt.

### RSA and timing attacks

The pure Rust `rsa` crate does not use constant time arithmetic and is affected by the Marvin attack ([RUSTSEC-2023-0071](https://rustsec.org/advisories/RUSTSEC-2023-0071.html)). An attacker who can measure how long private key operations take over the network may recover the key. Quark therefore offers RSA encryption only with the public key (`Encrypt`, which is unaffected) and no RSA decryption. RSA signing uses the private key as well: prefer Ed25519 or ECDSA for anything that signs data on request, and keep RSA for interoperability.

## JSON Web Tokens

| Function | Options |
| --- | --- |
| `Crypto.JWTSign(claims, key, options?)` | `Algorithm`, `ExpiresIn` (seconds), `IssuedAt` (default `true`), `Header` (extra header fields) |
| `Crypto.JWTVerify(token, key, options?)` | `Algorithms` (allow list), `Leeway` (seconds), `Issuer`, `Audience` |
| `Crypto.JWTDecode(token)` | none, decodes without verifying |

The key is a secret string for `HS256`/`HS384`/`HS512` or a key object for `RS*`, `PS*`, `ES256`, `ES384` and `EdDSA`. Without `Algorithm` the first algorithm matching the key is used. Verification only accepts algorithms matching the key type, which prevents algorithm confusion attacks, and checks `exp` and `nbf`. It returns `claims, header` or `nil, reason`.

```lua
local token = Crypto.JWTSign({ sub = player_id }, secret, { ExpiresIn = 3600 })
local claims, reason = Crypto.JWTVerify(token, secret)
```

## One time passwords

| Function | |
| --- | --- |
| `Crypto.TOTPGenerateSecret(bytes?)` | Base32 secret (20 bytes by default) |
| `Crypto.TOTP(secret, options?)` | current code, options `Algorithm` (`SHA1`), `Digits` (6), `Period` (30), `Time` |
| `Crypto.TOTPVerify(secret, code, options?)` | `true` / `false`, additional option `Window` (1 period before and after) |
| `Crypto.HOTP(secret, counter, options?)` | counter based code |
| `Crypto.TOTPUri(secret, account, options?)` | `otpauth://` URI for QR codes, additional option `Issuer` |

Secrets are Base32 strings as used by authenticator apps.

## Randomness and encoding

| Function | |
| --- | --- |
| `Crypto.RandomBytes(count)` | secure random bytes |
| `Crypto.RandomInt(minimum, maximum)` | uniformly distributed integer, both bounds inclusive |
| `Crypto.RandomString(length, alphabet?)` | random characters, alphanumeric by default |
| `Crypto.RandomToken(bytes?)` | URL safe Base64 token (32 bytes by default) |
| `Crypto.ConstantTimeEquals(a, b)` | comparison without timing leaks |
| `Crypto.HexEncode` / `HexDecode` | hex |
| `Crypto.Base64Encode(data, url_safe?)` / `Base64Decode(text, url_safe?)` | Base64, padding is optional when decoding |
| `Crypto.Base32Encode` / `Base32Decode` | Base32 (RFC 4648), decoding ignores case, spaces and padding |
| `Crypto.Base58Encode` / `Base58Decode` | Base58 with the Bitcoin alphabet |
