# Introduction

Quark is a batteries included utility library for the nanos world server, written in Rust. It adds what the built-in scripting API does not cover: cryptography, networking, data formats, compression, fast native computation and more, all from the global `Quark` in your server scripts.

```lua
local Crypto = Quark.Import("Crypto")

Console.Log(Crypto.SHA256("abc"))

Crypto.Argon2Hash("hunter2", function(hash, error)
    Console.Log(hash)
end)
```

Heavy work runs on native worker threads and the results come back on the game thread, so hashing passwords, generating terrain or answering HTTP requests does not stall the server.

## Modules

| Module | Covers |
| --- | --- |
| `Crypto` | [Hashing, HMAC, key derivation, password hashing, encryption, signatures, JWT, one time passwords, secure randomness](crypto.md) |
| `Buffer` | [Mutable byte arrays with typed number access, accepted wherever binary data is expected](buffer.md) |
| `Promise` | [Promises, `Quark.Async` / `Quark.Await`, `Quark.Delay`](promise.md) |
| `JSON`, `TOML`, `YAML`, `MessagePack`, `CBOR` | [Encoding and decoding, optionally off the game thread, JSON Schema validation](formats.md#data-formats) |
| `XML`, `INI`, `CSV`, `UUID`, `URL` | [Element trees, configuration files, tables, identifiers and URLs](formats.md) |
| `Compression`, `Zip`, `Tar` | [LZ4, Zstandard, Gzip, Deflate, Zlib, Brotli and archives in memory](compression.md) |
| `Net`, `TCP`, `UDP`, `WebSocket`, `HTTP` | [DNS, sockets, WebSocket clients and servers, HTTP servers and clients, TLS](net.md) |
| `MQTT`, `NATS` | [Messaging clients with automatic reconnects, queue groups and request/reply](messaging.md) |
| `Redis` | [Commands, pipelines, transactions, pub/sub, streams and cluster support](redis.md) |
| `Math`, `Geometry`, `BigInt`, `Decimal` | [Easing, curves, quaternions, statistics, formulas, 2D and 3D shapes, big integers and exact decimals](math.md) |
| `Pathfinding`, `Spatial` | [A*, Dijkstra and flow fields, navigation graphs, spatial hashing and KD trees](spatial.md) |
| `ProcGen` | [Caves, dungeons and heightmaps](procgen.md) |
| `Random` | [Seeded generators, distributions, weighted picking, loot tables, dice and noise](random.md) |
| `Image` | [PNG, JPEG, WebP and BMP images, drawing, resizing, minimaps](image.md) |
| `Worker` | [Sandboxed Lua states on separate threads for real parallel Lua](worker.md) |
| `Regex`, `Text`, `Template` | [Regular expressions, Unicode text helpers, fuzzy matching, word filters and Jinja templates](text.md) |
| `Time` | [Dates with time zones, durations, cron expressions and schedules](time.md) |
| `Locale` | [Translations with Fluent, plural rules and language negotiation](locale.md) |
| `RateLimit`, `Cache`, `PriorityQueue`, `RingBuffer`, `Bitset`, `Metrics`, `Log` | [Rate limiting, caches, data structures, Prometheus metrics and structured logging](utilities.md) |
| `Testing` | [Unit tests with async support, expectations, spies and mocks](testing.md) |

Start with the [installation](installation.md), then read the [basics](basics.md) shared by all modules.
