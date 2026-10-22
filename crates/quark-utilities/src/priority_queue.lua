local error = error
local setmetatable = setmetatable
local type = type

local PriorityQueue = {}
PriorityQueue.__index = PriorityQueue

local function before(self, first, second)
    local left, right = self.priorities[first], self.priorities[second]
    if left == right then
        return self.order[first] < self.order[second]
    end
    if self.max then
        return left > right
    end
    return left < right
end

local function swap(self, first, second)
    self.values[first], self.values[second] = self.values[second], self.values[first]
    self.priorities[first], self.priorities[second] = self.priorities[second], self.priorities[first]
    self.order[first], self.order[second] = self.order[second], self.order[first]
end

local function sift_up(self, index)
    while index > 1 do
        local parent = index // 2
        if not before(self, index, parent) then
            return
        end
        swap(self, index, parent)
        index = parent
    end
end

local function sift_down(self, index)
    local size = self.size
    while true do
        local left = index * 2
        local right = left + 1
        local best = index
        if left <= size and before(self, left, best) then
            best = left
        end
        if right <= size and before(self, right, best) then
            best = right
        end
        if best == index then
            return
        end
        swap(self, index, best)
        index = best
    end
end

function PriorityQueue:Push(value, priority)
    if value == nil then
        error("the value must not be nil", 2)
    end
    if type(priority) ~= "number" or priority ~= priority then
        error("the priority has to be a number", 2)
    end
    local index = self.size + 1
    self.size = index
    self.counter = self.counter + 1
    self.values[index] = value
    self.priorities[index] = priority
    self.order[index] = self.counter
    sift_up(self, index)
    return self
end

function PriorityQueue:Pop()
    local size = self.size
    if size == 0 then
        return nil
    end
    local value, priority = self.values[1], self.priorities[1]
    swap(self, 1, size)
    self.values[size] = nil
    self.priorities[size] = nil
    self.order[size] = nil
    self.size = size - 1
    sift_down(self, 1)
    return value, priority
end

function PriorityQueue:Peek()
    if self.size == 0 then
        return nil
    end
    return self.values[1], self.priorities[1]
end

function PriorityQueue:GetSize()
    return self.size
end

function PriorityQueue:IsEmpty()
    return self.size == 0
end

function PriorityQueue:Clear()
    self.values, self.priorities, self.order = {}, {}, {}
    self.size = 0
end

function PriorityQueue:Remove(value)
    for index = 1, self.size do
        if self.values[index] == value then
            local last = self.size
            swap(self, index, last)
            self.values[last] = nil
            self.priorities[last] = nil
            self.order[last] = nil
            self.size = last - 1
            if index <= self.size then
                sift_down(self, index)
                sift_up(self, index)
            end
            return true
        end
    end
    return false
end

function PriorityQueue:ToList()
    local copy = setmetatable({
        max = self.max,
        size = self.size,
        counter = self.counter,
        values = table.move(self.values, 1, self.size, 1, {}),
        priorities = table.move(self.priorities, 1, self.size, 1, {}),
        order = table.move(self.order, 1, self.size, 1, {}),
    }, PriorityQueue)
    local list = {}
    while copy.size > 0 do
        local value, priority = copy:Pop()
        list[#list + 1] = { Value = value, Priority = priority }
    end
    return list
end

local function create(options)
    if options ~= nil and type(options) ~= "table" then
        error("expected an options table", 2)
    end
    local order = options and options.Order or "Min"
    if order ~= "Min" and order ~= "Max" then
        error("Order has to be Min or Max", 2)
    end
    return setmetatable({
        max = order == "Max",
        size = 0,
        counter = 0,
        values = {},
        priorities = {},
        order = {},
    }, PriorityQueue)
end

return { New = create }
