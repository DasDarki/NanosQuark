---@meta

---@alias QuarkCell { X: integer, Y: integer }
---@alias QuarkNodeID integer|string
---@alias QuarkPoints QuarkVector[]|QuarkBufferView

---@class QuarkFlowField
local FlowField = {}

---@param x integer
---@param y integer
---@return integer dx
---@return integer dy
function FlowField:GetDirection(x, y) end

---@param x integer
---@param y integer
---@return number?
function FlowField:GetDistance(x, y) end

---@param x integer
---@param y integer
---@return QuarkCell[]?
function FlowField:GetPath(x, y) end

---@param x integer
---@param y integer
---@return boolean
function FlowField:IsReachable(x, y) end

---Pathfinding grid.
---@class QuarkGrid
local Grid = {}

---@return integer width
---@return integer height
function Grid:GetSize() end

---@param x integer
---@param y integer
---@param blocked? boolean
function Grid:SetBlocked(x, y, blocked) end

---@param x integer
---@param y integer
---@return boolean
function Grid:IsBlocked(x, y) end

---@param x integer
---@param y integer
---@param cost number|false
function Grid:SetCost(x, y, cost) end

---@param x integer
---@param y integer
---@return number?
function Grid:GetCost(x, y) end

---@param cost number|false
function Grid:Fill(cost) end

---@param x1 integer
---@param y1 integer
---@param x2 integer
---@param y2 integer
---@param cost number|false
function Grid:FillRect(x1, y1, x2, y2, cost) end

---@param values number[]|QuarkBufferView
function Grid:LoadCosts(values) end

---@param from QuarkCell
---@param to QuarkCell
---@param options? { Algorithm?: "AStar"|"Dijkstra", MaxNodes?: integer, Smooth?: boolean }
---@param callback? function
---@return QuarkCell[]?
---@return number|string? cost_or_message
function Grid:FindPath(from, to, options, callback) end

---@param goals QuarkCell|QuarkCell[]
---@param options? { MaxCost?: number }
---@param callback? function
---@return QuarkFlowField
function Grid:FlowField(goals, options, callback) end

---@param from QuarkCell
---@param to QuarkCell
---@return boolean
function Grid:HasLineOfSight(from, to) end

---@param position QuarkVector
---@return integer x
---@return integer y
---@return boolean inside
function Grid:ToCell(position) end

---@param x integer
---@param y integer
---@param z? number
---@return Vector
function Grid:ToWorld(x, y, z) end

---@return QuarkGrid
function Grid:Clone() end

---Navigation graph.
---@class QuarkGraph
local Graph = {}

---@param id QuarkNodeID
---@param position? QuarkVector
function Graph:AddNode(id, position) end

---@param id QuarkNodeID
---@return boolean
function Graph:RemoveNode(id) end

---@param from QuarkNodeID
---@param to QuarkNodeID
---@param cost? number
---@param options? { Bidirectional?: boolean }
function Graph:AddEdge(from, to, cost, options) end

---@param from QuarkNodeID
---@param to QuarkNodeID
---@param both? boolean
function Graph:RemoveEdge(from, to, both) end

---@param id QuarkNodeID
---@return boolean
function Graph:HasNode(id) end

---@return integer
function Graph:GetNodeCount() end

---@param id QuarkNodeID
---@return Vector?
function Graph:GetPosition(id) end

---@param id QuarkNodeID
---@return { ID: QuarkNodeID, Cost: number }[]
function Graph:GetNeighbors(id) end

---@param position QuarkVector
---@return QuarkNodeID?
---@return number? distance
function Graph:FindNearest(position) end

---@param from QuarkNodeID
---@param to QuarkNodeID
---@param options? { Heuristic?: boolean }
---@param callback? function
---@return QuarkNodeID[]?
---@return number|string? cost_or_message
function Graph:FindPath(from, to, options, callback) end

---@param from QuarkNodeID
---@param max_cost? number
---@return table<QuarkNodeID, number>
function Graph:GetDistances(from, max_cost) end

---@class QuarkPathfindingModule
local Pathfinding = {}

---@param width integer
---@param height integer
---@param options? { Diagonal?: boolean, CornerCutting?: boolean, CellSize?: number, Origin?: QuarkVector }
---@return QuarkGrid
function Pathfinding.Grid(width, height, options) end

---@return QuarkGraph
function Pathfinding.Graph() end

---@class QuarkSpatialHash
local SpatialHash = {}

---@param id QuarkNodeID
---@param position QuarkVector
function SpatialHash:Set(id, position) end

---@param id QuarkNodeID
---@return boolean
function SpatialHash:Remove(id) end

---@param id QuarkNodeID
---@return boolean
function SpatialHash:Has(id) end

---@param id QuarkNodeID
---@return Vector?
function SpatialHash:GetPosition(id) end

---@return integer
function SpatialHash:GetCount() end

function SpatialHash:Clear() end

---@param position QuarkVector
---@param radius number
---@param options? { Limit?: integer, WithDistance?: boolean }
---@return table
function SpatialHash:QueryRadius(position, radius, options) end

---@param minimum QuarkVector
---@param maximum QuarkVector
---@return QuarkNodeID[]
function SpatialHash:QueryBox(minimum, maximum) end

---@param position QuarkVector
---@param options? { Count?: integer, MaxDistance?: number, Exclude?: QuarkNodeID, WithDistance?: boolean }
---@return table
function SpatialHash:FindNearest(position, options) end

---@class QuarkKDTree
local KDTree = {}

---@param position QuarkVector
---@param options? { Count?: integer, MaxDistance?: number, WithDistance?: boolean }
---@return table
function KDTree:FindNearest(position, options) end

---@param position QuarkVector
---@param radius number
---@param options? { WithDistance?: boolean }
---@return table
function KDTree:QueryRadius(position, radius, options) end

---@return integer
function KDTree:GetCount() end

---@class QuarkSpatialModule
local Spatial = {}

---@param cell_size? number
---@return QuarkSpatialHash
function Spatial.Hash(cell_size) end

---@param points QuarkPoints
---@return QuarkKDTree
function Spatial.KDTree(points) end

---@param points QuarkPoints
---@param radius number
---@param options? { Limit?: integer }
---@return { A: integer, B: integer, Distance: number }[]
function Spatial.FindPairs(points, radius, options) end

---@param points QuarkPoints
---@param center QuarkVector
---@param radius number
---@param options? { WithDistance?: boolean }
---@return table
function Spatial.FindWithin(points, center, radius, options) end

---@param points QuarkPoints
---@param center QuarkVector
---@return number[]
function Spatial.Distances(points, center) end
