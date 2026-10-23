use std::io::{Read, Write};

use mlua::prelude::*;
use quark_core::{Binary, Bytes, Options, options_and_callback};

pub const DEFAULT_MAX_SIZE: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq)]
pub enum Algorithm {
    Lz4,
    Zstd,
    Gzip,
    Deflate,
    Zlib,
    Brotli,
}

const ALGORITHMS: &[(&str, Algorithm)] = &[
    ("lz4", Algorithm::Lz4),
    ("zstd", Algorithm::Zstd),
    ("gzip", Algorithm::Gzip),
    ("deflate", Algorithm::Deflate),
    ("zlib", Algorithm::Zlib),
    ("brotli", Algorithm::Brotli),
];

impl Algorithm {
    pub fn parse(name: &str) -> LuaResult<Self> {
        let lower = name.to_ascii_lowercase();
        let lower = match lower.as_str() {
            "zstandard" => "zstd",
            "gz" => "gzip",
            "br" => "brotli",
            other => other,
        };
        ALGORITHMS
            .iter()
            .find(|(candidate, _)| *candidate == lower)
            .map(|(_, algorithm)| *algorithm)
            .ok_or_else(|| {
                LuaError::runtime(format!(
                    "unknown compression algorithm '{name}', expected lz4, zstd, gzip, deflate, zlib or brotli"
                ))
            })
    }

    pub fn name(self) -> &'static str {
        ALGORITHMS
            .iter()
            .find(|(_, candidate)| *candidate == self)
            .map(|(name, _)| *name)
            .unwrap_or_default()
    }

    fn levels(self) -> Option<(i32, i32, i32)> {
        match self {
            Algorithm::Lz4 => None,
            Algorithm::Zstd => Some((1, 22, 3)),
            Algorithm::Gzip | Algorithm::Deflate | Algorithm::Zlib => Some((0, 9, 6)),
            Algorithm::Brotli => Some((0, 11, 6)),
        }
    }

    pub fn level(self, level: Option<i32>) -> LuaResult<i32> {
        match (self.levels(), level) {
            (None, None) => Ok(0),
            (None, Some(_)) => Err(LuaError::runtime(format!(
                "{} has no compression levels",
                self.name()
            ))),
            (Some((_, _, default)), None) => Ok(default),
            (Some((minimum, maximum, _)), Some(level)) if (minimum..=maximum).contains(&level) => {
                Ok(level)
            }
            (Some((minimum, maximum, _)), Some(_)) => Err(LuaError::runtime(format!(
                "the {} level has to be between {minimum} and {maximum}",
                self.name()
            ))),
        }
    }

    pub fn detect(data: &[u8]) -> Option<Self> {
        match data {
            [0x1f, 0x8b, ..] => Some(Algorithm::Gzip),
            [0x28, 0xb5, 0x2f, 0xfd, ..] => Some(Algorithm::Zstd),
            [0x04, 0x22, 0x4d, 0x18, ..] => Some(Algorithm::Lz4),
            [first, second, ..]
                if first & 0x0f == 8 && (u16::from(*first) << 8 | u16::from(*second)) % 31 == 0 =>
            {
                Some(Algorithm::Zlib)
            }
            _ => None,
        }
    }
}

impl FromLua for Algorithm {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        Algorithm::parse(&String::from_lua(value, lua)?)
    }
}

pub fn compress(algorithm: Algorithm, data: &[u8], level: i32) -> Result<Vec<u8>, String> {
    let failed =
        |error: std::io::Error| format!("{} compression failed: {error}", algorithm.name());
    match algorithm {
        Algorithm::Lz4 => {
            let mut encoder = lz4_flex::frame::FrameEncoder::new(Vec::new());
            encoder.write_all(data).map_err(failed)?;
            encoder
                .finish()
                .map_err(|error| format!("lz4 compression failed: {error}"))
        }
        Algorithm::Zstd => zstd::bulk::compress(data, level).map_err(failed),
        Algorithm::Gzip => {
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::new(level as u32));
            encoder.write_all(data).map_err(failed)?;
            encoder.finish().map_err(failed)
        }
        Algorithm::Deflate => {
            let mut encoder = flate2::write::DeflateEncoder::new(
                Vec::new(),
                flate2::Compression::new(level as u32),
            );
            encoder.write_all(data).map_err(failed)?;
            encoder.finish().map_err(failed)
        }
        Algorithm::Zlib => {
            let mut encoder =
                flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(level as u32));
            encoder.write_all(data).map_err(failed)?;
            encoder.finish().map_err(failed)
        }
        Algorithm::Brotli => {
            let mut output = Vec::new();
            {
                let mut encoder =
                    brotli::CompressorWriter::new(&mut output, 4096, level as u32, 22);
                encoder.write_all(data).map_err(failed)?;
            }
            Ok(output)
        }
    }
}

pub fn decompress(algorithm: Algorithm, data: &[u8], max_size: u64) -> Result<Vec<u8>, String> {
    let reader: Box<dyn Read + '_> = match algorithm {
        Algorithm::Lz4 => Box::new(lz4_flex::frame::FrameDecoder::new(data)),
        Algorithm::Zstd => Box::new(
            zstd::stream::read::Decoder::with_buffer(data)
                .map_err(|error| format!("zstd decompression failed: {error}"))?,
        ),
        Algorithm::Gzip => Box::new(flate2::read::MultiGzDecoder::new(data)),
        Algorithm::Deflate => Box::new(flate2::read::DeflateDecoder::new(data)),
        Algorithm::Zlib => Box::new(flate2::read::ZlibDecoder::new(data)),
        Algorithm::Brotli => Box::new(brotli::Decompressor::new(data, 4096)),
    };
    read_limited(reader, max_size)
        .map_err(|error| format!("{} decompression failed: {error}", algorithm.name()))
}

pub fn read_limited(reader: impl Read, max_size: u64) -> Result<Vec<u8>, String> {
    let mut output = Vec::new();
    reader
        .take(max_size + 1)
        .read_to_end(&mut output)
        .map_err(|error| error.to_string())?;
    if output.len() as u64 > max_size {
        return Err(format!(
            "the data is larger than the limit of {max_size} bytes"
        ));
    }
    Ok(output)
}

pub fn max_size(options: &Options) -> LuaResult<u64> {
    let max_size = options.get_or("MaxSize", DEFAULT_MAX_SIZE)?;
    if max_size == 0 || max_size > DEFAULT_MAX_SIZE {
        return Err(LuaError::runtime(format!(
            "MaxSize has to be between 1 and {DEFAULT_MAX_SIZE} bytes"
        )));
    }
    Ok(max_size)
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "Compress",
        lua.create_function(
            |lua,
             (algorithm, data, options, callback): (
                Algorithm,
                Binary,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = options_and_callback(options, callback)?;
                let level = algorithm.level(options.get("Level")?)?;
                quark_core::run(lua, callback, move || {
                    compress(algorithm, data.as_bytes(), level).map(Bytes)
                })
            },
        )?,
    )?;

    module.set(
        "Decompress",
        lua.create_function(
            |lua,
             (algorithm, data, options, callback): (
                LuaValue,
                Binary,
                LuaValue,
                Option<LuaFunction>,
            )| {
                let (options, callback) = options_and_callback(options, callback)?;
                let max_size = max_size(&options)?;
                let algorithm = match algorithm {
                    LuaValue::String(ref name) if name.to_str()?.eq_ignore_ascii_case("auto") => {
                        None
                    }
                    other => Some(Algorithm::from_lua(other, lua)?),
                };
                quark_core::run(lua, callback, move || {
                    let algorithm = match algorithm {
                        Some(algorithm) => algorithm,
                        None => Algorithm::detect(data.as_bytes())
                            .ok_or("the compression format could not be detected")?,
                    };
                    decompress(algorithm, data.as_bytes(), max_size).map(Bytes)
                })
            },
        )?,
    )?;

    crate::stream::register(lua, &module)?;

    module.set(
        "Detect",
        lua.create_function(|_, data: Binary| {
            Ok(Algorithm::detect(data.as_bytes()).map(Algorithm::name))
        })?,
    )?;

    module.set(
        "Algorithms",
        lua.create_function(|lua, ()| {
            lua.create_sequence_from(ALGORITHMS.iter().map(|(name, _)| *name))
        })?,
    )?;

    Ok(module)
}
