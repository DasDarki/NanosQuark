# Pathfinding and spatial queries

```lua
local grid = Quark.Pathfinding.Grid(200, 200, { CellSize = 100, Origin = Vector(-10000, -10000, 0) })
grid:SetBlocked(42, 17)

local path, cost = grid:FindPath({ X = 0, Y = 0 }, { X = 150, Y = 120 }, { Smooth = true })

local nearby = Quark.Spatial.Hash(1000)
nearby:Set(player:GetID(), player:GetControlledCharacter():GetLocation())
local close_players = nearby:QueryRadius(explosion, 800)
```

Two modules for work that is slow in Lua: `Pathfinding` for routes on grids and navigation graphs, and `Spatial` for finding objects near positions. Positions are nanos world `Vector`s or tables with `X`, `Y` and `Z`.

## Grids

`Pathfinding.Grid(width, height, options?)` is a grid of cells with movement costs, for top down maps, building interiors or AI on a coarse map. Cells are addressed with `X` and `Y` starting at the [index base](buffer.md#index-base) of the package (0 by default) and passed as tables `{ X = 4, Y = 2 }`.

| Option | |
| --- | --- |
| `Diagonal` | allow diagonal moves, which cost `sqrt(2)` times the cell cost (`true`) |
| `CornerCutting` | allow diagonal moves past a blocked corner (`false`) |
| `CellSize`, `Origin` | world size of a cell (100) and world position of the corner of the first cell, used by `ToCell` and `ToWorld` |

Every cell has a cost for entering it, 1 by default. Higher costs make paths avoid the cell (mud, water, danger), blocked cells cannot be entered.

| Method | |
| --- | --- |
| `:SetBlocked(x, y, blocked?)`, `:IsBlocked(x, y)` | |
| `:SetCost(x, y, cost)`, `:GetCost(x, y)` | `cost` greater than 0, `false` or 0 blocks the cell, `GetCost` is `nil` for blocked cells |
| `:Fill(cost)`, `:FillRect(x1, y1, x2, y2, cost)` | set many cells, `false` blocks them |
| `:LoadCosts(values)` | all costs from a list or [buffer view](buffer.md#views), row by row, 0 or less blocks a cell |
| `:FindPath(from, to, options?, callback?)` | list of cells from start to goal and the total cost, or `nil, message` |
| `:FlowField(goals, options?, callback?)` | flow field towards one goal or a list of goals, option `MaxCost` |
| `:HasLineOfSight(from, to)` | whether a straight line between the cells only crosses free cells |
| `:ToCell(position)` | `x, y, inside` of the cell containing a world position |
| `:ToWorld(x, y, z?)` | world position of the center of a cell |
| `:GetSize()`, `:Clone()` | |

`FindPath` options are `Algorithm` (`AStar`, or `Dijkstra` without heuristic), `MaxNodes` (give up after visiting this many cells) and `Smooth`, which removes cells that can be skipped with a straight line and leaves only the corners of the path. With a callback the search runs on a worker thread. The worker uses a snapshot of the grid, changes afterwards do not affect it, and taking the snapshot does not copy the grid until it is changed.

### Flow fields

A flow field stores for every cell the direction towards the nearest goal. It is calculated once and then answers for any number of units, which makes it ideal for many enemies chasing a player or crowds walking to an exit.

| Method | |
| --- | --- |
| `:GetDirection(x, y)` | step `dx, dy` towards the goal, `0, 0` at the goal and in unreachable cells |
| `:GetDistance(x, y)` | cost to the goal, `nil` when unreachable |
| `:GetPath(x, y)` | complete path to the goal, or `nil` |
| `:IsReachable(x, y)` | |

```lua
local field = grid:FlowField({ X = exit_x, Y = exit_y })
for _, npc in ipairs(crowd) do
    local x, y = grid:ToCell(npc:GetLocation())
    local dx, dy = field:GetDirection(x, y)
    npc:MoveTo(grid:ToWorld(x + dx, y + dy, npc:GetLocation().Z))
end
```

## Navigation graphs

`Pathfinding.Graph()` connects named points, like waypoints on roads, rooms of a building or stations of a train network. Node IDs are integers or strings.

| Method | |
| --- | --- |
| `:AddNode(id, position?)` | adds or moves a node |
| `:RemoveNode(id)` | removes the node and its edges |
| `:AddEdge(from, to, cost?, options?)` | connection, the cost defaults to the distance of the positions (or 1). Option `Bidirectional` (`true`) |
| `:RemoveEdge(from, to, both?)` | |
| `:FindPath(from, to, options?, callback?)` | list of node IDs and the total cost, or `nil, message` |
| `:GetDistances(from, max_cost?)` | table of node ID to cost for all reachable nodes |
| `:FindNearest(position)` | the node closest to a position and its distance |
| `:GetNeighbors(id)` | list of `{ ID, Cost }` |
| `:HasNode(id)`, `:GetPosition(id)`, `:GetNodeCount()` | |

When every node has a position, `FindPath` uses A* with the straight line distance as heuristic. This finds the cheapest path as long as no edge costs less than the distance between its nodes, with cheaper edges set the option `Heuristic = false`.

## Spatial hash

`Spatial.Hash(cell_size?)` keeps the positions of moving objects, for example players, vehicles or projectiles, and finds the ones near a position without looking at every object. `cell_size` (1000) should be roughly the radius of typical queries.

| Method | |
| --- | --- |
| `:Set(id, position)` | adds or moves an object, IDs are integers or strings |
| `:Remove(id)`, `:Has(id)`, `:GetPosition(id)`, `:GetCount()`, `:Clear()` | |
| `:QueryRadius(position, radius, options?)` | IDs within the radius, closest first. Options `Limit` and `WithDistance` (list of `{ ID, Distance }`) |
| `:QueryBox(minimum, maximum)` | IDs inside an axis aligned box |
| `:FindNearest(position, options?)` | closest IDs, options `Count` (1), `MaxDistance`, `Exclude` (an ID to skip, like the object asking) and `WithDistance` |

Update positions once per tick or less often for slow objects, `Set` only does work when an object moves into another cell.

## KD tree

`Spatial.KDTree(points)` indexes a fixed set of positions, like spawn points, resource nodes or shop locations, for fast nearest neighbour queries. Points are a list of positions or a buffer view with three numbers (`X`, `Y`, `Z`) per point. Results are point numbers starting at 1, in the order of the input.

| Method | |
| --- | --- |
| `:FindNearest(position, options?)` | numbers of the closest points, options `Count` (1), `MaxDistance` and `WithDistance` (list of `{ Index, Distance }`) |
| `:QueryRadius(position, radius, options?)` | numbers of all points within the radius, closest first, option `WithDistance` |
| `:GetCount()` | |

## Bulk queries

| Function | |
| --- | --- |
| `Spatial.FindPairs(points, radius, options?)` | all pairs of points closer than `radius` as `{ A, B, Distance }`, for collisions or proximity chat. Option `Limit` (1000000) |
| `Spatial.FindWithin(points, center, radius, options?)` | numbers of the points within the radius, closest first, option `WithDistance` |
| `Spatial.Distances(points, center)` | list of distances to a position |

These take the same points as `KDTree` and are faster than nested loops in Lua for hundreds of points or more.
