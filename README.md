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

Async work belongs to the package that started it. Callbacks of a package that was unloaded or reloaded in the meantime are dropped.

### Errors

Errors raised by Quark functions are plain strings with the script location, just like errors from Lua itself:

```
[my-package/Server/Index.lua]:18: Invalid symbol 33, offset 0.
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
├── quark-core     module registry, lazy imports, error boundary, event pump, worker runtime
└── quark-crypto   Crypto module
quark/             the cdylib (luaopen_quark), selects modules through Cargo features
```

A module is a function `fn(&Lua) -> LuaResult<LuaTable>` registered in [quark/src/lib.rs](quark/src/lib.rs). It uses [mlua](https://docs.rs/mlua) directly. `quark-core` provides:

| Item | Purpose |
| --- | --- |
| `run` | run work asynchronously when a callback is given and synchronously otherwise, the standard for slow functions |
| `spawn_blocking`, `spawn_future` | run CPU bound work or async I/O on the worker runtime and call a Lua callback with the result on the game thread |
| `Options`, `options_and_callback` | read optional options tables with clear errors, accept `(options?, callback?)` argument pairs |
| `Bytes`, `Object` | return binary strings and protected userdata, also from worker threads |
| `Dispatcher` | post arbitrary jobs to the game thread, for example events of a socket |
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

## License

MIT. The bundled `sdk/windows/lua.lib` is part of the [nanos world module SDK](https://github.com/nanos-world/module-sdk), see [sdk/LICENSE.txt](sdk/LICENSE.txt).
