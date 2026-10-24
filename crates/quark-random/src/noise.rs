use std::sync::Arc;

use fastnoise_lite::{
    CellularDistanceFunction, CellularReturnType, DomainWarpType, FastNoiseLite, FractalType,
    NoiseType,
};
use mlua::prelude::*;
use quark_core::{Buffer, Object, Options, create_userdata, options_and_callback};

const MAX_SAMPLES: usize = 16 * 1024 * 1024;

pub struct Noise(Arc<Sampler>);

pub(crate) struct Sampler {
    noise: FastNoiseLite,
    warp: Option<FastNoiseLite>,
}

impl Sampler {
    fn warp_2d(&self, x: f64, y: f64) -> (f64, f64) {
        match &self.warp {
            Some(warp) => warp.domain_warp_2d(x, y),
            None => (x, y),
        }
    }

    fn warp_3d(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        match &self.warp {
            Some(warp) => warp.domain_warp_3d(x, y, z),
            None => (x, y, z),
        }
    }

    pub(crate) fn sample(&self, x: f64, y: f64, z: Option<f64>) -> f32 {
        match z {
            Some(z) => {
                let (x, y, z) = self.warp_3d(x, y, z);
                self.noise.get_noise_3d(x, y, z)
            }
            None => {
                let (x, y) = self.warp_2d(x, y);
                self.noise.get_noise_2d(x, y)
            }
        }
    }
}

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

fn read_seed(options: &Options, default: i32) -> LuaResult<i32> {
    Ok(match options.get::<LuaValue>("Seed")? {
        None => default,
        Some(seed) => crate::generator::read_seed(seed)? as i32,
    })
}

fn read_fractal_settings(noise: &mut FastNoiseLite, options: &Options) -> LuaResult<()> {
    let octaves: i32 = options.get_or("Octaves", 3)?;
    if !(1..=16).contains(&octaves) {
        return Err(LuaError::runtime("Octaves has to be between 1 and 16"));
    }
    noise.set_fractal_octaves(Some(octaves));
    noise.set_fractal_lacunarity(Some(options.get_or("Lacunarity", 2.0)?));
    noise.set_fractal_gain(Some(options.get_or("Gain", 0.5)?));
    Ok(())
}

fn create_warp(options: &Options, seed: i32, frequency: f32) -> LuaResult<FastNoiseLite> {
    let mut warp = FastNoiseLite::with_seed(read_seed(options, seed)?);
    warp.set_domain_warp_type(Some(choose(
        "warp type",
        options.get("Type")?,
        &[
            ("OpenSimplex2", DomainWarpType::OpenSimplex2),
            ("OpenSimplex2Reduced", DomainWarpType::OpenSimplex2Reduced),
            ("BasicGrid", DomainWarpType::BasicGrid),
        ],
        DomainWarpType::OpenSimplex2,
    )?));
    warp.set_domain_warp_amp(Some(options.get_or("Amplitude", 30.0)?));
    warp.set_frequency(Some(options.get_or("Frequency", frequency)?));
    warp.set_fractal_type(Some(choose(
        "warp fractal type",
        options.get("Fractal")?,
        &[
            ("None", FractalType::None),
            ("Progressive", FractalType::DomainWarpProgressive),
            ("Independent", FractalType::DomainWarpIndependent),
        ],
        FractalType::None,
    )?));
    read_fractal_settings(&mut warp, options)?;
    Ok(warp)
}

pub(crate) fn create(options: &Options) -> LuaResult<Sampler> {
    let seed = read_seed(options, 1337)?;
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
    let frequency: f32 = options.get_or("Frequency", 0.01)?;
    noise.set_frequency(Some(frequency));
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

    read_fractal_settings(&mut noise, options)?;
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

    let warp = match options.get::<LuaValue>("Warp")? {
        None | Some(LuaValue::Boolean(false)) => None,
        Some(LuaValue::Boolean(true)) => Some(create_warp(&Options::default(), seed, frequency)?),
        Some(LuaValue::Table(table)) => {
            Some(create_warp(&Options::from_table(table), seed, frequency)?)
        }
        Some(other) => {
            return Err(LuaError::runtime(format!(
                "the Warp option has to be a table or true, got {}",
                other.type_name()
            )));
        }
    };

    Ok(Sampler { noise, warp })
}

struct Area {
    x: f64,
    y: f64,
    z: Option<f64>,
    step: f64,
    width: usize,
    height: usize,
}

fn fill(noise: &Sampler, area: &Area) -> Vec<u8> {
    let mut data = Vec::with_capacity(area.width * area.height * 4);
    for row in 0..area.height {
        let y = area.y + row as f64 * area.step;
        for column in 0..area.width {
            let x = area.x + column as f64 * area.step;
            let value = noise.sample(x, y, area.z);
            data.extend_from_slice(&value.to_le_bytes());
        }
    }
    data
}

impl LuaUserData for Noise {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Get", |_, noise, (x, y, z): (f64, f64, Option<f64>)| {
            Ok(noise.0.sample(x, y, z))
        });
        methods.add_method(
            "Warp",
            |lua, noise, (x, y, z): (f64, f64, Option<f64>)| match z {
                Some(z) => noise.0.warp_3d(x, y, z).into_lua_multi(lua),
                None => noise.0.warp_2d(x, y).into_lua_multi(lua),
            },
        );
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
                        })?
                        .into_lua_multi(lua)
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
