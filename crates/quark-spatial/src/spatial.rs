use std::collections::HashMap;

use mlua::prelude::*;
use quark_core::{Options, Vec3, create_userdata, make_vector, read_vector};

use crate::graph::Key;

const MAX_RESULTS: usize = 1_000_000;

type Cell = (i64, i64, i64);

pub struct SpatialHash {
    cell_size: f64,
    cells: HashMap<Cell, Vec<usize>>,
    items: Vec<Option<(Key, Vec3)>>,
    lookup: HashMap<Key, usize>,
    free: Vec<usize>,
}

impl SpatialHash {
    fn cell_of(&self, position: Vec3) -> Cell {
        (
            (position.x / self.cell_size).floor() as i64,
            (position.y / self.cell_size).floor() as i64,
            (position.z / self.cell_size).floor() as i64,
        )
    }

    fn detach(&mut self, slot: usize) {
        if let Some((_, position)) = &self.items[slot] {
            let cell = self.cell_of(*position);
            if let Some(entries) = self.cells.get_mut(&cell) {
                entries.retain(|entry| *entry != slot);
                if entries.is_empty() {
                    self.cells.remove(&cell);
                }
            }
        }
    }

    fn set(&mut self, key: Key, position: Vec3) {
        let slot = match self.lookup.get(&key) {
            Some(&slot) => {
                if self.items[slot]
                    .as_ref()
                    .is_some_and(|(_, old)| self.cell_of(*old) == self.cell_of(position))
                {
                    self.items[slot] = Some((key, position));
                    return;
                }
                self.detach(slot);
                slot
            }
            None => {
                let slot = self.free.pop().unwrap_or_else(|| {
                    self.items.push(None);
                    self.items.len() - 1
                });
                self.lookup.insert(key.clone(), slot);
                slot
            }
        };
        self.items[slot] = Some((key, position));
        let cell = self.cell_of(position);
        self.cells.entry(cell).or_default().push(slot);
    }

    fn remove(&mut self, key: &Key) -> bool {
        let Some(slot) = self.lookup.remove(key) else {
            return false;
        };
        self.detach(slot);
        self.items[slot] = None;
        self.free.push(slot);
        true
    }

    fn query_box(&self, minimum: Vec3, maximum: Vec3, mut visit: impl FnMut(usize, Vec3)) {
        let low = self.cell_of(minimum);
        let high = self.cell_of(maximum);
        let cells = (high.0 - low.0 + 1) * (high.1 - low.1 + 1) * (high.2 - low.2 + 1);
        if cells as usize > self.cells.len() * 4 {
            for entries in self.cells.values() {
                for &slot in entries {
                    if let Some((_, position)) = &self.items[slot] {
                        visit(slot, *position);
                    }
                }
            }
            return;
        }
        for x in low.0..=high.0 {
            for y in low.1..=high.1 {
                for z in low.2..=high.2 {
                    if let Some(entries) = self.cells.get(&(x, y, z)) {
                        for &slot in entries {
                            if let Some((_, position)) = &self.items[slot] {
                                visit(slot, *position);
                            }
                        }
                    }
                }
            }
        }
    }

    fn within(&self, center: Vec3, radius: f64) -> Vec<(usize, f64)> {
        let extent = Vec3::new(radius, radius, radius);
        let mut found = Vec::new();
        self.query_box(
            center.subtract(extent),
            center.plus(extent),
            |slot, position| {
                let distance = position.subtract(center).length();
                if distance <= radius {
                    found.push((slot, distance));
                }
            },
        );
        found.sort_by(|left, right| left.1.total_cmp(&right.1));
        found
    }
}

fn result_list(
    lua: &Lua,
    hash: &SpatialHash,
    found: Vec<(usize, f64)>,
    with_distance: bool,
) -> LuaResult<LuaTable> {
    let list = lua.create_table_with_capacity(found.len(), 0)?;
    for (slot, distance) in found {
        if let Some((key, _)) = &hash.items[slot] {
            if with_distance {
                let entry = lua.create_table_with_capacity(0, 2)?;
                entry.raw_set("ID", key.clone().into_lua(lua)?)?;
                entry.raw_set("Distance", distance)?;
                list.raw_push(entry)?;
            } else {
                list.raw_push(key.clone().into_lua(lua)?)?;
            }
        }
    }
    Ok(list)
}

impl LuaUserData for SpatialHash {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("Set", |_, hash, (id, position): (LuaValue, LuaValue)| {
            hash.set(Key::read(&id)?, read_vector(&position)?);
            Ok(())
        });
        methods.add_method_mut("Remove", |_, hash, id: LuaValue| {
            Ok(hash.remove(&Key::read(&id)?))
        });
        methods.add_method("Has", |_, hash, id: LuaValue| {
            Ok(hash.lookup.contains_key(&Key::read(&id)?))
        });
        methods.add_method("GetPosition", |lua, hash, id: LuaValue| {
            match hash.lookup.get(&Key::read(&id)?) {
                Some(&slot) => match &hash.items[slot] {
                    Some((_, position)) => make_vector(lua, *position),
                    None => Ok(LuaNil),
                },
                None => Ok(LuaNil),
            }
        });
        methods.add_method("GetCount", |_, hash, ()| Ok(hash.lookup.len()));
        methods.add_method_mut("Clear", |_, hash, ()| {
            hash.cells.clear();
            hash.items.clear();
            hash.lookup.clear();
            hash.free.clear();
            Ok(())
        });
        methods.add_method(
            "QueryRadius",
            |lua, hash, (center, radius, options): (LuaValue, f64, Options)| {
                let mut found = hash.within(read_vector(&center)?, radius.max(0.0));
                if let Some(limit) = options.get::<usize>("Limit")? {
                    found.truncate(limit);
                }
                result_list(lua, hash, found, options.get_or("WithDistance", false)?)
            },
        );
        methods.add_method(
            "QueryBox",
            |lua, hash, (minimum, maximum): (LuaValue, LuaValue)| {
                let (low, high) = (read_vector(&minimum)?, read_vector(&maximum)?);
                let (low, high) = (
                    Vec3::new(low.x.min(high.x), low.y.min(high.y), low.z.min(high.z)),
                    Vec3::new(low.x.max(high.x), low.y.max(high.y), low.z.max(high.z)),
                );
                let mut found = Vec::new();
                hash.query_box(low, high, |slot, position| {
                    if (low.x..=high.x).contains(&position.x)
                        && (low.y..=high.y).contains(&position.y)
                        && (low.z..=high.z).contains(&position.z)
                    {
                        found.push((slot, 0.0));
                    }
                });
                result_list(lua, hash, found, false)
            },
        );
        methods.add_method(
            "FindNearest",
            |lua, hash, (center, options): (LuaValue, Options)| {
                let center = read_vector(&center)?;
                let count: usize = options.get_or("Count", 1)?;
                let max_distance: f64 = options.get_or("MaxDistance", f64::INFINITY)?;
                let exclude = options
                    .get::<LuaValue>("Exclude")?
                    .map(|value| Key::read(&value))
                    .transpose()?;
                let mut radius = hash.cell_size;
                loop {
                    let mut found = hash.within(center, radius.min(max_distance));
                    found.retain(|(slot, _)| {
                        exclude.as_ref().is_none_or(|exclude| {
                            hash.items[*slot]
                                .as_ref()
                                .is_some_and(|(key, _)| key != exclude)
                        })
                    });
                    let total = hash.lookup.len() - usize::from(exclude.is_some());
                    if found.len() >= count || radius >= max_distance || found.len() >= total {
                        found.truncate(count);
                        return result_list(
                            lua,
                            hash,
                            found,
                            options.get_or("WithDistance", false)?,
                        );
                    }
                    radius *= 2.0;
                    if radius > 1e12 {
                        radius = max_distance.min(f64::MAX);
                    }
                }
            },
        );
    }
}

pub fn create_hash(lua: &Lua, cell_size: Option<f64>) -> LuaResult<LuaAnyUserData> {
    let cell_size = cell_size.unwrap_or(1000.0);
    if !(cell_size > 0.0 && cell_size.is_finite()) {
        return Err(LuaError::runtime("the cell size has to be greater than 0"));
    }
    create_userdata(
        lua,
        SpatialHash {
            cell_size,
            cells: HashMap::new(),
            items: Vec::new(),
            lookup: HashMap::new(),
            free: Vec::new(),
        },
    )
}

pub fn read_point_set(lua: &Lua, points: &LuaValue) -> LuaResult<Vec<Vec3>> {
    let _ = lua;
    match points {
        LuaValue::Table(table) => table
            .sequence_values::<LuaValue>()
            .map(|value| read_vector(&value?))
            .collect(),
        LuaValue::UserData(userdata) => {
            let numbers = quark_core::view_numbers(userdata)?.ok_or_else(|| {
                LuaError::runtime(
                    "expected a list of positions or a buffer view with X, Y, Z triples",
                )
            })?;
            if numbers.len() % 3 != 0 {
                return Err(LuaError::runtime(
                    "the buffer view needs three values (X, Y, Z) per point",
                ));
            }
            Ok(numbers
                .chunks_exact(3)
                .map(|chunk| Vec3::new(chunk[0], chunk[1], chunk[2]))
                .collect())
        }
        other => Err(LuaError::runtime(format!(
            "expected a list of positions or a buffer view, got {}",
            other.type_name()
        ))),
    }
}

pub fn check_count(count: usize) -> LuaResult<()> {
    if count > MAX_RESULTS {
        return Err(LuaError::runtime(format!(
            "at most {MAX_RESULTS} results can be returned at once"
        )));
    }
    Ok(())
}
