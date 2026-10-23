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
| `Net.CreateSelfSignedCertificate(names)` | `certificate, private_key` as PEM for one host name or a list, for development and tests |
| `Net.CreateCertificateAuthority(name?, options?)` | `certificate, private_key` of a new certificate authority, option `Days` (3650) |
| `Net.CreateCertificate(names, options?)` | `certificate, private_key` signed by `CA = { Certificate, PrivateKey }` (self signed without), options `Client` for client certificates and `Days` (365) |

`MX` records are `{ Priority, Exchange }`, `SRV` records `{ Priority, Weight, Port, Target }`, all other records are strings. Lookups without a callback block until the DNS server answers, use a callback or `Quark.Await`.

```lua
if Quark.Net.IsInRange(player_ip, "10.0.0.0/8") then
    Console.Log("player from the local network")
end
```

### GeoIP

`Net.GeoIP(path, options?, callback?)` opens a MaxMind database (`.mmdb`), for example the free GeoLite2 Country, City or ASN databases from [maxmind.com](https://dev.maxmind.com/geoip/geolite2-free-geolocation-data) or compatible databases like DB-IP. It returns a database object or `nil, message`. The file is read into memory once, with a callback on a worker thread. Option `Language` (`"en"`) selects the language of names, falling back to English.

| Method | |
| --- | --- |
| `:Lookup(ip)` | information about the address, `nil` when the database has nothing for it, `nil, message` for invalid addresses |
| `:GetCountry(ip)` | only the ISO country code like `"DE"`, or `nil` |
| `:LookupRaw(ip)` | the complete record as stored in the database |
| `:GetMetadata()` | `{ DatabaseType, BuildEpoch, IpVersion, NodeCount, Languages }` |

Depending on the database, results contain `Country`, `RegisteredCountry` and `Continent` (each `{ Code, Name, GeoNameID }`), `City`, `Subdivisions` (list of `{ Code, Name }`), `PostalCode`, `Location` (`{ Latitude, Longitude, AccuracyRadius, TimeZone }`), `ASN` and `Organization`, and `Network`, the range the record belongs to.

```lua
local geoip = assert(Quark.Net.GeoIP("Packages/my-package/GeoLite2-Country.mmdb"))

Player.Subscribe("Spawn", function(player)
    local country = geoip:GetCountry(player:GetIP())
    Chat.BroadcastMessage(player:GetName() .. " joined from " .. (country or "somewhere"))
end)
```

Lookups take microseconds and do not block. Location data is approximate, often only accurate to the country or region, and must not be used to identify individuals. Check the license of the database, GeoLite2 requires an attribution.

## TLS

Outgoing connections verify the server certificate against the Mozilla root store. Servers take their certificate as PEM text, for example read from a file of the package:

| Option | | |
| --- | --- | --- |
| `TLS = true` | TCP.Connect | encrypt the connection (`wss://` and `https://` URLs always do) |
| `ServerName` | TCP.Connect | name the certificate has to be valid for, defaults to the host |
| `CA` | clients | additional trusted certificates as PEM, for self signed or internal certificates |
| `Certificate`, `PrivateKey` | clients | client certificate for servers that require one (mutual TLS) |
| `TLS = { Certificate, PrivateKey }` | servers | serve TLS (HTTPS, WSS, TLS over TCP), the certificate may contain the whole chain |
| `ClientCA`, `RequireClientCertificate` | servers, inside `TLS` | only accept clients with a certificate signed by these CA certificates (PEM). With `RequireClientCertificate = false` clients without a certificate are accepted too |

```lua
local certificate, private_key = Quark.Net.CreateSelfSignedCertificate("localhost")
local server = assert(Quark.HTTP.Listen(8443, { TLS = { Certificate = certificate, PrivateKey = private_key } }))

local response = Quark.Await(Quark.HTTP.Get, "https://localhost:8443/", { CA = certificate })
```

TLS 1.2 and 1.3 are supported, handshakes time out after 10 seconds. Certificates that do not match their private key are rejected when the server starts.

### Mutual TLS

With mutual TLS both sides prove who they are, for example game servers talking to a central backend without passwords. Servers with `ClientCA` read the client certificate of every connection: `connection:GetPeerCertificate()` on TCP and WebSocket connections and `request.ClientCertificate` in HTTP handlers return `{ Subject, Issuer, Fingerprint, NotAfter, PEM }`, or `nil` without a certificate. `Subject` and `Issuer` are the common names, `Fingerprint` the SHA-256 hash of the certificate in hex, `NotAfter` the expiry as Unix time.

```lua
local ca_certificate, ca_key = Quark.Net.CreateCertificateAuthority("My Network")
local ca = { Certificate = ca_certificate, PrivateKey = ca_key }
local server_certificate, server_key = Quark.Net.CreateCertificate("backend.example.com", { CA = ca })
local client_certificate, client_key = Quark.Net.CreateCertificate("game-server-1", { CA = ca, Client = true })

local backend = Quark.HTTP.Listen(8443, {
    TLS = { Certificate = server_certificate, PrivateKey = server_key, ClientCA = ca_certificate },
})
backend:Get("/whoami", function(request, response)
    response:Text(request.ClientCertificate.Subject)
end)

Quark.HTTP.Get("https://backend.example.com:8443/whoami", {
    CA = ca_certificate,
    Certificate = client_certificate,
    PrivateKey = client_key,
}, function(response) end)
```

Keep the CA private key offline or at least out of the game server, it can create certificates for anyone.

## TCP

| Function | |
| --- | --- |
| `TCP.Connect(host, port, options?)` | socket in state `"connecting"`, emits `Connect`, `Data(data)`, `Error(message)`, `Close(reason)` |
| `TCP.Listen(port, options?)` | server, or `nil, message` when the port cannot be opened. Emits `Connection(socket)`, `Error(message)`, `Close(reason)` |

| Option | | |
| --- | --- | --- |
| `Framing` | both | `"raw"` (default): `Data` receives chunks as they arrive. `"line"`: one `Data` per line, `Send` appends the newline. `"length"`: every message is prefixed with its length as 32 bit big endian, `Send` adds the prefix |
| `MaxFrameSize` | both | largest line or length framed message (16 MiB) |
| `TLS`, `ServerName`, `CA` | Connect | encryption, see [TLS](#tls) |
| `Timeout` | Connect | connect and TLS handshake timeout in milliseconds (10000) |
| `TLS` | Listen | certificate and key, see [TLS](#tls) |
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
| `WebSocket.Connect(url, options?)` | `ws://` or `wss://` client, emits `Connect`, `Message(data, binary)`, `Error(message)`, `Close(code, reason)` |
| `WebSocket.Listen(port, options?)` | server, or `nil, message`. Emits `Connection(socket, request)`, `Error(message)`, `Close(reason)` |

| Option | | |
| --- | --- | --- |
| `Headers` | Connect | extra request headers |
| `Timeout` | Connect | connect and handshake timeout in milliseconds (10000) |
| `CA` | Connect | additional trusted certificates for `wss://`, see [TLS](#tls) |
| `Reconnect` | Connect | reconnect after connection losses, see below |
| `TLS` | Listen | certificate and key, see [TLS](#tls) |
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

### Reconnecting

With `Reconnect = true` or a table of settings a client reconnects whenever the connection is lost or cannot be established:

| Setting | |
| --- | --- |
| `InitialDelay` | delay before the first attempt in milliseconds (1000) |
| `MaxDelay` | upper limit of the delay (30000) |
| `Multiplier` | growth of the delay per attempt (2) |
| `MaxAttempts` | give up after this many attempts in a row, unlimited by default |

Delays vary randomly by up to 20 percent so that many clients do not reconnect at the same moment. A connection loss emits `Disconnect(code, reason)` instead of `Close`, every attempt emits `Reconnect(attempt, delay)` and a successful attempt emits `Connect` again. Messages sent while reconnecting are queued and sent once the connection is back. `Close` is only emitted at the very end: after `:Close()`, when the package unloads or when `MaxAttempts` is exceeded.

```lua
local socket = Quark.WebSocket.Connect("wss://events.example.com", { Reconnect = { MaxDelay = 10000 } })
socket:Subscribe("Connect", function() socket:Send("subscribe") end)
socket:Subscribe("Disconnect", function(code) Console.Warn("lost connection: " .. code) end)
```

```lua
local server = assert(Quark.WebSocket.Listen(8080, { Path = "/live" }))
server:Subscribe("Connection", function(client, request)
    client:Send(Quark.JSON.Encode({ welcome = request.RemoteAddress }))
end)

Server.Subscribe("PlayerSpawn", function(player)
    server:Broadcast(Quark.JSON.Encode({ event = "spawn", name = player:GetName() }))
end)
```

## HTTP

```lua
local server = assert(Quark.HTTP.Listen(8080))
server:Use(Quark.HTTP.Cors({ Origin = "*" }))

server:Get("/players/:id", function(request, response)
    local player = Quark.Await(LoadPlayer, request.Params.id)
    if not player then
        return response:Json({ error = "not found" }, 404)
    end
    response:Json(player)
end)

server:Post("/login", function(request, response)
    local body = request:Json()
    local valid = Quark.Await(Quark.Crypto.PasswordVerify, body.password, stored_hash)
    response:Status(valid and 200 or 401):Json({ ok = valid })
end)
```

Every request runs in its own `Quark.Async` coroutine, so handlers can `Quark.Await` database queries, password hashing or other requests while the server keeps serving. A handler has to send the response before it returns, either directly or after awaiting.

`HTTP.Listen(port, options?)` returns a server or `nil, message`.

| Option | |
| --- | --- |
| `Host` | address to listen on (`"0.0.0.0"`) |
| `MaxBodySize` | larger request bodies are answered with 413 (1 MiB) |
| `Timeout` | requests without a response after this many milliseconds are answered with 504 (30000) |
| `MaxConnections` | further connections are dropped (1024) |
| `MaxMessageSize` | largest WebSocket message on WebSocket routes (16 MiB) |
| `HTTP2` | also speak HTTP/2 (`true`), negotiated through TLS for HTTPS, plain HTTP/2 needs a client that starts with it |
| `TLS` | serve HTTPS, see [TLS](#tls) |
| `Compression` | compress responses, see [Compression](#compression) |

| Server method | |
| --- | --- |
| `:Get(pattern, handler)`, `:Post`, `:Put`, `:Patch`, `:Delete`, `:Options` | add a route, returns the server for chaining |
| `:All(pattern, handler)` | route for every method |
| `:Route(method, pattern, handler)` | route for any method name |
| `:WebSocket(pattern, handler)` | WebSocket route on the same port, see below |
| `:Static(prefix, directory, options?)` | serve files, see below |
| `:Use(middleware)` | add a middleware `function(request, response, next)` |
| `:GetPort()`, `:Close()` | as for TCP |

Patterns start with `/`. `:name` captures a segment into `request.Params.name`, a trailing `*` captures the rest of the path into `request.Params["*"]`. Routes are tried in the order they were added, captured segments are percent decoded. `GET` routes also answer `HEAD` requests without a body.

| Request field | |
| --- | --- |
| `Method`, `Path` | `"GET"`, `"/players/42"` |
| `Params` | captured path segments |
| `Query`, `RawQuery` | decoded query parameters and the raw query string |
| `Headers` | lowercase header names, repeated headers joined with `, ` |
| `Body` | raw body |
| `Cookies` | cookies sent by the client, values decoded |
| `RemoteAddress` | IP address of the client |
| `IsWebSocket` | whether the request asks for a WebSocket upgrade |
| `Version` | `"HTTP/1.1"` or `"HTTP/2"` |
| `ClientCertificate` | the client certificate with [mutual TLS](#mutual-tls), or `nil` |
| `:Json()` | decoded body, or `nil, message` |
| `:Form()` | `fields, files` of an `application/x-www-form-urlencoded` or `multipart/form-data` body, or `nil, message`. Files are `{ Field, FileName, ContentType, Data }` |
| `:GetHeader(name)` | header value, case insensitive |

| Response method | |
| --- | --- |
| `:Status(code)` / `:Header(name, value)` | set the status (default 200) or a header, chainable |
| `:Json(value, status?)` | send JSON |
| `:Text(text, status?)` / `:Html(html, status?)` | send text or HTML |
| `:Send(body?, content_type?, status?)` | send any body, the type defaults to text for UTF-8 and binary otherwise |
| `:Redirect(location, status?)` | redirect, 302 by default |
| `:SendFile(path, options?)` | send a file, read on a worker thread. Options `MaxAge` (cache seconds), `ContentType`, `MaxFileSize` (64 MiB). Missing files become 404 |
| `:Cookie(name, value, options?)` | set a cookie, chainable. Options `MaxAge`, `Path` (`"/"`), `Domain`, `Secure`, `HttpOnly` (`true`), `SameSite` (`"Lax"`, `"Strict"` or `"None"`, which requires `Secure`) |
| `:ClearCookie(name, options?)` | delete a cookie, use the same `Path` and `Domain` as when setting it |
| `:Compress(enabled?)` | allow or prevent compression of this response, chainable |
| `:IsSent()`, `:GetStatus()` | state of the response |

A response can be sent once, a second send raises an error. Unknown paths are answered with 404, known paths with the wrong method with 405 and an `Allow` header. Errors in handlers and handlers that return without responding produce a 500 and are logged with `Console.Error`, the error message is never sent to the client.

### Compression

With `Compression = true` responses are compressed with Brotli, Zstandard or gzip, whichever the client prefers in its `Accept-Encoding` header. Only text like HTML, CSS, JavaScript, JSON, XML and SVG of at least `MinSize` bytes is compressed, already compressed formats like images and archives are sent as they are. A table configures it:

| Option | |
| --- | --- |
| `Encodings` | allowed encodings in order of preference, ties in `Accept-Encoding` go to the first (`{ "br", "zstd", "gzip" }`) |
| `MinSize` | smaller bodies are sent uncompressed (1024) |
| `Level` | compression level, lower is faster (5 for Brotli, 3 for Zstandard, 6 for gzip) |

Compression runs on a worker thread. Responses that already have a `Content-Encoding` header and responses with `:Compress(false)` stay uncompressed.

### WebSocket routes

`server:WebSocket(pattern, handler)` accepts WebSocket connections on the HTTP port. The handler receives the same socket object as `WebSocket.Listen` and the request, it runs in a coroutine like route handlers:

```lua
server:WebSocket("/live/:room", function(socket, request)
    socket:Send("welcome to " .. request.Params.room)
    socket:Subscribe("Message", function(data) Console.Log(data) end)
end)
```

Middleware runs before the upgrade, so authentication and CORS apply to WebSocket routes as well. A middleware can refuse the upgrade by sending a response, `request.IsWebSocket` tells upgrade requests apart. A normal `GET` on the same path is routed to `GET` routes.

### Static files

`server:Static("/assets", "Packages/my-package/web")` serves the files of a directory below a prefix. The directory is relative to the working directory of the server. A request for a directory serves its `index.html` (option `Index`), the other options are those of `SendFile`. Paths containing `..`, absolute paths and symbolic links pointing outside of the directory are answered with 404.

### Middleware

Middleware runs before the route for every request, also for 404 responses. Calling `next()` continues with the next middleware or the route and returns when they are done, so code after `next()` runs afterwards. Sending a response without calling `next()` ends the request early:

```lua
server:Use(function(request, response, next)
    local token = (request.Headers.authorization or ""):match("^Bearer (.+)$")
    local claims = token and Quark.Crypto.JWTVerify(token, secret)
    if not claims then
        return response:Json({ error = "unauthorized" }, 401)
    end
    request.User = claims
    next()
end)
```

`HTTP.Cors(options?)` creates a middleware for cross origin requests and answers preflight requests:

| Option | |
| --- | --- |
| `Origin` | `"*"` (default), one origin or a list of origins |
| `Methods`, `Headers` | allowed methods and request headers |
| `Credentials` | allow cookies and authorization headers, needs explicit origins |
| `MaxAge` | how long browsers cache the preflight, in seconds (600) |

### Compared to the nanos world HTTP server

nanos world has its own HTTP server on the game port, enabled with `--enable_http_server`, with one server for the whole game server. Quark servers run on their own ports, every package can have its own, and handlers can wait for asynchronous work.

## HTTP client

| Function | |
| --- | --- |
| `HTTP.Request(options, callback?)` | send a request, `options.URL` is required |
| `HTTP.Get(url, options?, callback?)` | `GET` request |
| `HTTP.Post(url, body, options?, callback?)` | `POST` request, a table body is sent as JSON |
| `HTTP.Stream(options)` | streaming request object, see below |

| Option | |
| --- | --- |
| `URL`, `Method` | target and method (`"GET"`) |
| `Headers` | request headers |
| `Query` | query parameters appended to the URL |
| `Body` / `Json` | raw body (string or buffer) or a value sent as JSON |
| `Timeout` | for connecting and for the whole response in milliseconds (30000) |
| `MaxBodySize` | larger responses fail (16 MiB) |
| `FollowRedirects`, `MaxRedirects` | follow redirects (`true`, at most 5), 303 and redirected `POST`s continue as `GET` |
| `CA` | additional trusted certificates, see [TLS](#tls) |
| `Decompress` | ask for compressed responses and decode them (`true`) |
| `HTTP2` | use HTTP/2 when an HTTPS server supports it (`true`) |
| `Certificate`, `PrivateKey` | client certificate for [mutual TLS](#mutual-tls) |

The result is `{ Status, Version, Headers, Body, URL }` with the final URL after redirects, `:Json()` and `:IsSuccess()`. Error statuses like 404 are normal responses, only network problems, timeouts and size limits fail with `nil, message`.

```lua
local response = Quark.Await(Quark.HTTP.Get, "https://api.example.com/players", { Query = { page = 2 } })
if response:IsSuccess() then
    local players = response:Json()
end
```

Without a callback a request blocks the game thread until it finishes. Never send a blocking request to a Quark HTTP server in the same nanos world server: its handlers run on the blocked game thread, so the request waits until it times out. Use a callback or `Quark.Await`.

`HTTP.Stream(options)` takes the same options and returns an object with the events `Response(status, headers, url)`, `Data(chunk)`, `End`, `Error(message)` and `Close(reason)`, for downloads that should not be held in memory as a whole. `:Close()` aborts the download. Streams do not ask for compressed responses, chunks arrive as the server sends them.

With `Decompress` the client sends `Accept-Encoding: gzip, br, zstd, deflate` and decodes the body, removing `Content-Encoding` and `Content-Length` from the returned headers. `MaxBodySize` applies to the decoded body. A request with its own `Accept-Encoding` header gets the body exactly as the server sent it.

## Security

Servers bind to all interfaces by default and are reachable from the internet if the firewall allows it. Validate everything received, limit message sizes with `MaxFrameSize` / `MaxMessageSize`, bodies with `MaxBodySize` and connection counts with `MaxConnections`. Use TLS for servers reachable from the internet. Requests to URLs supplied by players can reach services in the local network of the server, check them with `Net.IsPrivate` and `Net.IsLoopback` after resolving.
