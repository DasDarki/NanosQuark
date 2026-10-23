---@meta

---A regular expression match.
---@class QuarkRegexMatch
---@field Text string
---@field Start integer
---@field End integer
---@field Groups (string|false)[]
---@field Named table<string, string>
local Match = {}

---Compiled regular expression.
---@class QuarkRegex
local Regex = {}

---@param text string
---@param start? integer
---@return boolean
function Regex:IsMatch(text, start) end

---@param text string
---@param start? integer
---@return integer? start
---@return integer? finish
---@return string? text
function Regex:Find(text, start) end

---@param text string
---@param start? integer
---@return QuarkRegexMatch?
function Regex:Match(text, start) end

---@param text string
---@param limit? integer
---@return QuarkRegexMatch[]
function Regex:MatchAll(text, limit) end

---@param text string
---@param replacement string|table|fun(match: QuarkRegexMatch): string?
---@param limit? integer
---@return string
---@return integer count
function Regex:Replace(text, replacement, limit) end

---@param text string
---@param limit? integer
---@return string[]
function Regex:Split(text, limit) end

---@param text string
---@return integer
function Regex:Count(text) end

---@return string
function Regex:GetPattern() end

---@return integer
function Regex:GetGroupCount() end

---@return string[]
function Regex:GetGroupNames() end

---@class QuarkRegexModule
local RegexModule = {}

---@param pattern string
---@param options? { CaseInsensitive?: boolean, Multiline?: boolean, DotAll?: boolean, Extended?: boolean, Ascii?: boolean }
---@return QuarkRegex?
---@return string? message
function RegexModule.New(pattern, options) end

---@param pattern string
---@return boolean
function RegexModule.IsValid(pattern) end

---@param text string
---@return string
function RegexModule.Escape(text) end

---@param pattern string
---@param text string
---@param start? integer
---@return boolean
function RegexModule.IsMatch(pattern, text, start) end

---@param pattern string
---@param text string
---@param start? integer
---@return integer? start
---@return integer? finish
---@return string? text
function RegexModule.Find(pattern, text, start) end

---@param pattern string
---@param text string
---@param start? integer
---@return QuarkRegexMatch?
function RegexModule.Match(pattern, text, start) end

---@param pattern string
---@param text string
---@param limit? integer
---@return QuarkRegexMatch[]
function RegexModule.MatchAll(pattern, text, limit) end

---@param pattern string
---@param text string
---@param replacement string|table|fun(match: QuarkRegexMatch): string?
---@param limit? integer
---@return string
---@return integer count
function RegexModule.Replace(pattern, text, replacement, limit) end

---@param pattern string
---@param text string
---@param limit? integer
---@return string[]
function RegexModule.Split(pattern, text, limit) end

---@class QuarkWordFilter
local WordFilter = {}

---@param text string
---@return boolean
function WordFilter:Contains(text) end

---@param text string
---@return { Word: string, Text: string, Start: integer, End: integer }[]
function WordFilter:Find(text) end

---@param text string
---@param replacement? string
---@return string
---@return integer count
function WordFilter:Censor(text, replacement) end

---@param words string[]
function WordFilter:Add(words) end

---@param words string[]
function WordFilter:Remove(words) end

---@return string[]
function WordFilter:GetWords() end

---Unicode aware text helpers.
---@class QuarkTextModule
local Text = {}

---@param text string
---@return string
function Text.Upper(text) end

---@param text string
---@return string
function Text.Lower(text) end

---@param text string
---@return string
function Text.Capitalize(text) end

---@param text string
---@return string
function Text.ToAscii(text) end

---@param text string
---@return string
function Text.Reverse(text) end

---@param text string
---@return string
function Text.Trim(text) end

---@param text string
---@param form? "NFC"|"NFD"|"NFKC"|"NFKD"
---@return string
function Text.Normalize(text, form) end

---@param text string
---@param separator? string
---@return string
function Text.Slugify(text, separator) end

---@param text string
---@return boolean
function Text.IsValid(text) end

---@param text string
---@return integer
function Text.Length(text) end

---@param text string
---@return string[]
function Text.Graphemes(text) end

---@param text string
---@return string[]
function Text.Words(text) end

---@param text string
---@param start integer
---@param finish? integer
---@return string
function Text.Sub(text, start, finish) end

---@param text string
---@param length integer
---@param suffix? string
---@return string
function Text.Truncate(text, length, suffix) end

---@param text string
---@param separator string
---@param options? { Trim?: boolean, RemoveEmpty?: boolean, Limit?: integer }
---@return string[]
function Text.Split(text, separator, options) end

---@param text string
---@param width integer
---@param options? { Align?: "Left"|"Right"|"Center", Char?: string }
---@return string
function Text.Pad(text, width, options) end

---@param text string
---@param width integer
---@return string[]
function Text.Wrap(text, width) end

---@param first string
---@param second string
---@return integer
function Text.Levenshtein(first, second) end

---@param first string
---@param second string
---@return integer
function Text.DamerauLevenshtein(first, second) end

---@param first string
---@param second string
---@param algorithm? "JaroWinkler"|"Jaro"|"Levenshtein"|"DamerauLevenshtein"|"SorensenDice"
---@return number
function Text.Similarity(first, second, algorithm) end

---@param query string
---@param list any[]
---@param options? { Threshold?: number, Limit?: integer, Key?: string, CaseSensitive?: boolean }
---@return { Item: any, Score: number, Index: integer }[]
function Text.FuzzyFind(query, list, options) end

---@param query string
---@param list any[]
---@param options? { Threshold?: number, Limit?: integer, Key?: string, CaseSensitive?: boolean }
---@return any
---@return number? score
function Text.Closest(query, list, options) end

---@param words string[]
---@param options? { WholeWords?: boolean, Leetspeak?: boolean, Repeats?: boolean }
---@return QuarkWordFilter
function Text.WordFilter(words, options) end

---Named Jinja templates.
---@class QuarkTemplates
local Templates = {}

---@param name string
---@param context? table
---@return string?
---@return string? message
function Templates:Render(name, context) end

---@param source string
---@param context? table
---@return string?
---@return string? message
function Templates:RenderString(source, context) end

---@param name string
---@param source string
---@return true?
---@return string? message
function Templates:Add(name, source) end

---@param name string
function Templates:Remove(name) end

---@param name string
---@return boolean
function Templates:Has(name) end

---@return string[]
function Templates:GetNames() end

---@param name string
---@param callback function
function Templates:AddFilter(name, callback) end

---@param name string
---@param callback function
function Templates:AddFunction(name, callback) end

---@param name string
---@param callback function
function Templates:AddTest(name, callback) end

---@class QuarkTemplateModule
local Template = {}

---@param source string
---@param context? table
---@param options? { AutoEscape?: "None"|"Html"|"Json", Undefined?: "Lenient"|"Chainable"|"Strict", TrimBlocks?: boolean, LstripBlocks?: boolean, KeepTrailingNewline?: boolean, Globals?: table, Fuel?: integer, Filters?: table<string, function>, Functions?: table<string, function>, Tests?: table<string, function>, Templates?: table<string, string> }
---@return string?
---@return string? message
function Template.Render(source, context, options) end

---@param options? { AutoEscape?: "None"|"Html"|"Json", Undefined?: "Lenient"|"Chainable"|"Strict", TrimBlocks?: boolean, LstripBlocks?: boolean, KeepTrailingNewline?: boolean, Globals?: table, Fuel?: integer, Filters?: table<string, function>, Functions?: table<string, function>, Tests?: table<string, function>, Templates?: table<string, string> }
---@return QuarkTemplates?
---@return string? message
function Template.New(options) end
