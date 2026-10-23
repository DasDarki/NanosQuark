local Test = Quark.Import("Testing")
local Expect = Test.Expect

local MQTT_URL = os and os.getenv and os.getenv("QUARK_MQTT_URL")
local NATS_URL = os and os.getenv and os.getenv("QUARK_NATS_URL")
local prefix = "quark-test/" .. Quark.Crypto.RandomToken(6)

local function next_event(object, event)
    return Quark.Promise.New(function(resolve)
        local handler
        handler = object:Subscribe(event, function(...)
            object:Unsubscribe(event, handler)
            resolve(...)
        end)
    end)
end

local function wait_open(client)
    if not client:IsOpen() then
        Quark.Await(next_event(client, "Connect"))
    end
end

;(MQTT_URL and Test.Describe or Test.DescribeSkip)("MQTT", function()
    local MQTT = Quark.MQTT

    Test.It("publishes and receives messages", function()
        local subscriber = MQTT.Connect(MQTT_URL)
        local publisher = MQTT.Connect(MQTT_URL, { ClientID = "quark-publisher-" .. Quark.Crypto.RandomToken(4) })
        wait_open(subscriber)
        wait_open(publisher)
        Expect(publisher:GetClientID()):ToContain("quark-publisher-")

        subscriber:Listen(prefix .. "/sensors/+", 1)
        Expect(subscriber:GetTopics()):ToEqual({ prefix .. "/sensors/+" })
        Quark.Await(Quark.Delay(200))

        local received = next_event(subscriber, "Message")
        Expect(publisher:Publish(prefix .. "/sensors/temperature", "21.5", { QoS = 1 })):ToBe(true)
        local topic, payload, details = Quark.Await(received)
        Expect({ topic, payload, details.QoS, details.Retain }):ToEqual({ prefix .. "/sensors/temperature", "21.5", 1, false })

        subscriber:Unlisten(prefix .. "/sensors/+")
        Expect(subscriber:GetTopics()):ToEqual({})
        subscriber:Close()
        publisher:Close()
        Quark.Await(next_event(publisher, "Close"))
        Expect(publisher:GetState()):ToBe("closed")
    end)

    Test.It("keeps retained messages and matches topics", function()
        local publisher = MQTT.Connect(MQTT_URL)
        wait_open(publisher)
        publisher:Publish(prefix .. "/status", "online", { Retain = true, QoS = 1 })
        Quark.Await(Quark.Delay(200))

        local late = MQTT.Connect(MQTT_URL)
        late:Listen(prefix .. "/#")
        local _, payload, details = Quark.Await(next_event(late, "Message"))
        Expect({ payload, details.Retain }):ToEqual({ "online", true })

        publisher:Publish(prefix .. "/status", "", { Retain = true })
        late:Close()
        publisher:Close()

        Expect(MQTT.TopicMatches("game/+/kills", "game/eu-1/kills")):ToBe(true)
        Expect(MQTT.TopicMatches("game/#", "game/eu-1/kills")):ToBe(true)
        Expect(MQTT.TopicMatches("game/+", "game/eu-1/kills")):ToBe(false)
        Expect(function() MQTT.Connect("http://localhost") end):ToThrow("unsupported MQTT URL scheme")
        Expect(function() publisher:Publish("x", "y", { QoS = 3 }) end):ToThrow("QoS")
    end, { Timeout = 10000 })

    Test.It("does not send the last will on a clean close", function()
        local watcher = MQTT.Connect(MQTT_URL)
        wait_open(watcher)
        watcher:Listen(prefix .. "/will")
        Quark.Await(Quark.Delay(200))

        local received = false
        watcher:Subscribe("Message", function() received = true end)
        local leaving = MQTT.Connect(MQTT_URL, { Will = { Topic = prefix .. "/will", Payload = "gone" } })
        wait_open(leaving)
        leaving:Close()
        Quark.Await(next_event(leaving, "Close"))
        Quark.Await(Quark.Delay(500))
        Expect(received):ToBe(false)
        watcher:Close()
    end)

    Test.It("reports unreachable brokers", function()
        local client = MQTT.Connect("mqtt://127.0.0.1:1")
        Expect(Quark.Await(next_event(client, "Error"))):ToContain("cannot connect to the MQTT broker")
        client:Close()
    end)
end)

;(NATS_URL and Test.Describe or Test.DescribeSkip)("NATS", function()
    local NATS = Quark.NATS
    local subject = prefix:gsub("/", ".")

    Test.It("publishes to listeners and queue groups", function()
        local client = assert(NATS.Connect(NATS_URL))
        wait_open(client)

        local received = {}
        local all = client:Listen(subject .. ".events.*", function(message)
            received[#received + 1] = message.Subject .. "=" .. message.Payload
        end)
        local workers = { 0, 0 }
        local first = client:Listen(subject .. ".jobs", function() workers[1] = workers[1] + 1 end, { Queue = "workers" })
        local second = client:Listen(subject .. ".jobs", function() workers[2] = workers[2] + 1 end, { Queue = "workers" })
        Expect(first:GetQueue()):ToBe("workers")
        Quark.Await(client.Flush, client)
        Quark.Await(Quark.Delay(100))

        client:Publish(subject .. ".events.kill", "1")
        client:Publish(subject .. ".events.join", "2", { Headers = { source = "test" } })
        for _ = 1, 10 do
            client:Publish(subject .. ".jobs", "work")
        end
        Quark.Await(client.Flush, client)
        Quark.Await(Quark.Delay(200))

        table.sort(received)
        Expect(received):ToEqual({ subject .. ".events.join=2", subject .. ".events.kill=1" })
        Expect(workers[1] + workers[2]):ToBe(10)

        all:Unlisten()
        Quark.Await(Quark.Delay(100))
        client:Publish(subject .. ".events.kill", "3")
        Quark.Await(client.Flush, client)
        Quark.Await(Quark.Delay(100))
        Expect(#received):ToBe(2)
        first:Unlisten()
        second:Unlisten()
        client:Close()
    end)

    Test.It("answers requests", function()
        local client = assert(NATS.Connect(NATS_URL))
        wait_open(client)
        local service = client:Listen(subject .. ".add", function(message)
            local a, b = message.Payload:match("(%d+)%+(%d+)")
            message:Respond(tostring(tonumber(a) + tonumber(b)), { worker = "lua" })
        end)
        Quark.Await(client.Flush, client)

        local answer = Quark.Await(client.Request, client, subject .. ".add", "20+22")
        Expect(answer.Payload):ToBe("42")
        Expect(answer.Headers.worker):ToBe("lua")

        local events = client:Listen(subject .. ".event")
        local message = next_event(events, "Message")
        client:Publish(subject .. ".event", "evented", { Reply = subject .. ".inbox" })
        local delivered = Quark.Await(message)
        Expect({ delivered.Payload, delivered.Reply }):ToEqual({ "evented", subject .. ".inbox" })

        Expect(function() Quark.Await(client.Request, client, subject .. ".nobody", "?", { Timeout = 300 }) end):ToThrow("failed")
        Expect(function() client:Publish("bad subject", "x") end):ToThrow("not a valid NATS subject")
        service:Unlisten()
        events:Unlisten()
        client:Close()
        Quark.Await(next_event(client, "Close"))
        Expect(client:IsOpen()):ToBe(false)
    end)
end)
