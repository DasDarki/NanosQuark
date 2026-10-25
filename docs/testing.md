# Testing

```lua
local Test = Quark.Import("Testing")
local Expect = Test.Expect

Test.Describe("Inventory", function()
    Test.It("stacks items", function()
        local inventory = Inventory.New()
        inventory:Add("wood", 3)
        inventory:Add("wood", 2)
        Expect(inventory:Count("wood")):ToBe(5)
    end)

    Test.It("saves to the database", function()
        local saved = Quark.Await(inventory_repository.Save, inventory)
        Expect(saved).Not:ToBeNil()
    end)
end)

Test.Run()
```

Every test runs in its own `Quark.Async` coroutine, so `Quark.Await` works directly inside tests and hooks. Tests are registered per package, several packages can run their own tests at the same time.

## Defining tests

| Function | |
| --- | --- |
| `Test.Describe(name, body)` | group tests, groups can be nested |
| `Test.It(name, body, options?)` (alias `Test.Test`) | define a test, option `Timeout` in milliseconds (default 5000) |
| `Test.Only(name, body, options?)` / `Test.DescribeOnly(name, body)` | run only these tests while any `Only` exists |
| `Test.Skip(name, body?)` / `Test.DescribeSkip(name, body)` | skip and report as skipped |
| `Test.BeforeAll(fn)`, `Test.AfterAll(fn)` | run once around the tests of the current group |
| `Test.BeforeEach(fn)`, `Test.AfterEach(fn)` | run around every test of the current group and its subgroups |
| `Test.Fail(message?)` | fail the current test |

## Running

`Test.Run(options?)` runs all tests registered by the calling package and returns a promise resolving with a summary `{ Passed, Failed, Skipped, Duration, Failures = { { Name, Message } } }`. Options:

| Option | |
| --- | --- |
| `Filter` | only run tests whose full name (`Group > Subgroup > Test`) contains this text |
| `Timeout` | default timeout for tests and hooks in milliseconds |
| `Silent` | only log failures and the summary |

Results are logged with `Console.Log`, failures with `Console.Error`:

```
[Testing] Inventory
[Testing]   PASS stacks items (0.04 ms)
[Testing]   FAIL saves to the database (2.10 ms): [inventory/Server/Tests.lua]:12: expected nil not to be nil
[Testing] 1 passed, 1 failed, 0 skipped (2.31 ms)
```

## Expectations

`Expect(value)` returns an expectation. `Expect(value).Not` negates every matcher.

| Matcher | Passes when |
| --- | --- |
| `:ToBe(expected)` | `value == expected` |
| `:ToEqual(expected)` | the values are deeply equal, tables compared key by key |
| `:ToBeNil()`, `:ToBeTruthy()`, `:ToBeFalsy()` | as named |
| `:ToBeType(name)` | `type(value)` matches, `"integer"` and `"float"` check `math.type` |
| `:ToBeGreaterThan(n)`, `:ToBeGreaterThanOrEqual(n)`, `:ToBeLessThan(n)`, `:ToBeLessThanOrEqual(n)` | numeric comparison |
| `:ToBeCloseTo(expected, tolerance?)` | the difference is at most `tolerance` (default `1e-9`) |
| `:ToContain(item)` | a string contains the text, or a table contains a deeply equal value |
| `:ToMatch(pattern)` | the string matches the Lua pattern |
| `:ToHaveLength(n)` | `#value == n` for strings, tables and buffers |
| `:ToThrow(fragment?)` | the function raises an error, containing `fragment` if given |
| `:ToHaveBeenCalled()`, `:ToHaveBeenCalledTimes(n)`, `:ToHaveBeenCalledWith(...)` | spy calls |

`Test.Equal(a, b)` exposes the deep comparison, `Test.Show(value)` the formatting used in failure messages.

## Spies and mocks

| Function | |
| --- | --- |
| `Test.Spy(implementation?)` | a function recording its calls, optionally forwarding to `implementation` |
| `Test.Mock(table, key, implementation?)` | replaces `table[key]` with a spy, restored after the current test |
| `Test.GetCalls(spy)` | list of calls, every call is a packed argument list |
| `Test.ResetSpy(spy)` | forget recorded calls |
| `Test.SetImplementation(spy, implementation)` | change what the spy does |

Spies are plain functions, so they can be passed anywhere a callback is expected.

```lua
Test.It("logs the greeting", function()
    local log = Test.Mock(Console, "Log")
    Greet("Alice")
    Expect(log):ToHaveBeenCalledWith("Hello Alice")
end)
```

## Running tests without a server

`tools/test-host/run.sh [package]` runs a package outside of nanos world on Linux. It builds Quark, compiles a small host that embeds the SDK's Lua, emulates the parts of the server API Quark relies on (`Server.Subscribe("Tick")`, `Package`, `Console`, one environment per package) and ticks every 16 ms. The package signals completion through the host only global `QuarkTestHost`:

```lua
Test.Run():Then(function(summary)
    if QuarkTestHost then
        QuarkTestHost.Finish(summary.Failed)
    end
end)
```

The exit code is 0 when all tests passed, which makes it suitable for CI. The default package is `packages/quark-test`, the test suite of Quark itself. Everything else of the nanos world API is missing in the host, so tests of gameplay code still need the real server.
