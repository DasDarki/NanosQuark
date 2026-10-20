use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use mlua::prelude::*;
use quark_core::{Binary, Bytes, Emitter, Options, create_object, emit_now, object_state, runtime};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_tungstenite::tungstenite::http::{HeaderName, HeaderValue, StatusCode};
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, WebSocketConfig};

use crate::connection::{
    Connected, Connection, add_connection_methods, bind_tcp, open_clients, weak_table,
};
use crate::framing::DEFAULT_MAX_FRAME_SIZE;
use crate::stream::{self, BoxedIo};

const DEFAULT_TIMEOUT: u64 = 10_000;
const DEFAULT_MAX_CONNECTIONS: usize = 1024;
const CLOSE_TIMEOUT: Duration = Duration::from_secs(2);

pub enum Command {
    Text(String),
    Binary(Vec<u8>),
    Close(u16, String),
}

fn close_command() -> Command {
    Command::Close(1001, "going away".to_owned())
}

pub struct WebSocket {
    connection: Arc<Connection<Command>>,
}

impl Connected for WebSocket {
    type Command = Command;

    fn connection(&self) -> &Arc<Connection<Command>> {
        &self.connection
    }
}

impl LuaUserData for WebSocket {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        add_connection_methods(methods);
        methods.add_method("Send", |_, socket, text: String| {
            Ok(socket.connection.send(Command::Text(text)))
        });
        methods.add_method("SendBinary", |_, socket, data: Binary| {
            Ok(socket.connection.send(Command::Binary(data.0)))
        });
        methods.add_method(
            "Close",
            |_, socket, (code, reason): (Option<u16>, Option<String>)| {
                let code = code.unwrap_or(1000);
                if !(1000..=4999).contains(&code) {
                    return Err(LuaError::runtime(
                        "WebSocket close codes have to be between 1000 and 4999",
                    ));
                }
                socket
                    .connection
                    .close_with(Command::Close(code, reason.unwrap_or_default()));
                Ok(())
            },
        );
        methods.add_meta_method(LuaMetaMethod::ToString, |_, socket, ()| {
            Ok(format!("WebSocket({})", socket.connection.state_name()))
        });
    }
}

struct ServerShared {
    port: u16,
    clients: Mutex<Vec<Weak<Connection<Command>>>>,
}

pub enum ServerCommand {
    Close,
}

pub struct WebSocketServer {
    connection: Arc<Connection<ServerCommand>>,
    shared: Arc<ServerShared>,
}

impl Connected for WebSocketServer {
    type Command = ServerCommand;

    fn connection(&self) -> &Arc<Connection<ServerCommand>> {
        &self.connection
    }
}

impl LuaUserData for WebSocketServer {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        add_connection_methods(methods);
        methods.add_method("GetPort", |_, server, ()| Ok(server.shared.port));
        methods.add_function("GetClients", |lua, this: LuaAnyUserData| {
            open_clients::<WebSocket>(lua, &this)
        });
        methods.add_method(
            "Broadcast",
            |_, server, (data, binary): (Binary, Option<bool>)| {
                let clients = server
                    .shared
                    .clients
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .clone();
                let mut sent = 0;
                for client in clients.iter().filter_map(Weak::upgrade) {
                    let command = if binary.unwrap_or(false) {
                        Command::Binary(data.0.clone())
                    } else {
                        Command::Text(String::from_utf8_lossy(data.as_bytes()).into_owned())
                    };
                    if client.send(command) {
                        sent += 1;
                    }
                }
                Ok(sent)
            },
        );
        methods.add_method("Close", |_, server, ()| {
            server.connection.close_with(ServerCommand::Close);
            Ok(())
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, server, ()| {
            Ok(format!(
                "WebSocketServer({}, port {})",
                server.connection.state_name(),
                server.shared.port
            ))
        });
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Connect", lua.create_function(connect)?)?;
    module.set("Listen", lua.create_function(listen)?)?;
    Ok(module)
}

fn config(options: &Options) -> LuaResult<WebSocketConfig> {
    let max = options.get_or("MaxMessageSize", DEFAULT_MAX_FRAME_SIZE)?;
    Ok(WebSocketConfig::default()
        .max_message_size(Some(max))
        .max_frame_size(Some(max)))
}

fn connect(lua: &Lua, (url, options): (String, Options)) -> LuaResult<LuaAnyUserData> {
    let mut request = url
        .as_str()
        .into_client_request()
        .map_err(|error| LuaError::runtime(format!("invalid WebSocket URL: {error}")))?;
    let secure = match request.uri().scheme_str() {
        Some("ws") => false,
        Some("wss") => true,
        _ => {
            return Err(LuaError::runtime(
                "WebSocket URLs have to start with ws:// or wss://",
            ));
        }
    };
    let host = request
        .uri()
        .host()
        .ok_or_else(|| LuaError::runtime("the WebSocket URL has no host"))?
        .trim_start_matches('[')
        .trim_end_matches(']')
        .to_owned();
    let port = request
        .uri()
        .port_u16()
        .unwrap_or(if secure { 443 } else { 80 });

    if let Some(headers) = options.get::<LuaTable>("Headers")? {
        for pair in headers.pairs::<String, String>() {
            let (name, value) = pair?;
            let name = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| LuaError::runtime(format!("'{name}' is not a valid header name")))?;
            let value = HeaderValue::from_str(&value).map_err(|_| {
                LuaError::runtime(format!("the value of the header '{name}' is invalid"))
            })?;
            request.headers_mut().insert(name, value);
        }
    }

    let timeout = Duration::from_millis(options.get_or("Timeout", DEFAULT_TIMEOUT)?);
    let config = config(&options)?;
    let (connection, commands) = Connection::new(close_command);
    let object = create_object(
        lua,
        WebSocket {
            connection: Arc::clone(&connection),
        },
    )?;
    let emitter = Emitter::new(lua, &object)?;
    connection.track(lua, &emitter)?;

    runtime().spawn(async move {
        let result = async {
            let connected =
                stream::connect(&host, port, secure.then_some(host.as_str()), timeout).await?;
            let (socket, _) = tokio::time::timeout(
                timeout,
                tokio_tungstenite::client_async_with_config(request, connected.io, Some(config)),
            )
            .await
            .map_err(|_| "the WebSocket handshake timed out".to_owned())?
            .map_err(|error| format!("the WebSocket handshake failed: {error}"))?;
            Ok::<_, String>((socket, connected.local, connected.remote))
        }
        .await;

        match result {
            Ok((socket, local, remote)) => {
                connection.open(local, remote);
                emitter.emit("Open", ());
                drive(socket, connection, emitter, commands).await;
            }
            Err(error) => {
                connection.closed();
                emitter.emit("Error", error.clone());
                emitter.emit("Close", (1006, error));
            }
        }
    });

    Ok(object)
}

async fn drive(
    socket: WebSocketStream<BoxedIo>,
    connection: Arc<Connection<Command>>,
    emitter: Emitter,
    mut commands: UnboundedReceiver<Command>,
) {
    let (mut sink, mut stream) = socket.split();
    let mut closing = false;
    let mut close_deadline = tokio::time::Instant::now();

    let (code, reason) = loop {
        tokio::select! {
            message = stream.next() => match message {
                Some(Ok(Message::Text(text))) => emitter.emit("Message", (text.to_string(), false)),
                Some(Ok(Message::Binary(data))) => emitter.emit("Message", (Bytes(data.to_vec()), true)),
                Some(Ok(Message::Close(frame))) => {
                    let (code, reason) = frame
                        .map(|frame| (u16::from(frame.code), frame.reason.to_string()))
                        .unwrap_or((1005, String::new()));
                    let _ = sink.close().await;
                    break (code, reason);
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => {
                    let message = error.to_string();
                    emitter.emit("Error", message.clone());
                    break (1006, message);
                }
                None => break (1006, "the connection was closed without a close frame".to_owned()),
            },
            command = commands.recv(), if !closing => match command {
                Some(Command::Text(text)) => {
                    if let Err(error) = sink.send(Message::text(text)).await {
                        let message = error.to_string();
                        emitter.emit("Error", message.clone());
                        break (1006, message);
                    }
                }
                Some(Command::Binary(data)) => {
                    if let Err(error) = sink.send(Message::binary(data)).await {
                        let message = error.to_string();
                        emitter.emit("Error", message.clone());
                        break (1006, message);
                    }
                }
                Some(Command::Close(code, reason)) => {
                    closing = true;
                    close_deadline = tokio::time::Instant::now() + CLOSE_TIMEOUT;
                    if sink.send(Message::Close(Some(close_frame(code, reason.clone())))).await.is_err() {
                        break (code, reason);
                    }
                }
                None => {
                    closing = true;
                    close_deadline = tokio::time::Instant::now() + CLOSE_TIMEOUT;
                    let _ = sink.send(Message::Close(None)).await;
                }
            },
            _ = tokio::time::sleep_until(close_deadline), if closing => break (1006, "the peer did not answer the close frame".to_owned()),
        }
    };

    connection.closed();
    emitter.emit("Close", (code, reason));
}

fn listen(lua: &Lua, (port, options): (u16, Options)) -> LuaResult<Result<LuaAnyUserData, String>> {
    let host = options.get_or("Host", "0.0.0.0".to_owned())?;
    let path = options.get::<String>("Path")?;
    let max_connections = options.get_or("MaxConnections", DEFAULT_MAX_CONNECTIONS)?;
    let config = config(&options)?;

    let listener = match bind_tcp(&host, port) {
        Ok(listener) => listener,
        Err(error) => return Ok(Err(error)),
    };
    let local = listener.local_addr().ok();
    let shared = Arc::new(ServerShared {
        port: local.map(|address| address.port()).unwrap_or(port),
        clients: Mutex::new(Vec::new()),
    });

    let (connection, mut commands) = Connection::new(|| ServerCommand::Close);
    connection.open(local, None);
    let object = create_object(
        lua,
        WebSocketServer {
            connection: Arc::clone(&connection),
            shared: Arc::clone(&shared),
        },
    )?;
    object_state(&object)?.raw_set("clients", weak_table(lua)?)?;
    let emitter = Emitter::new(lua, &object)?;
    connection.track(lua, &emitter)?;

    let active = Arc::new(AtomicUsize::new(0));
    runtime().spawn(async move {
        loop {
            tokio::select! {
                accepted = listener.accept() => match accepted {
                    Ok((socket, remote)) => {
                        if active.load(Ordering::Acquire) >= max_connections {
                            drop(socket);
                            continue;
                        }
                        let _ = socket.set_nodelay(true);
                        active.fetch_add(1, Ordering::AcqRel);
                        let emitter = emitter.clone();
                        let shared = Arc::clone(&shared);
                        let active = Arc::clone(&active);
                        let path = path.clone();
                        runtime().spawn(async move {
                            let local = socket.local_addr().ok();
                            let accepted = accept(Box::new(socket), path, config).await;
                            let Ok((socket, request)) = accepted else {
                                active.fetch_sub(1, Ordering::AcqRel);
                                return;
                            };
                            emitter.with(move |lua, server| {
                                let (connection, commands) = Connection::new(close_command);
                                connection.open(local, Some(remote));
                                let client = create_object(lua, WebSocket { connection: Arc::clone(&connection) })?;
                                let client_emitter = Emitter::new(lua, &client)?;
                                connection.track(lua, &client_emitter)?;
                                shared.clients.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(Arc::downgrade(&connection));
                                let clients: LuaTable = object_state(server)?.raw_get("clients")?;
                                clients.raw_set(&client, true)?;

                                runtime().spawn(async move {
                                    drive(socket, connection, client_emitter, commands).await;
                                    active.fetch_sub(1, Ordering::AcqRel);
                                });

                                let info = lua.create_table()?;
                                info.set("Path", request.path)?;
                                info.set("Query", request.query)?;
                                let headers = lua.create_table()?;
                                for (name, value) in request.headers {
                                    headers.set(name, value)?;
                                }
                                info.set("Headers", headers)?;
                                info.set("RemoteAddress", remote.ip().to_string())?;
                                emit_now(lua, server, "Connection", (client, info))
                            });
                        });
                    }
                    Err(error) => {
                        emitter.emit("Error", error.to_string());
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                },
                _ = commands.recv() => break,
            }
        }

        let clients = std::mem::take(&mut *shared.clients.lock().unwrap_or_else(|poisoned| poisoned.into_inner()));
        for client in clients.iter().filter_map(Weak::upgrade) {
            client.close_with(close_command());
        }
        connection.closed();
        emitter.emit("Close", "closed".to_owned());
    });

    Ok(Ok(object))
}

struct RequestInfo {
    path: String,
    query: Option<String>,
    headers: Vec<(String, String)>,
}

#[allow(clippy::result_large_err)]
async fn accept(
    io: BoxedIo,
    expected_path: Option<String>,
    config: WebSocketConfig,
) -> Result<(WebSocketStream<BoxedIo>, RequestInfo), String> {
    let captured = Arc::new(Mutex::new(None));
    let target = Arc::clone(&captured);
    let callback =
        move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
            let path = request.uri().path().to_owned();
            if expected_path
                .as_ref()
                .is_some_and(|expected| *expected != path)
            {
                let mut rejection = ErrorResponse::new(Some("not found".to_owned()));
                *rejection.status_mut() = StatusCode::NOT_FOUND;
                return Err(rejection);
            }
            let headers = request
                .headers()
                .iter()
                .map(|(name, value)| {
                    (
                        name.as_str().to_owned(),
                        String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    )
                })
                .collect();
            *target
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(RequestInfo {
                path,
                query: request.uri().query().map(str::to_owned),
                headers,
            });
            Ok(response)
        };

    let socket = tokio::time::timeout(
        Duration::from_millis(DEFAULT_TIMEOUT),
        tokio_tungstenite::accept_hdr_async_with_config(io, callback, Some(config)),
    )
    .await
    .map_err(|_| "the WebSocket handshake timed out".to_owned())?
    .map_err(|error| error.to_string())?;
    let request = captured
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .take()
        .ok_or("the WebSocket handshake did not provide a request")?;
    Ok((socket, request))
}

fn close_frame(code: u16, reason: String) -> CloseFrame {
    CloseFrame {
        code: CloseCode::from(code),
        reason: reason.into(),
    }
}
