use mlua::prelude::*;

fn read_numbers(values: LuaValue) -> LuaResult<Vec<f64>> {
    match values {
        LuaValue::Table(table) => table
            .sequence_values::<LuaValue>()
            .map(|value| match value? {
                LuaValue::Integer(integer) => Ok(integer as f64),
                LuaValue::Number(number) => Ok(number),
                other => Err(LuaError::runtime(format!(
                    "expected a list of numbers, found {}",
                    other.type_name()
                ))),
            })
            .collect(),
        LuaValue::UserData(userdata) => quark_core::view_numbers(&userdata)?
            .ok_or_else(|| LuaError::runtime("expected a list of numbers or a buffer view")),
        other => Err(LuaError::runtime(format!(
            "expected a list of numbers or a buffer view, got {}",
            other.type_name()
        ))),
    }
}

fn mean(numbers: &[f64]) -> Option<f64> {
    (!numbers.is_empty()).then(|| numbers.iter().sum::<f64>() / numbers.len() as f64)
}

fn variance(numbers: &[f64], sample: bool) -> Option<f64> {
    let average = mean(numbers)?;
    let divisor = if sample {
        numbers.len().checked_sub(1)?
    } else {
        numbers.len()
    };
    if divisor == 0 {
        return None;
    }
    Some(
        numbers
            .iter()
            .map(|number| (number - average).powi(2))
            .sum::<f64>()
            / divisor as f64,
    )
}

fn sorted(mut numbers: Vec<f64>) -> Vec<f64> {
    numbers.sort_by(|left, right| left.total_cmp(right));
    numbers
}

fn percentile(sorted: &[f64], percent: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let position = (percent / 100.0).clamp(0.0, 1.0) * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    Some(sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower as f64))
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "Sum",
        lua.create_function(|_, values: LuaValue| Ok(read_numbers(values)?.iter().sum::<f64>()))?,
    )?;
    module.set(
        "Mean",
        lua.create_function(|_, values: LuaValue| Ok(mean(&read_numbers(values)?)))?,
    )?;
    module.set(
        "Median",
        lua.create_function(|_, values: LuaValue| {
            Ok(percentile(&sorted(read_numbers(values)?), 50.0))
        })?,
    )?;
    module.set(
        "Percentile",
        lua.create_function(|_, (values, percent): (LuaValue, f64)| {
            if !(0.0..=100.0).contains(&percent) {
                return Err(LuaError::runtime(
                    "the percentile has to be between 0 and 100",
                ));
            }
            Ok(percentile(&sorted(read_numbers(values)?), percent))
        })?,
    )?;
    module.set(
        "Variance",
        lua.create_function(|_, (values, sample): (LuaValue, Option<bool>)| {
            Ok(variance(&read_numbers(values)?, sample.unwrap_or(false)))
        })?,
    )?;
    module.set(
        "StandardDeviation",
        lua.create_function(|_, (values, sample): (LuaValue, Option<bool>)| {
            Ok(variance(&read_numbers(values)?, sample.unwrap_or(false)).map(f64::sqrt))
        })?,
    )?;
    module.set(
        "Min",
        lua.create_function(|_, values: LuaValue| {
            Ok(read_numbers(values)?.into_iter().reduce(f64::min))
        })?,
    )?;
    module.set(
        "Max",
        lua.create_function(|_, values: LuaValue| {
            Ok(read_numbers(values)?.into_iter().reduce(f64::max))
        })?,
    )?;
    module.set(
        "Summarize",
        lua.create_function(|lua, values: LuaValue| {
            let numbers = read_numbers(values)?;
            let table = lua.create_table()?;
            table.set("Count", numbers.len())?;
            if numbers.is_empty() {
                return Ok(table);
            }
            let ordered = sorted(numbers.clone());
            table.set("Sum", numbers.iter().sum::<f64>())?;
            table.set("Mean", mean(&numbers))?;
            table.set("Min", ordered.first().copied())?;
            table.set("Max", ordered.last().copied())?;
            table.set("Median", percentile(&ordered, 50.0))?;
            table.set("P90", percentile(&ordered, 90.0))?;
            table.set("P99", percentile(&ordered, 99.0))?;
            table.set(
                "StandardDeviation",
                variance(&numbers, false).map(f64::sqrt),
            )?;
            Ok(table)
        })?,
    )?;
    module.set(
        "Histogram",
        lua.create_function(
            |lua, (values, bins, minimum, maximum): (LuaValue, usize, Option<f64>, Option<f64>)| {
                if !(1..=100_000).contains(&bins) {
                    return Err(LuaError::runtime(
                        "the number of bins has to be between 1 and 100000",
                    ));
                }
                let numbers = read_numbers(values)?;
                let low = minimum
                    .or_else(|| numbers.iter().copied().reduce(f64::min))
                    .unwrap_or(0.0);
                let high = maximum
                    .or_else(|| numbers.iter().copied().reduce(f64::max))
                    .unwrap_or(0.0);
                let mut counts = vec![0usize; bins];
                let width = (high - low) / bins as f64;
                for number in numbers {
                    if number < low || number > high {
                        continue;
                    }
                    let index = if width <= 0.0 {
                        0
                    } else {
                        (((number - low) / width) as usize).min(bins - 1)
                    };
                    counts[index] += 1;
                }
                lua.create_sequence_from(counts)
            },
        )?,
    )?;
    Ok(())
}
