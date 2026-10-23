---@meta

---@alias QuarkTimeUnit "Years"|"Months"|"Weeks"|"Days"|"Hours"|"Minutes"|"Seconds"|"Milliseconds"
---@alias QuarkCalendarUnit "Year"|"Month"|"Week"|"Day"|"Hour"|"Minute"|"Second"
---@alias QuarkDurationValue number|string|{ Years?: integer, Months?: integer, Weeks?: integer, Days?: integer, Hours?: integer, Minutes?: integer, Seconds?: integer, Milliseconds?: integer }

---Immutable date with a time zone.
---@class QuarkDateTime
---@field Year integer
---@field Month integer
---@field Day integer
---@field Hour integer
---@field Minute integer
---@field Second integer
---@field Millisecond integer
---@field Weekday integer 1 is Monday, 7 is Sunday
---@field YearDay integer
---@field Zone string
---@field Offset integer Seconds from UTC
local DateTime = {}

---@return number
function DateTime:GetUnix() end

---@return integer
function DateTime:GetUnixMs() end

---@return string
function DateTime:ToISO() end

---@return string
function DateTime:ToRFC2822() end

---@param format string
---@param options? { Locale?: string }
---@return string
function DateTime:Format(format, options) end

---@return table
function DateTime:ToTable() end

---@param zone string
---@return QuarkDateTime
function DateTime:InZone(zone) end

---@return QuarkDateTime
function DateTime:ToUTC() end

---@param duration QuarkDurationValue
---@return QuarkDateTime
function DateTime:Add(duration) end

---@param duration QuarkDurationValue
---@return QuarkDateTime
function DateTime:Subtract(duration) end

---@param fields { Year?: integer, Month?: integer, Day?: integer, Hour?: integer, Minute?: integer, Second?: integer, Millisecond?: integer }
---@return QuarkDateTime
function DateTime:With(fields) end

---@param unit QuarkCalendarUnit
---@return QuarkDateTime
function DateTime:StartOf(unit) end

---@param unit QuarkCalendarUnit
---@return QuarkDateTime
function DateTime:EndOf(unit) end

---Milliseconds, or a number in the unit.
---@param other QuarkDateTime
---@param unit? QuarkTimeUnit
---@return number
function DateTime:Since(other, unit) end

---@param now? QuarkDateTime
---@return string
function DateTime:Relative(now) end

---@param other QuarkDateTime
---@return boolean
function DateTime:IsBefore(other) end

---@param other QuarkDateTime
---@return boolean
function DateTime:IsAfter(other) end

---@class QuarkCron
local Cron = {}

---@param after? QuarkDateTime
---@return QuarkDateTime?
function Cron:Next(after) end

---@param count integer
---@param after? QuarkDateTime
---@return QuarkDateTime[]
function Cron:NextMany(count, after) end

---@param date QuarkDateTime
---@return boolean
function Cron:Matches(date) end

---@return string
function Cron:GetExpression() end

---@return string
function Cron:GetZone() end

---@class QuarkSchedule
local Schedule = {}

function Schedule:Stop() end

---@return boolean
function Schedule:IsActive() end

---@return QuarkDateTime?
function Schedule:GetNext() end

---@return string
function Schedule:GetExpression() end

---@class QuarkTimeModule
local Time = {}

---@param zone? string
---@return QuarkDateTime
function Time.Now(zone) end

---@param fields { Year: integer, Month?: integer, Day?: integer, Hour?: integer, Minute?: integer, Second?: integer, Millisecond?: integer }
---@param zone? string
---@return QuarkDateTime
function Time.Date(fields, zone) end

---@param seconds number
---@param zone? string
---@return QuarkDateTime
function Time.FromUnix(seconds, zone) end

---@param milliseconds integer
---@param zone? string
---@return QuarkDateTime
function Time.FromUnixMs(milliseconds, zone) end

---@param text string
---@param options? { Zone?: string, Format?: string }
---@return QuarkDateTime?
---@return string? message
function Time.Parse(text, options) end

---@return number
function Time.Unix() end

---@return integer
function Time.UnixMs() end

---@param value any
---@return boolean
function Time.IsDateTime(value) end

---@param name string
---@return boolean
function Time.IsValidZone(name) end

---@return string[]
function Time.GetZones() end

---@return string
function Time.GetSystemZone() end

---@param text string
---@return integer?
---@return string? message
function Time.ParseDuration(text) end

---@param duration QuarkDuration
---@param options? { Style?: "Short"|"Long"|"Clock", Largest?: integer }
---@return string
function Time.FormatDuration(duration, options) end

---@param expression string
---@param options? { Zone?: string }
---@return QuarkCron?
---@return string? message
function Time.Cron(expression, options) end

---@param expression string
---@param callback fun(time: QuarkDateTime)
---@param options? { Zone?: string }
---@return QuarkSchedule?
---@return string? message
function Time.Schedule(expression, callback, options) end
