local Test = Quark.Import("Testing")
local Expect = Test.Expect

Test.Describe("Worker", function()
    local Worker = Quark.Worker

    Test.It("runs code in a sandbox and copies values", function()
        local result, text = Worker.Run([[
            local data = ...
            local sum = 0
            for _, value in ipairs(data.Values) do
                sum = sum + value
            end
            return { Sum = sum, Nested = { data.Name:upper() } }, "done"
        ]], { Values = { 1, 2, 3, 4 }, Name = "quark" })
        Expect(result.Sum):ToBe(10)
        Expect(result.Nested[1]):ToBe("QUARK")
        Expect(text):ToBe("done")
        local checks = { Worker.Run([[return io == nil, require == nil, debug == nil, os.execute == nil, type(os.time())]]) }
        Expect(checks):ToEqual({ true, true, true, true, "number" })
    end)

    Test.It("reports errors, limits and invalid values", function()
        local failed, message = Worker.Run([[error("broken")]])
        Expect(failed):ToBe(nil)
        Expect(message):ToContain("broken")
        local slow, timeout = Worker.Run([[while true do end]], nil, { Timeout = 100 })
        Expect(slow):ToBe(nil)
        Expect(timeout):ToContain("longer than 100 ms")
        local hungry, memory = Worker.Run([[local parts = {} for index = 1, 1e7 do parts[index] = ("x"):rep(64) .. index end]], nil, { MemoryLimit = 8 * 1024 * 1024 })
        Expect(hungry):ToBe(nil)
        Expect(memory):ToContain("memory")
        Expect(function() Worker.Run("return 1", { print }) end):ToThrow("only nil, booleans")
        local cyclic = {}
        cyclic.Self = cyclic
        Expect(function() Worker.Run("return 1", cyclic) end):ToThrow("cycle")
        local returned, invalid = Worker.Run([[return function() end]])
        Expect(returned):ToBe(nil)
        Expect(invalid):ToContain("only nil, booleans")
        local syntax, problem = Worker.Run([[return +]])
        Expect(syntax):ToBe(nil)
        Expect(problem):ToContain("worker")
    end)

    Test.It("runs in the background with a callback", function()
        local value = Quark.Await(Worker.Run, [[
            local count = 0
            for index = 1, 2000000 do count = count + index % 7 end
            return count
        ]], nil, nil)
        Expect(value):ToBe(5999997)
    end)

    Test.It("keeps code loaded in a pool", function()
        local pool = Worker.Pool([[
            local calls = 0
            function Square(value)
                calls = calls + 1
                return value * value
            end
            function Count()
                return calls
            end
        ]], { Size = 2 })
        Expect(pool:GetSize()):ToBe(2)
        Expect(Quark.Await(pool.Call, pool, "Square", 12)):ToBe(144)
        local results = {}
        local promises = {}
        for index = 1, 8 do
            promises[index] = Quark.Promise.New(function(resolve)
                pool:Call("Square", index, function(value) resolve(value) end)
            end)
        end
        for index = 1, 8 do
            results[index] = Quark.Await(promises[index])
        end
        Expect(results):ToEqual({ 1, 4, 9, 16, 25, 36, 49, 64 })
        local missing, message = true, nil
        Quark.Await(Quark.Promise.New(function(resolve)
            pool:Call("Unknown", function(value, error)
                missing, message = value, error
                resolve()
            end)
        end))
        Expect(missing):ToBe(nil)
        Expect(message):ToContain("no global function 'Unknown'")
        Expect(function() pool:Call("Square", 2) end):ToThrow("callback")
        pool:Close()
        Expect(function() pool:Call("Square", 2, function() end) end):ToThrow("closed")
        local broken, reason = Worker.Pool([[error("setup failed")]], { Size = 1 })
        Expect(broken):ToBe(nil)
        Expect(reason):ToContain("setup failed")
    end)
end)
