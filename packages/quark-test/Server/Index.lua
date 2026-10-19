local Crypto = Quark.Import("Crypto")

local passed, failed, pending = 0, 0, 0

local function report(name, ok, message)
    if ok then
        passed = passed + 1
    else
        failed = failed + 1
        Console.Error("FAIL " .. name .. ": " .. tostring(message))
    end
end

local function test(name, body)
    local ok, message = pcall(body)
    report(name, ok, message)
end

local function async_test(name, start)
    pending = pending + 1
    local ok, message = pcall(start, function(condition, message)
        pending = pending - 1
        report(name, condition, message)
        if pending == 0 then
            Console.Log(string.format("Quark async tests finished: %d passed, %d failed", passed, failed))
        end
    end)
    if not ok then
        pending = pending - 1
        report(name, false, message)
    end
end

local function equal(actual, expected)
    if actual ~= expected then
        error("expected " .. tostring(expected) .. ", got " .. tostring(actual), 2)
    end
end

local function raises(body, fragment)
    local ok, message = pcall(body)
    if ok then
        error("expected an error", 2)
    end
    if type(message) ~= "string" then
        error("expected a string error, got " .. type(message), 2)
    end
    if fragment and not message:find(fragment, 1, true) then
        error("expected the error to contain '" .. fragment .. "', got: " .. message, 2)
    end
end

test("modules are lazily importable", function()
    equal(Quark.Crypto, Crypto)
    equal(Quark.DoesNotExist, nil)
    raises(function() Quark.Import("DoesNotExist") end, "Available modules")
end)

test("hash test vectors", function()
    equal(Crypto.MD5("abc"), "900150983cd24fb0d6963f7d28e17f72")
    equal(Crypto.SHA1("abc"), "a9993e364706816aba3e25717850c26c9cd0d89d")
    equal(Crypto.SHA256("abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
    equal(Crypto.SHA512("abc"), "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f")
    equal(Crypto.SHA3_256("abc"), "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532")
    equal(Crypto.BLAKE3("abc"), "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85")
    equal(Crypto.Hash("SHA-256", "abc"), Crypto.SHA256("abc"))
    equal(#Crypto.SHA256("abc", "raw"), 32)
    equal(Crypto.SHA256("abc", "base64"), "ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=")
    raises(function() Crypto.Hash("SHA0", "abc") end, "unsupported hash algorithm")
end)

test("incremental hasher", function()
    local hasher = Crypto.Hasher("BLAKE3")
    hasher:Update("a"):Update("bc")
    equal(hasher:Finalize(), Crypto.BLAKE3("abc"))
    raises(function() hasher:Finalize() end, "already finalized")
    raises(function() hasher:Update({}) end, "bad argument")
end)

test("hmac test vectors", function()
    local message = "The quick brown fox jumps over the lazy dog"
    equal(Crypto.HMAC("SHA256", "key", message), "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8")
    equal(Crypto.HMAC("SHA1", "key", message), "de7c9b85b8b78aa6bc8a7a36f70a90701c9db4d9")
    equal(Crypto.HMACVerify("SHA256", "key", message, "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8"), true)
    equal(Crypto.HMACVerify("SHA256", "key", message, "00"), false)
    local valid, error = Crypto.HMACVerify("SHA256", "key", message, "zz")
    equal(valid, nil)
    assert(error:find("invalid hex"))
end)

test("encodings", function()
    equal(Crypto.HexEncode("abc"), "616263")
    equal(Crypto.HexDecode("616263"), "abc")
    equal(Crypto.Base64Encode("Hello Quark"), "SGVsbG8gUXVhcms=")
    equal(Crypto.Base64Decode("SGVsbG8gUXVhcms="), "Hello Quark")
    equal(Crypto.Base64Decode("SGVsbG8gUXVhcms"), "Hello Quark")
    equal(Crypto.Base32Encode("foobar"), "MZXW6YTBOI")
    equal(Crypto.Base32Decode("mzxw 6ytb oi"), "foobar")
    local decoded, error = Crypto.Base64Decode("!!!")
    equal(decoded, nil)
    assert(error:find("invalid Base64"))
end)

test("hkdf rfc 5869 vector", function()
    local key = Crypto.HKDF(Crypto.HexDecode(string.rep("0b", 22)), {
        Salt = Crypto.HexDecode("000102030405060708090a0b0c"),
        Info = Crypto.HexDecode("f0f1f2f3f4f5f6f7f8f9"),
        Length = 42,
        Format = "hex",
    })
    equal(key, "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865")
end)

test("pbkdf2 vector", function()
    local key = Crypto.PBKDF2("password", "salt", { Iterations = 1, Length = 32, Format = "hex" })
    equal(key, "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b")
    raises(function() Crypto.PBKDF2("password", "salt", { Iterations = 0 }) end, "at least one iteration")
end)

test("one time passwords", function()
    local secret = Crypto.Base32Encode("12345678901234567890")
    equal(Crypto.HOTP(secret, 0), "755224")
    equal(Crypto.HOTP(secret, 9), "520489")
    equal(Crypto.TOTP(secret, { Time = 59, Digits = 8 }), "94287082")
    equal(Crypto.TOTP(secret, { Time = 1111111109, Digits = 8 }), "07081804")
    equal(Crypto.TOTPVerify(secret, "07081804", { Time = 1111111109 + 30, Digits = 8 }), true)
    equal(Crypto.TOTPVerify(secret, "07081804", { Time = 1111111109 + 90, Digits = 8 }), false)
    equal(Crypto.TOTPVerify(secret, 755224, { Time = 0 }), true)
    equal(#Crypto.TOTPGenerateSecret(), 32)
    equal(Crypto.TOTPUri("JBSWY3DPEHPK3PXP", "alice@example.com", { Issuer = "My Server" }),
        "otpauth://totp/My%20Server:alice%40example.com?secret=JBSWY3DPEHPK3PXP&algorithm=SHA1&digits=6&period=30&issuer=My%20Server")
end)

test("authenticated encryption", function()
    for _, cipher in ipairs({ { "AES", 32 }, { "AES", 16 }, { "XChaCha20", 32 } }) do
        local encrypt, decrypt = Crypto[cipher[1] .. "Encrypt"], Crypto[cipher[1] .. "Decrypt"]
        local key = Crypto.RandomBytes(cipher[2])
        local sealed = encrypt(key, "secret message", "context")
        equal(decrypt(key, sealed, "context"), "secret message")
        assert(encrypt(key, "secret message") ~= encrypt(key, "secret message"), "nonces must differ")

        local plaintext, error = decrypt(key, sealed, "other context")
        equal(plaintext, nil)
        assert(error:find("decryption failed"))

        local tampered = sealed:sub(1, -2) .. string.char((sealed:byte(-1) + 1) % 256)
        equal(decrypt(key, tampered, "context"), nil)
        equal(decrypt(Crypto.RandomBytes(cipher[2]), sealed, "context"), nil)
        equal(decrypt(key, "short"), nil)
    end
    raises(function() Crypto.AESEncrypt("short key", "data") end, "16 or 32 bytes")
end)

test("random values", function()
    equal(#Crypto.RandomBytes(64), 64)
    for _ = 1, 1000 do
        local value = Crypto.RandomInt(-3, 3)
        assert(value >= -3 and value <= 3 and math.type(value) == "integer", "out of range: " .. tostring(value))
    end
    equal(Crypto.RandomInt(7, 7), 7)
    equal(#Crypto.RandomString(24), 24)
    assert(Crypto.RandomString(50, "ab"):match("^[ab]+$"))
    equal(#Crypto.RandomToken(), 43)
    equal(Crypto.ConstantTimeEquals("abc", "abc"), true)
    equal(Crypto.ConstantTimeEquals("abc", "abd"), false)
    raises(function() Crypto.RandomInt(5, 1) end, "minimum")
end)

test("ed25519 and ecdsa key pairs", function()
    for _, algorithm in ipairs({ "Ed25519", "ECDSA-P256", "ECDSA-P384" }) do
        local private, public = Crypto.GenerateKeyPair(algorithm)
        equal(private:GetAlgorithm(), algorithm)
        equal(tostring(public), "PublicKey(" .. algorithm .. ")")

        local signature = private:Sign("payload")
        equal(public:Verify("payload", signature), true)
        equal(public:Verify("other payload", signature), false)
        equal(public:Verify("payload", private:Sign("payload", "base64"), "base64"), true)

        local imported_private = Crypto.ImportPrivateKey(private:ExportPEM())
        local imported_public = Crypto.ImportPublicKey(public:ExportPEM())
        equal(imported_public:Verify("payload", imported_private:Sign("payload")), true)

        local raw_private = Crypto.ImportPrivateKey(private:ExportRaw(), algorithm)
        local raw_public = Crypto.ImportPublicKey(public:ExportRaw(), algorithm)
        equal(raw_public:Verify("payload", raw_private:Sign("payload")), true)
    end

    local key, error = Crypto.ImportPrivateKey("-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----")
    equal(key, nil)
    assert(error:find("unsupported"))
end)

test("ecdsa hash restrictions", function()
    local p384 = Crypto.GenerateKeyPair("ECDSA-P384")
    raises(function() p384:Sign("payload", { Hash = "SHA256" }) end, "always use SHA384")
    equal(p384:GetPublicKey():Verify("payload", p384:Sign("payload", { Hash = "SHA384" })), true)
end)

test("elliptic curve key exchange", function()
    for _, algorithm in ipairs({ "ECDSA-P256", "ECDSA-P384" }) do
        local alice = Crypto.GenerateKeyPair(algorithm)
        local bob = Crypto.GenerateKeyPair(algorithm)
        local shared = alice:DeriveSharedSecret(bob:GetPublicKey(), "hex")
        equal(shared, bob:DeriveSharedSecret(alice:GetPublicKey(), "hex"))
    end
    local p256 = Crypto.GenerateKeyPair("ECDSA-P256")
    local _, p384_public = Crypto.GenerateKeyPair("ECDSA-P384")
    raises(function() p256:DeriveSharedSecret(p384_public) end, "same algorithm")
end)

test("sealed boxes", function()
    for _, algorithm in ipairs({ "X25519", "ECDSA-P256", "ECDSA-P384" }) do
        local private, public = Crypto.GenerateKeyPair(algorithm)
        local sealed = public:Seal("secret message", "context")
        equal(private:Open(sealed, "context"), "secret message")
        assert(public:Seal("secret message") ~= public:Seal("secret message"), "sealed boxes must differ")

        equal(private:Open(sealed, "other context"), nil)
        local other = Crypto.GenerateKeyPair(algorithm)
        local opened, error = other:Open(sealed, "context")
        equal(opened, nil)
        assert(error:find("could not be opened"))

        local tampered = sealed:sub(1, -2) .. string.char((sealed:byte(-1) + 1) % 256)
        equal(private:Open(tampered, "context"), nil)
        equal(private:Open("short"), nil)
    end
    local _, ed25519 = Crypto.GenerateKeyPair("Ed25519")
    raises(function() ed25519:Seal("data") end, "sealed boxes need")
    raises(function() ed25519:Encrypt("data") end, "needs an RSA public key")
end)

test("encrypted private keys", function()
    for _, algorithm in ipairs({ "Ed25519", "ECDSA-P256", "ECDSA-P384" }) do
        local private, public = Crypto.GenerateKeyPair(algorithm)
        local pem = private:ExportPEM("correct horse")
        assert(pem:find("BEGIN ENCRYPTED PRIVATE KEY", 1, true))

        local imported = Crypto.ImportPrivateKey(pem, { Password = "correct horse" })
        equal(imported:GetAlgorithm(), algorithm)
        equal(public:Verify("payload", imported:Sign("payload")), true)

        local missing, missing_error = Crypto.ImportPrivateKey(pem)
        equal(missing, nil)
        assert(missing_error:find("pass a Password option"))
        local wrong, wrong_error = Crypto.ImportPrivateKey(pem, { Password = "wrong" })
        equal(wrong, nil)
        assert(wrong_error:find("could not be decrypted"))
    end
end)

test("x25519 key exchange", function()
    local alice = Crypto.GenerateKeyPair("X25519")
    local bob = Crypto.GenerateKeyPair("X25519")
    local shared = alice:DeriveSharedSecret(bob:GetPublicKey())
    equal(#shared, 32)
    equal(shared, bob:DeriveSharedSecret(alice:GetPublicKey()))
    raises(function() alice:Sign("payload") end, "cannot sign")
end)

test("jwt with shared secrets", function()
    local token = Crypto.JWTSign({ sub = "player-1", roles = { "admin" } }, "secret", { ExpiresIn = 60, Header = { kid = "main" } })
    local claims, header = Crypto.JWTVerify(token, "secret")
    equal(claims.sub, "player-1")
    equal(claims.roles[1], "admin")
    equal(header.alg, "HS256")
    equal(header.kid, "main")

    local rejected, reason = Crypto.JWTVerify(token, "wrong secret")
    equal(rejected, nil)
    equal(reason, "invalid token signature")

    local expired = Crypto.JWTSign({ exp = 1 }, "secret")
    equal(select(2, Crypto.JWTVerify(expired, "secret")), "the token has expired")

    local hs512 = Crypto.JWTSign({ iss = "quark" }, "secret", { Algorithm = "HS512" })
    equal(Crypto.JWTVerify(hs512, "secret", { Issuer = "quark" }).iss, "quark")
    equal(select(2, Crypto.JWTVerify(hs512, "secret", { Issuer = "other" })), "the token issuer does not match")
    equal(select(2, Crypto.JWTVerify(hs512, "secret", { Algorithms = { "HS256" } })), "the token algorithm HS512 is not allowed")

    equal(Crypto.JWTDecode(token).sub, "player-1")
    equal(select(2, Crypto.JWTDecode("not a token")), "malformed token, expected three parts")
end)

test("jwt with key pairs and algorithm confusion", function()
    local private, public = Crypto.GenerateKeyPair("Ed25519")
    local token = Crypto.JWTSign({ sub = "player-2" }, private)
    equal(Crypto.JWTVerify(token, public).sub, "player-2")
    equal(select(2, Crypto.JWTVerify(token, "secret")), "the token algorithm EdDSA is not allowed")

    local ecdsa_private, ecdsa_public = Crypto.GenerateKeyPair("ECDSA-P256")
    local ecdsa_token = Crypto.JWTSign({ sub = "player-3" }, ecdsa_private)
    equal(Crypto.JWTVerify(ecdsa_token, ecdsa_public).sub, "player-3")
    equal(select(2, Crypto.JWTVerify(ecdsa_token, public)), "the token algorithm ES256 is not allowed")

    local p384_private, p384_public = Crypto.GenerateKeyPair("ECDSA-P384")
    local p384_token = Crypto.JWTSign({ sub = "player-4" }, p384_private)
    equal(Crypto.JWTDecode(p384_token) and select(2, Crypto.JWTDecode(p384_token)).alg, "ES384")
    equal(Crypto.JWTVerify(p384_token, p384_public).sub, "player-4")

    local forged = Crypto.JWTSign({ sub = "attacker" }, public:ExportPEM(), { Algorithm = "HS256" })
    equal(Crypto.JWTVerify(forged, public), nil)
end)

test("password hashing synchronously", function()
    local argon2 = Crypto.Argon2Hash("hunter2", { MemoryCost = 8192, TimeCost = 1 })
    assert(argon2:find("^%$argon2id%$"))
    equal(Crypto.Argon2Verify("hunter2", argon2), true)
    equal(Crypto.PasswordVerify("wrong", argon2), false)

    local bcrypt = Crypto.BcryptHash("hunter2", { Cost = 4 })
    assert(bcrypt:find("^%$2b%$04%$"))
    equal(Crypto.BcryptVerify("hunter2", bcrypt), true)
    equal(Crypto.PasswordVerify("wrong", bcrypt), false)

    local scrypt = Crypto.ScryptHash("hunter2", { LogN = 10 })
    assert(scrypt:find("^%$scrypt%$"))
    equal(Crypto.ScryptVerify("hunter2", scrypt), true)

    local valid, error = Crypto.BcryptVerify("hunter2", argon2)
    equal(valid, nil)
    equal(error, "expected a bcrypt hash, got a Argon2 hash")
    equal(select(2, Crypto.PasswordVerify("hunter2", "plain text")), "unknown password hash format")
    equal(select(2, Crypto.BcryptHash(string.rep("x", 100), { Cost = 4 })) ~= nil, true)
    raises(function() Crypto.BcryptHash("hunter2", { Cost = 2 }) end, "between 4 and 31")
    raises(function() Crypto.Argon2Hash("hunter2", { Variant = "argon3" }) end, "unknown Argon2 variant")
end)

async_test("argon2 asynchronously", function(done)
    Crypto.Argon2Hash("hunter2", function(hash, error)
        if not hash then return done(false, error) end
        Crypto.PasswordVerify("hunter2", hash, function(valid, verify_error)
            done(valid == true, verify_error)
        end)
    end)
end)

async_test("bcrypt asynchronously", function(done)
    Crypto.BcryptHash("hunter2", { Cost = 4 }, function(hash, error)
        if not hash then return done(false, error) end
        Crypto.BcryptVerify("wrong", hash, function(valid)
            done(valid == false, "expected the wrong password to be rejected")
        end)
    end)
end)

async_test("pbkdf2 asynchronously", function(done)
    Crypto.PBKDF2("password", "salt", { Iterations = 1, Format = "hex" }, function(key)
        done(key == "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b", key)
    end)
end)

async_test("async errors arrive as nil and message", function(done)
    Crypto.PasswordVerify("hunter2", "$argon2id$broken", function(valid, error)
        done(valid == nil and type(error) == "string", error)
    end)
end)

async_test("rsa key pairs", function(done)
    Crypto.GenerateKeyPair("RSA", { Bits = 2048 }, function(private, public)
        if not private then return done(false, public) end

        local pkcs1 = private:Sign("payload")
        local pss = private:Sign("payload", { Padding = "PSS", Hash = "SHA512" })
        local valid = public:Verify("payload", pkcs1)
            and public:Verify("payload", pss, { Padding = "PSS", Hash = "SHA512" })
            and not public:Verify("payload", pss)

        local imported = Crypto.ImportPublicKey(public:ExportPEM())
        local token = Crypto.JWTSign({ sub = "rsa" }, Crypto.ImportPrivateKey(private:ExportPEM()), { Algorithm = "PS256" })
        local claims = Crypto.JWTVerify(token, imported)

        local encrypted_pem = private:ExportPEM("rsa password")
        local reimported = Crypto.ImportPrivateKey(encrypted_pem, { Password = "rsa password" })
        local reimported_valid = reimported and public:Verify("payload", reimported:Sign("payload"))

        local first = public:Encrypt("secret")
        local second = public:Encrypt("secret", { Hash = "SHA512", Label = "context" })
        local encrypted = #first == 256 and #second == 256 and first ~= public:Encrypt("secret")
        local too_long = not pcall(public.Encrypt, public, string.rep("x", 300))

        done(valid and claims and claims.sub == "rsa" and reimported_valid and encrypted and too_long,
            "RSA signing, JWT, encrypted PEM or OAEP failed")
    end)
end)

Console.Log(string.format("Quark sync tests finished: %d passed, %d failed", passed, failed))
