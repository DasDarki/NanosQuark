use mlua::prelude::*;
use quark_core::{Binary, Options, describe, options_and_callback};
use serde::Serialize;
use serde::de::DeserializeOwned;

pub struct Data<V> {
    pub value: V,
    pub preserve_null: bool,
}

impl<V: Serialize> IntoLua for Data<V> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        lua.to_value_with(
            &self.value,
            LuaSerializeOptions::new()
                .serialize_none_to_null(self.preserve_null)
                .serialize_unit_to_null(self.preserve_null)
                .set_array_metatable(true),
        )
    }
}

pub fn read<V: DeserializeOwned>(lua: &Lua, value: LuaValue, options: &Options) -> LuaResult<V> {
    let empty_tables_as_arrays = options.get_or("EmptyTablesAsArrays", false)?;
    lua.from_value_with(
        value,
        LuaDeserializeOptions::new()
            .sort_keys(true)
            .deny_recursive_tables(true)
            .encode_empty_tables_as_array(empty_tables_as_arrays),
    )
    .map_err(|error| LuaError::runtime(format!("cannot encode the value: {}", describe(&error))))
}

pub fn decode<V, F>(
    lua: &Lua,
    (input, options, callback): (Binary, LuaValue, Option<LuaFunction>),
    parse: F,
) -> LuaResult<LuaMultiValue>
where
    V: Serialize + Send + 'static,
    F: FnOnce(&[u8]) -> Result<V, String> + Send + 'static,
{
    let (options, callback) = options_and_callback(options, callback)?;
    decode_with(lua, input, &options, callback, parse)
}

pub fn decode_with<V, F>(
    lua: &Lua,
    input: Binary,
    options: &Options,
    callback: Option<LuaFunction>,
    parse: F,
) -> LuaResult<LuaMultiValue>
where
    V: Serialize + Send + 'static,
    F: FnOnce(&[u8]) -> Result<V, String> + Send + 'static,
{
    let preserve_null = options.get_or("PreserveNull", false)?;
    let bytes = input.0;
    quark_core::run(lua, callback, move || {
        parse(&bytes).map(|value| Data {
            value,
            preserve_null,
        })
    })
}

pub fn encode<V, O, F>(
    lua: &Lua,
    (value, options, callback): (LuaValue, LuaValue, Option<LuaFunction>),
    serialize: F,
) -> LuaResult<LuaMultiValue>
where
    V: DeserializeOwned + Send + 'static,
    O: IntoLua + Send + 'static,
    F: FnOnce(V, &Options) -> LuaResult<Box<dyn FnOnce() -> Result<O, String> + Send>>,
{
    let (options, callback) = options_and_callback(options, callback)?;
    let value = read::<V>(lua, value, &options)?;
    let work = serialize(value, &options)?;
    quark_core::run(lua, callback, work)
}

pub fn text(bytes: &[u8]) -> Result<&str, String> {
    std::str::from_utf8(bytes).map_err(|_| "the input is not valid UTF-8".to_owned())
}

pub fn add_values(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set("Null", LuaValue::NULL)?;
    module.set(
        "Array",
        lua.create_function(|lua, table: Option<LuaTable>| {
            let table = match table {
                Some(table) => table,
                None => lua.create_table()?,
            };
            table.set_metatable(Some(lua.array_metatable()))?;
            Ok(table)
        })?,
    )?;
    module.set(
        "IsNull",
        lua.create_function(|_, value: LuaValue| Ok(value == LuaValue::NULL))?,
    )?;
    Ok(())
}
