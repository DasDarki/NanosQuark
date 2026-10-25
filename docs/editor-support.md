# Editor support

## Lua Language Server

The `library` folder of the repository contains [Lua Language Server](https://luals.github.io/) definitions for all modules, with parameter and return types and short descriptions. VS Code (with the Lua extension) and other editors using the Lua Language Server then complete Quark functions and warn about wrong arguments. Copy the folder into your project or point to it in a `.luarc.json` next to your packages:

```json
{
    "workspace.library": ["path/to/NanosQuark/library"]
}
```

## Nebra

For [Nebra](https://nebra-lang.github.io) projects, `quark-types` is a types-only package with declarations generated from the same definitions:

```bash
nebra add @dasdarki/quark-types
```

It declares the global `Quark` with every module, object and operator, server side only:

```lua
local total = Quark.Decimal.New("19.99") * 3
local view = Quark.Buffer.New(16):View("f32")
view[0] = 1.5
local formula = Quark.Math.Compile("2 * x + 1")
local value = formula({ x = 3 })
```

It requires Nebra 0.5.12 or newer.
