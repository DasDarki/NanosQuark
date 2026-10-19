use std::any::TypeId;
use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::c_int;

use mlua::ffi;
use mlua::prelude::*;

const MAX_TABLE_DEPTH: usize = 8;

struct Describer(LuaFunction);

#[derive(Default)]
struct ProtectedTypes(RefCell<HashSet<TypeId>>);

pub fn protect(lua: &Lua, function: LuaFunction) -> LuaResult<LuaFunction> {
    let describer = describer(lua)?;
    unsafe {
        lua.exec_raw((function, describer), |state| {
            ffi::lua_pushcclosure(state, trampoline, 2);
        })
    }
}

pub fn protect_table(lua: &Lua, table: &LuaTable) -> LuaResult<()> {
    protect_table_at_depth(lua, table, 0)
}

pub fn create_userdata<T: LuaUserData + 'static>(lua: &Lua, value: T) -> LuaResult<LuaAnyUserData> {
    let userdata = lua.create_userdata(value)?;

    let first_of_type = {
        if lua.app_data_ref::<ProtectedTypes>().is_none() {
            lua.set_app_data(ProtectedTypes::default());
        }
        let protected = lua
            .app_data_ref::<ProtectedTypes>()
            .ok_or_else(|| LuaError::runtime("failed to track protected userdata types"))?;
        protected.0.borrow_mut().insert(TypeId::of::<T>())
    };

    if first_of_type {
        protect_metatable(lua, &userdata)?;
    }

    Ok(userdata)
}

pub fn describe(error: &LuaError) -> String {
    match error {
        LuaError::CallbackError { cause, .. } => describe(cause),
        LuaError::RuntimeError(message) => message.clone(),
        LuaError::ExternalError(error) => error.to_string(),
        LuaError::WithContext { context, cause } => format!("{context}: {}", describe(cause)),
        LuaError::BadArgument {
            to,
            pos,
            name,
            cause,
        } => {
            let function = to
                .as_ref()
                .map(|to| format!(" to '{to}'"))
                .unwrap_or_default();
            let argument = name
                .as_ref()
                .map(|name| format!(" ({name})"))
                .unwrap_or_default();
            format!(
                "bad argument #{pos}{argument}{function}: {}",
                describe(cause)
            )
        }
        other => other.to_string(),
    }
}

fn describe_value(value: LuaValue) -> String {
    match value {
        LuaValue::Error(error) => describe(&error),
        LuaValue::String(message) => message.to_string_lossy(),
        other => other
            .to_string()
            .unwrap_or_else(|_| format!("error object of type {}", other.type_name())),
    }
}

fn describer(lua: &Lua) -> LuaResult<LuaFunction> {
    if let Some(describer) = lua.app_data_ref::<Describer>() {
        return Ok(describer.0.clone());
    }

    let function = lua.create_function(|_, value: LuaValue| Ok(describe_value(value)))?;
    lua.set_app_data(Describer(function.clone()));
    Ok(function)
}

fn protect_table_at_depth(lua: &Lua, table: &LuaTable, depth: usize) -> LuaResult<()> {
    if depth > MAX_TABLE_DEPTH {
        return Ok(());
    }

    let entries = table
        .pairs::<LuaValue, LuaValue>()
        .collect::<LuaResult<Vec<_>>>()?;

    for (key, value) in entries {
        match value {
            LuaValue::Function(function) => table.raw_set(key, protect(lua, function)?)?,
            LuaValue::Table(nested) => protect_table_at_depth(lua, &nested, depth + 1)?,
            _ => {}
        }
    }

    Ok(())
}

fn protect_metatable(lua: &Lua, userdata: &LuaAnyUserData) -> LuaResult<()> {
    let metatable = userdata.metatable()?;
    let entries = metatable
        .pairs::<LuaValue>()
        .collect::<LuaResult<Vec<_>>>()?;

    for (name, value) in entries {
        match (name.as_str(), value) {
            ("__index", LuaValue::Table(methods)) => protect_table(lua, &methods)?,
            ("__index" | "__newindex", _) => {}
            (_, LuaValue::Function(function)) => metatable.set(&name, protect(lua, function)?)?,
            _ => {}
        }
    }

    Ok(())
}

unsafe extern "C-unwind" fn trampoline(state: *mut ffi::lua_State) -> c_int {
    unsafe {
        let argument_count = ffi::lua_gettop(state);
        ffi::lua_pushvalue(state, ffi::lua_upvalueindex(1));
        ffi::lua_insert(state, 1);

        if ffi::lua_pcall(state, argument_count, ffi::LUA_MULTRET, 0) == ffi::LUA_OK {
            return ffi::lua_gettop(state);
        }

        ffi::lua_pushvalue(state, ffi::lua_upvalueindex(2));
        ffi::lua_insert(state, -2);
        if ffi::lua_pcall(state, 1, 1, 0) != ffi::LUA_OK
            || ffi::lua_type(state, -1) != ffi::LUA_TSTRING
        {
            ffi::lua_settop(state, 0);
            ffi::lua_pushstring(state, c"unknown native error".as_ptr());
        }

        ffi::luaL_where(state, 1);
        ffi::lua_insert(state, -2);
        ffi::lua_concat(state, 2);
        ffi::lua_error(state)
    }
}
