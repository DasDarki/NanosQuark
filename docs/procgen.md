# ProcGen

```lua
local ProcGen = Quark.ProcGen

local cave = ProcGen.Caves(120, 80, { Generator = Quark.Random.New("cave-" .. level) })
local dungeon = ProcGen.Dungeon(100, 60, { MaxRoomSize = 12 })
local heights = ProcGen.Heightmap(256, 256, { Fractal = "FBm", Island = true, Max = 3000 })
```

Procedural generation of tile maps and terrain. All generators take an optional `Generator` from [Random.New](random.md#generators), the same seed always produces the same map. Cells are addressed starting at the [index base](buffer.md#index-base) of the package, like pathfinding grids.

## Tile maps

`ProcGen.Caves(width, height, options?)` grows organic caves with a cellular automaton: cells start as random walls and floors and become walls when most neighbours are walls. The border is always wall.

| Option | |
| --- | --- |
| `FillChance` | share of initial walls (0.45), more gives narrower caves |
| `Steps` | smoothing steps (5) |
| `BirthLimit`, `DeathLimit` | a floor becomes wall with more than `BirthLimit` wall neighbours (4), a wall becomes floor with fewer than `DeathLimit` (4) |
| `KeepLargest` | fill all caves except the largest, so every floor cell is reachable (`true`) |
| `Generator` | random generator |

`ProcGen.Dungeon(width, height, options?)` places rectangular rooms without overlap and connects them with corridors along a minimum spanning tree, so all rooms are reachable, plus some extra corridors for loops.

| Option | |
| --- | --- |
| `Attempts` | how often a room is tried (60), more gives more rooms |
| `MinRoomSize`, `MaxRoomSize` | room sizes in cells (4 and 10) |
| `Loops` | expected number of extra connections per room for alternative routes (1), 0 for a tree |
| `Doors` | mark where corridors enter rooms (`true`) |
| `Generator` | random generator |

Both return a map. Tiles are numbers, `ProcGen.Tile` holds their names: `Wall` (0), `Floor` (1), `Corridor` (2) and `Door` (3).

| Method | |
| --- | --- |
| `:GetSize()` | width and height |
| `:GetTile(x, y)`, `:SetTile(x, y, tile)`, `:IsWalkable(x, y)` | |
| `:GetRooms()` | rooms of dungeons as `{ X, Y, Width, Height, Center = { X, Y } }` |
| `:GetFloorCells()` | all walkable cells as `{ X, Y }`, for example to pick spawn points |
| `:GetTiles()` | all tiles as a [buffer](buffer.md) of bytes, row by row |
| `:ToCosts()` | costs for [Pathfinding.Grid](spatial.md#grids) `:LoadCosts`, walls blocked |
| `:ToText()` | text picture with `#` walls, `.` floors, `,` corridors and `+` doors, handy for debugging |

```lua
local map = ProcGen.Dungeon(64, 64)
local grid = Quark.Pathfinding.Grid(64, 64, { CellSize = 400 })
grid:LoadCosts(map:ToCosts())

for y = 0, 63 do
    for x = 0, 63 do
        if not map:IsWalkable(x, y) then
            StaticMesh(grid:ToWorld(x, y), Rotator(), "nanos-world::SM_Cube"):SetScale(Vector(4, 4, 4))
        end
    end
end
```

## Heightmaps

`ProcGen.Heightmap(width, height, options?, callback?)` returns a [buffer](buffer.md) of `width * height` little endian 32 bit floats, row by row, read it with `buffer:View("f32")`. It takes all options of [Random.Noise](random.md#noise) and these:

| Option | |
| --- | --- |
| `Min`, `Max` | heights are scaled to this range (0 and 1) |
| `Step` | noise distance between neighbouring cells (1), combine it with `Frequency` |
| `Island` | lower the edges, so the map becomes an island surrounded by its lowest height |
| `Erosion`, `Talus` | iterations of thermal erosion that flattens steep slopes (0), and the slope that stays (0.01 of the height range) |

With a callback the heightmap is generated on a worker thread.
