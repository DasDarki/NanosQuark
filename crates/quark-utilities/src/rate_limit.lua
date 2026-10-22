local clock, duration = ...

local error = error
local math_ceil = math.ceil
local math_max = math.max
local math_min = math.min
local pairs = pairs
local setmetatable = setmetatable
local type = type

local PRUNE_INTERVAL = 1000

local function read_clock(options)
    local custom = options.Clock
    if custom ~= nil and type(custom) ~= "function" then
        error("the Clock option has to be a function returning seconds", 3)
    end
    return custom or clock
end

local function read_cost(cost, capacity)
    if cost == nil then
        return 1
    end
    if type(cost) ~= "number" or cost <= 0 then
        error("the cost has to be a number greater than 0", 3)
    end
    if cost > capacity then
        error("the cost is larger than the limit", 3)
    end
    return cost
end

local function check_key(key)
    if key == nil or key ~= key then
        error("the key must not be nil or NaN", 3)
    end
end

local TokenBucket = {}
TokenBucket.__index = TokenBucket

local function refill(self, key, now)
    local entry = self.entries[key]
    if not entry then
        return self.capacity
    end
    local tokens = math_min(self.capacity, entry.tokens + (now - entry.time) * self.rate)
    entry.tokens = tokens
    entry.time = now
    return tokens
end

local function prune_buckets(self, now)
    for key, entry in pairs(self.entries) do
        if entry.tokens + (now - entry.time) * self.rate >= self.capacity then
            self.entries[key] = nil
        end
    end
end

local function bucket_result(self, tokens, cost)
    local missing = cost - tokens
    local retry = missing > 0 and math_ceil(missing / self.rate * 1000) or 0
    return retry, tokens
end

function TokenBucket:Take(key, cost)
    check_key(key)
    cost = read_cost(cost, self.capacity)
    local now = self.clock()
    self.operations = self.operations + 1
    if self.operations % PRUNE_INTERVAL == 0 then
        prune_buckets(self, now)
    end

    local tokens = refill(self, key, now)
    if tokens >= cost then
        local entry = self.entries[key]
        if entry then
            entry.tokens = tokens - cost
        else
            self.entries[key] = { tokens = tokens - cost, time = now }
        end
        return true, 0, tokens - cost
    end
    local retry, remaining = bucket_result(self, tokens, cost)
    return false, retry, remaining
end

function TokenBucket:Check(key, cost)
    check_key(key)
    cost = read_cost(cost, self.capacity)
    local tokens = refill(self, key, self.clock())
    if tokens >= cost then
        return true, 0, tokens
    end
    local retry, remaining = bucket_result(self, tokens, cost)
    return false, retry, remaining
end

function TokenBucket:GetRemaining(key)
    check_key(key)
    return refill(self, key, self.clock())
end

function TokenBucket:Reset(key)
    check_key(key)
    self.entries[key] = nil
end

function TokenBucket:Clear()
    self.entries = {}
end

function TokenBucket:Prune()
    prune_buckets(self, self.clock())
end

function TokenBucket:GetCount()
    local count = 0
    for _ in pairs(self.entries) do
        count = count + 1
    end
    return count
end

local function token_bucket(options)
    if type(options) ~= "table" then
        error("expected an options table with Capacity and Per or Rate", 2)
    end
    local capacity = options.Capacity
    if type(capacity) ~= "number" or capacity <= 0 then
        error("Capacity has to be a number greater than 0", 2)
    end
    local rate = options.Rate
    if options.Per ~= nil then
        local period = duration(options.Per)
        if period <= 0 then
            error("Per has to be longer than 0 milliseconds", 2)
        end
        rate = capacity / (period / 1000)
    end
    if type(rate) ~= "number" or rate <= 0 then
        error("set Per (time to refill the whole bucket) or Rate (tokens per second)", 2)
    end
    return setmetatable({
        capacity = capacity,
        rate = rate,
        clock = read_clock(options),
        entries = {},
        operations = 0,
    }, TokenBucket)
end

local SlidingWindow = {}
SlidingWindow.__index = SlidingWindow

local function trim(self, key, now)
    local entry = self.entries[key]
    if not entry then
        return nil, 0
    end
    local limit = now - self.window
    local first = entry.first
    while first <= entry.last and entry.times[first] <= limit do
        entry.used = entry.used - entry.costs[first]
        entry.times[first] = nil
        entry.costs[first] = nil
        first = first + 1
    end
    entry.first = first
    if first > entry.last then
        self.entries[key] = nil
        return nil, 0
    end
    return entry, entry.used
end

local function window_retry(self, entry, used, cost, now)
    local needed = used + cost - self.limit
    local index = entry.first
    while needed > 0 and index <= entry.last do
        needed = needed - entry.costs[index]
        if needed <= 0 then
            return math_max(1, math_ceil((entry.times[index] + self.window - now) * 1000))
        end
        index = index + 1
    end
    return math_ceil(self.window * 1000)
end

function SlidingWindow:Take(key, cost)
    check_key(key)
    cost = read_cost(cost, self.limit)
    local now = self.clock()
    self.operations = self.operations + 1
    if self.operations % PRUNE_INTERVAL == 0 then
        for other in pairs(self.entries) do
            trim(self, other, now)
        end
    end

    local entry, used = trim(self, key, now)
    if used + cost > self.limit then
        return false, window_retry(self, entry, used, cost, now), self.limit - used
    end
    if not entry then
        entry = { first = 1, last = 0, used = 0, times = {}, costs = {} }
        self.entries[key] = entry
    end
    entry.last = entry.last + 1
    entry.times[entry.last] = now
    entry.costs[entry.last] = cost
    entry.used = used + cost
    return true, 0, self.limit - entry.used
end

function SlidingWindow:Check(key, cost)
    check_key(key)
    cost = read_cost(cost, self.limit)
    local now = self.clock()
    local entry, used = trim(self, key, now)
    if used + cost > self.limit then
        return false, window_retry(self, entry, used, cost, now), self.limit - used
    end
    return true, 0, self.limit - used
end

function SlidingWindow:GetRemaining(key)
    check_key(key)
    local _, used = trim(self, key, self.clock())
    return self.limit - used
end

SlidingWindow.Reset = TokenBucket.Reset
SlidingWindow.Clear = TokenBucket.Clear
SlidingWindow.GetCount = TokenBucket.GetCount

function SlidingWindow:Prune()
    local now = self.clock()
    for key in pairs(self.entries) do
        trim(self, key, now)
    end
end

local function sliding_window(options)
    if type(options) ~= "table" then
        error("expected an options table with Limit and Window", 2)
    end
    local limit = options.Limit
    if type(limit) ~= "number" or limit <= 0 then
        error("Limit has to be a number greater than 0", 2)
    end
    if options.Window == nil then
        error("Window is required", 2)
    end
    local window = duration(options.Window)
    if window <= 0 then
        error("Window has to be longer than 0 milliseconds", 2)
    end
    return setmetatable({
        limit = limit,
        window = window / 1000,
        clock = read_clock(options),
        entries = {},
        operations = 0,
    }, SlidingWindow)
end


return {
    TokenBucket = token_bucket,
    SlidingWindow = sliding_window,
}
