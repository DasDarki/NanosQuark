# Messaging

```lua
local mqtt = Quark.MQTT.Connect("mqtt://broker.example.com")
mqtt:Listen("game/+/events")
mqtt:Subscribe("Message", function(topic, payload)
    Console.Log(topic .. ": " .. payload)
end)

local nats = assert(Quark.NATS.Connect("nats://10.0.0.5:4222"))
nats:Listen("servers.*.status", function(message)
    Console.Log(message.Subject .. " is " .. message.Payload)
end)
```

Clients for two message brokers: `MQTT` for IoT devices, home automation and dashboards, `NATS` for fast messaging between servers and services. Both reconnect on their own and deliver messages as events on the game thread. For Redis Pub/Sub and streams see [docs/redis.md](redis.md).

## MQTT

`MQTT.Connect(url, options?)` connects to an MQTT 3.1.1 broker like Mosquitto, EMQX or HiveMQ. The URL is `mqtt://host:port` (1883) or `mqtts://host:port` (8883) for TLS, credentials can be part of it (`mqtt://user:password@host`). The client is returned right away and connects in the background.

| Option | |
| --- | --- |
| `ClientID` | identifies the client at the broker, random by default. Two clients with the same ID disconnect each other |
| `Username`, `Password` | credentials |
| `KeepAlive` | seconds between pings (30) |
| `CleanSession` | start without stored subscriptions and messages (`true`). With `false` the broker keeps QoS 1 and 2 messages for the client while it is offline |
| `Will` | `{ Topic, Payload, QoS, Retain }` published by the broker when the client disappears without closing, for example `"offline"` on a status topic |
| `MaxPacketSize` | largest message in bytes (1 MiB) |
| `TLS`, `CA`, `Certificate`, `PrivateKey` | TLS and client certificates like for [Net](net.md#tls) |

| Method | |
| --- | --- |
| `:Listen(topic, qos?)` | subscribe to a topic filter with `+` for one level and `#` for the rest, like `game/+/kills` or `game/#` |
| `:Unlisten(topic)` | unsubscribe |
| `:Publish(topic, payload, options?)` | send a message, options `QoS` (0, 1 or 2) and `Retain`. Returns `true` or `nil, message` when the send queue is full |
| `:GetTopics()`, `:GetClientID()` | |
| `:GetState()`, `:IsOpen()`, `:Close()` | |

| Event | |
| --- | --- |
| `Connect(session_present)` | connected, also after reconnecting. Subscriptions are restored automatically |
| `Message(topic, payload, details)` | a message, `details` is `{ QoS, Retain, Duplicate }` |
| `Disconnect(reason)` | the connection was lost, reconnecting starts with a growing delay (0.5 to 10 seconds) |
| `Error(message)`, `Close(reason)` | |

`MQTT.TopicMatches(filter, topic)` checks whether a topic matches a filter.

Retained messages are stored by the broker and delivered to every new subscriber, which makes them ideal for current states like `"server/eu-1/players" = "24"`. Publish an empty retained message to clear one.

## NATS

`NATS.Connect(urls?, options?)` connects to a NATS server or cluster. `urls` is a URL like `nats://host:4222` (the default is `nats://127.0.0.1:4222`) or a list of them. It returns a client or `nil, message` for invalid settings. The connection is made in the background and reestablished after failures.

| Option | |
| --- | --- |
| `Name` | client name shown in the server monitoring (`"quark"`) |
| `Username`, `Password` or `Token` | credentials |
| `Timeout` | connection and default request timeout in milliseconds (5000) |
| `TLS`, `CA`, `Certificate`, `PrivateKey` | require TLS and client certificates like for [Net](net.md#tls) |

| Method | |
| --- | --- |
| `:Publish(subject, payload, options?)` | send a message, options `Reply` (reply subject) and `Headers` (table) |
| `:Listen(subject, handler?, options?)` | receive messages on a subject, `*` matches one token and `>` the rest, like `servers.*.status` or `logs.>`. Returns a subscription |
| `:Request(subject, payload, options?, callback?)` | send a request and wait for the first answer, options `Timeout` and `Headers`. Fails without responders or after the timeout |
| `:Flush(callback?)` | waits until all published messages reached the server |
| `:GetState()`, `:IsOpen()`, `:Close()` | `Close` sends pending messages before closing |

The client emits `Connect`, `Disconnect(reason)`, `Error(message)` and `Close(reason)`.

Messages are tables with `Subject`, `Payload`, `Reply`, `Headers` and `:Respond(payload, headers?)`, which answers a request. With a `handler` messages go to the function, otherwise the subscription emits `Message(message)`. Subscriptions have `:Unlisten()`, `:GetSubject()` and `:GetQueue()` and stay active until unlistened, also when the object is not kept.

With the option `Queue` subscriptions with the same queue name share the messages: each message goes to only one of them, which spreads work across several servers.

```lua
nats:Listen("leaderboard.top", function(message)
    message:Respond(Quark.JSON.Encode(GetTopPlayers()))
end)

Quark.Async(function()
    local answer = Quark.Await(nats.Request, nats, "leaderboard.top", "")
    local top = Quark.JSON.Decode(answer.Payload)
end)
```
