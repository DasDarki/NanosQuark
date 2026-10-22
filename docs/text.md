# Text

```lua
local Regex, Text, Template = Quark.Regex, Quark.Text, Quark.Template

local command = Regex.Match("^/(\\w+)\\s*(.*)$", message)
local player = Text.Closest(name, player_names)
local clean = Text.WordFilter(bad_words):Censor(message)
local motd = Template.Render("Welcome {{ name }}, {{ online }} players online", { name = name, online = count })
```

Three modules for working with text: `Regex` for regular expressions, `Text` for Unicode aware string helpers, fuzzy matching and word filters, and `Template` for text templates.

## Regex

Regular expressions with the syntax of the Rust [regex](https://docs.rs/regex/latest/regex/#syntax) crate, which is close to Perl and PCRE. Matching runs in linear time, so patterns and text from players can never hang the server. In exchange look-around (`(?=...)`, `(?<=...)`) and backreferences (`\1`) are not supported.

`Regex.New(pattern, options?)` compiles a pattern and returns a regex, or `nil, message` for invalid patterns. Keep compiled regexes around when they are used often.

| Option | |
| --- | --- |
| `CaseInsensitive` | ignore case, Unicode aware |
| `Multiline` | `^` and `$` match at line starts and ends |
| `DotAll` | `.` also matches line breaks |
| `Extended` | ignore whitespace and allow `#` comments in the pattern |
| `Ascii` | `\w`, `\d`, `\s` and case folding only cover ASCII instead of Unicode |

| Method | |
| --- | --- |
| `:IsMatch(text, start?)` | whether the pattern occurs |
| `:Find(text, start?)` | `start, end, text` of the first match like `string.find`, or `nil` |
| `:Match(text, start?)` | the first match, or `nil` |
| `:MatchAll(text, limit?)` | list of matches |
| `:Replace(text, replacement, limit?)` | new text and the number of replacements |
| `:Split(text, limit?)` | list of the parts between matches, at most `limit` parts |
| `:Count(text)` | number of matches |
| `:GetPattern()`, `:GetGroupCount()`, `:GetGroupNames()` | |

Positions are byte positions starting at 1 like in the Lua string library, negative `start` values count from the end. A match is a table:

| Field | |
| --- | --- |
| `Text` | the matched text |
| `Start`, `End` | position of the match, `End` is inclusive |
| `Groups` | captured groups by number, `false` for groups that did not take part |
| `Named` | captured named groups `(?P<name>...)` or `(?<name>...)` |

```lua
local date = Regex.New("(?<year>\\d{4})-(?<month>\\d{2})-(?<day>\\d{2})")
local found = date:Match("Released on 2026-10-25")
Console.Log(found.Named.year, found.Groups[2], found.Start)
```

The replacement is a string with `$1`, `${1}` or `${name}` for groups and `$$` for a dollar sign, a function that receives the match and returns the new text (`nil` keeps the match), or a table that maps matched texts to replacements.

```lua
Regex.Replace("(\\w+)@(\\w+)", "admin@server", "$2: $1")                  -- "server: admin"
Regex.Replace("\\d+", "a1b2", function(found) return found.Text * 2 end)  -- "a2b4"
```

The module also has `Regex.IsMatch`, `Find`, `Match`, `MatchAll`, `Replace` and `Split` taking the pattern as first argument. They cache the last compiled patterns and raise an error for invalid ones. `Regex.Escape(text)` escapes all special characters, `Regex.IsValid(pattern)` checks a pattern.

## Text

Lua string functions work on bytes and only know ASCII. These helpers understand Unicode: lengths count characters as people see them (grapheme clusters, so `"é"` and `"👨‍👩‍👧"` count as 1), and case conversion works for all languages.

| Function | |
| --- | --- |
| `Upper(text)`, `Lower(text)` | Unicode case conversion: `Upper("straße")` is `"STRASSE"` |
| `Capitalize(text)` | first letter of every word upper case, the rest lower case |
| `Length(text)` | number of grapheme clusters |
| `Sub(text, start, end?)` | part of the text by grapheme positions, negative positions count from the end |
| `Reverse(text)` | reversed by graphemes, keeps accents and emoji intact |
| `Truncate(text, length, suffix?)` | shortens to `length` graphemes including the suffix (`"..."`) |
| `Graphemes(text)`, `Words(text)` | list of grapheme clusters / words without punctuation |
| `Split(text, separator, options?)` | split by a plain separator. Options `Trim`, `RemoveEmpty`, `Limit`. An empty separator splits into graphemes |
| `Trim(text)` | removes Unicode whitespace at both ends |
| `Pad(text, width, options?)` | pads to `width` graphemes. Options `Align` (`Left`, `Right`, `Center`) and `Char` (`" "`) |
| `Wrap(text, width)` | list of lines of at most `width` graphemes, breaking at spaces and splitting longer words |
| `Normalize(text, form?)` | Unicode normalization `NFC` (default), `NFD`, `NFKC`, `NFKD` |
| `ToAscii(text)` | transliteration to ASCII: `"Crème Brûlée"` becomes `"Creme Brulee"` |
| `Slugify(text, separator?)` | URL and file name friendly: `"Hello, Wörld!"` becomes `"hello-world"` |
| `IsValid(text)` | whether the text is valid UTF-8 |

### Fuzzy matching

| Function | |
| --- | --- |
| `Levenshtein(a, b)`, `DamerauLevenshtein(a, b)` | number of edits between two texts |
| `Similarity(a, b, algorithm?)` | similarity from 0 to 1 with `JaroWinkler` (default), `Jaro`, `Levenshtein`, `DamerauLevenshtein` or `SorensenDice` |
| `FuzzyFind(query, list, options?)` | matching entries of the list, best first |
| `Closest(query, list, options?)` | the best matching entry and its score, or `nil` |

`FuzzyFind` returns `{ Item, Score, Index }` entries. Exact matches score 1, then texts starting with the query, then texts containing it, then similar texts, so typing the start of a player name is enough. It ignores case unless `CaseSensitive` is set.

| Option | |
| --- | --- |
| `Threshold` | minimum score from 0 to 1 (0.5) |
| `Limit` | maximum number of results |
| `Key` | field to compare when the list contains tables |
| `CaseSensitive` | `false` |

```lua
local commands = { { Name = "teleport" }, { Name = "kick" }, { Name = "ban" } }
local command = Text.Closest("telport", commands, { Key = "Name" })
if command then
    Chat.SendMessage(player, "Did you mean /" .. command.Name .. "?")
end
```

### Word filters

`Text.WordFilter(words, options?)` finds words from a list in text, for chat filters and name checks. Matching ignores case, accents (`"ä"` matches `"a"`) and common character substitutions (`"4"` for `"a"`, `"3"` for `"e"`, `"0"` for `"o"`, `"$"` for `"s"` and more), and treats repeated letters as one (`"baaad"` matches `"bad"`). Entries with spaces match across any whitespace or punctuation between the words.

| Option | |
| --- | --- |
| `WholeWords` | only match complete words, so `"ass"` does not match `"class"` (`true`) |
| `Leetspeak` | match character substitutions (`true`) |
| `Repeats` | treat repeated letters as one (`true`) |

| Method | |
| --- | --- |
| `:Contains(text)` | whether any word occurs |
| `:Find(text)` | list of `{ Word, Text, Start, End }`, `Word` is the list entry, `Text` how it was written |
| `:Censor(text, replacement?)` | text with every match replaced and the number of matches. A single character (`"*"`) replaces every character of the match, longer replacements the whole match |
| `:Add(words)`, `:Remove(words)`, `:GetWords()` | change the list |

```lua
local filter = Text.WordFilter({ "darn", "heck" })
local censored = filter:Censor("D4rn it, what the h3ck")  -- "**** it, what the ****"
```

## Template

Text templates with the [Jinja](https://jinja.palletsprojects.com/en/stable/templates/) syntax, powered by [MiniJinja](https://docs.rs/minijinja/latest/minijinja/syntax/index.html): `{{ value }}` outputs values, `{% if %}` and `{% for %}` control the output, filters like `{{ name | upper }}` transform values and templates can extend each other.

`Template.Render(source, context?, options?)` renders a template once and returns the text, or `nil, message` for syntax and render errors. The context is a table, functions and other values that cannot be represented are left out.

```lua
local text = Template.Render([[
Top players:
{% for player in players %}
{{ loop.index }}. {{ player.Name }} - {{ player.Kills }} kills
{% endfor %}
]], { players = top }, { TrimBlocks = true })
```

| Option | |
| --- | --- |
| `AutoEscape` | `None` (default), `Html` or `Json` escaping of output values, use `Html` for web pages |
| `Undefined` | how missing values behave: `Lenient` renders them empty (default), `Chainable` also allows `missing.field`, `Strict` makes them an error |
| `TrimBlocks`, `LstripBlocks`, `KeepTrailingNewline` | whitespace handling around blocks |
| `Globals` | values available to every template |
| `Fuel` | limit for the work one render may do, protects against runaway templates (10000000) |

`Template.New(options?)` keeps named templates for `{% extends %}`, `{% include %}` and `{% import %}`. It takes the same options and `Templates`, a table of names and sources:

```lua
local pages = Template.New({
    AutoEscape = "Html",
    Templates = {
        layout = "<html><body>{% block body %}{% endblock %}</body></html>",
        status = "{% extends 'layout' %}{% block body %}{{ online }} players online{% endblock %}",
    },
})

local html = pages:Render("status", { online = #Player.GetAll() })
```

| Method | |
| --- | --- |
| `:Render(name, context?)` | renders a named template |
| `:RenderString(source, context?)` | renders a source that can use the named templates |
| `:Add(name, source)` | adds or replaces a template, returns `true` or `nil, message` |
| `:Remove(name)`, `:Has(name)`, `:GetNames()` | |
