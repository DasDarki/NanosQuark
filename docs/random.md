# Random

```lua
local Random = Quark.Random

local world = Random.New("map-seed")
local spawn_x, spawn_y = world:InsideCircle(5000)

local rarity = Random.WeightedPicker({ Common = 70, Rare = 25, Epic = 5 })
Console.Log(rarity:Pick())

local damage = Random.Roll("2d6+3")
```

Random numbers for gameplay: seeded generators that produce the same results on every platform, distributions, weighted picking, loot tables, dice and noise. None of this is meant for secrets, use `Crypto.RandomBytes`, `Crypto.RandomInt` and `Crypto.RandomToken` for tokens, passwords and keys ([docs/crypto.md](crypto.md)).

## Generators

`Random.New(seed?)` creates a generator. The seed is an integer or a string, a random seed is used without one. The same seed always produces the same sequence, on Linux and Windows and across Quark versions, so a seed is enough to recreate a map, a dungeon or a replay.

Generators use Xoshiro256++, strings are turned into seeds with FNV-1a (`Random.HashSeed(text)`).

| Method | |
| --- | --- |
| `:GetSeed()` | the seed as integer |
| `:GetState()` / `:SetState(state)` | the current position in the sequence as 64 hexadecimal characters, to save and continue later |
| `:Clone()` | an independent copy at the same position |

Every function below is a method of generators and a module function. Module functions use the default generator of the calling package, which is seeded randomly. `Random.Seed(seed?)` reseeds it and returns the seed, `Random.GetDefault()` returns it as generator. Each package has its own default generator, so seeding it does not affect other packages.

```lua
local generator = Random.New(1234)
generator:Int(1, 6)      -- same value on every run
Random.Int(1, 6)         -- default generator of this package
```

`Random.IsGenerator(value)` checks whether a value is a generator.

## Numbers

| Function | |
| --- | --- |
| `Int(min, max)` / `Int(max)` | integer from `min` to `max` (both inclusive), `Int(max)` starts at 1 |
| `Float()`, `Float(max)`, `Float(min, max)` | number from 0 to 1, 0 to `max` or `min` to `max`, the upper end is excluded |
| `Bool(probability?)` | `true` with the given probability (0.5) |
| `Normal(mean?, deviation?)` | normal distribution (0, 1), for values that cluster around an average |
| `Exponential(rate?)` | time between random events that happen `rate` times per unit on average (1) |
| `Poisson(mean)` | integer count of events in an interval with `mean` events on average |
| `Triangular(min, max, mode?)` | value between `min` and `max` that is most likely near `mode` (the middle) |

Integers are unbiased: every value of the range is exactly equally likely.

## Lists

| Function | |
| --- | --- |
| `Pick(list)` | random element and its index, `nil` for empty lists |
| `Shuffle(list)` | shuffles the list in place and returns it |
| `Sample(list, count)` | new list of `count` distinct elements, fewer if the list is shorter |
| `Weighted(weights)` | one item picked by weight, see [Weighted picking](#weighted-picking) for the format |

## Geometry

| Function | |
| --- | --- |
| `InsideCircle(radius?)` | `x, y` evenly distributed inside a circle (1) |
| `InsideSphere(radius?)` | `x, y, z` evenly distributed inside a sphere |
| `OnSphere(radius?)` | `x, y, z` on the surface of a sphere, also a random direction |

```lua
local x, y = Random.InsideCircle(300)
Character(Vector(center.X + x, center.Y + y, center.Z), Rotator(), "nanos-world::SK_Male")
```

## Dice

`Roll(notation)` rolls dice and returns the total and a list of all rolled dice:

```lua
local total, rolls = Random.Roll("2d6+3")
```

| Notation | |
| --- | --- |
| `NdM` | `N` dice with `M` sides, `N` is optional (`d20`) |
| `d%` | a die with 100 sides |
| `NdMkhK` / `NdMklK` | keep the highest / lowest `K` dice, for example `4d6kh3` |
| `+`, `-` | add and subtract dice and numbers: `1d8+1d6-2` |

At most 1000 dice per term with up to 1000000 sides. The rolls list contains every die, also those dropped by `kh` / `kl`.

## Weighted picking

`Random.WeightedPicker(weights)` prepares a picker that picks in constant time, no matter how many items there are. Weights are relative and do not need to add up to 100. Items with weight 0 are never picked.

```lua
local picker = Random.WeightedPicker({ Common = 70, Rare = 25, Epic = 5 })

local weapons = Random.WeightedPicker({
    { Item = { Name = "Pistol", Ammo = 30 }, Weight = 10 },
    { Item = { Name = "Rifle", Ammo = 90 }, Weight = 3 },
})
```

Weights are either a table of `item = weight` pairs, or a list of `{ Item, Weight }` entries, which allows any value as item, also tables. With pairs the items are ordered by key, so the same generator picks the same items regardless of table order.

| Method | |
| --- | --- |
| `:Pick(generator?)` | one item, uses the default generator without one |
| `:PickMany(count, generator?)` | list of `count` items, picked with replacement |
| `:GetProbability(item)` | chance of one pick returning the item, from 0 to 1 |
| `:GetItems()` | list of `{ Item, Weight }` |
| `:GetCount()`, `:GetTotalWeight()` | |

## Loot tables

`Random.LootTable(definition)` describes what a chest, an enemy or a crate drops:

```lua
local gems = Random.LootTable({
    Entries = { { Item = "Ruby", Weight = 2 }, { Item = "Diamond", Weight = 1 } },
})

local chest = Random.LootTable({
    Rolls = { 2, 4 },
    Guaranteed = {
        { Item = "Gold", Count = { 10, 50 } },
        { Item = "Key", Chance = 0.1 },
    },
    Entries = {
        { Item = "Bandage", Weight = 50, Count = { 1, 3 } },
        { Item = "Ammo", Weight = 30, Count = 20 },
        { Table = gems, Weight = 10 },
        { Item = "Golden Rifle", Weight = 1 },
        { Weight = 20 },
    },
    Pity = { { Item = "Golden Rifle", After = 50 } },
})

for _, drop in ipairs(chest:Roll({ Merge = true })) do
    Console.Log(drop.Item .. " x" .. drop.Count)
end
```

| Field | |
| --- | --- |
| `Rolls` | how often `Entries` is rolled, a number or `{ Min, Max }` (1) |
| `Entries` | weighted entries, one is picked per roll |
| `Guaranteed` | entries that drop on every roll of the table, with an optional `Chance` |
| `Pity` | rules `{ Item, After, Key? }` that force an item of `Entries` after `After` rolls without it |

| Entry field | |
| --- | --- |
| `Item` | the dropped value, any value except `nil`. Entries without `Item` and `Table` drop nothing |
| `Table` | another loot table that is rolled instead of dropping an item |
| `Weight` | relative weight in `Entries` (1) |
| `Count` | amount, a number or `{ Min, Max }` (1). For `Table` entries, how often the table is rolled |
| `Chance` | probability from 0 to 1 that the entry is applied at all |

`:Roll(options?)` returns a list of `{ Item, Count }` drops:

| Option | |
| --- | --- |
| `Generator` | generator to roll with, the default generator without one |
| `Merge` | combine drops of the same item into one entry (`false`) |
| `Pity` | table that keeps the pity counters, see below |

Pity counters only work when a table for them is passed, store one per player to make sure unlucky players get the item eventually. The counters are plain numbers under `Key` (the item by default), so the table can be saved, for example as JSON. Pity rules apply to the entries of the table that is rolled, not to nested tables.

```lua
local pity = player_pity[player] or {}
player_pity[player] = pity
local drops = chest:Roll({ Pity = pity })
```

`:GetChance(item)` returns the chance of a single roll of `Entries` picking the item.

## Noise

`Random.Noise(options?)` creates coherent noise: random values that change smoothly with the position, for terrain heights, resource distribution, weather or wobbling movement.

```lua
local terrain = Random.Noise({ Seed = "map-seed", Type = "OpenSimplex2", Fractal = "FBm", Octaves = 5, Frequency = 0.002 })
local height = terrain:Get(x, y) * 2000
```

| Option | |
| --- | --- |
| `Seed` | integer or string (1337) |
| `Type` | `OpenSimplex2`, `OpenSimplex2S`, `Perlin`, `Cellular`, `Value`, `ValueCubic` (`OpenSimplex2`) |
| `Frequency` | how quickly the noise changes, smaller values give larger features (0.01) |
| `Fractal` | `None`, `FBm`, `Ridged`, `PingPong`, layers several octaves for more detail (`None`) |
| `Octaves`, `Lacunarity`, `Gain` | number of layers (3), frequency multiplier (2) and strength multiplier (0.5) per layer |
| `WeightedStrength`, `PingPongStrength` | fractal fine tuning (0, 2) |
| `CellularDistance` | `Euclidean`, `EuclideanSq`, `Manhattan`, `Hybrid` (`EuclideanSq`) |
| `CellularReturn` | `CellValue`, `Distance`, `Distance2`, `Distance2Add`, `Distance2Sub`, `Distance2Mul`, `Distance2Div` (`Distance`) |
| `CellularJitter` | how irregular cells are (1) |

| Method | |
| --- | --- |
| `:Get(x, y, z?)` | value from about -1 to 1 at a 2D or 3D position |
| `:Fill(width, height, options?, callback?)` | `width * height` values as a [Buffer](buffer.md) of little endian 32 bit floats, row by row |

`Fill` samples a grid starting at `X`, `Y` with distance `Step` (1) between samples, at height `Z` for 3D noise. Up to 16777216 samples, with a callback the grid is computed on a worker thread.

```lua
terrain:Fill(256, 256, { X = chunk_x * 256, Y = chunk_y * 256, Step = 4 }, function(heights)
    local base = heights:GetIndexBase()
    for row = 0, 255 do
        for column = 0, 255 do
            local height = heights:Read("f32le", base + (row * 256 + column) * 4)
        end
    end
end)
```
