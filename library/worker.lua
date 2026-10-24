---@meta

---@alias QuarkWorkerValue nil|boolean|number|string|table

---@class QuarkWorkerOptions
---@field Timeout? integer|string
---@field MemoryLimit? integer

---@class QuarkWorkerPool
local Pool = {}

---Calls a global function of the pool code, the last argument has to be the callback.
---@param name string
---@param ... QuarkWorkerValue|fun(...: QuarkWorkerValue)
---@return boolean
function Pool:Call(name, ...) end

---@return integer
function Pool:GetSize() end

---@return integer
function Pool:GetPending() end

function Pool:Close() end

---@class QuarkWorkerModule
local WorkerModule = {}

---@param code string
---@param data? QuarkWorkerValue
---@param options? QuarkWorkerOptions|function
---@param callback? fun(...: QuarkWorkerValue)
---@return ...
function WorkerModule.Run(code, data, options, callback) end

---@param code string
---@param options? QuarkWorkerOptions|{ Size?: integer }
---@return QuarkWorkerPool?
---@return string? message
function WorkerModule.Pool(code, options) end
