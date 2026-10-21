# Redis

```lua
local redis = Quark.Redis.Connect("redis://127.0.0.1:6379")

redis:Set("motd", "Welcome!")
redis:SortedSetIncrement("kills", player:GetAccountID(), 1)

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

Replies are converted to Lua: strings stay strings, integers become integers, arrays become lists, maps become tables and `nil` stays `nil`.

```lua
local replies = redis:Pipeline({
    { "INCR", "visits" },
    { "EXPIRE", "visits", 3600 },
}, { Atomic = true })
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
    Chat.BroadcastMessage(message)
end)

redis:Publish("chat", "Server 2 restarts in 5 minutes")
```

## Security

Do not expose Redis to the internet without a password and TLS. Keys are shared between everything using the same Redis, prefix them per server or package, for example `"server-1:player:" .. id`.
