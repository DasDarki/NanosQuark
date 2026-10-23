---@meta

---Translations with Fluent.
---@class QuarkLocale
local Locale = {}

---@param language string
---@param id string
---@param arguments? table
---@return string text
---@return boolean found
---@return string? problems
function Locale:Format(language, id, arguments) end

---@param language string
---@return fun(id: string, arguments?: table): string, boolean, string?
function Locale:Bind(language) end

---@param language string
---@param source string
---@return true?
---@return string? message
function Locale:Add(language, source) end

---@param language string
---@param id string
---@return boolean
function Locale:Has(language, id) end

---@param requested string|string[]
---@return string?
function Locale:Negotiate(requested) end

---@return string[]
function Locale:GetLocales() end

---@return string?
function Locale:GetFallback() end

---@class QuarkLocaleModule
local LocaleModule = {}

---@param options? { Resources?: table<string, string>, Fallback?: string, Isolating?: boolean }
---@return QuarkLocale?
---@return string? message
function LocaleModule.New(options) end

---@param language string
---@param number number
---@param type? "Cardinal"|"Ordinal"
---@return "zero"|"one"|"two"|"few"|"many"|"other"
function LocaleModule.PluralCategory(language, number, type) end

---@param requested string|string[]
---@param available string[]
---@param fallback? string
---@return string?
function LocaleModule.Negotiate(requested, available, fallback) end
