# Ideas and Roadmap

Status: `[x]` done, `[ ]` planned. Function names are sketches, final APIs follow the nanos world naming style (PascalCase, static classes, callbacks last).

## Core

- [x] Lazy module registry (`Quark.Import`, `Quark.<Module>`, `Quark.Modules`)
- [x] Game thread event pump on `Tick`, one subscription per package, shared frame budget
- [x] Worker runtime (tokio) for blocking work and async I/O
- [x] String errors with script location for functions and userdata methods
- [x] Drop callbacks of unloaded packages
- [x] Pin the binary so worker threads survive package unloads
- [ ] Per package resource tracking: sockets, servers, timers and workers opened by a package are closed on its `Unload`
- [ ] `Quark.Configure({ TickBudget = 4, WorkerThreads = 4 })`, sensible defaults for game servers (do not use every core)
- [ ] Promise objects as an alternative to callbacks: `Crypto.Argon2Hash(password):Then(fn):Catch(fn)`
- [ ] Coroutine based await: `Quark.Async(function() local hash = Quark.Await(Crypto.Argon2Hash(password)) end)`
- [ ] Cancellation for long running tasks (`task:Cancel()`)
- [ ] `Quark.Buffer`: binary buffer / typed array userdata (u8, i32, f32, f64) passed to native functions without converting Lua tables, the key to fast bulk computation
- [ ] Lua Language Server definitions (`---@class`, `---@param`) generated from the Rust modules, for autocompletion in VS Code
- [ ] CI: build Linux and Windows, assemble the package, attach a zip to releases
- [ ] Test harness: standalone Lua host with `Server`/`Console`/`Package` stubs, so module tests run in CI without a nanos world server
- [ ] Optional allow lists for outgoing network targets and listening ports, since c-modules bypass the sandbox

## Crypto

- [x] SHA-256, SHA-512, incremental `Hasher`
- [x] Argon2id hash and verify (async)
- [x] Secure random bytes, Base64 (standard and URL safe)
- [ ] SHA-1, SHA-3, BLAKE3, MD5 (legacy interop only)
- [ ] HMAC (SHA-256/512), HKDF, PBKDF2, scrypt
- [ ] bcrypt hash and verify (async)
- [ ] AES-256-GCM and ChaCha20-Poly1305 encrypt and decrypt, key generation
- [ ] Signatures: Ed25519, ECDSA P-256, RSA (PKCS#1 v1.5 / PSS) sign and verify, PEM import and export
- [ ] X25519 key exchange
- [ ] JWT sign and verify (HS256, RS256, ES256, EdDSA)
- [ ] TOTP / HOTP for two factor authentication (admin panels)
- [ ] Constant time comparison
- [ ] Secure random integers, strings and tokens (`Crypto.RandomToken(32)`)

## Random

- [ ] Fast seeded generators (PCG, Xoshiro256++) as userdata: `local rng = Random.New(seed)`, reproducible across platforms
- [ ] Integer and float ranges, booleans with probability, shuffle, sample without replacement
- [ ] Distributions: normal, exponential, Poisson, triangular
- [ ] Weighted picker using the alias method, O(1) per pick: `Random.WeightedPicker({ common = 70, rare = 25, epic = 5 })`
- [ ] Loot tables with nested tables, guaranteed drops and pity counters
- [ ] Dice notation: `Random.Roll("2d6+3")`
- [ ] Noise: Perlin, Simplex, OpenSimplex2, fractal (fBm), single value and bulk into a `Quark.Buffer`

## Net

- [ ] DNS: resolve hostnames, reverse lookup, SRV / TXT / MX records (async)
- [ ] IP utilities: parse and validate IPv4 / IPv6, CIDR matching (`Net.IsInRange(ip, "10.0.0.0/8")`), private / loopback detection
- [ ] TCP client and server with events (`Connected`, `Data`, `Closed`), optional line or length prefixed framing
- [ ] UDP sockets, send and receive datagrams
- [ ] WebSocket client (ws / wss) with text and binary messages, ping / pong, reconnect
- [ ] WebSocket server, broadcast helpers, per connection data
- [ ] TLS for TCP and WebSocket (rustls)
- [ ] Optional: Redis client and pub/sub for multi server setups, MQTT / NATS messaging
- [ ] Optional: GeoIP lookup (MaxMind database) for player countries

## Formats

- [ ] JSON encode / decode with pretty printing and stable key order (faster than Lua implementations, handles big documents)
- [ ] TOML, YAML, XML, CSV, INI
- [ ] MessagePack, CBOR for compact network payloads
- [ ] UUID v4 and v7, parsing and validation
- [ ] Hex, Base32, Base58, URL / percent encoding
- [ ] Binary pack / unpack for protocols (`Format.Pack("<I4f", ...)`)
- [ ] JSON Schema validation for configs

## Compression

- [ ] LZ4, Zstandard, Gzip / Deflate, Brotli for strings and buffers (sync for small data, async for large)
- [ ] ZIP read and write (in memory and on disk)
- [ ] TAR read and write, `.tar.gz`

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
