# Redis

```lua
local redis = Quark.Redis.Connect("redis://127.0.0.1:6379")

redis:Set("motd", "Welcome!")
redis:SortedSetIncrement("kills", player_id, 1)

Quark.Async(function()
    local top = Quark.Await(redis.SortedSetRange, redis, "kills", 0, 9, { Reverse = true, WithScores = true })
end)
```

Redis is a fast in memory database, useful for data shared between several nanos world servers or with web services: sessions, leaderboards, rate limits, caches, queues and messages between servers.

## Connecting

`Redis.Connect(url?, options?)` returns a client. The URL defaults to `redis://127.0.0.1:6379`, use `rediss://` for TLS and `redis://user:password@host:port/database` for credentials.

| Option | |
| --- | --- |
| `Username`, `Password`, `Database` | override the values of the URL |
| `Timeout` | connect and response timeout in milliseconds (5000) |
| `Retries` | how often a lost connection is retried before a command fails (6) |
| `CA` | trusted certificates as PEM for `rediss://`, replaces the system certificates |

`Redis.Connect` also accepts a list of node URLs for a [cluster](#cluster).

The client connects on the first command and reconnects automatically after connection losses. Commands sent while Redis is unreachable fail with `nil, message`. The client is closed when the package unloads, or with `:Close()`.

## Commands

Every method takes an optional callback as last argument. With a callback, or through `Quark.Await`, it runs without blocking the server. Without one it blocks until Redis answers, which is usually well below a millisecond on the same machine. Error replies from Redis, like `WRONGTYPE`, are returned as `nil, message`.

Keys, values and arguments can be strings, numbers or buffers. Ranges follow Redis: start and stop are zero based and inclusive, negative values count from the end.

| Method | Redis | Returns |
| --- | --- | --- |
| `:Command(name, ...)` | any command | the reply |
| `:Get(key)` | `GET` | value or `nil` |
| `:Set(key, value, options?)` | `SET` | `true`, or `false` when `NX` / `XX` prevented it. Options `EX` (seconds), `PX` (milliseconds), `NX`, `XX`, `KeepTTL` |
| `:Delete(key, ...)` | `DEL` | number of deleted keys |
| `:Exists(key)` | `EXISTS` | boolean |
| `:Expire(key, seconds)` / `:TTL(key)` | `EXPIRE` / `TTL` | boolean / seconds left (`-1` without expiry, `-2` when missing) |
| `:Increment(key, by?)` / `:Decrement(key, by?)` | `INCRBY` / `DECRBY` | new value |
| `:HashGet(key, field)` / `:HashSet(key, field, value)` / `:HashSet(key, fields)` | `HGET` / `HSET` | value / number of new fields |
| `:HashGetAll(key)` | `HGETALL` | table of fields |
| `:HashDelete(key, field, ...)`, `:HashExists(key, field)`, `:HashIncrement(key, field, by?)` | `HDEL`, `HEXISTS`, `HINCRBY` | |
| `:ListPush(key, ...)` / `:ListPushFront(key, ...)` | `RPUSH` / `LPUSH` | new length |
| `:ListPop(key)` / `:ListPopBack(key)` | `LPOP` / `RPOP` | value or `nil` |
| `:ListRange(key, start, stop)`, `:ListLength(key)` | `LRANGE`, `LLEN` | |
| `:SetAdd(key, ...)`, `:SetRemove(key, ...)`, `:SetMembers(key)`, `:SetContains(key, member)` | `SADD`, `SREM`, `SMEMBERS`, `SISMEMBER` | |
| `:SortedSetAdd(key, member, score)`, `:SortedSetIncrement(key, member, by)` | `ZADD`, `ZINCRBY` | added count / new score |
| `:SortedSetScore(key, member)`, `:SortedSetRank(key, member, options?)` | `ZSCORE`, `ZRANK` / `ZREVRANK` | score / zero based rank, option `Reverse` |
| `:SortedSetRange(key, start, stop, options?)` | `ZRANGE` | members, or `{ Member, Score }` entries with `WithScores`, option `Reverse` |
| `:SortedSetRemove(key, ...)`, `:SortedSetCount(key)` | `ZREM`, `ZCARD` | |
| `:Publish(channel, message)` | `PUBLISH` | number of receivers |
| `:Pipeline(commands, options?)` | pipeline | list of replies, option `Atomic` wraps the commands in `MULTI` / `EXEC` |
| Stream methods | `X...` | see [Streams](#streams) |

Replies are converted to Lua: strings stay strings, integers become integers, arrays become lists, maps become tables and `nil` stays `nil`.

```lua
local replies = redis:Pipeline({
    { "INCR", "visits" },
    { "EXPIRE", "visits", 3600 },
}, { Atomic = true })
```

## Streams

Streams are append only logs with entries of fields and values. Unlike Pub/Sub, entries are kept and can be read later, by several independent readers or by consumer groups that share the work. Entries are `{ ID, Fields }` with IDs like `"1729800000000-0"`.

| Method | Redis | |
| --- | --- | --- |
| `:StreamAdd(key, fields, options?)` | `XADD` | returns the ID. Options `ID` (`"*"`), `MaxLength` with `Approximate` (`true`) to cap the stream |
| `:StreamRange(key, start?, stop?, options?)` | `XRANGE` / `XREVRANGE` | entries between two IDs (`"-"` to `"+"`), options `Count`, `Reverse` |
| `:StreamRead(streams, options?)` | `XREAD` | `{ [key] = entries }` or `nil`, `streams` maps keys to the last seen ID. Options `Count`, `Block` |
| `:StreamLength(key)`, `:StreamDelete(key, id, ...)`, `:StreamTrim(key, max_length, options?)` | `XLEN`, `XDEL`, `XTRIM` | option `Approximate` for trimming |
| `:StreamCreateGroup(key, group, options?)` | `XGROUP CREATE` | `true`, or `false` when the group exists. Options `ID` (`"$"`, only new entries), `MakeStream` (`true`) |
| `:StreamReadGroup(group, consumer, streams, options?)` | `XREADGROUP` | like `StreamRead`, a list of keys reads new entries (`">"`). Options `Count`, `Block`, `NoAck` |
| `:StreamAck(key, group, id, ...)` | `XACK` | number of acknowledged entries |
| `:StreamPending(key, group)` | `XPENDING` | `{ Count, First, Last, Consumers = { [name] = count } }` |
| `:StreamClaim(key, group, consumer, min_idle, options?)` | `XAUTOCLAIM` | `{ Entries, Next }`, takes over entries pending longer than `min_idle` milliseconds. Options `Start`, `Count` |

```lua
redis:StreamAdd("match-events", { type = "kill", killer = killer_id, victim = victim_id }, { MaxLength = 10000 })
```

### Stream readers

`Block` on a client delays every other command of that client until the read returns. To wait for new entries use `Redis.StreamReader(target, options)`, which reads on its own connection and emits an `Entry(stream, id, fields)` event per entry:

| Option | |
| --- | --- |
| `Streams` | a key or a list of keys, required |
| `Start` | without a group: the ID to start after (`"$"`, only new entries). With a group: the ID the group is created at |
| `Group`, `Consumer` | read as a member of a consumer group, `Consumer` is required with `Group` |
| `CreateGroup` | create the group if it does not exist (`true`) |
| `AutoAck` | acknowledge entries right after emitting them (`false`) |
| `Count`, `Block` | entries per read (100) and how long one read waits in milliseconds (2000) |

The reader also emits `Connect`, `Disconnect(reason)`, `Error(message)` and `Close(reason)`, has `:GetState()`, `:IsOpen()` and `:Close()`, and reconnects like the subscriber. Without a group it continues after the last entry it delivered, so no entries are skipped after a reconnect. Without `AutoAck`, acknowledge with `client:StreamAck` once an entry is processed, entries of crashed consumers can be taken over with `StreamClaim`.

```lua
local jobs = Quark.Redis.StreamReader(URL, { Streams = "jobs", Group = "game-servers", Consumer = server_name })
jobs:Subscribe("Entry", function(stream, id, fields)
    HandleJob(fields)
    redis:StreamAck(stream, "game-servers", id)
end)
```

## Pub/Sub

`Redis.Subscriber(url?, options?)` returns an object for receiving messages. It needs its own connection, which is why it is separate from the client.

| Method | |
| --- | --- |
| `:Listen(channel, ...)` / `:Unlisten(channel, ...)` | subscribe to channels |
| `:ListenPattern(pattern, ...)` / `:UnlistenPattern(pattern, ...)` | subscribe to patterns like `game.*` |
| `:GetChannels()`, `:GetPatterns()` | current subscriptions |
| `:GetState()`, `:IsOpen()`, `:Close()` | |

| Event | |
| --- | --- |
| `Connect` | connected and subscribed, also after reconnecting |
| `Message(channel, message, pattern)` | a message, `pattern` is set for pattern subscriptions |
| `Disconnect(reason)` | the connection was lost, reconnecting starts |
| `Error(message)`, `Close(reason)` | |

After a connection loss the subscriber reconnects with a growing delay (0.5 to 10 seconds) and subscribes to all channels and patterns again. Messages published while it is disconnected are lost, that is how Redis Pub/Sub works. Use lists or streams for messages that must not get lost.

```lua
local events = Quark.Redis.Subscriber("redis://127.0.0.1:6379")
events:Listen("chat")
events:Subscribe("Message", function(channel, message)
    Console.Log("[" .. channel .. "] " .. message)
end)

redis:Publish("chat", "Server 2 restarts in 5 minutes")
```

## Cluster

Pass a list of node URLs to connect to a Redis Cluster:

```lua
local cluster = Quark.Redis.Connect({ "redis://10.0.0.1:7000", "redis://10.0.0.2:7000", "redis://10.0.0.3:7000" })
```

The client discovers the other nodes, sends every command to the node owning its key and follows `MOVED` / `ASK` redirections when slots move. All methods work the same as with a single server, with the usual cluster rules:

- Commands with several keys (`DEL a b`, atomic pipelines, `XREAD` over several streams) need all keys in the same slot. Put the shared part of the key in braces to force that: `{player:42}:inventory` and `{player:42}:stats`.
- Pipelines without `Atomic` may mix keys of different slots, their commands are sent to the right nodes in parallel and the replies keep the original order.
- `Subscriber` connects to the first node of the list, Pub/Sub messages are forwarded across the whole cluster.
- Option `ReadFromReplicas` sends read commands to replicas.

Username, password, TLS (`rediss://` URLs) and `CA` work as for single servers.

## Security

Do not expose Redis to the internet without a password and TLS. Keys are shared between everything using the same Redis, prefix them per server or package, for example `"server-1:player:" .. id`.
