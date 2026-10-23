---@meta

---@alias QuarkLogLevel "Debug"|"Info"|"Warn"|"Error"

---Token bucket or sliding window limiter.
---@class QuarkRateLimiter
local RateLimiter = {}

---@param key any
---@param cost? number
---@return boolean allowed
---@return integer retry_after
---@return number remaining
function RateLimiter:Take(key, cost) end

---@param key any
---@param cost? number
---@return boolean allowed
---@return integer retry_after
---@return number remaining
function RateLimiter:Check(key, cost) end

---@param key any
---@return number
function RateLimiter:GetRemaining(key) end

---@param key any
function RateLimiter:Reset(key) end

function RateLimiter:Clear() end

function RateLimiter:Prune() end

---@return integer
function RateLimiter:GetCount() end

---Rate limit stored in Redis.
---@class QuarkSharedRateLimiter
local SharedRateLimiter = {}

---@param key string|number
---@param cost? number
---@param callback? function
---@return boolean? allowed
---@return integer|string? retry_after_or_message
---@return number? remaining
function SharedRateLimiter:Take(key, cost, callback) end

---@param key string|number
---@param cost? number
---@param callback? function
---@return boolean? allowed
---@return integer|string? retry_after_or_message
---@return number? remaining
function SharedRateLimiter:Check(key, cost, callback) end

---@param key string|number
---@param callback? function
---@return boolean?
---@return string? message
function SharedRateLimiter:Reset(key, callback) end

---@class QuarkRateLimitModule
local RateLimit = {}

---@param options { Capacity: number, Per?: QuarkDuration, Rate?: number, Clock?: fun(): number }
---@return QuarkRateLimiter
function RateLimit.TokenBucket(options) end

---@param options { Limit: number, Window: QuarkDuration, Clock?: fun(): number }
---@return QuarkRateLimiter
function RateLimit.SlidingWindow(options) end

---@param client QuarkRedisClient
---@param options { Limit?: integer, Window?: QuarkDuration, Capacity?: number, Per?: QuarkDuration, Rate?: number, Prefix?: string }
---@return QuarkSharedRateLimiter
function RateLimit.Redis(client, options) end

---LRU cache with expiry.
---@class QuarkCache
local Cache = {}

---@param key any
---@return any
function Cache:Get(key) end

---@param key any
---@return any
function Cache:Peek(key) end

---@param key any
---@return boolean
function Cache:Has(key) end

---@param key any
---@param value any
---@param ttl? QuarkDuration|false
---@return QuarkCache
function Cache:Set(key, value, ttl) end

---@param key any
---@param factory fun(key: any): any
---@param ttl? QuarkDuration|false
---@return any
function Cache:GetOrSet(key, factory, ttl) end

---@param key any
---@return boolean
function Cache:Delete(key) end

function Cache:Clear() end

---@return integer
function Cache:Prune() end

---@return integer
function Cache:GetSize() end

---@return integer?
function Cache:GetCapacity() end

---@return any[]
function Cache:GetKeys() end

---@param key any
---@return number?
function Cache:GetTTL(key) end

---@return { Hits: integer, Misses: integer, Evictions: integer, HitRate: number }
function Cache:GetStats() end

---Cache stored in Redis as JSON.
---@class QuarkSharedCache
local SharedCache = {}

---@param key string|number
---@param callback? function
---@return any
---@return string? message
function SharedCache:Get(key, callback) end

---@param key string|number
---@param value any
---@param ttl? QuarkDuration|false
---@param callback? function
---@return boolean?
---@return string? message
function SharedCache:Set(key, value, ttl, callback) end

---@param key string|number
---@param factory fun(key: string|number): any
---@param ttl? QuarkDuration|false
---@param callback? function
---@return any
---@return string? message
function SharedCache:GetOrSet(key, factory, ttl, callback) end

---@param key string|number
---@param callback? function
---@return boolean?
---@return string? message
function SharedCache:Has(key, callback) end

---@param key string|number
---@param callback? function
---@return boolean?
---@return string? message
function SharedCache:Delete(key, callback) end

---@class QuarkCacheModule
local CacheModule = {}

---@param options? { Capacity?: integer, TTL?: QuarkDuration, OnEvict?: (fun(key: any, value: any, reason: "Capacity"|"Expired"|"Replaced"|"Deleted")), Clock?: fun(): number }
---@return QuarkCache
function CacheModule.New(options) end

---@param callback fun(key: any): any
---@param options? { Capacity?: integer, TTL?: QuarkDuration }
---@return fun(key: any): any
---@return QuarkCache
function CacheModule.Memoize(callback, options) end

---@param client QuarkRedisClient
---@param options? { Prefix?: string, TTL?: QuarkDuration }
---@return QuarkSharedCache
function CacheModule.Redis(client, options) end

---@class QuarkPriorityQueue
local PriorityQueue = {}

---@param value any
---@param priority number
---@return QuarkPriorityQueue
function PriorityQueue:Push(value, priority) end

---@return any
---@return number? priority
function PriorityQueue:Pop() end

---@return any
---@return number? priority
function PriorityQueue:Peek() end

---@param value any
---@return boolean
function PriorityQueue:Remove(value) end

---@return integer
function PriorityQueue:GetSize() end

---@return boolean
function PriorityQueue:IsEmpty() end

function PriorityQueue:Clear() end

---@return { Value: any, Priority: number }[]
function PriorityQueue:ToList() end

---@class QuarkPriorityQueueModule
local PriorityQueueModule = {}

---@param options? { Order?: "Min"|"Max" }
---@return QuarkPriorityQueue
function PriorityQueueModule.New(options) end

---@class QuarkRingBuffer
local RingBuffer = {}

---@param value any
---@return any? evicted
function RingBuffer:Push(value) end

---@return any
function RingBuffer:Shift() end

---@return any
function RingBuffer:Pop() end

---@param index integer
---@return any
function RingBuffer:Get(index) end

---@param index integer
---@param value any
function RingBuffer:Set(index, value) end

---@return any
function RingBuffer:GetFirst() end

---@return any
function RingBuffer:GetLast() end

---@return integer
function RingBuffer:GetSize() end

---@return integer
function RingBuffer:GetCapacity() end

---@return boolean
function RingBuffer:IsEmpty() end

---@return boolean
function RingBuffer:IsFull() end

function RingBuffer:Clear() end

---@return any[]
function RingBuffer:ToList() end

---@return fun(): integer?, any
function RingBuffer:Values() end

---@class QuarkRingBufferModule
local RingBufferModule = {}

---@param capacity integer
---@return QuarkRingBuffer
function RingBufferModule.New(capacity) end

---Compact native booleans, indexed from the package index base.
---@class QuarkBitset
local Bitset = {}

---@param index integer
---@return boolean
function Bitset:Get(index) end

---@param index integer
---@param value? boolean
function Bitset:Set(index, value) end

---@param index integer
function Bitset:Unset(index) end

---@param index integer
---@return boolean
function Bitset:Toggle(index) end

---@param value? boolean
function Bitset:SetAll(value) end

---@return QuarkBitset
function Bitset:Invert() end

---@return integer
function Bitset:Count() end

---@return boolean
function Bitset:Any() end

---@return boolean
function Bitset:None() end

---@return boolean
function Bitset:All() end

---@param start? integer
---@return integer?
function Bitset:NextSet(start) end

---@param start? integer
---@return integer?
function Bitset:NextUnset(start) end

---@return integer[]
function Bitset:GetIndices() end

---@param other QuarkBitset
---@return QuarkBitset
function Bitset:Union(other) end

---@param other QuarkBitset
---@return QuarkBitset
function Bitset:Intersect(other) end

---@param other QuarkBitset
---@return QuarkBitset
function Bitset:Difference(other) end

---@param other QuarkBitset
---@return QuarkBitset
function Bitset:SymmetricDifference(other) end

---@return QuarkBitset
function Bitset:Clone() end

---@param size integer
function Bitset:Resize(size) end

---@return integer
function Bitset:GetSize() end

---@return string
function Bitset:ToBytes() end

---@class QuarkBitsetModule
local BitsetModule = {}

---@param size integer
---@return QuarkBitset
function BitsetModule.New(size) end

---@param data QuarkBinary
---@param size? integer
---@return QuarkBitset
function BitsetModule.FromBytes(data, size) end

---@param value any
---@return boolean
function BitsetModule.IsBitset(value) end

---@class QuarkCounter
local Counter = {}

---@param amount_or_labels? number|table
---@param labels? table
function Counter:Inc(amount_or_labels, labels) end

---@param labels? table
---@return number
function Counter:Get(labels) end

function Counter:Reset() end

---@class QuarkGauge
local Gauge = {}

---@param value number
---@param labels? table
function Gauge:Set(value, labels) end

---@param amount_or_labels? number|table
---@param labels? table
function Gauge:Inc(amount_or_labels, labels) end

---@param amount_or_labels? number|table
---@param labels? table
function Gauge:Dec(amount_or_labels, labels) end

---@param labels? table
---@return number
function Gauge:Get(labels) end

function Gauge:Reset() end

---@class QuarkHistogram
local Histogram = {}

---@param value number
---@param labels? table
function Histogram:Observe(value, labels) end

---@param callback fun(): ...
---@param labels? table
---@return ...
function Histogram:Time(callback, labels) end

---@param labels? table
---@return { Count: integer, Sum: number }
function Histogram:Get(labels) end

function Histogram:Reset() end

---@class QuarkMetricsRegistry
local Registry = {}

---@param name string
---@param help? string
---@param options? { Labels?: string[], Buckets?: number[], Collect?: fun(): number|{ Labels: table, Value: number }[] }
---@return QuarkCounter
function Registry:Counter(name, help, options) end

---@param name string
---@param help? string
---@param options? { Labels?: string[], Buckets?: number[], Collect?: fun(): number|{ Labels: table, Value: number }[] }
---@return QuarkGauge
function Registry:Gauge(name, help, options) end

---@param name string
---@param help? string
---@param options? { Labels?: string[], Buckets?: number[], Collect?: fun(): number|{ Labels: table, Value: number }[] }
---@return QuarkHistogram
function Registry:Histogram(name, help, options) end

---@param name string
---@return QuarkCounter|QuarkGauge|QuarkHistogram?
function Registry:Get(name) end

---@param name string
---@return boolean
function Registry:Unregister(name) end

function Registry:Clear() end

---@return string
function Registry:Render() end

---@param server QuarkHTTPServer
---@param path? string
---@return QuarkHTTPServer
function Registry:Expose(server, path) end

---Prometheus metrics, functions use the default registry.
---@class QuarkMetricsModule
local Metrics = {}

---@param name string
---@param help? string
---@param options? { Labels?: string[], Buckets?: number[], Collect?: fun(): number|{ Labels: table, Value: number }[] }
---@return QuarkCounter
function Metrics.Counter(name, help, options) end

---@param name string
---@param help? string
---@param options? { Labels?: string[], Buckets?: number[], Collect?: fun(): number|{ Labels: table, Value: number }[] }
---@return QuarkGauge
function Metrics.Gauge(name, help, options) end

---@param name string
---@param help? string
---@param options? { Labels?: string[], Buckets?: number[], Collect?: fun(): number|{ Labels: table, Value: number }[] }
---@return QuarkHistogram
function Metrics.Histogram(name, help, options) end

---@param name string
---@return QuarkCounter|QuarkGauge|QuarkHistogram?
function Metrics.Get(name) end

---@param name string
---@return boolean
function Metrics.Unregister(name) end

function Metrics.Clear() end

---@return string
function Metrics.Render() end

---@param server QuarkHTTPServer
---@param path? string
---@return QuarkHTTPServer
function Metrics.Expose(server, path) end

---@return QuarkMetricsRegistry
function Metrics.Registry() end

---@return QuarkMetricsRegistry
function Metrics.GetDefault() end

---@class QuarkLogger
local Logger = {}

---@param message any
---@param fields? table
function Logger:Debug(message, fields) end

---@param message any
---@param fields? table
function Logger:Info(message, fields) end

---@param message any
---@param fields? table
function Logger:Warn(message, fields) end

---@param message any
---@param fields? table
function Logger:Error(message, fields) end

---@param level QuarkLogLevel
---@param message any
---@param fields? table
function Logger:Log(level, message, fields) end

---@param fields table
---@return QuarkLogger
function Logger:With(fields) end

---@param level QuarkLogLevel
function Logger:SetLevel(level) end

---@return QuarkLogLevel
function Logger:GetLevel() end

---@param level QuarkLogLevel
---@return boolean
function Logger:IsEnabled(level) end

function Logger:Flush() end

---@class QuarkLogModule
local Log = {}

---@param options? { Name?: string, Level?: QuarkLogLevel, Format?: "Text"|"Json", Fields?: table, Console?: boolean, File?: string, MaxFileSize?: integer, MaxFiles?: integer }
---@return QuarkLogger
function Log.New(options) end

---@param message any
---@param fields? table
function Log.Debug(message, fields) end

---@param message any
---@param fields? table
function Log.Info(message, fields) end

---@param message any
---@param fields? table
function Log.Warn(message, fields) end

---@param message any
---@param fields? table
function Log.Error(message, fields) end
