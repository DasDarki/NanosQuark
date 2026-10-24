# Quark

A batteries included utility library for the nanos world server, written in Rust. Quark adds what the built-in scripting API does not cover: cryptography, networking, data formats, compression, fast native computation and more.

```lua
local Crypto = Quark.Import("Crypto")

Console.Log(Crypto.SHA256("abc"))

Crypto.Argon2Hash("hunter2", function(hash, error)
    Console.Log(hash)
end)
```

## Usage

1. Copy the `quark` package (see [Packaging](#packaging)) into the server's `Packages` folder.
2. Add `"quark"` to `packages_requirements` of every package that uses it.
3. Start the server with `--enable_unsafe_libs`, otherwise nanos world refuses to load c-module packages.

### Modules

Everything lives in the global `Quark`. Modules are built on first access, so unused modules cost nothing at runtime:

```lua
local Crypto = Quark.Import("Crypto")  -- errors if the module does not exist
local Same = Quark.Crypto               -- lazy access, nil if the module does not exist

Console.Log(Quark.Version)
Console.Log(table.concat(Quark.Modules(), ", "))
```

Modules can also be removed at compile time with Cargo features (`cargo build --no-default-features --features crypto`).

| Module | Reference |
| --- | --- |
| `Crypto` | [docs/crypto.md](docs/crypto.md): hashing, HMAC, key derivation, password hashing, encryption, signatures, JWT, one time passwords, secure randomness |
| `Buffer` | [docs/buffer.md](docs/buffer.md): mutable byte arrays with typed number access, accepted wherever binary data is expected |
| `Promise` | [docs/promise.md](docs/promise.md): promises, `Quark.Async` / `Quark.Await`, `Quark.Delay` |
| `JSON`, `TOML`, `YAML`, `MessagePack`, `CBOR` | [docs/formats.md](docs/formats.md#data-formats): encoding and decoding, optionally off the game thread, JSON Schema validation |
| `XML`, `INI` | [docs/formats.md](docs/formats.md#xml): element trees and configuration files |
| `CSV` | [docs/formats.md](docs/formats.md#csv): tables with and without header |
| `UUID` | [docs/formats.md](docs/formats.md#uuid): versions 4 and 7, parsing, timestamps |
| `URL` | [docs/formats.md](docs/formats.md#url): percent encoding, query strings, parsing |
| `Compression`, `Zip`, `Tar` | [docs/compression.md](docs/compression.md): LZ4, Zstandard, Gzip, Deflate, Zlib, Brotli and archives in memory |
| `Net`, `TCP`, `UDP`, `WebSocket`, `HTTP` | [docs/net.md](docs/net.md): DNS, IP ranges, TCP and UDP sockets, WebSocket clients and servers, HTTP servers with routing, async handlers, WebSocket routes and static files, HTTP client, TLS for clients and servers |
| `Math`, `Geometry`, `BigInt`, `Decimal` | [docs/math.md](docs/math.md): easing, curves, quaternions, statistics, safe formulas, 2D and 3D shapes with random points, polygons, big integers and exact decimals |
| `Pathfinding`, `Spatial` | [docs/spatial.md](docs/spatial.md): A*, Dijkstra and flow fields on grids, navigation graphs, spatial hashing, KD trees and bulk proximity queries |
| `ProcGen` | [docs/procgen.md](docs/procgen.md): caves, dungeons and heightmaps |
| `Random` | [docs/random.md](docs/random.md): seeded generators, distributions, weighted picking, loot tables with pity, dice and noise |
| `MQTT`, `NATS` | [docs/messaging.md](docs/messaging.md): MQTT and NATS clients with automatic reconnects, queue groups and request/reply |
| `Redis` | [docs/redis.md](docs/redis.md): Redis client with commands, pipelines, transactions, pub/sub, streams and cluster support |
| `Regex`, `Text`, `Template` | [docs/text.md](docs/text.md): regular expressions, Unicode aware text helpers, fuzzy matching, word filters and Jinja templates |
| `Time` | [docs/time.md](docs/time.md): dates with time zones, parsing and formatting, durations, cron expressions and schedules |
| `Locale` | [docs/locale.md](docs/locale.md): translations with Fluent, plural rules and language negotiation |
| `RateLimit`, `Cache`, `PriorityQueue`, `RingBuffer`, `Bitset`, `Metrics`, `Log` | [docs/utilities.md](docs/utilities.md): rate limiting, LRU and TTL caches, data structures, Prometheus metrics and structured logging |
| `Testing` | [docs/testing.md](docs/testing.md): unit tests with async support, expectations, spies and mocks |

### Configuration

Quark runs without configuration. Worker threads, the time budget per tick and network allow lists can be set in a `quark.toml` next to the server or with `Quark.Configure`, see [docs/configuration.md](docs/configuration.md).

### Async functions

Expensive functions (password hashing, key generation, heavy computation) take an optional callback as last argument. With a callback they run on native worker threads and the result comes back on the game thread, following the Lua convention `callback(result)` on success and `callback(nil, error)` on failure. Without a callback they run synchronously and return `result` or `nil, error`:

```lua
Crypto.Argon2Verify(password, stored_hash, function(valid, error)
    if error then
        Console.Error(error)
        return
    end
    Console.Log(valid and "welcome" or "wrong password")
end)
```

The same functions work with promises and coroutines, see [docs/promise.md](docs/promise.md):

```lua
Quark.Async(function()
    local valid = Quark.Await(Crypto.Argon2Verify, password, stored_hash)
end)
```

Async work belongs to the package that started it. Callbacks of a package that was unloaded or reloaded in the meantime are dropped.

Calls with a callback return a task. `task:Cancel()` stops waiting for the result: network requests and other asynchronous operations are aborted, work that already runs on a worker thread (like hashing) finishes in the background, but its callback is never called. `task:IsDone()` and `task:IsCancelled()` report the state.

```lua
local download = Quark.HTTP.Get(url, function(response) end)
download:Cancel()
```

### Durations

Options for times like `Timeout`, delays, TTLs and windows take milliseconds or a text with units: `"250ms"`, `"5s"`, `"1m 30s"`, `"2h"`, `"1d"`. Results are milliseconds unless a function says otherwise.

### Errors

Errors raised by Quark functions are plain strings with the script location, just like errors from Lua itself:

```
[my-package/Server/Index.lua]:18: Invalid symbol 33, offset 0.
```

### Editor support

The `library` folder contains [Lua Language Server](https://luals.github.io/) definitions for all modules, with parameter and return types and short descriptions. VS Code (with the Lua extension) and other editors using the Lua Language Server then complete Quark functions and warn about wrong arguments. Copy the folder into your project or point to it in a `.luarc.json` next to your packages:

```json
{
    "workspace.library": ["path/to/NanosQuark/library"]
}
```

## Building

```sh
cargo build --release
cargo xwin build --release --target x86_64-pc-windows-msvc
```

The Windows build can be done from Linux with [cargo-xwin](https://github.com/rust-cross/cargo-xwin). Only the MSVC target is supported, because the SDK's `lua.lib` is built with MSVC.

### Packaging

The package folder contains [packages/quark/Package.toml](packages/quark/Package.toml) and the binaries, named exactly as Cargo produces them:

```
Packages/quark/
├── Package.toml
├── libquark.so    target/release/libquark.so
└── quark.dll      target/x86_64-pc-windows-msvc/release/quark.dll
```

[packages/quark-test](packages/quark-test) is a script package exercising the modules, useful as a smoke test.

## Architecture

```
crates/
├── quark-compression  Compression, Zip and Tar modules
├── quark-core     module registry, lazy imports, error boundary, event pump, worker runtime, Buffer, Promise
├── quark-crypto   Crypto module
├── quark-net      Net, TCP, UDP, WebSocket and HTTP modules
├── quark-redis    Redis module
├── quark-formats  JSON, TOML, YAML, MessagePack, CBOR, CSV, UUID and URL modules
└── quark-testing  Testing module
quark/             the cdylib (luaopen_quark), selects modules through Cargo features
packages/          the c-module package and the test suite
tools/test-host/   runs a package outside of nanos world
```

A module is a function `fn(&Lua) -> LuaResult<LuaTable>` registered in [quark/src/lib.rs](quark/src/lib.rs). It uses [mlua](https://docs.rs/mlua) directly. `quark-core` provides:

| Item | Purpose |
| --- | --- |
| `run` | run work asynchronously when a callback is given and synchronously otherwise, the standard for slow functions |
| `spawn_blocking`, `spawn_future` | run CPU bound work or async I/O on the worker runtime and call a Lua callback with the result on the game thread |
| `Options`, `options_and_callback` | read optional options tables with clear errors, accept `(options?, callback?)` argument pairs |
| `Bytes`, `Object` | return binary strings and protected userdata, also from worker threads |
| `Binary` | accept a string or a `Buffer` as binary input |
| `promise` | the Promise module, for modules implementing promise based APIs |
| `package_environment` | the environment of the calling package |
| `Dispatcher` | post arbitrary jobs to the game thread, track resources that close when the package unloads |
| `Emitter`, `create_object`, `add_event_methods` | objects with `Subscribe` / `Unsubscribe` whose events are emitted from worker threads |
| `run_async` | like `run` for futures |
| `create_userdata` | create userdata whose methods raise string errors |
| `protect`, `protect_table` | wrap functions so errors become strings (applied to every module table automatically) |
| `report_error` | log an error through `Console.Error` |

### How nanos world loads modules

These details were verified against server version 1.160.0 and shape the design:

- All packages share one Lua state. Every package has its own global environment, the module's globals (`Quark`) live in the shared `_G`.
- The module is loaded once, `luaopen_quark` runs again on `package reload all`.
- `Server.Subscribe` attaches the subscription to the package of the calling Lua function, found through its `_ENV`. Calls without a Lua caller (for example from `luaopen_quark`) fail. The event pump therefore subscribes to `Tick` once per package environment that uses Quark, all subscriptions share one time budget per frame (4 ms).
- nanos world reads error messages with `lua_tostring`, so mlua's userdata errors show up as `Error (non-string type: userdata)`. Quark wraps every exported function in a small C trampoline that converts errors to strings.
- The binary is pinned in memory on load, so worker threads stay valid when nanos world unloads the package. Updating the binary requires a server restart.

## Tests

```sh
tools/test-host/run.sh
```

runs the test suite in [packages/quark-test](packages/quark-test) on Linux without a nanos world server, see [docs/testing.md](docs/testing.md#running-tests-without-a-server). The same package runs inside the server as well. Tests for external services run when the service is available and are skipped otherwise:

| Variable | Default | Tests |
| --- | --- | --- |
| `QUARK_REDIS_URL` | `redis://127.0.0.1:16379` | Redis, shared rate limits and caches |
| `QUARK_REDIS_CLUSTER` | ports 17000 to 17002 on localhost, comma separated | Redis Cluster |
| `QUARK_REDIS_SENTINELS` | ports 26390 to 26392 on localhost, comma separated, master `mymaster` | Redis Sentinel and failover |
| `QUARK_MQTT_URL` | none | MQTT |
| `QUARK_NATS_URL` | none | NATS |
| `QUARK_GEOIP_DIR` | `geoip-data` | GeoIP with the [MaxMind test databases](https://github.com/maxmind/MaxMind-DB/tree/main/test-data) |
| `QUARK_TEST_TMP` | `quark-test-logs` | folder for temporary files of the log and compression tests |

The definitions in `library` are checked against the built module with `tools/check-definitions`, a package for the test host. It reports module functions missing from the definitions and declared methods that do not exist:

```sh
target/test-host/host tools/test-host/host.lua target/release tools/check-definitions 30
```

## License

MIT. The bundled files in `sdk/` (`lua.lib`, `liblua.a` and the Lua headers) are part of the [nanos world module SDK](https://github.com/nanos-world/module-sdk), see [sdk/LICENSE.txt](sdk/LICENSE.txt).
