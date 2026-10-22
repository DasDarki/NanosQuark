use mlua::prelude::*;

use crate::dice;
use crate::rng::Rng;

pub type Operation = fn(&mut Rng, &Lua, LuaMultiValue) -> LuaResult<LuaMultiValue>;

pub const OPERATIONS: &[(&str, Operation)] = &[
    ("Int", int),
    ("Float", float),
    ("Bool", boolean),
    ("Pick", pick),
    ("Shuffle", shuffle),
    ("Sample", sample),
    ("Weighted", weighted),
    ("Normal", normal),
    ("Exponential", exponential),
    ("Poisson", poisson),
    ("Triangular", triangular),
    ("InsideCircle", inside_circle),
    ("InsideSphere", inside_sphere),
    ("OnSphere", on_sphere),
    ("Roll", roll),
];

fn arguments<T: FromLuaMulti>(lua: &Lua, values: LuaMultiValue) -> LuaResult<T> {
    T::from_lua_multi(values, lua)
}

fn int(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let (first, second): (i64, Option<i64>) = arguments(lua, values)?;
    let (minimum, maximum) = match second {
        Some(maximum) => (first, maximum),
        None => (1, first),
    };
    if minimum > maximum {
        return Err(LuaError::runtime(
            "the minimum must not be greater than the maximum",
        ));
    }
    rng.int(minimum, maximum).into_lua_multi(lua)
}

fn float(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let (first, second): (Option<f64>, Option<f64>) = arguments(lua, values)?;
    let value = match (first, second) {
        (None, _) => rng.float(),
        (Some(maximum), None) => rng.range(0.0, maximum),
        (Some(minimum), Some(maximum)) => rng.range(minimum, maximum),
    };
    value.into_lua_multi(lua)
}

fn boolean(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let probability: Option<f64> = arguments(lua, values)?;
    let probability = probability.unwrap_or(0.5);
    if !(0.0..=1.0).contains(&probability) {
        return Err(LuaError::runtime(
            "the probability has to be between 0 and 1",
        ));
    }
    (rng.float() < probability).into_lua_multi(lua)
}

fn pick(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let list: LuaTable = arguments(lua, values)?;
    let length = list.raw_len();
    if length == 0 {
        return LuaNil.into_lua_multi(lua);
    }
    let index = rng.below(length as u64) as usize + 1;
    (list.raw_get::<LuaValue>(index)?, index).into_lua_multi(lua)
}

fn shuffle(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let list: LuaTable = arguments(lua, values)?;
    let length = list.raw_len();
    for index in (2..=length).rev() {
        let other = rng.below(index as u64) as usize + 1;
        if other != index {
            let first: LuaValue = list.raw_get(index)?;
            let second: LuaValue = list.raw_get(other)?;
            list.raw_set(index, second)?;
            list.raw_set(other, first)?;
        }
    }
    list.into_lua_multi(lua)
}

fn sample(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let (list, count): (LuaTable, usize) = arguments(lua, values)?;
    let mut items = list
        .sequence_values::<LuaValue>()
        .collect::<LuaResult<Vec<_>>>()?;
    let count = count.min(items.len());
    for index in 0..count {
        let other = index + rng.below((items.len() - index) as u64) as usize;
        items.swap(index, other);
    }
    items.truncate(count);
    lua.create_sequence_from(items)?.into_lua_multi(lua)
}

fn weighted(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let weights: LuaTable = arguments(lua, values)?;
    let (items, weights) = crate::picker::read_weights(lua, &weights)?;
    let total: f64 = weights.iter().sum();
    let mut target = rng.float() * total;
    for (item, weight) in items.into_iter().zip(weights) {
        if target < weight {
            return item.into_lua_multi(lua);
        }
        target -= weight;
    }
    LuaNil.into_lua_multi(lua)
}

fn normal(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let (mean, deviation): (Option<f64>, Option<f64>) = arguments(lua, values)?;
    let deviation = deviation.unwrap_or(1.0);
    if deviation < 0.0 {
        return Err(LuaError::runtime(
            "the standard deviation must not be negative",
        ));
    }
    rng.normal(mean.unwrap_or(0.0), deviation)
        .into_lua_multi(lua)
}

fn exponential(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let rate: Option<f64> = arguments(lua, values)?;
    let rate = rate.unwrap_or(1.0);
    if rate <= 0.0 {
        return Err(LuaError::runtime("the rate has to be greater than 0"));
    }
    rng.exponential(rate).into_lua_multi(lua)
}

fn poisson(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let mean: f64 = arguments(lua, values)?;
    if !(0.0..=1e9).contains(&mean) {
        return Err(LuaError::runtime(
            "the mean has to be between 0 and 1000000000",
        ));
    }
    rng.poisson(mean).into_lua_multi(lua)
}

fn triangular(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let (minimum, maximum, mode): (f64, f64, Option<f64>) = arguments(lua, values)?;
    if minimum > maximum {
        return Err(LuaError::runtime(
            "the minimum must not be greater than the maximum",
        ));
    }
    let mode = mode.unwrap_or((minimum + maximum) / 2.0);
    if !(minimum..=maximum).contains(&mode) {
        return Err(LuaError::runtime(
            "the mode has to be between the minimum and the maximum",
        ));
    }
    rng.triangular(minimum, maximum, mode).into_lua_multi(lua)
}

fn inside_circle(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let radius: Option<f64> = arguments(lua, values)?;
    rng.inside_circle(radius.unwrap_or(1.0)).into_lua_multi(lua)
}

fn inside_sphere(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let radius: Option<f64> = arguments(lua, values)?;
    rng.inside_sphere(radius.unwrap_or(1.0)).into_lua_multi(lua)
}

fn on_sphere(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let radius: Option<f64> = arguments(lua, values)?;
    rng.on_sphere(radius.unwrap_or(1.0)).into_lua_multi(lua)
}

fn roll(rng: &mut Rng, lua: &Lua, values: LuaMultiValue) -> LuaResult<LuaMultiValue> {
    let notation: String = arguments(lua, values)?;
    let (total, rolls) = dice::roll(rng, &notation).map_err(LuaError::runtime)?;
    (total, lua.create_sequence_from(rolls)?).into_lua_multi(lua)
}
