local Promise, report, decode_component, decode_json = ...

local concat = table.concat
local error = error
local ipairs = ipairs
local setmetatable = setmetatable
local string_upper = string.upper
local tostring = tostring
local type = type

local METHODS = { GET = true, POST = true, PUT = true, PATCH = true, DELETE = true, HEAD = true, OPTIONS = true }

local Request = {}
Request.__index = Request

function Request:Json()
    return decode_json(self.Body)
end

function Request:GetHeader(name)
    return self.Headers[name:lower()]
end

local ClientResponse = {}
ClientResponse.__index = ClientResponse

function ClientResponse:Json()
    return decode_json(self.Body)
end

function ClientResponse:IsSuccess()
    return self.Status >= 200 and self.Status < 300
end

local function split(path)
    local segments = {}
    for segment in path:gmatch("[^/]+") do
        segments[#segments + 1] = segment
    end
    return segments
end

local function compile(pattern)
    if type(pattern) ~= "string" or pattern:sub(1, 1) ~= "/" then
        error("route patterns have to start with /", 3)
    end
    local segments = split(pattern)
    for index, segment in ipairs(segments) do
        if segment == "*" and index ~= #segments then
            error("the wildcard * is only allowed as the last segment", 3)
        end
    end
    return segments
end

local function match(segments, path)
    local params = {}
    for index, segment in ipairs(segments) do
        if segment == "*" then
            local rest = {}
            for position = index, #path do
                rest[#rest + 1] = path[position]
            end
            params["*"] = concat(rest, "/")
            return params
        end

        local value = path[index]
        if value == nil then
            return nil
        end
        if segment:sub(1, 1) == ":" then
            params[segment:sub(2)] = value
        elseif segment ~= value then
            return nil
        end
    end

    if #path ~= #segments then
        return nil
    end
    return params
end

local function add_route(state, method, pattern, handler)
    if type(handler) ~= "function" then
        error("route handlers have to be functions", 3)
    end
    method = string_upper(method)
    if method ~= "*" and not METHODS[method] then
        error("unsupported HTTP method '" .. method .. "'", 3)
    end
    state.routes[#state.routes + 1] = { method = method, segments = compile(pattern), handler = handler }
end

local function add_middleware(state, middleware)
    if type(middleware) ~= "function" then
        error("middleware has to be a function", 3)
    end
    state.middleware[#state.middleware + 1] = middleware
end

local function find_route(state, request)
    local path = split(request.Path)
    for index = 1, #path do
        path[index] = decode_component(path[index])
    end

    local allowed = {}
    for _, route in ipairs(state.routes) do
        local params = match(route.segments, path)
        if params then
            local method = request.Method
            if route.method == "*" or route.method == method or (method == "HEAD" and route.method == "GET") then
                return route, params, nil
            end
            allowed[#allowed + 1] = route.method
        end
    end
    return nil, nil, allowed
end

local function dispatch(state, request, response)
    setmetatable(request, Request)
    local label = request.Method .. " " .. request.Path

    Promise.Async(function()
        local route, params, allowed = find_route(state, request)
        request.Params = params or {}

        local index = 0
        local function next_handler()
            index = index + 1
            local middleware = state.middleware[index]
            if middleware then
                return middleware(request, response, next_handler)
            end
            if route then
                return route.handler(request, response)
            end
            if #allowed > 0 then
                response:Header("Allow", concat(allowed, ", "))
                return response:Text("Method Not Allowed", 405)
            end
            return response:Text("Not Found", 404)
        end

        next_handler()
        if not response:IsSent() then
            report("the HTTP handler for " .. label .. " finished without sending a response")
            response:Text("Internal Server Error", 500)
        end
    end):Catch(function(reason)
        report("the HTTP handler for " .. label .. " failed: " .. tostring(reason))
        if not response:IsSent() then
            response:Text("Internal Server Error", 500)
        end
    end)
end

local function contains(list, value)
    for _, entry in ipairs(list) do
        if entry == value then
            return true
        end
    end
    return false
end

local function cors(options)
    options = options or {}
    local origin = options.Origin or "*"
    local methods = options.Methods or "GET, POST, PUT, PATCH, DELETE, OPTIONS"
    local headers = options.Headers or "Content-Type, Authorization"
    local max_age = tostring(options.MaxAge or 600)

    return function(request, response, next_handler)
        local request_origin = request.Headers.origin
        local allowed
        if origin == "*" then
            allowed = "*"
        elseif type(origin) == "table" then
            allowed = contains(origin, request_origin) and request_origin or nil
        elseif origin == request_origin then
            allowed = origin
        end

        if allowed then
            response:Header("Access-Control-Allow-Origin", allowed)
            if allowed ~= "*" then
                response:Header("Vary", "Origin")
            end
            if options.Credentials then
                response:Header("Access-Control-Allow-Credentials", "true")
            end
        end

        if request.Method == "OPTIONS" and request.Headers["access-control-request-method"] then
            response:Header("Access-Control-Allow-Methods", methods)
            response:Header("Access-Control-Allow-Headers", headers)
            response:Header("Access-Control-Max-Age", max_age)
            return response:Send(nil, nil, 204)
        end
        return next_handler()
    end
end

return {
    ClientResponse = ClientResponse,
    add_route = add_route,
    add_middleware = add_middleware,
    dispatch = dispatch,
    cors = cors,
}
