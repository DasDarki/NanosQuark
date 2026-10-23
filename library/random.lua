---@meta

---@alias QuarkSeed integer|string

---Seeded generator, the same seed gives the same sequence everywhere.
---@class QuarkRandomGenerator
local Generator = {}

---Integer from minimum to maximum, both inclusive. With one argument from 1 to it.
---@param minimum_or_maximum integer
---@param maximum? integer
---@return integer
function Generator:Int(minimum_or_maximum, maximum) end

---Number from 0 to 1, 0 to maximum or minimum to maximum, the upper end is excluded.
---@param minimum_or_maximum? number
---@param maximum? number
---@return number
function Generator:Float(minimum_or_maximum, maximum) end

---@param probability? number
---@return boolean
function Generator:Bool(probability) end

---@param list any[]
---@return any
---@return integer? index
function Generator:Pick(list) end

---Shuffles in place and returns the list.
---@param list any[]
---@return any[]
function Generator:Shuffle(list) end

---Distinct elements without replacement.
---@param list any[]
---@param count integer
---@return any[]
function Generator:Sample(list, count) end

---One item picked by weight.
---@param weights table
---@return any
function Generator:Weighted(weights) end

---@param mean? number
---@param deviation? number
---@return number
function Generator:Normal(mean, deviation) end

---@param rate? number
---@return number
function Generator:Exponential(rate) end

---@param mean number
---@return integer
function Generator:Poisson(mean) end

---@param minimum number
---@param maximum number
---@param mode? number
---@return number
function Generator:Triangular(minimum, maximum, mode) end

---@param radius? number
---@return number x
---@return number y
function Generator:InsideCircle(radius) end

---@param radius? number
---@return number x
---@return number y
---@return number z
function Generator:InsideSphere(radius) end

---@param radius? number
---@return number x
---@return number y
---@return number z
function Generator:OnSphere(radius) end

---Dice notation like "2d6+3" or "4d6kh3".
---@param notation string
---@return integer total
---@return integer[] rolls
function Generator:Roll(notation) end

---@return integer
function Generator:GetSeed() end

---@return string
function Generator:GetState() end

---@param state string
function Generator:SetState(state) end

---@return QuarkRandomGenerator
function Generator:Clone() end

---@class QuarkWeightedPicker
local WeightedPicker = {}

---@param generator? QuarkRandomGenerator
---@return any
function WeightedPicker:Pick(generator) end

---@param count integer
---@param generator? QuarkRandomGenerator
---@return any[]
function WeightedPicker:PickMany(count, generator) end

---@param item any
---@return number
function WeightedPicker:GetProbability(item) end

---@return { Item: any, Weight: number }[]
function WeightedPicker:GetItems() end

---@return integer
function WeightedPicker:GetCount() end

---@return number
function WeightedPicker:GetTotalWeight() end

---@class QuarkLootTable
local LootTable = {}

---@param options? { Generator?: QuarkRandomGenerator, Merge?: boolean, Pity?: table }
---@return { Item: any, Count: integer }[]
function LootTable:Roll(options) end

---@param item any
---@return number
function LootTable:GetChance(item) end

---Coherent noise for terrain, distributions and movement.
---@class QuarkNoise
local Noise = {}

---@param x number
---@param y number
---@param z? number
---@return number
function Noise:Get(x, y, z) end

---@param x number
---@param y number
---@param z? number
---@return number x
---@return number y
---@return number? z
function Noise:Warp(x, y, z) end

---Grid of little endian f32 values, row by row.
---@param width integer
---@param height integer
---@param options? { X?: number, Y?: number, Z?: number, Step?: number }
---@param callback? function
---@return QuarkBuffer
function Noise:Fill(width, height, options, callback) end

---@class QuarkRandomModule
local Random = {}

---Integer from minimum to maximum, both inclusive. With one argument from 1 to it.
---@param minimum_or_maximum integer
---@param maximum? integer
---@return integer
function Random.Int(minimum_or_maximum, maximum) end

---Number from 0 to 1, 0 to maximum or minimum to maximum, the upper end is excluded.
---@param minimum_or_maximum? number
---@param maximum? number
---@return number
function Random.Float(minimum_or_maximum, maximum) end

---@param probability? number
---@return boolean
function Random.Bool(probability) end

---@param list any[]
---@return any
---@return integer? index
function Random.Pick(list) end

---Shuffles in place and returns the list.
---@param list any[]
---@return any[]
function Random.Shuffle(list) end

---Distinct elements without replacement.
---@param list any[]
---@param count integer
---@return any[]
function Random.Sample(list, count) end

---One item picked by weight.
---@param weights table
---@return any
function Random.Weighted(weights) end

---@param mean? number
---@param deviation? number
---@return number
function Random.Normal(mean, deviation) end

---@param rate? number
---@return number
function Random.Exponential(rate) end

---@param mean number
---@return integer
function Random.Poisson(mean) end

---@param minimum number
---@param maximum number
---@param mode? number
---@return number
function Random.Triangular(minimum, maximum, mode) end

---@param radius? number
---@return number x
---@return number y
function Random.InsideCircle(radius) end

---@param radius? number
---@return number x
---@return number y
---@return number z
function Random.InsideSphere(radius) end

---@param radius? number
---@return number x
---@return number y
---@return number z
function Random.OnSphere(radius) end

---Dice notation like "2d6+3" or "4d6kh3".
---@param notation string
---@return integer total
---@return integer[] rolls
function Random.Roll(notation) end

---@param seed? QuarkSeed
---@return QuarkRandomGenerator
function Random.New(seed) end

---Reseeds the default generator of the package.
---@param seed? QuarkSeed
---@return integer
function Random.Seed(seed) end

---@return QuarkRandomGenerator
function Random.GetDefault() end

---@param text string
---@return integer
function Random.HashSeed(text) end

---@param value any
---@return boolean
function Random.IsGenerator(value) end

---@param weights table
---@return QuarkWeightedPicker
function Random.WeightedPicker(weights) end

---@param definition { Rolls?: integer|integer[], Entries?: table[], Guaranteed?: table[], Pity?: { Item: any, After: integer, Key?: any }[] }
---@return QuarkLootTable
function Random.LootTable(definition) end

---@param options? { Seed?: QuarkSeed, Type?: "OpenSimplex2"|"OpenSimplex2S"|"Perlin"|"Cellular"|"Value"|"ValueCubic", Frequency?: number, Fractal?: "None"|"FBm"|"Ridged"|"PingPong", Octaves?: integer, Lacunarity?: number, Gain?: number, WeightedStrength?: number, PingPongStrength?: number, CellularDistance?: string, CellularReturn?: string, CellularJitter?: number, Warp?: boolean|table }
---@return QuarkNoise
function Random.Noise(options) end

---@param width number
---@param height number
---@param radius number
---@param options? { X?: number, Y?: number, Filter?: (fun(x: number, y: number): boolean), Limit?: integer, Attempts?: integer, Generator?: QuarkRandomGenerator }
---@return { X: number, Y: number }[]
function Random.PoissonDisk(width, height, radius, options) end
