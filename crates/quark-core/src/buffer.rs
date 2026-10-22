use mlua::prelude::*;

use crate::{Bytes, Object, package_environment};

const MAX_BUFFER_SIZE: usize = 256 * 1024 * 1024;

pub struct Buffer {
    pub data: Vec<u8>,
    base: i64,
}

struct IndexBases(LuaTable);

pub struct Binary(pub Vec<u8>);

impl Binary {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl FromLua for Binary {
    fn from_lua(value: LuaValue, _: &Lua) -> LuaResult<Self> {
        match value {
            LuaValue::String(string) => Ok(Binary(string.as_bytes().to_vec())),
            LuaValue::Integer(number) => Ok(Binary(number.to_string().into_bytes())),
            LuaValue::Number(number) => Ok(Binary(number.to_string().into_bytes())),
            LuaValue::UserData(userdata) if userdata.is::<Buffer>() => {
                Ok(Binary(userdata.borrow::<Buffer>()?.data.clone()))
            }
            other => Err(LuaError::runtime(format!(
                "expected a string or a Buffer, got {}",
                other.type_name()
            ))),
        }
    }
}

#[derive(Clone, Copy)]
enum Kind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

#[derive(Clone, Copy)]
struct NumberType {
    kind: Kind,
    big_endian: bool,
}

impl NumberType {
    fn parse(name: &str) -> LuaResult<Self> {
        let lower = name.to_ascii_lowercase();
        let (base, big_endian) = if let Some(base) = lower.strip_suffix("be") {
            (base, true)
        } else if let Some(base) = lower.strip_suffix("le") {
            (base, false)
        } else {
            (lower.as_str(), false)
        };

        let kind = match base {
            "u8" => Kind::U8,
            "i8" => Kind::I8,
            "u16" => Kind::U16,
            "i16" => Kind::I16,
            "u32" => Kind::U32,
            "i32" => Kind::I32,
            "u64" => Kind::U64,
            "i64" => Kind::I64,
            "f32" => Kind::F32,
            "f64" => Kind::F64,
            _ => {
                return Err(LuaError::runtime(format!(
                    "unknown number type '{name}', expected u8, i8, u16, i16, u32, i32, u64, i64, f32 or f64 with an optional le / be suffix"
                )));
            }
        };

        Ok(NumberType { kind, big_endian })
    }

    fn size(self) -> usize {
        match self.kind {
            Kind::U8 | Kind::I8 => 1,
            Kind::U16 | Kind::I16 => 2,
            Kind::U32 | Kind::I32 | Kind::F32 => 4,
            Kind::U64 | Kind::I64 | Kind::F64 => 8,
        }
    }

    fn read(self, bytes: &[u8]) -> LuaValue {
        macro_rules! decode {
            ($type:ty) => {{
                let array = bytes
                    .try_into()
                    .expect("slice has the size of the number type");
                if self.big_endian {
                    <$type>::from_be_bytes(array)
                } else {
                    <$type>::from_le_bytes(array)
                }
            }};
        }

        match self.kind {
            Kind::U8 => LuaValue::Integer(i64::from(bytes[0])),
            Kind::I8 => LuaValue::Integer(i64::from(bytes[0] as i8)),
            Kind::U16 => LuaValue::Integer(i64::from(decode!(u16))),
            Kind::I16 => LuaValue::Integer(i64::from(decode!(i16))),
            Kind::U32 => LuaValue::Integer(i64::from(decode!(u32))),
            Kind::I32 => LuaValue::Integer(i64::from(decode!(i32))),
            Kind::U64 => {
                let value = decode!(u64);
                match i64::try_from(value) {
                    Ok(value) => LuaValue::Integer(value),
                    Err(_) => LuaValue::Number(value as f64),
                }
            }
            Kind::I64 => LuaValue::Integer(decode!(i64)),
            Kind::F32 => LuaValue::Number(f64::from(decode!(f32))),
            Kind::F64 => LuaValue::Number(decode!(f64)),
        }
    }

    fn encode(self, value: &LuaValue) -> LuaResult<Vec<u8>> {
        macro_rules! encode {
            ($value:expr) => {{
                let value = $value;
                if self.big_endian {
                    value.to_be_bytes().to_vec()
                } else {
                    value.to_le_bytes().to_vec()
                }
            }};
        }

        match self.kind {
            Kind::F32 => Ok(encode!(number(value)? as f32)),
            Kind::F64 => Ok(encode!(number(value)?)),
            Kind::U64 => {
                let value = match value {
                    LuaValue::Integer(integer) => u64::try_from(*integer).ok(),
                    LuaValue::Number(number)
                        if number.fract() == 0.0
                            && *number >= 0.0
                            && *number < 18446744073709551616.0 =>
                    {
                        Some(*number as u64)
                    }
                    _ => None,
                }
                .ok_or_else(|| out_of_range(value, "u64"))?;
                Ok(encode!(value))
            }
            kind => {
                let integer = integer(value)?;
                let check = |minimum: i64, maximum: i64, name: &str| {
                    if (minimum..=maximum).contains(&integer) {
                        Ok(())
                    } else {
                        Err(out_of_range(value, name))
                    }
                };
                match kind {
                    Kind::U8 => check(0, u8::MAX.into(), "u8").map(|_| vec![integer as u8]),
                    Kind::I8 => check(i8::MIN.into(), i8::MAX.into(), "i8")
                        .map(|_| vec![integer as i8 as u8]),
                    Kind::U16 => check(0, u16::MAX.into(), "u16").map(|_| encode!(integer as u16)),
                    Kind::I16 => check(i16::MIN.into(), i16::MAX.into(), "i16")
                        .map(|_| encode!(integer as i16)),
                    Kind::U32 => check(0, u32::MAX.into(), "u32").map(|_| encode!(integer as u32)),
                    Kind::I32 => check(i32::MIN.into(), i32::MAX.into(), "i32")
                        .map(|_| encode!(integer as i32)),
                    _ => Ok(encode!(integer)),
                }
            }
        }
    }
}

impl FromLua for NumberType {
    fn from_lua(value: LuaValue, lua: &Lua) -> LuaResult<Self> {
        NumberType::parse(&String::from_lua(value, lua)?)
    }
}

fn number(value: &LuaValue) -> LuaResult<f64> {
    match value {
        LuaValue::Integer(integer) => Ok(*integer as f64),
        LuaValue::Number(number) => Ok(*number),
        other => Err(LuaError::runtime(format!(
            "expected a number, got {}",
            other.type_name()
        ))),
    }
}

fn integer(value: &LuaValue) -> LuaResult<i64> {
    match value {
        LuaValue::Integer(integer) => Ok(*integer),
        LuaValue::Number(number) if number.fract() == 0.0 && number.abs() < 9.2e18 => {
            Ok(*number as i64)
        }
        LuaValue::Number(_) => Err(LuaError::runtime(
            "expected an integer, got a number with a fraction",
        )),
        other => Err(LuaError::runtime(format!(
            "expected an integer, got {}",
            other.type_name()
        ))),
    }
}

fn out_of_range(value: &LuaValue, name: &str) -> LuaError {
    let shown = match value {
        LuaValue::Integer(integer) => integer.to_string(),
        LuaValue::Number(number) => number.to_string(),
        other => other.type_name().to_owned(),
    };
    LuaError::runtime(format!("{shown} does not fit into {name}"))
}

impl Buffer {
    pub fn new(lua: &Lua, data: Vec<u8>) -> LuaResult<Self> {
        Buffer::check_size(data.len())?;
        Ok(Buffer {
            data,
            base: package_base(lua)?,
        })
    }

    fn derive(&self, data: Vec<u8>) -> Self {
        Buffer {
            data,
            base: self.base,
        }
    }

    fn check_size(size: usize) -> LuaResult<()> {
        if size > MAX_BUFFER_SIZE {
            return Err(LuaError::runtime(format!(
                "buffers cannot be larger than {MAX_BUFFER_SIZE} bytes"
            )));
        }
        Ok(())
    }

    fn offset(&self, offset: i64) -> LuaResult<usize> {
        if offset < self.base {
            return Err(LuaError::runtime(format!(
                "offset {offset} is invalid, offsets of this buffer start at {}",
                self.base
            )));
        }
        usize::try_from(offset - self.base).map_err(|_| LuaError::runtime("offset overflow"))
    }

    fn range(&self, start: Option<i64>, finish: Option<i64>) -> LuaResult<(usize, usize)> {
        resolve_range(self.base, self.data.len(), start, finish, "buffer")
    }

    fn slice_at(&self, offset: usize, length: usize) -> LuaResult<&[u8]> {
        offset
            .checked_add(length)
            .and_then(|end| self.data.get(offset..end))
            .ok_or_else(|| {
                LuaError::runtime(format!(
                    "cannot read {length} bytes at offset {}, the buffer has {} bytes",
                    offset as i64 + self.base,
                    self.data.len()
                ))
            })
    }

    fn write_at(&mut self, offset: usize, bytes: &[u8]) -> LuaResult<()> {
        let end = offset
            .checked_add(bytes.len())
            .ok_or_else(|| LuaError::runtime("offset overflow"))?;
        Buffer::check_size(end)?;
        if end > self.data.len() {
            self.data.resize(end, 0);
        }
        self.data[offset..end].copy_from_slice(bytes);
        Ok(())
    }
}

fn resolve_range(
    base: i64,
    length: usize,
    start: Option<i64>,
    finish: Option<i64>,
    name: &str,
) -> LuaResult<(usize, usize)> {
    let shown_start = start.unwrap_or(base);
    let shown_finish = finish.unwrap_or(length as i64);
    let invalid = || {
        LuaError::runtime(format!(
            "the range {shown_start} to {shown_finish} is outside of the {name} (size {length}, offsets start at {base})"
        ))
    };

    let start = match start {
        Some(start) if start < base => return Err(invalid()),
        Some(start) => usize::try_from(start - base).map_err(|_| invalid())?,
        None => 0,
    };
    let end = match finish {
        Some(finish) => usize::try_from(finish).map_err(|_| invalid())?,
        None => length,
    };

    if start > end || end > length {
        return Err(invalid());
    }
    Ok((start, end))
}

fn check_base(base: i64) -> LuaResult<i64> {
    match base {
        0 | 1 => Ok(base),
        _ => Err(LuaError::runtime("the index base has to be 0 or 1")),
    }
}

fn index_bases(lua: &Lua) -> LuaResult<LuaTable> {
    if let Some(bases) = lua.app_data_ref::<IndexBases>() {
        return Ok(bases.0.clone());
    }

    let bases = lua.create_table()?;
    let weak_keys = lua.create_table()?;
    weak_keys.set("__mode", "k")?;
    bases.set_metatable(Some(weak_keys))?;
    lua.set_app_data(IndexBases(bases.clone()));
    Ok(bases)
}

pub fn package_base(lua: &Lua) -> LuaResult<i64> {
    let Some(environment) = package_environment(lua) else {
        return Ok(0);
    };
    Ok(index_bases(lua)?
        .raw_get::<Option<i64>>(environment)?
        .unwrap_or(0))
}

impl LuaUserData for Buffer {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetSize", |_, buffer, ()| Ok(buffer.data.len()));

        methods.add_method("GetIndexBase", |_, buffer, ()| Ok(buffer.base));

        methods.add_function("SetIndexBase", |_, (this, base): (LuaAnyUserData, i64)| {
            this.borrow_mut::<Buffer>()?.base = check_base(base)?;
            Ok(this)
        });

        methods.add_method_mut("Resize", |_, buffer, (size, fill): (usize, Option<u8>)| {
            Buffer::check_size(size)?;
            buffer.data.resize(size, fill.unwrap_or(0));
            Ok(())
        });

        methods.add_method("Read", |_, buffer, (kind, offset): (NumberType, i64)| {
            Ok(kind.read(buffer.slice_at(buffer.offset(offset)?, kind.size())?))
        });

        methods.add_method_mut(
            "Write",
            |_, buffer, (kind, offset, value): (NumberType, i64, LuaValue)| {
                let offset = buffer.offset(offset)?;
                let bytes = kind.encode(&value)?;
                buffer.write_at(offset, &bytes)
            },
        );

        methods.add_function(
            "Append",
            |_, (this, kind, value): (LuaAnyUserData, NumberType, LuaValue)| {
                {
                    let mut buffer = this.borrow_mut::<Buffer>()?;
                    let bytes = kind.encode(&value)?;
                    let offset = buffer.data.len();
                    buffer.write_at(offset, &bytes)?;
                }
                Ok(this)
            },
        );

        methods.add_method(
            "ReadArray",
            |lua, buffer, (kind, offset, count): (NumberType, i64, usize)| {
                let length = count
                    .checked_mul(kind.size())
                    .ok_or_else(|| LuaError::runtime("count overflow"))?;
                let bytes = buffer.slice_at(buffer.offset(offset)?, length)?;
                lua.create_sequence_from(
                    bytes
                        .chunks_exact(kind.size())
                        .map(|chunk| kind.read(chunk)),
                )
            },
        );

        methods.add_method_mut(
            "WriteArray",
            |_, buffer, (kind, offset, values): (NumberType, i64, Vec<LuaValue>)| {
                let offset = buffer.offset(offset)?;
                let mut bytes = Vec::with_capacity(values.len() * kind.size());
                for value in &values {
                    bytes.extend(kind.encode(value)?);
                }
                buffer.write_at(offset, &bytes)
            },
        );

        methods.add_method(
            "ReadString",
            |_, buffer, (offset, length): (i64, Option<usize>)| {
                let offset = buffer.offset(offset)?;
                let length = length.unwrap_or_else(|| buffer.data.len().saturating_sub(offset));
                Ok(Bytes(buffer.slice_at(offset, length)?.to_vec()))
            },
        );

        methods.add_method_mut("WriteString", |_, buffer, (offset, data): (i64, Binary)| {
            let offset = buffer.offset(offset)?;
            buffer.write_at(offset, data.as_bytes())
        });

        methods.add_function(
            "AppendString",
            |_, (this, data): (LuaAnyUserData, Binary)| {
                {
                    let mut buffer = this.borrow_mut::<Buffer>()?;
                    let offset = buffer.data.len();
                    buffer.write_at(offset, data.as_bytes())?;
                }
                Ok(this)
            },
        );

        methods.add_method(
            "Slice",
            |_, buffer, (start, finish): (Option<i64>, Option<i64>)| {
                let (start, end) = buffer.range(start, finish)?;
                Ok(Object(buffer.derive(buffer.data[start..end].to_vec())))
            },
        );

        methods.add_method_mut(
            "CopyFrom",
            |_,
             buffer,
             (source, offset, start, finish): (Binary, Option<i64>, Option<i64>, Option<i64>)| {
                let offset = buffer.offset(offset.unwrap_or(buffer.base))?;
                let source = source.as_bytes();
                let (start, end) = resolve_range(buffer.base, source.len(), start, finish, "source")?;
                buffer.write_at(offset, &source[start..end])
            },
        );

        methods.add_method_mut(
            "Fill",
            |_, buffer, (byte, start, finish): (u8, Option<i64>, Option<i64>)| {
                let (start, end) = buffer.range(start, finish)?;
                buffer.data[start..end].fill(byte);
                Ok(())
            },
        );

        methods.add_method(
            "GetBytes",
            |_, buffer, (start, finish): (Option<i64>, Option<i64>)| {
                let (start, end) = buffer.range(start, finish)?;
                Ok(Bytes(buffer.data[start..end].to_vec()))
            },
        );

        methods.add_method("ToHex", |_, buffer, ()| {
            Ok(buffer
                .data
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>())
        });

        methods.add_method("Equals", |_, buffer, other: Binary| {
            Ok(buffer.data == other.as_bytes())
        });

        methods.add_method("Clone", |_, buffer, ()| {
            Ok(Object(buffer.derive(buffer.data.clone())))
        });

        methods.add_meta_method(LuaMetaMethod::Len, |_, buffer, ()| Ok(buffer.data.len()));
        methods.add_meta_method(LuaMetaMethod::Eq, |_, buffer, other: LuaAnyUserData| {
            Ok(other
                .borrow::<Buffer>()
                .map(|other| other.data == buffer.data)
                .unwrap_or(false))
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, buffer, ()| {
            Ok(format!("Buffer({} bytes)", buffer.data.len()))
        });
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "New",
        lua.create_function(|lua, (size, fill): (Option<usize>, Option<u8>)| {
            let size = size.unwrap_or(0);
            Buffer::check_size(size)?;
            Ok(Object(Buffer::new(lua, vec![fill.unwrap_or(0); size])?))
        })?,
    )?;

    module.set(
        "FromString",
        lua.create_function(|lua, data: Binary| Ok(Object(Buffer::new(lua, data.0)?)))?,
    )?;

    module.set(
        "FromArray",
        lua.create_function(|lua, (kind, values): (NumberType, Vec<LuaValue>)| {
            let mut data = Vec::with_capacity(values.len() * kind.size());
            for value in &values {
                data.extend(kind.encode(value)?);
            }
            Ok(Object(Buffer::new(lua, data)?))
        })?,
    )?;

    module.set(
        "Concat",
        lua.create_function(|lua, parts: Vec<Binary>| {
            let data = parts
                .iter()
                .flat_map(|part| part.as_bytes().iter().copied())
                .collect::<Vec<_>>();
            Ok(Object(Buffer::new(lua, data)?))
        })?,
    )?;

    module.set(
        "IsBuffer",
        lua.create_function(|_, value: LuaValue| {
            Ok(matches!(value, LuaValue::UserData(userdata) if userdata.is::<Buffer>()))
        })?,
    )?;

    module.set(
        "SetIndexBase",
        lua.create_function(|lua, base: i64| {
            let base = check_base(base)?;
            let environment = package_environment(lua).ok_or_else(|| {
                LuaError::runtime("Buffer.SetIndexBase has to be called from a package script")
            })?;
            index_bases(lua)?.raw_set(environment, base)
        })?,
    )?;

    module.set(
        "GetIndexBase",
        lua.create_function(|lua, ()| package_base(lua))?,
    )?;

    Ok(module)
}
