use std::f64::consts::PI;

use mlua::prelude::*;
use quark_core::{Object, create_userdata};

use quark_core::{Rotator, Vec3, make_rotator, make_vector, read_rotator, read_vector};

const RADIANS: f64 = PI / 180.0;
const DEGREES: f64 = 180.0 / PI;
const SINGULARITY: f64 = 0.499_999_5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quaternion {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub w: f64,
}

pub fn normalize_axis(angle: f64) -> f64 {
    let wrapped = angle.rem_euclid(360.0);
    if wrapped > 180.0 {
        wrapped - 360.0
    } else {
        wrapped
    }
}

impl Quaternion {
    pub const IDENTITY: Quaternion = Quaternion {
        x: 0.0,
        y: 0.0,
        z: 0.0,
        w: 1.0,
    };

    pub fn from_rotator(rotator: Rotator) -> Self {
        let half = PI / 360.0;
        let (sin_pitch, cos_pitch) = (rotator.pitch * half).sin_cos();
        let (sin_yaw, cos_yaw) = (rotator.yaw * half).sin_cos();
        let (sin_roll, cos_roll) = (rotator.roll * half).sin_cos();
        Quaternion {
            x: cos_roll * sin_pitch * sin_yaw - sin_roll * cos_pitch * cos_yaw,
            y: -cos_roll * sin_pitch * cos_yaw - sin_roll * cos_pitch * sin_yaw,
            z: cos_roll * cos_pitch * sin_yaw - sin_roll * sin_pitch * cos_yaw,
            w: cos_roll * cos_pitch * cos_yaw + sin_roll * sin_pitch * sin_yaw,
        }
    }

    pub fn to_rotator(self) -> Rotator {
        let Quaternion { x, y, z, w } = self;
        let singularity = z * x - w * y;
        let yaw = (2.0 * (w * z + x * y)).atan2(1.0 - 2.0 * (y * y + z * z)) * DEGREES;
        if singularity < -SINGULARITY {
            Rotator {
                pitch: -90.0,
                yaw,
                roll: normalize_axis(-yaw - 2.0 * x.atan2(w) * DEGREES),
            }
        } else if singularity > SINGULARITY {
            Rotator {
                pitch: 90.0,
                yaw,
                roll: normalize_axis(yaw - 2.0 * x.atan2(w) * DEGREES),
            }
        } else {
            Rotator {
                pitch: (2.0 * singularity).asin() * DEGREES,
                yaw,
                roll: (-2.0 * (w * x + y * z)).atan2(1.0 - 2.0 * (x * x + y * y)) * DEGREES,
            }
        }
    }

    pub fn from_axis_angle(axis: Vec3, degrees: f64) -> Self {
        let axis = axis.normalized();
        let (sin, cos) = (degrees * RADIANS / 2.0).sin_cos();
        Quaternion {
            x: axis.x * sin,
            y: axis.y * sin,
            z: axis.z * sin,
            w: cos,
        }
    }

    pub fn between(from: Vec3, to: Vec3) -> Self {
        let from = from.normalized();
        let to = to.normalized();
        let dot = from.dot(to);
        if dot < -0.999_999 {
            let mut axis = Vec3::new(1.0, 0.0, 0.0).cross(from);
            if axis.length() < 1e-6 {
                axis = Vec3::new(0.0, 1.0, 0.0).cross(from);
            }
            return Quaternion::from_axis_angle(axis, 180.0);
        }
        let axis = from.cross(to);
        Quaternion {
            x: axis.x,
            y: axis.y,
            z: axis.z,
            w: 1.0 + dot,
        }
        .normalized()
    }

    pub fn length(self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z + self.w * self.w).sqrt()
    }

    pub fn normalized(self) -> Self {
        let length = self.length();
        if length < 1e-12 {
            return Quaternion::IDENTITY;
        }
        Quaternion {
            x: self.x / length,
            y: self.y / length,
            z: self.z / length,
            w: self.w / length,
        }
    }

    pub fn conjugate(self) -> Self {
        Quaternion {
            x: -self.x,
            y: -self.y,
            z: -self.z,
            w: self.w,
        }
    }

    pub fn inverse(self) -> Self {
        let squared = self.length().powi(2);
        if squared < 1e-12 {
            return Quaternion::IDENTITY;
        }
        let conjugate = self.conjugate();
        Quaternion {
            x: conjugate.x / squared,
            y: conjugate.y / squared,
            z: conjugate.z / squared,
            w: conjugate.w / squared,
        }
    }

    pub fn multiply(self, other: Quaternion) -> Self {
        Quaternion {
            x: self.w * other.x + self.x * other.w + self.y * other.z - self.z * other.y,
            y: self.w * other.y - self.x * other.z + self.y * other.w + self.z * other.x,
            z: self.w * other.z + self.x * other.y - self.y * other.x + self.z * other.w,
            w: self.w * other.w - self.x * other.x - self.y * other.y - self.z * other.z,
        }
    }

    pub fn dot(self, other: Quaternion) -> f64 {
        self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w
    }

    pub fn rotate(self, vector: Vec3) -> Vec3 {
        let axis = Vec3::new(self.x, self.y, self.z);
        let t = axis.cross(vector).scale(2.0);
        vector.plus(t.scale(self.w)).plus(axis.cross(t))
    }

    pub fn slerp(self, other: Quaternion, t: f64) -> Self {
        let mut other = other;
        let mut cosine = self.dot(other);
        if cosine < 0.0 {
            other = Quaternion {
                x: -other.x,
                y: -other.y,
                z: -other.z,
                w: -other.w,
            };
            cosine = -cosine;
        }
        let (scale_from, scale_to) = if cosine > 0.9999 {
            (1.0 - t, t)
        } else {
            let omega = cosine.clamp(-1.0, 1.0).acos();
            let sin = omega.sin();
            (((1.0 - t) * omega).sin() / sin, (t * omega).sin() / sin)
        };
        Quaternion {
            x: self.x * scale_from + other.x * scale_to,
            y: self.y * scale_from + other.y * scale_to,
            z: self.z * scale_from + other.z * scale_to,
            w: self.w * scale_from + other.w * scale_to,
        }
        .normalized()
    }

    pub fn axis_angle(self) -> (Vec3, f64) {
        let quaternion = self.normalized();
        let angle = 2.0 * quaternion.w.clamp(-1.0, 1.0).acos();
        let sin = (1.0 - quaternion.w * quaternion.w).sqrt();
        let axis = if sin < 1e-9 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(quaternion.x / sin, quaternion.y / sin, quaternion.z / sin)
        };
        (axis, angle * DEGREES)
    }

    pub fn angle_to(self, other: Quaternion) -> f64 {
        let dot = self.normalized().dot(other.normalized()).abs().min(1.0);
        2.0 * dot.acos() * DEGREES
    }
}

pub fn read_quaternion(value: &LuaValue) -> LuaResult<Quaternion> {
    match value {
        LuaValue::UserData(userdata) if userdata.is::<Quaternion>() => {
            Ok(*userdata.borrow::<Quaternion>()?)
        }
        other => Ok(Quaternion::from_rotator(read_rotator(other)?)),
    }
}

impl LuaUserData for Quaternion {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("X", |_, quaternion| Ok(quaternion.x));
        fields.add_field_method_get("Y", |_, quaternion| Ok(quaternion.y));
        fields.add_field_method_get("Z", |_, quaternion| Ok(quaternion.z));
        fields.add_field_method_get("W", |_, quaternion| Ok(quaternion.w));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Multiply", |_, quaternion, other: LuaValue| {
            Ok(Object(quaternion.multiply(read_quaternion(&other)?)))
        });
        methods.add_method("RotateVector", |lua, quaternion, vector: LuaValue| {
            make_vector(lua, quaternion.rotate(read_vector(&vector)?))
        });
        methods.add_method("UnrotateVector", |lua, quaternion, vector: LuaValue| {
            make_vector(lua, quaternion.inverse().rotate(read_vector(&vector)?))
        });
        methods.add_method("Inverse", |_, quaternion, ()| {
            Ok(Object(quaternion.inverse()))
        });
        methods.add_method("Normalize", |_, quaternion, ()| {
            Ok(Object(quaternion.normalized()))
        });
        methods.add_method("Dot", |_, quaternion, other: LuaValue| {
            Ok(quaternion.dot(read_quaternion(&other)?))
        });
        methods.add_method("AngleTo", |_, quaternion, other: LuaValue| {
            Ok(quaternion.angle_to(read_quaternion(&other)?))
        });
        methods.add_method("Slerp", |_, quaternion, (other, t): (LuaValue, f64)| {
            Ok(Object(quaternion.slerp(read_quaternion(&other)?, t)))
        });
        methods.add_method("ToRotator", |lua, quaternion, ()| {
            make_rotator(lua, quaternion.to_rotator())
        });
        methods.add_method("ToAxisAngle", |lua, quaternion, ()| {
            let (axis, angle) = quaternion.axis_angle();
            (make_vector(lua, axis)?, angle).into_lua_multi(lua)
        });
        methods.add_method("GetForward", |lua, quaternion, ()| {
            make_vector(lua, quaternion.rotate(Vec3::new(1.0, 0.0, 0.0)))
        });
        methods.add_method("GetRight", |lua, quaternion, ()| {
            make_vector(lua, quaternion.rotate(Vec3::new(0.0, 1.0, 0.0)))
        });
        methods.add_method("GetUp", |lua, quaternion, ()| {
            make_vector(lua, quaternion.rotate(Vec3::new(0.0, 0.0, 1.0)))
        });
        methods.add_meta_method(LuaMetaMethod::Mul, |lua, quaternion, other: LuaValue| {
            if let LuaValue::UserData(userdata) = &other
                && userdata.is::<Quaternion>()
            {
                return Object(quaternion.multiply(*userdata.borrow::<Quaternion>()?))
                    .into_lua(lua);
            }
            make_vector(lua, quaternion.rotate(read_vector(&other)?))
        });
        methods.add_meta_method(LuaMetaMethod::Eq, |_, quaternion, other: LuaAnyUserData| {
            Ok(other
                .borrow::<Quaternion>()
                .is_ok_and(|other| *other == *quaternion))
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, quaternion, ()| {
            Ok(format!(
                "Quaternion(X = {:.4}, Y = {:.4}, Z = {:.4}, W = {:.4})",
                quaternion.x, quaternion.y, quaternion.z, quaternion.w
            ))
        });
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "Quaternion",
        lua.create_function(
            |lua, (x, y, z, w): (Option<f64>, Option<f64>, Option<f64>, Option<f64>)| {
                let quaternion = match (x, y, z, w) {
                    (None, None, None, None) => Quaternion::IDENTITY,
                    (Some(x), Some(y), Some(z), Some(w)) => Quaternion { x, y, z, w },
                    _ => {
                        return Err(LuaError::runtime(
                            "a quaternion needs X, Y, Z and W, or no values for the identity",
                        ));
                    }
                };
                create_userdata(lua, quaternion)
            },
        )?,
    )?;
    module.set(
        "QuaternionFromRotator",
        lua.create_function(|_, rotator: LuaValue| {
            Ok(Object(Quaternion::from_rotator(read_rotator(&rotator)?)))
        })?,
    )?;
    module.set(
        "QuaternionFromAxisAngle",
        lua.create_function(|_, (axis, degrees): (LuaValue, f64)| {
            Ok(Object(Quaternion::from_axis_angle(
                read_vector(&axis)?,
                degrees,
            )))
        })?,
    )?;
    module.set(
        "QuaternionBetween",
        lua.create_function(|_, (from, to): (LuaValue, LuaValue)| {
            Ok(Object(Quaternion::between(
                read_vector(&from)?,
                read_vector(&to)?,
            )))
        })?,
    )?;
    module.set(
        "IsQuaternion",
        lua.create_function(|_, value: LuaValue| {
            Ok(matches!(value, LuaValue::UserData(userdata) if userdata.is::<Quaternion>()))
        })?,
    )?;

    module.set(
        "RotateVector",
        lua.create_function(|lua, (vector, rotation): (LuaValue, LuaValue)| {
            make_vector(
                lua,
                read_quaternion(&rotation)?.rotate(read_vector(&vector)?),
            )
        })?,
    )?;
    module.set(
        "UnrotateVector",
        lua.create_function(|lua, (vector, rotation): (LuaValue, LuaValue)| {
            make_vector(
                lua,
                read_quaternion(&rotation)?
                    .inverse()
                    .rotate(read_vector(&vector)?),
            )
        })?,
    )?;
    module.set(
        "GetDirections",
        lua.create_function(|lua, rotation: LuaValue| {
            let quaternion = read_quaternion(&rotation)?;
            (
                make_vector(lua, quaternion.rotate(Vec3::new(1.0, 0.0, 0.0)))?,
                make_vector(lua, quaternion.rotate(Vec3::new(0.0, 1.0, 0.0)))?,
                make_vector(lua, quaternion.rotate(Vec3::new(0.0, 0.0, 1.0)))?,
            )
                .into_lua_multi(lua)
        })?,
    )?;
    module.set(
        "LookAt",
        lua.create_function(|lua, (from, to): (LuaValue, LuaValue)| {
            let direction = read_vector(&to)?.subtract(read_vector(&from)?);
            let horizontal = (direction.x * direction.x + direction.y * direction.y).sqrt();
            make_rotator(
                lua,
                Rotator {
                    pitch: direction.z.atan2(horizontal) * DEGREES,
                    yaw: direction.y.atan2(direction.x) * DEGREES,
                    roll: 0.0,
                },
            )
        })?,
    )?;
    module.set(
        "SlerpRotator",
        lua.create_function(|lua, (from, to, t): (LuaValue, LuaValue, f64)| {
            let result = Quaternion::from_rotator(read_rotator(&from)?)
                .slerp(Quaternion::from_rotator(read_rotator(&to)?), t);
            make_rotator(lua, result.to_rotator())
        })?,
    )?;
    module.set(
        "CombineRotators",
        lua.create_function(|lua, (first, second): (LuaValue, LuaValue)| {
            let combined = Quaternion::from_rotator(read_rotator(&second)?)
                .multiply(Quaternion::from_rotator(read_rotator(&first)?));
            make_rotator(lua, combined.to_rotator())
        })?,
    )?;
    module.set(
        "NormalizeRotator",
        lua.create_function(|lua, rotator: LuaValue| {
            let rotator = read_rotator(&rotator)?;
            make_rotator(
                lua,
                Rotator {
                    pitch: normalize_axis(rotator.pitch),
                    yaw: normalize_axis(rotator.yaw),
                    roll: normalize_axis(rotator.roll),
                },
            )
        })?,
    )?;
    Ok(())
}
