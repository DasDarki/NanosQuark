local Test = Quark.Import("Testing")
local Expect = Test.Expect

Test.Describe("JSON", function()
    local JSON = Quark.JSON

    Test.It("encodes values with sorted keys", function()
        Expect(JSON.Encode({ b = 1, a = { true, false }, c = "x" })):ToBe('{"a":[true,false],"b":1,"c":"x"}')
        Expect(JSON.Encode({ 1, 2.5, "three" })):ToBe('[1,2.5,"three"]')
        Expect(JSON.Encode("text")):ToBe('"text"')
        Expect(JSON.Encode(42)):ToBe("42")
    end)

    Test.It("pretty prints", function()
        Expect(JSON.Encode({ a = 1 }, { Pretty = true })):ToBe('{\n  "a": 1\n}')
        Expect(JSON.Encode({ a = 1 }, { Indent = 4 })):ToBe('{\n    "a": 1\n}')
    end)

    Test.It("decodes documents", function()
        local value = JSON.Decode('{"name":"Quark","tags":["a","b"],"nested":{"pi":3.14,"count":3}}')
        Expect(value):ToEqual({ name = "Quark", tags = { "a", "b" }, nested = { pi = 3.14, count = 3 } })
        Expect(math.type(value.nested.count)):ToBe("integer")
    end)

    Test.It("handles null and empty arrays", function()
        Expect(JSON.Decode('{"a":null,"b":1}')):ToEqual({ b = 1 })
        local preserved = JSON.Decode('{"a":null}', { PreserveNull = true })
        Expect(JSON.IsNull(preserved.a)):ToBe(true)
        Expect(JSON.Encode({ a = JSON.Null })):ToBe('{"a":null}')

        Expect(JSON.Encode({})):ToBe("{}")
        Expect(JSON.Encode({}, { EmptyTablesAsArrays = true })):ToBe("[]")
        Expect(JSON.Encode({ list = JSON.Array() })):ToBe('{"list":[]}')
        Expect(JSON.Encode(JSON.Decode('{"list":[]}'))):ToBe('{"list":[]}')
    end)

    Test.It("reports invalid input", function()
        local value, error_message = JSON.Decode("{broken")
        Expect(value):ToBeNil()
        Expect(error_message):ToContain("invalid JSON")
        Expect(function() JSON.Encode({ callback = print }) end):ToThrow("cannot encode the value")
        local cycle = {}
        cycle.self = cycle
        Expect(function() JSON.Encode(cycle) end):ToThrow("cannot encode the value")
    end)

    Test.It("works asynchronously", function()
        Expect(Quark.Await(JSON.Decode, '{"a":[1,2,3]}')):ToEqual({ a = { 1, 2, 3 } })
        Expect(Quark.Await(JSON.Encode, { a = 1 }, { Pretty = true })):ToBe('{\n  "a": 1\n}')
        Expect(function() Quark.Await(JSON.Decode, "nope") end):ToThrow("invalid JSON")
    end)
end)

Test.Describe("TOML", function()
    local TOML = Quark.TOML

    Test.It("decodes documents with dates", function()
        local value = TOML.Decode([[
title = "Server"
created = 2026-10-24T22:00:00Z

[limits]
players = 64
ratio = 0.5
modes = ["ffa", "tdm"]
]])
        Expect(value.title):ToBe("Server")
        Expect(value.created):ToBe("2026-10-24T22:00:00Z")
        Expect(value.limits):ToEqual({ players = 64, ratio = 0.5, modes = { "ffa", "tdm" } })
    end)

    Test.It("encodes tables", function()
        local text = TOML.Encode({ name = "quark", server = { port = 7777 } })
        Expect(TOML.Decode(text)):ToEqual({ name = "quark", server = { port = 7777 } })
        Expect(text):ToContain("[server]")
        Expect(function() TOML.Encode("text") end):ToThrow("tables with string keys")
        Expect(select(2, TOML.Decode("= broken"))):ToContain("invalid TOML")
    end)
end)

Test.Describe("YAML", function()
    local YAML = Quark.YAML

    Test.It("round trips documents", function()
        local value = YAML.Decode("name: quark\nlist:\n  - 1\n  - two\nnested:\n  flag: true\n")
        Expect(value):ToEqual({ name = "quark", list = { 1, "two" }, nested = { flag = true } })
        Expect(YAML.Decode(YAML.Encode(value))):ToEqual(value)
        Expect(select(2, YAML.Decode("key: [unclosed"))):ToContain("invalid YAML")
    end)
end)

Test.Describe("MessagePack and CBOR", function()
    for _, name in ipairs({ "MessagePack", "CBOR" }) do
        local Format = Quark[name]

        Test.It(name .. " round trips values and binary data", function()
            local binary = "\0\255\1\254"
            local value = { id = 7, name = "quark", ratio = 0.25, list = { 1, 2, 3 }, binary = binary, flag = false }
            local encoded = Format.Encode(value)
            Expect(Format.Decode(encoded)):ToEqual(value)
            Expect(Format.Decode(Quark.Buffer.FromString(encoded))):ToEqual(value)
            Expect(select(2, Format.Decode(encoded:sub(1, -2)))):ToContain("invalid " .. name)
            Expect(select(2, Format.Decode(encoded .. "\0"))):ToContain("invalid " .. name)
        end)
    end

    Test.It("produces compact MessagePack", function()
        Expect(Quark.Crypto.HexEncode(Quark.MessagePack.Encode({ 1, 2, 3 }))):ToBe("93010203")
        Expect(Quark.Crypto.HexEncode(Quark.CBOR.Encode({ 1, 2, 3 }))):ToBe("83010203")
    end)
end)

Test.Describe("CSV", function()
    local CSV = Quark.CSV

    Test.It("decodes with and without a header", function()
        local rows = CSV.Decode('name,score\nalice,10\n"bob, jr",7\n')
        Expect(rows):ToEqual({ { name = "alice", score = "10" }, { name = "bob, jr", score = "7" } })
        Expect(CSV.Decode("a;b\nc;d", { Header = false, Delimiter = ";" })):ToEqual({ { "a", "b" }, { "c", "d" } })
        Expect(CSV.Decode("name , score\n alice , 10", { Trim = true })):ToEqual({ { name = "alice", score = "10" } })
    end)

    Test.It("encodes rows", function()
        Expect(CSV.Encode({ { name = "alice", score = 10 }, { name = "bob, jr", score = 7.5 } }))
            :ToBe('name,score\nalice,10\n"bob, jr",7.5\n')
        Expect(CSV.Encode({ { "a", "b" }, { 1, true } })):ToBe("a,b\n1,true\n")
        Expect(CSV.Encode({ { b = 2, a = 1 } }, { Columns = { "b", "a" } })):ToBe("b,a\n2,1\n")
        Expect(function() CSV.Encode({ { {} } }) end):ToThrow("CSV fields have to be")
    end)
end)

Test.Describe("UUID", function()
    local UUID = Quark.UUID

    Test.It("generates version 4 and 7", function()
        local v4, v7 = UUID.V4(), UUID.V7()
        Expect(v4):ToMatch("^%x%x%x%x%x%x%x%x%-%x%x%x%x%-4%x%x%x%-[89ab]%x%x%x%-%x%x%x%x%x%x%x%x%x%x%x%x$")
        Expect(UUID.GetVersion(v4)):ToBe(4)
        Expect(UUID.GetVersion(v7)):ToBe(7)
        Expect(UUID.V4()).Not:ToBe(v4)
    end)

    Test.It("orders version 7 by time", function()
        local previous = UUID.V7()
        for _ = 1, 100 do
            local current = UUID.V7()
            Expect(current > previous):ToBe(true)
            previous = current
        end
        local timestamp = UUID.GetTimestamp(previous)
        Expect(timestamp):ToBeType("integer")
        Expect(timestamp):ToBeGreaterThan(1700000000000)
        Expect(select(2, UUID.GetTimestamp(UUID.V4()))):ToContain("no timestamp")
    end)

    Test.It("parses and converts", function()
        Expect(UUID.Parse("{67E55044-10B1-426F-9247-BB680E5FE0C8}")):ToBe("67e55044-10b1-426f-9247-bb680e5fe0c8")
        Expect(UUID.IsValid("67e5504410b1426f9247bb680e5fe0c8")):ToBe(true)
        Expect(UUID.IsValid("not a uuid")):ToBe(false)
        Expect(UUID.IsValid(42)):ToBe(false)
        Expect(select(2, UUID.Parse("zz"))):ToContain("invalid UUID")
        local bytes = UUID.ToBytes(UUID.Max)
        Expect(bytes):ToBe(string.rep("\255", 16))
        Expect(UUID.FromBytes(bytes)):ToBe(UUID.Max)
        Expect(UUID.Nil):ToBe("00000000-0000-0000-0000-000000000000")
    end)
end)

Test.Describe("URL", function()
    local URL = Quark.URL

    Test.It("encodes and decodes components", function()
        Expect(URL.Encode("a b&c/d~e")):ToBe("a%20b%26c%2Fd~e")
        Expect(URL.Encode("a b", true)):ToBe("a+b")
        Expect(URL.Decode("a%20b%26c")):ToBe("a b&c")
        Expect(URL.Decode("a+b", true)):ToBe("a b")
        Expect(URL.Encode("äöü")):ToBe("%C3%A4%C3%B6%C3%BC")
    end)

    Test.It("builds and parses query strings", function()
        Expect(URL.EncodeQuery({ q = "hello world", page = 2, tag = { "a", "b" } })):ToBe("page=2&q=hello+world&tag=a&tag=b")
        Expect(URL.DecodeQuery("?q=hello+world&tag=a&tag=b&empty=")):ToEqual({ q = "hello world", tag = { "a", "b" }, empty = "" })
    end)

    Test.It("parses and joins URLs", function()
        local parts = URL.Parse("https://user:secret@example.com:8443/api/v1?x=1#top")
        Expect(parts):ToEqual({
            Scheme = "https", Username = "user", Password = "secret", Host = "example.com", Port = 8443,
            Path = "/api/v1", Query = "x=1", Fragment = "top", Origin = "https://example.com:8443",
        })
        Expect(URL.Parse("http://example.com").Port):ToBe(80)
        Expect(URL.Join("https://example.com/a/b", "../c")):ToBe("https://example.com/c")
        Expect(select(2, URL.Parse("not a url"))):ToContain("invalid URL")
    end)
end)

Test.Describe("XML", function()
    local XML = Quark.XML
    local DOCUMENT = [=[<?xml version="1.0"?>
<!-- server configuration -->
<server name="main" port="7777">
    <motd>Welcome &amp; have fun &#x263A;</motd>
    <admins>
        <admin id="1">alice</admin>
        <admin id="2"><![CDATA[<bob>]]></admin>
    </admins>
    <empty/>
</server>]=]

    Test.It("decodes documents into element tables", function()
        local root = XML.Decode(DOCUMENT)
        Expect(root.Name):ToBe("server")
        Expect(root.Attributes):ToEqual({ name = "main", port = "7777" })
        Expect(#root.Children):ToBe(3)
        Expect(XML.GetText(XML.Find(root, "motd"))):ToBe("Welcome & have fun ☺")

        local admins = XML.FindAll(XML.Find(root, "admins"), "admin")
        Expect(#admins):ToBe(2)
        Expect(admins[1].Attributes.id):ToBe("1")
        Expect(XML.GetText(admins[2])):ToBe("<bob>")
        Expect(XML.Find(root, "empty").Children):ToEqual({})
        Expect(XML.Find(root, "missing")):ToBeNil()
    end)

    Test.It("encodes element tables", function()
        local text = XML.Encode({
            Name = "player",
            Attributes = { id = 7, name = "A & B" },
            Children = { { Name = "score", Children = { 10 } }, "<raw>" },
        }, { Declaration = false })
        Expect(text):ToBe('<player id="7" name="A &amp; B"><score>10</score>&lt;raw&gt;</player>')
        Expect(XML.Decode(text).Attributes.name):ToBe("A & B")

        local pretty = XML.Encode({ Name = "a", Children = { { Name = "b" } } }, { Pretty = true })
        Expect(pretty):ToBe('<?xml version="1.0" encoding="UTF-8"?>\n<a>\n  <b/>\n</a>')
        Expect(function() XML.Encode({ Name = "bad name" }) end):ToThrow("not a valid element name")
    end)

    Test.It("reports invalid documents", function()
        Expect(select(2, XML.Decode("<a><b></a>"))):ToContain("invalid XML")
        Expect(select(2, XML.Decode("<a/><b/>"))):ToContain("more than one root")
        Expect(select(2, XML.Decode("<a>&unknown;</a>"))):ToContain("unknown entity")
        Expect(select(2, XML.Decode("<a>"))):ToContain("not closed")
    end)

    Test.It("keeps whitespace on request", function()
        Expect(XML.Decode("<a> <b/> </a>").Children):ToHaveLength(1)
        Expect(XML.Decode("<a> <b/> </a>", { PreserveWhitespace = true }).Children):ToHaveLength(3)
    end)
end)

Test.Describe("INI", function()
    local INI = Quark.INI

    Test.It("decodes sections and global keys", function()
        local document = INI.Decode("name = quark\n\n[server]\nport = 7777\nhost = 0.0.0.0\n\n[database]\nfile=data.db\n")
        Expect(document):ToEqual({
            name = "quark",
            server = { port = "7777", host = "0.0.0.0" },
            database = { file = "data.db" },
        })
    end)

    Test.It("encodes tables", function()
        local text = INI.Encode({ name = "quark", server = { port = 7777, debug = false } })
        Expect(text):ToBe("name=quark\n\n[server]\ndebug=false\nport=7777\n")
        Expect(INI.Decode(text).server.port):ToBe("7777")
        Expect(function() INI.Encode({ a = { b = { c = 1 } } }) end):ToThrow("cannot be nested")
    end)
end)

Test.Describe("JSON Schema", function()
    local JSON = Quark.JSON
    local SCHEMA = {
        type = "object",
        required = { "name", "level" },
        properties = {
            name = { type = "string", minLength = 3 },
            level = { type = "integer", minimum = 1, maximum = 100 },
            tags = { type = "array", items = { type = "string" } },
        },
    }

    Test.It("validates values", function()
        Expect(JSON.Validate({ name = "alice", level = 5 }, SCHEMA)):ToBe(true)

        local valid, errors = JSON.Validate({ name = "al", level = 500, tags = { "ok", 3 } }, SCHEMA)
        Expect(valid):ToBe(false)
        Expect(#errors):ToBe(3)
        local paths = {}
        for _, entry in ipairs(errors) do
            paths[#paths + 1] = entry.Path
            Expect(entry.Message):ToBeType("string")
        end
        table.sort(paths)
        Expect(paths):ToEqual({ "/level", "/name", "/tags/1" })
    end)

    Test.It("compiles schemas once", function()
        local schema = JSON.Schema('{"type":"array","items":{"type":"number"}}')
        Expect(schema:IsValid({ 1, 2.5 })):ToBe(true)
        Expect(schema:IsValid({ 1, "x" })):ToBe(false)
        Expect(select(2, schema:Validate({ "x" }))[1].Path):ToBe("/0")
        Expect(function() JSON.Schema({ type = "no such type" }) end):ToThrow("invalid JSON schema")
    end)

    Test.It("does not fetch remote references", function()
        Expect(function() JSON.Schema({ ["$ref"] = "https://example.com/schema.json" }) end):ToThrow("invalid JSON schema")
    end)
end)
