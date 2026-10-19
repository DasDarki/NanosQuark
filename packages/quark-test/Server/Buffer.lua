local Test = Quark.Import("Testing")
local Expect = Test.Expect
local Buffer = Quark.Buffer

Test.Describe("Buffer", function()
    Test.It("creates buffers", function()
        Expect(Buffer.New():GetSize()):ToBe(0)
        Expect(#Buffer.New(4)):ToBe(4)
        Expect(Buffer.New(3, 0xff):ToHex()):ToBe("ffffff")
        Expect(Buffer.FromString("abc"):GetBytes()):ToBe("abc")
        Expect(Buffer.FromArray("u16be", { 1, 2 }):ToHex()):ToBe("00010002")
        Expect(Buffer.Concat({ "ab", Buffer.FromString("cd") }):GetBytes()):ToBe("abcd")
        Expect(Buffer.IsBuffer(Buffer.New())):ToBe(true)
        Expect(Buffer.IsBuffer("string")):ToBe(false)
        Expect(tostring(Buffer.New(2))):ToBe("Buffer(2 bytes)")
    end)

    Test.It("reads and writes every number type in both byte orders", function()
        local cases = {
            { "u8", 255 }, { "i8", -128 }, { "u16", 65535 }, { "i16", -32768 },
            { "u32", 4294967295 }, { "i32", -2147483648 }, { "i64", math.mininteger },
            { "u64", 9007199254740993 }, { "f64", 3.141592653589793 },
        }
        for _, case in ipairs(cases) do
            for _, suffix in ipairs({ "", "le", "be" }) do
                local buffer = Buffer.New()
                buffer:Write(case[1] .. suffix, 0, case[2])
                Expect(buffer:Read(case[1] .. suffix, 0)):ToBe(case[2])
            end
        end
        local float = Buffer.New()
        float:Write("f32", 0, 1.5)
        Expect(float:Read("f32", 0)):ToBe(1.5)
        Expect(float:GetSize()):ToBe(4)
    end)

    Test.It("uses the requested byte order", function()
        local buffer = Buffer.New()
        buffer:Write("u32be", 0, 0x01020304)
        buffer:Write("u32le", 4, 0x01020304)
        Expect(buffer:ToHex()):ToBe("0102030404030201")
    end)

    Test.It("rejects values outside of the number type", function()
        local buffer = Buffer.New()
        Expect(function() buffer:Write("u8", 0, 256) end):ToThrow("does not fit into u8")
        Expect(function() buffer:Write("i16", 0, 40000) end):ToThrow("does not fit into i16")
        Expect(function() buffer:Write("u32", 0, -1) end):ToThrow("does not fit into u32")
        Expect(function() buffer:Write("i32", 0, 1.5) end):ToThrow("fraction")
        Expect(function() buffer:Write("u24", 0, 1) end):ToThrow("unknown number type")
    end)

    Test.It("rejects reads outside of the buffer", function()
        local buffer = Buffer.New(3)
        Expect(function() buffer:Read("u32", 0) end):ToThrow("cannot read 4 bytes at offset 0")
        Expect(function() buffer:ReadString(2, 5) end):ToThrow("cannot read")
        Expect(function() buffer:Slice(2, 1) end):ToThrow("outside of the buffer")
    end)

    Test.It("grows when writing past the end", function()
        local buffer = Buffer.New(1)
        buffer:Write("u16be", 3, 0xabcd)
        Expect(buffer:ToHex()):ToBe("000000abcd")
        buffer:WriteString(6, "x")
        Expect(buffer:GetSize()):ToBe(7)
    end)

    Test.It("appends values in a chain", function()
        local packet = Buffer.New():Append("u8", 1):Append("u16be", 2):AppendString("hi")
        Expect(packet:ToHex()):ToBe("0100026869")
        Expect(packet:ReadString(3)):ToBe("hi")
    end)

    Test.It("slices, copies, fills and clones", function()
        local buffer = Buffer.FromString("abcdef")
        Expect(buffer:Slice(1, 3):GetBytes()):ToBe("bc")
        Expect(buffer:GetBytes(4)):ToBe("ef")

        local target = Buffer.New(4, 0x2e)
        target:CopyFrom(buffer, 1, 2, 4)
        Expect(target:GetBytes()):ToBe(".cd.")

        target:Fill(0x41, 0, 2)
        Expect(target:GetBytes()):ToBe("AAd.")

        local clone = target:Clone()
        clone:Fill(0)
        Expect(target:GetBytes()):ToBe("AAd.")
    end)

    Test.It("reads and writes arrays", function()
        local buffer = Buffer.New()
        buffer:WriteArray("i16", 0, { -1, 2, 300 })
        Expect(buffer:ReadArray("i16", 0, 3)):ToEqual({ -1, 2, 300 })
        Expect(buffer:ReadArray("u8", 0, 2)):ToEqual({ 255, 255 })
    end)

    Test.It("compares buffers", function()
        Expect(Buffer.FromString("ab") == Buffer.FromString("ab")):ToBe(true)
        Expect(Buffer.FromString("ab") == Buffer.FromString("ac")):ToBe(false)
        Expect(Buffer.FromString("ab"):Equals("ab")):ToBe(true)
    end)

    Test.It("is accepted as binary input by other modules", function()
        local data = Buffer.FromString("abc")
        Expect(Quark.Crypto.SHA256(data)):ToBe(Quark.Crypto.SHA256("abc"))
        Expect(Quark.Crypto.Base64Encode(data)):ToBe("YWJj")
        local key = Buffer.FromString(Quark.Crypto.RandomBytes(32))
        Expect(Quark.Crypto.AESDecrypt(key, Quark.Crypto.AESEncrypt(key, data))):ToBe("abc")
    end)
end)
