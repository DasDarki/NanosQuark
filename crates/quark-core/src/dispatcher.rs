use std::cell::Cell;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use mlua::AppDataRef;
use mlua::prelude::*;

use crate::describe;

const TICK_BUDGET: Duration = Duration::from_millis(4);
const FRAME_WINDOW: Duration = Duration::from_millis(5);

type Job = Box<dyn FnOnce(&Lua) -> LuaResult<()> + Send>;

#[derive(Clone)]
pub struct Dispatcher {
    sender: Sender<Job>,
}

struct State {
    sender: Sender<Job>,
    receiver: Receiver<Job>,
    pump: LuaFunction,
    environments: LuaTable,
    window_start: Cell<Instant>,
}

impl Dispatcher {
    pub fn get(lua: &Lua) -> LuaResult<Self> {
        let state = state(lua)?;
        let environment = caller_environment(lua).ok_or_else(|| {
            LuaError::runtime("Quark async functions have to be called from a package script")
        })?;
        attach(lua, &state, environment)?;

        Ok(Dispatcher {
            sender: state.sender.clone(),
        })
    }

    pub fn post(&self, job: impl FnOnce(&Lua) -> LuaResult<()> + Send + 'static) {
        let _ = self.sender.send(Box::new(job));
    }
}

pub fn is_alive(lua: &Lua, function: &LuaFunction) -> LuaResult<bool> {
    let Some(environment) = function.environment() else {
        return Ok(true);
    };

    match lua.app_data_ref::<State>() {
        Some(state) => Ok(state
            .environments
            .raw_get::<Option<bool>>(environment)?
            .unwrap_or(true)),
        None => Ok(true),
    }
}

pub fn report_error(lua: &Lua, error: &LuaError) {
    let message = format!("[Quark] {}", describe(error));
    let logged = lua
        .globals()
        .get::<LuaTable>("Console")
        .and_then(|console| console.get::<LuaFunction>("Error"))
        .and_then(|console_error| console_error.call::<()>(message.as_str()));

    if logged.is_err() {
        eprintln!("{message}");
    }
}

fn state(lua: &Lua) -> LuaResult<AppDataRef<'_, State>> {
    if let Some(state) = lua.app_data_ref::<State>() {
        return Ok(state);
    }

    let environments = lua.create_table()?;
    let weak_keys = lua.create_table()?;
    weak_keys.set("__mode", "k")?;
    environments.set_metatable(Some(weak_keys))?;

    let (sender, receiver) = mpsc::channel();
    lua.set_app_data(State {
        sender,
        receiver,
        pump: lua.create_function(|lua, ()| pump(lua))?,
        environments,
        window_start: Cell::new(Instant::now()),
    });

    lua.app_data_ref::<State>()
        .ok_or_else(|| LuaError::runtime("failed to initialize the Quark dispatcher"))
}

fn caller_environment(lua: &Lua) -> Option<LuaTable> {
    for level in 1.. {
        let environment = lua.inspect_stack(level, |debug| debug.function().environment())?;
        if environment.is_some() {
            return environment;
        }
    }
    None
}

fn attach(lua: &Lua, state: &State, environment: LuaTable) -> LuaResult<()> {
    if state
        .environments
        .raw_get::<Option<bool>>(&environment)?
        .is_some()
    {
        return Ok(());
    }

    let server: LuaTable = environment.get("Server")?;
    let subscribe: LuaFunction = server.get("Subscribe")?;
    subscribe.call::<LuaValue>(("Tick", &state.pump))?;

    let environments = state.environments.clone();
    let unloaded_environment = environment.clone();
    let on_unload =
        lua.create_function(move |_, ()| environments.raw_set(&unloaded_environment, false))?;

    let package: LuaTable = environment.get("Package")?;
    let subscribe: LuaFunction = package.get("Subscribe")?;
    subscribe.call::<LuaValue>(("Unload", on_unload))?;

    state.environments.raw_set(environment, true)
}

fn pump(lua: &Lua) -> LuaResult<()> {
    let deadline = {
        let Some(state) = lua.app_data_ref::<State>() else {
            return Ok(());
        };

        let now = Instant::now();
        if now.duration_since(state.window_start.get()) > FRAME_WINDOW {
            state.window_start.set(now);
        }
        state.window_start.get() + TICK_BUDGET
    };

    while Instant::now() < deadline {
        let next = match lua.app_data_ref::<State>() {
            Some(state) => state.receiver.try_recv(),
            None => return Ok(()),
        };

        let Ok(job) = next else {
            break;
        };

        if let Err(error) = job(lua) {
            report_error(lua, &error);
        }
    }

    Ok(())
}
