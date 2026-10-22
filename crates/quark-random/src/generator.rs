use mlua::prelude::*;
use quark_core::{create_userdata, package_environment};

use crate::operations::OPERATIONS;
use crate::rng::{Rng, random_seed, seed_from_text};

pub struct Generator(pub Rng);

struct DefaultGenerators(LuaTable);

pub fn read_seed(value: LuaValue) -> LuaResult<u64> {
    match value {
        LuaNil => Ok(random_seed()),
        LuaValue::Integer(integer) => Ok(integer as u64),
        LuaValue::Number(number) if number.fract() == 0.0 => Ok(number as i64 as u64),
        LuaValue::String(text) => Ok(seed_from_text(&text.to_str()?)),
        other => Err(LuaError::runtime(format!(
            "a seed has to be an integer or a string, got {}",
            other.type_name()
        ))),
    }
}

fn seed_value(seed: u64) -> i64 {
    seed as i64
}

fn default_generators(lua: &Lua) -> LuaResult<LuaTable> {
    if let Some(generators) = lua.app_data_ref::<DefaultGenerators>() {
        return Ok(generators.0.clone());
    }

    let generators = lua.create_table()?;
    let weak_keys = lua.create_table()?;
    weak_keys.set("__mode", "k")?;
    generators.set_metatable(Some(weak_keys))?;
    lua.set_app_data(DefaultGenerators(generators.clone()));
    Ok(generators)
}

pub fn default_generator(lua: &Lua) -> LuaResult<LuaAnyUserData> {
    let generators = default_generators(lua)?;
    let key = match package_environment(lua) {
        Some(environment) => LuaValue::Table(environment),
        None => LuaValue::Boolean(true),
    };
    if let Some(generator) = generators.raw_get::<Option<LuaAnyUserData>>(&key)? {
        return Ok(generator);
    }

    let generator = create_userdata(lua, Generator(Rng::from_seed(random_seed())))?;
    generators.raw_set(key, &generator)?;
    Ok(generator)
}

pub fn with_rng<T>(
    lua: &Lua,
    generator: Option<LuaAnyUserData>,
    work: impl FnOnce(&mut Rng) -> T,
) -> LuaResult<T> {
    let generator = match generator {
        Some(generator) => generator,
        None => default_generator(lua)?,
    };
    let mut generator = generator.borrow_mut::<Generator>()?;
    Ok(work(&mut generator.0))
}

impl LuaUserData for Generator {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        for &(name, operation) in OPERATIONS {
            methods.add_method_mut(name, move |lua, generator, values: LuaMultiValue| {
                operation(&mut generator.0, lua, values)
            });
        }

        methods.add_method("GetSeed", |_, generator, ()| {
            Ok(seed_value(generator.0.seed()))
        });
        methods.add_method("GetState", |_, generator, ()| Ok(generator.0.state_text()));
        methods.add_method_mut("SetState", |_, generator, state: String| {
            generator
                .0
                .set_state_text(&state)
                .map_err(LuaError::runtime)
        });
        methods.add_method("Clone", |_, generator, ()| {
            Ok(quark_core::Object(Generator(generator.0.clone())))
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, generator, ()| {
            Ok(format!("Generator({})", seed_value(generator.0.seed())))
        });
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    for &(name, operation) in OPERATIONS {
        module.set(
            name,
            lua.create_function(move |lua, values: LuaMultiValue| {
                with_rng(lua, None, |rng| operation(rng, lua, values))?
            })?,
        )?;
    }

    module.set(
        "New",
        lua.create_function(|_, seed: LuaValue| {
            Ok(quark_core::Object(Generator(Rng::from_seed(read_seed(
                seed,
            )?))))
        })?,
    )?;
    module.set(
        "Seed",
        lua.create_function(|lua, seed: LuaValue| {
            let seed = read_seed(seed)?;
            with_rng(lua, None, |rng| *rng = Rng::from_seed(seed))?;
            Ok(seed_value(seed))
        })?,
    )?;
    module.set(
        "GetDefault",
        lua.create_function(|lua, ()| default_generator(lua))?,
    )?;
    module.set(
        "HashSeed",
        lua.create_function(|_, text: String| Ok(seed_value(seed_from_text(&text))))?,
    )?;
    Ok(())
}
