use std::io::{Cursor, Read, Write};

use mlua::prelude::*;
use quark_core::{Binary, Bytes, options_and_callback};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::archive::{Entries, Entry, read_inputs, safe_path};
use crate::codec::{max_size, read_limited};

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "Create",
        lua.create_function(
            |lua, (entries, options, callback): (LuaTable, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                let inputs = read_inputs(lua, entries)?;
                let method = match options
                    .get_or("Method", "deflate".to_owned())?
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "deflate" => CompressionMethod::Deflated,
                    "store" => CompressionMethod::Stored,
                    other => {
                        return Err(LuaError::runtime(format!(
                            "unknown ZIP method '{other}', expected deflate or store"
                        )));
                    }
                };
                let level = options.get::<i64>("Level")?;
                if level.is_some_and(|level| !(0..=9).contains(&level)) {
                    return Err(LuaError::runtime("the ZIP level has to be between 0 and 9"));
                }

                quark_core::run(lua, callback, move || {
                    let failed = |error: &dyn std::fmt::Display| {
                        format!("cannot create the ZIP archive: {error}")
                    };
                    let file_options = SimpleFileOptions::default()
                        .compression_method(method)
                        .compression_level(if method == CompressionMethod::Stored {
                            None
                        } else {
                            level
                        })
                        .large_file(false);

                    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
                    for input in &inputs {
                        if input.directory {
                            writer
                                .add_directory(input.name.as_str(), file_options)
                                .map_err(|error| failed(&error))?;
                        } else {
                            writer
                                .start_file(input.name.as_str(), file_options)
                                .map_err(|error| failed(&error))?;
                            writer
                                .write_all(&input.data)
                                .map_err(|error| failed(&error))?;
                        }
                    }
                    let cursor = writer.finish().map_err(|error| failed(&error))?;
                    Ok::<_, String>(Bytes(cursor.into_inner()))
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
    let failed = |error: &dyn std::fmt::Display| format!("invalid ZIP archive: {error}");
    let mut archive = ZipArchive::new(Cursor::new(data)).map_err(|error| failed(&error))?;
    let mut entries = Vec::with_capacity(archive.len());
    let mut remaining = max_size;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(|error| failed(&error))?;
        let name = file.name().map_err(|error| failed(&error))?.into_owned();
        let directory = file.is_dir();
        let mut contents = Vec::new();
        if with_data && !directory {
            contents = read_limited(file.by_ref(), remaining)
                .map_err(|error| format!("cannot extract '{name}': {error}"))?;
            remaining -= contents.len() as u64;
        }
        entries.push(Entry {
            safe_path: safe_path(&name),
            size: file.size(),
            compressed_size: Some(file.compressed_size()),
            name,
            data: contents,
            directory,
        });
    }

    Ok(Entries { entries, with_data })
}
