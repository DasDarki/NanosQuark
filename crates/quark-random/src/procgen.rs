use std::collections::VecDeque;

use mlua::prelude::*;
use quark_core::{Buffer, Object, Options, create_userdata, index_base, options_and_callback};

use crate::generator::{Generator, with_rng};
use crate::rng::Rng;

const MAX_CELLS: usize = 16 * 1024 * 1024;
const WALL: u8 = 0;
const FLOOR: u8 = 1;
const CORRIDOR: u8 = 2;
const DOOR: u8 = 3;

#[derive(Clone, Copy)]
struct Room {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

impl Room {
    fn center(&self) -> (usize, usize) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    fn overlaps(&self, other: &Room, gap: usize) -> bool {
        self.x < other.x + other.width + gap
            && other.x < self.x + self.width + gap
            && self.y < other.y + other.height + gap
            && other.y < self.y + self.height + gap
    }
}

pub struct Map {
    width: usize,
    height: usize,
    tiles: Vec<u8>,
    rooms: Vec<Room>,
    base: i64,
}

impl Map {
    fn index(&self, x: i64, y: i64) -> LuaResult<usize> {
        let (column, row) = (x - self.base, y - self.base);
        if column < 0 || row < 0 || column as usize >= self.width || row as usize >= self.height {
            return Err(LuaError::runtime(format!(
                "the cell {x}, {y} is outside of the map"
            )));
        }
        Ok(row as usize * self.width + column as usize)
    }
}

impl LuaUserData for Map {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetSize", |_, map, ()| Ok((map.width, map.height)));
        methods.add_method("GetTile", |_, map, (x, y): (i64, i64)| {
            Ok(map.tiles[map.index(x, y)?])
        });
        methods.add_method("IsWalkable", |_, map, (x, y): (i64, i64)| {
            Ok(map.tiles[map.index(x, y)?] != WALL)
        });
        methods.add_method_mut("SetTile", |_, map, (x, y, tile): (i64, i64, u8)| {
            let index = map.index(x, y)?;
            map.tiles[index] = tile;
            Ok(())
        });
        methods.add_method("GetTiles", |lua, map, ()| {
            Ok(Object(Buffer::new(lua, map.tiles.clone())?))
        });
        methods.add_method("ToCosts", |lua, map, ()| {
            lua.create_sequence_from(
                map.tiles
                    .iter()
                    .map(|tile| if *tile == WALL { 0 } else { 1 }),
            )
        });
        methods.add_method("GetRooms", |lua, map, ()| {
            let list = lua.create_table_with_capacity(map.rooms.len(), 0)?;
            for room in &map.rooms {
                let entry = lua.create_table()?;
                entry.set("X", room.x as i64 + map.base)?;
                entry.set("Y", room.y as i64 + map.base)?;
                entry.set("Width", room.width)?;
                entry.set("Height", room.height)?;
                let (center_x, center_y) = room.center();
                let center = lua.create_table()?;
                center.set("X", center_x as i64 + map.base)?;
                center.set("Y", center_y as i64 + map.base)?;
                entry.set("Center", center)?;
                list.raw_push(entry)?;
            }
            Ok(list)
        });
        methods.add_method("GetFloorCells", |lua, map, ()| {
            let list = lua.create_table()?;
            for (index, tile) in map.tiles.iter().enumerate() {
                if *tile != WALL {
                    let cell = lua.create_table_with_capacity(0, 2)?;
                    cell.raw_set("X", (index % map.width) as i64 + map.base)?;
                    cell.raw_set("Y", (index / map.width) as i64 + map.base)?;
                    list.raw_push(cell)?;
                }
            }
            Ok(list)
        });
        methods.add_method("ToText", |_, map, ()| {
            let mut text = String::with_capacity((map.width + 1) * map.height);
            for row in 0..map.height {
                for column in 0..map.width {
                    text.push(match map.tiles[row * map.width + column] {
                        WALL => '#',
                        FLOOR => '.',
                        CORRIDOR => ',',
                        DOOR => '+',
                        _ => '?',
                    });
                }
                text.push('\n');
            }
            Ok(text)
        });
    }
}

fn check_size(width: usize, height: usize) -> LuaResult<()> {
    if width < 3 || height < 3 || width.saturating_mul(height) > MAX_CELLS {
        return Err(LuaError::runtime(format!(
            "the map needs at least 3 by 3 and at most {MAX_CELLS} cells"
        )));
    }
    Ok(())
}

fn seeded_rng(lua: &Lua, options: &Options) -> LuaResult<Rng> {
    let generator = match options.get::<LuaValue>("Generator")? {
        Some(LuaValue::UserData(userdata)) if userdata.is::<Generator>() => Some(userdata),
        None => None,
        Some(_) => {
            return Err(LuaError::runtime(
                "the Generator option has to be a generator from Random.New",
            ));
        }
    };
    Ok(Rng::from_seed(with_rng(lua, generator, |rng| {
        rng.next_u64()
    })?))
}

fn regions(width: usize, height: usize, tiles: &[u8]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; tiles.len()];
    let mut result = Vec::new();
    for start in 0..tiles.len() {
        if seen[start] || tiles[start] == WALL {
            continue;
        }
        let mut region = Vec::new();
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        while let Some(cell) = queue.pop_front() {
            region.push(cell);
            let (x, y) = (cell % width, cell / width);
            let mut visit = |next: usize| {
                if !seen[next] && tiles[next] != WALL {
                    seen[next] = true;
                    queue.push_back(next);
                }
            };
            if x > 0 {
                visit(cell - 1);
            }
            if x + 1 < width {
                visit(cell + 1);
            }
            if y > 0 {
                visit(cell - width);
            }
            if y + 1 < height {
                visit(cell + width);
            }
        }
        result.push(region);
    }
    result
}

#[derive(Clone, Copy)]
struct CaveSettings {
    fill: f64,
    steps: usize,
    birth: usize,
    death: usize,
    keep_largest: bool,
}

fn caves(rng: &mut Rng, width: usize, height: usize, settings: CaveSettings) -> Vec<u8> {
    let CaveSettings {
        fill,
        steps,
        birth,
        death,
        keep_largest,
    } = settings;
    let border = |x: usize, y: usize| x == 0 || y == 0 || x + 1 == width || y + 1 == height;
    let mut tiles: Vec<u8> = (0..width * height)
        .map(|index| {
            let (x, y) = (index % width, index / width);
            if border(x, y) || rng.float() < fill {
                WALL
            } else {
                FLOOR
            }
        })
        .collect();
    for _ in 0..steps {
        let mut next = tiles.clone();
        for y in 0..height {
            for x in 0..width {
                if border(x, y) {
                    continue;
                }
                let mut walls = 0;
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                        if tiles[ny as usize * width + nx as usize] == WALL {
                            walls += 1;
                        }
                    }
                }
                let index = y * width + x;
                next[index] = if tiles[index] == WALL {
                    if walls < death { FLOOR } else { WALL }
                } else if walls > birth {
                    WALL
                } else {
                    FLOOR
                };
            }
        }
        tiles = next;
    }
    if keep_largest {
        let mut found = regions(width, height, &tiles);
        found.sort_by_key(|region| std::cmp::Reverse(region.len()));
        for region in found.iter().skip(1) {
            for &cell in region {
                tiles[cell] = WALL;
            }
        }
    }
    tiles
}

struct DungeonSettings {
    attempts: usize,
    minimum: usize,
    maximum: usize,
    loops: f64,
    doors: bool,
}

fn carve_corridor(
    tiles: &mut [u8],
    width: usize,
    from: (usize, usize),
    to: (usize, usize),
    horizontal_first: bool,
) {
    let mut carve = |x: usize, y: usize| {
        let index = y * width + x;
        if tiles[index] == WALL {
            tiles[index] = CORRIDOR;
        }
    };
    let (mut x, mut y) = from;
    let step = |current: usize, target: usize| {
        if current < target {
            current + 1
        } else {
            current - 1
        }
    };
    if horizontal_first {
        while x != to.0 {
            carve(x, y);
            x = step(x, to.0);
        }
        while y != to.1 {
            carve(x, y);
            y = step(y, to.1);
        }
    } else {
        while y != to.1 {
            carve(x, y);
            y = step(y, to.1);
        }
        while x != to.0 {
            carve(x, y);
            x = step(x, to.0);
        }
    }
    carve(x, y);
}

fn dungeon(
    rng: &mut Rng,
    width: usize,
    height: usize,
    settings: &DungeonSettings,
) -> (Vec<u8>, Vec<Room>) {
    let mut tiles = vec![WALL; width * height];
    let mut rooms: Vec<Room> = Vec::new();
    let maximum = settings
        .maximum
        .min(width - 2)
        .min(height - 2)
        .max(settings.minimum);
    for _ in 0..settings.attempts {
        let room_width =
            settings.minimum + rng.below((maximum - settings.minimum + 1) as u64) as usize;
        let room_height =
            settings.minimum + rng.below((maximum - settings.minimum + 1) as u64) as usize;
        if room_width + 2 > width || room_height + 2 > height {
            continue;
        }
        let room = Room {
            x: 1 + rng.below((width - room_width - 1) as u64) as usize,
            y: 1 + rng.below((height - room_height - 1) as u64) as usize,
            width: room_width,
            height: room_height,
        };
        if rooms.iter().any(|other| room.overlaps(other, 1)) {
            continue;
        }
        for y in room.y..room.y + room.height {
            for x in room.x..room.x + room.width {
                tiles[y * width + x] = FLOOR;
            }
        }
        rooms.push(room);
    }

    if rooms.len() > 1 {
        let distance = |a: &Room, b: &Room| {
            let (ax, ay) = a.center();
            let (bx, by) = b.center();
            (ax as f64 - bx as f64).hypot(ay as f64 - by as f64)
        };
        let mut connected = vec![false; rooms.len()];
        connected[0] = true;
        let mut edges = Vec::new();
        for _ in 1..rooms.len() {
            let mut best: Option<(usize, usize, f64)> = None;
            for (from, room) in rooms
                .iter()
                .enumerate()
                .filter(|(index, _)| connected[*index])
            {
                for (to, other) in rooms
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !connected[*index])
                {
                    let length = distance(room, other);
                    if best.is_none_or(|(_, _, current)| length < current) {
                        best = Some((from, to, length));
                    }
                }
            }
            if let Some((from, to, _)) = best {
                connected[to] = true;
                edges.push((from, to));
            }
        }
        for from in 0..rooms.len() {
            for to in from + 1..rooms.len() {
                if !edges.contains(&(from, to))
                    && !edges.contains(&(to, from))
                    && rng.float() < settings.loops / rooms.len() as f64
                {
                    edges.push((from, to));
                }
            }
        }
        for (from, to) in edges {
            let horizontal = rng.float() < 0.5;
            carve_corridor(
                &mut tiles,
                width,
                rooms[from].center(),
                rooms[to].center(),
                horizontal,
            );
        }
    }

    if settings.doors {
        for room in &rooms {
            let mut mark = |x: usize, y: usize| {
                let index = y * width + x;
                if tiles[index] == CORRIDOR {
                    tiles[index] = DOOR;
                }
            };
            for x in room.x..room.x + room.width {
                mark(x, room.y - 1);
                mark(x, room.y + room.height);
            }
            for y in room.y..room.y + room.height {
                mark(room.x - 1, y);
                mark(room.x + room.width, y);
            }
        }
    }
    (tiles, rooms)
}

fn erode(heights: &mut [f32], width: usize, height: usize, iterations: usize, talus: f32) {
    for _ in 0..iterations {
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let index = y * width + x;
                let (mut lowest, mut drop) = (index, 0.0);
                for next in [index - 1, index + 1, index - width, index + width] {
                    let difference = heights[index] - heights[next];
                    if difference > drop {
                        drop = difference;
                        lowest = next;
                    }
                }
                if drop > talus {
                    let moved = (drop - talus) / 2.0;
                    heights[index] -= moved;
                    heights[lowest] += moved;
                }
            }
        }
    }
}

fn heightmap(
    sampler: &crate::noise::Sampler,
    width: usize,
    height: usize,
    settings: (f64, bool, usize, f32, f32, f32),
) -> Vec<u8> {
    let (step, island, erosion, talus, minimum, maximum) = settings;
    let mut heights: Vec<f32> = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let mut value = (sampler.sample(x as f64 * step, y as f64 * step, None) + 1.0) / 2.0;
            if island {
                let dx = x as f32 / (width - 1).max(1) as f32 * 2.0 - 1.0;
                let dy = y as f32 / (height - 1).max(1) as f32 * 2.0 - 1.0;
                let distance = (dx * dx + dy * dy).sqrt().min(1.0);
                value *= 1.0 - distance.powi(2);
            }
            heights.push(value);
        }
    }
    if erosion > 0 && width > 2 && height > 2 {
        erode(&mut heights, width, height, erosion, talus);
    }
    let low = heights.iter().copied().fold(f32::INFINITY, f32::min);
    let high = heights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let range = (high - low).max(f32::EPSILON);
    heights
        .iter()
        .flat_map(|value| (minimum + (value - low) / range * (maximum - minimum)).to_le_bytes())
        .collect()
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    let tiles = lua.create_table()?;
    tiles.set("Wall", WALL)?;
    tiles.set("Floor", FLOOR)?;
    tiles.set("Corridor", CORRIDOR)?;
    tiles.set("Door", DOOR)?;
    module.set("Tile", tiles)?;

    module.set(
        "Caves",
        lua.create_function(|lua, (width, height, options): (usize, usize, Options)| {
            check_size(width, height)?;
            let fill: f64 = options.get_or("FillChance", 0.45)?;
            if !(0.0..=1.0).contains(&fill) {
                return Err(LuaError::runtime("FillChance has to be between 0 and 1"));
            }
            let mut rng = seeded_rng(lua, &options)?;
            let settings = CaveSettings {
                fill,
                steps: options.get_or("Steps", 5)?,
                birth: options.get_or("BirthLimit", 4)?,
                death: options.get_or("DeathLimit", 4)?,
                keep_largest: options.get_or("KeepLargest", true)?,
            };
            let tiles = caves(&mut rng, width, height, settings);
            create_userdata(
                lua,
                Map {
                    width,
                    height,
                    tiles,
                    rooms: Vec::new(),
                    base: index_base(lua)?,
                },
            )
        })?,
    )?;
    module.set(
        "Dungeon",
        lua.create_function(|lua, (width, height, options): (usize, usize, Options)| {
            check_size(width, height)?;
            let settings = DungeonSettings {
                attempts: options.get_or("Attempts", 60)?,
                minimum: options.get_or("MinRoomSize", 4)?,
                maximum: options.get_or("MaxRoomSize", 10)?,
                loops: options.get_or("Loops", 1.0)?,
                doors: options.get_or("Doors", true)?,
            };
            if settings.minimum < 1 || settings.maximum < settings.minimum {
                return Err(LuaError::runtime(
                    "MinRoomSize has to be at least 1 and at most MaxRoomSize",
                ));
            }
            let mut rng = seeded_rng(lua, &options)?;
            let (tiles, rooms) = dungeon(&mut rng, width, height, &settings);
            create_userdata(
                lua,
                Map {
                    width,
                    height,
                    tiles,
                    rooms,
                    base: index_base(lua)?,
                },
            )
        })?,
    )?;
    module.set(
        "Heightmap",
        lua.create_function(
            |lua, (width, height, options, callback): (usize, usize, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                if width == 0 || height == 0 || width.saturating_mul(height) > MAX_CELLS {
                    return Err(LuaError::runtime(format!("the heightmap needs between 1 and {MAX_CELLS} cells")));
                }
                let sampler = std::sync::Arc::new(crate::noise::create(&options)?);
                let settings = (
                    options.get_or("Step", 1.0)?,
                    options.get_or("Island", false)?,
                    options.get_or("Erosion", 0)?,
                    options.get_or("Talus", 0.01)?,
                    options.get_or("Min", 0.0)?,
                    options.get_or("Max", 1.0)?,
                );
                match callback {
                    None => Object(Buffer::new(lua, heightmap(&sampler, width, height, settings))?).into_lua_multi(lua),
                    Some(callback) => {
                        let deliver = lua.create_function(move |lua, (data, error): (Option<LuaString>, Option<String>)| match data {
                            Some(data) => callback.call::<()>(Object(Buffer::new(lua, data.as_bytes().to_vec())?)),
                            None => callback.call::<()>((LuaNil, error)),
                        })?;
                        quark_core::spawn_blocking(lua, deliver, move || {
                            Ok::<_, String>(quark_core::Bytes(heightmap(&sampler, width, height, settings)))
                        })?
                        .into_lua_multi(lua)
                    }
                }
            },
        )?,
    )?;
    Ok(module)
}
