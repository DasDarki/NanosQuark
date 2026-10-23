---@meta

---@alias QuarkCryptoFormat "hex"|"base64"|"base64url"|"raw"
---@alias QuarkHashAlgorithm "MD5"|"SHA1"|"SHA224"|"SHA256"|"SHA384"|"SHA512"|"SHA3-256"|"SHA3-384"|"SHA3-512"|"BLAKE3"|string
---@alias QuarkKeyAlgorithm "Ed25519"|"ECDSA-P256"|"ECDSA-P384"|"RSA"|"X25519"

---@class QuarkHasher
local Hasher = {}

---Adds data, chainable.
---@param data QuarkBinary
---@return QuarkHasher
function Hasher:Update(data) end

---The digest.
---@param format? QuarkCryptoFormat
---@return string
function Hasher:Finalize(format) end

---@class QuarkPrivateKey
local PrivateKey = {}

---Signature of the data.
---@param data QuarkBinary
---@param format_or_options? QuarkCryptoFormat|{ Format?: QuarkCryptoFormat, Padding?: "PKCS1"|"PSS", Hash?: "SHA256"|"SHA384"|"SHA512" }
---@return string
function PrivateKey:Sign(data, format_or_options) end

---@return QuarkPublicKey
function PrivateKey:GetPublicKey() end

---Shared secret for X25519, P-256 and P-384, run it through HKDF.
---@param public_key QuarkPublicKey
---@param format? QuarkCryptoFormat
---@return string
function PrivateKey:DeriveSharedSecret(public_key, format) end

---Plaintext of a sealed box.
---@param sealed QuarkBinary
---@param associated_data? QuarkBinary
---@return string?
---@return string? message
function PrivateKey:Open(sealed, associated_data) end

---PKCS#8 PEM, encrypted with the password if given.
---@param password? string
---@return string
function PrivateKey:ExportPEM(password) end

---@param format? QuarkCryptoFormat
---@return string
function PrivateKey:ExportRaw(format) end

---@return QuarkKeyAlgorithm
function PrivateKey:GetAlgorithm() end

---@class QuarkPublicKey
local PublicKey = {}

---@param data QuarkBinary
---@param signature QuarkBinary
---@param format_or_options? QuarkCryptoFormat|{ Format?: QuarkCryptoFormat, Padding?: "PKCS1"|"PSS", Hash?: "SHA256"|"SHA384"|"SHA512" }
---@return boolean
function PublicKey:Verify(data, signature, format_or_options) end

---Sealed box only the private key can open.
---@param data QuarkBinary
---@param associated_data? QuarkBinary
---@return string
function PublicKey:Seal(data, associated_data) end

---RSA-OAEP ciphertext.
---@param data QuarkBinary
---@param options? { Hash?: "SHA1"|"SHA256"|"SHA384"|"SHA512", Label?: string }
---@return string
function PublicKey:Encrypt(data, options) end

---@return string
function PublicKey:ExportPEM() end

---@param format? QuarkCryptoFormat
---@return string
function PublicKey:ExportRaw(format) end

---@return QuarkKeyAlgorithm
function PublicKey:GetAlgorithm() end

---@class QuarkCryptoModule
local Crypto = {}

---@param algorithm QuarkHashAlgorithm
---@param data QuarkBinary
---@param format? QuarkCryptoFormat
---@return string
function Crypto.Hash(algorithm, data, format) end

---@param data QuarkBinary
---@param format? QuarkCryptoFormat
---@return string
function Crypto.MD5(data, format) end

---@param data QuarkBinary
---@param format? QuarkCryptoFormat
---@return string
function Crypto.SHA1(data, format) end

---@param data QuarkBinary
---@param format? QuarkCryptoFormat
---@return string
function Crypto.SHA256(data, format) end

---@param data QuarkBinary
---@param format? QuarkCryptoFormat
---@return string
function Crypto.SHA384(data, format) end

---@param data QuarkBinary
---@param format? QuarkCryptoFormat
---@return string
function Crypto.SHA512(data, format) end

---@param data QuarkBinary
---@param format? QuarkCryptoFormat
---@return string
function Crypto.SHA3_256(data, format) end

---@param data QuarkBinary
---@param format? QuarkCryptoFormat
---@return string
function Crypto.SHA3_512(data, format) end

---@param data QuarkBinary
---@param format? QuarkCryptoFormat
---@return string
function Crypto.BLAKE3(data, format) end

---Incremental hasher.
---@param algorithm QuarkHashAlgorithm
---@return QuarkHasher
function Crypto.Hasher(algorithm) end

---@param algorithm QuarkHashAlgorithm
---@param key QuarkBinary
---@param data QuarkBinary
---@param format? QuarkCryptoFormat
---@return string
function Crypto.HMAC(algorithm, key, data, format) end

---Constant time comparison.
---@param algorithm QuarkHashAlgorithm
---@param key QuarkBinary
---@param data QuarkBinary
---@param mac string
---@param format? QuarkCryptoFormat
---@return boolean
function Crypto.HMACVerify(algorithm, key, data, mac, format) end

---@param key QuarkBinary
---@param options? { Algorithm?: QuarkHashAlgorithm, Salt?: QuarkBinary, Info?: QuarkBinary, Length?: integer, Format?: QuarkCryptoFormat }
---@return string
function Crypto.HKDF(key, options) end

---@param password QuarkBinary
---@param salt QuarkBinary
---@param options? { Algorithm?: QuarkHashAlgorithm, Iterations?: integer, Length?: integer, Format?: QuarkCryptoFormat }
---@param callback? function
---@return string?
---@return string? message
function Crypto.PBKDF2(password, salt, options, callback) end

---Password hash in PHC format.
---@param password QuarkBinary
---@param options? { Variant?: "argon2id"|"argon2i"|"argon2d", MemoryCost?: integer, TimeCost?: integer, Parallelism?: integer }
---@param callback? function
---@return string?
---@return string? message
function Crypto.Argon2Hash(password, options, callback) end

---@param password QuarkBinary
---@param options? { Cost?: integer }
---@param callback? function
---@return string?
---@return string? message
function Crypto.BcryptHash(password, options, callback) end

---@param password QuarkBinary
---@param options? { LogN?: integer, R?: integer, P?: integer }
---@param callback? function
---@return string?
---@return string? message
function Crypto.ScryptHash(password, options, callback) end

---@param password QuarkBinary
---@param hash string
---@param callback? function
---@return boolean?
---@return string? message
function Crypto.PasswordVerify(password, hash, callback) end

---@param password QuarkBinary
---@param hash string
---@param callback? function
---@return boolean?
---@return string? message
function Crypto.Argon2Verify(password, hash, callback) end

---@param password QuarkBinary
---@param hash string
---@param callback? function
---@return boolean?
---@return string? message
function Crypto.BcryptVerify(password, hash, callback) end

---@param password QuarkBinary
---@param hash string
---@param callback? function
---@return boolean?
---@return string? message
function Crypto.ScryptVerify(password, hash, callback) end

---Returns nonce .. ciphertext .. tag.
---@param key QuarkBinary
---@param plaintext QuarkBinary
---@param associated_data? QuarkBinary
---@return string
function Crypto.AESEncrypt(key, plaintext, associated_data) end

---@param key QuarkBinary
---@param ciphertext QuarkBinary
---@param associated_data? QuarkBinary
---@return string?
---@return string? message
function Crypto.AESDecrypt(key, ciphertext, associated_data) end

---Returns nonce .. ciphertext .. tag.
---@param key QuarkBinary
---@param plaintext QuarkBinary
---@param associated_data? QuarkBinary
---@return string
function Crypto.XChaCha20Encrypt(key, plaintext, associated_data) end

---@param key QuarkBinary
---@param ciphertext QuarkBinary
---@param associated_data? QuarkBinary
---@return string?
---@return string? message
function Crypto.XChaCha20Decrypt(key, ciphertext, associated_data) end

---@param algorithm QuarkKeyAlgorithm
---@param options? { Bits?: integer }
---@param callback? function
---@return QuarkPrivateKey
---@return QuarkPublicKey
function Crypto.GenerateKeyPair(algorithm, options, callback) end

---@param data QuarkBinary
---@param algorithm_or_options? QuarkKeyAlgorithm|{ Algorithm?: QuarkKeyAlgorithm, Password?: string }
---@return QuarkPrivateKey?
---@return string? message
function Crypto.ImportPrivateKey(data, algorithm_or_options) end

---@param data QuarkBinary
---@param algorithm? QuarkKeyAlgorithm
---@return QuarkPublicKey?
---@return string? message
function Crypto.ImportPublicKey(data, algorithm) end

---@param claims table
---@param key string|QuarkPrivateKey
---@param options? { Algorithm?: string, ExpiresIn?: integer, IssuedAt?: boolean, Header?: table }
---@return string
function Crypto.JWTSign(claims, key, options) end

---@param token string
---@param key string|QuarkPublicKey|QuarkPrivateKey
---@param options? { Algorithms?: string[], Leeway?: integer, Issuer?: string, Audience?: string }
---@return table? claims
---@return table|string? header_or_reason
function Crypto.JWTVerify(token, key, options) end

---Decodes without verifying.
---@param token string
---@return table? claims
---@return table|string? header_or_reason
function Crypto.JWTDecode(token) end

---Base32 secret.
---@param bytes? integer
---@return string
function Crypto.TOTPGenerateSecret(bytes) end

---@param secret string
---@param options? { Algorithm?: "SHA1"|"SHA256"|"SHA512", Digits?: integer, Period?: integer, Time?: integer }
---@return string
function Crypto.TOTP(secret, options) end

---@param secret string
---@param code string
---@param options? { Algorithm?: "SHA1"|"SHA256"|"SHA512", Digits?: integer, Period?: integer, Time?: integer, Window?: integer }
---@return boolean
function Crypto.TOTPVerify(secret, code, options) end

---@param secret string
---@param counter integer
---@param options? { Algorithm?: "SHA1"|"SHA256"|"SHA512", Digits?: integer }
---@return string
function Crypto.HOTP(secret, counter, options) end

---otpauth:// URI for QR codes.
---@param secret string
---@param account string
---@param options? { Algorithm?: "SHA1"|"SHA256"|"SHA512", Digits?: integer, Period?: integer, Issuer?: string }
---@return string
function Crypto.TOTPUri(secret, account, options) end

---@param count integer
---@return string
function Crypto.RandomBytes(count) end

---@param minimum integer
---@param maximum integer
---@return integer
function Crypto.RandomInt(minimum, maximum) end

---@param length integer
---@param alphabet? string
---@return string
function Crypto.RandomString(length, alphabet) end

---URL safe Base64 token.
---@param bytes? integer
---@return string
function Crypto.RandomToken(bytes) end

---@param a QuarkBinary
---@param b QuarkBinary
---@return boolean
function Crypto.ConstantTimeEquals(a, b) end

---@param data QuarkBinary
---@return string
function Crypto.HexEncode(data) end

---@param text string
---@return string?
---@return string? message
function Crypto.HexDecode(text) end

---@param data QuarkBinary
---@param url_safe? boolean
---@return string
function Crypto.Base64Encode(data, url_safe) end

---@param text string
---@param url_safe? boolean
---@return string?
---@return string? message
function Crypto.Base64Decode(text, url_safe) end

---@param data QuarkBinary
---@return string
function Crypto.Base32Encode(data) end

---@param text string
---@return string?
---@return string? message
function Crypto.Base32Decode(text) end

---@param data QuarkBinary
---@return string
function Crypto.Base58Encode(data) end

---@param text string
---@return string?
---@return string? message
function Crypto.Base58Decode(text) end
