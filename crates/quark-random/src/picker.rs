use std::cmp::Ordering;

use mlua::prelude::*;
use quark_core::create_userdata;

use crate::generator::with_rng;
use crate::rng::Rng;

const MAX_PICKS: usize = 1_000_000;

pub struct WeightedPicker {
    probabilities: Vec<f64>,
    aliases: Vec<usize>,
    weights: Vec<f64>,
    total: f64,
}

fn key_order(left: &LuaValue, right: &LuaValue) -> Ordering {
    fn rank(value: &LuaValue) -> u8 {
        match value {
            LuaValue::Boolean(_) => 0,
            LuaValue::Integer(_) | LuaValue::Number(_) => 1,
            LuaValue::String(_) => 2,
            _ => 3,
        }
    }

    match (left, right) {
        (LuaValue::String(left), LuaValue::String(right)) => left.as_bytes().cmp(&right.as_bytes()),
        (LuaValue::Boolean(left), LuaValue::Boolean(right)) => left.cmp(right),
        (left, right) if rank(left) == 1 && rank(right) == 1 => {
            let left = left.as_f64().unwrap_or_default();
            let right = right.as_f64().unwrap_or_default();
            left.total_cmp(&right)
        }
        (left, right) => rank(left).cmp(&rank(right)),
    }
}

fn check_weight(weight: f64) -> LuaResult<f64> {
    if weight.is_finite() && weight >= 0.0 {
        Ok(weight)
    } else {
        Err(LuaError::runtime(
            "weights have to be finite numbers of at least 0",
        ))
    }
}

pub fn read_weights(_: &Lua, table: &LuaTable) -> LuaResult<(Vec<LuaValue>, Vec<f64>)> {
    let mut items = Vec::new();
    let mut weights = Vec::new();

    if let LuaValue::Table(_) = table.raw_get::<LuaValue>(1)? {
        for entry in table.sequence_values::<LuaTable>() {
            let entry = entry.map_err(|_| {
                LuaError::runtime("weighted entries have to be { Item, Weight } tables")
            })?;
            let item: LuaValue = match entry.raw_get::<LuaValue>("Item")? {
                LuaNil => entry.raw_get(1)?,
                item => item,
            };
            if item.is_nil() {
                return Err(LuaError::runtime("a weighted entry is missing its Item"));
            }
            let weight: Option<f64> = match entry.raw_get::<Option<f64>>("Weight")? {
                None => entry.raw_get(2)?,
                weight => weight,
            };
            let weight = weight
                .ok_or_else(|| LuaError::runtime("a weighted entry is missing its Weight"))?;
            items.push(item);
            weights.push(check_weight(weight)?);
        }
    } else {
        let mut entries = table
            .pairs::<LuaValue, f64>()
            .collect::<LuaResult<Vec<_>>>()
            .map_err(|_| {
                LuaError::runtime(
                    "expected a table of item = weight pairs or a list of { Item, Weight } entries",
                )
            })?;
        entries.sort_by(|(left, _), (right, _)| key_order(left, right));
        for (item, weight) in entries {
            items.push(item);
            weights.push(check_weight(weight)?);
        }
    }

    if items.is_empty() {
        return Err(LuaError::runtime("at least one weighted item is required"));
    }
    if weights.iter().sum::<f64>() <= 0.0 {
        return Err(LuaError::runtime(
            "at least one weight has to be greater than 0",
        ));
    }
    Ok((items, weights))
}

impl WeightedPicker {
    fn new(weights: Vec<f64>) -> Self {
        let count = weights.len();
        let total: f64 = weights.iter().sum();
        let mut scaled: Vec<f64> = weights
            .iter()
            .map(|weight| weight * count as f64 / total)
            .collect();
        let mut probabilities = vec![1.0; count];
        let mut aliases: Vec<usize> = (0..count).collect();
        let (mut small, mut large): (Vec<usize>, Vec<usize>) =
            (0..count).partition(|&index| scaled[index] < 1.0);

        while let (Some(&less), Some(&more)) = (small.last(), large.last()) {
            small.pop();
            probabilities[less] = scaled[less];
            aliases[less] = more;
            scaled[more] += scaled[less] - 1.0;
            if scaled[more] < 1.0 {
                large.pop();
                small.push(more);
            }
        }
        let fallback = weights
            .iter()
            .position(|&weight| weight > 0.0)
            .unwrap_or_default();
        for index in small.into_iter().chain(large) {
            if weights[index] > 0.0 {
                probabilities[index] = 1.0;
            } else {
                probabilities[index] = 0.0;
                aliases[index] = fallback;
            }
        }

        WeightedPicker {
            probabilities,
            aliases,
            weights,
            total,
        }
    }

    pub fn pick_index(&self, rng: &mut Rng) -> usize {
        let column = rng.below(self.probabilities.len() as u64) as usize;
        if rng.float() < self.probabilities[column] {
            column
        } else {
            self.aliases[column]
        }
    }
}

fn items(this: &LuaAnyUserData) -> LuaResult<LuaTable> {
    this.user_value()
}

impl LuaUserData for WeightedPicker {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_function(
            "Pick",
            |lua, (this, generator): (LuaAnyUserData, Option<LuaAnyUserData>)| {
                let index = {
                    let picker = this.borrow::<WeightedPicker>()?;
                    with_rng(lua, generator, |rng| picker.pick_index(rng))?
                };
                items(&this)?.raw_get::<LuaValue>(index + 1)
            },
        );
        methods.add_function(
            "PickMany",
            |lua, (this, count, generator): (LuaAnyUserData, usize, Option<LuaAnyUserData>)| {
                if count > MAX_PICKS {
                    return Err(LuaError::runtime(format!(
                        "at most {MAX_PICKS} items can be picked at once"
                    )));
                }
                let indices = {
                    let picker = this.borrow::<WeightedPicker>()?;
                    with_rng(lua, generator, |rng| {
                        (0..count)
                            .map(|_| picker.pick_index(rng))
                            .collect::<Vec<_>>()
                    })?
                };
                let items = items(&this)?;
                let result = lua.create_table_with_capacity(count, 0)?;
                for index in indices {
                    result.raw_push(items.raw_get::<LuaValue>(index + 1)?)?;
                }
                Ok(result)
            },
        );
        methods.add_function(
            "GetProbability",
            |_, (this, item): (LuaAnyUserData, LuaValue)| {
                let picker = this.borrow::<WeightedPicker>()?;
                let items = items(&this)?;
                let mut weight = 0.0;
                for (index, entry) in items.sequence_values::<LuaValue>().enumerate() {
                    if entry? == item {
                        weight += picker.weights[index];
                    }
                }
                Ok(weight / picker.total)
            },
        );
        methods.add_function("GetItems", |lua, this: LuaAnyUserData| {
            let picker = this.borrow::<WeightedPicker>()?;
            let result = lua.create_table()?;
            for (index, item) in items(&this)?.sequence_values::<LuaValue>().enumerate() {
                let entry = lua.create_table()?;
                entry.set("Item", item?)?;
                entry.set("Weight", picker.weights[index])?;
                result.raw_push(entry)?;
            }
            Ok(result)
        });
        methods.add_method("GetCount", |_, picker, ()| Ok(picker.weights.len()));
        methods.add_method("GetTotalWeight", |_, picker, ()| Ok(picker.total));
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "WeightedPicker",
        lua.create_function(|lua, weights: LuaTable| {
            let (items, weights) = read_weights(lua, &weights)?;
            let picker = create_userdata(lua, WeightedPicker::new(weights))?;
            picker.set_user_value(lua.create_sequence_from(items)?)?;
            Ok(picker)
        })?,
    )
}
