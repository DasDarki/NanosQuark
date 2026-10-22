use std::sync::Arc;

use fastnoise_lite::{
    CellularDistanceFunction, CellularReturnType, FastNoiseLite, FractalType, NoiseType,
};
use mlua::prelude::*;
use quark_core::{Buffer, Object, Options, create_userdata, options_and_callback};

const MAX_SAMPLES: usize = 16 * 1024 * 1024;

pub struct Noise(Arc<FastNoiseLite>);

fn choose<T: Copy>(
    name: &str,
    value: Option<String>,
    choices: &[(&str, T)],
    default: T,
) -> LuaResult<T> {
    let Some(value) = value else {
        return Ok(default);
    };
    choices
        .iter()
        .find(|(choice, _)| choice.eq_ignore_ascii_case(&value))
        .map(|(_, choice)| *choice)
        .ok_or_else(|| {
            let names = choices
                .iter()
                .map(|(choice, _)| *choice)
                .collect::<Vec<_>>()
                .join(", ");
            LuaError::runtime(format!("unknown {name} '{value}', expected one of {names}"))
        })
}

fn create(options: &Options) -> LuaResult<FastNoiseLite> {
    let seed = match options.get::<LuaValue>("Seed")? {
        None => 1337,
        Some(seed) => crate::generator::read_seed(seed)? as i32,
    };
    let mut noise = FastNoiseLite::with_seed(seed);

    noise.set_noise_type(Some(choose(
        "noise type",
        options.get("Type")?,
        &[
            ("OpenSimplex2", NoiseType::OpenSimplex2),
            ("OpenSimplex2S", NoiseType::OpenSimplex2S),
            ("Cellular", NoiseType::Cellular),
            ("Perlin", NoiseType::Perlin),
            ("ValueCubic", NoiseType::ValueCubic),
            ("Value", NoiseType::Value),
        ],
        NoiseType::OpenSimplex2,
    )?));
    noise.set_frequency(Some(options.get_or("Frequency", 0.01)?));
    noise.set_fractal_type(Some(choose(
        "fractal type",
        options.get("Fractal")?,
        &[
            ("None", FractalType::None),
            ("FBm", FractalType::FBm),
            ("Ridged", FractalType::Ridged),
            ("PingPong", FractalType::PingPong),
        ],
        FractalType::None,
    )?));

    let octaves: i32 = options.get_or("Octaves", 3)?;
    if !(1..=16).contains(&octaves) {
        return Err(LuaError::runtime("Octaves has to be between 1 and 16"));
    }
    noise.set_fractal_octaves(Some(octaves));
    noise.set_fractal_lacunarity(Some(options.get_or("Lacunarity", 2.0)?));
    noise.set_fractal_gain(Some(options.get_or("Gain", 0.5)?));
    noise.set_fractal_weighted_strength(Some(options.get_or("WeightedStrength", 0.0)?));
    noise.set_fractal_ping_pong_strength(Some(options.get_or("PingPongStrength", 2.0)?));

    noise.set_cellular_distance_function(Some(choose(
        "cellular distance",
        options.get("CellularDistance")?,
        &[
            ("Euclidean", CellularDistanceFunction::Euclidean),
            ("EuclideanSq", CellularDistanceFunction::EuclideanSq),
            ("Manhattan", CellularDistanceFunction::Manhattan),
            ("Hybrid", CellularDistanceFunction::Hybrid),
        ],
        CellularDistanceFunction::EuclideanSq,
    )?));
    noise.set_cellular_return_type(Some(choose(
        "cellular return type",
        options.get("CellularReturn")?,
        &[
            ("CellValue", CellularReturnType::CellValue),
            ("Distance", CellularReturnType::Distance),
            ("Distance2", CellularReturnType::Distance2),
            ("Distance2Add", CellularReturnType::Distance2Add),
            ("Distance2Sub", CellularReturnType::Distance2Sub),
            ("Distance2Mul", CellularReturnType::Distance2Mul),
            ("Distance2Div", CellularReturnType::Distance2Div),
        ],
        CellularReturnType::Distance,
    )?));
    noise.set_cellular_jitter(Some(options.get_or("CellularJitter", 1.0)?));

    Ok(noise)
}

struct Area {
    x: f64,
    y: f64,
    z: Option<f64>,
    step: f64,
    width: usize,
    height: usize,
}

fn fill(noise: &FastNoiseLite, area: &Area) -> Vec<u8> {
    let mut data = Vec::with_capacity(area.width * area.height * 4);
    for row in 0..area.height {
        let y = area.y + row as f64 * area.step;
        for column in 0..area.width {
            let x = area.x + column as f64 * area.step;
            let value = match area.z {
                Some(z) => noise.get_noise_3d(x, y, z),
                None => noise.get_noise_2d(x, y),
            };
            data.extend_from_slice(&value.to_le_bytes());
        }
    }
    data
}

impl LuaUserData for Noise {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Get", |_, noise, (x, y, z): (f64, f64, Option<f64>)| {
            Ok(match z {
                Some(z) => noise.0.get_noise_3d(x, y, z),
                None => noise.0.get_noise_2d(x, y),
            })
        });
        methods.add_method(
            "Fill",
            |lua, noise, (width, height, options, callback): (usize, usize, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                if width == 0 || height == 0 || width.saturating_mul(height) > MAX_SAMPLES {
                    return Err(LuaError::runtime(format!(
                        "the area needs between 1 and {MAX_SAMPLES} samples"
                    )));
                }
                let area = Area {
                    x: options.get_or("X", 0.0)?,
                    y: options.get_or("Y", 0.0)?,
                    z: options.get("Z")?,
                    step: options.get_or("Step", 1.0)?,
                    width,
                    height,
                };
                let generator = noise.0.clone();
                match callback {
                    None => Object(Buffer::new(lua, fill(&generator, &area))?).into_lua_multi(lua),
                    Some(callback) => {
                        let deliver = lua.create_function(move |lua, (data, error): (Option<LuaString>, Option<String>)| {
                            match data {
                                Some(data) => callback.call::<()>(Object(Buffer::new(lua, data.as_bytes().to_vec())?)),
                                None => callback.call::<()>((LuaNil, error)),
                            }
                        })?;
                        quark_core::spawn_blocking(lua, deliver, move || {
                            Ok::<_, String>(quark_core::Bytes(fill(&generator, &area)))
                        })?;
                        Ok(LuaMultiValue::new())
                    }
                }
            },
        );
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "Noise",
        lua.create_function(|lua, options: Options| {
            create_userdata(lua, Noise(Arc::new(create(&options)?)))
        })?,
    )
}
