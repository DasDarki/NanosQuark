local Test = Quark.Import("Testing")
local Expect = Test.Expect

local Compression = Quark.Compression
local SAMPLE = string.rep("Quark compresses repetitive text very well. ", 200)

Test.Describe("Compression", function()
    Test.It("round trips every algorithm", function()
        for _, algorithm in ipairs(Compression.Algorithms()) do
            local compressed = Compression.Compress(algorithm, SAMPLE)
            Expect(#compressed):ToBeLessThan(#SAMPLE / 4)
            Expect(Compression.Decompress(algorithm, compressed)):ToBe(SAMPLE)
            Expect(Compression.Decompress(algorithm, Compression.Compress(algorithm, ""))):ToBe("")
        end
    end)

    Test.It("accepts levels", function()
        local fast = Compression.Compress("zstd", SAMPLE, { Level = 1 })
        local small = Compression.Compress("zstd", SAMPLE, { Level = 19 })
        Expect(Compression.Decompress("zstd", small)):ToBe(SAMPLE)
        Expect(#small):ToBeLessThanOrEqual(#fast)
        Expect(function() Compression.Compress("gzip", SAMPLE, { Level = 10 }) end):ToThrow("between 0 and 9")
        Expect(function() Compression.Compress("lz4", SAMPLE, { Level = 1 }) end):ToThrow("no compression levels")
        Expect(function() Compression.Compress("rar", SAMPLE) end):ToThrow("unknown compression algorithm")
    end)

    Test.It("detects formats", function()
        for _, algorithm in ipairs({ "gzip", "zstd", "lz4", "zlib" }) do
            local compressed = Compression.Compress(algorithm, SAMPLE)
            Expect(Compression.Detect(compressed)):ToBe(algorithm)
            Expect(Compression.Decompress("auto", compressed)):ToBe(SAMPLE)
        end
        Expect(Compression.Detect("plain text")):ToBeNil()
        Expect(select(2, Compression.Decompress("auto", "plain text"))):ToContain("could not be detected")
    end)

    Test.It("rejects damaged data and decompression bombs", function()
        local compressed = Compression.Compress("gzip", SAMPLE)
        Expect(select(2, Compression.Decompress("gzip", compressed:sub(1, 20)))):ToContain("gzip decompression failed")
        Expect(select(2, Compression.Decompress("zstd", "garbage"))):ToContain("zstd decompression failed")

        local bomb = Compression.Compress("zstd", string.rep("\0", 1024 * 1024))
        Expect(#bomb):ToBeLessThan(1024)
        local data, error_message = Compression.Decompress("zstd", bomb, { MaxSize = 1000 })
        Expect(data):ToBeNil()
        Expect(error_message):ToContain("larger than the limit of 1000 bytes")
    end)

    Test.It("works with buffers and asynchronously", function()
        local compressed = Quark.Await(Compression.Compress, "brotli", Quark.Buffer.FromString(SAMPLE))
        Expect(Quark.Await(Compression.Decompress, "brotli", compressed)):ToBe(SAMPLE)
    end)
end)

Test.Describe("Zip", function()
    local Zip = Quark.Zip

    Test.It("creates and extracts archives", function()
        local archive = Zip.Create({
            { Name = "readme.txt", Data = "hello" },
            { Name = "data/", IsDirectory = true },
            { Name = "data/sample.txt", Data = SAMPLE },
        })
        Expect(archive:sub(1, 2)):ToBe("PK")

        local entries = Zip.Extract(archive)
        Expect(#entries):ToBe(3)
        Expect(entries[1].Name):ToBe("readme.txt")
        Expect(entries[1].Data):ToBe("hello")
        Expect(entries[2].IsDirectory):ToBe(true)
        Expect(entries[3].Data):ToBe(SAMPLE)
        Expect(entries[3].CompressedSize):ToBeLessThan(entries[3].Size)

        local listed = Zip.List(archive)
        Expect(listed[3].Data):ToBeNil()
        Expect(listed[3].Size):ToBe(#SAMPLE)
    end)

    Test.It("accepts a name to data map and stored entries", function()
        local archive = Zip.Create({ ["b.txt"] = "B", ["a.txt"] = Quark.Buffer.FromString("A") }, { Method = "store" })
        local entries = Zip.Extract(archive)
        Expect(entries[1].Name):ToBe("a.txt")
        Expect(entries[2].Data):ToBe("B")
        Expect(entries[2].CompressedSize):ToBe(1)
    end)

    Test.It("marks unsafe paths", function()
        local entries = Zip.List(Zip.Create({
            { Name = "../escape.txt", Data = "x" },
            { Name = "nested/./file.txt", Data = "y" },
        }))
        Expect(entries[1].SafePath):ToBeNil()
        Expect(entries[2].SafePath):ToBe("nested/file.txt")
    end)

    Test.It("rejects invalid input and oversized content", function()
        Expect(select(2, Zip.Extract("not a zip"))):ToContain("invalid ZIP archive")
        local archive = Zip.Create({ { Name = "big.bin", Data = string.rep("\0", 100000) } })
        Expect(select(2, Zip.Extract(archive, { MaxSize = 1000 }))):ToContain("larger than the limit")
        Expect(function() Zip.Create({ { Data = "x" } }) end):ToThrow("needs a Name")
    end)

    Test.It("works asynchronously", function()
        local archive = Quark.Await(Zip.Create, { ["file.txt"] = SAMPLE })
        Expect(Quark.Await(Zip.Extract, archive)[1].Data):ToBe(SAMPLE)
    end)
end)

Test.Describe("Tar", function()
    local Tar = Quark.Tar

    Test.It("creates and extracts plain and compressed archives", function()
        for _, compression in ipairs({ false, "gzip", "zstd" }) do
            local archive = Tar.Create({
                { Name = "config/", IsDirectory = true },
                { Name = "config/server.toml", Data = "port = 7777" },
                { Name = "big.txt", Data = SAMPLE },
            }, { Compression = compression or nil })

            if compression then
                Expect(Compression.Detect(archive)):ToBe(compression)
            end

            local entries = Tar.Extract(archive)
            Expect(#entries):ToBe(3)
            Expect(entries[1].IsDirectory):ToBe(true)
            Expect(entries[2].Name):ToBe("config/server.toml")
            Expect(entries[2].Data):ToBe("port = 7777")
            Expect(entries[3].Data):ToBe(SAMPLE)
            Expect(Tar.List(archive)[3].Size):ToBe(#SAMPLE)
        end
    end)

    Test.It("limits the extracted size", function()
        local archive = Tar.Create({ ["big.bin"] = string.rep("\0", 100000) }, { Compression = "gzip" })
        Expect(select(2, Tar.Extract(archive, { MaxSize = 1000 }))):ToContain("larger than the limit")
    end)
end)

Test.Describe("Compression streams", function()
    local Compression = Quark.Compression
    local SAMPLE = string.rep("streaming data for quark ", 2000)

    local function chunks(data, size)
        local list = {}
        for index = 1, #data, size do
            list[#list + 1] = data:sub(index, index + size - 1)
        end
        return list
    end

    Test.It("compresses and decompresses in chunks", function()
        for _, algorithm in ipairs(Compression.Algorithms()) do
            local compressor = Compression.Stream(algorithm, { Mode = "Compress" })
            local parts = {}
            for _, chunk in ipairs(chunks(SAMPLE, 1000)) do
                parts[#parts + 1] = compressor:Write(chunk)
            end
            parts[#parts + 1] = compressor:Finish()
            local packed = table.concat(parts)
            Expect(compressor:GetTotalIn()):ToBe(#SAMPLE)
            Expect(compressor:GetTotalOut()):ToBe(#packed)
            Expect(compressor:IsFinished()):ToBe(true)
            Expect(Compression.Decompress(algorithm, packed)):ToBe(SAMPLE)

            local decompressor = Compression.Stream(algorithm, { Mode = "Decompress" })
            local output = {}
            for _, chunk in ipairs(chunks(packed, 97)) do
                output[#output + 1] = decompressor:Write(chunk)
            end
            output[#output + 1] = decompressor:Finish()
            Expect(table.concat(output)):ToBe(SAMPLE)
        end
    end)

    Test.It("enforces limits and reports errors", function()
        local packed = Compression.Compress("gzip", SAMPLE)
        local limited = Compression.Stream("gzip", { Mode = "Decompress", MaxSize = 1000 })
        Expect(function() limited:Write(packed) end):ToThrow("larger than the limit")
        Expect(function() limited:Write("x") end):ToThrow("already finished")

        local broken = Compression.Stream("zstd", { Mode = "Decompress" })
        Expect(function()
            broken:Write("definitely not zstd")
            broken:Finish()
        end):ToThrow("zstd decompression failed")

        local stream = Compression.Stream("brotli", { Level = 9 })
        stream:Write("abc")
        Expect(stream:Flush()):ToBeType("string")
        stream:Finish()
        Expect(function() stream:Finish() end):ToThrow("already finished")
        Expect(function() Compression.Stream("gzip", { Mode = "Sideways" }) end):ToThrow("unknown mode")
    end)

    Test.It("compresses files", function()
        local directory = os.getenv("QUARK_TEST_TMP") or "quark-test-logs"
        local base = directory .. "/compress-" .. tostring(math.floor(Quark.Clock() * 1000000))
        local file = assert(io.open(base .. ".txt", "wb"))
        file:write(SAMPLE)
        file:close()

        local sizes = Compression.CompressFile("zstd", base .. ".txt", base .. ".zst", { Level = 5 })
        Expect(sizes.Input):ToBe(#SAMPLE)
        Expect(sizes.Output < #SAMPLE):ToBe(true)
        local restored = Quark.Await(Compression.DecompressFile, "auto", base .. ".zst", base .. ".out")
        Expect(restored.Output):ToBe(#SAMPLE)
        local handle = assert(io.open(base .. ".out", "rb"))
        Expect(handle:read("a")):ToBe(SAMPLE)
        handle:close()

        Compression.CompressFile("brotli", base .. ".txt", base .. ".br")
        local result, message = Compression.DecompressFile("brotli", base .. ".br", base .. ".big", { MaxSize = 100 })
        Expect(result):ToBeNil()
        Expect(message):ToContain("larger than the limit")
        Expect(io.open(base .. ".big", "rb")):ToBeNil()

        local missing, missing_message = Compression.CompressFile("gzip", base .. ".missing", base .. ".gz")
        Expect(missing):ToBeNil()
        Expect(missing_message):ToContain("cannot open")
        for _, suffix in ipairs({ ".txt", ".zst", ".out", ".br" }) do
            os.remove(base .. suffix)
        end
    end)
end)

Test.Describe("Zip encryption", function()
    local Zip = Quark.Zip

    Test.It("encrypts entries with AES", function()
        local archive = Zip.Create({ ["secret.txt"] = "top secret", ["folder/"] = "" }, { Password = "hunter2" })
        Expect(archive:find("top secret", 1, true)):ToBeNil()

        local listed = Zip.List(archive)
        local secret
        for _, entry in ipairs(listed) do
            if entry.Name == "secret.txt" then
                secret = entry
            end
        end
        Expect(secret.IsEncrypted):ToBe(true)
        Expect(secret.Size):ToBe(10)

        local entries = Zip.Extract(archive, { Password = "hunter2" })
        local found
        for _, entry in ipairs(entries) do
            if entry.Name == "secret.txt" then
                found = entry.Data
            end
        end
        Expect(found):ToBe("top secret")

        local wrong, wrong_message = Zip.Extract(archive, { Password = "wrong" })
        Expect(wrong):ToBeNil()
        Expect(wrong_message):ToContain("password")
        local missing, missing_message = Zip.Extract(archive)
        Expect(missing):ToBeNil()
        Expect(missing_message):ToContain("Password option")

        local small = Zip.Create({ ["a.txt"] = "a" }, { Password = "x", Encryption = "AES128" })
        Expect(Zip.Extract(small, { Password = "x" })[1].Data):ToBe("a")
        Expect(Zip.Extract(Zip.Create({ ["plain.txt"] = "plain" }), { Password = "unused" })[1].Data):ToBe("plain")
        Expect(function() Zip.Create({ a = "a" }, { Password = "x", Encryption = "DES" }) end):ToThrow("unknown ZIP encryption")
    end)
end)
