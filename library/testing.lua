---@meta

---Result of Test.Expect, Not inverts every matcher.
---@class QuarkExpectation
---@field Not QuarkExpectation
local Expectation = {}

---@param expected any
function Expectation:ToBe(expected) end

---@param expected any
function Expectation:ToEqual(expected) end

function Expectation:ToBeNil() end

function Expectation:ToBeTruthy() end

function Expectation:ToBeFalsy() end

---@param name string
function Expectation:ToBeType(name) end

---@param number number
function Expectation:ToBeGreaterThan(number) end

---@param number number
function Expectation:ToBeGreaterThanOrEqual(number) end

---@param number number
function Expectation:ToBeLessThan(number) end

---@param number number
function Expectation:ToBeLessThanOrEqual(number) end

---@param expected number
---@param tolerance? number
function Expectation:ToBeCloseTo(expected, tolerance) end

---@param item any
function Expectation:ToContain(item) end

---@param pattern string
function Expectation:ToMatch(pattern) end

---@param length integer
function Expectation:ToHaveLength(length) end

---@param fragment? string
function Expectation:ToThrow(fragment) end

function Expectation:ToHaveBeenCalled() end

---@param count integer
function Expectation:ToHaveBeenCalledTimes(count) end

---@param ... any
function Expectation:ToHaveBeenCalledWith(...) end

---@class QuarkTestingModule
local Test = {}

---@param name string
---@param body fun()
function Test.Describe(name, body) end

---@param name string
---@param body fun()
function Test.DescribeOnly(name, body) end

---@param name string
---@param body fun()
function Test.DescribeSkip(name, body) end

---@param name string
---@param body fun()
---@param options? { Timeout?: integer }
function Test.It(name, body, options) end

---@param name string
---@param body fun()
---@param options? { Timeout?: integer }
function Test.Test(name, body, options) end

---@param name string
---@param body fun()
---@param options? { Timeout?: integer }
function Test.Only(name, body, options) end

---@param name string
---@param body? fun()
---@param options? { Timeout?: integer }
function Test.Skip(name, body, options) end

---@param hook fun()
function Test.BeforeAll(hook) end

---@param hook fun()
function Test.AfterAll(hook) end

---@param hook fun()
function Test.BeforeEach(hook) end

---@param hook fun()
function Test.AfterEach(hook) end

---@param value any
---@return QuarkExpectation
function Test.Expect(value) end

---@param message? string
function Test.Fail(message) end

---@param implementation? function
---@return function
function Test.Spy(implementation) end

---@param target table
---@param key any
---@param implementation? function
---@return function
function Test.Mock(target, key, implementation) end

---@param spy function
---@return table[]
function Test.GetCalls(spy) end

---@param spy function
function Test.ResetSpy(spy) end

---@param spy function
---@param implementation function
function Test.SetImplementation(spy, implementation) end

---@param options? { Filter?: string, Timeout?: integer, Silent?: boolean }
---@return QuarkPromise
function Test.Run(options) end

---@param left any
---@param right any
---@return boolean
function Test.Equal(left, right) end

---@param value any
---@return string
function Test.Show(value) end
