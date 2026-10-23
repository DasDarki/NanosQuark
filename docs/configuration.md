# Configuration

Quark works without configuration. Server owners can tune the worker threads and the time Quark may use per tick, and restrict which network targets packages may reach, either with a `quark.toml` file or once with `Quark.Configure`.

## quark.toml

A `quark.toml` in the working directory of the server (the folder with `Config.toml`) is read when Quark loads:

```toml
[runtime]
worker_threads = 4
blocking_threads = 64
tick_budget = 4

[network]
allow_connect = ["127.0.0.1", "api.example.com:443", "*.internal.example.com", "10.0.0.0/8:6379"]
allow_listen = [7777, "8080-8090"]
```

With the file, the configuration is fixed and `Quark.Configure` raises an error. This makes the network rules effective even against packages that would like to change them. An invalid file is ignored with a message in the server output.

## Quark.Configure

Without a `quark.toml`, the first call of `Quark.Configure(options)` sets the configuration and locks it, later calls raise an error. Call it in a package that loads before all others.

```lua
Quark.Configure({
    WorkerThreads = 4,
    TickBudget = 4,
    Network = { AllowConnect = { "127.0.0.1", "*.example.com:443" } },
})
```

| Option | `quark.toml` | |
| --- | --- | --- |
| `WorkerThreads` | `worker_threads` | threads for asynchronous work like networking and timers. Half of the CPU cores between 2 and 8 by default, so the game thread keeps a core. Only takes effect before the first asynchronous function runs |
| `BlockingThreads` | `blocking_threads` | maximum threads for heavy work like password hashing or compression (64) |
| `TickBudget` | `tick_budget` | milliseconds per tick Quark may spend delivering results and events to Lua (4). Further results wait for the next tick |
| `Network.AllowConnect` | `network.allow_connect` | targets packages may connect or send to, everything when not set |
| `Network.AllowListen` | `network.allow_listen` | ports packages may listen on, everything when not set |

`Quark.GetConfiguration()` returns the current values with `Locked` and `Source` (`"quark.toml"`, `"Quark.Configure"` or `nil`).

## Network rules

C-module packages are not sandboxed, they can reach anything the server can reach. Network rules limit Quark's networking (TCP, UDP, WebSocket, HTTP client, Redis, MQTT and NATS) to known targets:

| Rule | Allows |
| --- | --- |
| `"api.example.com"` | this host name on every port |
| `"api.example.com:443"` | this host name on one port, `:8000-8100` for a range |
| `"*.example.com"` | all subdomains of `example.com` |
| `"10.0.0.0/8"`, `"192.168.1.20"`, `"[::1]:6379"` | IP addresses and ranges, also when a host name resolves to them |
| `"*"` | everything, useful with a port: `"*:443"` |

Host names are checked as written. For names without a matching name rule, all addresses the name resolves to have to match an IP rule, and the connection goes to exactly these addresses. Listen rules are ports or ranges like `7777` or `"8080-8090"`, port 0 (a random port) is not allowed when listen rules are set. UDP sockets bound to port 0 for sending only are always allowed, their targets are checked.

The rules cover Quark only. They do not affect the nanos world HTTP API, other c-modules or Lua code without Quark. Redis cluster nodes discovered at runtime are not checked, list them in the rules anyway when using clusters.
