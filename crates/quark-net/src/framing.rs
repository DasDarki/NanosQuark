use mlua::prelude::*;
use quark_core::Options;

pub const DEFAULT_MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq)]
pub enum Framing {
    Raw,
    Line,
    Length,
}

impl Framing {
    pub fn from_options(options: &Options) -> LuaResult<(Self, usize)> {
        let framing = match options
            .get_or("Framing", "raw".to_owned())?
            .to_ascii_lowercase()
            .as_str()
        {
            "raw" => Framing::Raw,
            "line" => Framing::Line,
            "length" => Framing::Length,
            other => {
                return Err(LuaError::runtime(format!(
                    "unknown framing '{other}', expected raw, line or length"
                )));
            }
        };
        let max = options.get_or("MaxFrameSize", DEFAULT_MAX_FRAME_SIZE)?;
        if max == 0 || max > u32::MAX as usize {
            return Err(LuaError::runtime(
                "MaxFrameSize has to be between 1 and 4294967295",
            ));
        }
        Ok((framing, max))
    }

    pub fn encode(self, data: &[u8], max: usize) -> LuaResult<Vec<u8>> {
        if self != Framing::Raw && data.len() > max {
            return Err(LuaError::runtime(format!(
                "the frame has {} bytes, the limit is {max}",
                data.len()
            )));
        }
        Ok(match self {
            Framing::Raw => data.to_vec(),
            Framing::Line => {
                if data.contains(&b'\n') {
                    return Err(LuaError::runtime(
                        "line framed messages must not contain a newline",
                    ));
                }
                let mut frame = Vec::with_capacity(data.len() + 1);
                frame.extend_from_slice(data);
                frame.push(b'\n');
                frame
            }
            Framing::Length => {
                let mut frame = Vec::with_capacity(data.len() + 4);
                frame.extend_from_slice(&(data.len() as u32).to_be_bytes());
                frame.extend_from_slice(data);
                frame
            }
        })
    }
}

pub struct Decoder {
    framing: Framing,
    max: usize,
    buffer: Vec<u8>,
}

impl Decoder {
    pub fn new(framing: Framing, max: usize) -> Self {
        Decoder {
            framing,
            max,
            buffer: Vec::new(),
        }
    }

    pub fn push(&mut self, data: &[u8]) -> Result<Vec<Vec<u8>>, String> {
        if self.framing == Framing::Raw {
            return Ok(vec![data.to_vec()]);
        }

        self.buffer.extend_from_slice(data);
        let mut frames = Vec::new();
        loop {
            match self.framing {
                Framing::Line => {
                    let Some(position) = self.buffer.iter().position(|byte| *byte == b'\n') else {
                        break;
                    };
                    let mut line: Vec<u8> = self.buffer.drain(..=position).collect();
                    line.pop();
                    if line.last() == Some(&b'\r') {
                        line.pop();
                    }
                    frames.push(line);
                }
                Framing::Length => {
                    if self.buffer.len() < 4 {
                        break;
                    }
                    let length = u32::from_be_bytes([
                        self.buffer[0],
                        self.buffer[1],
                        self.buffer[2],
                        self.buffer[3],
                    ]) as usize;
                    if length > self.max {
                        return Err(format!(
                            "received a frame of {length} bytes, the limit is {}",
                            self.max
                        ));
                    }
                    if self.buffer.len() < 4 + length {
                        break;
                    }
                    let frame = self.buffer[4..4 + length].to_vec();
                    self.buffer.drain(..4 + length);
                    frames.push(frame);
                }
                Framing::Raw => break,
            }
        }

        if self.buffer.len() > self.max + 4 {
            return Err(format!(
                "received more than {} bytes without a complete frame",
                self.max
            ));
        }
        Ok(frames)
    }
}
