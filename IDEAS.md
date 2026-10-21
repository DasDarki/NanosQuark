# Ideas and Roadmap

Status: `[x]` done, `[ ]` planned. Function names are sketches, final APIs follow the nanos world naming style (PascalCase, static classes, callbacks last).

## Core

- [x] Lazy module registry (`Quark.Import`, `Quark.<Module>`, `Quark.Modules`)
- [x] Game thread event pump on `Tick`, one subscription per package, shared frame budget
- [x] Worker runtime (tokio) for blocking work and async I/O
- [x] String errors with script location for functions and userdata methods
- [x] Drop callbacks of unloaded packages
- [x] Pin the binary so worker threads survive package unloads
- [x] One style for slow work: optional callback last, asynchronous with a callback and synchronous without
- [x] Async jobs belong to the package that started them, so nested async calls inside callbacks work
- [x] Per package resource tracking: sockets and servers opened by a package are closed on its `Unload`
- [x] Event objects (`Subscribe` / `Unsubscribe`) fed from worker threads
- [ ] `Quark.Configure({ TickBudget = 4, WorkerThreads = 4 })`, sensible defaults for game servers (do not use every core)
- [x] Promise objects with `Then`, `Catch`, `Finally`, `All`, `AllSettled`, `Race` and `From` for callback functions
- [x] Coroutine based await: `Quark.Async(function() local hash = Quark.Await(Crypto.Argon2Hash, password) end)`, `Quark.Delay`, `Quark.Clock`
- [ ] Cancellation for long running tasks (`task:Cancel()`)
- [x] `Quark.Buffer`: byte buffer with typed number access in both byte orders, accepted as binary input everywhere
- [ ] Typed views on buffers for bulk numeric work in native functions (`Buffer:View("f32")`)
- [ ] Lua Language Server definitions (`---@class`, `---@param`) generated from the Rust modules, for autocompletion in VS Code
- [ ] CI: build Linux and Windows, assemble the package, attach a zip to releases
- [ ] Binary size: the module is about 12 MB, try `opt-level = "s"`, fewer default modules or splitting rarely used modules into separate packages
- [x] Test harness: standalone Lua host with `Server`/`Console`/`Package` stubs, so module tests run in CI without a nanos world server
- [ ] Optional allow lists for outgoing network targets and listening ports, since c-modules bypass the sandbox

## Crypto

- [x] Hashes: MD5, SHA-1, SHA-2 (224/256/384/512), SHA-3 (256/384/512), BLAKE3, incremental `Hasher`
- [x] HMAC with every hash except BLAKE3, constant time `HMACVerify`
- [x] HKDF and PBKDF2
- [x] Password hashing: Argon2 (id, i, d), bcrypt, scrypt, `PasswordVerify` detecting the hash format
- [x] AES-GCM (128 and 256 bit) and XChaCha20-Poly1305 with random nonces and associated data
- [x] Key pairs: Ed25519, ECDSA P-256 and P-384, RSA (PKCS#1 v1.5 and PSS), X25519, PEM and raw import and export
- [x] Key exchange with X25519, P-256 and P-384
- [x] Sealed boxes (`Seal` / `Open`) for X25519, P-256 and P-384
- [x] RSA-OAEP encryption with the public key
- [x] Encrypted PKCS#8 private keys (PBES2 with scrypt and AES-256-CBC)
- [x] JWT sign, verify and decode (HS256/384/512, RS256/384/512, PS256/384/512, ES256, ES384, EdDSA) with expiry, issuer, audience and algorithm allow lists
- [x] TOTP and HOTP, secret generation and `otpauth://` URIs for authenticator apps
- [x] Secure random bytes, integers, strings and tokens, constant time comparison
- [x] Hex, Base64 (standard and URL safe) and Base32
- [ ] RSA decryption, blocked until the `rsa` crate is no longer affected by the Marvin timing attack (RUSTSEC-2023-0071)
- [ ] Switch to a stable `rsa` release once 0.10 is published (currently `0.10.0-rc.19`)

## Random

- [ ] Fast seeded generators (PCG, Xoshiro256++) as userdata: `local rng = Random.New(seed)`, reproducible across platforms
- [ ] Integer and float ranges, booleans with probability, shuffle, sample without replacement
- [ ] Distributions: normal, exponential, Poisson, triangular
- [ ] Weighted picker using the alias method, O(1) per pick: `Random.WeightedPicker({ common = 70, rare = 25, epic = 5 })`
- [ ] Loot tables with nested tables, guaranteed drops and pity counters
- [ ] Dice notation: `Random.Roll("2d6+3")`
- [ ] Noise: Perlin, Simplex, OpenSimplex2, fractal (fBm), single value and bulk into a `Quark.Buffer`

## Net

- [x] DNS: resolve host names, A, AAAA, MX, TXT, SRV, CNAME, NS and PTR records, reverse lookups
- [x] IP utilities: validation, normalization, CIDR matching, private and loopback detection
- [x] TCP client and server with events, raw, line and length prefixed framing
- [x] UDP sockets
- [x] WebSocket client (ws and wss) and server with path filter, request headers, broadcast and close codes
- [x] TLS for outgoing TCP and WebSocket connections (rustls with the Mozilla root store)
- [x] HTTP server with routing, path parameters, middleware, CORS and async handlers
- [x] HTTP server: WebSocket routes on the same port, static files, cookies, url encoded and multipart forms
- [x] HTTP client with redirects, timeouts, size limits and streaming
- [x] TLS for servers, custom root certificates, self signed certificates for development
- [x] WebSocket reconnect with exponential backoff and jitter
- [ ] HTTP/2, response compression, client certificates (mutual TLS)
- [ ] Optional: Redis client and pub/sub for multi server setups, MQTT / NATS messaging
- [ ] Optional: GeoIP lookup (MaxMind database) for player countries

## Formats

- [x] JSON encode / decode with pretty printing, sorted keys, null and empty array handling, optionally off the game thread
- [x] TOML, YAML
- [x] MessagePack, CBOR for compact network payloads, binary data preserved
- [x] CSV with and without header
- [x] UUID v4 and v7, parsing, validation, timestamps, binary form
- [x] URL / percent encoding, query strings, URL parsing and joining
- [x] Hex, Base32 and Base64 (in Crypto)
- [x] XML element trees, INI
- [x] Base58 (in Crypto)
- [x] JSON Schema validation without remote references

## Compression

- [x] LZ4, Zstandard, Gzip, Deflate, Zlib and Brotli for strings and buffers, levels, format detection, decompression limits
- [x] ZIP create, extract and list in memory, path sanitizing
- [x] TAR create, extract and list, `.tar.gz` and other compressed archives
- [ ] Streaming compression for data larger than memory
- [ ] Encrypted ZIP archives (AES)

## Math

- [ ] Vector, rotator and quaternion helpers compatible with the nanos world `Vector` / `Rotator` types
- [ ] Interpolation and easing functions, Bezier curves, Catmull-Rom splines
- [ ] Big integers and fixed point decimals (money without float errors)
- [ ] Statistics: mean, median, percentiles, standard deviation on large arrays
- [ ] Expression evaluation: `Math.Evaluate("2 * x + 1", { x = 3 })`, useful for configurable formulas
- [ ] Geometry: point in polygon, convex hull, segment intersection, triangulation

## Native computation

Lua code cannot run on worker threads, so heavy computation means native algorithms fed with data. Every algorithm gets a sync variant for small inputs and an async variant for large ones.

- [ ] Pathfinding: A*, Dijkstra and flow fields on grids and navigation graphs
- [ ] Spatial indexing: spatial hash and KD-tree for "players near this point" or nearest neighbour queries, rebuilt per tick from a buffer
- [ ] Bulk operations on buffers: sort, filter, distance checks many against many
- [ ] Procedural generation: heightmaps, cave generation, dungeon layouts, Poisson disk sampling for spawn points
- [ ] Image processing: decode, resize and encode PNG / JPEG, for example for generated minimaps
- [ ] Workers: a separate, sandboxed Lua state on a worker thread running self contained code, with messages passed by value (`Quark.Worker.Run(code, data, callback)`). No access to the nanos world API, but real parallel Lua

## Text and time

- [ ] Regex (match, captures, replace, split)
- [ ] Fuzzy matching: Levenshtein, Jaro-Winkler, for command suggestions and player name search
- [ ] Unicode aware upper / lower case, normalization, slugify, truncation by grapheme
- [ ] Templating (MiniJinja) for messages, emails, generated files
- [ ] Profanity filter with configurable word lists
- [ ] Date and time: ISO 8601 parsing and formatting, time zones, durations, high resolution monotonic clock
- [ ] Cron expressions: `Time.Schedule("0 */6 * * *", callback)`

## Server utilities

- [ ] Rate limiting: token bucket and sliding window per key, against spam and abuse
- [ ] LRU / TTL cache
- [ ] Data structures with native performance: priority queue, ring buffer, bitset
- [ ] Prometheus metrics endpoint and structured logging
- [ ] Localization: plural rules and message formatting (Fluent)
- [x] Buffers: create, read, write, slice, and copy operations for binary data
- [x] Converting numbers and strings to binary representations and back

## Testing Framework

- [x] Unit testing framework with groups, hooks, expectations, async tests, timeouts and reporting
- [x] Spies and mocks that restore themselves after each test
- [ ] Code coverage analysis to measure test effectiveness (needs a line hook, only feasible in the test host)
- [ ] Run the test host in CI on every push