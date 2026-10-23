use mlua::prelude::*;
use quark_core::Bytes;

pub struct Entry {
    pub name: String,
    pub data: Vec<u8>,
    pub directory: bool,
    pub safe_path: Option<String>,
    pub size: u64,
    pub compressed_size: Option<u64>,
    pub encrypted: bool,
}

pub struct Entries {
    pub entries: Vec<Entry>,
    pub with_data: bool,
}

impl IntoLua for Entries {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let list = lua.create_table()?;
        for entry in self.entries {
            let table = lua.create_table()?;
            table.set("Name", entry.name)?;
            table.set("IsDirectory", entry.directory)?;
            table.set("SafePath", entry.safe_path)?;
            table.set("Size", entry.size)?;
            table.set("CompressedSize", entry.compressed_size)?;
            table.set("IsEncrypted", entry.encrypted)?;
            if self.with_data && !entry.directory {
                table.set("Data", Bytes(entry.data))?;
            }
            list.raw_push(table)?;
        }
        Ok(LuaValue::Table(list))
    }
}

pub struct Input {
    pub name: String,
    pub data: Vec<u8>,
    pub directory: bool,
}

pub fn read_inputs(lua: &Lua, entries: LuaTable) -> LuaResult<Vec<Input>> {
    let mut inputs = Vec::new();

    if entries.raw_len() > 0 {
        for entry in entries.sequence_values::<LuaTable>() {
            let entry = entry?;
            let name: String = entry
                .get("Name")
                .map_err(|_| LuaError::runtime("every entry needs a Name"))?;
            let directory =
                entry.get::<Option<bool>>("IsDirectory")?.unwrap_or(false) || name.ends_with('/');
            let data = match entry.get::<LuaValue>("Data")? {
                LuaNil if directory => Vec::new(),
                LuaNil => return Err(LuaError::runtime(format!("the entry '{name}' has no Data"))),
                value => quark_core::Binary::from_lua(value, lua)?.0,
            };
            inputs.push(Input {
                name,
                data,
                directory,
            });
        }
    } else {
        for pair in entries.pairs::<String, LuaValue>() {
            let (name, value) = pair?;
            let directory = name.ends_with('/');
            let data = if directory {
                Vec::new()
            } else {
                quark_core::Binary::from_lua(value, lua)?.0
            };
            inputs.push(Input {
                name,
                data,
                directory,
            });
        }
        inputs.sort_by(|left, right| left.name.cmp(&right.name));
    }

    for input in &inputs {
        if input.name.is_empty() {
            return Err(LuaError::runtime("entry names must not be empty"));
        }
    }
    Ok(inputs)
}

pub fn safe_path(name: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in name.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return None,
            part if part.contains(':') => return None,
            part => parts.push(part),
        }
    }
    if name.starts_with('/') || name.starts_with('\\') || parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}
