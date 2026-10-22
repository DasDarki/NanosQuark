local clock, duration, report = ...

local error = error
local pcall = pcall
local select = select
local setmetatable = setmetatable
local type = type

local Cache = {}
Cache.__index = Cache

local function unlink(self, node)
    if node.newer then
        node.newer.older = node.older
    else
        self.newest = node.older
    end
    if node.older then
        node.older.newer = node.newer
    else
        self.oldest = node.newer
    end
    node.newer = nil
    node.older = nil
end

local function link_newest(self, node)
    node.older = self.newest
    node.newer = nil
    if self.newest then
        self.newest.newer = node
    else
        self.oldest = node
    end
    self.newest = node
end

local function notify(self, node, reason)
    if self.on_evict then
        local ok, message = pcall(self.on_evict, node.key, node.value, reason)
        if not ok then
            report(message)
        end
    end
end

local function remove(self, node, reason)
    unlink(self, node)
    self.nodes[node.key] = nil
    self.size = self.size - 1
    if reason ~= "Deleted" then
        self.evictions = self.evictions + 1
    end
    notify(self, node, reason)
end

local function expired(self, node, now)
    return node.expires ~= nil and node.expires <= now
end

local function lookup(self, key, refresh)
    local node = self.nodes[key]
    if not node then
        return nil
    end
    if expired(self, node, self.clock()) then
        remove(self, node, "Expired")
        return nil
    end
    if refresh and self.newest ~= node then
        unlink(self, node)
        link_newest(self, node)
    end
    return node
end

local function read_ttl(self, ttl)
    if ttl == nil then
        return self.ttl
    end
    if ttl == false then
        return nil
    end
    local milliseconds = duration(ttl)
    if milliseconds <= 0 then
        error("the TTL has to be longer than 0 milliseconds", 3)
    end
    return milliseconds / 1000
end

local function check_key(key)
    if key == nil or key ~= key then
        error("cache keys must not be nil or NaN", 3)
    end
end

function Cache:Get(key)
    check_key(key)
    local node = lookup(self, key, true)
    if node then
        self.hits = self.hits + 1
        return node.value
    end
    self.misses = self.misses + 1
    return nil
end

function Cache:Peek(key)
    check_key(key)
    local node = lookup(self, key, false)
    return node and node.value
end

function Cache:Has(key)
    check_key(key)
    return lookup(self, key, false) ~= nil
end

function Cache:Set(key, value, ttl)
    check_key(key)
    if value == nil then
        local node = self.nodes[key]
        if node then
            remove(self, node, "Deleted")
        end
        return self
    end
    local lifetime = read_ttl(self, ttl)
    local expires = lifetime and self.clock() + lifetime

    local node = self.nodes[key]
    if node then
        local previous = node.value
        node.value = value
        node.expires = expires
        if self.newest ~= node then
            unlink(self, node)
            link_newest(self, node)
        end
        if previous ~= value and self.on_evict then
            notify(self, { key = key, value = previous }, "Replaced")
        end
        return self
    end

    node = { key = key, value = value, expires = expires }
    self.nodes[key] = node
    link_newest(self, node)
    self.size = self.size + 1
    while self.capacity and self.size > self.capacity do
        remove(self, self.oldest, "Capacity")
    end
    return self
end

function Cache:GetOrSet(key, factory, ttl)
    check_key(key)
    if type(factory) ~= "function" then
        error("GetOrSet needs a function that creates the value", 2)
    end
    local node = lookup(self, key, true)
    if node then
        self.hits = self.hits + 1
        return node.value
    end
    self.misses = self.misses + 1
    local value = factory(key)
    if value ~= nil then
        self:Set(key, value, ttl)
    end
    return value
end

function Cache:Delete(key)
    check_key(key)
    local node = self.nodes[key]
    if not node then
        return false
    end
    remove(self, node, "Deleted")
    return true
end

function Cache:Clear()
    local node = self.oldest
    while node do
        local newer = node.newer
        remove(self, node, "Deleted")
        node = newer
    end
end

function Cache:Prune()
    local now = self.clock()
    local removed = 0
    local node = self.oldest
    while node do
        local newer = node.newer
        if expired(self, node, now) then
            remove(self, node, "Expired")
            removed = removed + 1
        end
        node = newer
    end
    return removed
end

function Cache:GetSize()
    return self.size
end

function Cache:GetCapacity()
    return self.capacity
end

function Cache:GetKeys()
    local keys = {}
    local now = self.clock()
    local node = self.newest
    while node do
        if not expired(self, node, now) then
            keys[#keys + 1] = node.key
        end
        node = node.older
    end
    return keys
end

function Cache:GetTTL(key)
    check_key(key)
    local node = lookup(self, key, false)
    if not node then
        return nil
    end
    if not node.expires then
        return -1
    end
    return (node.expires - self.clock()) * 1000
end

function Cache:GetStats()
    local total = self.hits + self.misses
    return {
        Hits = self.hits,
        Misses = self.misses,
        Evictions = self.evictions,
        HitRate = total > 0 and self.hits / total or 0,
    }
end

local function create(options)
    if options ~= nil and type(options) ~= "table" then
        error("expected an options table", 2)
    end
    options = options or {}
    local capacity = options.Capacity
    if capacity ~= nil and (math.type(capacity) ~= "integer" or capacity < 1) then
        error("Capacity has to be an integer of at least 1", 2)
    end
    if options.OnEvict ~= nil and type(options.OnEvict) ~= "function" then
        error("OnEvict has to be a function", 2)
    end
    if options.Clock ~= nil and type(options.Clock) ~= "function" then
        error("the Clock option has to be a function returning seconds", 2)
    end
    local self = setmetatable({
        capacity = capacity,
        on_evict = options.OnEvict,
        clock = options.Clock or clock,
        nodes = {},
        size = 0,
        hits = 0,
        misses = 0,
        evictions = 0,
    }, Cache)
    self.ttl = options.TTL ~= nil and read_ttl(self, options.TTL) or nil
    return self
end

local function memoize(callback, options)
    if type(callback) ~= "function" then
        error("Memoize needs a function", 2)
    end
    local cache = create(options)
    local function wrapper(key, ...)
        if select("#", ...) > 0 then
            error("memoized functions take a single argument", 2)
        end
        return cache:GetOrSet(key, callback)
    end
    return wrapper, cache
end

return {
    New = create,
    Memoize = memoize,
}
