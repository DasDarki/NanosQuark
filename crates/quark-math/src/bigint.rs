use mlua::prelude::*;
use num_bigint::{BigInt as Integer, Sign};
use num_integer::Integer as _;
use num_traits::{One, Signed, ToPrimitive, Zero};
use quark_core::{Object, create_userdata};

const MAX_BITS: u64 = 1 << 20;

#[derive(Clone)]
pub struct BigInt(pub Integer);

fn parse(text: &str) -> LuaResult<Integer> {
    let trimmed = text.trim().replace('_', "");
    let (negative, digits) = match trimmed.strip_prefix('-') {
        Some(rest) => (true, rest.to_owned()),
        None => (
            false,
            trimmed.strip_prefix('+').unwrap_or(&trimmed).to_owned(),
        ),
    };
    let (radix, digits) = match digits.get(..2).map(str::to_ascii_lowercase).as_deref() {
        Some("0x") => (16, &digits[2..]),
        Some("0b") => (2, &digits[2..]),
        Some("0o") => (8, &digits[2..]),
        _ => (10, digits.as_str()),
    };
    let value = Integer::parse_bytes(digits.as_bytes(), radix)
        .ok_or_else(|| LuaError::runtime(format!("'{text}' is not an integer")))?;
    Ok(if negative { -value } else { value })
}

pub fn read(value: &LuaValue) -> LuaResult<Integer> {
    match value {
        LuaValue::Integer(integer) => Ok(Integer::from(*integer)),
        LuaValue::Number(number) if number.fract() == 0.0 && number.is_finite() => {
            parse(&format!("{number:.0}"))
        }
        LuaValue::Number(number) => {
            Err(LuaError::runtime(format!("{number} is not a whole number")))
        }
        LuaValue::String(text) => parse(&text.to_str()?),
        LuaValue::UserData(userdata) if userdata.is::<BigInt>() => {
            Ok(userdata.borrow::<BigInt>()?.0.clone())
        }
        other => Err(LuaError::runtime(format!(
            "expected an integer, a text or a BigInt, got {}",
            other.type_name()
        ))),
    }
}

fn check(value: Integer) -> LuaResult<Object<BigInt>> {
    if value.bits() > MAX_BITS {
        return Err(LuaError::runtime("the result is larger than 1048576 bits"));
    }
    Ok(Object(BigInt(value)))
}

fn divisor(value: &LuaValue) -> LuaResult<Integer> {
    let value = read(value)?;
    if value.is_zero() {
        return Err(LuaError::runtime("division by zero"));
    }
    Ok(value)
}

fn exponent(value: &LuaValue) -> LuaResult<u32> {
    let exponent = read(value)?;
    exponent
        .to_u32()
        .filter(|exponent| *exponent <= 1_000_000)
        .ok_or_else(|| LuaError::runtime("the exponent has to be between 0 and 1000000"))
}

impl LuaUserData for BigInt {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("ToString", |_, value, radix: Option<u32>| {
            let radix = radix.unwrap_or(10);
            if !(2..=36).contains(&radix) {
                return Err(LuaError::runtime("the base has to be between 2 and 36"));
            }
            Ok(value.0.to_str_radix(radix))
        });
        methods.add_method("ToNumber", |_, value, ()| {
            Ok(match value.0.to_i64() {
                Some(integer) => LuaValue::Integer(integer),
                None => LuaValue::Number(value.0.to_f64().unwrap_or(f64::NAN)),
            })
        });
        methods.add_method("FitsInteger", |_, value, ()| Ok(value.0.to_i64().is_some()));
        methods.add_method("Abs", |_, value, ()| check(value.0.abs()));
        methods.add_method("Pow", |_, value, power: LuaValue| {
            check(value.0.pow(exponent(&power)?))
        });
        methods.add_method(
            "ModPow",
            |_, value, (power, modulus): (LuaValue, LuaValue)| {
                let power = read(&power)?;
                if power.is_negative() {
                    return Err(LuaError::runtime("the exponent must not be negative"));
                }
                check(value.0.modpow(&power, &divisor(&modulus)?))
            },
        );
        methods.add_method("Gcd", |_, value, other: LuaValue| {
            check(value.0.gcd(&read(&other)?))
        });
        methods.add_method("Lcm", |_, value, other: LuaValue| {
            check(value.0.lcm(&read(&other)?))
        });
        methods.add_method("Sqrt", |_, value, ()| {
            if value.0.is_negative() {
                return Err(LuaError::runtime(
                    "the square root of a negative number is not an integer",
                ));
            }
            check(value.0.sqrt())
        });
        methods.add_method("DivMod", |lua, value, other: LuaValue| {
            let (quotient, remainder) = value.0.div_mod_floor(&divisor(&other)?);
            (check(quotient)?, check(remainder)?).into_lua_multi(lua)
        });
        methods.add_method("GetSign", |_, value, ()| {
            Ok(match value.0.sign() {
                Sign::Minus => -1,
                Sign::NoSign => 0,
                Sign::Plus => 1,
            })
        });
        methods.add_method("IsZero", |_, value, ()| Ok(value.0.is_zero()));
        methods.add_method("IsEven", |_, value, ()| Ok(value.0.is_even()));
        methods.add_method("GetBits", |_, value, ()| Ok(value.0.bits()));
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

        methods.add_meta_function(
            LuaMetaMethod::Add,
            |_, (left, right): (LuaValue, LuaValue)| check(read(&left)? + read(&right)?),
        );
        methods.add_meta_function(
            LuaMetaMethod::Sub,
            |_, (left, right): (LuaValue, LuaValue)| check(read(&left)? - read(&right)?),
        );
        methods.add_meta_function(
            LuaMetaMethod::Mul,
            |_, (left, right): (LuaValue, LuaValue)| check(read(&left)? * read(&right)?),
        );
        methods.add_meta_function(
            LuaMetaMethod::Div,
            |_, (left, right): (LuaValue, LuaValue)| {
                check(read(&left)?.div_floor(&divisor(&right)?))
            },
        );
        methods.add_meta_function(
            LuaMetaMethod::IDiv,
            |_, (left, right): (LuaValue, LuaValue)| {
                check(read(&left)?.div_floor(&divisor(&right)?))
            },
        );
        methods.add_meta_function(
            LuaMetaMethod::Mod,
            |_, (left, right): (LuaValue, LuaValue)| {
                check(read(&left)?.mod_floor(&divisor(&right)?))
            },
        );
        methods.add_meta_function(
            LuaMetaMethod::Pow,
            |_, (left, right): (LuaValue, LuaValue)| check(read(&left)?.pow(exponent(&right)?)),
        );
        methods.add_meta_method(LuaMetaMethod::Unm, |_, value, ()| check(-value.0.clone()));
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
            Ok(value.0.to_string())
        });
        methods.add_meta_function(
            LuaMetaMethod::Concat,
            |_, (left, right): (LuaValue, LuaValue)| {
                let text = |value: &LuaValue| -> LuaResult<String> {
                    match value {
                        LuaValue::UserData(userdata) if userdata.is::<BigInt>() => {
                            Ok(userdata.borrow::<BigInt>()?.0.to_string())
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
        lua.create_function(|lua, value: LuaValue| create_userdata(lua, BigInt(read(&value)?)))?,
    )?;
    module.set(
        "Parse",
        lua.create_function(|lua, text: String| match parse(&text) {
            Ok(value) => create_userdata(lua, BigInt(value))?.into_lua_multi(lua),
            Err(error) => (LuaNil, quark_core::describe(&error)).into_lua_multi(lua),
        })?,
    )?;
    module.set(
        "IsBigInt",
        lua.create_function(|_, value: LuaValue| {
            Ok(matches!(value, LuaValue::UserData(userdata) if userdata.is::<BigInt>()))
        })?,
    )?;
    module.set(
        "Factorial",
        lua.create_function(|_, value: u32| {
            if value > 20_000 {
                return Err(LuaError::runtime("factorials are limited to 20000"));
            }
            let mut result = Integer::one();
            for factor in 2..=value {
                result *= factor;
            }
            check(result)
        })?,
    )?;
    Ok(module)
}
