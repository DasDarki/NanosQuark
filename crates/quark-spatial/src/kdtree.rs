use mlua::prelude::*;
use quark_core::{Options, Vec3, create_userdata, read_vector};

use crate::spatial::{check_count, read_point_set};

pub struct KDTree {
    points: Vec<Vec3>,
    order: Vec<usize>,
}

fn axis_value(point: Vec3, axis: usize) -> f64 {
    match axis {
        0 => point.x,
        1 => point.y,
        _ => point.z,
    }
}

fn build(points: &[Vec3], order: &mut [usize], depth: usize) {
    if order.len() <= 1 {
        return;
    }
    let axis = depth % 3;
    let middle = order.len() / 2;
    order.select_nth_unstable_by(middle, |left, right| {
        axis_value(points[*left], axis).total_cmp(&axis_value(points[*right], axis))
    });
    let (left, right) = order.split_at_mut(middle);
    build(points, left, depth + 1);
    build(points, &mut right[1..], depth + 1);
}

impl KDTree {
    fn nearest(&self, target: Vec3, count: usize, max_distance: f64) -> Vec<(usize, f64)> {
        let mut best: Vec<(usize, f64)> = Vec::with_capacity(count + 1);
        self.search(
            &self.order,
            0,
            target,
            count,
            max_distance * max_distance,
            &mut best,
        );
        best.into_iter()
            .map(|(index, squared)| (index, squared.sqrt()))
            .collect()
    }

    fn search(
        &self,
        order: &[usize],
        depth: usize,
        target: Vec3,
        count: usize,
        limit: f64,
        best: &mut Vec<(usize, f64)>,
    ) {
        if order.is_empty() {
            return;
        }
        let middle = order.len() / 2;
        let index = order[middle];
        let point = self.points[index];
        let delta = point.subtract(target);
        let squared = delta.dot(delta);
        if squared <= limit && (best.len() < count || squared < best[best.len() - 1].1) {
            let position = best.partition_point(|(_, distance)| *distance <= squared);
            best.insert(position, (index, squared));
            best.truncate(count);
        }
        let axis = depth % 3;
        let difference = axis_value(target, axis) - axis_value(point, axis);
        let (near, far) = if difference < 0.0 {
            (&order[..middle], &order[middle + 1..])
        } else {
            (&order[middle + 1..], &order[..middle])
        };
        self.search(near, depth + 1, target, count, limit, best);
        let worst = if best.len() < count {
            limit
        } else {
            best[best.len() - 1].1.min(limit)
        };
        if difference * difference <= worst {
            self.search(far, depth + 1, target, count, limit, best);
        }
    }

    fn within(
        &self,
        order: &[usize],
        depth: usize,
        target: Vec3,
        squared_radius: f64,
        found: &mut Vec<(usize, f64)>,
    ) {
        if order.is_empty() {
            return;
        }
        let middle = order.len() / 2;
        let index = order[middle];
        let point = self.points[index];
        let delta = point.subtract(target);
        let squared = delta.dot(delta);
        if squared <= squared_radius {
            found.push((index, squared.sqrt()));
        }
        let axis = depth % 3;
        let difference = axis_value(target, axis) - axis_value(point, axis);
        if difference <= 0.0 || difference * difference <= squared_radius {
            self.within(&order[..middle], depth + 1, target, squared_radius, found);
        }
        if difference >= 0.0 || difference * difference <= squared_radius {
            self.within(
                &order[middle + 1..],
                depth + 1,
                target,
                squared_radius,
                found,
            );
        }
    }
}

fn indices(lua: &Lua, found: Vec<(usize, f64)>, with_distance: bool) -> LuaResult<LuaTable> {
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
}

impl LuaUserData for KDTree {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method(
            "FindNearest",
            |lua, tree, (target, options): (LuaValue, Options)| {
                let count: usize = options.get_or("Count", 1)?;
                check_count(count)?;
                let found = tree.nearest(
                    read_vector(&target)?,
                    count,
                    options.get_or("MaxDistance", f64::INFINITY)?,
                );
                indices(lua, found, options.get_or("WithDistance", false)?)
            },
        );
        methods.add_method(
            "QueryRadius",
            |lua, tree, (target, radius, options): (LuaValue, f64, Options)| {
                let mut found = Vec::new();
                tree.within(
                    &tree.order,
                    0,
                    read_vector(&target)?,
                    radius.max(0.0).powi(2),
                    &mut found,
                );
                found.sort_by(|left, right| left.1.total_cmp(&right.1));
                indices(lua, found, options.get_or("WithDistance", false)?)
            },
        );
        methods.add_method("GetCount", |_, tree, ()| Ok(tree.points.len()));
    }
}

pub fn create(lua: &Lua, points: LuaValue) -> LuaResult<LuaAnyUserData> {
    let points = read_point_set(lua, &points)?;
    let mut order: Vec<usize> = (0..points.len()).collect();
    build(&points, &mut order, 0);
    create_userdata(lua, KDTree { points, order })
}
