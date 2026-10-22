use std::borrow::Cow;
use std::collections::BTreeMap;

use fluent_bundle::{FluentArgs, FluentBundle, FluentError, FluentResource, FluentValue};
use fluent_langneg::{
    LanguageIdentifier as NegotiationIdentifier, NegotiationStrategy, negotiate_languages,
};
use intl_pluralrules::{PluralCategory, PluralRuleType, PluralRules};
use mlua::prelude::*;
use quark_core::{Options, create_userdata};
use unic_langid::LanguageIdentifier;

pub struct Locale {
    bundles: BTreeMap<String, FluentBundle<FluentResource>>,
    fallback: Option<String>,
    isolating: bool,
}

fn parse_identifier(name: &str) -> LuaResult<LanguageIdentifier> {
    name.parse().map_err(|_| {
        LuaError::runtime(format!(
            "'{name}' is not a valid language tag like en or de-AT"
        ))
    })
}

fn describe_errors(errors: &[FluentError]) -> String {
    errors
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("; ")
}

fn read_arguments(arguments: Option<LuaTable>) -> LuaResult<Option<FluentArgs<'static>>> {
    let Some(arguments) = arguments else {
        return Ok(None);
    };
    let mut result = FluentArgs::new();
    for pair in arguments.pairs::<String, LuaValue>() {
        let (name, value) = pair?;
        let value: FluentValue<'static> = match value {
            LuaValue::Integer(number) => FluentValue::from(number),
            LuaValue::Number(number) => FluentValue::from(number),
            LuaValue::String(text) => FluentValue::from(text.to_string_lossy()),
            LuaValue::Boolean(flag) => FluentValue::from(flag.to_string()),
            other => FluentValue::from(other.to_string()?),
        };
        result.set(name, value);
    }
    Ok(Some(result))
}

impl Locale {
    fn add(&mut self, locale: &str, source: String) -> LuaResult<Result<(), String>> {
        let identifier = parse_identifier(locale)?;
        let key = identifier.to_string();
        let resource = match FluentResource::try_new(source) {
            Ok(resource) => resource,
            Err((_, errors)) => {
                let messages: Vec<String> = errors
                    .iter()
                    .map(|error| format!("{:?} at {:?}", error.kind, error.pos))
                    .collect();
                return Ok(Err(format!(
                    "invalid Fluent syntax for {key}: {}",
                    messages.join("; ")
                )));
            }
        };
        let isolating = self.isolating;
        let bundle = self.bundles.entry(key.clone()).or_insert_with(|| {
            let mut bundle = FluentBundle::new(vec![identifier.clone()]);
            bundle.set_use_isolating(isolating);
            bundle
        });
        bundle.add_resource_overriding(resource);
        Ok(Ok(()))
    }

    fn chain(&self, locale: &str) -> Vec<&str> {
        let mut candidates = Vec::new();
        if let Ok(identifier) = locale.parse::<LanguageIdentifier>() {
            candidates.push(identifier.to_string());
            let mut parent = identifier.clone();
            parent.region = None;
            candidates.push(parent.to_string());
            parent.script = None;
            parent.clear_variants();
            candidates.push(parent.to_string());
        }
        if let Some(fallback) = &self.fallback {
            candidates.push(fallback.clone());
        }

        let mut chain: Vec<&str> = Vec::new();
        for candidate in candidates {
            if let Some((key, _)) = self.bundles.get_key_value(&candidate)
                && !chain.contains(&key.as_str())
            {
                chain.push(key.as_str());
            }
        }
        chain
    }

    fn format(
        &self,
        locale: &str,
        id: &str,
        arguments: Option<&FluentArgs>,
    ) -> Option<(String, Vec<FluentError>)> {
        let (message_id, attribute) = match id.split_once('.') {
            Some((message, attribute)) => (message, Some(attribute)),
            None => (id, None),
        };
        for name in self.chain(locale) {
            let bundle = &self.bundles[name];
            let Some(message) = bundle.get_message(message_id) else {
                continue;
            };
            let pattern = match attribute {
                Some(attribute) => message
                    .get_attribute(attribute)
                    .map(|attribute| attribute.value()),
                None => message.value(),
            };
            let Some(pattern) = pattern else {
                continue;
            };
            let mut errors = Vec::new();
            let text: Cow<str> = bundle.format_pattern(pattern, arguments, &mut errors);
            return Some((text.into_owned(), errors));
        }
        None
    }

    fn negotiate(&self, requested: Vec<String>) -> Option<String> {
        let available: Vec<String> = self.bundles.keys().cloned().collect();
        negotiate(&requested, &available, self.fallback.as_deref())
    }
}

fn negotiate(requested: &[String], available: &[String], fallback: Option<&str>) -> Option<String> {
    let parse = |names: &[String]| -> Vec<NegotiationIdentifier> {
        names.iter().filter_map(|name| name.parse().ok()).collect()
    };
    let requested = parse(requested);
    let available = parse(available);
    let fallback: Option<NegotiationIdentifier> = fallback.and_then(|name| name.parse().ok());
    negotiate_languages(
        &requested,
        &available,
        fallback.as_ref(),
        NegotiationStrategy::Lookup,
    )
    .first()
    .map(|identifier| identifier.to_string())
}

fn requested_locales(value: LuaValue) -> LuaResult<Vec<String>> {
    match value {
        LuaValue::String(text) => Ok(text
            .to_str()?
            .split(',')
            .map(|part| part.split(';').next().unwrap_or("").trim().to_owned())
            .filter(|part| !part.is_empty())
            .collect()),
        LuaValue::Table(table) => table.sequence_values::<String>().collect(),
        other => Err(LuaError::runtime(format!(
            "expected a language tag, an Accept-Language header or a list of tags, got {}",
            other.type_name()
        ))),
    }
}

fn format_result(
    lua: &Lua,
    locale: &Locale,
    name: &str,
    id: &str,
    arguments: Option<LuaTable>,
) -> LuaResult<LuaMultiValue> {
    let arguments = read_arguments(arguments)?;
    match locale.format(name, id, arguments.as_ref()) {
        Some((text, errors)) if errors.is_empty() => (text, true).into_lua_multi(lua),
        Some((text, errors)) => (text, true, describe_errors(&errors)).into_lua_multi(lua),
        None => (id.to_owned(), false).into_lua_multi(lua),
    }
}

impl LuaUserData for Locale {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut(
            "Add",
            |lua, locale, (name, source): (String, String)| match locale.add(&name, source)? {
                Ok(()) => true.into_lua_multi(lua),
                Err(message) => (LuaNil, message).into_lua_multi(lua),
            },
        );
        methods.add_method(
            "Format",
            |lua, locale, (name, id, arguments): (String, String, Option<LuaTable>)| {
                format_result(lua, locale, &name, &id, arguments)
            },
        );
        methods.add_method("Has", |_, locale, (name, id): (String, String)| {
            let message_id = id
                .split_once('.')
                .map_or(id.as_str(), |(message, _)| message);
            Ok(locale
                .chain(&name)
                .iter()
                .any(|bundle| locale.bundles[*bundle].has_message(message_id)))
        });
        methods.add_method("GetLocales", |lua, locale, ()| {
            lua.create_sequence_from(locale.bundles.keys().cloned())
        });
        methods.add_method("GetFallback", |_, locale, ()| Ok(locale.fallback.clone()));
        methods.add_method("Negotiate", |_, locale, requested: LuaValue| {
            Ok(locale.negotiate(requested_locales(requested)?))
        });
        methods.add_function("Bind", |lua, (this, name): (LuaAnyUserData, String)| {
            parse_identifier(&name)?;
            lua.create_function(move |lua, (id, arguments): (String, Option<LuaTable>)| {
                let locale = this.borrow::<Locale>()?;
                format_result(lua, &locale, &name, &id, arguments)
            })
        });
    }
}

fn plural_category(locale: &str, number: f64, ordinal: bool) -> LuaResult<&'static str> {
    let identifier = parse_identifier(locale)?;
    let rule_type = if ordinal {
        PluralRuleType::ORDINAL
    } else {
        PluralRuleType::CARDINAL
    };
    let rules = PluralRules::create(identifier.clone(), rule_type)
        .or_else(|_| {
            let mut language = identifier.clone();
            language.region = None;
            language.script = None;
            language.clear_variants();
            PluralRules::create(language, rule_type)
        })
        .map_err(|_| LuaError::runtime(format!("no plural rules for '{locale}'")))?;
    let category = rules
        .select(number)
        .map_err(|error| LuaError::runtime(format!("cannot select a plural category: {error}")))?;
    Ok(match category {
        PluralCategory::ZERO => "zero",
        PluralCategory::ONE => "one",
        PluralCategory::TWO => "two",
        PluralCategory::FEW => "few",
        PluralCategory::MANY => "many",
        PluralCategory::OTHER => "other",
    })
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set(
        "New",
        lua.create_function(|lua, options: Options| {
            let fallback: Option<String> = options.get("Fallback")?;
            let fallback = fallback
                .map(|name| parse_identifier(&name).map(|identifier| identifier.to_string()))
                .transpose()?;
            let mut locale = Locale {
                bundles: BTreeMap::new(),
                fallback,
                isolating: options.get_or("Isolating", false)?,
            };
            if let Some(resources) = options.get::<LuaTable>("Resources")? {
                for pair in resources.pairs::<String, String>() {
                    let (name, source) = pair?;
                    if let Err(message) = locale.add(&name, source)? {
                        return (LuaNil, message).into_lua_multi(lua);
                    }
                }
            }
            create_userdata(lua, locale)?.into_lua_multi(lua)
        })?,
    )?;
    module.set(
        "PluralCategory",
        lua.create_function(|_, (locale, number, kind): (String, f64, Option<String>)| {
            let ordinal = match kind.as_deref() {
                None | Some("Cardinal") => false,
                Some("Ordinal") => true,
                Some(other) => {
                    return Err(LuaError::runtime(format!(
                        "unknown plural type '{other}', expected Cardinal or Ordinal"
                    )));
                }
            };
            plural_category(&locale, number, ordinal)
        })?,
    )?;
    module.set(
        "Negotiate",
        lua.create_function(
            |_, (requested, available, fallback): (LuaValue, Vec<String>, Option<String>)| {
                if let Some(fallback) = &fallback {
                    parse_identifier(fallback)?;
                }
                Ok(negotiate(
                    &requested_locales(requested)?,
                    &available,
                    fallback.as_deref(),
                ))
            },
        )?,
    )?;
    Ok(module)
}
