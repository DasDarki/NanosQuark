use mlua::prelude::*;

use crate::buffer::{Buffer, NumberType};
use crate::create_userdata;

pub struct BufferView {
    buffer: LuaAnyUserData,
    kind: NumberType,
    name: String,
    offset: usize,
    count: usize,
    base: i64,
}

impl BufferView {
    fn position(&self, index: i64) -> LuaResult<usize> {
        let position = index - self.base;
        if position < 0 || position as usize >= self.count {
            return Err(LuaError::runtime(format!(
                "index {index} is outside of the view ({} to {})",
                self.base,
                self.base + self.count as i64 - 1
            )));
        }
        Ok(self.offset + position as usize * self.kind.size())
    }

    fn ensure_fits(&self, buffer: &Buffer) -> LuaResult<()> {
        if self.offset + self.count * self.kind.size() > buffer.data.len() {
            return Err(LuaError::runtime(
                "the buffer became smaller than the view, create a new view",
            ));
        }
        Ok(())
    }

    fn get(&self, index: i64) -> LuaResult<LuaValue> {
        let byte = self.position(index)?;
        let buffer = self.buffer.borrow::<Buffer>()?;
        self.ensure_fits(&buffer)?;
        Ok(self.kind.read(&buffer.data[byte..byte + self.kind.size()]))
    }

    fn set(&self, index: i64, value: &LuaValue) -> LuaResult<()> {
        let byte = self.position(index)?;
        let bytes = self.kind.encode(value)?;
        let mut buffer = self.buffer.borrow_mut::<Buffer>()?;
        self.ensure_fits(&buffer)?;
        buffer.data[byte..byte + bytes.len()].copy_from_slice(&bytes);
        Ok(())
    }

    fn numbers(&self) -> LuaResult<Vec<f64>> {
        let buffer = self.buffer.borrow::<Buffer>()?;
        self.ensure_fits(&buffer)?;
        let size = self.kind.size();
        Ok((0..self.count)
            .map(|index| {
                let start = self.offset + index * size;
                match self.kind.read(&buffer.data[start..start + size]) {
                    LuaValue::Integer(integer) => integer as f64,
                    LuaValue::Number(number) => number,
                    _ => 0.0,
                }
            })
            .collect())
    }

    fn write_all(&self, values: impl Iterator<Item = LuaValue>) -> LuaResult<()> {
        let size = self.kind.size();
        let mut encoded = Vec::with_capacity(self.count * size);
        for value in values {
            encoded.extend_from_slice(&self.kind.encode(&value)?);
        }
        let mut buffer = self.buffer.borrow_mut::<Buffer>()?;
        self.ensure_fits(&buffer)?;
        buffer.data[self.offset..self.offset + encoded.len()].copy_from_slice(&encoded);
        Ok(())
    }

    fn to_value(&self, number: f64) -> LuaValue {
        if self.name.starts_with('f') {
            LuaValue::Number(number)
        } else {
            LuaValue::Integer(number.round() as i64)
        }
    }
}

impl LuaUserData for BufferView {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Get", |_, view, index: i64| view.get(index));
        methods.add_method("Set", |_, view, (index, value): (i64, LuaValue)| {
            view.set(index, &value)
        });
        methods.add_method("GetCount", |_, view, ()| Ok(view.count));
        methods.add_method("GetType", |_, view, ()| Ok(view.name.clone()));
        methods.add_method("GetBuffer", |_, view, ()| Ok(view.buffer.clone()));
        methods.add_method("ToTable", |lua, view, ()| {
            let buffer = view.buffer.borrow::<Buffer>()?;
            view.ensure_fits(&buffer)?;
            let size = view.kind.size();
            let table = lua.create_table_with_capacity(view.count, 0)?;
            for index in 0..view.count {
                let start = view.offset + index * size;
                table.raw_push(view.kind.read(&buffer.data[start..start + size]))?;
            }
            Ok(table)
        });
        methods.add_method("FromTable", |_, view, values: LuaTable| {
            let length = values.raw_len();
            if length > view.count {
                return Err(LuaError::runtime(format!(
                    "the list has {length} values but the view only {}",
                    view.count
                )));
            }
            let values = values
                .sequence_values::<LuaValue>()
                .collect::<LuaResult<Vec<_>>>()?;
            view.write_all(values.into_iter())
        });
        methods.add_method("Fill", |_, view, value: LuaValue| {
            view.write_all(std::iter::repeat_n(value, view.count))
        });
        methods.add_method("Scale", |_, view, (factor, offset): (f64, Option<f64>)| {
            let numbers = view.numbers()?;
            let offset = offset.unwrap_or(0.0);
            view.write_all(
                numbers
                    .into_iter()
                    .map(|number| view.to_value(number * factor + offset)),
            )
        });
        methods.add_method("Sum", |_, view, ()| {
            let sum: f64 = view.numbers()?.iter().sum();
            Ok(view.to_value(sum))
        });
        methods.add_method("Average", |_, view, ()| {
            let numbers = view.numbers()?;
            Ok(if numbers.is_empty() {
                None
            } else {
                Some(numbers.iter().sum::<f64>() / numbers.len() as f64)
            })
        });
        for (name, pick_max) in [("Min", false), ("Max", true)] {
            methods.add_method(name, move |lua, view, ()| {
                let numbers = view.numbers()?;
                let mut best: Option<(usize, f64)> = None;
                for (index, number) in numbers.into_iter().enumerate() {
                    let better = match best {
                        None => true,
                        Some((_, current)) if pick_max => number > current,
                        Some((_, current)) => number < current,
                    };
                    if better {
                        best = Some((index, number));
                    }
                }
                match best {
                    Some((index, number)) => {
                        (view.to_value(number), index as i64 + view.base).into_lua_multi(lua)
                    }
                    None => LuaNil.into_lua_multi(lua),
                }
            });
        }
        methods.add_method("Sort", |_, view, descending: Option<bool>| {
            let mut numbers = view.numbers()?;
            numbers.sort_by(|left, right| left.total_cmp(right));
            if descending.unwrap_or(false) {
                numbers.reverse();
            }
            view.write_all(numbers.into_iter().map(|number| view.to_value(number)))
        });

        methods.add_meta_method(
            LuaMetaMethod::Index,
            |_, view, index: LuaValue| {
                let index = match index {
                    LuaValue::Integer(index) => index,
                    LuaValue::Number(number) if number.fract() == 0.0 => number as i64,
                    _ => return Ok(LuaNil),
                };
                if index < view.base || index - view.base >= view.count as i64 {
                    return Ok(LuaNil);
                }
                view.get(index)
            },
        );
        methods.add_meta_method(
            LuaMetaMethod::NewIndex,
            |_, view, (index, value): (i64, LuaValue)| view.set(index, &value),
        );
        methods.add_meta_method(LuaMetaMethod::Len, |_, view, ()| Ok(view.count));
        methods.add_meta_method(LuaMetaMethod::ToString, |_, view, ()| {
            Ok(format!("BufferView({}, {} values)", view.name, view.count))
        });
    }
}

pub fn create(
    lua: &Lua,
    buffer: LuaAnyUserData,
    kind: &str,
    offset: Option<i64>,
    count: Option<usize>,
) -> LuaResult<LuaAnyUserData> {
    let number = NumberType::parse(kind)?;
    let (start, base, length) = {
        let data = buffer.borrow::<Buffer>()?;
        let base = data.base;
        let offset = offset.unwrap_or(base);
        if offset < base || (offset - base) as usize > data.data.len() {
            return Err(LuaError::runtime(format!(
                "offset {offset} is outside of the buffer"
            )));
        }
        ((offset - base) as usize, base, data.data.len())
    };
    let available = (length - start) / number.size();
    let count = count.unwrap_or(available);
    if count > available {
        return Err(LuaError::runtime(format!(
            "the buffer has room for {available} values of type {kind} from this offset, not {count}"
        )));
    }
    create_userdata(
        lua,
        BufferView {
            buffer,
            kind: number,
            name: kind.to_ascii_lowercase(),
            offset: start,
            count,
            base,
        },
    )
}
