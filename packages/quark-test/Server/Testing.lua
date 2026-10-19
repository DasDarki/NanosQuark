local Test = Quark.Import("Testing")
local Expect = Test.Expect

local order = {}

Test.Describe("Testing", function()
    Test.BeforeAll(function() order[#order + 1] = "before all" end)
    Test.BeforeEach(function() order[#order + 1] = "before each" end)
    Test.AfterEach(function() order[#order + 1] = "after each" end)

    Test.It("runs hooks in order", function()
        Expect(order):ToEqual({ "before all", "before each" })
    end)

    Test.It("compares deeply", function()
        local cycle = {}
        cycle.self = cycle
        Expect(Test.Equal({ 1, { 2, 3 } }, { 1, { 2, 3 } })):ToBe(true)
        Expect(Test.Equal({ a = 1 }, { a = 1, b = 2 })):ToBe(false)
        Expect(Test.Equal(cycle, cycle)):ToBe(true)
    end)

    Test.It("negates matchers", function()
        Expect(1).Not:ToBe(2)
        Expect({ 1 }).Not:ToEqual({ 2 })
        Expect("text").Not:ToContain("other")
        Expect(function() end).Not:ToThrow()
    end)

    Test.It("explains failures", function()
        Expect(function() Expect(1):ToBe(2) end):ToThrow("expected 1 to be 2")
        Expect(function() Expect({ a = 1 }):ToEqual({ a = 2 }) end):ToThrow("expected { a = 1 } to equal { a = 2 }")
        Expect(function() Expect("abc"):ToMatch("^z") end):ToThrow("to match the pattern")
        Expect(function() Expect(function() end):ToThrow() end):ToThrow("it did not throw")
        Expect(function() Test.Fail("custom") end):ToThrow("custom")
    end)

    Test.It("records spy calls", function()
        local spy = Test.Spy(function(value) return value end)
        Expect(spy(1)):ToBe(1)
        spy("a", nil)
        Expect(spy):ToHaveBeenCalledTimes(2)
        Expect(spy):ToHaveBeenCalledWith("a", nil)
        Expect(#Test.GetCalls(spy)):ToBe(2)
        Test.ResetSpy(spy)
        Expect(spy).Not:ToHaveBeenCalled()
    end)

    Test.It("restores mocks after each test", function()
        local target = { Value = function() return "real" end }
        Test.Mock(target, "Value", function() return "fake" end)
        Expect(target.Value()):ToBe("fake")
        _G.QuarkTestMockTarget = target
    end)

    Test.It("sees restored mocks", function()
        Expect(_G.QuarkTestMockTarget.Value()):ToBe("real")
        _G.QuarkTestMockTarget = nil
    end)

    Test.It("supports async tests", function()
        Expect(Quark.Await(Quark.Async(function() return "ok" end))):ToBe("ok")
    end, { Timeout = 1000 })

    Test.Skip("skips tests", function() error("never runs") end)
end)
