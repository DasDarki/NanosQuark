---@meta

---@alias QuarkConnectionState "connecting"|"open"|"closing"|"closed"
---@alias QuarkDNSRecordType "A"|"AAAA"|"MX"|"TXT"|"SRV"|"CNAME"|"NS"|"PTR"
---@alias QuarkServerTLS { Certificate: string, PrivateKey: string, ClientCA?: string, RequireClientCertificate?: boolean }

---Client certificate of a connection with mutual TLS.
---@class QuarkPeerCertificate
---@field Subject string?
---@field Issuer string?
---@field Fingerprint string SHA-256 in hex
---@field NotAfter integer? Unix time
---@field PEM string
local PeerCertificate = {}

---Common methods of sockets and servers.
---@class QuarkConnection: QuarkEmitter
local Connection = {}

---@return QuarkConnectionState
function Connection:GetState() end

---@return boolean
function Connection:IsOpen() end

---@return string? ip
---@return integer? port
function Connection:GetLocalAddress() end

---@return string? ip
---@return integer? port
function Connection:GetRemoteAddress() end

---@return QuarkPeerCertificate?
function Connection:GetPeerCertificate() end

---@class QuarkNetModule
local Net = {}

---@param host string
---@param callback? function
---@return string[]?
---@return string? message
function Net.Resolve(host, callback) end

---@param name string
---@param type? QuarkDNSRecordType
---@param callback? function
---@return any[]?
---@return string? message
function Net.Lookup(name, type, callback) end

---@param ip string
---@param callback? function
---@return string[]?
---@return string? message
function Net.ReverseLookup(ip, callback) end

---@param value any
---@return boolean
function Net.IsIP(value) end

---@param value any
---@return boolean
function Net.IsIPv4(value) end

---@param value any
---@return boolean
function Net.IsIPv6(value) end

---@param value any
---@return boolean
function Net.IsPrivate(value) end

---@param value any
---@return boolean
function Net.IsLoopback(value) end

---@param text string
---@return string?
---@return string? message
function Net.NormalizeIP(text) end

---@param ip string
---@param range string
---@return boolean
function Net.IsInRange(ip, range) end

---@param names string|string[]
---@return string certificate
---@return string private_key
function Net.CreateSelfSignedCertificate(names) end

---@param name? string
---@param options? { Days?: integer }
---@return string certificate
---@return string private_key
function Net.CreateCertificateAuthority(name, options) end

---@param names string|string[]
---@param options? { CA?: { Certificate: string, PrivateKey: string }, Client?: boolean, Days?: integer }
---@return string certificate
---@return string private_key
function Net.CreateCertificate(names, options) end

---Opens a MaxMind database.
---@param path string
---@param options? { Language?: string }
---@param callback? function
---@return QuarkGeoIP?
---@return string? message
function Net.GeoIP(path, options, callback) end

---@class QuarkGeoIPPlace
---@field Code string?
---@field Name string?
---@field GeoNameID integer?
---@field IsInEuropeanUnion boolean?
local Place = {}

---@class QuarkGeoIPResult
---@field Network string
---@field Country QuarkGeoIPPlace?
---@field RegisteredCountry QuarkGeoIPPlace?
---@field Continent QuarkGeoIPPlace?
---@field City string?
---@field Subdivisions QuarkGeoIPPlace[]?
---@field PostalCode string?
---@field Location { Latitude: number?, Longitude: number?, AccuracyRadius: integer?, TimeZone: string? }?
---@field ASN integer?
---@field Organization string?
local GeoIPResult = {}

---A MaxMind database.
---@class QuarkGeoIP
local GeoIP = {}

---@param ip string
---@return QuarkGeoIPResult?
---@return string? message
function GeoIP:Lookup(ip) end

---@param ip string
---@return string?
function GeoIP:GetCountry(ip) end

---@param ip string
---@return table?
---@return string? message
function GeoIP:LookupRaw(ip) end

---@return { DatabaseType: string, BuildEpoch: integer, IpVersion: integer, NodeCount: integer, Languages: string[] }
function GeoIP:GetMetadata() end

---Events: Connect, Data(data), Error(message), Close(reason).
---@class QuarkTCPSocket: QuarkConnection
local TCPSocket = {}

---@param data QuarkBinary
---@return boolean
function TCPSocket:Send(data) end

function TCPSocket:Close() end

---Events: Connection(socket), Error(message), Close(reason).
---@class QuarkTCPServer: QuarkConnection
local TCPServer = {}

---@return integer
function TCPServer:GetPort() end

---@return QuarkTCPSocket[]
function TCPServer:GetClients() end

function TCPServer:Close() end

---@class QuarkTCPModule
local TCP = {}

---@param host string
---@param port integer
---@param options? { Framing?: "raw"|"line"|"length", MaxFrameSize?: integer, Timeout?: QuarkDuration, TLS?: boolean, ServerName?: string, CA?: string, Certificate?: string, PrivateKey?: string }
---@return QuarkTCPSocket
function TCP.Connect(host, port, options) end

---@param port integer
---@param options? { Framing?: "raw"|"line"|"length", MaxFrameSize?: integer, Host?: string, MaxConnections?: integer, TLS?: QuarkServerTLS }
---@return QuarkTCPServer?
---@return string? message
function TCP.Listen(port, options) end

---Events: Message(data, ip, port), Error(message), Close(reason).
---@class QuarkUDPSocket: QuarkConnection
local UDPSocket = {}

---@param data QuarkBinary
---@param host string
---@param port integer
---@return boolean
function UDPSocket:Send(data, host, port) end

function UDPSocket:Close() end

---@class QuarkUDPModule
local UDP = {}

---@param port? integer
---@param options? { Host?: string }
---@return QuarkUDPSocket?
---@return string? message
function UDP.Bind(port, options) end

---Events: Connect, Message(data, binary), Disconnect(code, reason), Reconnect(attempt, delay), Error(message), Close(code, reason).
---@class QuarkWebSocket: QuarkConnection
local WebSocket = {}

---@param text string
---@return boolean
function WebSocket:Send(text) end

---@param data QuarkBinary
---@return boolean
function WebSocket:SendBinary(data) end

---@param code? integer
---@param reason? string
function WebSocket:Close(code, reason) end

---Events: Connection(socket, request), Error(message), Close(reason).
---@class QuarkWebSocketServer: QuarkConnection
local WebSocketServer = {}

---@param data QuarkBinary
---@param binary? boolean
---@return integer
function WebSocketServer:Broadcast(data, binary) end

---@return integer
function WebSocketServer:GetPort() end

---@return QuarkWebSocket[]
function WebSocketServer:GetClients() end

function WebSocketServer:Close() end

---@class QuarkWebSocketModule
local WebSocketModule = {}

---@param url string
---@param options? { Headers?: table<string, string>, Timeout?: QuarkDuration, CA?: string, Certificate?: string, PrivateKey?: string, MaxMessageSize?: integer, Reconnect?: boolean|{ InitialDelay?: QuarkDuration, MaxDelay?: QuarkDuration, Multiplier?: number, MaxAttempts?: integer } }
---@return QuarkWebSocket
function WebSocketModule.Connect(url, options) end

---@param port integer
---@param options? { Host?: string, Path?: string, MaxConnections?: integer, MaxMessageSize?: integer, TLS?: QuarkServerTLS }
---@return QuarkWebSocketServer?
---@return string? message
function WebSocketModule.Listen(port, options) end

---A request received by an HTTP server.
---@class QuarkHTTPRequest
---@field Method string
---@field Path string
---@field Params table<string, string>
---@field Query table<string, string>
---@field RawQuery string
---@field Headers table<string, string>
---@field Body string
---@field Cookies table<string, string>
---@field RemoteAddress string
---@field IsWebSocket boolean
---@field Version string
---@field ClientCertificate QuarkPeerCertificate?
local Request = {}

---@return any
---@return string? message
function Request:Json() end

---@return table? fields
---@return table|string? files_or_message
function Request:Form() end

---@param name string
---@return string?
function Request:GetHeader(name) end

---Response of an HTTP handler.
---@class QuarkHTTPResponse
local Response = {}

---@param code integer
---@return QuarkHTTPResponse
function Response:Status(code) end

---@param name string
---@param value string
---@return QuarkHTTPResponse
function Response:Header(name, value) end

---@param value any
---@param status? integer
---@return boolean
function Response:Json(value, status) end

---@param text string
---@param status? integer
---@return boolean
function Response:Text(text, status) end

---@param html string
---@param status? integer
---@return boolean
function Response:Html(html, status) end

---@param body? QuarkBinary
---@param content_type? string
---@param status? integer
---@return boolean
function Response:Send(body, content_type, status) end

---@param location string
---@param status? integer
---@return boolean
function Response:Redirect(location, status) end

---@param path string
---@param options? { MaxAge?: integer, ContentType?: string, MaxFileSize?: integer }
---@return boolean
function Response:SendFile(path, options) end

---@param name string
---@param value string
---@param options? { MaxAge?: integer, Path?: string, Domain?: string, Secure?: boolean, HttpOnly?: boolean, SameSite?: "Lax"|"Strict"|"None" }
---@return QuarkHTTPResponse
function Response:Cookie(name, value, options) end

---@param name string
---@param options? { Path?: string, Domain?: string }
---@return QuarkHTTPResponse
function Response:ClearCookie(name, options) end

---@param enabled? boolean
---@return QuarkHTTPResponse
function Response:Compress(enabled) end

---@return boolean
function Response:IsSent() end

---@return integer
function Response:GetStatus() end

---Events: Error(message), Close(reason).
---@class QuarkHTTPServer: QuarkConnection
local HTTPServer = {}

---@param pattern string
---@param handler fun(request: QuarkHTTPRequest, response: QuarkHTTPResponse)
---@return QuarkHTTPServer
function HTTPServer:Get(pattern, handler) end

---@param pattern string
---@param handler fun(request: QuarkHTTPRequest, response: QuarkHTTPResponse)
---@return QuarkHTTPServer
function HTTPServer:Post(pattern, handler) end

---@param pattern string
---@param handler fun(request: QuarkHTTPRequest, response: QuarkHTTPResponse)
---@return QuarkHTTPServer
function HTTPServer:Put(pattern, handler) end

---@param pattern string
---@param handler fun(request: QuarkHTTPRequest, response: QuarkHTTPResponse)
---@return QuarkHTTPServer
function HTTPServer:Patch(pattern, handler) end

---@param pattern string
---@param handler fun(request: QuarkHTTPRequest, response: QuarkHTTPResponse)
---@return QuarkHTTPServer
function HTTPServer:Delete(pattern, handler) end

---@param pattern string
---@param handler fun(request: QuarkHTTPRequest, response: QuarkHTTPResponse)
---@return QuarkHTTPServer
function HTTPServer:Options(pattern, handler) end

---@param pattern string
---@param handler fun(request: QuarkHTTPRequest, response: QuarkHTTPResponse)
---@return QuarkHTTPServer
function HTTPServer:All(pattern, handler) end

---@param method string
---@param pattern string
---@param handler fun(request: QuarkHTTPRequest, response: QuarkHTTPResponse)
---@return QuarkHTTPServer
function HTTPServer:Route(method, pattern, handler) end

---@param pattern string
---@param handler fun(socket: QuarkWebSocket, request: QuarkHTTPRequest)
---@return QuarkHTTPServer
function HTTPServer:WebSocket(pattern, handler) end

---@param prefix string
---@param directory string
---@param options? { Index?: string, MaxAge?: integer }
---@return QuarkHTTPServer
function HTTPServer:Static(prefix, directory, options) end

---@param middleware fun(request: QuarkHTTPRequest, response: QuarkHTTPResponse, next: fun())
---@return QuarkHTTPServer
function HTTPServer:Use(middleware) end

---@return integer
function HTTPServer:GetPort() end

function HTTPServer:Close() end

---Response of an HTTP request.
---@class QuarkHTTPClientResponse
---@field Status integer
---@field Version string
---@field Headers table<string, string>
---@field Body string
---@field URL string
local ClientResponse = {}

---@return any
---@return string? message
function ClientResponse:Json() end

---@return boolean
function ClientResponse:IsSuccess() end

---Events: Response(status, headers, url), Data(chunk), End, Error(message), Close(reason).
---@class QuarkHTTPStream: QuarkConnection
local HTTPStream = {}

function HTTPStream:Close() end

---@class QuarkHTTPModule
local HTTP = {}

---@param port integer
---@param options? { Host?: string, MaxBodySize?: integer, Timeout?: QuarkDuration, MaxConnections?: integer, MaxMessageSize?: integer, TLS?: QuarkServerTLS, Compression?: boolean|{ Encodings?: string[], MinSize?: integer, Level?: integer }, HTTP2?: boolean }
---@return QuarkHTTPServer?
---@return string? message
function HTTP.Listen(port, options) end

---CORS middleware for HTTPServer:Use.
---@param options? { Origin?: string|string[], Methods?: string, Headers?: string, Credentials?: boolean, MaxAge?: integer }
---@return function
function HTTP.Cors(options) end

---@param options { URL?: string, Method?: string, Headers?: table<string, string>, Query?: table, Body?: QuarkBinary, Json?: any, Timeout?: QuarkDuration, MaxBodySize?: integer, FollowRedirects?: boolean, MaxRedirects?: integer, CA?: string, Certificate?: string, PrivateKey?: string, Decompress?: boolean, HTTP2?: boolean }
---@param callback? function
---@return QuarkHTTPClientResponse?
---@return string? message
function HTTP.Request(options, callback) end

---@param url string
---@param options? { URL?: string, Method?: string, Headers?: table<string, string>, Query?: table, Body?: QuarkBinary, Json?: any, Timeout?: QuarkDuration, MaxBodySize?: integer, FollowRedirects?: boolean, MaxRedirects?: integer, CA?: string, Certificate?: string, PrivateKey?: string, Decompress?: boolean, HTTP2?: boolean }
---@param callback? function
---@return QuarkHTTPClientResponse?
---@return string? message
function HTTP.Get(url, options, callback) end

---@param url string
---@param body any
---@param options? { URL?: string, Method?: string, Headers?: table<string, string>, Query?: table, Body?: QuarkBinary, Json?: any, Timeout?: QuarkDuration, MaxBodySize?: integer, FollowRedirects?: boolean, MaxRedirects?: integer, CA?: string, Certificate?: string, PrivateKey?: string, Decompress?: boolean, HTTP2?: boolean }
---@param callback? function
---@return QuarkHTTPClientResponse?
---@return string? message
function HTTP.Post(url, body, options, callback) end

---@param options { URL?: string, Method?: string, Headers?: table<string, string>, Query?: table, Body?: QuarkBinary, Json?: any, Timeout?: QuarkDuration, MaxBodySize?: integer, FollowRedirects?: boolean, MaxRedirects?: integer, CA?: string, Certificate?: string, PrivateKey?: string, Decompress?: boolean, HTTP2?: boolean }
---@return QuarkHTTPStream
function HTTP.Stream(options) end
