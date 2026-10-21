local Test = Quark.Import("Testing")
local Expect = Test.Expect
local Promise = Quark.Promise

local function next_event(object, event)
    return Promise.New(function(resolve)
        local handler
        handler = object:Subscribe(event, function(...)
            object:Unsubscribe(event, handler)
            resolve(...)
        end)
    end)
end

local function collect(object, event, count)
    return Promise.New(function(resolve)
        local received = {}
        object:Subscribe(event, function(value)
            received[#received + 1] = value
            if #received == count then
                resolve(received)
            end
        end)
    end)
end

Test.Describe("Net", function()
    local Net = Quark.Net

    Test.It("checks IP addresses and ranges", function()
        Expect(Net.IsIP("192.168.1.10")):ToBe(true)
        Expect(Net.IsIP("::1")):ToBe(true)
        Expect(Net.IsIP("example.com")):ToBe(false)
        Expect(Net.IsIPv4("10.0.0.1")):ToBe(true)
        Expect(Net.IsIPv6("10.0.0.1")):ToBe(false)
        Expect(Net.NormalizeIP("::ffff:127.0.0.1")):ToBe("127.0.0.1")
        Expect(Net.NormalizeIP("2001:0db8:0000::0001")):ToBe("2001:db8::1")
        Expect(Net.IsInRange("10.1.2.3", "10.0.0.0/8")):ToBe(true)
        Expect(Net.IsInRange("11.1.2.3", "10.0.0.0/8")):ToBe(false)
        Expect(Net.IsInRange("2001:db8::5", "2001:db8::/32")):ToBe(true)
        Expect(Net.IsPrivate("192.168.0.1")):ToBe(true)
        Expect(Net.IsPrivate("8.8.8.8")):ToBe(false)
        Expect(Net.IsPrivate("fd00::1")):ToBe(true)
        Expect(Net.IsLoopback("127.0.0.1")):ToBe(true)
        Expect(function() Net.IsInRange("1.2.3.4", "nonsense") end):ToThrow("not a CIDR range")
    end)

    Test.It("resolves host names", function()
        Expect(Net.Resolve("localhost")):ToContain("127.0.0.1")
        Expect(Quark.Await(Net.Resolve, "127.0.0.1")):ToEqual({ "127.0.0.1" })
        Expect(select(2, Net.Resolve("does-not-exist.invalid"))):ToContain("cannot resolve")
    end)

    Test.It("accepts a callback in place of the record type", function()
        local returned = table.pack(Net.Lookup("example.com", function() end))
        Expect(returned.n):ToBe(0)
        Expect(function() Net.Lookup("example.com", "SOA") end):ToThrow("unsupported record type")
        Expect(function() Net.Lookup("example.com", {}) end):ToThrow("expected a record type or a callback")
    end)
end)

Test.Describe("TCP", function()
    local TCP = Quark.TCP

    local function echo_server(options)
        local server = assert(TCP.Listen(0, options))
        server:Subscribe("Connection", function(client)
            client:Subscribe("Data", function(data)
                client:Send(data)
            end)
        end)
        return server
    end

    Test.It("exchanges line framed messages", function()
        local server = echo_server({ Host = "127.0.0.1", Framing = "line" })
        Expect(server:GetPort()):ToBeGreaterThan(0)

        local client = TCP.Connect("127.0.0.1", server:GetPort(), { Framing = "line" })
        Expect(client:GetState()):ToBe("connecting")
        Quark.Await(next_event(client, "Connect"))
        Expect(client:GetState()):ToBe("open")
        Expect(select(2, client:GetRemoteAddress())):ToBe(server:GetPort())

        local replies = collect(client, "Data", 3)
        client:Send("first")
        client:Send("second")
        client:Send(Quark.Buffer.FromString("third"))
        Expect(Quark.Await(replies)):ToEqual({ "first", "second", "third" })
        Expect(#server:GetClients()):ToBe(1)
        Expect(function() client:Send("line\nbreak") end):ToThrow("must not contain a newline")

        local closed = next_event(client, "Close")
        client:Close()
        Expect(Quark.Await(closed)):ToBe("closed")
        Expect(client:Send("late")):ToBe(false)
        server:Close()
    end)

    Test.It("exchanges length prefixed binary frames", function()
        local server = echo_server({ Host = "127.0.0.1", Framing = "length" })
        local client = TCP.Connect("127.0.0.1", server:GetPort(), { Framing = "length" })
        Quark.Await(next_event(client, "Connect"))

        local payload = string.rep("\0\1\2\255", 5000)
        local reply = next_event(client, "Data")
        client:Send(payload)
        Expect(Quark.Await(reply)):ToBe(payload)
        server:Close()
    end)

    Test.It("closes clients together with the server", function()
        local server = echo_server({ Host = "127.0.0.1" })
        local client = TCP.Connect("127.0.0.1", server:GetPort())
        Quark.Await(next_event(client, "Connect"))
        Quark.Await(Quark.Delay(20))

        local client_closed = next_event(client, "Close")
        local server_closed = next_event(server, "Close")
        server:Close()
        Quark.Await(server_closed)
        Expect(Quark.Await(client_closed)):ToBe("closed by peer")
    end)

    Test.It("reports connection failures", function()
        local server = assert(TCP.Listen(0, { Host = "127.0.0.1" }))
        local port = server:GetPort()
        local closed = next_event(server, "Close")
        server:Close()
        Quark.Await(closed)

        local client = TCP.Connect("127.0.0.1", port, { Timeout = 2000 })
        local error_message = Quark.Await(next_event(client, "Error"))
        Expect(error_message):ToContain("cannot connect")
        Expect(client:GetState()):ToBe("closed")
    end)

    Test.It("rejects ports in use", function()
        local server = assert(TCP.Listen(0, { Host = "127.0.0.1" }))
        local second, error_message = TCP.Listen(server:GetPort(), { Host = "127.0.0.1" })
        Expect(second):ToBeNil()
        Expect(error_message):ToContain("cannot listen")
        server:Close()
    end)
end)

Test.Describe("UDP", function()
    local UDP = Quark.UDP

    Test.It("sends and receives datagrams", function()
        local first = assert(UDP.Bind(0, { Host = "127.0.0.1" }))
        local second = assert(UDP.Bind(0, { Host = "127.0.0.1" }))
        local _, first_port = first:GetLocalAddress()
        local _, second_port = second:GetLocalAddress()

        second:Subscribe("Message", function(data, address, port)
            second:Send("pong:" .. data, address, port)
        end)

        local reply = next_event(first, "Message")
        first:Send("ping", "127.0.0.1", second_port)
        local data, address, port = Quark.Await(reply)
        Expect(data):ToBe("pong:ping")
        Expect(address):ToBe("127.0.0.1")
        Expect(port):ToBe(second_port)
        Expect(first_port).Not:ToBe(second_port)

        first:Close()
        second:Close()
        Expect(function() first:Send(string.rep("x", 70000), "127.0.0.1", 1) end):ToThrow("cannot be larger")
    end)
end)

Test.Describe("WebSocket", function()
    local WebSocket = Quark.WebSocket

    local function chat_server()
        local server = assert(WebSocket.Listen(0, { Host = "127.0.0.1", Path = "/chat" }))
        server:Subscribe("Connection", function(client, request)
            client:Subscribe("Message", function(data, binary)
                if binary then
                    client:SendBinary(data)
                elseif data == "who" then
                    client:Send(request.Path .. "?" .. (request.Query or "") .. " " .. (request.Headers["x-player"] or ""))
                else
                    client:Send("echo:" .. data)
                end
            end)
        end)
        return server
    end

    Test.It("exchanges text and binary messages", function()
        local server = chat_server()
        local client = WebSocket.Connect("ws://127.0.0.1:" .. server:GetPort() .. "/chat?room=1", {
            Headers = { ["X-Player"] = "alice" },
        })
        Quark.Await(next_event(client, "Open"))
        Expect(client:IsOpen()):ToBe(true)

        local replies = collect(client, "Message", 3)
        client:Send("hello")
        client:SendBinary("\0\1\2")
        client:Send("who")
        Expect(Quark.Await(replies)):ToEqual({ "echo:hello", "\0\1\2", "/chat?room=1 alice" })
        server:Close()
    end)

    Test.It("broadcasts to all clients", function()
        local server = chat_server()
        local url = "ws://127.0.0.1:" .. server:GetPort() .. "/chat"
        local first, second = WebSocket.Connect(url), WebSocket.Connect(url)
        Quark.Await(Promise.All({ next_event(first, "Open"), next_event(second, "Open") }))
        Quark.Await(Quark.Delay(20))

        local received = Promise.All({ next_event(first, "Message"), next_event(second, "Message") })
        Expect(server:Broadcast("news")):ToBe(2)
        Expect(Quark.Await(received)):ToEqual({ "news", "news" })
        Expect(#server:GetClients()):ToBe(2)
        server:Close()
    end)

    Test.It("passes close codes and reasons", function()
        local server = chat_server()
        local server_side = Promise.New(function(resolve)
            server:Subscribe("Connection", function(client)
                client:Subscribe("Close", function(code, reason)
                    resolve({ code, reason })
                end)
            end)
        end)

        local client = WebSocket.Connect("ws://127.0.0.1:" .. server:GetPort() .. "/chat")
        Quark.Await(next_event(client, "Open"))
        local client_closed = next_event(client, "Close")
        client:Close(4001, "bye")
        Expect(Quark.Await(server_side)):ToEqual({ 4001, "bye" })
        Expect(Quark.Await(client_closed)):ToBe(4001)
        Expect(function() client:Close(42) end):ToThrow("between 1000 and 4999")
        server:Close()
    end)

    Test.It("rejects unknown paths and bad URLs", function()
        local server = chat_server()
        local client = WebSocket.Connect("ws://127.0.0.1:" .. server:GetPort() .. "/other")
        Expect(Quark.Await(next_event(client, "Error"))):ToContain("handshake failed")
        Expect(function() WebSocket.Connect("http://example.com") end):ToThrow("ws:// or wss://")
        server:Close()
    end)
end)

Test.Describe("HTTP", function()
    local HTTP = Quark.HTTP

    local function request(port, method, path, headers, body)
        return Promise.New(function(resolve, reject)
            local socket = Quark.TCP.Connect("127.0.0.1", port)
            local chunks = {}
            socket:Subscribe("Connect", function()
                local lines = { method .. " " .. path .. " HTTP/1.1", "Host: localhost", "Connection: close" }
                for name, value in pairs(headers or {}) do
                    lines[#lines + 1] = name .. ": " .. value
                end
                if body then
                    lines[#lines + 1] = "Content-Length: " .. #body
                end
                socket:Send(table.concat(lines, "\r\n") .. "\r\n\r\n" .. (body or ""))
            end)
            socket:Subscribe("Data", function(data)
                chunks[#chunks + 1] = data
            end)
            socket:Subscribe("Error", reject)
            socket:Subscribe("Close", function()
                local raw = table.concat(chunks)
                local head, content = raw:match("^(.-)\r\n\r\n(.*)$")
                if not head then
                    return reject("incomplete response: " .. raw)
                end
                local status = tonumber(head:match("^HTTP/1%.1 (%d+)"))
                local response_headers = {}
                for name, value in head:gmatch("\r\n([^:\r\n]+):%s*([^\r\n]*)") do
                    response_headers[name:lower()] = value
                end
                resolve({ Status = status, Headers = response_headers, Body = content })
            end)
        end)
    end

    local function api()
        local server = assert(HTTP.Listen(0, { Host = "127.0.0.1", MaxBodySize = 1024, Timeout = 500 }))
        server:Get("/players/:id", function(req, res)
            res:Json({ id = req.Params.id, page = req.Query.page })
        end)
        server:Post("/echo", function(req, res)
            local data, error_message = req:Json()
            if not data then
                return res:Json({ error = error_message }, 400)
            end
            res:Status(201):Header("X-Echo", "yes"):Json(data)
        end)
        server:Get("/slow", function(_, res)
            Quark.Await(Quark.Delay(30))
            res:Text("waited")
        end)
        server:Get("/files/*", function(req, res)
            res:Text(req.Params["*"])
        end)
        server:Get("/broken", function()
            error("handler failure")
        end)
        server:Get("/silent", function() end)
        server:Get("/hang", function()
            Quark.Await(Quark.Delay(2000))
        end)
        server:Get("/redirect", function(_, res)
            res:Redirect("/players/1")
        end)
        return server
    end

    Test.It("routes requests with parameters and query strings", function()
        local server = api()
        local response = Quark.Await(request(server:GetPort(), "GET", "/players/42?page=3"))
        Expect(response.Status):ToBe(200)
        Expect(response.Headers["content-type"]):ToBe("application/json")
        Expect(Quark.JSON.Decode(response.Body)):ToEqual({ id = "42", page = "3" })

        local encoded = Quark.Await(request(server:GetPort(), "GET", "/players/a%20b"))
        Expect(Quark.JSON.Decode(encoded.Body).id):ToBe("a b")

        local wildcard = Quark.Await(request(server:GetPort(), "GET", "/files/maps/test.umap"))
        Expect(wildcard.Body):ToBe("maps/test.umap")
        server:Close()
    end)

    Test.It("reads request bodies and sets status and headers", function()
        local server = api()
        local response = Quark.Await(request(server:GetPort(), "POST", "/echo", { ["Content-Type"] = "application/json" }, '{"name":"alice"}'))
        Expect(response.Status):ToBe(201)
        Expect(response.Headers["x-echo"]):ToBe("yes")
        Expect(Quark.JSON.Decode(response.Body)):ToEqual({ name = "alice" })

        local invalid = Quark.Await(request(server:GetPort(), "POST", "/echo", nil, "{broken"))
        Expect(invalid.Status):ToBe(400)
        Expect(Quark.JSON.Decode(invalid.Body).error):ToContain("invalid JSON")

        local too_large = Quark.Await(request(server:GetPort(), "POST", "/echo", nil, string.rep("x", 2000)))
        Expect(too_large.Status):ToBe(413)
        server:Close()
    end)

    Test.It("awaits inside handlers", function()
        local server = api()
        local slow = request(server:GetPort(), "GET", "/slow")
        local fast = request(server:GetPort(), "GET", "/players/1")
        local results = Quark.Await(Promise.All({ slow, fast }))
        Expect(results[1].Body):ToBe("waited")
        Expect(results[2].Status):ToBe(200)
        server:Close()
    end)

    Test.It("answers unknown routes, wrong methods and failures", function()
        local server = api()
        local port = server:GetPort()
        local report = Test.Mock(Console, "Error")

        local missing = Quark.Await(request(port, "GET", "/nothing"))
        Expect(missing.Status):ToBe(404)

        local wrong_method = Quark.Await(request(port, "DELETE", "/players/1"))
        Expect(wrong_method.Status):ToBe(405)
        Expect(wrong_method.Headers.allow):ToBe("GET")

        Expect(Quark.Await(request(port, "GET", "/broken")).Status):ToBe(500)
        Expect(Quark.Await(request(port, "GET", "/silent")).Status):ToBe(500)
        Expect(Quark.Await(request(port, "GET", "/hang")).Status):ToBe(504)
        Expect(report):ToHaveBeenCalledTimes(2)

        local redirect = Quark.Await(request(port, "GET", "/redirect"))
        Expect(redirect.Status):ToBe(302)
        Expect(redirect.Headers.location):ToBe("/players/1")

        local head = Quark.Await(request(port, "HEAD", "/players/1"))
        Expect(head.Status):ToBe(200)
        Expect(head.Body):ToBe("")
        server:Close()
    end, { Timeout = 5000 })

    Test.It("runs middleware in order", function()
        local server = assert(HTTP.Listen(0, { Host = "127.0.0.1" }))
        local order = {}
        server:Use(function(req, res, next)
            order[#order + 1] = "first"
            res:Header("X-Request", req.Method)
            next()
            order[#order + 1] = "first after"
        end)
        server:Use(function(req, res, next)
            if req.Headers.authorization ~= "Bearer secret" then
                return res:Text("Unauthorized", 401)
            end
            next()
        end)
        server:Get("/private", function(_, res)
            order[#order + 1] = "handler"
            res:Text("secret data")
        end)

        local denied = Quark.Await(request(server:GetPort(), "GET", "/private"))
        Expect(denied.Status):ToBe(401)
        Expect(denied.Headers["x-request"]):ToBe("GET")

        order = {}
        local allowed = Quark.Await(request(server:GetPort(), "GET", "/private", { Authorization = "Bearer secret" }))
        Expect(allowed.Body):ToBe("secret data")
        Expect(order):ToEqual({ "first", "handler", "first after" })
        server:Close()
    end)

    Test.It("handles CORS", function()
        local server = assert(HTTP.Listen(0, { Host = "127.0.0.1" }))
        server:Use(HTTP.Cors({ Origin = { "https://panel.example.com" }, Credentials = true }))
        server:Get("/data", function(_, res)
            res:Json({ ok = true })
        end)

        local preflight = Quark.Await(request(server:GetPort(), "OPTIONS", "/data", {
            Origin = "https://panel.example.com",
            ["Access-Control-Request-Method"] = "GET",
        }))
        Expect(preflight.Status):ToBe(204)
        Expect(preflight.Headers["access-control-allow-origin"]):ToBe("https://panel.example.com")
        Expect(preflight.Headers["access-control-allow-methods"]):ToContain("GET")
        Expect(preflight.Headers["access-control-allow-credentials"]):ToBe("true")

        local foreign = Quark.Await(request(server:GetPort(), "GET", "/data", { Origin = "https://evil.example.com" }))
        Expect(foreign.Status):ToBe(200)
        Expect(foreign.Headers["access-control-allow-origin"]):ToBeNil()
        server:Close()
    end)

    Test.It("validates routes and responses", function()
        local server = assert(HTTP.Listen(0, { Host = "127.0.0.1" }))
        Expect(function() server:Get("no-slash", function() end) end):ToThrow("have to start with /")
        Expect(function() server:Get("/a/*/b", function() end) end):ToThrow("only allowed as the last segment")
        Expect(function() server:Route("BREW", "/coffee", function() end) end):ToThrow("unsupported HTTP method")
        Expect(function() server:Use("not a function") end):ToThrow("middleware has to be a function")

        server:Get("/double", function(_, res)
            res:Text("first")
            res:Text("second")
        end)
        local report = Test.Mock(Console, "Error")
        Expect(Quark.Await(request(server:GetPort(), "GET", "/double")).Body):ToBe("first")
        Quark.Await(Quark.Delay(20))
        Expect(report):ToHaveBeenCalled()
        server:Close()
    end)
end)

Test.Describe("TLS", function()
    local certificate, private_key = Quark.Net.CreateSelfSignedCertificate({ "localhost" })
    local server_tls = { Certificate = certificate, PrivateKey = private_key }
    local client_tls = { TLS = true, ServerName = "localhost", CA = certificate }

    local function merge(base, extra)
        local merged = {}
        for key, value in pairs(base) do merged[key] = value end
        for key, value in pairs(extra or {}) do merged[key] = value end
        return merged
    end

    Test.It("creates self signed certificates", function()
        Expect(certificate):ToContain("BEGIN CERTIFICATE")
        Expect(private_key):ToContain("PRIVATE KEY")
        Expect(function() Quark.Net.CreateSelfSignedCertificate({}) end):ToThrow("at least one host name")
    end)

    Test.It("encrypts TCP connections", function()
        local server = assert(Quark.TCP.Listen(0, { Host = "127.0.0.1", Framing = "line", TLS = server_tls }))
        server:Subscribe("Connection", function(client)
            client:Subscribe("Data", function(line) client:Send("secure:" .. line) end)
        end)

        local client = Quark.TCP.Connect("127.0.0.1", server:GetPort(), merge(client_tls, { Framing = "line" }))
        Quark.Await(next_event(client, "Connect"))
        local reply = next_event(client, "Data")
        client:Send("hello")
        Expect(Quark.Await(reply)):ToBe("secure:hello")
        client:Close()
        server:Close()
    end)

    Test.It("rejects untrusted certificates and wrong names", function()
        local server = assert(Quark.TCP.Listen(0, { Host = "127.0.0.1", TLS = server_tls }))
        local untrusted = Quark.TCP.Connect("127.0.0.1", server:GetPort(), { TLS = true, ServerName = "localhost" })
        Expect(Quark.Await(next_event(untrusted, "Error"))):ToContain("TLS handshake")

        local wrong_name = Quark.TCP.Connect("127.0.0.1", server:GetPort(), merge(client_tls, { ServerName = "example.com" }))
        Expect(Quark.Await(next_event(wrong_name, "Error"))):ToContain("TLS handshake")
        server:Close()
    end)

    Test.It("serves secure WebSockets", function()
        local server = assert(Quark.WebSocket.Listen(0, { Host = "127.0.0.1", TLS = server_tls }))
        server:Subscribe("Connection", function(client)
            client:Subscribe("Message", function(data) client:Send("wss:" .. data) end)
        end)

        local client = Quark.WebSocket.Connect("wss://localhost:" .. server:GetPort(), { CA = certificate })
        Quark.Await(next_event(client, "Open"))
        local reply = next_event(client, "Message")
        client:Send("ping")
        Expect(Quark.Await(reply)):ToBe("wss:ping")
        client:Close()
        server:Close()
    end)

    Test.It("serves HTTPS", function()
        local server = assert(Quark.HTTP.Listen(0, { Host = "127.0.0.1", TLS = server_tls }))
        server:Get("/secure", function(_, response) response:Text("encrypted") end)

        local client = Quark.TCP.Connect("127.0.0.1", server:GetPort(), client_tls)
        local chunks = {}
        client:Subscribe("Connect", function()
            client:Send("GET /secure HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        end)
        client:Subscribe("Data", function(data) chunks[#chunks + 1] = data end)
        Quark.Await(next_event(client, "Close"))
        local raw = table.concat(chunks)
        Expect(raw):ToMatch("^HTTP/1%.1 200")
        Expect(raw):ToMatch("encrypted$")
        server:Close()
    end)

    Test.It("validates TLS options", function()
        Expect(function() Quark.TCP.Listen(0, { TLS = true }) end):ToThrow("has to be a table")
        Expect(function() Quark.TCP.Listen(0, { TLS = { Certificate = certificate } }) end):ToThrow("needs a PrivateKey")
        local _, other_key = Quark.Net.CreateSelfSignedCertificate("localhost")
        Expect(function() Quark.HTTP.Listen(0, { TLS = { Certificate = certificate, PrivateKey = other_key } }) end)
            :ToThrow("does not match the private key")
        Expect(function() Quark.TCP.Connect("localhost", 1, { TLS = true, CA = "garbage" }) end):ToThrow("no certificate")
    end)
end)

Test.Describe("HTTP client", function()
    local HTTP = Quark.HTTP
    local certificate, private_key = Quark.Net.CreateSelfSignedCertificate({ "localhost" })

    local function backend(options)
        local server = assert(HTTP.Listen(0, options or { Host = "127.0.0.1" }))
        server:Get("/players/:id", function(request, response)
            response:Json({ id = request.Params.id, query = request.Query, agent = request.Headers["user-agent"] })
        end)
        server:All("/echo", function(request, response)
            response:Header("X-Method", request.Method)
            response:Send(request.Body, request.Headers["content-type"])
        end)
        server:Get("/moved", function(_, response) response:Redirect("/players/7") end)
        server:Post("/submit", function(_, response) response:Redirect("/echo", 303) end)
        server:Get("/hang", function() Quark.Await(Quark.Delay(2000)) end)
        server:Get("/large", function(_, response) response:Text(string.rep("x", 100000)) end)
        return server
    end

    Test.It("sends GET requests with query parameters", function()
        local server = backend()
        local base = "http://127.0.0.1:" .. server:GetPort()
        local response = Quark.Await(HTTP.Get, base .. "/players/5", { Query = { page = 2 } })
        Expect(response.Status):ToBe(200)
        Expect(response:IsSuccess()):ToBe(true)
        Expect(response.Headers["content-type"]):ToBe("application/json")
        local data = response:Json()
        Expect(data.id):ToBe("5")
        Expect(data.query):ToEqual({ page = "2" })
        Expect(data.agent):ToMatch("^Quark/")
        server:Close()
    end)

    Test.It("posts JSON, raw bodies and custom methods", function()
        local server = backend()
        local base = "http://127.0.0.1:" .. server:GetPort()

        local json = Quark.Await(HTTP.Post, base .. "/echo", { name = "alice" })
        Expect(json.Headers["content-type"]):ToBe("application/json")
        Expect(json:Json()):ToEqual({ name = "alice" })

        local raw = Quark.Await(HTTP.Post, base .. "/echo", "plain body", { Headers = { ["Content-Type"] = "text/plain" } })
        Expect(raw.Body):ToBe("plain body")
        Expect(raw.Headers["x-method"]):ToBe("POST")

        local put = Quark.Await(HTTP.Request, { URL = base .. "/echo", Method = "put", Body = Quark.Buffer.FromString("buffer") })
        Expect(put.Headers["x-method"]):ToBe("PUT")
        Expect(put.Body):ToBe("buffer")
        server:Close()
    end)

    Test.It("follows redirects", function()
        local server = backend()
        local base = "http://127.0.0.1:" .. server:GetPort()

        local followed = Quark.Await(HTTP.Get, base .. "/moved")
        Expect(followed.Status):ToBe(200)
        Expect(followed.URL):ToBe(base .. "/players/7")

        local manual = Quark.Await(HTTP.Get, base .. "/moved", { FollowRedirects = false })
        Expect(manual.Status):ToBe(302)
        Expect(manual.Headers.location):ToBe("/players/7")

        local see_other = Quark.Await(HTTP.Post, base .. "/submit", { data = 1 })
        Expect(see_other.Headers["x-method"]):ToBe("GET")
        Expect(see_other.Body):ToBe("")
        server:Close()
    end)

    Test.It("enforces timeouts and size limits", function()
        local server = backend()
        local base = "http://127.0.0.1:" .. server:GetPort()
        Expect(function() Quark.Await(HTTP.Get, base .. "/hang", { Timeout = 200 }) end):ToThrow("timed out")
        Expect(function() Quark.Await(HTTP.Get, base .. "/large", { MaxBodySize = 1000 }) end):ToThrow("larger than 1000 bytes")
        server:Close()
    end, { Timeout = 5000 })

    Test.It("reports connection errors", function()
        local closed = assert(HTTP.Listen(0, { Host = "127.0.0.1" }))
        local port = closed:GetPort()
        closed:Close()
        Quark.Await(Quark.Delay(20))

        local response, error_message = HTTP.Get("http://127.0.0.1:" .. port .. "/")
        Expect(response):ToBeNil()
        Expect(error_message):ToContain("cannot connect")
        Expect(function() HTTP.Get("ftp://example.com") end):ToThrow("only http:// and https://")
        Expect(function() HTTP.Request({}) end):ToThrow("URL option is required")
    end)

    Test.It("uses TLS with custom root certificates", function()
        local server = backend({ Host = "127.0.0.1", TLS = { Certificate = certificate, PrivateKey = private_key } })
        local url = "https://localhost:" .. server:GetPort() .. "/players/9"
        local response = Quark.Await(HTTP.Get, url, { CA = certificate })
        Expect(response:Json().id):ToBe("9")
        Expect(function() Quark.Await(HTTP.Get, url) end):ToThrow("TLS handshake")
        server:Close()
    end)

    Test.It("streams responses", function()
        local server = backend()
        local stream = HTTP.Stream({ URL = "http://127.0.0.1:" .. server:GetPort() .. "/large" })
        local status, headers = Quark.Await(next_event(stream, "Response"))
        Expect(status):ToBe(200)
        Expect(headers["content-length"]):ToBe("100000")

        local received = 0
        stream:Subscribe("Data", function(chunk) received = received + #chunk end)
        Expect(Quark.Await(next_event(stream, "Close"))):ToBe("finished")
        Expect(received):ToBe(100000)
        server:Close()
    end)
end)

Test.Describe("WebSocket reconnect", function()
    local WebSocket = Quark.WebSocket
    local FAST = { InitialDelay = 30, MaxDelay = 60 }

    local function server_on(port)
        local server = assert(WebSocket.Listen(port or 0, { Host = "127.0.0.1" }))
        local received = {}
        local clients = {}
        server:Subscribe("Connection", function(client)
            clients[#clients + 1] = client
            client:Subscribe("Message", function(data) received[#received + 1] = data end)
        end)
        return server, received, clients
    end

    Test.It("reconnects after the server drops the connection", function()
        local server, received, clients = server_on()
        local client = WebSocket.Connect("ws://127.0.0.1:" .. server:GetPort(), { Reconnect = { InitialDelay = 200, MaxDelay = 300 } })
        Quark.Await(next_event(client, "Open"))
        Quark.Await(Quark.Delay(20))

        local disconnected = next_event(client, "Disconnect")
        local reconnecting = next_event(client, "Reconnect")
        local reopened = Promise.New(function(resolve)
            client:Subscribe("Disconnect", function()
                client:Subscribe("Open", resolve)
            end)
        end)
        clients[1]:Close(4002, "kicked")

        local code, reason = Quark.Await(disconnected)
        Expect(code):ToBe(4002)
        Expect(reason):ToBe("kicked")
        local attempt, delay = Quark.Await(reconnecting)
        Expect(attempt):ToBe(1)
        Expect(delay):ToBeLessThanOrEqual(300)
        Expect(client:GetState()):ToBe("connecting")
        client:Send("queued while reconnecting")

        Quark.Await(reopened)
        Expect(client:IsOpen()):ToBe(true)
        Quark.Await(Quark.Delay(50))
        Expect(received):ToContain("queued while reconnecting")
        Expect(#clients):ToBe(2)

        local closed = next_event(client, "Close")
        client:Close()
        Expect(Quark.Await(closed)):ToBe(1000)
        server:Close()
    end)

    Test.It("gives up after MaxAttempts", function()
        local server = server_on()
        local port = server:GetPort()
        local server_closed = next_event(server, "Close")
        server:Close()
        Quark.Await(server_closed)

        local client = WebSocket.Connect("ws://127.0.0.1:" .. port, {
            Reconnect = { InitialDelay = 20, MaxDelay = 40, MaxAttempts = 2 },
        })
        local attempts = {}
        client:Subscribe("Reconnect", function(attempt) attempts[#attempts + 1] = attempt end)
        local errors = 0
        client:Subscribe("Error", function() errors = errors + 1 end)

        local code, reason = Quark.Await(next_event(client, "Close"))
        Expect(code):ToBe(1006)
        Expect(reason):ToContain("cannot connect")
        Expect(attempts):ToEqual({ 1, 2 })
        Expect(errors):ToBe(3)
    end)

    Test.It("stops reconnecting when closed", function()
        local server = server_on()
        local port = server:GetPort()
        local server_closed = next_event(server, "Close")
        server:Close()
        Quark.Await(server_closed)

        local client = WebSocket.Connect("ws://127.0.0.1:" .. port, { Reconnect = { InitialDelay = 5000, MaxDelay = 5000 } })
        Quark.Await(next_event(client, "Reconnect"))
        local closed = next_event(client, "Close")
        client:Close()
        local code, reason = Quark.Await(closed)
        Expect(code):ToBe(1000)
        Expect(reason):ToBe("closed")
    end, { Timeout = 2000 })

    Test.It("validates the options", function()
        Expect(function() WebSocket.Connect("ws://127.0.0.1:1", { Reconnect = { Multiplier = 0.5 } }) end):ToThrow("Multiplier >= 1")
        Expect(function() WebSocket.Connect("ws://127.0.0.1:1", { Reconnect = "always" }) end):ToThrow("boolean or a table")
    end)
end)

Test.Describe("HTTP server features", function()
    local HTTP = Quark.HTTP

    local function base(server)
        return "http://127.0.0.1:" .. server:GetPort()
    end

    Test.It("reads and writes cookies", function()
        local server = assert(HTTP.Listen(0, { Host = "127.0.0.1" }))
        server:Get("/login", function(_, response)
            response:Cookie("session", "abc 123", { MaxAge = 3600 })
            response:Cookie("theme", "dark", { HttpOnly = false, SameSite = "Strict", Secure = true })
            response:Text("ok")
        end)
        server:Get("/me", function(request, response)
            response:Json({ session = request.Cookies.session, theme = request.Cookies.theme })
        end)
        server:Get("/logout", function(_, response)
            response:ClearCookie("session"):Text("bye")
        end)
        server:Get("/insecure", function(_, response)
            local ok, reason = pcall(response.Cookie, response, "x", "y", { SameSite = "None" })
            response:Text(ok and "accepted" or reason)
        end)

        local login = Quark.Await(HTTP.Get, base(server) .. "/login")
        local cookies = login.Headers["set-cookie"]
        Expect(cookies):ToContain("session=abc%20123; Path=/; Max-Age=3600; SameSite=Lax; HttpOnly")
        Expect(cookies):ToContain("theme=dark; Path=/; SameSite=Strict; Secure")

        local me = Quark.Await(HTTP.Get, base(server) .. "/me", { Headers = { Cookie = "session=abc%20123; theme=dark" } })
        Expect(me:Json()):ToEqual({ session = "abc 123", theme = "dark" })

        Expect(Quark.Await(HTTP.Get, base(server) .. "/logout").Headers["set-cookie"]):ToContain("session=; Path=/; Max-Age=0")
        Expect(Quark.Await(HTTP.Get, base(server) .. "/insecure").Body):ToContain("have to be Secure")
        server:Close()
    end)

    Test.It("parses url encoded and multipart forms", function()
        local server = assert(HTTP.Listen(0, { Host = "127.0.0.1" }))
        server:Post("/form", function(request, response)
            local fields, files = request:Form()
            if not fields then
                return response:Json({ error = files }, 400)
            end
            local uploaded = {}
            for _, file in ipairs(files) do
                uploaded[#uploaded + 1] = { field = file.Field, name = file.FileName, type = file.ContentType, size = #file.Data }
            end
            response:Json({ fields = fields, files = uploaded })
        end)

        local encoded = Quark.Await(HTTP.Post, base(server) .. "/form", "name=alice&level=5&note=a+b%21", {
            Headers = { ["Content-Type"] = "application/x-www-form-urlencoded" },
        })
        Expect(encoded:Json().fields):ToEqual({ name = "alice", level = "5", note = "a b!" })

        local boundary = "quarkboundary"
        local body = table.concat({
            "--" .. boundary,
            'Content-Disposition: form-data; name="title"',
            "",
            "screenshot",
            "--" .. boundary,
            'Content-Disposition: form-data; name="image"; filename="shot.png"',
            "Content-Type: image/png",
            "",
            "\137PNG\0\1\2",
            "--" .. boundary .. "--",
            "",
        }, "\r\n")
        local multipart = Quark.Await(HTTP.Post, base(server) .. "/form", body, {
            Headers = { ["Content-Type"] = "multipart/form-data; boundary=" .. boundary },
        })
        Expect(multipart:Json()):ToEqual({
            fields = { title = "screenshot" },
            files = { { field = "image", name = "shot.png", type = "image/png", size = 7 } },
        })

        local unsupported = Quark.Await(HTTP.Post, base(server) .. "/form", "{}", { Headers = { ["Content-Type"] = "application/json" } })
        Expect(unsupported.Status):ToBe(400)
        Expect(unsupported:Json().error):ToContain("unsupported form content type")
        server:Close()
    end)

    local with_files = (io and io.open and os and os.remove and os.execute) and Test.It or Test.Skip

    with_files("serves static files safely", function()
        local directory = "quark-static-test"
        os.execute("mkdir -p " .. directory .. "/js")
        local function write(path, content)
            local file = assert(io.open(directory .. "/" .. path, "wb"))
            file:write(content)
            file:close()
        end
        write("index.html", "<h1>Quark</h1>")
        write("js/app.js", "console.log(1)")

        local server = assert(HTTP.Listen(0, { Host = "127.0.0.1" }))
        server:Static("/assets", directory, { MaxAge = 60 })

        local index = Quark.Await(HTTP.Get, base(server) .. "/assets/")
        Expect(index.Body):ToBe("<h1>Quark</h1>")
        Expect(index.Headers["content-type"]):ToBe("text/html; charset=utf-8")
        Expect(index.Headers["cache-control"]):ToBe("public, max-age=60")

        local script = Quark.Await(HTTP.Get, base(server) .. "/assets/js/app.js")
        Expect(script.Headers["content-type"]):ToBe("text/javascript; charset=utf-8")
        Expect(script.Body):ToBe("console.log(1)")

        Expect(Quark.Await(HTTP.Get, base(server) .. "/assets/missing.txt").Status):ToBe(404)
        Expect(Quark.Await(HTTP.Get, base(server) .. "/assets/%2e%2e/Cargo.toml").Status):ToBe(404)
        Expect(Quark.Await(HTTP.Get, base(server) .. "/assets/js/..%2f..%2fCargo.toml").Status):ToBe(404)
        server:Close()

        os.remove(directory .. "/js/app.js")
        os.remove(directory .. "/js")
        os.remove(directory .. "/index.html")
        os.remove(directory)
        Expect(function() server:Static("assets", directory) end):ToThrow("have to start with /")
    end)

    Test.It("upgrades WebSocket routes behind middleware", function()
        local server = assert(HTTP.Listen(0, { Host = "127.0.0.1" }))
        server:Use(function(request, response, next)
            if request.IsWebSocket and request.Query.token ~= "letmein" then
                return response:Text("Unauthorized", 401)
            end
            next()
        end)
        server:WebSocket("/live/:room", function(socket, request)
            socket:Send("joined " .. request.Params.room)
            socket:Subscribe("Message", function(data) socket:Send("echo " .. data) end)
        end)
        server:Get("/live/:room", function(_, response) response:Text("use a WebSocket") end)

        local url = "ws://127.0.0.1:" .. server:GetPort() .. "/live/lobby"
        local client = Quark.WebSocket.Connect(url .. "?token=letmein")
        local messages = collect(client, "Message", 2)
        Quark.Await(next_event(client, "Open"))
        client:Send("hi")
        Expect(Quark.Await(messages)):ToEqual({ "joined lobby", "echo hi" })

        local rejected = Quark.WebSocket.Connect(url .. "?token=wrong")
        Expect(Quark.Await(next_event(rejected, "Error"))):ToContain("401")

        Expect(Quark.Await(HTTP.Get, base(server) .. "/live/lobby").Body):ToBe("use a WebSocket")
        client:Close()
        server:Close()
    end)
end)
