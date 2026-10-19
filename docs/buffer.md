# Buffer

```lua
local Buffer = Quark.Import("Buffer")
```

A buffer is a mutable byte array for binary data: network packets, file formats, bulk numbers for native functions. Every Quark function that takes binary data (hashing, encryption, encoding, ...) accepts a buffer as well as a string.

Reading outside of the buffer raises an error, writing past the end grows the buffer and fills the gap with zeros. Buffers are limited to 256 MiB.

## Index base

Offsets are **zero based by default**, as in binary format specifications. A package that prefers Lua style one based offsets switches for itself, other packages are not affected:

```lua
Buffer.SetIndexBase(1)
```

| Base | First byte | Ranges `(start, finish)` | Like |
| --- | --- | --- | --- |
| `0` (default) | `0` | `finish` is exclusive | `DataView` / `slice` in JavaScript |
| `1` | `1` | `finish` is inclusive | `string.sub` |

Every buffer takes the base of the package creating it and keeps it, buffers from `Slice` and `Clone` take the base of their source. `buffer:SetIndexBase(base)` changes the base of a single buffer, for example one received from another package, and returns the buffer. `Buffer.GetIndexBase()` and `buffer:GetIndexBase()` return the current setting.

## Creating buffers

| Function | |
| --- | --- |
| `Buffer.New(size?, fill?)` | buffer of `size` bytes (default 0), filled with `fill` (default 0) |
| `Buffer.FromString(data)` | copy of a string or buffer |
| `Buffer.FromArray(type, values)` | numbers encoded one after another |
| `Buffer.Concat(parts)` | strings and buffers joined into a new buffer |
| `Buffer.IsBuffer(value)` | whether the value is a buffer |
| `Buffer.SetIndexBase(base)` / `Buffer.GetIndexBase()` | index base for buffers created by the calling package |

## Number types

`u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `u64`, `i64`, `f32`, `f64`, little endian by default. Append `be` for big endian or `le` to be explicit: `u32be`, `i16le`. Writing a value that does not fit into the type (for example 256 as `u8`, or 1.5 as an integer type) raises an error. `u64` values above the Lua integer range are read as floats.

## Methods

| Method | |
| --- | --- |
| `:Read(type, offset)` / `:Write(type, offset, value)` | single number |
| `:ReadArray(type, offset, count)` / `:WriteArray(type, offset, values)` | several numbers |
| `:Append(type, value)` | write a number at the end, returns the buffer for chaining |
| `:ReadString(offset, length?)` / `:WriteString(offset, data)` | raw bytes, `length` defaults to the rest of the buffer |
| `:AppendString(data)` | append raw bytes, returns the buffer for chaining |
| `:GetBytes(start?, finish?)` | contents as a Lua string, `finish` is exclusive |
| `:Slice(start?, finish?)` | copy of a range as a new buffer |
| `:CopyFrom(source, offset?, source_start?, source_finish?)` | copy from a string or buffer into this buffer |
| `:Fill(byte, start?, finish?)` | set a range to one byte value |
| `:Resize(size, fill?)` | grow or shrink |
| `:GetSize()` or `#buffer` | size in bytes |
| `:ToHex()` | lowercase hex |
| `:Equals(other)` or `==` | byte wise comparison |
| `:Clone()` | independent copy |
| `:SetIndexBase(base)` / `:GetIndexBase()` | index base of this buffer |

```lua
local packet = Buffer.New()
    :Append("u8", 1)
    :Append("u16be", player_id)
    :Append("f32", health)
    :AppendString(name)

local kind = packet:Read("u8", 0)
local id = packet:Read("u16be", 1)
```
