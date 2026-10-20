# Formats

Every format is its own module, imported on first use:

```lua
local config = Quark.TOML.Decode(text)
local payload = Quark.JSON.Encode({ id = Quark.UUID.V7(), score = 10 })
```

## Data formats

`JSON`, `TOML`, `YAML`, `MessagePack` and `CBOR` share one API:

| Function | |
| --- | --- |
| `Format.Encode(value, options?, callback?)` | text (JSON, TOML, YAML) or binary string (MessagePack, CBOR) |
| `Format.Decode(input, options?, callback?)` | Lua value, or `nil, message` for invalid input. `input` can be a string or a `Buffer` |

Both run synchronously, or on a worker thread when a callback is given (or with `Quark.Await`). Converting between Lua tables and the format always happens on the game thread, parsing and printing happen on the worker, which keeps large documents off the game thread.

| Option | Applies to | |
| --- | --- | --- |
| `Pretty` | JSON, TOML | human readable output, JSON indented by 2 spaces |
| `Indent` | JSON | pretty output with this many spaces |
| `EmptyTablesAsArrays` | encoding | encode `{}` as an empty array instead of an empty object |
| `PreserveNull` | decoding | decode `null` as `Format.Null` instead of `nil`, keeps keys and array positions |

### Mapping to Lua

| Format | Lua |
| --- | --- |
| object / map | table, keys are always sorted when encoding |
| array | sequence table, marked so that it encodes as an array again even when empty |
| string, number, boolean | string, integer or float, boolean |
| `null` | `nil`, or `Format.Null` with `PreserveNull` |
| binary (MessagePack, CBOR) | string |
| date and time (TOML) | string in RFC 3339 format |

A table is encoded as an array when it is a sequence or was marked with `Format.Array(table?)`, otherwise as an object. Tables mixing both, functions, userdata and recursive tables cannot be encoded. JSON and TOML objects need string keys, MessagePack and CBOR maps accept any keys.

`Null`, `Array` and `IsNull` exist in `JSON`, `YAML`, `MessagePack` and `CBOR`, the values are the same in all of them.

TOML documents have to be tables. MessagePack decoding follows `rmpv`, which reads the reserved byte `0xC1` as `nil`. Trailing bytes after a MessagePack or CBOR value are rejected.

## CSV

| Function | Options |
| --- | --- |
| `CSV.Decode(input, options?, callback?)` | `Header` (default `true`): rows become tables keyed by column name, otherwise lists. `Delimiter` (`,`), `Trim` (`false`) |
| `CSV.Encode(rows, options?, callback?)` | `Columns` (column order, default: sorted keys of all rows), `Header` (default `true` for keyed rows), `Delimiter` |

Fields are decoded as strings. When encoding, rows can be lists or keyed tables, values have to be strings, numbers or booleans, `nil` becomes an empty field. Quoting follows RFC 4180.

```lua
local rows = Quark.CSV.Decode("name,score\nalice,10\n")
-- { { name = "alice", score = "10" } }
```

## UUID

| Function | |
| --- | --- |
| `UUID.V4()` | random UUID |
| `UUID.V7()` | time ordered UUID, later calls always sort after earlier ones, ideal as database key |
| `UUID.Parse(text)` | canonical lowercase form, accepts braces, URNs and the form without hyphens, or `nil, message` |
| `UUID.IsValid(value)` | whether the value is a valid UUID string |
| `UUID.GetVersion(text)` | version number |
| `UUID.GetTimestamp(text)` | creation time in Unix milliseconds for time based versions (1, 6, 7), or `nil, message` |
| `UUID.ToBytes(text)` / `UUID.FromBytes(bytes)` | 16 byte binary form |
| `UUID.Nil`, `UUID.Max` | all zeros and all ones |

## URL

| Function | |
| --- | --- |
| `URL.Encode(text, form?)` | percent encode a component, keeps `A-Z a-z 0-9 - . _ ~`. With `form = true` spaces become `+` |
| `URL.Decode(text, form?)` | percent decode, with `form = true` `+` becomes a space |
| `URL.EncodeQuery(parameters)` | `a=1&b=2` with sorted keys, lists become repeated keys |
| `URL.DecodeQuery(query)` | table of parameters, repeated keys become lists, a leading `?` is ignored |
| `URL.Parse(url)` | `{ Scheme, Username, Password, Host, Port, Path, Query, Fragment, Origin }` (the port falls back to the scheme default), or `nil, message` |
| `URL.Join(base, relative)` | resolved URL, or `nil, message` |

```lua
local url = "https://api.example.com/search?" .. Quark.URL.EncodeQuery({ q = "hello world", page = 2 })
-- https://api.example.com/search?page=2&q=hello+world
```
