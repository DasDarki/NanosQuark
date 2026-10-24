# Image

```lua
local Image = Quark.Image

local minimap = Image.FromHeights(256, 256, heights:View("f32"), {
    Colors = { { 0, "#1f4e8c" }, { 0.35, "#e0d090" }, { 0.5, "#3c8c3c" }, { 0.8, "#806040" }, { 1, "#ffffff" } },
})
for _, character in ipairs(Character.GetAll()) do
    local location = character:GetLocation()
    minimap:DrawCircle(math.floor(location.X / 100), math.floor(location.Y / 100), 3, "#ff0000")
end

server:Get("/minimap.webp", function(_, response)
    response:Send(minimap:Encode("webp"), "image/webp")
end)
```

Images in memory: decode and encode PNG, JPEG, WebP and BMP, draw, transform and build images from height maps or tile maps, for example minimaps on a web page, generated thumbnails or screenshots received from clients.

Pixels are addressed with `x` and `y` starting at the [index base](buffer.md#index-base) of the package (0 by default), the origin is the top left corner. Images are limited to 16384 pixels per side and 64 million pixels.

## Colors

Colors are hex texts (`"#rgb"`, `"#rgba"`, `"#rrggbb"`, `"#rrggbbaa"`), lists of 0 to 255 values (`{ 255, 128, 0, 255 }`, alpha optional) or tables and nanos world `Color` values with `R`, `G`, `B` and `A` from 0 to 1. Pixels are read as `r, g, b, a` from 0 to 255.

## Creating images

| Function | |
| --- | --- |
| `Image.New(width, height, color?)` | image filled with a color, transparent by default |
| `Image.Decode(data, callback?)` | image from PNG, JPEG, WebP or BMP data, or `nil, message`. The format is detected |
| `Image.FromHeights(width, height, values, options?)` | colors a height map, a list or [buffer view](buffer.md#views) of `width * height` numbers row by row |
| `Image.FromIndexed(width, height, data, palette)` | one color per value, for tile maps. `data` is a string, buffer, list or buffer view, `palette` maps values to colors |
| `Image.IsImage(value)` | |

`FromHeights` options are `Min` and `Max` (the smallest and largest value by default) and `Colors`, a list of `{ position, color }` from 0 to 1 that the heights are interpolated between (black to white by default).

```lua
local map = Quark.ProcGen.Dungeon(64, 64)
local Tile = Quark.ProcGen.Tile
local picture = Image.FromIndexed(64, 64, map:GetTiles(), {
    [Tile.Wall] = "#202020", [Tile.Floor] = "#c8c8c8", [Tile.Corridor] = "#909090", [Tile.Door] = "#a05000",
}):Resize(512, 512, "Nearest")
```

## Drawing

| Method | |
| --- | --- |
| `:GetPixel(x, y)`, `:SetPixel(x, y, color)` | |
| `:Fill(color)` | replace every pixel |
| `:FillRect(x, y, width, height, color)` | rectangle, blended with transparent colors |
| `:DrawLine(x1, y1, x2, y2, color, thickness?)` | |
| `:DrawCircle(x, y, radius, color, filled?)` | filled (`true`) or outline |
| `:DrawImage(image, x, y, options?)` | draws another image on top, with transparency and the option `Opacity` (1) |

Drawing outside of the image is clipped.

## Transforming

These return a new image and keep the original:

| Method | |
| --- | --- |
| `:Resize(width, height, filter?)` | filter `Nearest` (sharp pixels), `Linear` (default), `Cubic` or `Lanczos` (best quality, slowest) |
| `:Crop(x, y, width, height)` | part of the image, cut at the borders |
| `:Rotate(degrees)` | 90, 180 or 270 degrees clockwise |
| `:Flip(direction?)` | `Horizontal` (default) or `Vertical` |
| `:Blur(sigma)` | Gaussian blur |
| `:Grayscale()`, `:Clone()` | |

## Encoding

`:Encode(format?, options?, callback?)` returns the image as `png` (default), `jpeg`, `webp` or `bmp`. JPEG takes the option `Quality` from 1 to 100 (85) and drops transparency. WebP is encoded lossless, which suits generated images like minimaps: they are usually smaller than PNG and keep every pixel exact. With a callback encoding runs on a worker thread, which is recommended for large images. `:GetSize()` returns width and height, `:GetBytes()` the raw RGBA pixels.

| Format | Content type |
| --- | --- |
| `png` | `image/png` |
| `jpeg` | `image/jpeg` |
| `webp` | `image/webp` |
| `bmp` | `image/bmp` |
