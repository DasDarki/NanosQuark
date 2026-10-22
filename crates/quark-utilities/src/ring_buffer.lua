local error = error
local math_type = math.type
local setmetatable = setmetatable

local RingBuffer = {}
RingBuffer.__index = RingBuffer

local function slot(self, position)
    return (self.start + position - 1) % self.capacity + 1
end

local function resolve(self, index)
    if math_type(index) ~= "integer" then
        error("the index has to be an integer", 3)
    end
    if index < 0 then
        index = self.size + index + 1
    end
    if index < 1 or index > self.size then
        return nil
    end
    return slot(self, index - 1)
end

function RingBuffer:Push(value)
    if value == nil then
        error("the value must not be nil", 2)
    end
    if self.size < self.capacity then
        self.items[slot(self, self.size)] = value
        self.size = self.size + 1
        return nil
    end
    local evicted = self.items[self.start]
    self.items[self.start] = value
    self.start = self.start % self.capacity + 1
    return evicted
end

function RingBuffer:Shift()
    if self.size == 0 then
        return nil
    end
    local value = self.items[self.start]
    self.items[self.start] = nil
    self.start = self.start % self.capacity + 1
    self.size = self.size - 1
    return value
end

function RingBuffer:Pop()
    if self.size == 0 then
        return nil
    end
    local index = slot(self, self.size - 1)
    local value = self.items[index]
    self.items[index] = nil
    self.size = self.size - 1
    return value
end

function RingBuffer:Get(index)
    local position = resolve(self, index)
    return position and self.items[position]
end

function RingBuffer:Set(index, value)
    if value == nil then
        error("the value must not be nil", 2)
    end
    local position = resolve(self, index)
    if not position then
        error("index " .. index .. " is outside of the buffer", 2)
    end
    self.items[position] = value
end

function RingBuffer:GetFirst()
    return self:Get(1)
end

function RingBuffer:GetLast()
    return self:Get(-1)
end

function RingBuffer:GetSize()
    return self.size
end

function RingBuffer:GetCapacity()
    return self.capacity
end

function RingBuffer:IsEmpty()
    return self.size == 0
end

function RingBuffer:IsFull()
    return self.size == self.capacity
end

function RingBuffer:Clear()
    self.items = {}
    self.start = 1
    self.size = 0
end

function RingBuffer:ToList()
    local list = {}
    for position = 0, self.size - 1 do
        list[position + 1] = self.items[slot(self, position)]
    end
    return list
end

function RingBuffer:Values()
    local position = 0
    return function()
        position = position + 1
        if position > self.size then
            return nil
        end
        return position, self.items[slot(self, position - 1)]
    end
end

local function create(capacity)
    if math_type(capacity) ~= "integer" or capacity < 1 then
        error("the capacity has to be an integer of at least 1", 2)
    end
    return setmetatable({ capacity = capacity, items = {}, start = 1, size = 0 }, RingBuffer)
end

return { New = create }
