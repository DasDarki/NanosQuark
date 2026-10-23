# Server utilities

```lua
local chat_limit = Quark.RateLimit.TokenBucket({ Capacity = 5, Per = "10s" })

Chat.Subscribe("PlayerSubmit", function(message, player)
    local allowed, retry = chat_limit:Take(player)
    if not allowed then
        Chat.SendMessage(player, "Slow down, try again in " .. math.ceil(retry / 1000) .. "s")
        return false
    end
end)
```

Building blocks that most servers need sooner or later: `RateLimit`, `Cache`, `PriorityQueue`, `RingBuffer`, `Bitset`, `Metrics` and `Log`.

Durations in options are milliseconds or texts like `"10s"`, `"1m 30s"` or `"2h"`. Rate limits and caches accept any Lua value as key, also players and other entities.

## RateLimit

Rate limits protect against spam and abuse: chat messages, commands, login attempts, API requests. Both limiters return the same three values from `:Take`:

```lua
local allowed, retry_after, remaining = limiter:Take(key, cost?)
```

`retry_after` is the number of milliseconds until the request would be allowed (0 when allowed), `remaining` how much is left. `cost` (1) lets expensive actions use more of the limit.

### Token bucket

`RateLimit.TokenBucket(options)` allows bursts up to `Capacity` and refills continuously. Good for chat and actions where a short burst is fine but the average has to stay low.

| Option | |
| --- | --- |
| `Capacity` | maximum number of tokens, also the largest burst |
| `Per` | time to refill the whole bucket, `{ Capacity = 5, Per = "10s" }` allows 5 messages per 10 seconds |
| `Rate` | alternative to `Per`: tokens added per second |
| `Clock` | function returning seconds, for example to use game time (`Quark.Clock`) |

### Sliding window

`RateLimit.SlidingWindow(options)` allows at most `Limit` within any `Window`, for example 5 login attempts per 15 minutes. It is exact and remembers every request inside the window, so keep `Limit` reasonable.

| Option | |
| --- | --- |
| `Limit` | maximum within the window |
| `Window` | the window length |
| `Clock` | as above |

### Methods

| Method | |
| --- | --- |
| `:Take(key, cost?)` | consume if allowed |
| `:Check(key, cost?)` | same result without consuming |
| `:GetRemaining(key)` | what is left right now |
| `:Reset(key)`, `:Clear()` | forget one or all keys |
| `:Prune()`, `:GetCount()` | remove keys that are back at the full limit / number of tracked keys |

Keys that are back at their full limit are removed automatically from time to time, so limiters do not grow with every player that ever joined.

### Shared limits

`RateLimit.Redis(client, options)` keeps the limit in Redis, so it applies across all game servers and services using the same Redis, for example one chat limit per player on a whole network. It takes a [Redis client](redis.md) and either `Limit` and `Window` for a sliding window or `Capacity` with `Per` or `Rate` for a token bucket, plus `Prefix` for the key names (`"quark:ratelimit:"`). The time comes from the Redis server, so differences between server clocks do not matter.

| Method | |
| --- | --- |
| `:Take(key, cost?, callback?)` | `allowed, retry_after, remaining` like the local limiters |
| `:Check(key, cost?, callback?)` | the same without consuming |
| `:Reset(key, callback?)` | forget a key |

Keys are strings or numbers, use stable IDs like the Steam ID instead of player objects. Without a callback each call waits for Redis, which is usually well below a millisecond on the same network. Use a callback or `Quark.Await` for remote Redis servers. Errors from Redis return `nil, message`.

```lua
local logins = Quark.RateLimit.Redis(redis, { Limit = 5, Window = "15m" })

Quark.Async(function()
    local allowed, retry = Quark.Await(logins.Take, logins, player:GetSteamID())
    if not allowed then
        Chat.SendMessage(player, "Too many attempts, try again in " .. Quark.Time.FormatDuration(retry))
    end
end)
```

## Cache

`Cache.New(options?)` is a key value cache that removes the least recently used entries when it is full and expired entries when they are read.

| Option | |
| --- | --- |
| `Capacity` | maximum number of entries, unlimited without |
| `TTL` | default lifetime of entries, forever without |
| `OnEvict` | `function(key, value, reason)` with `Capacity`, `Expired`, `Replaced` or `Deleted` |
| `Clock` | function returning seconds |

| Method | |
| --- | --- |
| `:Get(key)` | value or `nil`, marks the entry as recently used |
| `:Peek(key)`, `:Has(key)` | read without marking |
| `:Set(key, value, ttl?)` | store, returns the cache. `ttl` overrides the default, `false` stores forever. `nil` deletes |
| `:GetOrSet(key, factory, ttl?)` | cached value, or calls `factory(key)` and stores its result |
| `:Delete(key)`, `:Clear()` | |
| `:Prune()` | removes expired entries, returns how many |
| `:GetTTL(key)` | milliseconds left, `-1` without expiry, `nil` when missing |
| `:GetSize()`, `:GetCapacity()`, `:GetKeys()` | keys are listed from most to least recently used |
| `:GetStats()` | `{ Hits, Misses, Evictions, HitRate }` |

```lua
local profiles = Quark.Cache.New({ Capacity = 500, TTL = "5m" })

local function GetProfile(steam_id)
    return profiles:GetOrSet(steam_id, LoadProfileFromDatabase)
end
```

`Cache.Redis(client, options?)` stores values in Redis as JSON, shared between all servers. Options `Prefix` (`"quark:cache:"`) and `TTL`. It has `:Get(key, callback?)`, `:Set(key, value, ttl?, callback?)`, `:GetOrSet(key, factory, ttl?, callback?)`, `:Has(key, callback?)` and `:Delete(key, callback?)`. Values have to be representable as JSON, keys are strings or numbers. Other services can read the values directly from Redis.

`Cache.Memoize(callback, options?)` returns a function that caches the results of `callback` by its single argument, and the cache itself.

## PriorityQueue

`PriorityQueue.New(options?)` returns the entry with the lowest priority first, or the highest with `{ Order = "Max" }`. Entries with the same priority come out in the order they were added. Push and pop take logarithmic time.

| Method | |
| --- | --- |
| `:Push(value, priority)` | add, returns the queue |
| `:Pop()`, `:Peek()` | `value, priority` of the next entry, `nil` when empty |
| `:Remove(value)` | removes the first entry with that value |
| `:GetSize()`, `:IsEmpty()`, `:Clear()` | |
| `:ToList()` | `{ Value, Priority }` entries in pop order, the queue stays unchanged |

```lua
local events = Quark.PriorityQueue.New()
events:Push(SpawnAirdrop, Quark.Clock() + 300)

Server.Subscribe("Tick", function()
    local next_event, time = events:Peek()
    while next_event and time <= Quark.Clock() do
        events:Pop()()
        next_event, time = events:Peek()
    end
end)
```

## RingBuffer

`RingBuffer.New(capacity)` keeps the last `capacity` values, for chat history, recent positions or moving averages. Index 1 is the oldest value, negative indices count from the newest.

| Method | |
| --- | --- |
| `:Push(value)` | append, returns the value that dropped out when full |
| `:Shift()` / `:Pop()` | remove the oldest / newest value |
| `:Get(index)`, `:Set(index, value)` | access by position |
| `:GetFirst()`, `:GetLast()` | oldest and newest value |
| `:GetSize()`, `:GetCapacity()`, `:IsEmpty()`, `:IsFull()`, `:Clear()` | |
| `:ToList()`, `:Values()` | values from oldest to newest, `Values` is an iterator for `for index, value in buffer:Values()` |

## Bitset

`Bitset.New(size)` stores `size` booleans in a compact native array, for visited cells, unlocked achievements, occupied slots or large flag sets. Indices use the [index base](buffer.md#index-base) of the package, like buffers (0 by default).

| Method | |
| --- | --- |
| `:Get(index)`, `:Set(index, value?)`, `:Unset(index)`, `:Toggle(index)` | single bits |
| `:SetAll(value?)`, `:Invert()` | all bits |
| `:Count()`, `:Any()`, `:None()`, `:All()` | |
| `:NextSet(start?)`, `:NextUnset(start?)` | index of the next bit with that value from `start` on, or `nil` |
| `:GetIndices()` | list of all set indices |
| `:Union(other)`, `:Intersect(other)`, `:Difference(other)`, `:SymmetricDifference(other)` | combine with a bitset of the same size, changes this one and returns it |
| `:Clone()`, `:Resize(size)`, `:GetSize()`, `#bitset` | |
| `:ToBytes()`, `Bitset.FromBytes(data, size?)` | compact binary form for saving, 1 byte per 8 bits |

`==` compares two bitsets, `Bitset.IsBitset(value)` checks the type.

## Metrics

Prometheus metrics, to watch servers with Grafana or any other Prometheus compatible tool.

```lua
local Metrics = Quark.Metrics

local players = Metrics.Gauge("nanos_players", "Connected players", {
    Collect = function() return #Player.GetAll() end,
})
local chat = Metrics.Counter("nanos_chat_messages_total", "Chat messages")
local tick = Metrics.Histogram("nanos_tick_seconds", "Time spent in the tick handler", { Buckets = { 0.001, 0.005, 0.01, 0.05 } })

Metrics.Expose(Quark.HTTP.Listen(9100))
```

`Metrics.Counter(name, help, options?)`, `Metrics.Gauge` and `Metrics.Histogram` create metrics in the default registry, `Metrics.Registry()` creates a separate one with the same methods. Registering a name again with the same type and labels returns the existing metric, so scripts can be reloaded.

| Option | |
| --- | --- |
| `Labels` | list of label names, values are passed as table when updating |
| `Buckets` | histogram upper bounds in ascending order (Prometheus defaults from 0.005 to 10) |
| `Collect` | gauges only: function called on every scrape, returns a number or a list of `{ Labels, Value }` |

| Metric | Methods |
| --- | --- |
| Counter | `:Inc(amount?, labels?)`, `:Get(labels?)`, `:Reset()` |
| Gauge | `:Set(value, labels?)`, `:Inc(amount?, labels?)`, `:Dec(amount?, labels?)`, `:Get(labels?)`, `:Reset()` |
| Histogram | `:Observe(value, labels?)`, `:Time(callback, labels?)` measuring the call in seconds and returning its results, `:Get(labels?)` returning `{ Count, Sum }`, `:Reset()` |

```lua
local requests = Metrics.Counter("api_requests_total", "API requests", { Labels = { "route", "status" } })
requests:Inc({ route = "/players", status = 200 })
```

| Registry function | |
| --- | --- |
| `Render()` | the metrics in the Prometheus text format |
| `Expose(server, path?)` | adds a `GET` route (`/metrics`) to a [Quark HTTP server](net.md#http) and returns the server |
| `Get(name)`, `Unregister(name)`, `Clear()` | |

Each label combination is a separate time series, keep labels to values with few variants (routes, status codes, map names), never player names or IDs.

## Log

`Log.New(options?)` creates a structured logger: messages with fields, written to the server console and optionally to rotating files.

```lua
local log = Quark.Log.New({ Name = "economy", File = "Logs/economy.log", Fields = { server = "eu-1" } })

log:Info("purchase", { player = player:GetName(), item = "rifle", price = 1200 })
-- console: [economy] purchase item=rifle player=Ada price=1200 server=eu-1
-- file:    2026-10-25T14:03:11.402+02:00 INFO  [economy] purchase item=rifle player=Ada price=1200 server=eu-1
```

| Option | |
| --- | --- |
| `Name` | shown in front of every message |
| `Level` | minimum level: `Debug`, `Info` (default), `Warn` or `Error` |
| `Format` | `Text` (default) or `Json`, one JSON object per line for log collectors |
| `Fields` | fields added to every message |
| `Console` | write to the console with `Console.Log`, `Console.Warn` and `Console.Error` (`true`) |
| `File` | also append to this file, the folder is created when needed |
| `MaxFileSize`, `MaxFiles` | rotate the file at this size in bytes (10 MiB), keeping this many old files as `.1`, `.2` and so on (5) |

| Method | |
| --- | --- |
| `:Debug(message, fields?)`, `:Info`, `:Warn`, `:Error` | write a message |
| `:Log(level, message, fields?)` | with the level as text |
| `:With(fields)` | a child logger that adds fields to every message, sharing the output |
| `:SetLevel(level)`, `:GetLevel()`, `:IsEnabled(level)` | |
| `:Flush()` | waits until everything is written to the file |

Files are written on a background thread, logging never waits for the disk. Files are closed when the package unloads. `Log.Debug`, `Log.Info`, `Log.Warn` and `Log.Error` write to the console without creating a logger.
