---@meta

---@alias QuarkRedisKey string|number|QuarkBuffer
---@alias QuarkRedisTarget string|string[]|{ Sentinels: string[], MasterName: string }
---@alias QuarkStreamEntry { ID: string, Fields: table<string, string> }

---Redis client, every method takes an optional callback as last argument.
---@class QuarkRedisClient
local Client = {}

---Any Redis command.
---@param name string
---@param ... any Arguments, optionally followed by a callback
---@return any
---@return string? message
function Client:Command(name, ...) end

---@param key QuarkRedisKey
---@param callback? function
---@return string?
---@return string? message
function Client:Get(key, callback) end

---@param key QuarkRedisKey
---@param value QuarkRedisKey
---@param options? { EX?: integer, PX?: integer, NX?: boolean, XX?: boolean, KeepTTL?: boolean }
---@param callback? function
---@return boolean?
---@return string? message
function Client:Set(key, value, options, callback) end

---@param key QuarkRedisKey
---@param ... any Arguments, optionally followed by a callback
---@return integer?
---@return string? message
function Client:Delete(key, ...) end

---@param key QuarkRedisKey
---@param callback? function
---@return boolean?
---@return string? message
function Client:Exists(key, callback) end

---@param key QuarkRedisKey
---@param seconds integer
---@param callback? function
---@return boolean?
---@return string? message
function Client:Expire(key, seconds, callback) end

---@param key QuarkRedisKey
---@param callback? function
---@return integer?
---@return string? message
function Client:TTL(key, callback) end

---@param key QuarkRedisKey
---@param by? integer
---@param callback? function
---@return integer?
---@return string? message
function Client:Increment(key, by, callback) end

---@param key QuarkRedisKey
---@param by? integer
---@param callback? function
---@return integer?
---@return string? message
function Client:Decrement(key, by, callback) end

---@param key QuarkRedisKey
---@param field QuarkRedisKey
---@param callback? function
---@return string?
---@return string? message
function Client:HashGet(key, field, callback) end

---@param key QuarkRedisKey
---@param field_or_fields QuarkRedisKey|table
---@param value? QuarkRedisKey
---@param callback? function
---@return integer?
---@return string? message
function Client:HashSet(key, field_or_fields, value, callback) end

---@param key QuarkRedisKey
---@param callback? function
---@return table<string, string>?
---@return string? message
function Client:HashGetAll(key, callback) end

---@param key QuarkRedisKey
---@param field QuarkRedisKey
---@param ... any Arguments, optionally followed by a callback
---@return integer?
---@return string? message
function Client:HashDelete(key, field, ...) end

---@param key QuarkRedisKey
---@param field QuarkRedisKey
---@param callback? function
---@return boolean?
---@return string? message
function Client:HashExists(key, field, callback) end

---@param key QuarkRedisKey
---@param field QuarkRedisKey
---@param by? integer
---@param callback? function
---@return integer?
---@return string? message
function Client:HashIncrement(key, field, by, callback) end

---@param key QuarkRedisKey
---@param ... any Arguments, optionally followed by a callback
---@return integer?
---@return string? message
function Client:ListPush(key, ...) end

---@param key QuarkRedisKey
---@param ... any Arguments, optionally followed by a callback
---@return integer?
---@return string? message
function Client:ListPushFront(key, ...) end

---@param key QuarkRedisKey
---@param callback? function
---@return string?
---@return string? message
function Client:ListPop(key, callback) end

---@param key QuarkRedisKey
---@param callback? function
---@return string?
---@return string? message
function Client:ListPopBack(key, callback) end

---@param key QuarkRedisKey
---@param start integer
---@param stop integer
---@param callback? function
---@return string[]?
---@return string? message
function Client:ListRange(key, start, stop, callback) end

---@param key QuarkRedisKey
---@param callback? function
---@return integer?
---@return string? message
function Client:ListLength(key, callback) end

---@param key QuarkRedisKey
---@param ... any Arguments, optionally followed by a callback
---@return integer?
---@return string? message
function Client:SetAdd(key, ...) end

---@param key QuarkRedisKey
---@param ... any Arguments, optionally followed by a callback
---@return integer?
---@return string? message
function Client:SetRemove(key, ...) end

---@param key QuarkRedisKey
---@param callback? function
---@return string[]?
---@return string? message
function Client:SetMembers(key, callback) end

---@param key QuarkRedisKey
---@param member QuarkRedisKey
---@param callback? function
---@return boolean?
---@return string? message
function Client:SetContains(key, member, callback) end

---@param key QuarkRedisKey
---@param member QuarkRedisKey
---@param score number
---@param callback? function
---@return integer?
---@return string? message
function Client:SortedSetAdd(key, member, score, callback) end

---@param key QuarkRedisKey
---@param member QuarkRedisKey
---@param by number
---@param callback? function
---@return number?
---@return string? message
function Client:SortedSetIncrement(key, member, by, callback) end

---@param key QuarkRedisKey
---@param member QuarkRedisKey
---@param callback? function
---@return number?
---@return string? message
function Client:SortedSetScore(key, member, callback) end

---@param key QuarkRedisKey
---@param member QuarkRedisKey
---@param options? { Reverse?: boolean }
---@param callback? function
---@return integer?
---@return string? message
function Client:SortedSetRank(key, member, options, callback) end

---@param key QuarkRedisKey
---@param start integer
---@param stop integer
---@param options? { Reverse?: boolean, WithScores?: boolean }
---@param callback? function
---@return table?
---@return string? message
function Client:SortedSetRange(key, start, stop, options, callback) end

---@param key QuarkRedisKey
---@param ... any Arguments, optionally followed by a callback
---@return integer?
---@return string? message
function Client:SortedSetRemove(key, ...) end

---@param key QuarkRedisKey
---@param callback? function
---@return integer?
---@return string? message
function Client:SortedSetCount(key, callback) end

---@param channel string
---@param message QuarkRedisKey
---@param callback? function
---@return integer?
---@return string? message
function Client:Publish(channel, message, callback) end

---@param commands table[]
---@param options? { Atomic?: boolean }
---@param callback? function
---@return any[]?
---@return string? message
function Client:Pipeline(commands, options, callback) end

---@param key QuarkRedisKey
---@param fields table
---@param options? { ID?: string, MaxLength?: integer, Approximate?: boolean }
---@param callback? function
---@return string?
---@return string? message
function Client:StreamAdd(key, fields, options, callback) end

---@param key QuarkRedisKey
---@param start? string
---@param stop? string
---@param options? { Count?: integer, Reverse?: boolean }
---@param callback? function
---@return QuarkStreamEntry[]?
---@return string? message
function Client:StreamRange(key, start, stop, options, callback) end

---@param streams table<string, string>
---@param options? { Count?: integer, Block?: QuarkDuration }
---@param callback? function
---@return table<string, QuarkStreamEntry[]>?
---@return string? message
function Client:StreamRead(streams, options, callback) end

---@param key QuarkRedisKey
---@param callback? function
---@return integer?
---@return string? message
function Client:StreamLength(key, callback) end

---@param key QuarkRedisKey
---@param id string
---@param ... any Arguments, optionally followed by a callback
---@return integer?
---@return string? message
function Client:StreamDelete(key, id, ...) end

---@param key QuarkRedisKey
---@param max_length integer
---@param options? { Approximate?: boolean }
---@param callback? function
---@return integer?
---@return string? message
function Client:StreamTrim(key, max_length, options, callback) end

---@param key QuarkRedisKey
---@param group string
---@param options? { ID?: string, MakeStream?: boolean }
---@param callback? function
---@return boolean?
---@return string? message
function Client:StreamCreateGroup(key, group, options, callback) end

---@param group string
---@param consumer string
---@param streams table|string[]
---@param options? { Count?: integer, Block?: QuarkDuration, NoAck?: boolean }
---@param callback? function
---@return table<string, QuarkStreamEntry[]>?
---@return string? message
function Client:StreamReadGroup(group, consumer, streams, options, callback) end

---@param key QuarkRedisKey
---@param group string
---@param id string
---@param ... any Arguments, optionally followed by a callback
---@return integer?
---@return string? message
function Client:StreamAck(key, group, id, ...) end

---@param key QuarkRedisKey
---@param group string
---@param callback? function
---@return { Count: integer, First: string?, Last: string?, Consumers: table<string, integer> }?
---@return string? message
function Client:StreamPending(key, group, callback) end

---@param key QuarkRedisKey
---@param group string
---@param consumer string
---@param min_idle integer
---@param options? { Start?: string, Count?: integer }
---@param callback? function
---@return { Entries: QuarkStreamEntry[], Next: string }?
---@return string? message
function Client:StreamClaim(key, group, consumer, min_idle, options, callback) end

---@return boolean
function Client:IsClosed() end

function Client:Close() end

---Events: Connect, Message(channel, message, pattern), Disconnect(reason), Error(message), Close(reason).
---@class QuarkRedisSubscriber: QuarkEmitter
local Subscriber = {}

---@param channel string
---@param ... string
function Subscriber:Listen(channel, ...) end

---@param channel string
---@param ... string
function Subscriber:Unlisten(channel, ...) end

---@param channel string
---@param ... string
function Subscriber:ListenPattern(channel, ...) end

---@param channel string
---@param ... string
function Subscriber:UnlistenPattern(channel, ...) end

---@return string[]
function Subscriber:GetChannels() end

---@return string[]
function Subscriber:GetPatterns() end

---@return "connecting"|"open"|"closed"
function Subscriber:GetState() end

---@return boolean
function Subscriber:IsOpen() end

function Subscriber:Close() end

---Events: Entry(stream, id, fields), Connect, Disconnect(reason), Error(message), Close(reason).
---@class QuarkRedisStreamReader: QuarkEmitter
local StreamReader = {}

---@return "connecting"|"open"|"closed"
function StreamReader:GetState() end

---@return boolean
function StreamReader:IsOpen() end

function StreamReader:Close() end

---@class QuarkRedisModule
local Redis = {}

---@param target? QuarkRedisTarget
---@param options? { Username?: string, Password?: string, Database?: integer, Timeout?: QuarkDuration, Retries?: integer, CA?: string, TLS?: boolean, ReadFromReplicas?: boolean }
---@return QuarkRedisClient
function Redis.Connect(target, options) end

---@param target? QuarkRedisTarget
---@param options? { Username?: string, Password?: string, Database?: integer, Timeout?: QuarkDuration, Retries?: integer, CA?: string, TLS?: boolean, ReadFromReplicas?: boolean }
---@return QuarkRedisSubscriber
function Redis.Subscriber(target, options) end

---@param target QuarkRedisTarget
---@param options { Streams: string|string[], Start?: string, Group?: string, Consumer?: string, CreateGroup?: boolean, AutoAck?: boolean, Count?: integer, Block?: QuarkDuration, Username?: string, Password?: string, Database?: integer, Timeout?: QuarkDuration, CA?: string }
---@return QuarkRedisStreamReader
function Redis.StreamReader(target, options) end
