# quark-types

[Nebra](https://github.com/nebra-lang/nebra) type declarations for [Quark](../README.md), the utility library for the nanos world server.

Types-only package: it emits no Lua. The declarations are discovered from `nebra_modules/` once the package is installed, and depend on [nanos-world-types](https://github.com/nebra-lang/nanos-world-types) for `Vector`, `Rotator` and `Color`.

## Install

```bash
nebra add @dasdarki/quark-types
```

The Quark package itself still has to be installed on the server and listed in `packages_requirements`, see the [usage](../README.md#usage).

## What you get

The global `Quark` with every module, object and method, generated from the [LuaLS definitions](../library). `Quark` is marked `@side(server)`, so with a `[sides]` block in `nebra.toml` using it from a client file is a compile error.

```lua
local image = Quark.Image.New(256, 256, "#000000")
local width, height = image:GetSize()

Quark.Worker.Run("return ... * 2", 21, nil, function(value: any)
    print(value)
end)
```

Objects such as `QuarkImage` or `QuarkBigInt` are declared as classes, module tables and data shapes like options as interfaces. The operators Quark implements are declared as well:

```lua
local total = Quark.Decimal.New("19.99") * 3
local view = Quark.Buffer.New(16):View("f32")
view[0] = 1.5
local formula = Quark.Math.Compile("2 * x + 1")
local value = formula({ x = 3 })
```

Requires Nebra 0.5.13 or newer.

## Regenerating

The declarations are generated, do not edit `src/quark.d.neb` by hand. After changing the definitions in `library/`, run from the repository root:

```bash
python3 tools/nebra-types/generate.py
```

`python3 tools/nebra-types/generate.py --check` fails when the declarations are out of date.

## Publishing

The package lives in the `nebra` directory of the Quark repository, versions are the repository's `vX.Y.Z` tags:

```bash
cd nebra
nebra login
nebra publish --description "Nebra type declarations for Quark, the nanos world server utility library"
```
