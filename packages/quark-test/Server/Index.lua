local Crypto = Quark.Import("Crypto")

Console.Log("Quark " .. Quark.Version .. ", modules: " .. table.concat(Quark.Modules(), ", "))
Console.Log("Lazy access returns the imported table: " .. tostring(Quark.Crypto == Crypto))
Console.Log("Unknown lazy access: " .. tostring(Quark.DoesNotExist))

local _, import_error = pcall(Quark.Import, "DoesNotExist")
Console.Log("Unknown import: " .. tostring(import_error))

Console.Log("SHA256('abc') = " .. Crypto.SHA256("abc"))
Console.Log("SHA512('abc') = " .. Crypto.SHA512("abc"))

local encoded = Crypto.Base64Encode("Hello Quark")
Console.Log("Base64: " .. encoded .. " -> " .. Crypto.Base64Decode(encoded))
Console.Log("Base64 URL safe: " .. Crypto.Base64Encode(Crypto.RandomBytes(16), true))
Console.Log("RandomBytes(32) length: " .. #Crypto.RandomBytes(32))

Crypto.Argon2Hash("hunter2", function(hash, hash_error)
    Console.Log("Argon2Hash: " .. tostring(hash) .. " (error: " .. tostring(hash_error) .. ")")

    Crypto.Argon2Verify("hunter2", hash, function(valid)
        Console.Log("Argon2Verify correct password: " .. tostring(valid))
    end)

    Crypto.Argon2Verify("wrong", hash, function(valid)
        Console.Log("Argon2Verify wrong password: " .. tostring(valid))
    end)

    Crypto.Argon2Verify("hunter2", "not a hash", function(valid, verify_error)
        Console.Log("Argon2Verify malformed hash: " .. tostring(valid) .. " (error: " .. tostring(verify_error) .. ")")
    end)

    Crypto.Argon2Verify("hunter2", hash, function()
        error("callback errors are reported without breaking the pump")
    end)
end)

Console.Log("Argon2Hash scheduled, the game thread keeps running")

local function expect_error(label, f, ...)
    local ok, message = pcall(f, ...)
    Console.Log(label .. ": failed=" .. tostring(not ok) .. ", message type=" .. type(message) .. " -> " .. tostring(message))
end

expect_error("Bad argument", Crypto.SHA256, {})
expect_error("Invalid Base64", Crypto.Base64Decode, "!!!")
expect_error("Unsupported algorithm", Crypto.Hasher, "md4")

local hasher = Crypto.Hasher("SHA256")
hasher:Update("a")
hasher:Update("bc")
Console.Log("Hasher('abc') = " .. hasher:Finalize())
expect_error("Finalize twice", hasher.Finalize, hasher)
expect_error("Bad method argument", hasher.Update, hasher, {})
