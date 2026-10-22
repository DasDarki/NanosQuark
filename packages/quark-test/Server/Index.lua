Package.Require("Crypto.lua")
Package.Require("Buffer.lua")
Package.Require("Promise.lua")
Package.Require("Formats.lua")
Package.Require("Compression.lua")
Package.Require("Net.lua")
Package.Require("Redis.lua")
Package.Require("Random.lua")
Package.Require("Text.lua")
Package.Require("Time.lua")
Package.Require("Utilities.lua")
Package.Require("Locale.lua")
Package.Require("Testing.lua")

Quark.Testing.Run():Then(function(summary)
    if QuarkTestHost then
        QuarkTestHost.Finish(summary.Failed)
    end
end)
