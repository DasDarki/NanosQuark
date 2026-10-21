use std::net::SocketAddr;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, Weak};

use mlua::prelude::*;
use quark_core::{Emitter, Resource};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

const CONNECTING: u8 = 0;
const OPEN: u8 = 1;
const CLOSING: u8 = 2;
const CLOSED: u8 = 3;

pub struct Connection<C> {
    state: AtomicU8,
    commands: UnboundedSender<C>,
    close_command: fn() -> C,
    addresses: Mutex<(Option<SocketAddr>, Option<SocketAddr>)>,
}

impl<C: Send + 'static> Connection<C> {
    pub fn new(close_command: fn() -> C) -> (Arc<Self>, UnboundedReceiver<C>) {
        let (commands, receiver) = unbounded_channel();
        let connection = Arc::new(Connection {
            state: AtomicU8::new(CONNECTING),
            commands,
            close_command,
            addresses: Mutex::new((None, None)),
        });
        (connection, receiver)
    }

    pub fn open(&self, local: Option<SocketAddr>, remote: Option<SocketAddr>) {
        *self
            .addresses
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = (local, remote);
        let _ = self
            .state
            .compare_exchange(CONNECTING, OPEN, Ordering::AcqRel, Ordering::Acquire);
    }

    pub fn reconnecting(&self) {
        let _ = self
            .state
            .compare_exchange(OPEN, CONNECTING, Ordering::AcqRel, Ordering::Acquire);
    }

    pub fn closed(&self) {
        self.state.store(CLOSED, Ordering::Release);
    }

    pub fn is_open(&self) -> bool {
        self.state.load(Ordering::Acquire) == OPEN
    }

    pub fn is_finished(&self) -> bool {
        self.state.load(Ordering::Acquire) >= CLOSING
    }

    pub fn state_name(&self) -> &'static str {
        match self.state.load(Ordering::Acquire) {
            CONNECTING => "connecting",
            OPEN => "open",
            CLOSING => "closing",
            _ => "closed",
        }
    }

    pub fn send(&self, command: C) -> bool {
        if self.is_finished() {
            return false;
        }
        self.commands.send(command).is_ok()
    }

    pub fn close_with(&self, command: C) {
        let previous = self.state.fetch_max(CLOSING, Ordering::AcqRel);
        if previous < CLOSING {
            let _ = self.commands.send(command);
        }
    }

    pub fn local_address(&self) -> Option<SocketAddr> {
        self.addresses
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .0
    }

    pub fn remote_address(&self) -> Option<SocketAddr> {
        self.addresses
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .1
    }

    pub fn track(self: &Arc<Self>, lua: &Lua, emitter: &Emitter) -> LuaResult<()> {
        let weak: Weak<dyn Resource> = Arc::downgrade(self) as Weak<dyn Resource>;
        emitter.dispatcher().track(lua, weak)
    }
}

impl<C: Send + 'static> Resource for Connection<C> {
    fn close(&self) {
        self.close_with((self.close_command)());
    }
}

pub trait Connected {
    type Command: Send + 'static;

    fn connection(&self) -> &Arc<Connection<Self::Command>>;
}

pub fn address(address: Option<SocketAddr>) -> (Option<String>, Option<u16>) {
    match address {
        Some(address) => (Some(address.ip().to_string()), Some(address.port())),
        None => (None, None),
    }
}

pub fn add_connection_methods<T: Connected + 'static, M: LuaUserDataMethods<T>>(methods: &mut M) {
    methods.add_method("GetState", |_, object, ()| {
        Ok(object.connection().state_name())
    });
    methods.add_method("IsOpen", |_, object, ()| Ok(object.connection().is_open()));
    methods.add_method("GetRemoteAddress", |_, object, ()| {
        Ok(address(object.connection().remote_address()))
    });
    methods.add_method("GetLocalAddress", |_, object, ()| {
        Ok(address(object.connection().local_address()))
    });
    quark_core::add_event_methods(methods);
}

pub fn weak_table(lua: &Lua) -> LuaResult<LuaTable> {
    let table = lua.create_table()?;
    let metatable = lua.create_table()?;
    metatable.set("__mode", "k")?;
    table.set_metatable(Some(metatable))?;
    Ok(table)
}

pub fn open_clients<T: Connected + 'static>(
    lua: &Lua,
    server: &LuaAnyUserData,
) -> LuaResult<LuaTable> {
    let clients: LuaTable = quark_core::object_state(server)?.raw_get("clients")?;
    let open = lua.create_table()?;
    for pair in clients.pairs::<LuaAnyUserData, bool>() {
        let (client, _) = pair?;
        if client.borrow::<T>()?.connection().is_open() {
            open.raw_push(client)?;
        }
    }
    Ok(open)
}

pub fn bind_tcp(host: &str, port: u16) -> Result<tokio::net::TcpListener, String> {
    let listener = std::net::TcpListener::bind((host, port))
        .map_err(|error| format!("cannot listen on {host}:{port}: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("cannot listen on {host}:{port}: {error}"))?;
    let _context = quark_core::runtime().enter();
    tokio::net::TcpListener::from_std(listener)
        .map_err(|error| format!("cannot listen on {host}:{port}: {error}"))
}
