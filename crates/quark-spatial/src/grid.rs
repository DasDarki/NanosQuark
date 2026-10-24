use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;

use mlua::prelude::*;
use quark_core::{
    Object, Options, Vec3, create_userdata, index_base, make_vector, options_and_callback,
    read_vector,
};

const MAX_CELLS: usize = 16 * 1024 * 1024;
const SQRT2: f64 = std::f64::consts::SQRT_2;

#[derive(Clone)]
pub struct GridData {
    pub width: usize,
    pub height: usize,
    pub costs: Vec<f32>,
    pub diagonal: bool,
    pub corner_cutting: bool,
}

#[derive(Clone, Copy, PartialEq)]
struct Entry {
    priority: f64,
    cell: usize,
}

impl Eq for Entry {}

impl Ord for Entry {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .priority
            .total_cmp(&self.priority)
            .then(other.cell.cmp(&self.cell))
    }
}

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl GridData {
    pub fn walkable(&self, x: isize, y: isize) -> bool {
        x >= 0
            && y >= 0
            && (x as usize) < self.width
            && (y as usize) < self.height
            && self.costs[y as usize * self.width + x as usize].is_finite()
    }

    fn neighbors(&self, cell: usize, output: &mut Vec<(usize, f64)>) {
        output.clear();
        let (x, y) = ((cell % self.width) as isize, (cell / self.width) as isize);
        for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            if self.walkable(x + dx, y + dy) {
                let next = (y + dy) as usize * self.width + (x + dx) as usize;
                output.push((next, f64::from(self.costs[next])));
            }
        }
        if !self.diagonal {
            return;
        }
        for (dx, dy) in [(1, 1), (1, -1), (-1, 1), (-1, -1)] {
            if !self.walkable(x + dx, y + dy) {
                continue;
            }
            let side_a = self.walkable(x + dx, y);
            let side_b = self.walkable(x, y + dy);
            if !(side_a && side_b || self.corner_cutting && (side_a || side_b)) {
                continue;
            }
            let next = (y + dy) as usize * self.width + (x + dx) as usize;
            output.push((next, f64::from(self.costs[next]) * SQRT2));
        }
    }

    fn heuristic(&self, from: usize, to: usize, minimum_cost: f64) -> f64 {
        let dx = ((from % self.width) as f64 - (to % self.width) as f64).abs();
        let dy = ((from / self.width) as f64 - (to / self.width) as f64).abs();
        minimum_cost
            * if self.diagonal {
                dx.max(dy) + (SQRT2 - 1.0) * dx.min(dy)
            } else {
                dx + dy
            }
    }

    pub fn find_path(
        &self,
        start: usize,
        goal: usize,
        use_heuristic: bool,
        max_nodes: usize,
    ) -> Result<(Vec<usize>, f64), String> {
        if !self.costs[start].is_finite() {
            return Err("the start cell is blocked".to_owned());
        }
        if !self.costs[goal].is_finite() {
            return Err("the goal cell is blocked".to_owned());
        }
        let minimum_cost = if use_heuristic {
            self.costs
                .iter()
                .copied()
                .filter(|cost| cost.is_finite())
                .fold(f32::INFINITY, f32::min)
                .into()
        } else {
            0.0
        };
        let mut distances = vec![f64::INFINITY; self.costs.len()];
        let mut previous = vec![usize::MAX; self.costs.len()];
        let mut closed = vec![false; self.costs.len()];
        let mut open = BinaryHeap::new();
        let mut neighbors = Vec::with_capacity(8);
        distances[start] = 0.0;
        open.push(Entry {
            priority: self.heuristic(start, goal, minimum_cost),
            cell: start,
        });
        let mut visited = 0;

        while let Some(Entry { cell, .. }) = open.pop() {
            if closed[cell] {
                continue;
            }
            if cell == goal {
                let mut path = vec![goal];
                let mut current = goal;
                while current != start {
                    current = previous[current];
                    path.push(current);
                }
                path.reverse();
                return Ok((path, distances[goal]));
            }
            closed[cell] = true;
            visited += 1;
            if visited > max_nodes {
                return Err(format!("no path found within {max_nodes} visited cells"));
            }
            self.neighbors(cell, &mut neighbors);
            for &(next, cost) in &neighbors {
                let candidate = distances[cell] + cost;
                if candidate < distances[next] {
                    distances[next] = candidate;
                    previous[next] = cell;
                    open.push(Entry {
                        priority: candidate + self.heuristic(next, goal, minimum_cost),
                        cell: next,
                    });
                }
            }
        }
        Err("there is no path between the cells".to_owned())
    }

    pub fn distances(&self, goals: &[usize], max_cost: f64) -> Vec<f64> {
        let mut distances = vec![f64::INFINITY; self.costs.len()];
        let mut open = BinaryHeap::new();
        let mut neighbors = Vec::with_capacity(8);
        for &goal in goals {
            if self.costs[goal].is_finite() {
                distances[goal] = 0.0;
                open.push(Entry {
                    priority: 0.0,
                    cell: goal,
                });
            }
        }
        while let Some(Entry { priority, cell }) = open.pop() {
            if priority > distances[cell] || priority > max_cost {
                continue;
            }
            self.neighbors(cell, &mut neighbors);
            for &(next, _) in &neighbors {
                let cost = if next % self.width != cell % self.width
                    && next / self.width != cell / self.width
                {
                    f64::from(self.costs[cell]) * SQRT2
                } else {
                    f64::from(self.costs[cell])
                };
                let candidate = distances[cell] + cost;
                if candidate < distances[next] {
                    distances[next] = candidate;
                    open.push(Entry {
                        priority: candidate,
                        cell: next,
                    });
                }
            }
        }
        distances
    }

    pub fn line_of_sight(&self, from: usize, to: usize) -> bool {
        let (mut x0, mut y0) = ((from % self.width) as isize, (from / self.width) as isize);
        let (x1, y1) = ((to % self.width) as isize, (to / self.width) as isize);
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let mut error = dx + dy;
        loop {
            if !self.walkable(x0, y0) {
                return false;
            }
            if x0 == x1 && y0 == y1 {
                return true;
            }
            let doubled = 2 * error;
            if doubled >= dy
                && doubled <= dx
                && (!self.walkable(x0 + sx, y0) || !self.walkable(x0, y0 + sy))
            {
                return false;
            }
            if doubled >= dy {
                error += dy;
                x0 += sx;
            }
            if doubled <= dx {
                error += dx;
                y0 += sy;
            }
        }
    }

    pub fn smooth(&self, path: Vec<usize>) -> Vec<usize> {
        if path.len() <= 2 {
            return path;
        }
        let mut result = vec![path[0]];
        let mut anchor = 0;
        let mut index = 2;
        while index < path.len() {
            if !self.line_of_sight(path[anchor], path[index]) {
                result.push(path[index - 1]);
                anchor = index - 1;
            }
            index += 1;
        }
        result.push(*path.last().unwrap_or(&path[0]));
        result
    }
}

pub struct Grid {
    data: Arc<GridData>,
    base: i64,
    origin: Vec3,
    cell_size: f64,
}

fn cost_value(value: LuaValue) -> LuaResult<f32> {
    match value {
        LuaValue::Boolean(false) | LuaNil => Ok(f32::INFINITY),
        LuaValue::Boolean(true) => Ok(1.0),
        LuaValue::Integer(integer) if integer > 0 => Ok(integer as f32),
        LuaValue::Number(number) if number > 0.0 => Ok(number as f32),
        LuaValue::Integer(_) | LuaValue::Number(_) => Ok(f32::INFINITY),
        other => Err(LuaError::runtime(format!(
            "a cell cost is a positive number, got {}",
            other.type_name()
        ))),
    }
}

impl Grid {
    fn cell(&self, x: i64, y: i64) -> LuaResult<usize> {
        let (column, row) = (x - self.base, y - self.base);
        if column < 0
            || row < 0
            || column as usize >= self.data.width
            || row as usize >= self.data.height
        {
            return Err(LuaError::runtime(format!(
                "the cell {x}, {y} is outside of the grid ({} to {}, {} to {})",
                self.base,
                self.base + self.data.width as i64 - 1,
                self.base,
                self.base + self.data.height as i64 - 1
            )));
        }
        Ok(row as usize * self.data.width + column as usize)
    }

    fn read_cell(&self, x: LuaValue, y: Option<i64>) -> LuaResult<usize> {
        match (x, y) {
            (LuaValue::Integer(x), Some(y)) => self.cell(x, y),
            (value @ (LuaValue::Table(_) | LuaValue::UserData(_)), None) => {
                let vector = read_vector(&value)?;
                self.cell(vector.x as i64, vector.y as i64)
            }
            _ => Err(LuaError::runtime(
                "expected x and y, or a table with X and Y",
            )),
        }
    }
}

struct FoundPath {
    cells: Vec<usize>,
    cost: f64,
    width: usize,
    base: i64,
}

impl IntoLuaMulti for FoundPath {
    fn into_lua_multi(self, lua: &Lua) -> LuaResult<LuaMultiValue> {
        let list = lua.create_table_with_capacity(self.cells.len(), 0)?;
        for cell in self.cells {
            let point = lua.create_table_with_capacity(0, 2)?;
            point.raw_set("X", (cell % self.width) as i64 + self.base)?;
            point.raw_set("Y", (cell / self.width) as i64 + self.base)?;
            list.raw_push(point)?;
        }
        (list, self.cost).into_lua_multi(lua)
    }
}

pub struct FlowField {
    data: Arc<GridData>,
    distances: Vec<f64>,
    base: i64,
}

impl FlowField {
    fn cell(&self, x: i64, y: i64) -> LuaResult<usize> {
        let (column, row) = (x - self.base, y - self.base);
        if column < 0
            || row < 0
            || column as usize >= self.data.width
            || row as usize >= self.data.height
        {
            return Err(LuaError::runtime(format!(
                "the cell {x}, {y} is outside of the grid"
            )));
        }
        Ok(row as usize * self.data.width + column as usize)
    }

    fn next(&self, cell: usize) -> Option<usize> {
        if self.distances[cell] == 0.0 || !self.distances[cell].is_finite() {
            return None;
        }
        let mut neighbors = Vec::with_capacity(8);
        self.data.neighbors(cell, &mut neighbors);
        neighbors
            .iter()
            .map(|(next, _)| *next)
            .filter(|next| self.distances[*next] < self.distances[cell])
            .min_by(|left, right| self.distances[*left].total_cmp(&self.distances[*right]))
    }
}

impl LuaUserData for FlowField {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetDirection", |lua, field, (x, y): (i64, i64)| {
            let cell = field.cell(x, y)?;
            match field.next(cell) {
                Some(next) => {
                    let width = field.data.width;
                    let dx = (next % width) as i64 - (cell % width) as i64;
                    let dy = (next / width) as i64 - (cell / width) as i64;
                    (dx, dy).into_lua_multi(lua)
                }
                None => (0, 0).into_lua_multi(lua),
            }
        });
        methods.add_method("GetDistance", |_, field, (x, y): (i64, i64)| {
            let distance = field.distances[field.cell(x, y)?];
            Ok(distance.is_finite().then_some(distance))
        });
        methods.add_method("GetPath", |lua, field, (x, y): (i64, i64)| {
            let mut cell = field.cell(x, y)?;
            if !field.distances[cell].is_finite() {
                return Ok(None);
            }
            let mut path = vec![cell];
            while let Some(next) = field.next(cell) {
                path.push(next);
                cell = next;
            }
            let list = lua.create_table_with_capacity(path.len(), 0)?;
            for cell in path {
                let point = lua.create_table_with_capacity(0, 2)?;
                point.raw_set("X", (cell % field.data.width) as i64 + field.base)?;
                point.raw_set("Y", (cell / field.data.width) as i64 + field.base)?;
                list.raw_push(point)?;
            }
            Ok(Some(list))
        });
        methods.add_method("IsReachable", |_, field, (x, y): (i64, i64)| {
            Ok(field.distances[field.cell(x, y)?].is_finite())
        });
    }
}

fn snapshot_path(
    data: Arc<GridData>,
    start: usize,
    goal: usize,
    options: (bool, usize, bool),
    base: i64,
) -> Result<FoundPath, String> {
    let (use_heuristic, max_nodes, smooth) = options;
    let (mut cells, cost) = data.find_path(start, goal, use_heuristic, max_nodes)?;
    if smooth {
        cells = data.smooth(cells);
    }
    Ok(FoundPath {
        cells,
        cost,
        width: data.width,
        base,
    })
}

impl LuaUserData for Grid {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetSize", |_, grid, ()| {
            Ok((grid.data.width, grid.data.height))
        });
        methods.add_method_mut("SetCost", |_, grid, (x, y, cost): (i64, i64, LuaValue)| {
            let cell = grid.cell(x, y)?;
            Arc::make_mut(&mut grid.data).costs[cell] = cost_value(cost)?;
            Ok(())
        });
        methods.add_method("GetCost", |_, grid, (x, y): (i64, i64)| {
            let cost = grid.data.costs[grid.cell(x, y)?];
            Ok(cost.is_finite().then_some(f64::from(cost)))
        });
        methods.add_method_mut(
            "SetBlocked",
            |_, grid, (x, y, blocked): (i64, i64, Option<bool>)| {
                let cell = grid.cell(x, y)?;
                let data = Arc::make_mut(&mut grid.data);
                data.costs[cell] = if blocked.unwrap_or(true) {
                    f32::INFINITY
                } else {
                    1.0
                };
                Ok(())
            },
        );
        methods.add_method("IsBlocked", |_, grid, (x, y): (i64, i64)| {
            Ok(!grid.data.costs[grid.cell(x, y)?].is_finite())
        });
        methods.add_method_mut("Fill", |_, grid, cost: LuaValue| {
            let cost = cost_value(cost)?;
            Arc::make_mut(&mut grid.data)
                .costs
                .iter_mut()
                .for_each(|cell| *cell = cost);
            Ok(())
        });
        methods.add_method_mut(
            "FillRect",
            |_, grid, (x1, y1, x2, y2, cost): (i64, i64, i64, i64, LuaValue)| {
                let cost = cost_value(cost)?;
                let (low_x, high_x) = (x1.min(x2), x1.max(x2));
                let (low_y, high_y) = (y1.min(y2), y1.max(y2));
                grid.cell(low_x, low_y)?;
                grid.cell(high_x, high_y)?;
                let base = grid.base;
                let width = grid.data.width;
                let data = Arc::make_mut(&mut grid.data);
                for y in low_y..=high_y {
                    for x in low_x..=high_x {
                        data.costs[(y - base) as usize * width + (x - base) as usize] = cost;
                    }
                }
                Ok(())
            },
        );
        methods.add_method_mut("LoadCosts", |_, grid, values: LuaValue| {
            let numbers = match &values {
                LuaValue::Table(table) => table
                    .sequence_values::<f64>()
                    .collect::<LuaResult<Vec<_>>>()?,
                LuaValue::UserData(userdata) => {
                    quark_core::view_numbers(userdata)?.ok_or_else(|| {
                        LuaError::runtime("expected a list of costs or a buffer view")
                    })?
                }
                other => {
                    return Err(LuaError::runtime(format!(
                        "expected a list of costs or a buffer view, got {}",
                        other.type_name()
                    )));
                }
            };
            if numbers.len() != grid.data.costs.len() {
                return Err(LuaError::runtime(format!(
                    "expected {} costs (width * height, row by row), got {}",
                    grid.data.costs.len(),
                    numbers.len()
                )));
            }
            let data = Arc::make_mut(&mut grid.data);
            for (cell, cost) in data.costs.iter_mut().zip(numbers) {
                *cell = if cost > 0.0 && cost.is_finite() {
                    cost as f32
                } else {
                    f32::INFINITY
                };
            }
            Ok(())
        });
        methods.add_method(
            "FindPath",
            |lua, grid, (from, to, options, callback): (LuaValue, LuaValue, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                let start = grid.read_cell(from, None)?;
                let goal = grid.read_cell(to, None)?;
                let algorithm: String = options.get_or("Algorithm", "AStar".to_owned())?;
                let use_heuristic = match algorithm.as_str() {
                    "AStar" => true,
                    "Dijkstra" => false,
                    other => return Err(LuaError::runtime(format!("unknown algorithm '{other}', expected AStar or Dijkstra"))),
                };
                let settings = (use_heuristic, options.get_or("MaxNodes", usize::MAX)?, options.get_or("Smooth", false)?);
                let data = Arc::clone(&grid.data);
                let base = grid.base;
                quark_core::run(lua, callback, move || snapshot_path(data, start, goal, settings, base))
            },
        );
        methods.add_method(
            "HasLineOfSight",
            |_, grid, (from, to): (LuaValue, LuaValue)| {
                Ok(grid
                    .data
                    .line_of_sight(grid.read_cell(from, None)?, grid.read_cell(to, None)?))
            },
        );
        methods.add_method(
            "FlowField",
            |lua, grid, (goals, options, callback): (LuaValue, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                let goals = match &goals {
                    LuaValue::Table(table)
                        if table.raw_len() > 0 && table.raw_get::<LuaValue>(1)?.is_table() =>
                    {
                        table
                            .sequence_values::<LuaValue>()
                            .map(|goal| grid.read_cell(goal?, None))
                            .collect::<LuaResult<Vec<_>>>()?
                    }
                    other => vec![grid.read_cell(other.clone(), None)?],
                };
                let max_cost: f64 = options.get_or("MaxCost", f64::INFINITY)?;
                let data = Arc::clone(&grid.data);
                let base = grid.base;
                let compute = move || {
                    let distances = data.distances(&goals, max_cost);
                    Ok::<_, String>(Object(FlowField {
                        data,
                        distances,
                        base,
                    }))
                };
                match callback {
                    None => compute()
                        .map_err(LuaError::runtime)
                        .and_then(|field| create_userdata(lua, field.0))?
                        .into_lua_multi(lua),
                    Some(callback) => quark_core::run(lua, Some(callback), compute),
                }
            },
        );
        methods.add_method("ToCell", |lua, grid, position: LuaValue| {
            let position = read_vector(&position)?;
            let x = ((position.x - grid.origin.x) / grid.cell_size).floor() as i64 + grid.base;
            let y = ((position.y - grid.origin.y) / grid.cell_size).floor() as i64 + grid.base;
            let inside = grid.cell(x, y).is_ok();
            (x, y, inside).into_lua_multi(lua)
        });
        methods.add_method(
            "ToWorld",
            |lua, grid, (x, y, z): (i64, i64, Option<f64>)| {
                make_vector(
                    lua,
                    Vec3::new(
                        grid.origin.x + ((x - grid.base) as f64 + 0.5) * grid.cell_size,
                        grid.origin.y + ((y - grid.base) as f64 + 0.5) * grid.cell_size,
                        z.unwrap_or(grid.origin.z),
                    ),
                )
            },
        );
        methods.add_method("Clone", |lua, grid, ()| {
            create_userdata(
                lua,
                Grid {
                    data: Arc::new((*grid.data).clone()),
                    base: grid.base,
                    origin: grid.origin,
                    cell_size: grid.cell_size,
                },
            )
        });
    }
}

pub fn create(
    lua: &Lua,
    (width, height, options): (usize, usize, Options),
) -> LuaResult<LuaAnyUserData> {
    if width == 0 || height == 0 || width.saturating_mul(height) > MAX_CELLS {
        return Err(LuaError::runtime(format!(
            "a grid needs between 1 and {MAX_CELLS} cells"
        )));
    }
    let cell_size: f64 = options.get_or("CellSize", 100.0)?;
    if cell_size <= 0.0 {
        return Err(LuaError::runtime("CellSize has to be greater than 0"));
    }
    let origin = match options.get::<LuaValue>("Origin")? {
        Some(value) => read_vector(&value)?,
        None => Vec3::default(),
    };
    create_userdata(
        lua,
        Grid {
            data: Arc::new(GridData {
                width,
                height,
                costs: vec![1.0; width * height],
                diagonal: options.get_or("Diagonal", true)?,
                corner_cutting: options.get_or("CornerCutting", false)?,
            }),
            base: index_base(lua)?,
            origin,
            cell_size,
        },
    )
}
