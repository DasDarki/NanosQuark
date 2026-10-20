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
        local ok, reason = pcall(Quark.Await, Net.Lookup, "does-not-exist.invalid")
        Expect(ok):ToBe(false)
        Expect(reason):ToContain("cannot look up A records")
        Expect(function() Net.Lookup("example.com", "SOA") end):ToThrow("unsupported record type")
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
