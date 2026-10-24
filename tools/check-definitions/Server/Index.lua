local LIBRARY = os.getenv("QUARK_LIBRARY") or "library"
local FILES = { "quark", "crypto", "formats", "compression", "net", "redis", "messaging", "random", "text", "time", "locale", "utilities", "testing", "math" }

local classes, variables, parents, functions, root_fields = {}, {}, {}, {}, {}

for _, name in ipairs(FILES) do
    local file = assert(io.open(LIBRARY .. "/" .. name .. ".lua", "r"))
    local pending
    for line in file:lines() do
        local class, parent = line:match("^%-%-%-@class ([%w_.]+):? ?([%w_.]*)")
        if class then
            pending = class
            parents[class] = parent ~= "" and parent or nil
        end
        local variable = line:match("^local ([%w_]+) = {}")
        if variable and pending then
            classes[pending] = variable
            variables[variable] = pending
            functions[pending] = functions[pending] or {}
            pending = nil
        end
        local field, kind = line:match("^%-%-%-@field ([%w_]+) (Quark[%w_]+)")
        if field and pending == "Quark" then
            root_fields[field] = kind
        end
        local owner, separator, method = line:match("^function ([%w_]+)([.:])([%w_]+)%(")
        if owner and variables[owner] then
            functions[variables[owner]][method] = separator
        end
    end
    file:close()
end

local function declared(class)
    local names = {}
    while class do
        for name in pairs(functions[class] or {}) do
            names[name] = true
        end
        class = parents[class]
    end
    return names
end

local problems = 0
local function compare(label, class, actual)
    local expected = declared(class)
    for name in pairs(actual) do
        if not expected[name] then
            print("missing in library: " .. label .. "." .. name)
            problems = problems + 1
        end
    end
    for name in pairs(expected) do
        if not actual[name] and not (functions.QuarkEmitter or {})[name] then
            print("not in Quark: " .. label .. "." .. name)
            problems = problems + 1
        end
    end
end

for _, module in ipairs(Quark.Modules()) do
    local class = root_fields[module]
    if not class then
        print("module without library field: " .. module)
        problems = problems + 1
    else
        local actual = {}
        for key, value in pairs(Quark.Import(module)) do
            if type(value) == "function" then
                actual[key] = true
            end
        end
        compare(module, class, actual)
    end
end

local function check_object(class, object)
    if type(object) == "userdata" then
        for name in pairs(declared(class)) do
            local ok, value = pcall(function() return object[name] end)
            if not ok or value == nil then
                print("not in Quark: " .. class .. ":" .. name)
                problems = problems + 1
            end
        end
        return
    end
    local actual = {}
    local metatable = getmetatable(object)
    local index = type(metatable) == "table" and metatable.__index
    for _, source in ipairs({ type(index) == "table" and index or {}, object }) do
        for key, value in pairs(source) do
            if type(value) == "function" and key:match("^%u") then
                actual[key] = true
            end
        end
    end
    compare(class, class, actual)
end

local buffer = Quark.Buffer.New(8)
local server = Quark.HTTP.Listen(0, { Host = "127.0.0.1" })
local objects = {
    QuarkBuffer = buffer,
    QuarkBufferView = buffer:View("u8"),
    QuarkRegex = Quark.Regex.New("a"),
    QuarkRandomGenerator = Quark.Random.New(1),
    QuarkWeightedPicker = Quark.Random.WeightedPicker({ a = 1 }),
    QuarkNoise = Quark.Random.Noise(),
    QuarkDateTime = Quark.Time.Now(),
    QuarkCron = Quark.Time.Cron("* * * * *"),
    QuarkCache = Quark.Cache.New(),
    QuarkPriorityQueue = Quark.PriorityQueue.New(),
    QuarkRingBuffer = Quark.RingBuffer.New(1),
    QuarkBitset = Quark.Bitset.New(1),
    QuarkLogger = Quark.Log.New({ Console = false }),
    QuarkWordFilter = Quark.Text.WordFilter({}),
    QuarkTemplates = Quark.Template.New(),
    QuarkLocale = Quark.Locale.New(),
    QuarkCompressionStream = Quark.Compression.Stream("gzip"),
    QuarkHasher = Quark.Crypto.Hasher("SHA256"),
    QuarkHTTPServer = server,
    QuarkMetricsRegistry = Quark.Metrics.Registry(),
    QuarkQuaternion = Quark.Math.Quaternion(),
    QuarkExpression = Quark.Math.Compile("1"),
    QuarkShape = Quark.Geometry.Sphere({ X = 0, Y = 0, Z = 0 }, 1),
    QuarkBigInt = Quark.BigInt.New(1),
    QuarkDecimal = Quark.Decimal.New(1),
}
for class, object in pairs(objects) do
    check_object(class, object)
end
server:Close()

print(problems == 0 and "definitions match" or (problems .. " problems"))
QuarkTestHost.Finish(problems)
