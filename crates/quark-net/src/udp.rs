use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use mlua::prelude::*;
use quark_core::{Binary, Bytes, Emitter, Options, create_object, runtime};

use crate::connection::{Connected, Connection, add_connection_methods};

const MAX_DATAGRAM_SIZE: usize = 65_507;

pub enum Command {
    Send(Vec<u8>, String, u16),
    Close,
}

pub struct UdpSocket {
    connection: Arc<Connection<Command>>,
}

impl Connected for UdpSocket {
    type Command = Command;

    fn connection(&self) -> &Arc<Connection<Command>> {
        &self.connection
    }
}

impl LuaUserData for UdpSocket {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        add_connection_methods(methods);
        methods.add_method(
            "Send",
            |_, socket, (data, host, port): (Binary, String, u16)| {
                if data.as_bytes().len() > MAX_DATAGRAM_SIZE {
                    return Err(LuaError::runtime(format!(
                        "datagrams cannot be larger than {MAX_DATAGRAM_SIZE} bytes"
                    )));
                }
                Ok(socket.connection.send(Command::Send(data.0, host, port)))
            },
        );
        methods.add_method("Close", |_, socket, ()| {
            socket.connection.close_with(Command::Close);
            Ok(())
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, socket, ()| {
            Ok(format!("UDPSocket({})", socket.connection.state_name()))
        });
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Bind", lua.create_function(bind)?)?;
    Ok(module)
}

fn bind(
    lua: &Lua,
    (port, options): (Option<u16>, Options),
) -> LuaResult<Result<LuaAnyUserData, String>> {
    let host = options.get_or("Host", "0.0.0.0".to_owned())?;
    let port = port.unwrap_or(0);
    if port != 0
        && let Err(error) = quark_core::check_listen(port)
    {
        return Ok(Err(error));
    }

    let socket = match std::net::UdpSocket::bind((host.as_str(), port)).and_then(|socket| {
        socket.set_nonblocking(true)?;
        let _context = runtime().enter();
        tokio::net::UdpSocket::from_std(socket)
    }) {
        Ok(socket) => socket,
        Err(error) => return Ok(Err(format!("cannot bind {host}:{port}: {error}"))),
    };

    let (connection, mut commands) = Connection::new(|| Command::Close);
    connection.open(socket.local_addr().ok(), None);
    let object = create_object(
        lua,
        UdpSocket {
            connection: Arc::clone(&connection),
        },
    )?;
    let emitter = Emitter::new(lua, &object)?;
    connection.track(lua, &emitter)?;

    runtime().spawn(async move {
        let mut buffer = vec![0; 65_536];
        let reason = loop {
            tokio::select! {
                received = socket.recv_from(&mut buffer) => match received {
                    Ok((count, from)) => {
                        emitter.emit("Message", (Bytes(buffer[..count].to_vec()), from.ip().to_string(), from.port()));
                    }
                    Err(error) => emitter.emit("Error", error.to_string()),
                },
                command = commands.recv() => match command {
                    Some(Command::Send(data, host, port)) => {
                        let target = match host.parse::<IpAddr>() {
                            Ok(ip) => Ok(SocketAddr::new(ip, port)),
                            Err(_) => tokio::net::lookup_host((host.as_str(), port))
                                .await
                                .map_err(|error| error.to_string())
                                .and_then(|mut addresses| addresses.next().ok_or_else(|| format!("cannot resolve '{host}'"))),
                        };
                        let target = target.and_then(|target| {
                            quark_core::check_connect(&host, port, &[target.ip()]).map(|()| target)
                        });
                        match target {
                            Ok(target) => {
                                if let Err(error) = socket.send_to(&data, target).await {
                                    emitter.emit("Error", format!("cannot send to {target}: {error}"));
                                }
                            }
                            Err(error) => emitter.emit("Error", error),
                        }
                    }
                    Some(Command::Close) | None => break "closed".to_owned(),
                },
            }
        };
        connection.closed();
        emitter.emit("Close", reason);
    });

    Ok(Ok(object))
}
