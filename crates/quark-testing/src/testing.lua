local Promise, package_environment, log, log_error, clock = ...

local error = error
local getmetatable = getmetatable
local ipairs = ipairs
local math_abs = math.abs
local math_type = math.type
local pack = table.pack
local pairs = pairs
local pcall = pcall
local rawequal = rawequal
local setmetatable = setmetatable
local string_find = string.find
local string_format = string.format
local string_rep = string.rep
local table_concat = table.concat
local table_sort = table.sort
local tostring = tostring
local type = type
local unpack = table.unpack

local DEFAULT_TIMEOUT = 5000
local MAX_SHOWN_LENGTH = 200

local Testing = {}

local states = setmetatable({}, { __mode = "k" })
local fallback_environment = {}
local spies = setmetatable({}, { __mode = "k" })
local active_mocks = {}

local function new_suite(name, parent)
    return {
        name = name,
        parent = parent,
        items = {},
        before_all = {},
        after_all = {},
        before_each = {},
        after_each = {},
        only = false,
        skip = false,
    }
end

local function state()
    local environment = package_environment() or fallback_environment
    local current = states[environment]
    if not current then
        local root = new_suite(nil, nil)
        current = { root = root, current = root }
        states[environment] = current
    end
    return current
end

local function show(value, depth)
    depth = depth or 0
    local kind = type(value)

    if kind == "string" then
        local shown = string_format("%q", value)
        if #shown > MAX_SHOWN_LENGTH then
            shown = shown:sub(1, MAX_SHOWN_LENGTH) .. "...\""
        end
        return shown
    end

    if kind ~= "table" or depth > 1 or (getmetatable(value) and getmetatable(value).__tostring) then
        return tostring(value)
    end

    local keys = {}
    for key in pairs(value) do
        keys[#keys + 1] = key
    end
    table_sort(keys, function(left, right)
        return tostring(left) < tostring(right)
    end)

    local parts = {}
    for index, key in ipairs(keys) do
        if index > 10 then
            parts[#parts + 1] = "..."
            break
        end
        local shown_key = type(key) == "string" and key or "[" .. show(key, depth + 1) .. "]"
        parts[#parts + 1] = shown_key .. " = " .. show(value[key], depth + 1)
    end

    local shown = "{ " .. table_concat(parts, ", ") .. " }"
    if #shown > MAX_SHOWN_LENGTH then
        shown = shown:sub(1, MAX_SHOWN_LENGTH) .. "... }"
    end
    return shown
end

local function deep_equal(left, right, seen)
    if rawequal(left, right) then
        return true
    end
    if type(left) ~= "table" or type(right) ~= "table" then
        return left == right
    end

    seen = seen or {}
    if seen[left] == right then
        return true
    end
    seen[left] = right

    for key, value in pairs(left) do
        if not deep_equal(value, right[key], seen) then
            return false
        end
    end
    for key in pairs(right) do
        if left[key] == nil then
            return false
        end
    end
    return true
end

local function count_entries(value)
    if type(value) == "string" then
        return #value
    end
    if type(value) == "table" then
        return #value
    end
    local metatable = getmetatable(value)
    if metatable and metatable.__len then
        return #value
    end
    return nil
end

local Expectation = {}
Expectation.__index = Expectation

local function create_expectation(value, negated)
    return setmetatable({ value = value, negated = negated }, Expectation)
end

function Testing.Expect(value)
    local expectation = create_expectation(value, false)
    expectation.Not = create_expectation(value, true)
    return expectation
end

local function check(expectation, passed, message, negated_message)
    if expectation.negated then
        passed = not passed
        message = negated_message
    end
    if not passed then
        error(message, 3)
    end
end

function Expectation:ToBe(expected)
    check(self, self.value == expected,
        "expected " .. show(self.value) .. " to be " .. show(expected),
        "expected " .. show(self.value) .. " not to be " .. show(expected))
end

function Expectation:ToEqual(expected)
    check(self, deep_equal(self.value, expected),
        "expected " .. show(self.value) .. " to equal " .. show(expected),
        "expected " .. show(self.value) .. " not to equal " .. show(expected))
end

function Expectation:ToBeNil()
    check(self, self.value == nil,
        "expected " .. show(self.value) .. " to be nil",
        "expected the value not to be nil")
end

function Expectation:ToBeTruthy()
    check(self, not not self.value,
        "expected " .. show(self.value) .. " to be truthy",
        "expected " .. show(self.value) .. " to be falsy")
end

function Expectation:ToBeFalsy()
    check(self, not self.value,
        "expected " .. show(self.value) .. " to be falsy",
        "expected " .. show(self.value) .. " to be truthy")
end

function Expectation:ToBeType(expected)
    local actual = type(self.value)
    if expected == "integer" or expected == "float" then
        actual = math_type(self.value) or actual
    end
    check(self, actual == expected,
        "expected " .. show(self.value) .. " to be of type " .. expected .. ", got " .. actual,
        "expected " .. show(self.value) .. " not to be of type " .. expected)
end

function Expectation:ToBeGreaterThan(expected)
    check(self, self.value > expected,
        "expected " .. show(self.value) .. " to be greater than " .. show(expected),
        "expected " .. show(self.value) .. " not to be greater than " .. show(expected))
end

function Expectation:ToBeGreaterThanOrEqual(expected)
    check(self, self.value >= expected,
        "expected " .. show(self.value) .. " to be greater than or equal to " .. show(expected),
        "expected " .. show(self.value) .. " to be less than " .. show(expected))
end

function Expectation:ToBeLessThan(expected)
    check(self, self.value < expected,
        "expected " .. show(self.value) .. " to be less than " .. show(expected),
        "expected " .. show(self.value) .. " not to be less than " .. show(expected))
end

function Expectation:ToBeLessThanOrEqual(expected)
    check(self, self.value <= expected,
        "expected " .. show(self.value) .. " to be less than or equal to " .. show(expected),
        "expected " .. show(self.value) .. " to be greater than " .. show(expected))
end

function Expectation:ToBeCloseTo(expected, tolerance)
    tolerance = tolerance or 1e-9
    check(self, type(self.value) == "number" and math_abs(self.value - expected) <= tolerance,
        "expected " .. show(self.value) .. " to be within " .. tolerance .. " of " .. show(expected),
        "expected " .. show(self.value) .. " not to be within " .. tolerance .. " of " .. show(expected))
end

function Expectation:ToContain(expected)
    local found = false
    if type(self.value) == "string" then
        found = string_find(self.value, tostring(expected), 1, true) ~= nil
    elseif type(self.value) == "table" then
        for _, value in pairs(self.value) do
            if deep_equal(value, expected) then
                found = true
                break
            end
        end
    end
    check(self, found,
        "expected " .. show(self.value) .. " to contain " .. show(expected),
        "expected " .. show(self.value) .. " not to contain " .. show(expected))
end

function Expectation:ToMatch(pattern)
    check(self, type(self.value) == "string" and string_find(self.value, pattern) ~= nil,
        "expected " .. show(self.value) .. " to match the pattern " .. show(pattern),
        "expected " .. show(self.value) .. " not to match the pattern " .. show(pattern))
end

function Expectation:ToHaveLength(expected)
    local length = count_entries(self.value)
    check(self, length == expected,
        "expected " .. show(self.value) .. " to have length " .. expected .. ", got " .. tostring(length),
        "expected " .. show(self.value) .. " not to have length " .. expected)
end

function Expectation:ToThrow(fragment)
    if type(self.value) ~= "function" then
        error("ToThrow expects a function, got " .. type(self.value), 2)
    end

    local ok, reason = pcall(self.value)
    local message = tostring(reason)
    local matches = not ok and (fragment == nil or string_find(message, fragment, 1, true) ~= nil)

    local expected = fragment and ("an error containing " .. show(fragment)) or "an error"
    local actual = ok and "it did not throw" or ("it threw " .. show(message))
    check(self, matches,
        "expected " .. expected .. ", but " .. actual,
        "expected no " .. (fragment and ("error containing " .. show(fragment)) or "error") .. ", but " .. actual)
end

local function spy_record(value)
    local record = spies[value]
    if not record then
        error("expected a spy created with Testing.Spy or Testing.Mock, got " .. show(value), 3)
    end
    return record
end

function Expectation:ToHaveBeenCalled()
    local record = spy_record(self.value)
    check(self, #record.Calls > 0,
        "expected the spy to have been called",
        "expected the spy not to have been called, it was called " .. #record.Calls .. " times")
end

function Expectation:ToHaveBeenCalledTimes(expected)
    local record = spy_record(self.value)
    check(self, #record.Calls == expected,
        "expected the spy to have been called " .. expected .. " times, it was called " .. #record.Calls .. " times",
        "expected the spy not to have been called " .. expected .. " times")
end

function Expectation:ToHaveBeenCalledWith(...)
    local record = spy_record(self.value)
    local expected = pack(...)
    local found = false
    for _, call in ipairs(record.Calls) do
        if call.n == expected.n and deep_equal(call, expected) then
            found = true
            break
        end
    end
    local shown = {}
    for index = 1, expected.n do
        shown[index] = show(expected[index])
    end
    check(self, found,
        "expected the spy to have been called with (" .. table_concat(shown, ", ") .. ")",
        "expected the spy not to have been called with (" .. table_concat(shown, ", ") .. ")")
end

function Testing.Fail(message)
    error(message or "the test failed", 2)
end

function Testing.Spy(implementation)
    local record = { Calls = {}, implementation = implementation }
    local function spy(...)
        record.Calls[#record.Calls + 1] = pack(...)
        if record.implementation then
            return record.implementation(...)
        end
    end
    spies[spy] = record
    return spy
end

function Testing.GetCalls(spy)
    return spy_record(spy).Calls
end

function Testing.ResetSpy(spy)
    spy_record(spy).Calls = {}
end

function Testing.SetImplementation(spy, implementation)
    spy_record(spy).implementation = implementation
end

function Testing.Mock(target, key, implementation)
    if type(target) ~= "table" then
        error("Testing.Mock expects a table, got " .. type(target), 2)
    end
    local original = target[key]
    local spy = Testing.Spy(implementation)
    target[key] = spy
    active_mocks[#active_mocks + 1] = { target = target, key = key, original = original }
    return spy
end

local function restore_mocks()
    for index = #active_mocks, 1, -1 do
        local mock = active_mocks[index]
        mock.target[mock.key] = mock.original
        active_mocks[index] = nil
    end
end

local function add_item(item)
    local current = state().current
    current.items[#current.items + 1] = item
end

local function describe(name, body, only, skip)
    local current_state = state()
    local suite = new_suite(name, current_state.current)
    suite.only = only
    suite.skip = skip
    add_item({ kind = "suite", suite = suite })

    local previous = current_state.current
    current_state.current = suite
    local ok, reason = pcall(body)
    current_state.current = previous

    if not ok then
        suite.items[#suite.items + 1] = {
            kind = "test",
            name = "suite definition",
            body = function()
                error(reason, 0)
            end,
        }
    end
end

function Testing.Describe(name, body)
    describe(name, body, false, false)
end

function Testing.DescribeOnly(name, body)
    describe(name, body, true, false)
end

function Testing.DescribeSkip(name, body)
    describe(name, body, false, true)
end

local function test(name, body, options, only, skip)
    add_item({
        kind = "test",
        name = name,
        body = body,
        timeout = options and options.Timeout,
        only = only,
        skip = skip or body == nil,
    })
end

function Testing.It(name, body, options)
    test(name, body, options, false, false)
end

function Testing.Only(name, body, options)
    test(name, body, options, true, false)
end

function Testing.Skip(name, body, options)
    test(name, body, options, false, true)
end

Testing.Test = Testing.It

for _, hook in ipairs({ "BeforeAll", "AfterAll", "BeforeEach", "AfterEach" }) do
    local field = hook:gsub("(%l)(%u)", "%1_%2"):lower()
    Testing[hook] = function(callback)
        local current = state().current
        current[field][#current[field] + 1] = callback
    end
end

local function contains_only(suite)
    for _, item in ipairs(suite.items) do
        if item.kind == "test" and item.only then
            return true
        end
        if item.kind == "suite" and (item.suite.only or contains_only(item.suite)) then
            return true
        end
    end
    return false
end

local function run_body(body, timeout)
    local task = Promise.Async(body)
    local timer = Promise.Delay(timeout):Then(function()
        error("timed out after " .. timeout .. " ms", 0)
    end)
    return pcall(Promise.Await, Promise.Race({ task, timer }))
end

local function run_hooks(hooks, timeout)
    for _, hook in ipairs(hooks) do
        local ok, reason = run_body(hook, timeout)
        if not ok then
            return false, reason
        end
    end
    return true
end

local function each_hooks(suite, field, outer_first)
    local chain = {}
    local current = suite
    while current do
        chain[#chain + 1] = current
        current = current.parent
    end

    local hooks = {}
    local first, last, step = #chain, 1, -1
    if not outer_first then
        first, last, step = 1, #chain, 1
    end
    for index = first, last, step do
        for _, hook in ipairs(chain[index][field]) do
            hooks[#hooks + 1] = hook
        end
    end
    return hooks
end

local function run_suite(suite, context, depth, included, skipped)
    local indent = string_rep("  ", depth)
    included = included or suite.only
    skipped = skipped or suite.skip

    if suite.name then
        log(indent .. suite.name)
        depth = depth + 1
        indent = string_rep("  ", depth)
    end

    local setup_ok, setup_reason = true, nil
    if not skipped then
        setup_ok, setup_reason = run_hooks(suite.before_all, context.timeout)
    end

    for _, item in ipairs(suite.items) do
        if item.kind == "suite" then
            run_suite(item.suite, context, depth, included, skipped)
        else
            local name = item.name
            local full_name = (suite.name and (context.path(suite) .. " > ") or "") .. name
            local excluded = context.only and not (included or item.only)
            local filtered = context.filter and string_find(full_name, context.filter, 1, true) == nil

            if skipped or item.skip or excluded or filtered then
                context.summary.Skipped = context.summary.Skipped + 1
                if not context.silent and not excluded and not filtered then
                    log(indent .. "SKIP " .. name)
                end
            else
                local started = clock()
                local ok, reason = setup_ok, setup_reason

                if ok then
                    ok, reason = run_hooks(each_hooks(suite, "before_each", true), context.timeout)
                end
                if ok then
                    ok, reason = run_body(item.body, item.timeout or context.timeout)
                end
                local teardown_ok, teardown_reason = run_hooks(each_hooks(suite, "after_each", false), context.timeout)
                if ok and not teardown_ok then
                    ok, reason = false, teardown_reason
                end
                restore_mocks()

                local duration = string_format("%.2f ms", (clock() - started) * 1000)
                if ok then
                    context.summary.Passed = context.summary.Passed + 1
                    if not context.silent then
                        log(indent .. "PASS " .. name .. " (" .. duration .. ")")
                    end
                else
                    context.summary.Failed = context.summary.Failed + 1
                    local message = tostring(reason)
                    context.summary.Failures[#context.summary.Failures + 1] = { Name = full_name, Message = message }
                    log_error(indent .. "FAIL " .. name .. " (" .. duration .. "): " .. message)
                end
            end
        end
    end

    if not skipped then
        local ok, reason = run_hooks(suite.after_all, context.timeout)
        if not ok then
            context.summary.Failed = context.summary.Failed + 1
            context.summary.Failures[#context.summary.Failures + 1] = {
                Name = (suite.name or "root") .. " > AfterAll",
                Message = tostring(reason),
            }
            log_error(indent .. "FAIL AfterAll: " .. tostring(reason))
        end
    end
end

local function suite_path(suite)
    local names = {}
    local current = suite
    while current and current.name do
        names[#names + 1] = current.name
        current = current.parent
    end
    local path = {}
    for index = #names, 1, -1 do
        path[#path + 1] = names[index]
    end
    return table_concat(path, " > ")
end

function Testing.Run(options)
    options = options or {}
    local current_state = state()
    local root = current_state.root
    local fresh = new_suite(nil, nil)
    current_state.root = fresh
    current_state.current = fresh

    return Promise.Async(function()
        local summary = { Passed = 0, Failed = 0, Skipped = 0, Failures = {} }
        local context = {
            summary = summary,
            timeout = options.Timeout or DEFAULT_TIMEOUT,
            filter = options.Filter,
            silent = options.Silent,
            only = contains_only(root),
            path = suite_path,
        }

        local started = clock()
        run_suite(root, context, 0, false, false)
        summary.Duration = (clock() - started) * 1000

        local line = string_format("%d passed, %d failed, %d skipped (%.2f ms)",
            summary.Passed, summary.Failed, summary.Skipped, summary.Duration)
        if summary.Failed > 0 then
            log_error(line)
        else
            log(line)
        end
        return summary
    end)
end

function Testing.Equal(left, right)
    return deep_equal(left, right)
end

function Testing.Show(value)
    return show(value)
end


return Testing
