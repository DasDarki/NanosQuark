use std::f64::consts::TAU;

use mlua::prelude::*;
use quark_core::Options;

use crate::generator::with_rng;
use crate::rng::Rng;

const MAX_CELLS: usize = 16 * 1024 * 1024;
const DEFAULT_LIMIT: usize = 100_000;

struct Grid {
    cell: f64,
    columns: usize,
    rows: usize,
    cells: Vec<u32>,
}

impl Grid {
    fn place(&mut self, points: &mut Vec<(f64, f64)>, active: &mut Vec<usize>, x: f64, y: f64) {
        points.push((x, y));
        let index = self.index(x, y);
        self.cells[index] = points.len() as u32;
        active.push(points.len() - 1);
    }

    fn index(&self, x: f64, y: f64) -> usize {
        let column = ((x / self.cell) as usize).min(self.columns - 1);
        let row = ((y / self.cell) as usize).min(self.rows - 1);
        row * self.columns + column
    }

    fn is_free(&self, points: &[(f64, f64)], x: f64, y: f64, radius: f64) -> bool {
        let column = ((x / self.cell) as isize).min(self.columns as isize - 1);
        let row = ((y / self.cell) as isize).min(self.rows as isize - 1);
        let squared = radius * radius;
        for other_row in (row - 2).max(0)..=(row + 2).min(self.rows as isize - 1) {
            for other_column in (column - 2).max(0)..=(column + 2).min(self.columns as isize - 1) {
                let entry = self.cells[other_row as usize * self.columns + other_column as usize];
                if entry == 0 {
                    continue;
                }
                let (other_x, other_y) = points[entry as usize - 1];
                let (delta_x, delta_y) = (other_x - x, other_y - y);
                if delta_x * delta_x + delta_y * delta_y < squared {
                    return false;
                }
            }
        }
        true
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "PoissonDisk",
        lua.create_function(
            |lua, (width, height, radius, options): (f64, f64, f64, Options)| {
                if !(width > 0.0 && height > 0.0 && width.is_finite() && height.is_finite()) {
                    return Err(LuaError::runtime("the width and height have to be greater than 0"));
                }
                if !(radius > 0.0 && radius.is_finite()) {
                    return Err(LuaError::runtime("the radius has to be greater than 0"));
                }
                let cell = radius / std::f64::consts::SQRT_2;
                let columns = (width / cell).ceil() as usize;
                let rows = (height / cell).ceil() as usize;
                if columns.saturating_mul(rows) > MAX_CELLS {
                    return Err(LuaError::runtime(
                        "the area is too large for this radius, use a larger radius or a smaller area",
                    ));
                }
                let attempts: usize = options.get_or("Attempts", 30)?;
                let limit: usize = options.get_or("Limit", DEFAULT_LIMIT)?;
                let offset_x: f64 = options.get_or("X", 0.0)?;
                let offset_y: f64 = options.get_or("Y", 0.0)?;
                let filter: Option<LuaFunction> = options.get("Filter")?;
                let generator: Option<LuaAnyUserData> = options.get("Generator")?;
                let mut rng = Rng::from_seed(with_rng(lua, generator, |rng| rng.next_u64())?);

                let accepts = |x: f64, y: f64| -> LuaResult<bool> {
                    match &filter {
                        Some(filter) => filter.call::<bool>((x + offset_x, y + offset_y)),
                        None => Ok(true),
                    }
                };

                let mut grid = Grid {
                    cell,
                    columns,
                    rows,
                    cells: vec![0; columns * rows],
                };
                let mut points: Vec<(f64, f64)> = Vec::new();
                let mut active: Vec<usize> = Vec::new();

                for _ in 0..attempts.max(1) * 10 {
                    let (x, y) = (rng.float() * width, rng.float() * height);
                    if accepts(x, y)? {
                        grid.place(&mut points, &mut active, x, y);
                        break;
                    }
                }

                while !active.is_empty() && points.len() < limit {
                    let slot = rng.below(active.len() as u64) as usize;
                    let (center_x, center_y) = points[active[slot]];
                    let mut found = false;
                    for _ in 0..attempts {
                        let angle = rng.float() * TAU;
                        let distance = radius * (1.0 + rng.float());
                        let x = center_x + angle.cos() * distance;
                        let y = center_y + angle.sin() * distance;
                        if x < 0.0 || y < 0.0 || x >= width || y >= height {
                            continue;
                        }
                        if grid.is_free(&points, x, y, radius) && accepts(x, y)? {
                            grid.place(&mut points, &mut active, x, y);
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        active.swap_remove(slot);
                    }
                }

                let result = lua.create_table_with_capacity(points.len(), 0)?;
                for (x, y) in points {
                    let point = lua.create_table_with_capacity(0, 2)?;
                    point.raw_set("X", x + offset_x)?;
                    point.raw_set("Y", y + offset_y)?;
                    result.raw_push(point)?;
                }
                Ok(result)
            },
        )?,
    )
}
