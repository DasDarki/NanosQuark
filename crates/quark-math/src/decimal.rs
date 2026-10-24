use std::str::FromStr;

use mlua::prelude::*;
use quark_core::{Object, Options, create_userdata};
use rust_decimal::prelude::{FromPrimitive, MathematicalOps, ToPrimitive};
use rust_decimal::{Decimal as Number, RoundingStrategy};

#[derive(Clone, Copy)]
pub struct Decimal(pub Number);

fn parse(text: &str) -> LuaResult<Number> {
    let cleaned = text.trim().replace('_', "");
    Number::from_str(&cleaned)
        .or_else(|_| Number::from_scientific(&cleaned))
        .map_err(|_| LuaError::runtime(format!("'{text}' is not a decimal number")))
}

pub fn read(value: &LuaValue) -> LuaResult<Number> {
    match value {
        LuaValue::Integer(integer) => Ok(Number::from(*integer)),
        LuaValue::Number(number) => {
            if !number.is_finite() {
                return Err(LuaError::runtime(
                    "infinite and NaN values cannot be decimals",
                ));
            }
            parse(&number.to_string()).or_else(|_| {
                Number::from_f64(*number).ok_or_else(|| {
                    LuaError::runtime(format!("{number} is out of the decimal range"))
                })
            })
        }
        LuaValue::String(text) => parse(&text.to_str()?),
        LuaValue::UserData(userdata) if userdata.is::<Decimal>() => {
            Ok(userdata.borrow::<Decimal>()?.0)
        }
        other => Err(LuaError::runtime(format!(
            "expected a number, a text or a Decimal, got {}",
            other.type_name()
        ))),
    }
}

fn wrap(value: Option<Number>, operation: &str) -> LuaResult<Object<Decimal>> {
    value.map(|value| Object(Decimal(value))).ok_or_else(|| {
        LuaError::runtime(format!(
            "the {operation} is out of the decimal range or divides by zero"
        ))
    })
}

fn strategy(name: Option<String>) -> LuaResult<RoundingStrategy> {
    Ok(match name.as_deref().unwrap_or("HalfEven") {
        "HalfEven" => RoundingStrategy::MidpointNearestEven,
        "HalfUp" => RoundingStrategy::MidpointAwayFromZero,
        "HalfDown" => RoundingStrategy::MidpointTowardZero,
        "Up" => RoundingStrategy::AwayFromZero,
        "Down" => RoundingStrategy::ToZero,
        "Ceil" => RoundingStrategy::ToPositiveInfinity,
        "Floor" => RoundingStrategy::ToNegativeInfinity,
        other => {
            return Err(LuaError::runtime(format!(
                "unknown rounding mode '{other}', expected HalfEven, HalfUp, HalfDown, Up, Down, Ceil or Floor"
            )));
        }
    })
}

fn format(value: Number, options: &Options) -> LuaResult<String> {
    let decimals: Option<u32> = options.get("Decimals")?;
    let value = match decimals {
        Some(decimals) => {
            value.round_dp_with_strategy(decimals, strategy(options.get("Rounding")?)?)
        }
        None => value,
    };
    let separator: String = options.get_or("Separator", ",".to_owned())?;
    let point: String = options.get_or("Point", ".".to_owned())?;
    let mut text = match decimals {
        Some(decimals) => format!("{value:.*}", decimals as usize),
        None => value.normalize().to_string(),
    };
    let negative = text.starts_with('-');
    if negative {
        text.remove(0);
    }
    let (whole, fraction) = match text.split_once('.') {
        Some((whole, fraction)) => (whole.to_owned(), Some(fraction.to_owned())),
        None => (text, None),
    };
    let mut grouped = String::new();
    for (index, digit) in whole.chars().enumerate() {
        if index > 0 && (whole.len() - index) % 3 == 0 {
            grouped.push_str(&separator);
        }
        grouped.push(digit);
    }
    let mut result = if negative {
        format!("-{grouped}")
    } else {
        grouped
    };
    if let Some(fraction) = fraction {
        result.push_str(&point);
        result.push_str(&fraction);
    }
    Ok(result)
}

impl LuaUserData for Decimal {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("ToString", |_, value, ()| {
            Ok(value.0.normalize().to_string())
        });
        methods.add_method("ToNumber", |_, value, ()| Ok(value.0.to_f64()));
        methods.add_method("Format", |_, value, options: Options| {
            format(value.0, &options)
        });
        methods.add_method(
            "Round",
            |_, value, (decimals, mode): (Option<u32>, Option<String>)| {
                Ok(Object(Decimal(value.0.round_dp_with_strategy(
                    decimals.unwrap_or(0),
                    strategy(mode)?,
                ))))
            },
        );
        methods.add_method("Truncate", |_, value, decimals: Option<u32>| {
            Ok(Object(Decimal(
                value.0.trunc_with_scale(decimals.unwrap_or(0)),
            )))
        });
        methods.add_method("Floor", |_, value, ()| Ok(Object(Decimal(value.0.floor()))));
        methods.add_method("Ceil", |_, value, ()| Ok(Object(Decimal(value.0.ceil()))));
        methods.add_method("Abs", |_, value, ()| Ok(Object(Decimal(value.0.abs()))));
        methods.add_method("Sqrt", |_, value, ()| wrap(value.0.sqrt(), "square root"));
        methods.add_method("Pow", |_, value, exponent: i64| {
            wrap(value.0.checked_powi(exponent), "power")
        });
        methods.add_method("GetScale", |_, value, ()| Ok(value.0.scale()));
        methods.add_method("IsZero", |_, value, ()| Ok(value.0.is_zero()));
        methods.add_method("IsNegative", |_, value, ()| {
            Ok(value.0.is_sign_negative() && !value.0.is_zero())
        });
        methods.add_method("Equals", |_, value, other: LuaValue| {
            Ok(read(&other).is_ok_and(|other| other == value.0))
        });
        methods.add_method("Compare", |_, value, other: LuaValue| {
            Ok(match value.0.cmp(&read(&other)?) {
                std::cmp::Ordering::Less => -1,
                std::cmp::Ordering::Equal => 0,
                std::cmp::Ordering::Greater => 1,
            })
        });
        methods.add_method("Split", |lua, value, parts: usize| {
            if !(1..=100_000).contains(&parts) {
                return Err(LuaError::runtime(
                    "the number of parts has to be between 1 and 100000",
                ));
            }
            let scale = value.0.scale();
            let unit = Number::new(1, scale);
            let count = Number::from(parts);
            let base = (value.0 / count).trunc_with_scale(scale);
            let mut remainder = value.0 - base * count;
            let list = lua.create_table_with_capacity(parts, 0)?;
            for _ in 0..parts {
                let mut share = base;
                if remainder >= unit {
                    share += unit;
                    remainder -= unit;
                } else if remainder <= -unit {
                    share -= unit;
                    remainder += unit;
                }
                list.raw_push(Object(Decimal(share)))?;
            }
            Ok(list)
        });

        methods.add_meta_function(
            LuaMetaMethod::Add,
            |_, (left, right): (LuaValue, LuaValue)| {
                wrap(read(&left)?.checked_add(read(&right)?), "sum")
            },
        );
        methods.add_meta_function(
            LuaMetaMethod::Sub,
            |_, (left, right): (LuaValue, LuaValue)| {
                wrap(read(&left)?.checked_sub(read(&right)?), "difference")
            },
        );
        methods.add_meta_function(
            LuaMetaMethod::Mul,
            |_, (left, right): (LuaValue, LuaValue)| {
                wrap(read(&left)?.checked_mul(read(&right)?), "product")
            },
        );
        methods.add_meta_function(
            LuaMetaMethod::Div,
            |_, (left, right): (LuaValue, LuaValue)| {
                wrap(read(&left)?.checked_div(read(&right)?), "quotient")
            },
        );
        methods.add_meta_function(
            LuaMetaMethod::Mod,
            |_, (left, right): (LuaValue, LuaValue)| {
                wrap(read(&left)?.checked_rem(read(&right)?), "remainder")
            },
        );
        methods.add_meta_method(LuaMetaMethod::Unm, |_, value, ()| {
            Ok(Object(Decimal(-value.0)))
        });
        methods.add_meta_function(
            LuaMetaMethod::Eq,
            |_, (left, right): (LuaValue, LuaValue)| Ok(read(&left)? == read(&right)?),
        );
        methods.add_meta_function(
            LuaMetaMethod::Lt,
            |_, (left, right): (LuaValue, LuaValue)| Ok(read(&left)? < read(&right)?),
        );
        methods.add_meta_function(
            LuaMetaMethod::Le,
            |_, (left, right): (LuaValue, LuaValue)| Ok(read(&left)? <= read(&right)?),
        );
        methods.add_meta_method(LuaMetaMethod::ToString, |_, value, ()| {
            Ok(value.0.normalize().to_string())
        });
        methods.add_meta_function(
            LuaMetaMethod::Concat,
            |_, (left, right): (LuaValue, LuaValue)| {
                let text = |value: &LuaValue| -> LuaResult<String> {
                    match value {
                        LuaValue::UserData(userdata) if userdata.is::<Decimal>() => {
                            Ok(userdata.borrow::<Decimal>()?.0.normalize().to_string())
                        }
                        other => other.to_string(),
                    }
                };
                Ok(text(&left)? + &text(&right)?)
            },
        );
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set(
        "New",
        lua.create_function(|lua, value: LuaValue| create_userdata(lua, Decimal(read(&value)?)))?,
    )?;
    module.set(
        "Parse",
        lua.create_function(|lua, text: String| match parse(&text) {
            Ok(value) => create_userdata(lua, Decimal(value))?.into_lua_multi(lua),
            Err(error) => (LuaNil, quark_core::describe(&error)).into_lua_multi(lua),
        })?,
    )?;
    module.set(
        "IsDecimal",
        lua.create_function(|_, value: LuaValue| {
            Ok(matches!(value, LuaValue::UserData(userdata) if userdata.is::<Decimal>()))
        })?,
    )?;
    module.set(
        "Sum",
        lua.create_function(|_, values: LuaTable| {
            let mut total = Number::ZERO;
            for value in values.sequence_values::<LuaValue>() {
                total = total
                    .checked_add(read(&value?)?)
                    .ok_or_else(|| LuaError::runtime("the sum is out of the decimal range"))?;
            }
            Ok(Object(Decimal(total)))
        })?,
    )?;
    Ok(module)
}
