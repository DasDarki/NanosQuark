# Basics

Conventions shared by all modules.

## Modules

Everything lives in the global `Quark`. Modules are built on first access, so unused modules cost nothing at runtime:

```lua
local Crypto = Quark.Import("Crypto")  -- errors if the module does not exist
local Same = Quark.Crypto               -- lazy access, nil if the module does not exist

Console.Log(Quark.Version)
Console.Log(table.concat(Quark.Modules(), ", "))
```

## Async functions

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

The same functions work with promises and coroutines, see [Promise](promise.md):

```lua
Quark.Async(function()
    local valid = Quark.Await(Crypto.Argon2Verify, password, stored_hash)
end)
```

Async work belongs to the package that started it. Callbacks of a package that was unloaded or reloaded in the meantime are dropped.

## Tasks

Calls with a callback return a task. `task:Cancel()` stops waiting for the result: network requests and other asynchronous operations are aborted, work that already runs on a worker thread (like hashing) finishes in the background, but its callback is never called. `task:IsDone()` and `task:IsCancelled()` report the state.

```lua
local download = Quark.HTTP.Get(url, function(response) end)
download:Cancel()
```

## Durations

Options for times like `Timeout`, delays, TTLs and windows take milliseconds or a text with units: `"250ms"`, `"5s"`, `"1m 30s"`, `"2h"`, `"1d"`. Results are milliseconds unless a function says otherwise.

## Indexes

Positions in buffers, images, grids and lists start at the [index base](buffer.md#index-base) of the package, 0 by default.

## Errors

Errors raised by Quark functions are plain strings with the script location, just like errors from Lua itself:

```
[my-package/Server/Index.lua]:18: Invalid symbol 33, offset 0.
```
