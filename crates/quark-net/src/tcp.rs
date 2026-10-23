use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use mlua::prelude::*;
use quark_core::{Binary, Bytes, Emitter, Options, create_object, emit_now, object_state, runtime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::mpsc::UnboundedReceiver;

use crate::connection::{
    Connected, Connection, add_connection_methods, bind_tcp, open_clients, weak_table,
};
use crate::framing::{Decoder, Framing};
use crate::stream::{self, BoxedIo};
use crate::tls;

const DEFAULT_TIMEOUT: u64 = 10_000;
const DEFAULT_MAX_CONNECTIONS: usize = 1024;

pub enum Command {
    Send(Vec<u8>),
    Close,
}

pub enum ServerCommand {
    Close,
}

pub struct TcpSocket {
    connection: Arc<Connection<Command>>,
    framing: Framing,
    max_frame_size: usize,
}

impl Connected for TcpSocket {
    type Command = Command;

    fn connection(&self) -> &Arc<Connection<Command>> {
        &self.connection
    }
}

impl LuaUserData for TcpSocket {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        add_connection_methods(methods);
        methods.add_method("Send", |_, socket, data: Binary| {
            let frame = socket
                .framing
                .encode(data.as_bytes(), socket.max_frame_size)?;
            Ok(socket.connection.send(Command::Send(frame)))
        });
        methods.add_method("Close", |_, socket, ()| {
            socket.connection.close_with(Command::Close);
            Ok(())
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, socket, ()| {
            Ok(format!("TCPSocket({})", socket.connection.state_name()))
        });
    }
}

struct ServerShared {
    port: u16,
    clients: Mutex<Vec<Weak<Connection<Command>>>>,
}

pub struct TcpServer {
    connection: Arc<Connection<ServerCommand>>,
    shared: Arc<ServerShared>,
}

impl Connected for TcpServer {
    type Command = ServerCommand;

    fn connection(&self) -> &Arc<Connection<ServerCommand>> {
        &self.connection
    }
}

impl LuaUserData for TcpServer {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        add_connection_methods(methods);
        methods.add_method("GetPort", |_, server, ()| Ok(server.shared.port));
        methods.add_function("GetClients", |lua, this: LuaAnyUserData| {
            open_clients::<TcpSocket>(lua, &this)
        });
        methods.add_method("Close", |_, server, ()| {
            server.connection.close_with(ServerCommand::Close);
            Ok(())
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, server, ()| {
            Ok(format!(
                "TCPServer({}, port {})",
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

fn connect(lua: &Lua, (host, port, options): (String, u16, Options)) -> LuaResult<LuaAnyUserData> {
    let (framing, max_frame_size) = Framing::from_options(&options)?;
    let timeout = Duration::from_millis(options.get_or("Timeout", DEFAULT_TIMEOUT)?);
    let tls = tls::client(&options, &host, false)?;

    let (connection, commands) = Connection::new(|| Command::Close);
    let object = create_object(
        lua,
        TcpSocket {
            connection: Arc::clone(&connection),
            framing,
            max_frame_size,
        },
    )?;
    let emitter = Emitter::new(lua, &object)?;
    connection.track(lua, &emitter)?;

    runtime().spawn(async move {
        match stream::connect(&host, port, tls, timeout).await {
            Ok(connected) => {
                connection.open(connected.local, connected.remote);
                emitter.emit("Connect", ());
                drive(
                    connected.io,
                    connection,
                    emitter,
                    framing,
                    max_frame_size,
                    commands,
                )
                .await;
            }
            Err(error) => {
                connection.closed();
                emitter.emit("Error", error.clone());
                emitter.emit("Close", error);
            }
        }
    });

    Ok(object)
}

async fn drive(
    io: BoxedIo,
    connection: Arc<Connection<Command>>,
    emitter: Emitter,
    framing: Framing,
    max_frame_size: usize,
    mut commands: UnboundedReceiver<Command>,
) {
    let (mut reader, mut writer) = tokio::io::split(io);
    let mut decoder = Decoder::new(framing, max_frame_size);
    let mut buffer = vec![0; 16 * 1024];

    let reason = loop {
        tokio::select! {
            read = reader.read(&mut buffer) => match read {
                Ok(0) => break "closed by peer".to_owned(),
                Ok(count) => match decoder.push(&buffer[..count]) {
                    Ok(frames) => {
                        for frame in frames {
                            emitter.emit("Data", Bytes(frame));
                        }
                    }
                    Err(error) => {
                        emitter.emit("Error", error.clone());
                        break error;
                    }
                },
                Err(error) => {
                    let message = error.to_string();
                    emitter.emit("Error", message.clone());
                    break message;
                }
            },
            command = commands.recv() => match command {
                Some(Command::Send(data)) => {
                    if let Err(error) = writer.write_all(&data).await {
                        let message = error.to_string();
                        emitter.emit("Error", message.clone());
                        break message;
                    }
                }
                Some(Command::Close) | None => {
                    let _ = writer.shutdown().await;
                    break "closed".to_owned();
                }
            },
        }
    };

    connection.closed();
    emitter.emit("Close", reason);
}

fn listen(lua: &Lua, (port, options): (u16, Options)) -> LuaResult<Result<LuaAnyUserData, String>> {
    let host = options.get_or("Host", "0.0.0.0".to_owned())?;
    let (framing, max_frame_size) = Framing::from_options(&options)?;
    let max_connections = options.get_or("MaxConnections", DEFAULT_MAX_CONNECTIONS)?;
    let acceptor = tls::server(&options, &[])?;

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
        TcpServer {
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
                        let local = socket.local_addr().ok();
                        let shared = Arc::clone(&shared);
                        let active = Arc::clone(&active);
                        let emitter = emitter.clone();
                        let acceptor = acceptor.clone();
                        active.fetch_add(1, Ordering::AcqRel);

                        runtime().spawn(async move {
                        let accepted = match stream::accept(socket, acceptor, Duration::from_millis(DEFAULT_TIMEOUT)).await {
                            Ok(accepted) => accepted,
                            Err(error) => {
                                emitter.emit("Error", error);
                                active.fetch_sub(1, Ordering::AcqRel);
                                return;
                            }
                        };
                        emitter.with(move |lua, server| {
                            let (connection, commands) = Connection::new(|| Command::Close);
                            connection.open(local, Some(remote));
                            connection.set_peer(accepted.peer);
                            let io = accepted.io;
                            let client = create_object(lua, TcpSocket { connection: Arc::clone(&connection), framing, max_frame_size })?;
                            let client_emitter = Emitter::new(lua, &client)?;
                            connection.track(lua, &client_emitter)?;
                            shared.clients.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(Arc::downgrade(&connection));
                            let clients: LuaTable = object_state(server)?.raw_get("clients")?;
                            clients.raw_set(&client, true)?;

                            runtime().spawn(async move {
                                drive(io, connection, client_emitter, framing, max_frame_size, commands).await;
                                active.fetch_sub(1, Ordering::AcqRel);
                            });
                            emit_now(lua, server, "Connection", client)
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
            client.close_with(Command::Close);
        }
        connection.closed();
        emitter.emit("Close", "closed".to_owned());
    });

    Ok(Ok(object))
}
