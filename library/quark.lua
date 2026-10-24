---@meta

---@alias QuarkBinary string|QuarkBuffer
---@alias QuarkDuration number|string Milliseconds or a text like "5s" or "1m 30s".
---@alias QuarkNumberType "u8"|"i8"|"u16"|"i16"|"u32"|"i32"|"u64"|"i64"|"f32"|"f64"|"u16le"|"u16be"|"i16le"|"i16be"|"u32le"|"u32be"|"i32le"|"i32be"|"u64le"|"u64be"|"i64le"|"i64be"|"f32le"|"f32be"|"f64le"|"f64be"

---Quark, a utility library for the nanos world server.
---@class Quark
---@field Version string
---@field BigInt QuarkBigIntModule
---@field Bitset QuarkBitsetModule
---@field Buffer QuarkBufferModule
---@field CBOR QuarkDataFormat
---@field CSV QuarkCSVModule
---@field Cache QuarkCacheModule
---@field Compression QuarkCompressionModule
---@field Crypto QuarkCryptoModule
---@field Decimal QuarkDecimalModule
---@field Geometry QuarkGeometryModule
---@field HTTP QuarkHTTPModule
---@field INI QuarkINIModule
---@field Image QuarkImageModule
---@field JSON QuarkJSONModule
---@field Locale QuarkLocaleModule
---@field Log QuarkLogModule
---@field Math QuarkMathModule
---@field MQTT QuarkMQTTModule
---@field MessagePack QuarkDataFormat
---@field Metrics QuarkMetricsModule
---@field NATS QuarkNATSModule
---@field Net QuarkNetModule
---@field Pathfinding QuarkPathfindingModule
---@field PriorityQueue QuarkPriorityQueueModule
---@field ProcGen QuarkProcGenModule
---@field Promise QuarkPromiseModule
---@field Random QuarkRandomModule
---@field RateLimit QuarkRateLimitModule
---@field Redis QuarkRedisModule
---@field Regex QuarkRegexModule
---@field RingBuffer QuarkRingBufferModule
---@field Spatial QuarkSpatialModule
---@field TCP QuarkTCPModule
---@field TOML QuarkTOMLModule
---@field Tar QuarkArchiveModule
---@field Template QuarkTemplateModule
---@field Testing QuarkTestingModule
---@field Text QuarkTextModule
---@field Time QuarkTimeModule
---@field UDP QuarkUDPModule
---@field URL QuarkURLModule
---@field UUID QuarkUUIDModule
---@field WebSocket QuarkWebSocketModule
---@field Worker QuarkWorkerModule
---@field XML QuarkXMLModule
---@field YAML QuarkDataFormat
---@field Zip QuarkArchiveModule
Quark = {}

---Returns a module, raises an error if it does not exist.
---@param name string
---@return table
function Quark.Import(name) end

---Names of all available modules.
---@return string[]
function Quark.Modules() end

---Monotonic time in seconds with sub millisecond precision.
---@return number
function Quark.Clock() end

---Runs a function as a coroutine in which `Quark.Await` can be used.
---@param body fun(...): ...
---@param ... any
---@return QuarkPromise
function Quark.Async(body, ...) end

---Waits for a promise or calls a callback style function and waits for its result. Only inside `Quark.Async`.
---@param target QuarkPromise|function
---@param ... any
---@return ...
function Quark.Await(target, ...) end

---Promise resolving after the delay.
---@param milliseconds number
---@return QuarkPromise
function Quark.Delay(milliseconds) end

---Sets the configuration once, see docs/configuration.md.
---@param options { TickBudget?: integer, WorkerThreads?: integer, BlockingThreads?: integer, Network?: { AllowConnect?: string[], AllowListen?: (integer|string)[] } }
function Quark.Configure(options) end

---The current configuration.
---@return { TickBudget: integer, WorkerThreads: integer, BlockingThreads: integer, Locked: boolean, Source: string?, Network: { AllowConnect: string[]?, AllowListen: string[]? } }
function Quark.GetConfiguration() end

---Handle of asynchronous work started with a callback.
---@class QuarkTask
local Task = {}

---Stops waiting for the result, the callback is never called. Returns whether the task was pending.
---@return boolean
function Task:Cancel() end

---@return boolean
function Task:IsDone() end

---@return boolean
function Task:IsCancelled() end

---@class QuarkPromise
local Promise = {}

---@param on_fulfilled? fun(...): ...
---@param on_rejected? fun(reason: any): ...
---@return QuarkPromise
function Promise:Then(on_fulfilled, on_rejected) end

---@param on_rejected fun(reason: any): ...
---@return QuarkPromise
function Promise:Catch(on_rejected) end

---@param callback fun()
---@return QuarkPromise
function Promise:Finally(callback) end

---Rejects a pending promise and cancels its native task.
---@param reason? any
---@return boolean
function Promise:Cancel(reason) end

---@return "pending"|"fulfilled"|"rejected"
function Promise:GetState() end

---@class QuarkPromiseModule
local PromiseModule = {}

---@param executor fun(resolve: (fun(...)), reject: fun(reason: any))
---@return QuarkPromise
function PromiseModule.New(executor) end

---@param ... any
---@return QuarkPromise
function PromiseModule.Resolve(...) end

---@param reason any
---@return QuarkPromise
function PromiseModule.Reject(reason) end

---Wraps a callback style function, `callback(nil, error)` rejects.
---@param callback_function function
---@param ... any
---@return QuarkPromise
function PromiseModule.From(callback_function, ...) end

---@param promises QuarkPromise[]
---@return QuarkPromise
function PromiseModule.All(promises) end

---@param promises QuarkPromise[]
---@return QuarkPromise
function PromiseModule.AllSettled(promises) end

---@param promises QuarkPromise[]
---@return QuarkPromise
function PromiseModule.Race(promises) end

---@param value any
---@return boolean
function PromiseModule.IsPromise(value) end

---@param milliseconds number
---@return QuarkPromise
function PromiseModule.Delay(milliseconds) end

---@param body fun(...): ...
---@param ... any
---@return QuarkPromise
function PromiseModule.Async(body, ...) end

---@param target QuarkPromise|function
---@param ... any
---@return ...
function PromiseModule.Await(target, ...) end

---Mutable byte array, offsets start at the index base of the package (0 by default).
---@class QuarkBuffer
local Buffer = {}

---@param kind QuarkNumberType
---@param offset integer
---@return number
function Buffer:Read(kind, offset) end

---@param kind QuarkNumberType
---@param offset integer
---@param value number
function Buffer:Write(kind, offset, value) end

---@param kind QuarkNumberType
---@param offset integer
---@param count integer
---@return number[]
function Buffer:ReadArray(kind, offset, count) end

---@param kind QuarkNumberType
---@param offset integer
---@param values number[]
function Buffer:WriteArray(kind, offset, values) end

---Writes a number at the end.
---@param kind QuarkNumberType
---@param value number
---@return QuarkBuffer
function Buffer:Append(kind, value) end

---@param offset integer
---@param length? integer
---@return string
function Buffer:ReadString(offset, length) end

---@param offset integer
---@param data QuarkBinary
function Buffer:WriteString(offset, data) end

---@param data QuarkBinary
---@return QuarkBuffer
function Buffer:AppendString(data) end

---@param start? integer
---@param finish? integer
---@return string
function Buffer:GetBytes(start, finish) end

---@param start? integer
---@param finish? integer
---@return QuarkBuffer
function Buffer:Slice(start, finish) end

---@param source QuarkBinary
---@param offset? integer
---@param source_start? integer
---@param source_finish? integer
function Buffer:CopyFrom(source, offset, source_start, source_finish) end

---@param byte integer
---@param start? integer
---@param finish? integer
function Buffer:Fill(byte, start, finish) end

---@param size integer
---@param fill? integer
function Buffer:Resize(size, fill) end

---@return integer
function Buffer:GetSize() end

---@return string
function Buffer:ToHex() end

---@param other QuarkBuffer
---@return boolean
function Buffer:Equals(other) end

---@return QuarkBuffer
function Buffer:Clone() end

---@param base 0|1
---@return QuarkBuffer
function Buffer:SetIndexBase(base) end

---@return 0|1
function Buffer:GetIndexBase() end

---Typed view on the buffer.
---@param kind QuarkNumberType
---@param offset? integer
---@param count? integer
---@return QuarkBufferView
function Buffer:View(kind, offset, count) end

---Array of numbers of one type inside a buffer, `view[index]` reads and writes values.
---@class QuarkBufferView
---@field [integer] number
local BufferView = {}

---@param index integer
---@return number
function BufferView:Get(index) end

---@param index integer
---@param value number
function BufferView:Set(index, value) end

---@return number[]
function BufferView:ToTable() end

---@param values number[]
function BufferView:FromTable(values) end

---@param value number
function BufferView:Fill(value) end

---Multiplies every value and adds `offset`.
---@param factor number
---@param offset? number
function BufferView:Scale(factor, offset) end

---@return number
function BufferView:Sum() end

---@return number?
function BufferView:Average() end

---@return number? value
---@return integer? index
function BufferView:Min() end

---@return number? value
---@return integer? index
function BufferView:Max() end

---@param descending? boolean
function BufferView:Sort(descending) end

---@return integer
function BufferView:GetCount() end

---@return string
function BufferView:GetType() end

---@return QuarkBuffer
function BufferView:GetBuffer() end

---@class QuarkBufferModule
local BufferModule = {}

---@param size? integer
---@param fill? integer
---@return QuarkBuffer
function BufferModule.New(size, fill) end

---@param data QuarkBinary
---@return QuarkBuffer
function BufferModule.FromString(data) end

---@param kind QuarkNumberType
---@param values number[]
---@return QuarkBuffer
function BufferModule.FromArray(kind, values) end

---@param parts QuarkBinary[]
---@return QuarkBuffer
function BufferModule.Concat(parts) end

---@param value any
---@return boolean
function BufferModule.IsBuffer(value) end

---Index base for buffers created by the calling package.
---@param base 0|1
function BufferModule.SetIndexBase(base) end

---@return 0|1
function BufferModule.GetIndexBase() end

---Objects with events, see docs/net.md#events.
---@class QuarkEmitter
local Emitter = {}

---Adds an event handler and returns it.
---@param event string
---@param handler function
---@return function
function Emitter:Subscribe(event, handler) end

---Removes one handler, or all handlers of the event.
---@param event string
---@param handler? function
function Emitter:Unsubscribe(event, handler) end
