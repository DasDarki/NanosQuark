---@meta

---Generated tile map.
---@class QuarkMap
local Map = {}

---@return integer width
---@return integer height
function Map:GetSize() end

---@param x integer
---@param y integer
---@return integer
function Map:GetTile(x, y) end

---@param x integer
---@param y integer
---@param tile integer
function Map:SetTile(x, y, tile) end

---@param x integer
---@param y integer
---@return boolean
function Map:IsWalkable(x, y) end

---@return { X: integer, Y: integer, Width: integer, Height: integer, Center: { X: integer, Y: integer } }[]
function Map:GetRooms() end

---@return { X: integer, Y: integer }[]
function Map:GetFloorCells() end

---@return QuarkBuffer
function Map:GetTiles() end

---@return integer[]
function Map:ToCosts() end

---@return string
function Map:ToText() end

---@class QuarkProcGenModule
---@field Tile { Wall: integer, Floor: integer, Corridor: integer, Door: integer }
local ProcGen = {}

---@param width integer
---@param height integer
---@param options? { FillChance?: number, Steps?: integer, BirthLimit?: integer, DeathLimit?: integer, KeepLargest?: boolean, Generator?: QuarkRandomGenerator }
---@return QuarkMap
function ProcGen.Caves(width, height, options) end

---@param width integer
---@param height integer
---@param options? { Attempts?: integer, MinRoomSize?: integer, MaxRoomSize?: integer, Loops?: number, Doors?: boolean, Generator?: QuarkRandomGenerator }
---@return QuarkMap
function ProcGen.Dungeon(width, height, options) end

---@param width integer
---@param height integer
---@param options? table
---@param callback? function
---@return QuarkBuffer
function ProcGen.Heightmap(width, height, options, callback) end
