use std::cell::{Cell, RefCell};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use mlua::AppDataRef;
use mlua::prelude::*;

use crate::describe;

const TICK_BUDGET: Duration = Duration::from_millis(4);
const FRAME_WINDOW: Duration = Duration::from_millis(5);

type Job = Box<dyn FnOnce(&Lua) -> LuaResult<()> + Send>;

pub trait Resource: Send + Sync {
    fn close(&self);
}

struct ResourceList(RefCell<Vec<Weak<dyn Resource>>>);

impl LuaUserData for ResourceList {}

#[derive(Clone)]
pub struct Dispatcher {
    sender: Sender<Job>,
    owner: Arc<LuaRegistryKey>,
}

struct State {
    sender: Sender<Job>,
    receiver: Receiver<Job>,
    pump: LuaFunction,
    environments: LuaTable,
    resources: LuaTable,
    current: RefCell<Option<LuaTable>>,
    window_start: Cell<Instant>,
}

impl Dispatcher {
    pub fn get(lua: &Lua) -> LuaResult<Self> {
        let state = state(lua)?;
        let environment = caller_environment(lua)
            .or_else(|| state.current.borrow().clone())
            .ok_or_else(|| {
                LuaError::runtime("Quark async functions have to be called from a package script")
            })?;
        attach(lua, &state, &environment)?;

        Ok(Dispatcher {
            sender: state.sender.clone(),
            owner: Arc::new(lua.create_registry_value(environment)?),
        })
    }

    pub fn post(&self, job: impl FnOnce(&Lua) -> LuaResult<()> + Send + 'static) {
        let owner = Arc::clone(&self.owner);
        let _ = self.sender.send(Box::new(move |lua: &Lua| {
            let environment: LuaTable = lua.registry_value(&owner)?;
            if !is_alive(lua, &environment)? {
                return Ok(());
            }

            let previous = set_current(lua, Some(environment));
            let result = job(lua);
            set_current(lua, previous);
            result
        }));
    }

    pub fn track(&self, lua: &Lua, resource: Weak<dyn Resource>) -> LuaResult<()> {
        let state = state(lua)?;
        let environment: LuaTable = lua.registry_value(&self.owner)?;
        let list = match state
            .resources
            .raw_get::<Option<LuaAnyUserData>>(&environment)?
        {
            Some(list) => list,
            None => {
                let list = lua.create_userdata(ResourceList(RefCell::new(Vec::new())))?;
                state.resources.raw_set(&environment, &list)?;
                list
            }
        };

        let list = list.borrow::<ResourceList>()?;
        let mut resources = list.0.borrow_mut();
        if resources.len() >= 64 && resources.len().is_power_of_two() {
            resources.retain(|resource| resource.strong_count() > 0);
        }
        resources.push(resource);
        Ok(())
    }
}

fn close_resources(resources: &LuaTable, environment: &LuaTable) -> LuaResult<()> {
    let Some(list) = resources.raw_get::<Option<LuaAnyUserData>>(environment)? else {
        return Ok(());
    };
    let open = list.borrow::<ResourceList>()?.0.take();
    for resource in open.iter().filter_map(Weak::upgrade) {
        resource.close();
    }
    resources.raw_set(environment, LuaNil)
}

fn is_alive(lua: &Lua, environment: &LuaTable) -> LuaResult<bool> {
    match lua.app_data_ref::<State>() {
        Some(state) => Ok(state
            .environments
            .raw_get::<Option<bool>>(environment)?
            .unwrap_or(true)),
        None => Ok(false),
    }
}

pub(crate) fn enter_environment(
    lua: &Lua,
    environment: Option<LuaTable>,
) -> LuaResult<Option<LuaTable>> {
    let state = state(lua)?;
    Ok(state.current.replace(environment))
}

fn set_current(lua: &Lua, environment: Option<LuaTable>) -> Option<LuaTable> {
    match lua.app_data_ref::<State>() {
        Some(state) => state.current.replace(environment),
        None => None,
    }
}

pub fn report_error(lua: &Lua, error: &LuaError) {
    report_message(lua, &describe(error));
}

pub(crate) fn report_message(lua: &Lua, message: &str) {
    let message = format!("[Quark] {message}");
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

    let resources = lua.create_table()?;
    let weak_resource_keys = lua.create_table()?;
    weak_resource_keys.set("__mode", "k")?;
    resources.set_metatable(Some(weak_resource_keys))?;

    let (sender, receiver) = mpsc::channel();
    lua.set_app_data(State {
        sender,
        receiver,
        pump: lua.create_function(|lua, ()| pump(lua))?,
        environments,
        resources,
        current: RefCell::new(None),
        window_start: Cell::new(Instant::now()),
    });

    lua.app_data_ref::<State>()
        .ok_or_else(|| LuaError::runtime("failed to initialize the Quark dispatcher"))
}

pub(crate) fn caller_environment(lua: &Lua) -> Option<LuaTable> {
    let globals = lua.globals();
    for level in 1.. {
        let environment = lua.inspect_stack(level, |debug| debug.function().environment())?;
        if let Some(environment) = environment
            && environment != globals
        {
            return Some(environment);
        }
    }
    None
}

pub fn package_environment(lua: &Lua) -> Option<LuaTable> {
    caller_environment(lua).or_else(|| {
        lua.app_data_ref::<State>()
            .and_then(|state| state.current.borrow().clone())
    })
}

fn attach(lua: &Lua, state: &State, environment: &LuaTable) -> LuaResult<()> {
    if state
        .environments
        .raw_get::<Option<bool>>(environment)?
        .is_some()
    {
        return Ok(());
    }

    let server: LuaTable = environment.get("Server")?;
    let subscribe: LuaFunction = server.get("Subscribe")?;
    subscribe.call::<LuaValue>(("Tick", &state.pump))?;

    let environments = state.environments.clone();
    let resources = state.resources.clone();
    let unloaded_environment = environment.clone();
    let on_unload = lua.create_function(move |_, ()| {
        environments.raw_set(&unloaded_environment, false)?;
        close_resources(&resources, &unloaded_environment)
    })?;

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
