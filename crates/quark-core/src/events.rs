use std::sync::Arc;

use mlua::prelude::*;

use crate::{Dispatcher, create_userdata, report_error};

const HANDLERS: &str = "handlers";

pub fn create_object<T: LuaUserData + 'static>(lua: &Lua, value: T) -> LuaResult<LuaAnyUserData> {
    let userdata = create_userdata(lua, value)?;
    let state = lua.create_table()?;
    state.raw_set(HANDLERS, lua.create_table()?)?;
    userdata.set_user_value(state)?;
    Ok(userdata)
}

pub fn object_state(object: &LuaAnyUserData) -> LuaResult<LuaTable> {
    object.user_value::<LuaTable>()
}

pub fn add_event_methods<T: 'static, M: LuaUserDataMethods<T>>(methods: &mut M) {
    methods.add_function(
        "Subscribe",
        |lua, (this, event, handler): (LuaAnyUserData, String, LuaFunction)| {
            let handlers: LuaTable = object_state(&this)?.raw_get(HANDLERS)?;
            let list = match handlers.raw_get::<Option<LuaTable>>(event.as_str())? {
                Some(list) => list,
                None => {
                    let list = lua.create_table()?;
                    handlers.raw_set(event.as_str(), &list)?;
                    list
                }
            };
            list.raw_push(&handler)?;
            Ok(handler)
        },
    );

    methods.add_function(
        "Unsubscribe",
        |lua, (this, event, handler): (LuaAnyUserData, String, Option<LuaFunction>)| {
            let handlers: LuaTable = object_state(&this)?.raw_get(HANDLERS)?;
            let Some(handler) = handler else {
                return handlers.raw_set(event.as_str(), LuaNil);
            };
            let Some(list) = handlers.raw_get::<Option<LuaTable>>(event.as_str())? else {
                return Ok(());
            };
            let remaining = list
                .sequence_values::<LuaFunction>()
                .filter(|existing| {
                    existing
                        .as_ref()
                        .map_or(true, |existing| *existing != handler)
                })
                .collect::<LuaResult<Vec<_>>>()?;
            handlers.raw_set(event.as_str(), lua.create_sequence_from(remaining)?)
        },
    );
}

pub fn emit_now(
    lua: &Lua,
    target: &LuaAnyUserData,
    event: &str,
    arguments: impl IntoLuaMulti,
) -> LuaResult<()> {
    let handlers: LuaTable = object_state(target)?.raw_get(HANDLERS)?;
    let Some(list) = handlers.raw_get::<Option<LuaTable>>(event)? else {
        return Ok(());
    };

    let arguments = arguments.into_lua_multi(lua)?;
    let handlers = list
        .sequence_values::<LuaFunction>()
        .collect::<LuaResult<Vec<_>>>()?;
    for handler in handlers {
        if let Err(error) = handler.call::<()>(arguments.clone()) {
            report_error(lua, &error);
        }
    }
    Ok(())
}

#[derive(Clone)]
pub struct Emitter {
    dispatcher: Dispatcher,
    target: Arc<LuaRegistryKey>,
}

impl Emitter {
    pub fn new(lua: &Lua, target: &LuaAnyUserData) -> LuaResult<Self> {
        Ok(Emitter {
            dispatcher: Dispatcher::get(lua)?,
            target: Arc::new(lua.create_registry_value(target.clone())?),
        })
    }

    pub fn dispatcher(&self) -> &Dispatcher {
        &self.dispatcher
    }

    pub fn emit<A: IntoLuaMulti + Send + 'static>(&self, event: &'static str, arguments: A) {
        self.with(move |lua, target| emit_now(lua, target, event, arguments));
    }

    pub fn with<F>(&self, job: F)
    where
        F: FnOnce(&Lua, &LuaAnyUserData) -> LuaResult<()> + Send + 'static,
    {
        let target = Arc::clone(&self.target);
        self.dispatcher.post(move |lua| {
            let target: LuaAnyUserData = lua.registry_value(&target)?;
            job(lua, &target)
        });
    }
}
