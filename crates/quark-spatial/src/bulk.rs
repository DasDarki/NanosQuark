use std::collections::HashMap;

use mlua::prelude::*;
use quark_core::{Options, Vec3, read_vector};

use crate::spatial::{check_count, read_point_set};

fn pairs_within(points: &[Vec3], radius: f64, limit: usize) -> Vec<(usize, usize, f64)> {
    let size = radius.max(1e-9);
    let cell = |point: Vec3| {
        (
            (point.x / size).floor() as i64,
            (point.y / size).floor() as i64,
            (point.z / size).floor() as i64,
        )
    };
    let mut grid: HashMap<(i64, i64, i64), Vec<usize>> = HashMap::new();
    for (index, point) in points.iter().enumerate() {
        grid.entry(cell(*point)).or_default().push(index);
    }
    let mut pairs = Vec::new();
    for (index, point) in points.iter().enumerate() {
        let (x, y, z) = cell(*point);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let Some(entries) = grid.get(&(x + dx, y + dy, z + dz)) else {
                        continue;
                    };
                    for &other in entries {
                        if other <= index {
                            continue;
                        }
                        let distance = points[other].subtract(*point).length();
                        if distance <= radius {
                            pairs.push((index, other, distance));
                            if pairs.len() >= limit {
                                return pairs;
                            }
                        }
                    }
                }
            }
        }
    }
    pairs
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "FindPairs",
        lua.create_function(|lua, (points, radius, options): (LuaValue, f64, Options)| {
            let points = read_point_set(lua, &points)?;
            let limit: usize = options.get_or("Limit", 1_000_000)?;
            check_count(limit)?;
            let mut pairs = pairs_within(&points, radius.max(0.0), limit);
            pairs.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));
            let list = lua.create_table_with_capacity(pairs.len(), 0)?;
            for (first, second, distance) in pairs {
                let entry = lua.create_table_with_capacity(0, 3)?;
                entry.raw_set("A", first + 1)?;
                entry.raw_set("B", second + 1)?;
                entry.raw_set("Distance", distance)?;
                list.raw_push(entry)?;
            }
            Ok(list)
        })?,
    )?;
    module.set(
        "FindWithin",
        lua.create_function(
            |lua, (points, center, radius, options): (LuaValue, LuaValue, f64, Options)| {
                let points = read_point_set(lua, &points)?;
                let center = read_vector(&center)?;
                let mut found: Vec<(usize, f64)> = points
                    .iter()
                    .enumerate()
                    .map(|(index, point)| (index, point.subtract(center).length()))
                    .filter(|(_, distance)| *distance <= radius)
                    .collect();
                found.sort_by(|left, right| left.1.total_cmp(&right.1));
                let with_distance: bool = options.get_or("WithDistance", false)?;
                let list = lua.create_table_with_capacity(found.len(), 0)?;
                for (index, distance) in found {
                    if with_distance {
                        let entry = lua.create_table_with_capacity(0, 2)?;
                        entry.raw_set("Index", index + 1)?;
                        entry.raw_set("Distance", distance)?;
                        list.raw_push(entry)?;
                    } else {
                        list.raw_push(index + 1)?;
                    }
                }
                Ok(list)
            },
        )?,
    )?;
    module.set(
        "Distances",
        lua.create_function(|lua, (points, center): (LuaValue, LuaValue)| {
            let points = read_point_set(lua, &points)?;
            let center = read_vector(&center)?;
            lua.create_sequence_from(points.iter().map(|point| point.subtract(center).length()))
        })?,
    )?;
    Ok(())
}
