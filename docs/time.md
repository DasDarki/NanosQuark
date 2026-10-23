# Time

```lua
local Time = Quark.Time

local now = Time.Now("Europe/Berlin")
Console.Log(now:Format("%d.%m.%Y %H:%M"))

local expires = now:Add("7d")
local ban = Time.ParseDuration("2h 30m")

Time.Schedule("0 4 * * *", function()
    Chat.BroadcastMessage("Daily restart in 5 minutes")
end, { Zone = "Europe/Berlin" })
```

Dates with time zones, durations and cron schedules. Time zones come from the system database, with a built-in copy of the IANA database as fallback, so they work the same on Linux and Windows servers.

## Dates

Dates are immutable objects with a time zone. Functions that create dates take an optional zone name like `"Europe/Berlin"`, `"UTC"` or `"Local"`. Without one the system time zone of the server is used.

| Function | |
| --- | --- |
| `Time.Now(zone?)` | the current date |
| `Time.Date(fields, zone?)` | date from `{ Year, Month, Day, Hour, Minute, Second, Millisecond }`, only `Year` is required |
| `Time.FromUnix(seconds, zone?)`, `Time.FromUnixMs(milliseconds, zone?)` | date from a Unix timestamp |
| `Time.Parse(text, options?)` | date from text, or `nil, message` |
| `Time.Unix()`, `Time.UnixMs()` | current Unix time in seconds (with fractions) and milliseconds |
| `Time.IsDateTime(value)` | whether a value is a date |

`Time.Parse` understands ISO 8601 and RFC 3339 (`"2026-06-01T10:00:00+02:00"`, `"2026-06-01T08:00:00Z"`, `"2026-06-01 10:00"`, `"2026-06-01"`), dates with a zone annotation (`"2026-06-01T10:00:00+02:00[Europe/Berlin]"`) and RFC 2822 (`"Mon, 1 Jun 2026 10:00:00 +0000"`). Dates with an offset keep it.

| Option | |
| --- | --- |
| `Zone` | zone for texts without offset, and zone the result is converted to |
| `Format` | parse with a [strftime format](https://docs.rs/jiff/latest/jiff/fmt/strtime/index.html) like `"%d.%m.%Y %H:%M"` |

### Fields and formatting

Dates have the fields `Year`, `Month`, `Day`, `Hour`, `Minute`, `Second`, `Millisecond`, `Weekday` (1 is Monday, 7 is Sunday), `YearDay`, `Zone` (the zone name) and `Offset` (seconds from UTC). `:ToTable()` returns all of them as a table.

| Method | |
| --- | --- |
| `:Format(format, options?)` | text with [strftime directives](https://docs.rs/jiff/latest/jiff/fmt/strtime/index.html#conversion-specifications): `%Y-%m-%d %H:%M:%S`, `%A, %B %-d`. Option `Locale` for month and weekday names in other languages |
| `:ToISO()` | `"2026-06-01T10:00:00+02:00"` |
| `:ToRFC2822()` | `"Mon, 1 Jun 2026 10:00:00 +0200"`, for HTTP and e-mails |
| `tostring(date)` | ISO with the zone: `"2026-06-01T10:00:00+02:00[Europe/Berlin]"` |
| `:GetUnix()`, `:GetUnixMs()` | Unix timestamp in seconds and milliseconds |
| `:Relative(now?)` | `"in 5 minutes"`, `"3 hours ago"`, `"just now"` |

With `Locale` the names of `%A`, `%a`, `%B` and `%b` are written in that language: `de`, `en`, `es`, `fr`, `it`, `nl`, `pl`, `pt`, `ru`, `sv`, `tr` or `cs`, regional tags like `de-AT` use their language. Only the names change, the order of the parts is up to the format. Month names are in their standalone form, which reads naturally in headings and lists but not always after a day in languages with grammatical cases like Russian or Polish.

```lua
Time.Now("Europe/Berlin"):Format("%A, %-d. %B %Y", { Locale = "de" })  -- "Montag, 1. Juni 2026"
```

### Calculating

| Method | |
| --- | --- |
| `:Add(duration)`, `:Subtract(duration)` | new date. The duration is milliseconds, a text like `"1h 30m"` or a table `{ Years, Months, Weeks, Days, Hours, Minutes, Seconds, Milliseconds }` |
| `:Since(other, unit?)` | time since another date, in milliseconds or as number in `Years`, `Months`, `Weeks`, `Days`, `Hours`, `Minutes` or `Seconds` |
| `:With(fields)` | new date with some fields changed |
| `:StartOf(unit)`, `:EndOf(unit)` | first and last millisecond of the `Year`, `Month`, `Week` (starting Monday), `Day`, `Hour`, `Minute` or `Second` |
| `:InZone(zone)`, `:ToUTC()` | the same moment in another time zone |
| `:IsBefore(other)`, `:IsAfter(other)`, `==`, `<`, `<=` | compare moments, independent of the zone |

Days, weeks, months and years follow the calendar of the date's zone: adding one day keeps the time of day across daylight saving changes, while adding 24 hours does not, and adding one month to January 31 gives the last day of February.

```lua
local season_end = Time.Now("UTC"):EndOf("Month")
local days_left = math.floor(season_end:Since(Time.Now("UTC"), "Days"))
```

### Time zones

`Time.IsValidZone(name)` checks a zone name, `Time.GetZones()` lists all known zones and `Time.GetSystemZone()` returns the zone of the server.

## Durations

`Time.ParseDuration(text)` converts a duration to milliseconds, or returns `nil, message`. It accepts `"90s"`, `"1h 30m"`, `"2 days 3 hours"`, `"1.5h"` and ISO 8601 durations like `"PT1H30M"`. Days are 24 hours and weeks 7 days, months and years are rejected because their length varies.

`Time.FormatDuration(duration, options?)` turns milliseconds (or a duration text) into readable text:

| Option | |
| --- | --- |
| `Style` | `Short` gives `"1h 30m"` (default), `Long` gives `"1 hour, 30 minutes"`, `Clock` gives `"1:30:00"` |
| `Largest` | maximum number of units, `{ Largest = 2 }` turns `1d 2h 3m` into `1d 2h` |

```lua
local duration = Time.ParseDuration(arguments[2])
if not duration then
    return Chat.SendMessage(player, "Usage: /ban <player> <duration>, like 2h or 7d")
end
Chat.BroadcastMessage(target:GetName() .. " was banned for " .. Time.FormatDuration(duration, { Style = "Long" }))
```

## Cron

Cron expressions describe repeating times. They have five fields, or six with seconds first:

```
┌───────────── second (0-59, optional)
│ ┌─────────── minute (0-59)
│ │ ┌───────── hour (0-23)
│ │ │ ┌─────── day of the month (1-31, or L for the last day)
│ │ │ │ ┌───── month (1-12 or JAN-DEC)
│ │ │ │ │ ┌─── day of the week (0-7 or SUN-SAT, 0 and 7 are Sunday)
* * * * * *
```

Fields accept `*`, lists (`1,15`), ranges (`MON-FRI`) and steps (`*/15`, `9-17/2`). When both day fields are set, a day matches if either does, like in classic cron. The macros `@yearly`, `@monthly`, `@weekly`, `@daily` and `@hourly` are shortcuts.

| Expression | |
| --- | --- |
| `0 4 * * *` | every day at 04:00 |
| `*/15 * * * *` | every 15 minutes |
| `0 18 * * FRI` | Fridays at 18:00 |
| `0 0 L * *` | midnight on the last day of every month |
| `*/10 * * * * *` | every 10 seconds |

`Time.Cron(expression, options?)` parses an expression, or returns `nil, message`. The option `Zone` sets the time zone the expression is evaluated in (system zone).

| Method | |
| --- | --- |
| `:Next(after?)` | the next matching date after a date (now), `nil` if there is none in the next 8 years |
| `:NextMany(count, after?)` | list of the next dates |
| `:Matches(date)` | whether a date matches |
| `:GetExpression()`, `:GetZone()` | |

Times that do not exist because the clock jumps forward for daylight saving run once right after the jump, times that exist twice run once.

### Schedules

`Time.Schedule(expression, callback, options?)` calls the callback with the scheduled date at every matching time, or returns `nil, message` for invalid expressions. It takes the same `Zone` option.

| Method | |
| --- | --- |
| `:Stop()` | stops the schedule |
| `:IsActive()` | whether it still runs |
| `:GetNext()` | the next time it runs, `nil` when stopped |
| `:GetExpression()` | |

Schedules keep running when the returned object is not kept, and stop when the package unloads. They follow the wall clock, so changing the system time or a daylight saving change does not shift them.

```lua
local restart = Time.Schedule("55 3 * * *", function(time)
    Chat.BroadcastMessage("The server restarts in 5 minutes")
end, { Zone = "Europe/Berlin" })

Console.Log("Next restart warning: " .. restart:GetNext():Format("%H:%M"))
```
