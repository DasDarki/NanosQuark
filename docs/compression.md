# Compression

## Codecs

```lua
local Compression = Quark.Import("Compression")

local packed = Compression.Compress("zstd", data)
local unpacked = Compression.Decompress("zstd", packed)
```

| Function | |
| --- | --- |
| `Compression.Compress(algorithm, data, options?, callback?)` | compressed bytes, option `Level` |
| `Compression.Decompress(algorithm, data, options?, callback?)` | original bytes or `nil, message`, option `MaxSize`. `algorithm` can be `"auto"` to detect the format |
| `Compression.Detect(data)` | `"gzip"`, `"zstd"`, `"lz4"` or `"zlib"` from the magic bytes, or `nil` |
| `Compression.Algorithms()` | list of supported algorithms |

| Algorithm | Levels (default) | Notes |
| --- | --- | --- |
| `lz4` | none | fastest, frame format compatible with the `lz4` tool |
| `zstd` | 1 to 22 (3) | best balance of speed and size |
| `gzip` | 0 to 9 (6) | `.gz` files, HTTP `Content-Encoding: gzip` |
| `deflate` | 0 to 9 (6) | raw deflate stream without header |
| `zlib` | 0 to 9 (6) | deflate with zlib header |
| `brotli` | 0 to 11 (6) | small output for text, slow at high levels |

Data can be a string or a `Buffer`, results are strings. With a callback (or `Quark.Await`) the work runs on a worker thread, which is recommended for large data and high levels. Raw `deflate` and `brotli` streams cannot be detected, pass the algorithm explicitly.

### Decompression limits

Decompression stops with `nil, message` once the output grows beyond `MaxSize` bytes (default and maximum 256 MiB). This protects the server against small inputs that expand to gigabytes. Lower it when decompressing data received from players.

## Zip

| Function | |
| --- | --- |
| `Zip.Create(entries, options?, callback?)` | archive as a string, options `Method` (`"deflate"` or `"store"`) and `Level` (0 to 9) |
| `Zip.Extract(archive, options?, callback?)` | list of entries with data, option `MaxSize` for the total extracted size |
| `Zip.List(archive, options?, callback?)` | list of entries without data |

Entries for `Create` are a list of `{ Name, Data, IsDirectory? }` (names ending in `/` are directories) or a map from name to data, which is written in sorted order:

```lua
local archive = Quark.Zip.Create({ ["config.toml"] = config, ["logs/today.txt"] = log })
```

Extracted entries have `Name`, `Data` (not for directories and not from `List`), `IsDirectory`, `Size`, `CompressedSize` and `SafePath`.

## Tar

`Tar.Create`, `Tar.Extract` and `Tar.List` work like their `Zip` counterparts. `Tar.Create` takes the option `Compression` (any codec, for example `"gzip"` for `.tar.gz`) and its `Level`. `Extract` and `List` detect gzip, zstd, lz4 and zlib compressed archives automatically. Only files and directories are read, links and special entries are skipped. Tar entries have no `CompressedSize`.

## Untrusted archives

Archives work in memory, Quark never writes files. Entry names are returned as stored in the archive and can contain `../` or absolute paths. `SafePath` holds a normalized relative path, or `nil` when the name would escape the target directory. Use `SafePath` instead of `Name` when writing extracted files to disk.
