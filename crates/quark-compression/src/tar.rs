use std::io::Read;

use mlua::prelude::*;
use quark_core::{Binary, Bytes, options_and_callback};

use crate::archive::{Entries, Entry, read_inputs, safe_path};
use crate::codec::{Algorithm, compress, decompress, max_size, read_limited};

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "Create",
        lua.create_function(
            |lua, (entries, options, callback): (LuaTable, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                let inputs = read_inputs(lua, entries)?;
                let compression = match options.get::<String>("Compression")? {
                    Some(name) => {
                        let algorithm = Algorithm::parse(&name)?;
                        Some((algorithm, algorithm.level(options.get("Level")?)?))
                    }
                    None => None,
                };

                quark_core::run(lua, callback, move || {
                    let failed =
                        |error: std::io::Error| format!("cannot create the TAR archive: {error}");
                    let mut builder = ::tar::Builder::new(Vec::new());
                    for input in &inputs {
                        let mut header = ::tar::Header::new_gnu();
                        header.set_mode(if input.directory { 0o755 } else { 0o644 });
                        header.set_size(input.data.len() as u64);
                        header.set_entry_type(if input.directory {
                            ::tar::EntryType::Directory
                        } else {
                            ::tar::EntryType::Regular
                        });
                        header.set_cksum();
                        builder
                            .append_data(&mut header, &input.name, input.data.as_slice())
                            .map_err(failed)?;
                    }
                    let archive = builder.into_inner().map_err(failed)?;
                    match compression {
                        Some((algorithm, level)) => compress(algorithm, &archive, level).map(Bytes),
                        None => Ok(Bytes(archive)),
                    }
                })
            },
        )?,
    )?;

    for (name, with_data) in [("Extract", true), ("List", false)] {
        module.set(
            name,
            lua.create_function(
                move |lua, (data, options, callback): (Binary, LuaValue, Option<LuaFunction>)| {
                    let (options, callback) = options_and_callback(options, callback)?;
                    let max_size = max_size(&options)?;
                    quark_core::run(lua, callback, move || read(data.0, max_size, with_data))
                },
            )?,
        )?;
    }

    Ok(module)
}

fn read(data: Vec<u8>, max_size: u64, with_data: bool) -> Result<Entries, String> {
    let data = match Algorithm::detect(&data) {
        Some(algorithm) => decompress(algorithm, &data, max_size)?,
        None => data,
    };

    let failed = |error: std::io::Error| format!("invalid TAR archive: {error}");
    let mut archive = ::tar::Archive::new(data.as_slice());
    let mut entries = Vec::new();
    let mut remaining = max_size;

    for entry in archive.entries().map_err(failed)? {
        let mut entry = entry.map_err(failed)?;
        let name = String::from_utf8_lossy(&entry.path_bytes()).into_owned();
        let kind = entry.header().entry_type();
        let directory = kind.is_dir();
        if !directory && !kind.is_file() {
            continue;
        }

        let size = entry.size();
        let mut contents = Vec::new();
        if with_data && !directory {
            contents = read_limited(entry.by_ref(), remaining)
                .map_err(|error| format!("cannot extract '{name}': {error}"))?;
            remaining -= contents.len() as u64;
        }
        entries.push(Entry {
            encrypted: false,
            safe_path: safe_path(&name),
            size,
            compressed_size: None,
            name,
            data: contents,
            directory,
        });
    }

    Ok(Entries { entries, with_data })
}
