local Test = Quark.Import("Testing")
local Expect = Test.Expect
local Promise = Quark.Promise

Test.Describe("Promise", function()
    Test.It("chains values through Then", function()
        local result = Quark.Await(Promise.Resolve(2)
            :Then(function(value) return value * 3 end)
            :Then(function(value) return Promise.Resolve(value + 1) end))
        Expect(result):ToBe(7)
    end)

    Test.It("passes multiple values", function()
        local first, second = Quark.Await(Promise.New(function(resolve) resolve(1, 2) end))
        Expect(first):ToBe(1)
        Expect(second):ToBe(2)
    end)

    Test.It("turns errors into rejections", function()
        local reason = Quark.Await(Promise.New(function() error("broken", 0) end)
            :Catch(function(error_message) return "caught " .. error_message end))
        Expect(reason):ToBe("caught broken")

        local chained = Quark.Await(Promise.Resolve(1)
            :Then(function() error("inside then", 0) end)
            :Then(function() return "skipped" end)
            :Catch(function(error_message) return error_message end))
        Expect(chained):ToBe("inside then")
    end)

    Test.It("skips Catch handlers of fulfilled promises", function()
        local caught = Test.Spy()
        local value = Quark.Await(Promise.Resolve("value"):Catch(caught))
        Expect(value):ToBe("value")
        Expect(caught).Not:ToHaveBeenCalled()

        local fulfilled = Test.Spy()
        local reason = Quark.Await(Promise.Reject("reason"):Then(fulfilled):Catch(function(error_message) return error_message end))
        Expect(reason):ToBe("reason")
        Expect(fulfilled).Not:ToHaveBeenCalled()
    end)

    Test.It("runs Finally for both outcomes", function()
        local calls = 0
        Quark.Await(Promise.Resolve(1):Finally(function() calls = calls + 1 end))
        pcall(Quark.Await, Promise.Reject("no"):Finally(function() calls = calls + 1 end))
        Expect(calls):ToBe(2)
    end)

    Test.It("combines promises", function()
        local all = Quark.Await(Promise.All({ Promise.Resolve(1), 2, Quark.Delay(10):Then(function() return 3 end) }))
        Expect(all):ToEqual({ 1, 2, 3 })

        local settled = Quark.Await(Promise.AllSettled({ Promise.Resolve("a"), Promise.Reject("b") }))
        Expect(settled):ToEqual({ { Status = "fulfilled", Value = "a" }, { Status = "rejected", Reason = "b" } })

        local winner = Quark.Await(Promise.Race({ Quark.Delay(50):Then(function() return "slow" end), Promise.Resolve("fast") }))
        Expect(winner):ToBe("fast")

        Expect(function() Quark.Await(Promise.All({ Promise.Resolve(1), Promise.Reject("failed") })) end):ToThrow("failed")
    end)

    Test.It("wraps callback functions", function()
        local hash = Quark.Await(Promise.From(Quark.Crypto.BcryptHash, "password", { Cost = 4 }))
        Expect(hash):ToMatch("^%$2b%$04%$")
        Expect(Quark.Await(Quark.Crypto.PasswordVerify, "password", hash)):ToBe(true)
        Expect(function() Quark.Await(Quark.Crypto.PasswordVerify, "password", "garbage") end):ToThrow("unknown password hash format")
    end)

    Test.It("waits with Delay", function()
        local started = Quark.Clock()
        Quark.Await(Quark.Delay(40))
        Expect(Quark.Clock() - started):ToBeGreaterThanOrEqual(0.035)
    end)

    Test.It("resolves Async with the return values", function()
        local value = Quark.Await(Quark.Async(function(input)
            Quark.Await(Quark.Delay(1))
            return input * 2
        end, 21))
        Expect(value):ToBe(42)
        Expect(function() Quark.Await(Quark.Async(function() error("async failure", 0) end)) end):ToThrow("async failure")
    end)

    Test.It("refuses Await outside of Async", function()
        local thread = coroutine.create(function() return Quark.Await(Quark.Delay(1)) end)
        local ok, reason = coroutine.resume(thread)
        Expect(ok):ToBe(false)
        Expect(reason):ToContain("can only be used inside Quark.Async")
    end)

    Test.It("reports unhandled rejections", function()
        local report = Test.Mock(Console, "Error")
        Promise.Reject("nobody listens")
        Quark.Await(Quark.Delay(50))
        Expect(report):ToHaveBeenCalledWith("[Quark] Unhandled promise rejection: nobody listens")
    end)

    Test.It("does not report handled rejections", function()
        local report = Test.Mock(Console, "Error")
        Promise.Reject("handled later"):Catch(function() end)
        Quark.Await(Quark.Delay(50))
        Expect(report).Not:ToHaveBeenCalled()
    end)
end)
