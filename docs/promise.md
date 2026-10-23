# Promises and async / await

Quark offers three styles for asynchronous work. All of them run the work on native worker threads and deliver the result on the game thread.

```lua
Crypto.Argon2Hash(password, function(hash, error) end)          -- callback

Quark.Promise.From(Crypto.Argon2Hash, password)                 -- promise
    :Then(function(hash) end)
    :Catch(function(error) end)

Quark.Async(function()                                          -- async / await
    local hash = Quark.Await(Crypto.Argon2Hash, password)
end)
```

## Async and Await

`Quark.Async(body, ...)` runs `body` in a coroutine and returns a promise that resolves with the return values of `body` or rejects with its error. Inside, `Quark.Await` pauses the coroutine until a result is available, without blocking the server:

| Call | Awaits |
| --- | --- |
| `Quark.Await(promise)` | a promise |
| `Quark.Await(callback_function, ...)` | any Quark function taking a callback as last argument, the callback is appended automatically |

`Await` returns the result values. When the work fails it raises the error, so use `pcall` to handle it locally or let it reject the promise of `Async`:

```lua
Quark.Async(function()
    local ok, valid = pcall(Quark.Await, Crypto.PasswordVerify, input, stored_hash)
    if not ok then
        Console.Error("could not verify: " .. valid)
        return
    end
    Quark.Await(Quark.Delay(500))
    Console.Log(valid and "welcome" or "wrong password")
end)
```

`Await` raises an error outside of `Quark.Async`.

## Promise

```lua
local Promise = Quark.Import("Promise")
```

| Function | |
| --- | --- |
| `Promise.New(function(resolve, reject) end)` | new promise, errors in the executor reject it |
| `Promise.Resolve(...)` / `Promise.Reject(reason)` | settled promise |
| `Promise.From(callback_function, ...)` | wraps a callback style function, `callback(nil, error)` rejects |
| `Promise.All(list)` | resolves with a list of the first value of every promise, rejects with the first rejection |
| `Promise.AllSettled(list)` | resolves with `{ Status = "fulfilled", Value = ... }` or `{ Status = "rejected", Reason = ... }` entries |
| `Promise.Race(list)` | settles like the first promise that settles |
| `Promise.IsPromise(value)` | whether the value is a promise |
| `Quark.Delay(milliseconds)` | promise resolving after the delay |

| Method | |
| --- | --- |
| `:Then(on_fulfilled?, on_rejected?)` | new promise with the handler result, returned promises are adopted |
| `:Cancel(reason?)` | rejects a pending promise with `reason` (`"cancelled"`), for promises from `Promise.From` the native task is cancelled too. Returns whether the promise was pending |
| `:Catch(on_rejected)` | shorthand for `:Then(nil, on_rejected)` |
| `:Finally(callback)` | runs on both outcomes, keeps the result |
| `:GetState()` | `"pending"`, `"fulfilled"` or `"rejected"` |

Handlers run immediately when a promise settles, there is no extra queue. A rejection without any handler is reported through `Console.Error` one tick later.

## Utilities

| Function | |
| --- | --- |
| `Quark.Clock()` | monotonic time in seconds with sub millisecond precision, for measuring durations |
