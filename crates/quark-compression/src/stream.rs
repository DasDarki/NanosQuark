use std::cell::RefCell;
use std::fs::File;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::rc::Rc;

use mlua::prelude::*;
use quark_core::{Binary, Bytes, Options, create_userdata, options_and_callback};

use crate::codec::{Algorithm, DEFAULT_MAX_SIZE, decompress};

#[derive(Clone, Default)]
struct Output(Rc<RefCell<Vec<u8>>>);

impl Write for Output {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Output {
    fn take(&self) -> Vec<u8> {
        std::mem::take(&mut *self.0.borrow_mut())
    }
}

trait Finish: Write {
    fn finish(self: Box<Self>) -> io::Result<()>;
}

macro_rules! finish_with {
    ($type:ty, $finish:expr) => {
        impl Finish for $type {
            fn finish(self: Box<Self>) -> io::Result<()> {
                let finish: fn($type) -> io::Result<()> = $finish;
                finish(*self)
            }
        }
    };
}

finish_with!(lz4_flex::frame::FrameEncoder<Output>, |encoder| encoder
    .finish()
    .map(drop)
    .map_err(io::Error::other));
finish_with!(zstd::stream::write::Encoder<'static, Output>, |encoder| {
    encoder.finish().map(drop)
});
finish_with!(
    zstd::stream::write::Decoder<'static, Output>,
    |mut decoder| decoder.flush()
);
finish_with!(flate2::write::GzEncoder<Output>, |encoder| encoder
    .finish()
    .map(drop));
finish_with!(flate2::write::GzDecoder<Output>, |decoder| decoder
    .finish()
    .map(drop));
finish_with!(flate2::write::DeflateEncoder<Output>, |encoder| encoder
    .finish()
    .map(drop));
finish_with!(flate2::write::DeflateDecoder<Output>, |decoder| decoder
    .finish()
    .map(drop));
finish_with!(flate2::write::ZlibEncoder<Output>, |encoder| encoder
    .finish()
    .map(drop));
finish_with!(flate2::write::ZlibDecoder<Output>, |decoder| decoder
    .finish()
    .map(drop));
finish_with!(brotli::CompressorWriter<Output>, |encoder| {
    encoder.into_inner();
    Ok(())
});
finish_with!(brotli::DecompressorWriter<Output>, |mut decoder| decoder
    .close()
    .map_err(|_| io::Error::other(
        "the brotli stream is incomplete or corrupt"
    )));

struct Buffered(Vec<u8>, Output, u64);

impl Write for Buffered {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if (self.0.len() + data.len()) as u64 > DEFAULT_MAX_SIZE {
            return Err(io::Error::other(
                "lz4 streams are decompressed at the end and are limited to 256 MiB",
            ));
        }
        self.0.extend_from_slice(data);
        Ok(data.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Finish for Buffered {
    fn finish(self: Box<Self>) -> io::Result<()> {
        let Buffered(data, mut output, max_size) = *self;
        let decompressed = decompress(Algorithm::Lz4, &data, max_size).map_err(io::Error::other)?;
        output.write_all(&decompressed)
    }
}

fn create_writer(
    algorithm: Algorithm,
    compress: bool,
    level: i32,
    output: Output,
    max_size: u64,
) -> io::Result<Box<dyn Finish>> {
    Ok(match (algorithm, compress) {
        (Algorithm::Lz4, true) => Box::new(lz4_flex::frame::FrameEncoder::new(output)),
        (Algorithm::Lz4, false) => Box::new(Buffered(Vec::new(), output, max_size)),
        (Algorithm::Zstd, true) => Box::new(zstd::stream::write::Encoder::new(output, level)?),
        (Algorithm::Zstd, false) => Box::new(zstd::stream::write::Decoder::new(output)?),
        (Algorithm::Gzip, true) => Box::new(flate2::write::GzEncoder::new(
            output,
            flate2::Compression::new(level as u32),
        )),
        (Algorithm::Gzip, false) => Box::new(flate2::write::GzDecoder::new(output)),
        (Algorithm::Deflate, true) => Box::new(flate2::write::DeflateEncoder::new(
            output,
            flate2::Compression::new(level as u32),
        )),
        (Algorithm::Deflate, false) => Box::new(flate2::write::DeflateDecoder::new(output)),
        (Algorithm::Zlib, true) => Box::new(flate2::write::ZlibEncoder::new(
            output,
            flate2::Compression::new(level as u32),
        )),
        (Algorithm::Zlib, false) => Box::new(flate2::write::ZlibDecoder::new(output)),
        (Algorithm::Brotli, true) => Box::new(brotli::CompressorWriter::new(
            output,
            4096,
            level as u32,
            22,
        )),
        (Algorithm::Brotli, false) => Box::new(brotli::DecompressorWriter::new(output, 4096)),
    })
}

pub struct Stream {
    algorithm: Algorithm,
    compress: bool,
    writer: Option<Box<dyn Finish>>,
    output: Output,
    max_size: u64,
    total_in: u64,
    total_out: u64,
}

impl Stream {
    fn collect(&mut self) -> LuaResult<Vec<u8>> {
        let data = self.output.take();
        self.total_out += data.len() as u64;
        if !self.compress && self.total_out > self.max_size {
            self.writer = None;
            return Err(LuaError::runtime(format!(
                "the decompressed data is larger than the limit of {} bytes",
                self.max_size
            )));
        }
        Ok(data)
    }

    fn failed(&self, error: io::Error) -> LuaError {
        let action = if self.compress {
            "compression"
        } else {
            "decompression"
        };
        LuaError::runtime(format!(
            "{} {action} failed: {error}",
            self.algorithm.name()
        ))
    }
}

impl LuaUserData for Stream {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("Write", |_, stream, data: Binary| {
            let Some(writer) = stream.writer.as_mut() else {
                return Err(LuaError::runtime("the stream is already finished"));
            };
            let result = writer.write_all(data.as_bytes());
            if let Err(error) = result {
                stream.writer = None;
                return Err(stream.failed(error));
            }
            stream.total_in += data.as_bytes().len() as u64;
            stream.collect().map(Bytes)
        });
        methods.add_method_mut("Flush", |_, stream, ()| {
            let Some(writer) = stream.writer.as_mut() else {
                return Err(LuaError::runtime("the stream is already finished"));
            };
            if let Err(error) = writer.flush() {
                stream.writer = None;
                return Err(stream.failed(error));
            }
            stream.collect().map(Bytes)
        });
        methods.add_method_mut("Finish", |_, stream, ()| {
            let Some(writer) = stream.writer.take() else {
                return Err(LuaError::runtime("the stream is already finished"));
            };
            writer.finish().map_err(|error| stream.failed(error))?;
            stream.collect().map(Bytes)
        });
        methods.add_method("IsFinished", |_, stream, ()| Ok(stream.writer.is_none()));
        methods.add_method("GetTotalIn", |_, stream, ()| Ok(stream.total_in));
        methods.add_method("GetTotalOut", |_, stream, ()| Ok(stream.total_out));
    }
}

fn copy_file(
    algorithm: Option<Algorithm>,
    compress: bool,
    level: i32,
    source: &str,
    target: &str,
    max_size: Option<u64>,
) -> Result<(u64, u64), String> {
    let open_failed = |path: &str, error: io::Error| format!("cannot open '{path}': {error}");
    let mut input = BufReader::new(File::open(source).map_err(|error| open_failed(source, error))?);

    let algorithm = match algorithm {
        Some(algorithm) => algorithm,
        None => {
            let mut header = [0u8; 4];
            let read = File::open(source)
                .and_then(|mut file| file.read(&mut header))
                .map_err(|error| open_failed(source, error))?;
            Algorithm::detect(&header[..read])
                .ok_or("the compression format could not be detected")?
        }
    };
    let failed = |error: io::Error| {
        let action = if compress {
            "compression"
        } else {
            "decompression"
        };
        format!("{} {action} failed: {error}", algorithm.name())
    };

    if let Some(parent) = std::path::Path::new(target).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|error| open_failed(target, error))?;
    }
    let output = File::create(target).map_err(|error| open_failed(target, error))?;
    let mut output = BufWriter::new(output);

    let total_in = if compress {
        macro_rules! encode {
            ($encoder:expr, $finish:expr) => {{
                let mut encoder = $encoder;
                let copied = io::copy(&mut input, &mut encoder).map_err(failed)?;
                let finish: &dyn Fn(_) -> io::Result<()> = &$finish;
                finish(encoder).map_err(failed)?;
                copied
            }};
        }
        let compression = flate2::Compression::new(level as u32);
        match algorithm {
            Algorithm::Lz4 => encode!(
                lz4_flex::frame::FrameEncoder::new(&mut output),
                |encoder: lz4_flex::frame::FrameEncoder<_>| encoder
                    .finish()
                    .map(drop)
                    .map_err(io::Error::other)
            ),
            Algorithm::Zstd => encode!(
                zstd::stream::write::Encoder::new(&mut output, level).map_err(failed)?,
                |encoder: zstd::stream::write::Encoder<_>| encoder.finish().map(drop)
            ),
            Algorithm::Gzip => encode!(
                flate2::write::GzEncoder::new(&mut output, compression),
                |encoder: flate2::write::GzEncoder<_>| encoder.finish().map(drop)
            ),
            Algorithm::Deflate => encode!(
                flate2::write::DeflateEncoder::new(&mut output, compression),
                |encoder: flate2::write::DeflateEncoder<_>| encoder.finish().map(drop)
            ),
            Algorithm::Zlib => encode!(
                flate2::write::ZlibEncoder::new(&mut output, compression),
                |encoder: flate2::write::ZlibEncoder<_>| encoder.finish().map(drop)
            ),
            Algorithm::Brotli => encode!(
                brotli::CompressorWriter::new(&mut output, 4096, level as u32, 22),
                |encoder: brotli::CompressorWriter<_>| {
                    encoder.into_inner();
                    Ok(())
                }
            ),
        }
    } else {
        let reader: Box<dyn Read> = match algorithm {
            Algorithm::Lz4 => Box::new(lz4_flex::frame::FrameDecoder::new(input)),
            Algorithm::Zstd => Box::new(zstd::stream::read::Decoder::new(input).map_err(failed)?),
            Algorithm::Gzip => Box::new(flate2::read::MultiGzDecoder::new(input)),
            Algorithm::Deflate => Box::new(flate2::read::DeflateDecoder::new(input)),
            Algorithm::Zlib => Box::new(flate2::read::ZlibDecoder::new(input)),
            Algorithm::Brotli => Box::new(brotli::Decompressor::new(input, 4096)),
        };
        let limit = max_size.unwrap_or(u64::MAX);
        let copied =
            io::copy(&mut reader.take(limit.saturating_add(1)), &mut output).map_err(failed)?;
        if copied > limit {
            drop(output);
            let _ = std::fs::remove_file(target);
            return Err(format!(
                "the decompressed data is larger than the limit of {limit} bytes"
            ));
        }
        std::fs::metadata(source)
            .map(|metadata| metadata.len())
            .unwrap_or(0)
    };
    output.flush().map_err(failed)?;
    let total_out = output
        .get_ref()
        .metadata()
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    Ok((total_in, total_out))
}

fn read_algorithm(lua: &Lua, value: LuaValue, allow_auto: bool) -> LuaResult<Option<Algorithm>> {
    match value {
        LuaValue::String(ref name) if allow_auto && name.to_str()?.eq_ignore_ascii_case("auto") => {
            Ok(None)
        }
        other => Ok(Some(Algorithm::from_lua(other, lua)?)),
    }
}

struct Sizes(u64, u64);

impl IntoLuaMulti for Sizes {
    fn into_lua_multi(self, lua: &Lua) -> LuaResult<LuaMultiValue> {
        let table = lua.create_table()?;
        table.set("Input", self.0)?;
        table.set("Output", self.1)?;
        table.into_lua_multi(lua)
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "Stream",
        lua.create_function(|lua, (algorithm, options): (Algorithm, Options)| {
            let mode: String = options.get_or("Mode", "Compress".to_owned())?;
            let compress = match mode.as_str() {
                "Compress" => true,
                "Decompress" => false,
                other => {
                    return Err(LuaError::runtime(format!(
                        "unknown mode '{other}', expected Compress or Decompress"
                    )));
                }
            };
            let level = if compress {
                algorithm.level(options.get("Level")?)?
            } else {
                0
            };
            let max_size: u64 = options.get_or("MaxSize", DEFAULT_MAX_SIZE)?;
            let output = Output::default();
            let writer = create_writer(algorithm, compress, level, output.clone(), max_size)
                .map_err(|error| LuaError::runtime(error.to_string()))?;
            create_userdata(
                lua,
                Stream {
                    algorithm,
                    compress,
                    writer: Some(writer),
                    output,
                    max_size,
                    total_in: 0,
                    total_out: 0,
                },
            )
        })?,
    )?;

    for (name, compress) in [("CompressFile", true), ("DecompressFile", false)] {
        module.set(
            name,
            lua.create_function(
                move |lua,
                      (algorithm, source, target, options, callback): (
                    LuaValue,
                    String,
                    String,
                    LuaValue,
                    Option<LuaFunction>,
                )| {
                    let (options, callback) = options_and_callback(options, callback)?;
                    let algorithm = read_algorithm(lua, algorithm, !compress)?;
                    let level = match (compress, algorithm) {
                        (true, Some(algorithm)) => algorithm.level(options.get("Level")?)?,
                        _ => 0,
                    };
                    let max_size: Option<u64> = options.get("MaxSize")?;
                    quark_core::run(lua, callback, move || {
                        copy_file(algorithm, compress, level, &source, &target, max_size)
                            .map(|(input, output)| Sizes(input, output))
                    })
                },
            )?,
        )?;
    }
    Ok(())
}
