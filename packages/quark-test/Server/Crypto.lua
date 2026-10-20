local Test = Quark.Import("Testing")
local Expect = Test.Expect
local Crypto = Quark.Import("Crypto")

local FOX = "The quick brown fox jumps over the lazy dog"

Test.Describe("Crypto", function()
    Test.Describe("hashing", function()
        Test.It("matches the test vectors", function()
            Expect(Crypto.MD5("abc")):ToBe("900150983cd24fb0d6963f7d28e17f72")
            Expect(Crypto.SHA1("abc")):ToBe("a9993e364706816aba3e25717850c26c9cd0d89d")
            Expect(Crypto.SHA256("abc")):ToBe("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
            Expect(Crypto.SHA512("abc")):ToBe("ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f")
            Expect(Crypto.SHA3_256("abc")):ToBe("3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532")
            Expect(Crypto.BLAKE3("abc")):ToBe("6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85")
        end)

        Test.It("supports algorithm names and output formats", function()
            Expect(Crypto.Hash("SHA-256", "abc")):ToBe(Crypto.SHA256("abc"))
            Expect(Crypto.SHA256("abc", "raw")):ToHaveLength(32)
            Expect(Crypto.SHA256("abc", "base64")):ToBe("ungWv48Bz+pBQUDeXa4iI7ADYaOWF3qctBD/YfIAFa0=")
            Expect(function() Crypto.Hash("SHA0", "abc") end):ToThrow("unsupported hash algorithm")
        end)

        Test.It("hashes incrementally", function()
            local hasher = Crypto.Hasher("BLAKE3")
            hasher:Update("a"):Update("bc")
            Expect(hasher:Finalize()):ToBe(Crypto.BLAKE3("abc"))
            Expect(function() hasher:Finalize() end):ToThrow("already finalized")
            Expect(function() Crypto.Hasher("SHA256"):Update({}) end):ToThrow("bad argument")
        end)
    end)

    Test.Describe("hmac", function()
        Test.It("matches the test vectors", function()
            Expect(Crypto.HMAC("SHA256", "key", FOX)):ToBe("f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8")
            Expect(Crypto.HMAC("SHA1", "key", FOX)):ToBe("de7c9b85b8b78aa6bc8a7a36f70a90701c9db4d9")
        end)

        Test.It("verifies in constant time", function()
            Expect(Crypto.HMACVerify("SHA256", "key", FOX, "f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8")):ToBe(true)
            Expect(Crypto.HMACVerify("SHA256", "key", FOX, "00")):ToBe(false)
            local valid, error_message = Crypto.HMACVerify("SHA256", "key", FOX, "zz")
            Expect(valid):ToBeNil()
            Expect(error_message):ToContain("invalid hex")
        end)
    end)

    Test.It("encodes and decodes", function()
        Expect(Crypto.HexEncode("abc")):ToBe("616263")
        Expect(Crypto.HexDecode("616263")):ToBe("abc")
        Expect(Crypto.Base64Encode("Hello Quark")):ToBe("SGVsbG8gUXVhcms=")
        Expect(Crypto.Base64Decode("SGVsbG8gUXVhcms=")):ToBe("Hello Quark")
        Expect(Crypto.Base64Decode("SGVsbG8gUXVhcms")):ToBe("Hello Quark")
        Expect(Crypto.Base32Encode("foobar")):ToBe("MZXW6YTBOI")
        Expect(Crypto.Base32Decode("mzxw 6ytb oi")):ToBe("foobar")
        Expect(Crypto.Base58Encode("hello world")):ToBe("StV1DL6CwTryKyV")
        Expect(Crypto.Base58Decode("StV1DL6CwTryKyV")):ToBe("hello world")
        Expect(Crypto.Base58Encode("\0\0abc")):ToBe("11ZiCa")
        Expect(select(2, Crypto.Base58Decode("0OIl"))):ToContain("invalid Base58")
        local decoded, error_message = Crypto.Base64Decode("!!!")
        Expect(decoded):ToBeNil()
        Expect(error_message):ToContain("invalid Base64")
    end)

    Test.Describe("key derivation", function()
        Test.It("matches the HKDF RFC 5869 vector", function()
            local key = Crypto.HKDF(Crypto.HexDecode(string.rep("0b", 22)), {
                Salt = Crypto.HexDecode("000102030405060708090a0b0c"),
                Info = Crypto.HexDecode("f0f1f2f3f4f5f6f7f8f9"),
                Length = 42,
                Format = "hex",
            })
            Expect(key):ToBe("3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865")
        end)

        Test.It("matches the PBKDF2 vector synchronously and asynchronously", function()
            local expected = "120fb6cffcf8b32c43e7225256c4f837a86548c92ccc35480805987cb70be17b"
            Expect(Crypto.PBKDF2("password", "salt", { Iterations = 1, Format = "hex" })):ToBe(expected)
            Expect(Quark.Await(Crypto.PBKDF2, "password", "salt", { Iterations = 1, Format = "hex" })):ToBe(expected)
            Expect(function() Crypto.PBKDF2("password", "salt", { Iterations = 0 }) end):ToThrow("at least one iteration")
        end)
    end)

    Test.It("generates one time passwords", function()
        local secret = Crypto.Base32Encode("12345678901234567890")
        Expect(Crypto.HOTP(secret, 0)):ToBe("755224")
        Expect(Crypto.HOTP(secret, 9)):ToBe("520489")
        Expect(Crypto.TOTP(secret, { Time = 59, Digits = 8 })):ToBe("94287082")
        Expect(Crypto.TOTP(secret, { Time = 1111111109, Digits = 8 })):ToBe("07081804")
        Expect(Crypto.TOTPVerify(secret, "07081804", { Time = 1111111109 + 30, Digits = 8 })):ToBe(true)
        Expect(Crypto.TOTPVerify(secret, "07081804", { Time = 1111111109 + 90, Digits = 8 })):ToBe(false)
        Expect(Crypto.TOTPVerify(secret, 755224, { Time = 0 })):ToBe(true)
        Expect(Crypto.TOTPGenerateSecret()):ToHaveLength(32)
        Expect(Crypto.TOTPUri("JBSWY3DPEHPK3PXP", "alice@example.com", { Issuer = "My Server" }))
            :ToBe("otpauth://totp/My%20Server:alice%40example.com?secret=JBSWY3DPEHPK3PXP&algorithm=SHA1&digits=6&period=30&issuer=My%20Server")
    end)

    Test.It("encrypts with authentication", function()
        for _, cipher in ipairs({ { "AES", 32 }, { "AES", 16 }, { "XChaCha20", 32 } }) do
            local encrypt, decrypt = Crypto[cipher[1] .. "Encrypt"], Crypto[cipher[1] .. "Decrypt"]
            local key = Crypto.RandomBytes(cipher[2])
            local sealed = encrypt(key, "secret message", "context")
            Expect(decrypt(key, sealed, "context")):ToBe("secret message")
            Expect(encrypt(key, "secret message")).Not:ToBe(encrypt(key, "secret message"))

            local plaintext, error_message = decrypt(key, sealed, "other context")
            Expect(plaintext):ToBeNil()
            Expect(error_message):ToContain("decryption failed")

            local tampered = sealed:sub(1, -2) .. string.char((sealed:byte(-1) + 1) % 256)
            Expect(decrypt(key, tampered, "context")):ToBeNil()
            Expect(decrypt(Crypto.RandomBytes(cipher[2]), sealed, "context")):ToBeNil()
            Expect(decrypt(key, "short")):ToBeNil()
        end
        Expect(function() Crypto.AESEncrypt("short key", "data") end):ToThrow("16 or 32 bytes")
    end)

    Test.It("generates secure random values", function()
        Expect(Crypto.RandomBytes(64)):ToHaveLength(64)
        for _ = 1, 1000 do
            local value = Crypto.RandomInt(-3, 3)
            Expect(value):ToBeType("integer")
            Expect(value):ToBeGreaterThanOrEqual(-3)
            Expect(value):ToBeLessThanOrEqual(3)
        end
        Expect(Crypto.RandomInt(7, 7)):ToBe(7)
        Expect(Crypto.RandomString(24)):ToHaveLength(24)
        Expect(Crypto.RandomString(50, "ab")):ToMatch("^[ab]+$")
        Expect(Crypto.RandomToken()):ToHaveLength(43)
        Expect(Crypto.ConstantTimeEquals("abc", "abc")):ToBe(true)
        Expect(Crypto.ConstantTimeEquals("abc", "abd")):ToBe(false)
        Expect(function() Crypto.RandomInt(5, 1) end):ToThrow("minimum")
    end)

    Test.Describe("key pairs", function()
        Test.It("sign, verify, export and import", function()
            for _, algorithm in ipairs({ "Ed25519", "ECDSA-P256", "ECDSA-P384" }) do
                local private, public = Crypto.GenerateKeyPair(algorithm)
                Expect(private:GetAlgorithm()):ToBe(algorithm)
                Expect(tostring(public)):ToBe("PublicKey(" .. algorithm .. ")")

                local signature = private:Sign("payload")
                Expect(public:Verify("payload", signature)):ToBe(true)
                Expect(public:Verify("other payload", signature)):ToBe(false)
                Expect(public:Verify("payload", private:Sign("payload", "base64"), "base64")):ToBe(true)

                local imported_private = Crypto.ImportPrivateKey(private:ExportPEM())
                local imported_public = Crypto.ImportPublicKey(public:ExportPEM())
                Expect(imported_public:Verify("payload", imported_private:Sign("payload"))):ToBe(true)

                local raw_private = Crypto.ImportPrivateKey(private:ExportRaw(), algorithm)
                local raw_public = Crypto.ImportPublicKey(public:ExportRaw(), algorithm)
                Expect(raw_public:Verify("payload", raw_private:Sign("payload"))):ToBe(true)
            end

            local key, error_message = Crypto.ImportPrivateKey("-----BEGIN PRIVATE KEY-----\nAAAA\n-----END PRIVATE KEY-----")
            Expect(key):ToBeNil()
            Expect(error_message):ToContain("unsupported")
        end)

        Test.It("enforces the curve hash", function()
            local p384 = Crypto.GenerateKeyPair("ECDSA-P384")
            Expect(function() p384:Sign("payload", { Hash = "SHA256" }) end):ToThrow("always use SHA384")
            Expect(p384:GetPublicKey():Verify("payload", p384:Sign("payload", { Hash = "SHA384" }))):ToBe(true)
        end)

        Test.It("exchanges keys", function()
            for _, algorithm in ipairs({ "X25519", "ECDSA-P256", "ECDSA-P384" }) do
                local alice = Crypto.GenerateKeyPair(algorithm)
                local bob = Crypto.GenerateKeyPair(algorithm)
                Expect(alice:DeriveSharedSecret(bob:GetPublicKey(), "hex"))
                    :ToBe(bob:DeriveSharedSecret(alice:GetPublicKey(), "hex"))
            end
            local p256 = Crypto.GenerateKeyPair("ECDSA-P256")
            local _, p384_public = Crypto.GenerateKeyPair("ECDSA-P384")
            Expect(function() p256:DeriveSharedSecret(p384_public) end):ToThrow("same algorithm")
            Expect(function() Crypto.GenerateKeyPair("X25519"):Sign("payload") end):ToThrow("cannot sign")
        end)

        Test.It("seals and opens boxes", function()
            for _, algorithm in ipairs({ "X25519", "ECDSA-P256", "ECDSA-P384" }) do
                local private, public = Crypto.GenerateKeyPair(algorithm)
                local sealed = public:Seal("secret message", "context")
                Expect(private:Open(sealed, "context")):ToBe("secret message")
                Expect(public:Seal("secret message")).Not:ToBe(public:Seal("secret message"))
                Expect(private:Open(sealed, "other context")):ToBeNil()

                local opened, error_message = Crypto.GenerateKeyPair(algorithm):Open(sealed, "context")
                Expect(opened):ToBeNil()
                Expect(error_message):ToContain("could not be opened")

                local tampered = sealed:sub(1, -2) .. string.char((sealed:byte(-1) + 1) % 256)
                Expect(private:Open(tampered, "context")):ToBeNil()
                Expect(private:Open("short")):ToBeNil()
            end
            local _, ed25519 = Crypto.GenerateKeyPair("Ed25519")
            Expect(function() ed25519:Seal("data") end):ToThrow("sealed boxes need")
            Expect(function() ed25519:Encrypt("data") end):ToThrow("needs an RSA public key")
        end)

        Test.It("encrypts private keys with a password", function()
            for _, algorithm in ipairs({ "Ed25519", "ECDSA-P256", "ECDSA-P384" }) do
                local private, public = Crypto.GenerateKeyPair(algorithm)
                local pem = private:ExportPEM("correct horse")
                Expect(pem):ToContain("BEGIN ENCRYPTED PRIVATE KEY")

                local imported = Crypto.ImportPrivateKey(pem, { Password = "correct horse" })
                Expect(imported:GetAlgorithm()):ToBe(algorithm)
                Expect(public:Verify("payload", imported:Sign("payload"))):ToBe(true)

                Expect(select(2, Crypto.ImportPrivateKey(pem))):ToContain("pass a Password option")
                Expect(select(2, Crypto.ImportPrivateKey(pem, { Password = "wrong" }))):ToContain("could not be decrypted")
            end
        end)

        Test.It("generates and uses RSA keys", function()
            local private, public = Quark.Await(Crypto.GenerateKeyPair, "RSA", { Bits = 2048 })

            local pss = private:Sign("payload", { Padding = "PSS", Hash = "SHA512" })
            Expect(public:Verify("payload", private:Sign("payload"))):ToBe(true)
            Expect(public:Verify("payload", pss, { Padding = "PSS", Hash = "SHA512" })):ToBe(true)
            Expect(public:Verify("payload", pss)):ToBe(false)

            local token = Crypto.JWTSign({ sub = "rsa" }, Crypto.ImportPrivateKey(private:ExportPEM()), { Algorithm = "PS256" })
            Expect(Crypto.JWTVerify(token, Crypto.ImportPublicKey(public:ExportPEM())).sub):ToBe("rsa")

            local reimported = Crypto.ImportPrivateKey(private:ExportPEM("rsa password"), { Password = "rsa password" })
            Expect(public:Verify("payload", reimported:Sign("payload"))):ToBe(true)

            local ciphertext = public:Encrypt("secret")
            Expect(ciphertext):ToHaveLength(256)
            Expect(public:Encrypt("secret", { Hash = "SHA512", Label = "context" })):ToHaveLength(256)
            Expect(ciphertext).Not:ToBe(public:Encrypt("secret"))
            Expect(function() public:Encrypt(string.rep("x", 300)) end):ToThrow("RSA encryption failed")
        end, { Timeout = 30000 })
    end)

    Test.Describe("jwt", function()
        Test.It("signs and verifies with shared secrets", function()
            local token = Crypto.JWTSign({ sub = "player-1", roles = { "admin" } }, "secret", { ExpiresIn = 60, Header = { kid = "main" } })
            local claims, header = Crypto.JWTVerify(token, "secret")
            Expect(claims.sub):ToBe("player-1")
            Expect(claims.roles):ToEqual({ "admin" })
            Expect(header.alg):ToBe("HS256")
            Expect(header.kid):ToBe("main")
            Expect(Crypto.JWTDecode(token).sub):ToBe("player-1")
        end)

        Test.It("rejects invalid tokens", function()
            local token = Crypto.JWTSign({ sub = "player-1" }, "secret")
            Expect(select(2, Crypto.JWTVerify(token, "wrong secret"))):ToBe("invalid token signature")
            Expect(select(2, Crypto.JWTVerify(Crypto.JWTSign({ exp = 1 }, "secret"), "secret"))):ToBe("the token has expired")

            local hs512 = Crypto.JWTSign({ iss = "quark" }, "secret", { Algorithm = "HS512" })
            Expect(Crypto.JWTVerify(hs512, "secret", { Issuer = "quark" }).iss):ToBe("quark")
            Expect(select(2, Crypto.JWTVerify(hs512, "secret", { Issuer = "other" }))):ToBe("the token issuer does not match")
            Expect(select(2, Crypto.JWTVerify(hs512, "secret", { Algorithms = { "HS256" } }))):ToBe("the token algorithm HS512 is not allowed")
            Expect(select(2, Crypto.JWTDecode("not a token"))):ToBe("malformed token, expected three parts")
        end)

        Test.It("signs with key pairs and prevents algorithm confusion", function()
            local private, public = Crypto.GenerateKeyPair("Ed25519")
            local token = Crypto.JWTSign({ sub = "player-2" }, private)
            Expect(Crypto.JWTVerify(token, public).sub):ToBe("player-2")
            Expect(select(2, Crypto.JWTVerify(token, "secret"))):ToBe("the token algorithm EdDSA is not allowed")

            for _, case in ipairs({ { "ECDSA-P256", "ES256" }, { "ECDSA-P384", "ES384" } }) do
                local ecdsa_private, ecdsa_public = Crypto.GenerateKeyPair(case[1])
                local ecdsa_token = Crypto.JWTSign({ sub = case[2] }, ecdsa_private)
                Expect(select(2, Crypto.JWTDecode(ecdsa_token)).alg):ToBe(case[2])
                Expect(Crypto.JWTVerify(ecdsa_token, ecdsa_public).sub):ToBe(case[2])
                Expect(select(2, Crypto.JWTVerify(ecdsa_token, public))):ToBe("the token algorithm " .. case[2] .. " is not allowed")
            end

            local forged = Crypto.JWTSign({ sub = "attacker" }, public:ExportPEM(), { Algorithm = "HS256" })
            Expect(Crypto.JWTVerify(forged, public)):ToBeNil()
        end)
    end)

    Test.Describe("passwords", function()
        Test.It("hashes and verifies synchronously", function()
            local argon2 = Crypto.Argon2Hash("hunter2", { MemoryCost = 8192, TimeCost = 1 })
            Expect(argon2):ToMatch("^%$argon2id%$")
            Expect(Crypto.Argon2Verify("hunter2", argon2)):ToBe(true)
            Expect(Crypto.PasswordVerify("wrong", argon2)):ToBe(false)

            local bcrypt = Crypto.BcryptHash("hunter2", { Cost = 4 })
            Expect(bcrypt):ToMatch("^%$2b%$04%$")
            Expect(Crypto.BcryptVerify("hunter2", bcrypt)):ToBe(true)
            Expect(Crypto.PasswordVerify("wrong", bcrypt)):ToBe(false)

            local scrypt = Crypto.ScryptHash("hunter2", { LogN = 10 })
            Expect(scrypt):ToMatch("^%$scrypt%$")
            Expect(Crypto.ScryptVerify("hunter2", scrypt)):ToBe(true)
        end)

        Test.It("hashes and verifies asynchronously", function()
            local hash = Quark.Await(Crypto.Argon2Hash, "hunter2")
            Expect(Quark.Await(Crypto.PasswordVerify, "hunter2", hash)):ToBe(true)
            Expect(Quark.Await(Crypto.PasswordVerify, "wrong", hash)):ToBe(false)

            local bcrypt = Quark.Await(Crypto.BcryptHash, "hunter2", { Cost = 4 })
            Expect(Quark.Await(Crypto.BcryptVerify, "wrong", bcrypt)):ToBe(false)
        end)

        Test.It("reports invalid input", function()
            local argon2 = Crypto.Argon2Hash("hunter2", { MemoryCost = 8192, TimeCost = 1 })
            Expect(select(2, Crypto.BcryptVerify("hunter2", argon2))):ToBe("expected a bcrypt hash, got a Argon2 hash")
            Expect(select(2, Crypto.PasswordVerify("hunter2", "plain text"))):ToBe("unknown password hash format")
            Expect(select(2, Crypto.BcryptHash(string.rep("x", 100), { Cost = 4 }))).Not:ToBeNil()
            Expect(function() Quark.Await(Crypto.PasswordVerify, "hunter2", "$argon2id$broken") end):ToThrow()
            Expect(function() Crypto.BcryptHash("hunter2", { Cost = 2 }) end):ToThrow("between 4 and 31")
            Expect(function() Crypto.Argon2Hash("hunter2", { Variant = "argon3" }) end):ToThrow("unknown Argon2 variant")
        end)
    end)
end)
