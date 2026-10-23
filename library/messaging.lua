---@meta

---Events: Connect(session_present), Message(topic, payload, details), Disconnect(reason), Error(message), Close(reason).
---@class QuarkMQTTClient: QuarkEmitter
local MQTTClient = {}

---@param topic string
---@param qos? 0|1|2
function MQTTClient:Listen(topic, qos) end

---@param topic string
function MQTTClient:Unlisten(topic) end

---@param topic string
---@param payload QuarkBinary
---@param options? { QoS?: 0|1|2, Retain?: boolean }
---@return boolean?
---@return string? message
function MQTTClient:Publish(topic, payload, options) end

---@return string[]
function MQTTClient:GetTopics() end

---@return string
function MQTTClient:GetClientID() end

---@return "connecting"|"open"|"closed"
function MQTTClient:GetState() end

---@return boolean
function MQTTClient:IsOpen() end

function MQTTClient:Close() end

---@class QuarkMQTTModule
local MQTT = {}

---@param url string
---@param options? { ClientID?: string, Username?: string, Password?: string, KeepAlive?: QuarkDuration, CleanSession?: boolean, Will?: { Topic: string, Payload?: QuarkBinary, QoS?: 0|1|2, Retain?: boolean }, MaxPacketSize?: integer, TLS?: boolean, CA?: string, Certificate?: string, PrivateKey?: string }
---@return QuarkMQTTClient
function MQTT.Connect(url, options) end

---@param filter string
---@param topic string
---@return boolean
function MQTT.TopicMatches(filter, topic) end

---A received NATS message.
---@class QuarkNATSMessage
---@field Subject string
---@field Payload string
---@field Reply string?
---@field Headers table<string, string>?
local NATSMessage = {}

---@param payload QuarkBinary
---@param headers? table<string, string>
---@return boolean
function NATSMessage:Respond(payload, headers) end

---Events: Message(message) when no handler was given.
---@class QuarkNATSSubscription: QuarkEmitter
local NATSSubscription = {}

function NATSSubscription:Unlisten() end

---@return string
function NATSSubscription:GetSubject() end

---@return string?
function NATSSubscription:GetQueue() end

---Events: Connect, Disconnect(reason), Error(message), Close(reason).
---@class QuarkNATSClient: QuarkEmitter
local NATSClient = {}

---@param subject string
---@param payload QuarkBinary
---@param options? { Reply?: string, Headers?: table<string, string> }
---@return boolean
function NATSClient:Publish(subject, payload, options) end

---@param subject string
---@param handler? fun(message: QuarkNATSMessage)
---@param options? { Queue?: string }
---@return QuarkNATSSubscription
function NATSClient:Listen(subject, handler, options) end

---@param subject string
---@param payload QuarkBinary
---@param options? { Timeout?: QuarkDuration, Headers?: table<string, string> }
---@param callback? function
---@return QuarkNATSMessage?
---@return string? message
function NATSClient:Request(subject, payload, options, callback) end

---@param callback? function
---@return boolean?
---@return string? message
function NATSClient:Flush(callback) end

---@return "connecting"|"open"|"closed"
function NATSClient:GetState() end

---@return boolean
function NATSClient:IsOpen() end

function NATSClient:Close() end

---@class QuarkNATSModule
local NATS = {}

---@param urls? string|string[]
---@param options? { Name?: string, Username?: string, Password?: string, Token?: string, Timeout?: QuarkDuration, TLS?: boolean, CA?: string, Certificate?: string, PrivateKey?: string }
---@return QuarkNATSClient
function NATS.Connect(urls, options) end
