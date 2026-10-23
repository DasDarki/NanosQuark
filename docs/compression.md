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

### Streams

`Compression.Stream(algorithm, options?)` compresses or decompresses data piece by piece, for data that arrives in chunks or is too large to hold twice in memory, like network streams or long recordings.

| Option | |
| --- | --- |
| `Mode` | `Compress` (default) or `Decompress` |
| `Level` | compression level |
| `MaxSize` | decompression stops with an error beyond this many output bytes (256 MiB) |

| Method | |
| --- | --- |
| `:Write(data)` | adds input, returns the output produced so far (can be empty) |
| `:Flush()` | returns pending output without ending the stream, so the receiver can decode everything sent until now |
| `:Finish()` | ends the stream and returns the remaining output |
| `:IsFinished()`, `:GetTotalIn()`, `:GetTotalOut()` | |

```lua
local stream = Quark.Compression.Stream("zstd")
socket:Subscribe("Data", function(chunk)
    upload:Write(stream:Write(chunk))
end)
socket:Subscribe("Close", function()
    upload:Write(stream:Finish())
end)
```

Errors and exceeding `MaxSize` raise an error and end the stream. `lz4` streams are compressed piece by piece, but decompressed when `Finish` is called.

### Files

`Compression.CompressFile(algorithm, source, target, options?, callback?)` and `Compression.DecompressFile(algorithm, source, target, options?, callback?)` stream between two files on a worker thread without loading them into memory, so they also work for files larger than the available memory. They return `{ Input, Output }` with both sizes in bytes, or `nil, message`.

`CompressFile` takes the option `Level`, `DecompressFile` accepts `"auto"` as algorithm and the option `MaxSize`, unlimited by default. A target that would exceed `MaxSize` is deleted. Folders of the target are created when needed.

```lua
Quark.Compression.CompressFile("zstd", "Logs/game.log.1", "Archive/game-" .. os.date("%Y%m%d") .. ".log.zst", function(sizes, error)
    if sizes then
        os.remove("Logs/game.log.1")
    end
end)
```

## Zip

| Function | |
| --- | --- |
| `Zip.Create(entries, options?, callback?)` | archive as a string, options `Method` (`"deflate"` or `"store"`), `Level` (0 to 9), `Password` and `Encryption` |
| `Zip.Extract(archive, options?, callback?)` | list of entries with data, options `MaxSize` for the total extracted size and `Password` |
| `Zip.List(archive, options?, callback?)` | list of entries without data, works without the password |

Entries for `Create` are a list of `{ Name, Data, IsDirectory? }` (names ending in `/` are directories) or a map from name to data, which is written in sorted order:

```lua
local archive = Quark.Zip.Create({ ["config.toml"] = config, ["logs/today.txt"] = log })
```

Extracted entries have `Name`, `Data` (not for directories and not from `List`), `IsDirectory`, `IsEncrypted`, `Size`, `CompressedSize` and `SafePath`.

### Encryption

With `Password` every file is encrypted with AES (`Encryption` is `AES256` by default, or `AES128` / `AES192`), compatible with 7-Zip, WinZip and most other archive tools. Names and sizes stay readable, only the contents are encrypted. Extracting an encrypted archive without or with a wrong password returns `nil, message`. Archives with the old ZipCrypto encryption can be extracted too, but its password check is weak, so a wrong password can occasionally produce garbage instead of an error.

## Tar

`Tar.Create`, `Tar.Extract` and `Tar.List` work like their `Zip` counterparts. `Tar.Create` takes the option `Compression` (any codec, for example `"gzip"` for `.tar.gz`) and its `Level`. `Extract` and `List` detect gzip, zstd, lz4 and zlib compressed archives automatically. Only files and directories are read, links and special entries are skipped. Tar entries have no `CompressedSize`.

## Untrusted archives

Archives work in memory, `Zip` and `Tar` never write files. Entry names are returned as stored in the archive and can contain `../` or absolute paths. `SafePath` holds a normalized relative path, or `nil` when the name would escape the target directory. Use `SafePath` instead of `Name` when writing extracted files to disk.
