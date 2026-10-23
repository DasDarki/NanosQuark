local Test = Quark.Import("Testing")
local Expect = Test.Expect

Test.Describe("Quark.Configure", function()
    Test.It("reports the configuration", function()
        local configuration = Quark.GetConfiguration()
        Expect(configuration.TickBudget):ToBe(4)
        Expect(configuration.WorkerThreads >= 2):ToBe(true)
        Expect(configuration.BlockingThreads):ToBe(64)
        Expect(configuration.Network):ToEqual({})
    end)

    Test.It("restricts network targets once", function()
        Quark.Configure({
            TickBudget = 5,
            Network = { AllowConnect = { "127.0.0.1", "localhost", "[::1]", "*.example.test:443", "10.20.0.0/16:6379" } },
        })
        local configuration = Quark.GetConfiguration()
        Expect(configuration.TickBudget):ToBe(5)
        Expect(configuration.Locked):ToBe(true)
        Expect(configuration.Source):ToBe("Quark.Configure")
        Expect(#configuration.Network.AllowConnect):ToBe(5)
        Expect(configuration.Network.AllowListen):ToBeNil()
        Expect(function() Quark.Configure({ TickBudget = 3 }) end):ToThrow("only be called once")

        local response, message = Quark.HTTP.Get("http://192.0.2.1:9/")
        Expect(response):ToBeNil()
        Expect(message):ToContain("not allowed by the Quark network configuration")
        Expect(function() Quark.Redis.Connect("redis://10.30.0.1:6379") end):ToThrow("not allowed")
        Expect(function() Quark.Redis.Connect("redis://10.20.0.1:6380") end):ToThrow("not allowed")
        local redis = Quark.Redis.Connect("redis://10.20.0.1:6379", { Timeout = 100, Retries = 0 })
        redis:Close()

        local socket = assert(Quark.UDP.Bind(0, { Host = "127.0.0.1" }))
        local errored = Quark.Promise.New(function(resolve)
            socket:Subscribe("Error", resolve)
        end)
        socket:Send("hello", "192.0.2.1", 9)
        Expect(Quark.Await(errored)):ToContain("not allowed")
        socket:Close()

        local server = assert(Quark.HTTP.Listen(0, { Host = "127.0.0.1" }))
        server:Get("/", function(_, response) response:Text("still local") end)
        Expect(Quark.Await(Quark.HTTP.Get, "http://127.0.0.1:" .. server:GetPort() .. "/").Body):ToBe("still local")
        server:Close()
    end)
end)
