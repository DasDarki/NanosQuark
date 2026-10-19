local module_directory, package_directory = ...

package.cpath = module_directory .. "/lib?.so;" .. package.cpath

local host = { Finished = false, ExitCode = 0 }
local tick_handlers = {}
local unload_handlers = {}

local function join(...)
    local parts = table.pack(...)
    for index = 1, parts.n do
        parts[index] = tostring(parts[index])
    end
    return table.concat(parts, " ")
end

Console = {
    Log = function(...)
        io.stdout:write(join(...), "\n")
    end,
    Warn = function(...)
        io.stdout:write("WARN  ", join(...), "\n")
    end,
    Error = function(...)
        io.stderr:write("ERROR ", join(...), "\n")
    end,
}

Server = {
    Subscribe = function(event, callback)
        if event == "Tick" then
            tick_handlers[#tick_handlers + 1] = callback
        end
        return callback
    end,
    Unsubscribe = function() end,
}

require("quark")

local environment = setmetatable({}, { __index = _G })

environment.Package = {
    Subscribe = function(event, callback)
        if event == "Unload" then
            unload_handlers[#unload_handlers + 1] = callback
        end
        return callback
    end,
    GetName = function()
        return package_directory:match("([^/]+)/?$")
    end,
    Require = function(path)
        local chunk = assert(loadfile(package_directory .. "/Server/" .. path, "t", environment))
        return chunk()
    end,
}

environment.QuarkTestHost = {
    Finish = function(failed)
        host.ExitCode = (failed and failed > 0) and 1 or 0
        host.Finished = true
    end,
}

local main = assert(loadfile(package_directory .. "/Server/Index.lua", "t", environment))
main()

function host.Tick(delta)
    for index = 1, #tick_handlers do
        tick_handlers[index](delta)
    end
end

return host
