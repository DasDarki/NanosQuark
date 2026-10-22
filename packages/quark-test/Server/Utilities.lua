local Test = Quark.Import("Testing")
local Expect = Test.Expect

local function fake_clock()
    local clock = { now = 0 }
    function clock.read()
        return clock.now
    end
    return clock
end

Test.Describe("RateLimit", function()
    local RateLimit = Quark.RateLimit

    Test.It("limits with token buckets", function()
        local clock = fake_clock()
        local chat = RateLimit.TokenBucket({ Capacity = 3, Per = "3s", Clock = clock.read })
        Expect({ chat:Take("player") }):ToEqual({ true, 0, 2 })
        chat:Take("player")
        chat:Take("player")
        local allowed, retry, remaining = chat:Take("player")
        Expect({ allowed, retry, remaining }):ToEqual({ false, 1000, 0 })
        Expect(chat:Take("other")):ToBe(true)

        clock.now = 1.5
        Expect(chat:GetRemaining("player")):ToBeCloseTo(1.5, 0.0001)
        Expect(chat:Take("player")):ToBe(true)
        Expect(chat:Take("player")):ToBe(false)
        Expect(chat:Check("player")):ToBe(false)

        clock.now = 10
        chat:Prune()
        Expect(chat:GetCount()):ToBe(0)
        Expect(chat:GetRemaining("player")):ToBe(3)
    end)

    Test.It("supports costs, rates and any key", function()
        local clock = fake_clock()
        local actions = RateLimit.TokenBucket({ Capacity = 10, Rate = 2, Clock = clock.read })
        local key = {}
        Expect(actions:Take(key, 8)):ToBe(true)
        local allowed, retry = actions:Take(key, 5)
        Expect({ allowed, retry }):ToEqual({ false, 1500 })
        actions:Reset(key)
        Expect(actions:Take(key, 10)):ToBe(true)
        Expect(function() actions:Take(key, 11) end):ToThrow("larger than the limit")
        Expect(function() actions:Take(nil) end):ToThrow("nil")
        Expect(function() RateLimit.TokenBucket({ Capacity = 1 }) end):ToThrow("Per")
    end)

    Test.It("limits with sliding windows", function()
        local clock = fake_clock()
        local logins = RateLimit.SlidingWindow({ Limit = 3, Window = "1m", Clock = clock.read })
        for second = 0, 2 do
            clock.now = second * 10
            Expect(logins:Take("1.2.3.4")):ToBe(true)
        end
        clock.now = 30
        local allowed, retry, remaining = logins:Take("1.2.3.4")
        Expect({ allowed, retry, remaining }):ToEqual({ false, 30000, 0 })
        clock.now = 60
        Expect({ logins:Take("1.2.3.4") }):ToEqual({ true, 0, 0 })
        clock.now = 70
        Expect(logins:GetRemaining("1.2.3.4")):ToBe(1)
        clock.now = 200
        Expect(logins:GetRemaining("1.2.3.4")):ToBe(3)
        Expect(logins:GetCount()):ToBe(0)
    end)

    Test.It("works with the real clock", function()
        local limiter = RateLimit.SlidingWindow({ Limit = 1, Window = 100 })
        Expect(limiter:Take("x")):ToBe(true)
        Expect(limiter:Take("x")):ToBe(false)
        Quark.Await(Quark.Delay(150))
        Expect(limiter:Take("x")):ToBe(true)
    end)
end)

Test.Describe("Cache", function()
    local Cache = Quark.Cache

    Test.It("evicts the least recently used entries", function()
        local evicted = {}
        local cache = Cache.New({ Capacity = 2, OnEvict = function(key, value, reason)
            evicted[#evicted + 1] = key .. "=" .. value .. ":" .. reason
        end })
        cache:Set("a", 1):Set("b", 2)
        Expect(cache:Get("a")):ToBe(1)
        cache:Set("c", 3)
        Expect(cache:Has("b")):ToBe(false)
        Expect(cache:GetKeys()):ToEqual({ "c", "a" })
        cache:Set("a", 10)
        Expect(evicted):ToEqual({ "b=2:Capacity", "a=1:Replaced" })
        Expect(cache:Delete("a")):ToBe(true)
        Expect(cache:Delete("a")):ToBe(false)
        Expect(cache:GetSize()):ToBe(1)
        Expect(cache:Peek("c")):ToBe(3)
        cache:Clear()
        Expect(cache:GetSize()):ToBe(0)
    end)

    Test.It("expires entries", function()
        local clock = fake_clock()
        local cache = Cache.New({ TTL = "10s", Clock = clock.read })
        cache:Set("session", "abc")
        cache:Set("forever", true, false)
        cache:Set("short", 1, 1000)
        Expect(cache:GetTTL("session")):ToBe(10000)
        Expect(cache:GetTTL("forever")):ToBe(-1)
        clock.now = 5
        Expect(cache:Get("short")):ToBeNil()
        Expect(cache:Get("session")):ToBe("abc")
        clock.now = 11
        Expect(cache:Prune()):ToBe(1)
        Expect(cache:GetKeys()):ToEqual({ "forever" })
    end)

    Test.It("creates missing values and counts hits", function()
        local cache = Cache.New()
        local created = 0
        local function load(key)
            created = created + 1
            return key * 2
        end
        Expect(cache:GetOrSet(21, load)):ToBe(42)
        Expect(cache:GetOrSet(21, load)):ToBe(42)
        Expect(created):ToBe(1)
        Expect(cache:Get(1)):ToBeNil()
        local stats = cache:GetStats()
        Expect({ stats.Hits, stats.Misses }):ToEqual({ 1, 2 })
        Expect(stats.HitRate):ToBeCloseTo(1 / 3, 0.0001)

        local calls = 0
        local square = Cache.Memoize(function(value)
            calls = calls + 1
            return value * value
        end, { Capacity = 10 })
        Expect(square(4) + square(4)):ToBe(32)
        Expect(calls):ToBe(1)
        Expect(function() Cache.New({ Capacity = 0 }) end):ToThrow("Capacity")
    end)
end)

Test.Describe("PriorityQueue", function()
    local PriorityQueue = Quark.PriorityQueue

    Test.It("pops by priority and keeps insertion order", function()
        local queue = PriorityQueue.New()
        queue:Push("low", 5):Push("first", 1):Push("second", 1):Push("middle", 3)
        Expect(queue:GetSize()):ToBe(4)
        Expect({ queue:Peek() }):ToEqual({ "first", 1 })
        Expect({ queue:Pop() }):ToEqual({ "first", 1 })
        Expect(queue:Pop()):ToBe("second")
        Expect(queue:Pop()):ToBe("middle")
        Expect(queue:Pop()):ToBe("low")
        Expect(queue:Pop()):ToBeNil()
        Expect(queue:IsEmpty()):ToBe(true)
    end)

    Test.It("supports max order, removal and listing", function()
        local queue = PriorityQueue.New({ Order = "Max" })
        for index = 1, 100 do
            queue:Push(index, (index * 37) % 101)
        end
        Expect(queue:Remove(1)):ToBe(true)
        Expect(queue:Remove(1000)):ToBe(false)
        local previous = math.huge
        local list = queue:ToList()
        Expect(#list):ToBe(99)
        Expect(queue:GetSize()):ToBe(99)
        for _, entry in ipairs(list) do
            Expect(entry.Priority <= previous):ToBe(true)
            previous = entry.Priority
        end
        for index = 1, 99 do
            local _, priority = queue:Pop()
            Expect(priority):ToBe(list[index].Priority)
        end
        Expect(function() queue:Push("x", "high") end):ToThrow("number")
    end)
end)

Test.Describe("RingBuffer", function()
    local RingBuffer = Quark.RingBuffer

    Test.It("keeps the latest values", function()
        local history = RingBuffer.New(3)
        Expect(history:Push("a")):ToBeNil()
        history:Push("b")
        history:Push("c")
        Expect(history:IsFull()):ToBe(true)
        Expect(history:Push("d")):ToBe("a")
        Expect(history:ToList()):ToEqual({ "b", "c", "d" })
        Expect(history:Get(1)):ToBe("b")
        Expect(history:Get(-1)):ToBe("d")
        Expect(history:Get(4)):ToBeNil()
        Expect({ history:GetFirst(), history:GetLast() }):ToEqual({ "b", "d" })
        history:Set(2, "C")

        local seen = {}
        for index, value in history:Values() do
            seen[index] = value
        end
        Expect(seen):ToEqual({ "b", "C", "d" })
        Expect(history:Shift()):ToBe("b")
        Expect(history:Pop()):ToBe("d")
        Expect(history:ToList()):ToEqual({ "C" })
        history:Clear()
        Expect(history:IsEmpty()):ToBe(true)
        Expect(history:GetCapacity()):ToBe(3)
        Expect(function() RingBuffer.New(0) end):ToThrow("capacity")
    end)
end)

Test.Describe("Bitset", function()
    local Bitset = Quark.Bitset

    Test.It("sets, counts and finds bits", function()
        local bits = Bitset.New(130)
        local base = Quark.Buffer.New(1):GetIndexBase()
        bits:Set(base)
        bits:Set(base + 64)
        bits:Set(base + 129)
        Expect(bits:Get(base + 64)):ToBe(true)
        Expect(bits:Get(base + 65)):ToBe(false)
        Expect(bits:Count()):ToBe(3)
        Expect(#bits):ToBe(130)
        Expect(bits:NextSet(base + 1)):ToBe(base + 64)
        Expect(bits:NextUnset(base)):ToBe(base + 1)
        Expect(bits:GetIndices()):ToEqual({ base, base + 64, base + 129 })
        Expect(bits:Toggle(base)):ToBe(false)
        bits:Unset(base + 64)
        Expect(bits:NextSet(base + 130)):ToBeNil()
        Expect(function() bits:Get(base + 130) end):ToThrow("outside")
        bits:SetAll()
        Expect(bits:All()):ToBe(true)
        bits:Invert()
        Expect(bits:None()):ToBe(true)
        Expect(tostring(bits)):ToBe("Bitset(0 of 130 set)")
    end)

    Test.It("combines and serializes bitsets", function()
        local base = Quark.Buffer.New(1):GetIndexBase()
        local left, right = Bitset.New(10), Bitset.New(10)
        left:Set(base + 1); left:Set(base + 2)
        right:Set(base + 2); right:Set(base + 3)
        Expect(left:Clone():Union(right):GetIndices()):ToEqual({ base + 1, base + 2, base + 3 })
        Expect(left:Clone():Intersect(right):GetIndices()):ToEqual({ base + 2 })
        Expect(left:Clone():Difference(right):GetIndices()):ToEqual({ base + 1 })
        Expect(left:Clone():SymmetricDifference(right):GetIndices()):ToEqual({ base + 1, base + 3 })
        Expect(left == left:Clone()):ToBe(true)
        Expect(left == right):ToBe(false)
        Expect(function() left:Union(Bitset.New(5)) end):ToThrow("same size")

        local restored = Bitset.FromBytes(left:ToBytes(), 10)
        Expect(restored == left):ToBe(true)
        Expect(#left:ToBytes()):ToBe(2)
        left:Resize(4)
        Expect(left:GetSize()):ToBe(4)
        left:Resize(100)
        Expect(left:GetIndices()):ToEqual({ base + 1, base + 2 })
        Expect(Bitset.IsBitset(left)):ToBe(true)
    end)
end)

Test.Describe("Metrics", function()
    local Metrics = Quark.Metrics

    Test.It("renders counters, gauges and histograms", function()
        local registry = Metrics.Registry()
        local requests = registry:Counter("http_requests_total", "Handled requests", { Labels = { "method", "status" } })
        requests:Inc({ method = "GET", status = 200 })
        requests:Inc(2, { method = "GET", status = 200 })
        requests:Inc({ method = "POST", status = 500 })
        Expect(requests:Get({ method = "GET", status = 200 })):ToBe(3)

        local players = registry:Gauge("players_online", "Connected players")
        players:Set(5)
        players:Dec()

        local tick = registry:Histogram("tick_seconds", "Tick duration", { Buckets = { 0.01, 0.1 } })
        tick:Observe(0.005)
        tick:Observe(0.05)
        tick:Observe(1)

        Expect(registry:Render()):ToBe(table.concat({
            "# HELP http_requests_total Handled requests",
            "# TYPE http_requests_total counter",
            'http_requests_total{method="GET",status="200"} 3',
            'http_requests_total{method="POST",status="500"} 1',
            "# HELP players_online Connected players",
            "# TYPE players_online gauge",
            "players_online 4",
            "# HELP tick_seconds Tick duration",
            "# TYPE tick_seconds histogram",
            'tick_seconds_bucket{le="0.01"} 1',
            'tick_seconds_bucket{le="0.10000000000000001"} 2',
            'tick_seconds_bucket{le="+Inf"} 3',
            "tick_seconds_sum 1.0549999999999999",
            "tick_seconds_count 3",
            "",
        }, "\n"))
        Expect(tick:Get()):ToEqual({ Count = 3, Sum = 1.055 })
    end)

    Test.It("collects values and validates labels", function()
        local registry = Metrics.Registry()
        registry:Gauge("entities", "Entities by type", {
            Labels = { "type" },
            Collect = function()
                return { { Labels = { type = "car" }, Value = 2 }, { Labels = { type = "prop" }, Value = 7 } }
            end,
        })
        Expect(registry:Render()):ToContain('entities{type="prop"} 7')

        local counter = registry:Counter("events_total", "Events", { Labels = { "name" } })
        Expect(registry:Counter("events_total", "Events", { Labels = { "name" } })):ToBe(counter)
        Expect(function() registry:Gauge("events_total") end):ToThrow("already registered")
        Expect(function() counter:Inc() end):ToThrow("needs the labels")
        Expect(function() counter:Inc({ name = "a", other = "b" }) end):ToThrow("unknown label")
        Expect(function() counter:Inc(-1, { name = "a" }) end):ToThrow("only increase")
        Expect(function() registry:Counter("1bad") end):ToThrow("metric names")
        counter:Inc({ name = 'quote"and\\slash' })
        Expect(registry:Render()):ToContain('events_total{name="quote\\"and\\\\slash"} 1')

        local timer = registry:Histogram("work_seconds", "Work")
        Expect(timer:Time(function() return 42 end)):ToBe(42)
        Expect(timer:Get().Count):ToBe(1)
        Expect(registry:Unregister("work_seconds")):ToBe(true)
        Expect(registry:Get("work_seconds")):ToBeNil()
    end)

    Test.It("serves metrics over HTTP", function()
        local HTTP = Quark.HTTP
        local registry = Metrics.Registry()
        registry:Counter("visits_total", "Visits"):Inc()
        local server = assert(HTTP.Listen(0, { Host = "127.0.0.1" }))
        registry:Expose(server)
        local response = Quark.Await(HTTP.Request, { URL = "http://127.0.0.1:" .. server:GetPort() .. "/metrics" })
        server:Close()
        Expect(response.Status):ToBe(200)
        Expect(response.Headers["content-type"]):ToContain("text/plain")
        Expect(response.Body):ToContain("visits_total 1")
    end)
end)

Test.Describe("Log", function()
    local Log = Quark.Log

    Test.It("writes text and JSON lines to files", function()
        local directory = os.getenv("QUARK_TEST_TMP") or "quark-test-logs"
        local path = directory .. "/" .. tostring(math.floor(Quark.Clock() * 1000000)) .. ".log"
        local logger = Log.New({ Name = "game", File = path, Console = false, Fields = { server = 1 } })
        logger:Debug("hidden")
        logger:Info("player joined", { player = "Ada", reason = "first visit" })
        local child = logger:With({ match = 7 })
        child:Warn("low tick rate", { tps = 28.5 })
        logger:Flush()

        local file = assert(io.open(path, "r"))
        local lines = {}
        for line in file:lines() do
            lines[#lines + 1] = line
        end
        file:close()
        Expect(#lines):ToBe(2)
        Expect(lines[1]):ToMatch("^%d%d%d%d%-%d%d%-%d%dT%d%d:%d%d:%d%d%.%d%d%d[+-]%d%d:%d%d INFO  %[game%] player joined player=Ada reason=\"first visit\" server=1$")
        Expect(lines[2]):ToContain("WARN  [game] low tick rate match=7 server=1 tps=28.5")

        local json_path = path .. ".json"
        local json = Log.New({ File = json_path, Format = "Json", Console = false, Level = "Debug" })
        json:Debug("details", { list = { 1, 2 } })
        json:Flush()
        local handle = assert(io.open(json_path, "r"))
        local entry = Quark.JSON.Decode(handle:read("l"))
        handle:close()
        Expect(entry.level):ToBe("debug")
        Expect(entry.message):ToBe("details")
        Expect(entry.list):ToEqual({ 1, 2 })
        Expect(entry.time):ToBeType("string")
        os.remove(path)
        os.remove(json_path)
    end)

    Test.It("rotates files", function()
        local directory = os.getenv("QUARK_TEST_TMP") or "quark-test-logs"
        local path = directory .. "/rotate-" .. tostring(math.floor(Quark.Clock() * 1000000)) .. ".log"
        local logger = Log.New({ File = path, Console = false, MaxFileSize = 200, MaxFiles = 2 })
        for index = 1, 20 do
            logger:Info("line number " .. index)
        end
        logger:Flush()
        Expect(io.open(path .. ".1", "r") ~= nil):ToBe(true)
        Expect(io.open(path .. ".2", "r") ~= nil):ToBe(true)
        Expect(io.open(path .. ".3", "r")):ToBeNil()
        local last = assert(io.open(path, "r")):read("a")
        Expect(last):ToContain("line number 20")
        for _, suffix in ipairs({ "", ".1", ".2" }) do
            os.remove(path .. suffix)
        end
    end)

    Test.It("filters by level and writes to the console", function()
        local logger = Log.New({ Name = "console", Level = "Warn" })
        Expect(logger:IsEnabled("Info")):ToBe(false)
        Expect(logger:IsEnabled("Error")):ToBe(true)
        logger:SetLevel("Debug")
        Expect(logger:GetLevel()):ToBe("Debug")
        local original = Console.Warn
        local written
        Console.Warn = function(text) written = text end
        local ok, message = pcall(logger.Warn, logger, "careful", { hint = "x" })
        Console.Warn = original
        Expect(ok):ToBe(true, message)
        Expect(written):ToBe("[console] careful hint=x")
        Expect(function() logger:SetLevel("Loud") end):ToThrow("unknown log level")
        Expect(function() Log.New({ Format = "Xml" }) end):ToThrow("unknown log format")
    end)
end)
