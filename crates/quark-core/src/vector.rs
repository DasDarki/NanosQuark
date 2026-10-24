use crate::package_environment;
use mlua::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Rotator {
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
}

impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    pub fn plus(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }

    pub fn subtract(self, other: Vec3) -> Vec3 {
        Vec3::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }

    pub fn scale(self, factor: f64) -> Vec3 {
        Vec3::new(self.x * factor, self.y * factor, self.z * factor)
    }

    pub fn dot(self, other: Vec3) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Vec3) -> Vec3 {
        Vec3::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }

    pub fn normalized(self) -> Vec3 {
        let length = self.length();
        if length < 1e-12 {
            Vec3::default()
        } else {
            self.scale(1.0 / length)
        }
    }

    pub fn lerp(self, other: Vec3, t: f64) -> Vec3 {
        self.plus(other.subtract(self).scale(t))
    }
}

fn field(value: &LuaValue, names: &[&str], index: usize) -> LuaResult<Option<f64>> {
    let get = |key: &str| -> LuaResult<LuaValue> {
        match value {
            LuaValue::Table(table) => table.get(key),
            LuaValue::UserData(userdata) => userdata.get(key),
            _ => Ok(LuaNil),
        }
    };
    for name in names {
        match get(name)? {
            LuaNil => continue,
            found => return number(found).map(Some),
        }
    }
    let positional = match value {
        LuaValue::Table(table) => table.raw_get::<LuaValue>(index)?,
        _ => LuaNil,
    };
    match positional {
        LuaNil => Ok(None),
        other => number(other).map(Some),
    }
}

fn number(value: LuaValue) -> LuaResult<f64> {
    match value {
        LuaValue::Integer(integer) => Ok(integer as f64),
        LuaValue::Number(number) => Ok(number),
        other => Err(LuaError::runtime(format!(
            "expected a number, got {}",
            other.type_name()
        ))),
    }
}

fn is_object(value: &LuaValue) -> bool {
    matches!(value, LuaValue::Table(_) | LuaValue::UserData(_))
}

pub fn read_vector(value: &LuaValue) -> LuaResult<Vec3> {
    if !is_object(value) {
        return Err(LuaError::runtime(format!(
            "expected a Vector or a table with X, Y and Z, got {}",
            value.type_name()
        )));
    }
    let x = field(value, &["X", "x"], 1)?;
    let y = field(value, &["Y", "y"], 2)?;
    let z = field(value, &["Z", "z"], 3)?;
    match (x, y) {
        (Some(x), Some(y)) => Ok(Vec3::new(x, y, z.unwrap_or(0.0))),
        _ => Err(LuaError::runtime(
            "expected a Vector or a table with X, Y and Z",
        )),
    }
}

pub fn is_vector(value: &LuaValue) -> bool {
    is_object(value) && read_vector(value).is_ok()
}

pub fn read_rotator(value: &LuaValue) -> LuaResult<Rotator> {
    if !is_object(value) {
        return Err(LuaError::runtime(format!(
            "expected a Rotator or a table with Pitch, Yaw and Roll, got {}",
            value.type_name()
        )));
    }
    let pitch = field(value, &["Pitch", "pitch"], 1)?;
    let yaw = field(value, &["Yaw", "yaw"], 2)?;
    let roll = field(value, &["Roll", "roll"], 3)?;
    match (pitch, yaw) {
        (Some(pitch), Some(yaw)) => Ok(Rotator {
            pitch,
            yaw,
            roll: roll.unwrap_or(0.0),
        }),
        _ => Err(LuaError::runtime(
            "expected a Rotator or a table with Pitch, Yaw and Roll",
        )),
    }
}

fn constructor(lua: &Lua, name: &str) -> LuaResult<Option<LuaFunction>> {
    if let Some(environment) = package_environment(lua)
        && let LuaValue::Function(function) = environment.get::<LuaValue>(name)?
    {
        return Ok(Some(function));
    }
    match lua.globals().get::<LuaValue>(name)? {
        LuaValue::Function(function) => Ok(Some(function)),
        LuaValue::Table(table) => {
            match table.metatable().map(|meta| meta.get::<LuaValue>("__call")) {
                Some(Ok(LuaValue::Function(_))) => Ok(Some(lua.create_function(
                    move |_, arguments: LuaMultiValue| table.call::<LuaValue>(arguments),
                )?)),
                _ => Ok(None),
            }
        }
        _ => Ok(None),
    }
}

pub fn make_vector(lua: &Lua, vector: Vec3) -> LuaResult<LuaValue> {
    if let Some(function) = constructor(lua, "Vector")? {
        return function.call((vector.x, vector.y, vector.z));
    }
    let table = lua.create_table_with_capacity(0, 3)?;
    table.set("X", vector.x)?;
    table.set("Y", vector.y)?;
    table.set("Z", vector.z)?;
    Ok(LuaValue::Table(table))
}

pub fn make_vector2(lua: &Lua, x: f64, y: f64) -> LuaResult<LuaValue> {
    if let Some(function) = constructor(lua, "Vector2D")? {
        return function.call((x, y));
    }
    let table = lua.create_table_with_capacity(0, 2)?;
    table.set("X", x)?;
    table.set("Y", y)?;
    Ok(LuaValue::Table(table))
}

pub fn make_rotator(lua: &Lua, rotator: Rotator) -> LuaResult<LuaValue> {
    if let Some(function) = constructor(lua, "Rotator")? {
        return function.call((rotator.pitch, rotator.yaw, rotator.roll));
    }
    let table = lua.create_table_with_capacity(0, 3)?;
    table.set("Pitch", rotator.pitch)?;
    table.set("Yaw", rotator.yaw)?;
    table.set("Roll", rotator.roll)?;
    Ok(LuaValue::Table(table))
}
