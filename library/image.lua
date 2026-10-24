---@meta

---@alias QuarkColor string|integer[]|{ R: number, G: number, B: number, A: number? }|Color
---@alias QuarkImageFormat "png"|"jpeg"|"webp"|"bmp"

---nanos world color with channels from 0 to 1.
---@class Color
---@field R number
---@field G number
---@field B number
---@field A number

---@class QuarkImage
local Image = {}

---@return integer width
---@return integer height
function Image:GetSize() end

---@param x integer
---@param y integer
---@return integer r
---@return integer g
---@return integer b
---@return integer a
function Image:GetPixel(x, y) end

---@param x integer
---@param y integer
---@param color QuarkColor
function Image:SetPixel(x, y, color) end

---@param color QuarkColor
function Image:Fill(color) end

---@param x integer
---@param y integer
---@param width integer
---@param height integer
---@param color QuarkColor
function Image:FillRect(x, y, width, height, color) end

---@param x1 integer
---@param y1 integer
---@param x2 integer
---@param y2 integer
---@param color QuarkColor
---@param thickness? integer
function Image:DrawLine(x1, y1, x2, y2, color, thickness) end

---@param x integer
---@param y integer
---@param radius integer
---@param color QuarkColor
---@param filled? boolean
function Image:DrawCircle(x, y, radius, color, filled) end

---@param image QuarkImage
---@param x integer
---@param y integer
---@param options? { Opacity?: number }
function Image:DrawImage(image, x, y, options) end

---@param width integer
---@param height integer
---@param filter? "Nearest"|"Linear"|"Cubic"|"Lanczos"
---@return QuarkImage
function Image:Resize(width, height, filter) end

---@param x integer
---@param y integer
---@param width integer
---@param height integer
---@return QuarkImage
function Image:Crop(x, y, width, height) end

---@param degrees 90|180|270
---@return QuarkImage
function Image:Rotate(degrees) end

---@param direction? "Horizontal"|"Vertical"
---@return QuarkImage
function Image:Flip(direction) end

---@param sigma number
---@return QuarkImage
function Image:Blur(sigma) end

---@return QuarkImage
function Image:Grayscale() end

---@return QuarkImage
function Image:Clone() end

---@return string
function Image:GetBytes() end

---@param format? QuarkImageFormat
---@param options? { Quality?: integer }
---@param callback? function
---@return string?
---@return string? message
function Image:Encode(format, options, callback) end

---@class QuarkImageModule
local ImageModule = {}

---@param width integer
---@param height integer
---@param color? QuarkColor
---@return QuarkImage
function ImageModule.New(width, height, color) end

---@param data QuarkBinary
---@param callback? function
---@return QuarkImage?
---@return string? message
function ImageModule.Decode(data, callback) end

---@param width integer
---@param height integer
---@param values number[]|QuarkBufferView
---@param options? { Min?: number, Max?: number, Colors?: { [1]: number, [2]: QuarkColor }[] }
---@return QuarkImage
function ImageModule.FromHeights(width, height, values, options) end

---@param width integer
---@param height integer
---@param data string|QuarkBuffer|number[]|QuarkBufferView
---@param palette table<integer, QuarkColor>
---@return QuarkImage
function ImageModule.FromIndexed(width, height, data, palette) end

---@param value any
---@return boolean
function ImageModule.IsImage(value) end
