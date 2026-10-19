use mlua::prelude::*;

use crate::create_userdata;

pub struct Bytes(pub Vec<u8>);

impl IntoLua for Bytes {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.create_string(self.0).map(LuaValue::String)
    }
}

pub struct Object<T>(pub T);

impl<T: LuaUserData + 'static> IntoLua for Object<T> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        create_userdata(lua, self.0).map(LuaValue::UserData)
    }
}
