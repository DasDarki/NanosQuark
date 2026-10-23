local defer, report, native_delay, capture, enter = ...

local coroutine_create = coroutine.create
local coroutine_resume = coroutine.resume
local coroutine_running = coroutine.running
local coroutine_yield = coroutine.yield
local error = error
local getmetatable = getmetatable
local pack = table.pack
local pcall = pcall
local setmetatable = setmetatable
local tostring = tostring
local type = type
local unpack = table.unpack

local PENDING, FULFILLED, REJECTED = "pending", "fulfilled", "rejected"

local Promise = {}
Promise.__index = Promise

local async_threads = setmetatable({}, { __mode = "k" })

local function resume(thread)
    local previous = enter(async_threads[thread] or nil)
    local ok, reason = coroutine_resume(thread)
    enter(previous)
    return ok, reason
end

local function is_promise(value)
    return type(value) == "table" and getmetatable(value) == Promise
end

local function create()
    return setmetatable({ state = PENDING, handlers = {}, handled = false }, Promise)
end

local function schedule_rejection_check(promise)
    local function check()
        if not promise.handled then
            report("Unhandled promise rejection: " .. tostring(promise.values[1]))
        end
    end

    if not pcall(defer, check) then
        check()
    end
end

local function settle(promise, state, values)
    if promise.state ~= PENDING then
        return
    end

    promise.state = state
    promise.values = values
    local handlers = promise.handlers
    promise.handlers = nil

    if state == REJECTED and #handlers == 0 then
        schedule_rejection_check(promise)
    end

    for index = 1, #handlers do
        handlers[index]()
    end
end

local function resolve_values(promise, values, first)
    local value = values[first]
    if values.n >= first and is_promise(value) then
        value:Then(function(...)
            settle(promise, FULFILLED, pack(...))
        end, function(reason)
            settle(promise, REJECTED, pack(reason))
        end)
    else
        settle(promise, FULFILLED, pack(unpack(values, first, values.n)))
    end
end

function Promise.New(executor)
    local promise = create()

    local function resolve(...)
        resolve_values(promise, pack(...), 1)
    end

    local function reject(reason)
        settle(promise, REJECTED, pack(reason))
    end

    local ok, reason = pcall(executor, resolve, reject)
    if not ok then
        reject(reason)
    end

    return promise
end

function Promise.Resolve(...)
    local values = pack(...)
    if values.n == 1 and is_promise(values[1]) then
        return values[1]
    end
    return Promise.New(function(resolve)
        resolve(unpack(values, 1, values.n))
    end)
end

function Promise.Reject(reason)
    return Promise.New(function(_, reject)
        reject(reason)
    end)
end

function Promise.IsPromise(value)
    return is_promise(value)
end

function Promise:Then(on_fulfilled, on_rejected)
    local next_promise = create()

    local function run()
        local handler = on_rejected
        if self.state == FULFILLED then
            handler = on_fulfilled
        end
        if handler == nil then
            settle(next_promise, self.state, self.values)
            return
        end

        local results = pack(pcall(handler, unpack(self.values, 1, self.values.n)))
        if results[1] then
            resolve_values(next_promise, results, 2)
        else
            settle(next_promise, REJECTED, pack(results[2]))
        end
    end

    self.handled = true
    if self.state == PENDING then
        self.handlers[#self.handlers + 1] = run
    else
        run()
    end

    return next_promise
end

function Promise:Catch(on_rejected)
    return self:Then(nil, on_rejected)
end

function Promise:Finally(callback)
    return self:Then(function(...)
        callback()
        return ...
    end, function(reason)
        callback()
        error(reason, 0)
    end)
end

function Promise:GetState()
    return self.state
end

function Promise:__tostring()
    return "Promise(" .. self.state .. ")"
end

function Promise.All(promises)
    return Promise.New(function(resolve, reject)
        local count = #promises
        local results = {}
        local remaining = count

        if count == 0 then
            resolve(results)
            return
        end

        for index = 1, count do
            Promise.Resolve(promises[index]):Then(function(value)
                results[index] = value
                remaining = remaining - 1
                if remaining == 0 then
                    resolve(results)
                end
            end, reject)
        end
    end)
end

function Promise.AllSettled(promises)
    return Promise.New(function(resolve)
        local count = #promises
        local results = {}
        local remaining = count

        if count == 0 then
            resolve(results)
            return
        end

        local function done(index, result)
            results[index] = result
            remaining = remaining - 1
            if remaining == 0 then
                resolve(results)
            end
        end

        for index = 1, count do
            Promise.Resolve(promises[index]):Then(function(value)
                done(index, { Status = FULFILLED, Value = value })
            end, function(reason)
                done(index, { Status = REJECTED, Reason = reason })
            end)
        end
    end)
end

function Promise.Race(promises)
    return Promise.New(function(resolve, reject)
        for index = 1, #promises do
            Promise.Resolve(promises[index]):Then(resolve, reject)
        end
    end)
end

function Promise.From(callback_function, ...)
    local arguments = pack(...)
    local task
    local promise = Promise.New(function(resolve, reject)
        arguments[arguments.n + 1] = function(...)
            local first, second = ...
            if first == nil and second ~= nil then
                reject(second)
            else
                resolve(...)
            end
        end
        task = callback_function(unpack(arguments, 1, arguments.n + 1))
    end)
    if type(task) == "userdata" and promise.state == PENDING then
        promise.task = task
    end
    return promise
end

function Promise:Cancel(reason)
    if self.state ~= PENDING then
        return false
    end
    local task = self.task
    self.task = nil
    if task then
        pcall(task.Cancel, task)
    end
    self.handled = true
    settle(self, REJECTED, pack(reason or "cancelled"))
    return true
end

function Promise.Delay(milliseconds)
    return Promise.New(function(resolve)
        native_delay(milliseconds, function()
            resolve()
        end)
    end)
end

function Promise.Async(body, ...)
    local arguments = pack(...)
    local environment = capture() or false
    return Promise.New(function(resolve, reject)
        local thread = coroutine_create(function()
            local results = pack(pcall(body, unpack(arguments, 1, arguments.n)))
            if results[1] then
                resolve(unpack(results, 2, results.n))
            else
                reject(results[2])
            end
        end)

        async_threads[thread] = environment
        local ok, reason = resume(thread)
        if not ok then
            reject(reason)
        end
    end)
end

function Promise.Await(target, ...)
    local promise = is_promise(target) and target or Promise.From(target, ...)

    if promise.state == PENDING then
        local thread = coroutine_running()
        if async_threads[thread] == nil then
            error("Quark.Await can only be used inside Quark.Async", 2)
        end

        local function continue()
            local ok, reason = resume(thread)
            if not ok then
                report("Quark.Async failed to resume: " .. tostring(reason))
            end
        end

        promise:Then(continue, continue)
        coroutine_yield()
    end

    promise.handled = true
    if promise.state == FULFILLED then
        return unpack(promise.values, 1, promise.values.n)
    end
    error(promise.values[1], 0)
end

return {
    New = Promise.New,
    Resolve = Promise.Resolve,
    Reject = Promise.Reject,
    IsPromise = Promise.IsPromise,
    All = Promise.All,
    AllSettled = Promise.AllSettled,
    Race = Promise.Race,
    From = Promise.From,
    Delay = Promise.Delay,
    Async = Promise.Async,
    Await = Promise.Await,
}
