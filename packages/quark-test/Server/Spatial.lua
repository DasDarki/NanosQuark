local Test = Quark.Import("Testing")
local Expect = Test.Expect

Test.Describe("Pathfinding.Grid", function()
    local Pathfinding = Quark.Pathfinding
    local base = Quark.Buffer.GetIndexBase()

    local function wall_grid()
        local grid = Pathfinding.Grid(10, 10, { CellSize = 100 })
        for y = base, base + 8 do
            grid:SetBlocked(base + 5, y)
        end
        return grid
    end

    Test.It("finds shortest paths around walls", function()
        local grid = wall_grid()
        local path, cost = grid:FindPath({ X = base, Y = base }, { X = base + 9, Y = base })
        Expect(path[1]):ToEqual({ X = base, Y = base })
        Expect(path[#path]):ToEqual({ X = base + 9, Y = base })
        for _, point in ipairs(path) do
            Expect(grid:IsBlocked(point.X, point.Y)):ToBe(false)
        end
        Expect(cost > 20):ToBe(true)

        local dijkstra, dijkstra_cost = grid:FindPath({ X = base, Y = base }, { X = base + 9, Y = base }, { Algorithm = "Dijkstra" })
        Expect(dijkstra_cost):ToBeCloseTo(cost, 0.0001)
        Expect(#dijkstra > 0):ToBe(true)

        local straight = Pathfinding.Grid(5, 1):FindPath({ X = base, Y = base }, { X = base + 4, Y = base })
        Expect(#straight):ToBe(5)
    end)

    Test.It("respects costs, diagonals and smoothing", function()
        local grid = Pathfinding.Grid(3, 3, { Diagonal = false })
        grid:SetCost(base + 1, base + 1, 50)
        local path, cost = grid:FindPath({ X = base, Y = base + 1 }, { X = base + 2, Y = base + 1 })
        Expect(cost):ToBe(4)
        Expect(#path):ToBe(5)

        local open = Pathfinding.Grid(20, 20)
        local smooth = open:FindPath({ X = base, Y = base }, { X = base + 19, Y = base + 7 }, { Smooth = true })
        Expect(#smooth):ToBe(2)
        Expect(open:HasLineOfSight({ X = base, Y = base }, { X = base + 19, Y = base + 19 })):ToBe(true)
        open:FillRect(base + 5, base + 5, base + 6, base + 6, false)
        Expect(open:HasLineOfSight({ X = base, Y = base }, { X = base + 19, Y = base + 19 })):ToBe(false)
    end)

    Test.It("reports missing paths and works in the background", function()
        local grid = Pathfinding.Grid(5, 5)
        for y = base, base + 4 do
            grid:SetBlocked(base + 2, y)
        end
        local path, message = grid:FindPath({ X = base, Y = base }, { X = base + 4, Y = base + 4 })
        Expect(path):ToBeNil()
        Expect(message):ToContain("no path")
        Expect(select(2, grid:FindPath({ X = base + 2, Y = base }, { X = base, Y = base }))):ToContain("start cell is blocked")
        Expect(function() grid:FindPath({ X = base + 9, Y = base }, { X = base, Y = base }) end):ToThrow("outside of the grid")

        local big = Pathfinding.Grid(300, 300)
        local task_result = Quark.Await(big.FindPath, big, { X = base, Y = base }, { X = base + 299, Y = base + 299 })
        Expect(#task_result):ToBe(300)
        big:SetBlocked(base + 1, base + 1)
        Expect(big:IsBlocked(base + 1, base + 1)):ToBe(true)
    end)

    Test.It("builds flow fields", function()
        local grid = wall_grid()
        local field = grid:FlowField({ X = base + 9, Y = base })
        Expect(field:GetDistance(base + 9, base)):ToBe(0)
        Expect({ field:GetDirection(base + 8, base) }):ToEqual({ 1, 0 })
        Expect(field:IsReachable(base, base)):ToBe(true)
        local path = field:GetPath(base, base)
        Expect(path[#path]):ToEqual({ X = base + 9, Y = base })
        Expect(field:GetDistance(base + 5, base)):ToBeNil()
        local async_field = Quark.Await(grid.FlowField, grid, { { X = base, Y = base }, { X = base + 9, Y = base + 9 } })
        Expect(async_field:GetDistance(base + 9, base + 9)):ToBe(0)
    end)

    Test.It("converts between world and cell coordinates", function()
        local grid = Pathfinding.Grid(100, 100, { CellSize = 50, Origin = { X = -2500, Y = -2500, Z = 10 } })
        local x, y, inside = grid:ToCell({ X = 0, Y = 0, Z = 0 })
        Expect({ x, y, inside }):ToEqual({ base + 50, base + 50, true })
        local world = grid:ToWorld(base + 50, base + 50)
        Expect({ world.X, world.Y, world.Z }):ToEqual({ 25, 25, 10 })
        Expect(select(3, grid:ToCell({ X = 99999, Y = 0 }))):ToBe(false)

        local costs = {}
        for index = 1, 4 do costs[index] = index == 2 and 0 or 1 end
        local small = Pathfinding.Grid(2, 2)
        small:LoadCosts(costs)
        Expect(small:IsBlocked(base + 1, base)):ToBe(true)
        Expect(function() small:LoadCosts({ 1 }) end):ToThrow("expected 4 costs")
    end)
end)

Test.Describe("Pathfinding.Graph", function()
    local Pathfinding = Quark.Pathfinding

    Test.It("finds routes through navigation graphs", function()
        local graph = Pathfinding.Graph()
        graph:AddNode("spawn", { X = 0, Y = 0, Z = 0 })
        graph:AddNode("bridge", { X = 100, Y = 0, Z = 0 })
        graph:AddNode("tower", { X = 100, Y = 100, Z = 0 })
        graph:AddNode("cave", { X = 0, Y = 300, Z = 0 })
        graph:AddEdge("spawn", "bridge")
        graph:AddEdge("bridge", "tower")
        graph:AddEdge("spawn", "cave", 50)
        graph:AddEdge("cave", "tower", 500, { Bidirectional = false })

        local path, cost = graph:FindPath("spawn", "tower")
        Expect(path):ToEqual({ "spawn", "bridge", "tower" })
        Expect(cost):ToBe(200)
        Expect(graph:GetDistances("spawn")):ToEqual({ spawn = 0, bridge = 100, tower = 200, cave = 50 })
        Expect(graph:GetDistances("spawn", 120)):ToEqual({ spawn = 0, bridge = 100, cave = 50 })
        Expect(graph:FindNearest({ X = 90, Y = 90, Z = 0 })):ToBe("tower")
        Expect(#graph:GetNeighbors("spawn")):ToBe(2)
        Expect(graph:GetNodeCount()):ToBe(4)

        graph:RemoveEdge("bridge", "tower")
        local detour, detour_cost = Quark.Await(graph.FindPath, graph, "spawn", "tower")
        Expect(detour):ToEqual({ "spawn", "cave", "tower" })
        Expect(detour_cost):ToBe(550)
        Expect(select(2, graph:FindPath("tower", "spawn"))):ToContain("no path")
        Expect(graph:RemoveNode("cave")):ToBe(true)
        Expect(graph:HasNode("cave")):ToBe(false)
        Expect(select(2, graph:FindPath("spawn", "cave"))):ToContain("does not exist")

        local numbered = Pathfinding.Graph()
        for id = 1, 5 do numbered:AddNode(id) end
        for id = 1, 4 do numbered:AddEdge(id, id + 1, 2) end
        Expect({ numbered:FindPath(1, 5) }):ToEqual({ { 1, 2, 3, 4, 5 }, 8 })
    end)
end)

Test.Describe("Spatial", function()
    local Spatial = Quark.Spatial

    Test.It("tracks moving objects in a spatial hash", function()
        local hash = Spatial.Hash(100)
        hash:Set(1, { X = 0, Y = 0, Z = 0 })
        hash:Set(2, { X = 50, Y = 0, Z = 0 })
        hash:Set("boss", { X = 500, Y = 500, Z = 0 })
        Expect(hash:QueryRadius({ X = 0, Y = 0, Z = 0 }, 60)):ToEqual({ 1, 2 })
        Expect(hash:QueryRadius({ X = 0, Y = 0, Z = 0 }, 60, { WithDistance = true })[2]):ToEqual({ ID = 2, Distance = 50 })
        hash:Set(2, { X = 1000, Y = 0, Z = 0 })
        Expect(hash:QueryRadius({ X = 0, Y = 0, Z = 0 }, 60)):ToEqual({ 1 })
        Expect(hash:FindNearest({ X = 900, Y = 0, Z = 0 })):ToEqual({ 2 })
        Expect(hash:FindNearest({ X = 0, Y = 0, Z = 0 }, { Count = 2, Exclude = 1 })):ToEqual({ "boss", 2 })
        Expect(#hash:QueryBox({ X = -10, Y = -10, Z = -10 }, { X = 600, Y = 600, Z = 10 })):ToBe(2)
        Expect(hash:GetPosition("boss").X):ToBe(500)
        Expect(hash:Remove(1)):ToBe(true)
        Expect(hash:Has(1)):ToBe(false)
        Expect(hash:GetCount()):ToBe(2)
        hash:Clear()
        Expect(hash:GetCount()):ToBe(0)
    end)

    Test.It("answers nearest neighbour queries with a KD tree", function()
        local generator = Quark.Random.New(1)
        local points = {}
        for index = 1, 2000 do
            points[index] = { X = generator:Float(-1000, 1000), Y = generator:Float(-1000, 1000), Z = generator:Float(-100, 100) }
        end
        local tree = Spatial.KDTree(points)
        Expect(tree:GetCount()):ToBe(2000)
        local target = { X = 12, Y = -40, Z = 5 }
        local best, best_distance = nil, math.huge
        for index, point in ipairs(points) do
            local distance = math.sqrt((point.X - target.X) ^ 2 + (point.Y - target.Y) ^ 2 + (point.Z - target.Z) ^ 2)
            if distance < best_distance then
                best, best_distance = index, distance
            end
        end
        Expect(tree:FindNearest(target)):ToEqual({ best })
        local five = tree:FindNearest(target, { Count = 5, WithDistance = true })
        Expect(#five):ToBe(5)
        Expect(five[1].Distance):ToBeCloseTo(best_distance, 0.0001)
        Expect(five[1].Distance <= five[5].Distance):ToBe(true)
        local within = tree:QueryRadius(target, 150)
        Expect(#within):ToBe(#Spatial.FindWithin(points, target, 150))

        local buffer = Quark.Buffer.New(6 * 4)
        buffer:View("f32"):FromTable({ 0, 0, 0, 10, 0, 0 })
        Expect(Spatial.KDTree(buffer:View("f32")):FindNearest({ X = 9, Y = 0, Z = 0 })):ToEqual({ 2 })
    end)

    Test.It("finds close pairs in bulk", function()
        local points = { { X = 0, Y = 0, Z = 0 }, { X = 5, Y = 0, Z = 0 }, { X = 100, Y = 0, Z = 0 }, { X = 103, Y = 4, Z = 0 } }
        local pairs_found = Spatial.FindPairs(points, 10)
        Expect(pairs_found):ToEqual({ { A = 1, B = 2, Distance = 5 }, { A = 3, B = 4, Distance = 5 } })
        Expect(Spatial.FindWithin(points, { X = 0, Y = 0, Z = 0 }, 6)):ToEqual({ 1, 2 })
        Expect(Spatial.Distances(points, { X = 0, Y = 0, Z = 0 })[3]):ToBe(100)
        Expect(#Spatial.FindPairs(points, 1000, { Limit = 2 })):ToBe(2)
    end)
end)
