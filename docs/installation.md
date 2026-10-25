# Installation

Quark is a c-module package: a native library that nanos world loads into the server. It runs on Linux and Windows servers.

## Adding Quark to a server

1. Download `quark-<version>.zip` from the [releases](https://github.com/DasDarki/NanosQuark/releases) and extract it into the `Packages` folder of the server, so that the server has a `Packages/quark` folder.
2. Add `"quark"` to `packages_requirements` in the `Package.toml` of every package that uses it.
3. Start the server with `--enable_unsafe_libs`, otherwise nanos world refuses to load c-module packages.

```toml
# Packages/my-package/Package.toml
[meta]
    title = "My Package"

[script]
    packages_requirements = ["quark"]
```

Every package that requires Quark finds the global `Quark` in its server scripts:

```lua
Console.Log("Quark " .. Quark.Version .. " with " .. table.concat(Quark.Modules(), ", "))
```

## The package folder

The package folder contains `Package.toml` and the binaries for both platforms:

```
Packages/quark/
├── Package.toml
├── libquark.so
└── quark.dll
```

The release also contains `quark-library-<version>.zip` with the [editor definitions](editor-support.md) and a `SHA256SUMS` file to verify the downloads.

## Building from source

Quark needs a current Rust toolchain:

```sh
cargo build --release
cargo xwin build --release --target x86_64-pc-windows-msvc
```

The Windows build can be done from Linux with [cargo-xwin](https://github.com/rust-cross/cargo-xwin). Only the MSVC target is supported, because the SDK's `lua.lib` is built with MSVC. The results are `target/release/libquark.so` and `target/x86_64-pc-windows-msvc/release/quark.dll`.

Modules can be left out at compile time with Cargo features, for example `cargo build --release --no-default-features --features crypto,formats`.
