local duration, encode, decode, token = ...

local error = error
local math_type = math.type
local setmetatable = setmetatable
local table_pack = table.pack
local table_unpack = table.unpack
local tonumber = tonumber
local tostring = tostring
local type = type

local SLIDING_WINDOW = [[
local key = KEYS[1]
local limit = tonumber(ARGV[1])
local window = tonumber(ARGV[2])
local cost = tonumber(ARGV[3])
local consume = ARGV[4] == '1'
local id = ARGV[5]
local time = redis.call('TIME')
local now = tonumber(time[1]) * 1000 + math.floor(tonumber(time[2]) / 1000)
redis.call('ZREMRANGEBYSCORE', key, '-inf', now - window)
local used = redis.call('ZCARD', key)
if used + cost <= limit then
    if consume then
        for index = 1, cost do
            redis.call('ZADD', key, now, id .. ':' .. index)
        end
        redis.call('PEXPIRE', key, window)
        used = used + cost
    end
    return { 1, 0, limit - used }
end
local position = used + cost - limit - 1
local entry = redis.call('ZRANGE', key, position, position, 'WITHSCORES')
local retry = window
if entry[2] then
    retry = tonumber(entry[2]) + window - now
end
return { 0, math.max(retry, 1), limit - used }
]]

local TOKEN_BUCKET = [[
local key = KEYS[1]
local capacity = tonumber(ARGV[1])
local rate = tonumber(ARGV[2])
local cost = tonumber(ARGV[3])
local consume = ARGV[4] == '1'
local time = redis.call('TIME')
local now = tonumber(time[1]) * 1000 + math.floor(tonumber(time[2]) / 1000)
local stored = redis.call('HMGET', key, 'tokens', 'time')
local tokens = tonumber(stored[1]) or capacity
local last = tonumber(stored[2]) or now
tokens = math.min(capacity, tokens + math.max(0, now - last) * rate)
if tokens >= cost then
    if consume then
        tokens = tokens - cost
        redis.call('HSET', key, 'tokens', tostring(tokens), 'time', now)
        redis.call('PEXPIRE', key, math.ceil(capacity / rate) + 1000)
    end
    return { 1, 0, tostring(tokens) }
end
return { 0, math.ceil((cost - tokens) / rate), tostring(tokens) }
]]

local function check_client(client)
    if type(client) ~= "userdata" and type(client) ~= "table" then
        error("expected a Redis client from Quark.Redis.Connect", 3)
    end
end

local function check_key(key)
    local kind = type(key)
    if kind ~= "string" and kind ~= "number" then
        error("shared keys have to be strings or numbers, use an ID like the Steam ID for players", 3)
    end
    return tostring(key)
end

local function command(client, callback, convert, ...)
    if callback then
        local arguments = table_pack(...)
        arguments.n = arguments.n + 1
        arguments[arguments.n] = function(result, message)
            if message ~= nil then
                return callback(nil, message)
            end
            return callback(convert(result))
        end
        client:Command(table_unpack(arguments, 1, arguments.n))
        return
    end
    local result, message = client:Command(...)
    if message ~= nil then
        return nil, message
    end
    return convert(result)
end

local function limit_result(reply)
    return reply[1] == 1, reply[2], tonumber(reply[3])
end

local SharedLimiter = {}
SharedLimiter.__index = SharedLimiter

local function run_limiter(self, key, cost, callback, consume)
    if type(cost) == "function" then
        cost, callback = nil, cost
    end
    cost = cost or 1
    if type(cost) ~= "number" or cost <= 0 or (self.kind == "window" and math_type(cost) ~= "integer") then
        error("the cost has to be a number greater than 0, whole for sliding windows", 3)
    end
    if cost > self.limit then
        error("the cost is larger than the limit", 3)
    end
    local name = self.prefix .. check_key(key)
    local flag = consume and "1" or "0"
    if self.kind == "window" then
        return command(self.client, callback, limit_result, "EVAL", SLIDING_WINDOW, 1, name, self.limit, self.window, cost, flag, token())
    end
    return command(self.client, callback, limit_result, "EVAL", TOKEN_BUCKET, 1, name, self.limit, self.rate, cost, flag)
end

function SharedLimiter:Take(key, cost, callback)
    return run_limiter(self, key, cost, callback, true)
end

function SharedLimiter:Check(key, cost, callback)
    return run_limiter(self, key, cost, callback, false)
end

function SharedLimiter:Reset(key, callback)
    return command(self.client, callback, function() return true end, "DEL", self.prefix .. check_key(key))
end

local function shared_limiter(client, options)
    check_client(client)
    if type(options) ~= "table" then
        error("expected an options table", 2)
    end
    local prefix = options.Prefix or "quark:ratelimit:"
    if options.Window ~= nil then
        local limit = options.Limit
        if math_type(limit) ~= "integer" or limit < 1 then
            error("Limit has to be a whole number of at least 1", 2)
        end
        local window = duration(options.Window)
        if window < 1 then
            error("Window has to be at least 1 millisecond", 2)
        end
        return setmetatable({ client = client, kind = "window", limit = limit, window = math.floor(window), prefix = prefix }, SharedLimiter)
    end
    local capacity = options.Capacity
    if type(capacity) ~= "number" or capacity <= 0 then
        error("set Limit and Window for a sliding window, or Capacity with Per or Rate for a token bucket", 2)
    end
    local rate
    if options.Per ~= nil then
        rate = capacity / duration(options.Per)
    elseif type(options.Rate) == "number" and options.Rate > 0 then
        rate = options.Rate / 1000
    else
        error("a token bucket needs Per (time to refill the whole bucket) or Rate (tokens per second)", 2)
    end
    return setmetatable({ client = client, kind = "bucket", limit = capacity, rate = rate, prefix = prefix }, SharedLimiter)
end

local SharedCache = {}
SharedCache.__index = SharedCache

local function decode_value(raw)
    if raw == nil then
        return nil
    end
    return decode(raw)
end

local function expiry(self, ttl)
    if ttl == nil then
        return self.ttl
    end
    if ttl == false then
        return nil
    end
    local milliseconds = duration(ttl)
    if milliseconds < 1 then
        error("the TTL has to be at least 1 millisecond", 3)
    end
    return math.floor(milliseconds)
end

function SharedCache:Get(key, callback)
    return command(self.client, callback, decode_value, "GET", self.prefix .. check_key(key))
end

function SharedCache:Set(key, value, ttl, callback)
    if type(ttl) == "function" then
        ttl, callback = nil, ttl
    end
    local name = self.prefix .. check_key(key)
    if value == nil then
        return command(self.client, callback, function() return true end, "DEL", name)
    end
    local milliseconds = expiry(self, ttl)
    if milliseconds then
        return command(self.client, callback, function() return true end, "SET", name, encode(value), "PX", milliseconds)
    end
    return command(self.client, callback, function() return true end, "SET", name, encode(value))
end

function SharedCache:Delete(key, callback)
    return command(self.client, callback, function(count) return count > 0 end, "DEL", self.prefix .. check_key(key))
end

function SharedCache:Has(key, callback)
    return command(self.client, callback, function(count) return count > 0 end, "EXISTS", self.prefix .. check_key(key))
end

function SharedCache:GetOrSet(key, factory, ttl, callback)
    if type(ttl) == "function" then
        ttl, callback = nil, ttl
    end
    if type(factory) ~= "function" then
        error("GetOrSet needs a function that creates the value", 2)
    end
    if not callback then
        local value, message = self:Get(key)
        if message ~= nil then
            return nil, message
        end
        if value ~= nil then
            return value
        end
        value = factory(key)
        if value ~= nil then
            local stored, store_message = self:Set(key, value, ttl)
            if not stored then
                return nil, store_message
            end
        end
        return value
    end
    self:Get(key, function(value, message)
        if message ~= nil then
            return callback(nil, message)
        end
        if value ~= nil then
            return callback(value)
        end
        local created = factory(key)
        if created == nil then
            return callback(nil)
        end
        self:Set(key, created, ttl, function(stored, store_message)
            if not stored then
                return callback(nil, store_message)
            end
            callback(created)
        end)
    end)
end

local function shared_cache(client, options)
    check_client(client)
    if options ~= nil and type(options) ~= "table" then
        error("expected an options table", 2)
    end
    options = options or {}
    local self = setmetatable({ client = client, prefix = options.Prefix or "quark:cache:" }, SharedCache)
    self.ttl = options.TTL ~= nil and expiry(self, options.TTL) or nil
    return self
end

return {
    RateLimit = shared_limiter,
    Cache = shared_cache,
}
