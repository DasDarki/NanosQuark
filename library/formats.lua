---@meta

---JSON, TOML, YAML, MessagePack and CBOR share this API.
---@class QuarkDataFormat
---@field Null userdata Marker for null values
local DataFormat = {}

---@param value any
---@param options? { Pretty?: boolean, Indent?: integer, EmptyTablesAsArrays?: boolean }
---@param callback? function
---@return string?
---@return string? message
function DataFormat.Encode(value, options, callback) end

---@param input QuarkBinary
---@param options? { PreserveNull?: boolean }
---@param callback? function
---@return any
---@return string? message
function DataFormat.Decode(input, options, callback) end

---Marks a table to be encoded as an array.
---@param table? table
---@return table
function DataFormat.Array(table) end

---@param value any
---@return boolean
function DataFormat.IsNull(value) end

---@class QuarkJSONModule: QuarkDataFormat
local JSON = {}

---Compiled JSON Schema validator.
---@class QuarkJSONSchema
local Schema = {}

---@param value any
---@return boolean
---@return { Path: string, Message: string }[]?
function Schema:Validate(value) end

---@param value any
---@return boolean
function Schema:IsValid(value) end

---@param value any
---@param schema table|string
---@return boolean
---@return { Path: string, Message: string }[]?
function JSON.Validate(value, schema) end

---@param schema table|string
---@return QuarkJSONSchema?
---@return string? message
function JSON.Schema(schema) end

---@class QuarkCSVModule
local CSV = {}

---@param input QuarkBinary
---@param options? { Header?: boolean, Delimiter?: string, Trim?: boolean }
---@param callback? function
---@return table[]?
---@return string? message
function CSV.Decode(input, options, callback) end

---@param rows table[]
---@param options? { Columns?: string[], Header?: boolean, Delimiter?: string }
---@param callback? function
---@return string?
---@return string? message
function CSV.Encode(rows, options, callback) end

---@class QuarkUUIDModule
---@field Nil string
---@field Max string
local UUID = {}

---Random UUID.
---@return string
function UUID.V4() end

---Time ordered UUID.
---@return string
function UUID.V7() end

---@param text string
---@return string?
---@return string? message
function UUID.Parse(text) end

---@param value any
---@return boolean
function UUID.IsValid(value) end

---@param text string
---@return integer?
---@return string? message
function UUID.GetVersion(text) end

---@param text string
---@return integer?
---@return string? message
function UUID.GetTimestamp(text) end

---@param text string
---@return string?
---@return string? message
function UUID.ToBytes(text) end

---@param bytes QuarkBinary
---@return string?
---@return string? message
function UUID.FromBytes(bytes) end

---@class QuarkURLModule
local URL = {}

---@param text string
---@param form? boolean
---@return string
function URL.Encode(text, form) end

---@param text string
---@param form? boolean
---@return string
function URL.Decode(text, form) end

---@param parameters table
---@return string
function URL.EncodeQuery(parameters) end

---@param query string
---@return table
function URL.DecodeQuery(query) end

---@param url string
---@return { Scheme: string, Username: string, Password: string?, Host: string?, Port: integer?, Path: string, Query: string?, Fragment: string?, Origin: string }?
---@return string? message
function URL.Parse(url) end

---@param base string
---@param relative string
---@return string?
---@return string? message
function URL.Join(base, relative) end

---An XML element.
---@class QuarkXMLElement
---@field Name string
---@field Attributes table<string, string>
---@field Children (QuarkXMLElement|string)[]
local Element = {}

---@class QuarkXMLModule
local XML = {}

---@param input QuarkBinary
---@param options? { PreserveWhitespace?: boolean }
---@param callback? function
---@return QuarkXMLElement?
---@return string? message
function XML.Decode(input, options, callback) end

---@param element QuarkXMLElement
---@param options? { Pretty?: boolean, Indent?: integer, Declaration?: boolean }
---@param callback? function
---@return string?
---@return string? message
function XML.Encode(element, options, callback) end

---@param element QuarkXMLElement
---@param name string
---@return QuarkXMLElement?
function XML.Find(element, name) end

---@param element QuarkXMLElement
---@param name? string
---@return QuarkXMLElement[]
function XML.FindAll(element, name) end

---@param element QuarkXMLElement
---@return string
function XML.GetText(element) end

---@class QuarkINIModule
local INI = {}

---@param input QuarkBinary
---@param options? table
---@param callback? function
---@return table?
---@return string? message
function INI.Decode(input, options, callback) end

---@param document table
---@return string
function INI.Encode(document) end

---@class QuarkTOMLModule
local TOML = {}

---@param value table
---@param options? { Pretty?: boolean }
---@param callback? function
---@return string?
---@return string? message
function TOML.Encode(value, options, callback) end

---@param input QuarkBinary
---@param options? table
---@param callback? function
---@return table?
---@return string? message
function TOML.Decode(input, options, callback) end
