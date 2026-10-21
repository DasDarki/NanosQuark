local Test = Quark.Import("Testing")
local Expect = Test.Expect
local Redis = Quark.Redis

local URL = (os and os.getenv and os.getenv("QUARK_REDIS_URL")) or "redis://127.0.0.1:16379"
local probe = Redis.Connect(URL, { Timeout = 500, Retries = 0 })
local available = probe:Command("PING") == "PONG"
probe:Close()

local describe = available and Test.Describe or Test.DescribeSkip
local prefix = "quark-test:" .. Quark.Crypto.RandomToken(6) .. ":"

local function next_event(object, event)
    return Quark.Promise.New(function(resolve)
        local handler
        handler = object:Subscribe(event, function(...)
            object:Unsubscribe(event, handler)
            resolve(...)
        end)
    end)
end

describe("Redis", function()
    local redis

    Test.BeforeAll(function()
        redis = Redis.Connect(URL)
    end)

    Test.AfterAll(function()
        local keys = redis:Command("KEYS", prefix .. "*")
        if #keys > 0 then
            redis:Delete(table.unpack(keys))
        end
        redis:Close()
    end)

    Test.It("stores strings, numbers and binary values", function()
        local key = prefix .. "string"
        Expect(redis:Set(key, "value")):ToBe(true)
        Expect(redis:Get(key)):ToBe("value")
        Expect(redis:Set(key, "other", { NX = true })):ToBe(false)
        Expect(redis:Exists(key)):ToBe(true)
        Expect(redis:Get(prefix .. "missing")):ToBeNil()

        Expect(redis:Set(prefix .. "binary", "\0\1\255")):ToBe(true)
        Expect(redis:Get(prefix .. "binary")):ToBe("\0\1\255")
        Expect(redis:Set(prefix .. "buffer", Quark.Buffer.FromString("buf"))):ToBe(true)
        Expect(redis:Get(prefix .. "buffer")):ToBe("buf")

        Expect(redis:Increment(prefix .. "counter")):ToBe(1)
        Expect(redis:Increment(prefix .. "counter", 10)):ToBe(11)
        Expect(redis:Decrement(prefix .. "counter", 2)):ToBe(9)
        Expect(redis:Delete(key, prefix .. "counter")):ToBe(2)
    end)

    Test.It("expires keys", function()
        local key = prefix .. "expiring"
        redis:Set(key, "soon", { PX = 100 })
        Expect(redis:TTL(key)):ToBeGreaterThanOrEqual(0)
        Quark.Await(Quark.Delay(200))
        Expect(redis:Get(key)):ToBeNil()

        redis:Set(key, "later")
        Expect(redis:Expire(key, 60)):ToBe(true)
        Expect(redis:TTL(key)):ToBeGreaterThan(50)
        Expect(redis:Expire(prefix .. "missing", 60)):ToBe(false)
    end)

    Test.It("works with hashes", function()
        local key = prefix .. "player"
        Expect(redis:HashSet(key, { name = "alice", level = 5 })):ToBe(2)
        Expect(redis:HashSet(key, "guild", "quark")):ToBe(1)
        Expect(redis:HashGet(key, "name")):ToBe("alice")
        Expect(redis:HashIncrement(key, "level", 2)):ToBe(7)
        Expect(redis:HashGetAll(key)):ToEqual({ name = "alice", level = "7", guild = "quark" })
        Expect(redis:HashExists(key, "guild")):ToBe(true)
        Expect(redis:HashDelete(key, "guild")):ToBe(1)
        Expect(redis:HashExists(key, "guild")):ToBe(false)
    end)

    Test.It("works with lists and sets", function()
        local list = prefix .. "queue"
        Expect(redis:ListPush(list, "a", "b", "c")):ToBe(3)
        Expect(redis:ListPushFront(list, "first")):ToBe(4)
        Expect(redis:ListRange(list, 0, -1)):ToEqual({ "first", "a", "b", "c" })
        Expect(redis:ListPop(list)):ToBe("first")
        Expect(redis:ListPopBack(list)):ToBe("c")
        Expect(redis:ListLength(list)):ToBe(2)

        local set = prefix .. "online"
        Expect(redis:SetAdd(set, "alice", "bob", "alice")):ToBe(2)
        Expect(redis:SetContains(set, "bob")):ToBe(true)
        Expect(redis:SetRemove(set, "bob")):ToBe(1)
        Expect(redis:SetMembers(set)):ToEqual({ "alice" })
    end)

    Test.It("keeps leaderboards in sorted sets", function()
        local board = prefix .. "kills"
        redis:SortedSetAdd(board, "alice", 10)
        redis:SortedSetAdd(board, "bob", 25)
        redis:SortedSetAdd(board, "carol", 15)
        Expect(redis:SortedSetIncrement(board, "alice", 20)):ToBe(30)
        Expect(redis:SortedSetScore(board, "carol")):ToBe(15)
        Expect(redis:SortedSetScore(board, "nobody")):ToBeNil()
        Expect(redis:SortedSetRank(board, "alice", { Reverse = true })):ToBe(0)
        Expect(redis:SortedSetCount(board)):ToBe(3)
        Expect(redis:SortedSetRange(board, 0, 1, { Reverse = true })):ToEqual({ "alice", "bob" })
        Expect(redis:SortedSetRange(board, 0, -1, { Reverse = true, WithScores = true })):ToEqual({
            { Member = "alice", Score = 30 },
            { Member = "bob", Score = 25 },
            { Member = "carol", Score = 15 },
        })
        Expect(redis:SortedSetRemove(board, "bob")):ToBe(1)
    end)

    Test.It("runs raw commands and reports errors", function()
        Expect(redis:Command("PING")):ToBe("PONG")
        Expect(redis:Command("ECHO", "hello")):ToBe("hello")
        redis:Set(prefix .. "text", "not a number")
        local value, error_message = redis:Command("INCR", prefix .. "text")
        Expect(value):ToBeNil()
        Expect(error_message):ToContain("not an integer")
        Expect(function() redis:Set(prefix .. "flag", true) end):ToThrow("have to be strings, numbers or buffers")
    end)

    Test.It("runs pipelines and transactions", function()
        local key = prefix .. "pipeline"
        local replies = redis:Pipeline({
            { "SET", key, "1" },
            { "INCRBY", key, 5 },
            { "GET", key },
        }, { Atomic = true })
        Expect(replies):ToEqual({ "OK", 6, "6" })
        Expect(function() redis:Pipeline({ { 42 } }) end):ToThrow("start with a command name")
    end)

    Test.It("works asynchronously", function()
        local key = prefix .. "async"
        Expect(Quark.Await(redis.Set, redis, key, "awaited")):ToBe(true)
        Expect(Quark.Await(redis.Get, redis, key)):ToBe("awaited")
        Expect(Quark.Await(redis.Increment, redis, prefix .. "async-counter")):ToBe(1)
        Expect(function() Quark.Await(redis.Command, redis, "NOSUCHCOMMAND") end):ToThrow("unknown command")
    end)

    Test.It("selects databases", function()
        local other = Redis.Connect(URL, { Database = 1 })
        local key = prefix .. "database"
        redis:Set(key, "zero")
        Expect(other:Get(key)):ToBeNil()
        other:Close()
        Expect(other:IsClosed()):ToBe(true)
        local value, error_message = other:Get(key)
        Expect(value):ToBeNil()
        Expect(error_message):ToContain("closed")
    end)

    Test.It("publishes and receives messages", function()
        local subscriber = Redis.Subscriber(URL)
        subscriber:Listen(prefix .. "news")
        subscriber:ListenPattern(prefix .. "game.*")
        Quark.Await(next_event(subscriber, "Connect"))
        Expect(subscriber:IsOpen()):ToBe(true)
        Expect(subscriber:GetChannels()):ToEqual({ prefix .. "news" })

        local direct = next_event(subscriber, "Message")
        Expect(redis:Publish(prefix .. "news", "hello")):ToBe(1)
        local channel, message, pattern = Quark.Await(direct)
        Expect(channel):ToBe(prefix .. "news")
        Expect(message):ToBe("hello")
        Expect(pattern):ToBeNil()

        local matched = next_event(subscriber, "Message")
        redis:Publish(prefix .. "game.start", "round 1")
        channel, message, pattern = Quark.Await(matched)
        Expect(channel):ToBe(prefix .. "game.start")
        Expect(message):ToBe("round 1")
        Expect(pattern):ToBe(prefix .. "game.*")

        subscriber:Unlisten(prefix .. "news")
        Quark.Await(Quark.Delay(50))
        Expect(redis:Publish(prefix .. "news", "nobody listens")):ToBe(0)

        local closed = next_event(subscriber, "Close")
        subscriber:Close()
        Expect(Quark.Await(closed)):ToBe("closed")
    end)
end)
