# Networking

All network work runs on native worker threads, events arrive on the game thread through the event pump. Sockets and servers belong to the package that created them and are closed automatically when that package unloads or reloads.

## Events

Sockets and servers are objects with events, following the nanos world style:

```lua
local socket = Quark.TCP.Connect("example.com", 443, { TLS = true })
socket:Subscribe("Connect", function() socket:Send("...") end)
socket:Subscribe("Data", function(data) Console.Log(data) end)
socket:Subscribe("Close", function(reason) Console.Log("closed: " .. reason) end)
```

| Method | |
| --- | --- |
| `:Subscribe(event, callback)` | returns the callback |
| `:Unsubscribe(event, callback?)` | removes one callback, or all callbacks of the event |
| `:GetState()` | `"connecting"`, `"open"`, `"closing"` or `"closed"` |
| `:IsOpen()` | whether the state is `"open"` |
| `:GetLocalAddress()` / `:GetRemoteAddress()` | `ip, port`, or `nil` while unknown |

Every object emits `Error(message)` for problems and exactly one `Close` at the end, also when connecting fails. A socket stays alive while it is open, even without a Lua reference.

## Net

| Function | |
| --- | --- |
| `Net.Resolve(host, callback?)` | list of IP addresses through the system resolver (including the hosts file) |
| `Net.Lookup(name, type?, callback?)` | DNS records, `type` is `A` (default), `AAAA`, `MX`, `TXT`, `SRV`, `CNAME`, `NS` or `PTR` |
| `Net.ReverseLookup(ip, callback?)` | host names of an IP address |
| `Net.IsIP(value)`, `Net.IsIPv4(text)`, `Net.IsIPv6(text)` | address checks |
| `Net.NormalizeIP(text)` | canonical form, IPv4 mapped IPv6 addresses become IPv4 |
| `Net.IsInRange(ip, range)` | whether the address is in a CIDR range like `10.0.0.0/8` or `2001:db8::/32` |
| `Net.IsPrivate(ip)` | private, carrier grade NAT, link local and unique local ranges |
| `Net.IsLoopback(ip)` | `127.0.0.0/8` and `::1` |

`MX` records are `{ Priority, Exchange }`, `SRV` records `{ Priority, Weight, Port, Target }`, all other records are strings. Lookups without a callback block until the DNS server answers, use a callback or `Quark.Await`.

```lua
if Quark.Net.IsInRange(player_ip, "10.0.0.0/8") then
    Console.Log("player from the local network")
end
```

## TCP

| Function | |
| --- | --- |
| `TCP.Connect(host, port, options?)` | socket in state `"connecting"`, emits `Connect`, `Data(data)`, `Error(message)`, `Close(reason)` |
| `TCP.Listen(port, options?)` | server, or `nil, message` when the port cannot be opened. Emits `Connection(socket)`, `Error(message)`, `Close(reason)` |

| Option | | |
| --- | --- | --- |
| `Framing` | both | `"raw"` (default): `Data` receives chunks as they arrive. `"line"`: one `Data` per line, `Send` appends the newline. `"length"`: every message is prefixed with its length as 32 bit big endian, `Send` adds the prefix |
| `MaxFrameSize` | both | largest line or length framed message (16 MiB) |
| `TLS`, `ServerName` | Connect | encrypt the connection, verify the certificate against the Mozilla root store, `ServerName` defaults to the host |
| `Timeout` | Connect | connect and TLS handshake timeout in milliseconds (10000) |
| `Host` | Listen | address to listen on (`"0.0.0.0"`, use `"127.0.0.1"` for local only access) |
| `MaxConnections` | Listen | further connections are dropped (1024) |

| Socket method | |
| --- | --- |
| `:Send(data)` | string or buffer, `false` when the socket is not open |
| `:Close()` | graceful shutdown |

| Server method | |
| --- | --- |
| `:GetPort()` | the port, useful with port `0` (picks a free port) |
| `:GetClients()` | open client sockets |
| `:Close()` | stops listening and closes all clients |

```lua
local server = assert(Quark.TCP.Listen(9000, { Framing = "line" }))
server:Subscribe("Connection", function(client)
    client:Subscribe("Data", function(line)
        client:Send("echo: " .. line)
    end)
end)
```

## UDP

`UDP.Bind(port?, options?)` returns a socket (or `nil, message`), port `0` or `nil` picks a free port. Option `Host` as for TCP.

| | |
| --- | --- |
| `:Send(data, host, port)` | send a datagram (at most 65507 bytes), the host can be a name |
| `Message(data, ip, port)` | event for every received datagram |
| `:Close()` | close the socket |

## WebSocket

| Function | |
| --- | --- |
| `WebSocket.Connect(url, options?)` | `ws://` or `wss://` client, emits `Open`, `Message(data, binary)`, `Error(message)`, `Close(code, reason)` |
| `WebSocket.Listen(port, options?)` | server, or `nil, message`. Emits `Connection(socket, request)`, `Error(message)`, `Close(reason)` |

| Option | | |
| --- | --- | --- |
| `Headers` | Connect | extra request headers |
| `Timeout` | Connect | connect and handshake timeout in milliseconds (10000) |
| `MaxMessageSize` | both | largest message (16 MiB) |
| `Host`, `MaxConnections` | Listen | as for TCP |
| `Path` | Listen | only accept this path, other paths get a 404 |

`request` is `{ Path, Query, Headers, RemoteAddress }`, header names are lowercase.

| Socket method | |
| --- | --- |
| `:Send(text)` / `:SendBinary(data)` | send a text or binary message, `false` when not open |
| `:Close(code?, reason?)` | close with a code between 1000 and 4999 (default 1000) |

| Server method | |
| --- | --- |
| `:Broadcast(data, binary?)` | send to all open clients, returns the number of clients |
| `:GetPort()`, `:GetClients()`, `:Close()` | as for TCP |

Pings are answered automatically. The close code is 1005 when the peer sent no code and 1006 when the connection broke without a close frame.

```lua
local server = assert(Quark.WebSocket.Listen(8080, { Path = "/live" }))
server:Subscribe("Connection", function(client, request)
    client:Send(Quark.JSON.Encode({ welcome = request.RemoteAddress }))
end)

Server.Subscribe("PlayerSpawn", function(player)
    server:Broadcast(Quark.JSON.Encode({ event = "spawn", name = player:GetName() }))
end)
```

## Security

Servers bind to all interfaces by default and are reachable from the internet if the firewall allows it. Validate everything received, limit message sizes with `MaxFrameSize` / `MaxMessageSize` and connection counts with `MaxConnections`. TLS is available for outgoing connections, for incoming TLS put a reverse proxy in front of the server.
