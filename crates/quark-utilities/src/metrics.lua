local clock, report = ...

local concat = table.concat
local error = error
local ipairs = ipairs
local math_type = math.type
local pairs = pairs
local pcall = pcall
local setmetatable = setmetatable
local sort = table.sort
local string_find = string.find
local string_format = string.format
local string_gsub = string.gsub
local string_match = string.match
local tostring = tostring
local type = type

local DEFAULT_BUCKETS = { 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1, 2.5, 5, 10 }

local function format_number(value)
    if value == math.huge then
        return "+Inf"
    elseif value == -math.huge then
        return "-Inf"
    elseif value ~= value then
        return "NaN"
    elseif math_type(value) == "integer" then
        return tostring(value)
    elseif value == math.floor(value) and value > -1e15 and value < 1e15 then
        return string_format("%d", value)
    end
    return string_format("%.17g", value)
end

local function escape_label(value)
    return (string_gsub(value, "[\\\"\n]", { ["\\"] = "\\\\", ["\""] = "\\\"", ["\n"] = "\\n" }))
end

local function escape_help(value)
    return (string_gsub(value, "[\\\n]", { ["\\"] = "\\\\", ["\n"] = "\\n" }))
end

local function read_labels(metric, labels, level)
    local names = metric.labels
    if labels == nil then
        if #names > 0 then
            error("the metric " .. metric.name .. " needs the labels " .. concat(names, ", "), level)
        end
        return ""
    end
    if type(labels) ~= "table" then
        error("labels have to be a table", level)
    end
    local values = {}
    for index, name in ipairs(names) do
        local value = labels[name]
        if value == nil then
            error("missing label '" .. name .. "' for the metric " .. metric.name, level)
        end
        values[index] = tostring(value)
    end
    for name in pairs(labels) do
        if not metric.label_set[name] then
            error("unknown label '" .. tostring(name) .. "' for the metric " .. metric.name, level)
        end
    end
    return concat(values, "\0")
end

local function series(metric, key)
    local entry = metric.series[key]
    if not entry then
        entry = { key = key, value = 0 }
        if metric.kind == "histogram" then
            entry.counts = {}
            for index = 1, #metric.buckets do
                entry.counts[index] = 0
            end
            entry.count = 0
        end
        metric.series[key] = entry
    end
    return entry
end

local function label_text(metric, key, extra)
    local parts = {}
    if #metric.labels > 0 then
        local start = 1
        for index, name in ipairs(metric.labels) do
            local stop = string_find(key, "\0", start, true) or #key + 1
            parts[index] = name .. "=\"" .. escape_label(key:sub(start, stop - 1)) .. "\""
            start = stop + 1
        end
    end
    if extra then
        parts[#parts + 1] = extra
    end
    if #parts == 0 then
        return ""
    end
    return "{" .. concat(parts, ",") .. "}"
end

local function sorted_series(metric)
    local list = {}
    for _, entry in pairs(metric.series) do
        list[#list + 1] = entry
    end
    sort(list, function(left, right)
        return left.key < right.key
    end)
    return list
end

local function split_amount(amount, labels)
    if type(amount) == "table" then
        return 1, amount
    end
    if amount == nil then
        return 1, labels
    end
    if type(amount) ~= "number" then
        error("the amount has to be a number", 3)
    end
    return amount, labels
end

local Counter = {}
Counter.__index = Counter

function Counter:Inc(amount, labels)
    amount, labels = split_amount(amount, labels)
    if amount < 0 then
        error("counters can only increase", 2)
    end
    local entry = series(self, read_labels(self, labels, 3))
    entry.value = entry.value + amount
end

function Counter:Get(labels)
    local entry = self.series[read_labels(self, labels, 3)]
    return entry and entry.value or 0
end

function Counter:Reset()
    self.series = {}
end

local Gauge = {}
Gauge.__index = Gauge

function Gauge:Set(value, labels)
    if type(value) ~= "number" then
        error("the value has to be a number", 2)
    end
    series(self, read_labels(self, labels, 3)).value = value
end

function Gauge:Inc(amount, labels)
    amount, labels = split_amount(amount, labels)
    local entry = series(self, read_labels(self, labels, 3))
    entry.value = entry.value + amount
end

function Gauge:Dec(amount, labels)
    amount, labels = split_amount(amount, labels)
    local entry = series(self, read_labels(self, labels, 3))
    entry.value = entry.value - amount
end

Gauge.Get = Counter.Get
Gauge.Reset = Counter.Reset

local Histogram = {}
Histogram.__index = Histogram

function Histogram:Observe(value, labels)
    if type(value) ~= "number" then
        error("the value has to be a number", 2)
    end
    local entry = series(self, read_labels(self, labels, 3))
    entry.value = entry.value + value
    entry.count = entry.count + 1
    for index, bound in ipairs(self.buckets) do
        if value <= bound then
            entry.counts[index] = entry.counts[index] + 1
        end
    end
end

local function finish_timer(self, labels, started, ok, ...)
    self:Observe(clock() - started, labels)
    if not ok then
        error((...), 0)
    end
    return ...
end

function Histogram:Time(callback, labels)
    if type(callback) ~= "function" then
        error("Time needs a function to measure", 2)
    end
    read_labels(self, labels, 3)
    local started = clock()
    return finish_timer(self, labels, started, pcall(callback))
end

function Histogram:Get(labels)
    local entry = self.series[read_labels(self, labels, 3)]
    if not entry then
        return { Count = 0, Sum = 0 }
    end
    return { Count = entry.count, Sum = entry.value }
end

Histogram.Reset = Counter.Reset

local Registry = {}
Registry.__index = Registry

local KINDS = { counter = Counter, gauge = Gauge, histogram = Histogram }

local function same_labels(left, right)
    if #left ~= #right then
        return false
    end
    for index, name in ipairs(left) do
        if right[index] ~= name then
            return false
        end
    end
    return true
end

local function register(registry, kind, name, help, options)
    if type(name) ~= "string" or not string_match(name, "^[%a_:][%w_:]*$") then
        error("metric names may only contain letters, digits, _ and : and must not start with a digit", 3)
    end
    if help ~= nil and type(help) ~= "string" then
        error("the help text has to be a string", 3)
    end
    if options ~= nil and type(options) ~= "table" then
        error("expected an options table", 3)
    end
    options = options or {}

    local labels = {}
    local label_set = {}
    for index, label in ipairs(options.Labels or {}) do
        if type(label) ~= "string" or not string_match(label, "^[%a_][%w_]*$") or label:sub(1, 2) == "__" then
            error("invalid label name '" .. tostring(label) .. "'", 3)
        end
        if kind == "histogram" and label == "le" then
            error("histograms cannot have a label named le", 3)
        end
        labels[index] = label
        label_set[label] = true
    end

    local buckets
    if kind == "histogram" then
        buckets = {}
        for index, bound in ipairs(options.Buckets or DEFAULT_BUCKETS) do
            if type(bound) ~= "number" then
                error("buckets have to be numbers", 3)
            end
            if index > 1 and bound <= buckets[index - 1] then
                error("buckets have to be sorted and unique", 3)
            end
            buckets[index] = bound
        end
        if #buckets == 0 then
            error("a histogram needs at least one bucket", 3)
        end
    end
    if options.Collect ~= nil and (kind ~= "gauge" or type(options.Collect) ~= "function") then
        error("Collect is a function and only available for gauges", 3)
    end

    local existing = registry.metrics[name]
    if existing then
        if existing.kind ~= kind or not same_labels(existing.labels, labels) then
            error("a different metric named " .. name .. " is already registered", 3)
        end
        existing.help = help or existing.help
        existing.collect = options.Collect or existing.collect
        return existing
    end

    local metric = setmetatable({
        kind = kind,
        name = name,
        help = help or "",
        labels = labels,
        label_set = label_set,
        buckets = buckets,
        collect = options.Collect,
        series = {},
    }, KINDS[kind])
    registry.metrics[name] = metric
    registry.order[#registry.order + 1] = name
    return metric
end

function Registry:Counter(name, help, options)
    return register(self, "counter", name, help, options)
end

function Registry:Gauge(name, help, options)
    return register(self, "gauge", name, help, options)
end

function Registry:Histogram(name, help, options)
    return register(self, "histogram", name, help, options)
end

function Registry:Get(name)
    return self.metrics[name]
end

function Registry:Unregister(name)
    if not self.metrics[name] then
        return false
    end
    self.metrics[name] = nil
    for index, other in ipairs(self.order) do
        if other == name then
            table.remove(self.order, index)
            break
        end
    end
    return true
end

function Registry:Clear()
    self.metrics = {}
    self.order = {}
end

local function collect(metric)
    local ok, result = pcall(metric.collect)
    if not ok then
        report("collecting the metric " .. metric.name .. " failed: " .. tostring(result))
        return
    end
    if type(result) == "number" then
        metric:Set(result)
    elseif type(result) == "table" then
        metric.series = {}
        for _, entry in ipairs(result) do
            local valid, message = pcall(metric.Set, metric, entry.Value, entry.Labels)
            if not valid then
                report("collecting the metric " .. metric.name .. " failed: " .. tostring(message))
            end
        end
    elseif result ~= nil then
        report("the collector of the metric " .. metric.name .. " returned a " .. type(result))
    end
end

function Registry:Render()
    local lines = {}
    local names = {}
    for index, name in ipairs(self.order) do
        names[index] = name
    end
    sort(names)

    for _, name in ipairs(names) do
        local metric = self.metrics[name]
        if metric.collect then
            collect(metric)
        end
        lines[#lines + 1] = "# HELP " .. name .. " " .. escape_help(metric.help)
        lines[#lines + 1] = "# TYPE " .. name .. " " .. metric.kind
        for _, entry in ipairs(sorted_series(metric)) do
            if metric.kind == "histogram" then
                for index, bound in ipairs(metric.buckets) do
                    lines[#lines + 1] = name .. "_bucket" .. label_text(metric, entry.key, "le=\"" .. format_number(bound) .. "\"") .. " " .. format_number(entry.counts[index])
                end
                lines[#lines + 1] = name .. "_bucket" .. label_text(metric, entry.key, "le=\"+Inf\"") .. " " .. format_number(entry.count)
                lines[#lines + 1] = name .. "_sum" .. label_text(metric, entry.key) .. " " .. format_number(entry.value)
                lines[#lines + 1] = name .. "_count" .. label_text(metric, entry.key) .. " " .. format_number(entry.count)
            else
                lines[#lines + 1] = name .. label_text(metric, entry.key) .. " " .. format_number(entry.value)
            end
        end
    end
    if #lines == 0 then
        return ""
    end
    return concat(lines, "\n") .. "\n"
end

function Registry:Expose(server, path)
    if type(server) ~= "userdata" and type(server) ~= "table" then
        error("Expose needs an HTTP server from Quark.HTTP.Listen", 2)
    end
    path = path or "/metrics"
    server:Get(path, function(_, response)
        response:Send(self:Render(), "text/plain; version=0.0.4; charset=utf-8")
    end)
    return server
end

local function create_registry()
    return setmetatable({ metrics = {}, order = {} }, Registry)
end

local default = create_registry()

local module = {
    Registry = create_registry,
    GetDefault = function()
        return default
    end,
}

for _, method in ipairs({ "Counter", "Gauge", "Histogram", "Get", "Unregister", "Clear", "Render", "Expose" }) do
    module[method] = function(...)
        return Registry[method](default, ...)
    end
end


return module
