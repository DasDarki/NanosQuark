use std::f64::consts::PI;

use mlua::prelude::*;

use crate::vector::{Vec3, is_vector, make_vector, read_vector};

const EASINGS: &[&str] = &[
    "Linear",
    "InSine",
    "OutSine",
    "InOutSine",
    "InQuad",
    "OutQuad",
    "InOutQuad",
    "InCubic",
    "OutCubic",
    "InOutCubic",
    "InQuart",
    "OutQuart",
    "InOutQuart",
    "InQuint",
    "OutQuint",
    "InOutQuint",
    "InExpo",
    "OutExpo",
    "InOutExpo",
    "InCirc",
    "OutCirc",
    "InOutCirc",
    "InBack",
    "OutBack",
    "InOutBack",
    "InElastic",
    "OutElastic",
    "InOutElastic",
    "InBounce",
    "OutBounce",
    "InOutBounce",
];

fn out_bounce(t: f64) -> f64 {
    const N: f64 = 7.5625;
    const D: f64 = 2.75;
    if t < 1.0 / D {
        N * t * t
    } else if t < 2.0 / D {
        let t = t - 1.5 / D;
        N * t * t + 0.75
    } else if t < 2.5 / D {
        let t = t - 2.25 / D;
        N * t * t + 0.9375
    } else {
        let t = t - 2.625 / D;
        N * t * t + 0.984375
    }
}

pub fn ease(name: &str, t: f64) -> Option<f64> {
    let t = t.clamp(0.0, 1.0);
    const C1: f64 = 1.70158;
    const C2: f64 = C1 * 1.525;
    const C3: f64 = C1 + 1.0;
    const C4: f64 = 2.0 * PI / 3.0;
    const C5: f64 = 2.0 * PI / 4.5;
    let in_out = |inner: &dyn Fn(f64) -> f64| {
        if t < 0.5 {
            inner(2.0 * t) / 2.0
        } else {
            1.0 - inner(2.0 - 2.0 * t) / 2.0
        }
    };
    Some(match name {
        "Linear" => t,
        "InSine" => 1.0 - (t * PI / 2.0).cos(),
        "OutSine" => (t * PI / 2.0).sin(),
        "InOutSine" => -((PI * t).cos() - 1.0) / 2.0,
        "InQuad" => t.powi(2),
        "OutQuad" => 1.0 - (1.0 - t).powi(2),
        "InOutQuad" => in_out(&|x| x.powi(2)),
        "InCubic" => t.powi(3),
        "OutCubic" => 1.0 - (1.0 - t).powi(3),
        "InOutCubic" => in_out(&|x| x.powi(3)),
        "InQuart" => t.powi(4),
        "OutQuart" => 1.0 - (1.0 - t).powi(4),
        "InOutQuart" => in_out(&|x| x.powi(4)),
        "InQuint" => t.powi(5),
        "OutQuint" => 1.0 - (1.0 - t).powi(5),
        "InOutQuint" => in_out(&|x| x.powi(5)),
        "InExpo" => {
            if t == 0.0 {
                0.0
            } else {
                2f64.powf(10.0 * t - 10.0)
            }
        }
        "OutExpo" => {
            if t == 1.0 {
                1.0
            } else {
                1.0 - 2f64.powf(-10.0 * t)
            }
        }
        "InOutExpo" => in_out(&|x| {
            if x == 0.0 {
                0.0
            } else {
                2f64.powf(10.0 * x - 10.0)
            }
        }),
        "InCirc" => 1.0 - (1.0 - t * t).sqrt(),
        "OutCirc" => (1.0 - (t - 1.0).powi(2)).sqrt(),
        "InOutCirc" => in_out(&|x| 1.0 - (1.0 - x * x).sqrt()),
        "InBack" => C3 * t.powi(3) - C1 * t * t,
        "OutBack" => 1.0 + C3 * (t - 1.0).powi(3) + C1 * (t - 1.0).powi(2),
        "InOutBack" => {
            if t < 0.5 {
                ((2.0 * t).powi(2) * ((C2 + 1.0) * 2.0 * t - C2)) / 2.0
            } else {
                ((2.0 * t - 2.0).powi(2) * ((C2 + 1.0) * (t * 2.0 - 2.0) + C2) + 2.0) / 2.0
            }
        }
        "InElastic" => {
            if t == 0.0 || t == 1.0 {
                t
            } else {
                -(2f64.powf(10.0 * t - 10.0)) * ((t * 10.0 - 10.75) * C4).sin()
            }
        }
        "OutElastic" => {
            if t == 0.0 || t == 1.0 {
                t
            } else {
                2f64.powf(-10.0 * t) * ((t * 10.0 - 0.75) * C4).sin() + 1.0
            }
        }
        "InOutElastic" => {
            if t == 0.0 || t == 1.0 {
                t
            } else if t < 0.5 {
                -(2f64.powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * C5).sin()) / 2.0
            } else {
                2f64.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * C5).sin() / 2.0 + 1.0
            }
        }
        "InBounce" => 1.0 - out_bounce(1.0 - t),
        "OutBounce" => out_bounce(t),
        "InOutBounce" => {
            if t < 0.5 {
                (1.0 - out_bounce(1.0 - 2.0 * t)) / 2.0
            } else {
                (1.0 + out_bounce(2.0 * t - 1.0)) / 2.0
            }
        }
        _ => return None,
    })
}

#[derive(Clone, Copy)]
pub enum Point {
    Number(f64),
    Vector(Vec3),
}

impl Point {
    pub fn read(value: &LuaValue) -> LuaResult<Point> {
        match value {
            LuaValue::Integer(integer) => Ok(Point::Number(*integer as f64)),
            LuaValue::Number(number) => Ok(Point::Number(*number)),
            other if is_vector(other) => Ok(Point::Vector(read_vector(other)?)),
            other => Err(LuaError::runtime(format!(
                "expected a number or a Vector, got {}",
                other.type_name()
            ))),
        }
    }

    pub fn to_vector(self) -> Vec3 {
        match self {
            Point::Number(number) => Vec3::new(number, 0.0, 0.0),
            Point::Vector(vector) => vector,
        }
    }
}

pub fn read_points(points: &LuaTable) -> LuaResult<(Vec<Vec3>, bool)> {
    let mut vectors = Vec::new();
    let mut numbers = true;
    for value in points.sequence_values::<LuaValue>() {
        let point = Point::read(&value?)?;
        if matches!(point, Point::Vector(_)) {
            numbers = false;
        }
        vectors.push(point.to_vector());
    }
    Ok((vectors, numbers))
}

pub fn output(lua: &Lua, vector: Vec3, numbers: bool) -> LuaResult<LuaValue> {
    if numbers {
        Ok(LuaValue::Number(vector.x))
    } else {
        make_vector(lua, vector)
    }
}

pub fn bezier(points: &[Vec3], t: f64) -> Vec3 {
    let mut work = points.to_vec();
    for level in (1..work.len()).rev() {
        for index in 0..level {
            work[index] = work[index].lerp(work[index + 1], t);
        }
    }
    work[0]
}

fn catmull_rom_segment(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f64, alpha: f64) -> Vec3 {
    let knot =
        |a: Vec3, b: Vec3, previous: f64| previous + b.subtract(a).length().powf(alpha).max(1e-9);
    let t0 = 0.0;
    let t1 = knot(p0, p1, t0);
    let t2 = knot(p1, p2, t1);
    let t3 = knot(p2, p3, t2);
    let t = t1 + (t2 - t1) * t;
    let blend = |a: Vec3, b: Vec3, start: f64, end: f64| {
        a.scale((end - t) / (end - start))
            .add(b.scale((t - start) / (end - start)))
    };
    let a1 = blend(p0, p1, t0, t1);
    let a2 = blend(p1, p2, t1, t2);
    let a3 = blend(p2, p3, t2, t3);
    let b1 = blend(a1, a2, t0, t2);
    let b2 = blend(a2, a3, t1, t3);
    blend(b1, b2, t1, t2)
}

pub fn catmull_rom(points: &[Vec3], t: f64, closed: bool, alpha: f64) -> Vec3 {
    let count = points.len();
    let segments = if closed { count } else { count - 1 };
    let scaled = t.clamp(0.0, 1.0) * segments as f64;
    let segment = (scaled.floor() as usize).min(segments - 1);
    let local = scaled - segment as f64;
    let at = |index: isize| -> Vec3 {
        if closed {
            points[index.rem_euclid(count as isize) as usize]
        } else if index < 0 {
            points[0].scale(2.0).subtract(points[1])
        } else if index as usize >= count {
            points[count - 1].scale(2.0).subtract(points[count - 2])
        } else {
            points[index as usize]
        }
    };
    let index = segment as isize;
    catmull_rom_segment(
        at(index - 1),
        at(index),
        at(index + 1),
        at(index + 2),
        local,
        alpha,
    )
}

fn number_or_vector_lerp(lua: &Lua, from: LuaValue, to: LuaValue, t: f64) -> LuaResult<LuaValue> {
    match (Point::read(&from)?, Point::read(&to)?) {
        (Point::Number(from), Point::Number(to)) => Ok(LuaValue::Number(from + (to - from) * t)),
        (from, to) => make_vector(lua, from.to_vector().lerp(to.to_vector(), t)),
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set("Pi", PI)?;
    module.set("Tau", 2.0 * PI)?;
    module.set(
        "Clamp",
        lua.create_function(|_, (value, minimum, maximum): (f64, f64, f64)| {
            Ok(if minimum <= maximum {
                value.clamp(minimum, maximum)
            } else {
                value.clamp(maximum, minimum)
            })
        })?,
    )?;
    module.set(
        "Lerp",
        lua.create_function(|lua, (from, to, t): (LuaValue, LuaValue, f64)| {
            number_or_vector_lerp(lua, from, to, t)
        })?,
    )?;
    module.set(
        "InverseLerp",
        lua.create_function(|_, (from, to, value): (f64, f64, f64)| {
            Ok(if (to - from).abs() < f64::EPSILON {
                0.0
            } else {
                (value - from) / (to - from)
            })
        })?,
    )?;
    module.set(
        "Remap",
        lua.create_function(
            |_,
             (value, from_min, from_max, to_min, to_max, clamp): (
                f64,
                f64,
                f64,
                f64,
                f64,
                Option<bool>,
            )| {
                let mut t = if (from_max - from_min).abs() < f64::EPSILON {
                    0.0
                } else {
                    (value - from_min) / (from_max - from_min)
                };
                if clamp.unwrap_or(false) {
                    t = t.clamp(0.0, 1.0);
                }
                Ok(to_min + (to_max - to_min) * t)
            },
        )?,
    )?;
    module.set(
        "SmoothStep",
        lua.create_function(|_, (edge0, edge1, value): (f64, f64, f64)| {
            if (edge1 - edge0).abs() < f64::EPSILON {
                return Ok(if value < edge0 { 0.0 } else { 1.0 });
            }
            let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
            Ok(t * t * (3.0 - 2.0 * t))
        })?,
    )?;
    module.set(
        "MoveTowards",
        lua.create_function(|_, (current, target, step): (f64, f64, f64)| {
            Ok(if (target - current).abs() <= step {
                target
            } else {
                current + (target - current).signum() * step
            })
        })?,
    )?;
    module.set(
        "Round",
        lua.create_function(|_, (value, decimals): (f64, Option<i32>)| {
            let factor = 10f64.powi(decimals.unwrap_or(0));
            Ok((value * factor).round() / factor)
        })?,
    )?;
    module.set(
        "Sign",
        lua.create_function(|_, value: f64| {
            Ok(if value > 0.0 {
                1
            } else if value < 0.0 {
                -1
            } else {
                0
            })
        })?,
    )?;
    module.set(
        "Wrap",
        lua.create_function(|_, (value, minimum, maximum): (f64, f64, f64)| {
            let range = maximum - minimum;
            if range <= 0.0 {
                return Err(LuaError::runtime(
                    "the maximum has to be greater than the minimum",
                ));
            }
            Ok(minimum + (value - minimum).rem_euclid(range))
        })?,
    )?;
    module.set(
        "NormalizeAngle",
        lua.create_function(|_, angle: f64| Ok(crate::quaternion::normalize_axis(angle)))?,
    )?;
    module.set(
        "DeltaAngle",
        lua.create_function(|_, (from, to): (f64, f64)| {
            Ok(crate::quaternion::normalize_axis(to - from))
        })?,
    )?;
    module.set(
        "LerpAngle",
        lua.create_function(|_, (from, to, t): (f64, f64, f64)| {
            Ok(from + crate::quaternion::normalize_axis(to - from) * t)
        })?,
    )?;
    module.set(
        "IsNearlyEqual",
        lua.create_function(|_, (first, second, tolerance): (f64, f64, Option<f64>)| {
            Ok((first - second).abs() <= tolerance.unwrap_or(1e-8))
        })?,
    )?;

    module.set(
        "Ease",
        lua.create_function(|_, (name, t): (String, f64)| {
            ease(&name, t).ok_or_else(|| {
                LuaError::runtime(format!("unknown easing '{name}', see Math.GetEasings()"))
            })
        })?,
    )?;
    module.set(
        "GetEasings",
        lua.create_function(|lua, ()| lua.create_sequence_from(EASINGS.iter().copied()))?,
    )?;
    module.set(
        "Tween",
        lua.create_function(
            |lua, (from, to, t, easing): (LuaValue, LuaValue, f64, Option<String>)| {
                let name = easing.unwrap_or_else(|| "Linear".to_owned());
                let eased = ease(&name, t).ok_or_else(|| {
                    LuaError::runtime(format!("unknown easing '{name}', see Math.GetEasings()"))
                })?;
                number_or_vector_lerp(lua, from, to, eased)
            },
        )?,
    )?;

    module.set(
        "Bezier",
        lua.create_function(|lua, (points, t): (LuaTable, f64)| {
            let (points, numbers) = read_points(&points)?;
            if points.len() < 2 {
                return Err(LuaError::runtime("a Bezier curve needs at least 2 points"));
            }
            output(lua, bezier(&points, t.clamp(0.0, 1.0)), numbers)
        })?,
    )?;
    let spline_options = |options: &quark_core::Options| -> LuaResult<(bool, f64)> {
        let closed: bool = options.get_or("Closed", false)?;
        let alpha: f64 = options.get_or("Alpha", 0.5)?;
        if !(0.0..=1.0).contains(&alpha) {
            return Err(LuaError::runtime("Alpha has to be between 0 and 1"));
        }
        Ok((closed, alpha))
    };
    module.set(
        "Spline",
        lua.create_function(
            move |lua, (points, t, options): (LuaTable, f64, quark_core::Options)| {
                let (points, numbers) = read_points(&points)?;
                if points.len() < 2 {
                    return Err(LuaError::runtime("a spline needs at least 2 points"));
                }
                let (closed, alpha) = spline_options(&options)?;
                output(lua, catmull_rom(&points, t, closed, alpha), numbers)
            },
        )?,
    )?;
    module.set(
        "SampleSpline",
        lua.create_function(
            move |lua, (points, count, options): (LuaTable, usize, quark_core::Options)| {
                let (points, numbers) = read_points(&points)?;
                if points.len() < 2 {
                    return Err(LuaError::runtime("a spline needs at least 2 points"));
                }
                if !(2..=100_000).contains(&count) {
                    return Err(LuaError::runtime(
                        "the sample count has to be between 2 and 100000",
                    ));
                }
                let (closed, alpha) = spline_options(&options)?;
                let result = lua.create_table_with_capacity(count, 0)?;
                for index in 0..count {
                    let t = index as f64 / if closed { count } else { count - 1 } as f64;
                    result.raw_push(output(
                        lua,
                        catmull_rom(&points, t, closed, alpha),
                        numbers,
                    )?)?;
                }
                Ok(result)
            },
        )?,
    )?;
    Ok(())
}
