local WeightedPicker, Int, Float, is_generator = ...

local error = error
local getmetatable = getmetatable
local ipairs = ipairs
local setmetatable = setmetatable
local type = type

local MAX_DEPTH = 16
local MAX_ROLLS = 10000

local LootTable = {}
LootTable.__index = LootTable

local function random_int(generator, minimum, maximum)
    if generator then
        return generator:Int(minimum, maximum)
    end
    return Int(minimum, maximum)
end

local function random_float(generator)
    if generator then
        return generator:Float()
    end
    return Float()
end

local function read_range(value, name, default)
    if value == nil then
        return default, default
    end
    if type(value) == "number" then
        if value < 0 or value % 1 ~= 0 then
            error(name .. " has to be a whole number of at least 0", 4)
        end
        return value, value
    end
    if type(value) == "table" then
        local minimum = value.Min or value[1]
        local maximum = value.Max or value[2] or minimum
        if type(minimum) ~= "number" or type(maximum) ~= "number" or minimum < 0 or minimum > maximum or minimum % 1 ~= 0 or maximum % 1 ~= 0 then
            error(name .. " has to be { Min, Max } with whole numbers and Min <= Max", 4)
        end
        return minimum, maximum
    end
    error(name .. " has to be a number or a { Min, Max } table", 4)
end

local function read_entry(entry, index, section)
    if type(entry) ~= "table" then
        error(section .. " entry " .. index .. " has to be a table", 3)
    end
    if entry.Table ~= nil and getmetatable(entry.Table) ~= LootTable then
        error(section .. " entry " .. index .. " has a Table that is not a loot table", 3)
    end
    if entry.Chance ~= nil and (type(entry.Chance) ~= "number" or entry.Chance < 0 or entry.Chance > 1) then
        error(section .. " entry " .. index .. " needs a Chance between 0 and 1", 3)
    end
    local minimum, maximum = read_range(entry.Count, section .. " entry " .. index .. " Count", 1)
    return {
        Item = entry.Item,
        Table = entry.Table,
        Chance = entry.Chance,
        Weight = entry.Weight or 1,
        Min = minimum,
        Max = maximum,
    }
end

local function create(definition)
    if type(definition) ~= "table" then
        error("a loot table needs a definition table", 2)
    end

    local self = setmetatable({ Entries = {}, Guaranteed = {}, Pity = {} }, LootTable)
    self.MinRolls, self.MaxRolls = read_range(definition.Rolls, "Rolls", 1)
    if self.MaxRolls > MAX_ROLLS then
        error("a loot table can roll at most " .. MAX_ROLLS .. " times", 2)
    end

    local weights = {}
    for index, entry in ipairs(definition.Entries or {}) do
        local parsed = read_entry(entry, index, "Entries")
        self.Entries[index] = parsed
        weights[index] = { Item = index, Weight = parsed.Weight }
    end
    if #weights > 0 then
        self.Picker = WeightedPicker(weights)
    end

    for index, entry in ipairs(definition.Guaranteed or {}) do
        self.Guaranteed[index] = read_entry(entry, index, "Guaranteed")
    end

    for index, rule in ipairs(definition.Pity or {}) do
        if type(rule) ~= "table" or type(rule.After) ~= "number" or rule.After < 1 then
            error("Pity rule " .. index .. " needs After of at least 1", 2)
        end
        local target
        for entry_index, entry in ipairs(self.Entries) do
            if entry.Item ~= nil and entry.Item == rule.Item then
                target = entry_index
                break
            end
        end
        if not target then
            error("Pity rule " .. index .. " names an item that is not in Entries", 2)
        end
        self.Pity[index] = { Entry = target, After = rule.After, Key = rule.Key or rule.Item }
    end

    return self
end

local function add_drop(drops, merged, item, count, merge)
    if count <= 0 or item == nil then
        return
    end
    if merge then
        local existing = merged[item]
        if existing then
            existing.Count = existing.Count + count
            return
        end
        local drop = { Item = item, Count = count }
        merged[item] = drop
        drops[#drops + 1] = drop
        return
    end
    drops[#drops + 1] = { Item = item, Count = count }
end

local roll_into

local function apply_entry(entry, drops, merged, options, depth)
    local generator = options.Generator
    if entry.Chance ~= nil and random_float(generator) >= entry.Chance then
        return
    end
    local count = random_int(generator, entry.Min, entry.Max)
    if entry.Table then
        for _ = 1, count do
            roll_into(entry.Table, drops, merged, options, depth + 1)
        end
    else
        add_drop(drops, merged, entry.Item, count, options.Merge)
    end
end

roll_into = function(self, drops, merged, options, depth)
    if depth > MAX_DEPTH then
        error("loot tables are nested deeper than " .. MAX_DEPTH .. " levels", 0)
    end
    local generator = options.Generator
    local pity = options.Pity

    for _, entry in ipairs(self.Guaranteed) do
        apply_entry(entry, drops, merged, options, depth)
    end

    if not self.Picker then
        return
    end

    local rolls = random_int(generator, self.MinRolls, self.MaxRolls)
    for _ = 1, rolls do
        local index
        if pity and depth == 0 then
            for _, rule in ipairs(self.Pity) do
                if (pity[rule.Key] or 0) + 1 >= rule.After then
                    index = rule.Entry
                    break
                end
            end
        end
        index = index or self.Picker:Pick(generator)

        if pity and depth == 0 then
            for _, rule in ipairs(self.Pity) do
                if rule.Entry == index then
                    pity[rule.Key] = 0
                else
                    pity[rule.Key] = (pity[rule.Key] or 0) + 1
                end
            end
        end

        apply_entry(self.Entries[index], drops, merged, options, depth)
    end
end

function LootTable:Roll(options)
    if options ~= nil and type(options) ~= "table" then
        error("expected an options table", 2)
    end
    options = options or {}
    if options.Generator ~= nil and not is_generator(options.Generator) then
        error("the Generator option has to be a generator from Random.New", 2)
    end
    if options.Pity ~= nil and type(options.Pity) ~= "table" then
        error("the Pity option has to be a table that keeps the pity counters", 2)
    end
    local drops = {}
    roll_into(self, drops, {}, options, 0)
    return drops
end

function LootTable:GetChance(item)
    if not self.Picker then
        return 0
    end
    local chance = 0
    for index, entry in ipairs(self.Entries) do
        if entry.Item ~= nil and entry.Item == item then
            chance = chance + self.Picker:GetProbability(index)
        end
    end
    return chance
end

return create
