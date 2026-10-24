# Worker

```lua
local Worker = Quark.Worker

local pool = Worker.Pool([[
    function Simulate(state, steps)
        for _ = 1, steps do
            for _, unit in ipairs(state.Units) do
                unit.X = unit.X + unit.SpeedX
                unit.Y = unit.Y + unit.SpeedY
            end
        end
        return state
    end
]], { Size = 2 })

pool:Call("Simulate", { Units = units }, 1000, function(state, error)
    if not state then
        return Console.Error(error)
    end
    ApplySimulation(state)
end)
```

Real parallel Lua: code runs in its own sandboxed Lua state on a separate thread, so heavy scripted work (simulations, generators, number crunching written in Lua) does not block the game thread.

A worker is fully isolated from the package that started it. It has no access to the nanos world API, to Quark, to globals of the package or to files. The standard libraries `string`, `table`, `math`, `utf8` and `coroutine` are available, plus `os.time`, `os.clock`, `os.date` and `os.difftime`. `print` writes to the server console with a `[Quark Worker]` prefix. `require`, `dofile`, `loadfile`, `collectgarbage`, `io` and `debug` do not exist.

## Values

Values passed to and returned from workers are copied: `nil`, booleans, numbers, strings and tables of these (nested up to 64 levels). Functions, userdata like buffers or nanos world entities and tables with cycles raise an error. Changes a worker makes to a table do not affect the original, return the table to get them back.

## Limits

| Option | Default | |
| --- | --- | --- |
| `Timeout` | `10000` | milliseconds or a duration text like `"5s"` a single run or call may take, then it fails with an error |
| `MemoryLimit` | 256 MiB | bytes one Lua state may allocate, at least 1 MiB |

## Running code once

| Function | |
| --- | --- |
| `Worker.Run(code, data?, options?, callback?)` | Runs `code` in a fresh state, `data` is passed as `...`. Returns the values the code returns, or `nil, error`. With a callback it runs on a worker thread and returns a [task](../README.md#async-functions) |

```lua
Worker.Run([[
    local size = ...
    local primes, sieve = {}, {}
    for number = 2, size do
        if not sieve[number] then
            primes[#primes + 1] = number
            for multiple = number * number, size, number do
                sieve[multiple] = true
            end
        end
    end
    return primes
]], 1000000, { Timeout = "5s" }, function(primes, error)
    print(primes and #primes or error)
end)
```

Every run creates a new Lua state, which costs well under a millisecond. For many small calls a pool is faster.

## Pools

`Worker.Pool(code, options?)` starts `Size` threads (default half of the CPU cores, at most 64), each with its own state that runs `code` once. Global functions defined by the code can then be called. Returns the pool, or `nil, error` if the code fails to load.

| Method | |
| --- | --- |
| `pool:Call(name, ..., callback)` | Calls the global function `name` on the next free worker, `callback(...)` receives the returned values or `nil, error`. Works with `Quark.Await(pool.Call, pool, name, ...)` |
| `pool:GetSize()` | Number of workers |
| `pool:GetPending()` | Calls that are queued or running |
| `pool:Close()` | Stops the workers. Calls that are running fail, queued calls are dropped. Pools close when the package unloads |

The states of a pool keep their globals between calls, so caches or loaded data survive, but each call may run on a different worker, so do not rely on state written by an earlier call.
