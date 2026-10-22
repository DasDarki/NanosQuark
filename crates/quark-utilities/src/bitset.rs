use mlua::prelude::*;
use quark_core::{Binary, Bytes, Object, index_base};

const MAX_SIZE: usize = 1 << 31;

#[derive(Clone, PartialEq)]
pub struct Bitset {
    words: Vec<u64>,
    size: usize,
    base: i64,
}

impl Bitset {
    fn new(size: usize, base: i64) -> LuaResult<Self> {
        if size > MAX_SIZE {
            return Err(LuaError::runtime(format!(
                "a bitset holds at most {MAX_SIZE} bits"
            )));
        }
        Ok(Bitset {
            words: vec![0; size.div_ceil(64)],
            size,
            base,
        })
    }

    fn position(&self, index: i64) -> LuaResult<usize> {
        let position = index - self.base;
        if position < 0 || position as usize >= self.size {
            return Err(LuaError::runtime(format!(
                "index {index} is outside of the bitset ({} to {})",
                self.base,
                self.base + self.size as i64 - 1
            )));
        }
        Ok(position as usize)
    }

    fn get(&self, position: usize) -> bool {
        self.words[position / 64] & (1 << (position % 64)) != 0
    }

    fn set(&mut self, position: usize, value: bool) {
        let mask = 1 << (position % 64);
        if value {
            self.words[position / 64] |= mask;
        } else {
            self.words[position / 64] &= !mask;
        }
    }

    fn trim(&mut self) {
        let used = self.size % 64;
        if used != 0
            && let Some(last) = self.words.last_mut()
        {
            *last &= (1u64 << used) - 1;
        }
    }

    fn count(&self) -> usize {
        self.words
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    fn find(&self, start: usize, value: bool) -> Option<usize> {
        if start >= self.size {
            return None;
        }
        let mut word_index = start / 64;
        let mut word = if value {
            self.words[word_index]
        } else {
            !self.words[word_index]
        };
        word &= u64::MAX << (start % 64);
        loop {
            if word != 0 {
                let position = word_index * 64 + word.trailing_zeros() as usize;
                return (position < self.size).then_some(position);
            }
            word_index += 1;
            if word_index >= self.words.len() {
                return None;
            }
            word = if value {
                self.words[word_index]
            } else {
                !self.words[word_index]
            };
        }
    }

    fn combine(&mut self, other: &Bitset, operation: impl Fn(u64, u64) -> u64) -> LuaResult<()> {
        if other.size != self.size {
            return Err(LuaError::runtime(format!(
                "both bitsets need the same size, got {} and {}",
                self.size, other.size
            )));
        }
        for (word, other) in self.words.iter_mut().zip(&other.words) {
            *word = operation(*word, *other);
        }
        self.trim();
        Ok(())
    }

    fn start(&self, start: Option<i64>) -> usize {
        match start {
            Some(start) => (start - self.base).max(0) as usize,
            None => 0,
        }
    }
}

fn combine(
    this: LuaAnyUserData,
    other: LuaAnyUserData,
    operation: fn(u64, u64) -> u64,
) -> LuaResult<LuaAnyUserData> {
    let other = other.borrow::<Bitset>()?.clone();
    this.borrow_mut::<Bitset>()?.combine(&other, operation)?;
    Ok(this)
}

impl LuaUserData for Bitset {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Get", |_, bitset, index: i64| {
            Ok(bitset.get(bitset.position(index)?))
        });
        methods.add_method_mut("Set", |_, bitset, (index, value): (i64, Option<bool>)| {
            let position = bitset.position(index)?;
            bitset.set(position, value.unwrap_or(true));
            Ok(())
        });
        methods.add_method_mut("Unset", |_, bitset, index: i64| {
            let position = bitset.position(index)?;
            bitset.set(position, false);
            Ok(())
        });
        methods.add_method_mut("Toggle", |_, bitset, index: i64| {
            let position = bitset.position(index)?;
            let value = !bitset.get(position);
            bitset.set(position, value);
            Ok(value)
        });
        methods.add_method_mut("SetAll", |_, bitset, value: Option<bool>| {
            let fill = if value.unwrap_or(true) { u64::MAX } else { 0 };
            bitset.words.iter_mut().for_each(|word| *word = fill);
            bitset.trim();
            Ok(())
        });
        methods.add_method("Count", |_, bitset, ()| Ok(bitset.count()));
        methods.add_method("GetSize", |_, bitset, ()| Ok(bitset.size));
        methods.add_method("Any", |_, bitset, ()| {
            Ok(bitset.words.iter().any(|word| *word != 0))
        });
        methods.add_method("None", |_, bitset, ()| {
            Ok(bitset.words.iter().all(|word| *word == 0))
        });
        methods.add_method("All", |_, bitset, ()| Ok(bitset.count() == bitset.size));
        methods.add_method("NextSet", |_, bitset, start: Option<i64>| {
            Ok(bitset
                .find(bitset.start(start), true)
                .map(|position| position as i64 + bitset.base))
        });
        methods.add_method("NextUnset", |_, bitset, start: Option<i64>| {
            Ok(bitset
                .find(bitset.start(start), false)
                .map(|position| position as i64 + bitset.base))
        });
        methods.add_method("GetIndices", |lua, bitset, ()| {
            let indices = lua.create_table_with_capacity(bitset.count(), 0)?;
            let mut next = bitset.find(0, true);
            while let Some(position) = next {
                indices.raw_push(position as i64 + bitset.base)?;
                next = bitset.find(position + 1, true);
            }
            Ok(indices)
        });
        methods.add_function(
            "Union",
            |_, (this, other): (LuaAnyUserData, LuaAnyUserData)| {
                combine(this, other, |left, right| left | right)
            },
        );
        methods.add_function(
            "Intersect",
            |_, (this, other): (LuaAnyUserData, LuaAnyUserData)| {
                combine(this, other, |left, right| left & right)
            },
        );
        methods.add_function(
            "Difference",
            |_, (this, other): (LuaAnyUserData, LuaAnyUserData)| {
                combine(this, other, |left, right| left & !right)
            },
        );
        methods.add_function(
            "SymmetricDifference",
            |_, (this, other): (LuaAnyUserData, LuaAnyUserData)| {
                combine(this, other, |left, right| left ^ right)
            },
        );
        methods.add_function("Invert", |_, this: LuaAnyUserData| {
            {
                let mut bitset = this.borrow_mut::<Bitset>()?;
                bitset.words.iter_mut().for_each(|word| *word = !*word);
                bitset.trim();
            }
            Ok(this)
        });
        methods.add_method_mut("Resize", |_, bitset, size: usize| {
            if size > MAX_SIZE {
                return Err(LuaError::runtime(format!(
                    "a bitset holds at most {MAX_SIZE} bits"
                )));
            }
            bitset.size = size;
            bitset.words.resize(size.div_ceil(64), 0);
            bitset.trim();
            Ok(())
        });
        methods.add_method("Clone", |_, bitset, ()| Ok(Object(bitset.clone())));
        methods.add_method("ToBytes", |_, bitset, ()| {
            let mut bytes: Vec<u8> = bitset
                .words
                .iter()
                .flat_map(|word| word.to_le_bytes())
                .collect();
            bytes.truncate(bitset.size.div_ceil(8));
            Ok(Bytes(bytes))
        });
        methods.add_meta_method(LuaMetaMethod::Eq, |_, bitset, other: LuaAnyUserData| {
            Ok(other
                .borrow::<Bitset>()
                .is_ok_and(|other| other.size == bitset.size && other.words == bitset.words))
        });
        methods.add_meta_method(LuaMetaMethod::Len, |_, bitset, ()| Ok(bitset.size));
        methods.add_meta_method(LuaMetaMethod::ToString, |_, bitset, ()| {
            Ok(format!("Bitset({} of {} set)", bitset.count(), bitset.size))
        });
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set(
        "New",
        lua.create_function(|lua, size: usize| Ok(Object(Bitset::new(size, index_base(lua)?)?)))?,
    )?;
    module.set(
        "FromBytes",
        lua.create_function(|lua, (data, size): (Binary, Option<usize>)| {
            let bytes = data.as_bytes();
            let size = size.unwrap_or(bytes.len() * 8);
            let mut bitset = Bitset::new(size, index_base(lua)?)?;
            for (index, chunk) in bytes.chunks(8).enumerate().take(bitset.words.len()) {
                let mut word = [0u8; 8];
                word[..chunk.len()].copy_from_slice(chunk);
                bitset.words[index] = u64::from_le_bytes(word);
            }
            bitset.trim();
            Ok(Object(bitset))
        })?,
    )?;
    module.set(
        "IsBitset",
        lua.create_function(|_, value: LuaValue| {
            Ok(matches!(value, LuaValue::UserData(userdata) if userdata.is::<Bitset>()))
        })?,
    )?;
    Ok(module)
}
