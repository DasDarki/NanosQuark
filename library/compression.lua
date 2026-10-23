---@meta

---@alias QuarkCompressionAlgorithm "lz4"|"zstd"|"gzip"|"deflate"|"zlib"|"brotli"

---Compresses or decompresses data piece by piece.
---@class QuarkCompressionStream
local Stream = {}

---Adds input, returns the output produced so far.
---@param data QuarkBinary
---@return string
function Stream:Write(data) end

---@return string
function Stream:Flush() end

---@return string
function Stream:Finish() end

---@return boolean
function Stream:IsFinished() end

---@return integer
function Stream:GetTotalIn() end

---@return integer
function Stream:GetTotalOut() end

---@class QuarkCompressionModule
local Compression = {}

---@param algorithm QuarkCompressionAlgorithm
---@param data QuarkBinary
---@param options? { Level?: integer }
---@param callback? function
---@return string?
---@return string? message
function Compression.Compress(algorithm, data, options, callback) end

---@param algorithm QuarkCompressionAlgorithm|"auto"
---@param data QuarkBinary
---@param options? { MaxSize?: integer }
---@param callback? function
---@return string?
---@return string? message
function Compression.Decompress(algorithm, data, options, callback) end

---@param data QuarkBinary
---@return QuarkCompressionAlgorithm?
function Compression.Detect(data) end

---@return QuarkCompressionAlgorithm[]
function Compression.Algorithms() end

---@param algorithm QuarkCompressionAlgorithm
---@param options? { Mode?: "Compress"|"Decompress", Level?: integer, MaxSize?: integer }
---@return QuarkCompressionStream
function Compression.Stream(algorithm, options) end

---@param algorithm QuarkCompressionAlgorithm
---@param source string
---@param target string
---@param options? { Level?: integer }
---@param callback? function
---@return { Input: integer, Output: integer }?
---@return string? message
function Compression.CompressFile(algorithm, source, target, options, callback) end

---@param algorithm QuarkCompressionAlgorithm|"auto"
---@param source string
---@param target string
---@param options? { MaxSize?: integer }
---@param callback? function
---@return { Input: integer, Output: integer }?
---@return string? message
function Compression.DecompressFile(algorithm, source, target, options, callback) end

---An archive entry.
---@class QuarkArchiveEntry
---@field Name string
---@field Data string?
---@field IsDirectory boolean
---@field IsEncrypted boolean
---@field Size integer
---@field CompressedSize integer?
---@field SafePath string?
local Entry = {}

---Zip and Tar archives in memory.
---@class QuarkArchiveModule
local Archive = {}

---@param entries table
---@param options? { Method?: "deflate"|"store", Level?: integer, Password?: string, Encryption?: "AES128"|"AES192"|"AES256", Compression?: QuarkCompressionAlgorithm }
---@param callback? function
---@return string?
---@return string? message
function Archive.Create(entries, options, callback) end

---@param archive QuarkBinary
---@param options? { MaxSize?: integer, Password?: string }
---@param callback? function
---@return QuarkArchiveEntry[]?
---@return string? message
function Archive.Extract(archive, options, callback) end

---@param archive QuarkBinary
---@param options? { Password?: string }
---@param callback? function
---@return QuarkArchiveEntry[]?
---@return string? message
function Archive.List(archive, options, callback) end
