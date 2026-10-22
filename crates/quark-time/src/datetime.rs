use jiff::civil::{Date, Time};
use jiff::fmt::temporal::{Pieces, PiecesOffset};
use jiff::fmt::{rfc2822, strtime};
use jiff::tz::TimeZone;
use jiff::{SignedDuration, Span, Timestamp, Unit, Zoned};
use mlua::prelude::*;
use quark_core::{Object, Options};

use crate::duration::{parse_span, read_milliseconds};

#[derive(Clone)]
pub struct DateTime(pub Zoned);

const UNITS: &[(&str, Unit)] = &[
    ("Years", Unit::Year),
    ("Months", Unit::Month),
    ("Weeks", Unit::Week),
    ("Days", Unit::Day),
    ("Hours", Unit::Hour),
    ("Minutes", Unit::Minute),
    ("Seconds", Unit::Second),
    ("Milliseconds", Unit::Millisecond),
];

fn error(error: impl std::fmt::Display) -> LuaError {
    LuaError::runtime(error.to_string())
}

pub fn zone(name: &str) -> LuaResult<TimeZone> {
    match name {
        "UTC" | "utc" | "Z" => Ok(TimeZone::UTC),
        "Local" | "local" => Ok(TimeZone::system()),
        name => TimeZone::get(name)
            .map_err(|_| LuaError::runtime(format!("unknown time zone '{name}'"))),
    }
}

pub fn zone_option(name: Option<String>) -> LuaResult<TimeZone> {
    match name {
        Some(name) => zone(&name),
        None => Ok(TimeZone::system()),
    }
}

fn zone_name(zoned: &Zoned) -> String {
    match zoned.time_zone().iana_name() {
        Some(name) => name.to_owned(),
        None => zoned.offset().to_string(),
    }
}

fn parse_automatic(
    text: &str,
    fallback: TimeZone,
    convert: Option<TimeZone>,
) -> Result<Zoned, String> {
    let text = text.trim();
    let parsed = match Pieces::parse(text) {
        Ok(pieces) if pieces.time_zone_annotation().is_some() => {
            text.parse::<Zoned>().map_err(|error| error.to_string())
        }
        Ok(pieces) => {
            let datetime = pieces
                .date()
                .to_datetime(pieces.time().unwrap_or(Time::midnight()));
            let zone = match pieces.offset() {
                Some(PiecesOffset::Zulu) => TimeZone::UTC,
                Some(PiecesOffset::Numeric(offset)) => TimeZone::fixed(offset.offset()),
                Some(_) => fallback,
                None => fallback,
            };
            datetime.to_zoned(zone).map_err(|error| error.to_string())
        }
        Err(_) => rfc2822::parse(text).map_err(|_| format!("'{text}' is not a known date format")),
    }?;
    Ok(match convert {
        Some(zone) => parsed.with_time_zone(zone),
        None => parsed,
    })
}

pub fn parse(text: &str, options: &Options) -> LuaResult<Result<Zoned, String>> {
    let zone_name: Option<String> = options.get("Zone")?;
    let target = zone_name.as_deref().map(zone).transpose()?;
    let format: Option<String> = options.get("Format")?;

    Ok(match format {
        None => parse_automatic(text, target.clone().unwrap_or(TimeZone::system()), target),
        Some(format) => strtime::parse(&format, text)
            .map_err(|error| error.to_string())
            .and_then(|parsed| {
                if parsed.offset().is_some() || parsed.iana_time_zone().is_some() {
                    let zoned = parsed.to_zoned().map_err(|error| error.to_string())?;
                    return Ok(match &target {
                        Some(zone) => zoned.with_time_zone(zone.clone()),
                        None => zoned,
                    });
                }
                let datetime = match parsed.to_datetime() {
                    Ok(datetime) => datetime,
                    Err(_) => parsed
                        .to_date()
                        .map_err(|error| error.to_string())?
                        .to_datetime(Time::midnight()),
                };
                datetime
                    .to_zoned(target.clone().unwrap_or(TimeZone::system()))
                    .map_err(|error| error.to_string())
            }),
    })
}

pub fn span_from_table(table: &LuaTable) -> LuaResult<Span> {
    let mut span = Span::new();
    for pair in table.pairs::<String, i64>() {
        let (name, value) = pair?;
        span = match name.as_str() {
            "Years" => span.try_years(value),
            "Months" => span.try_months(value),
            "Weeks" => span.try_weeks(value),
            "Days" => span.try_days(value),
            "Hours" => span.try_hours(value),
            "Minutes" => span.try_minutes(value),
            "Seconds" => span.try_seconds(value),
            "Milliseconds" => span.try_milliseconds(value),
            other => {
                return Err(LuaError::runtime(format!(
                    "unknown duration field '{other}'"
                )));
            }
        }
        .map_err(error)?;
    }
    Ok(span)
}

fn shift(zoned: &Zoned, value: LuaValue, direction: i64) -> LuaResult<Zoned> {
    let result = match value {
        LuaValue::Table(table) => {
            let span = span_from_table(&table)?;
            zoned.checked_add(if direction < 0 { span.negate() } else { span })
        }
        LuaValue::String(text) => {
            let span = parse_span(&text.to_str()?).map_err(LuaError::runtime)?;
            zoned.checked_add(if direction < 0 { span.negate() } else { span })
        }
        other => {
            let milliseconds = read_milliseconds(other)?;
            zoned.checked_add(SignedDuration::from_millis(milliseconds * direction))
        }
    };
    result.map_err(error)
}

fn unit(name: &str) -> LuaResult<Unit> {
    UNITS
        .iter()
        .find(|(unit_name, _)| *unit_name == name)
        .map(|(_, unit)| *unit)
        .ok_or_else(|| {
            LuaError::runtime(format!(
                "unknown unit '{name}', expected Years, Months, Weeks, Days, Hours, Minutes, Seconds or Milliseconds"
            ))
        })
}

fn start_of(zoned: &Zoned, name: &str) -> LuaResult<Zoned> {
    let date = zoned.date();
    let date = match name {
        "Year" => date.first_of_year(),
        "Month" => date.first_of_month(),
        "Week" => date
            .checked_sub(Span::new().days(i64::from(date.weekday().to_monday_zero_offset())))
            .map_err(error)?,
        "Day" | "Hour" | "Minute" | "Second" => date,
        other => {
            return Err(LuaError::runtime(format!(
                "unknown unit '{other}', expected Year, Month, Week, Day, Hour, Minute or Second"
            )));
        }
    };
    let time = match name {
        "Hour" => Time::new(zoned.hour(), 0, 0, 0),
        "Minute" => Time::new(zoned.hour(), zoned.minute(), 0, 0),
        "Second" => Time::new(zoned.hour(), zoned.minute(), zoned.second(), 0),
        _ => Ok(Time::midnight()),
    }
    .map_err(error)?;
    date.to_datetime(time)
        .to_zoned(zoned.time_zone().clone())
        .map_err(error)
}

fn end_of(zoned: &Zoned, name: &str) -> LuaResult<Zoned> {
    let start = start_of(zoned, name)?;
    let length = match name {
        "Year" => Span::new().years(1),
        "Month" => Span::new().months(1),
        "Week" => Span::new().weeks(1),
        "Day" => Span::new().days(1),
        "Hour" => Span::new().hours(1),
        "Minute" => Span::new().minutes(1),
        _ => Span::new().seconds(1),
    };
    start
        .checked_add(length)
        .and_then(|next| next.checked_sub(SignedDuration::from_millis(1)))
        .map_err(error)
}

fn relative(target: &Zoned, now: &Zoned) -> String {
    let milliseconds = target.timestamp().as_millisecond() - now.timestamp().as_millisecond();
    let seconds = milliseconds.abs() as f64 / 1000.0;
    if seconds < 45.0 {
        return "just now".to_owned();
    }
    let (amount, name) = match seconds {
        seconds if seconds < 45.0 * 60.0 => ((seconds / 60.0).round().max(1.0), "minute"),
        seconds if seconds < 22.0 * 3600.0 => ((seconds / 3600.0).round().max(1.0), "hour"),
        seconds if seconds < 26.0 * 86400.0 => ((seconds / 86400.0).round().max(1.0), "day"),
        seconds if seconds < 320.0 * 86400.0 => {
            ((seconds / (30.44 * 86400.0)).round().max(1.0), "month")
        }
        seconds => ((seconds / (365.25 * 86400.0)).round().max(1.0), "year"),
    };
    let amount = amount as i64;
    let unit = if amount == 1 {
        name.to_owned()
    } else {
        format!("{name}s")
    };
    if milliseconds > 0 {
        format!("in {amount} {unit}")
    } else {
        format!("{amount} {unit} ago")
    }
}

fn other(value: &LuaAnyUserData) -> LuaResult<Zoned> {
    Ok(value.borrow::<DateTime>()?.0.clone())
}

pub fn to_object(zoned: Zoned) -> Object<DateTime> {
    Object(DateTime(zoned))
}

impl LuaUserData for DateTime {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("Year", |_, time| Ok(time.0.year()));
        fields.add_field_method_get("Month", |_, time| Ok(time.0.month()));
        fields.add_field_method_get("Day", |_, time| Ok(time.0.day()));
        fields.add_field_method_get("Hour", |_, time| Ok(time.0.hour()));
        fields.add_field_method_get("Minute", |_, time| Ok(time.0.minute()));
        fields.add_field_method_get("Second", |_, time| Ok(time.0.second()));
        fields.add_field_method_get("Millisecond", |_, time| Ok(time.0.millisecond()));
        fields.add_field_method_get("Weekday", |_, time| {
            Ok(time.0.weekday().to_monday_one_offset())
        });
        fields.add_field_method_get("YearDay", |_, time| Ok(time.0.day_of_year()));
        fields.add_field_method_get("Zone", |_, time| Ok(zone_name(&time.0)));
        fields.add_field_method_get("Offset", |_, time| Ok(time.0.offset().seconds()));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("GetUnix", |_, time, ()| {
            Ok(time.0.timestamp().as_millisecond() as f64 / 1000.0)
        });
        methods.add_method("GetUnixMs", |_, time, ()| {
            Ok(time.0.timestamp().as_millisecond())
        });
        methods.add_method("ToISO", |_, time, ()| {
            Ok(time
                .0
                .timestamp()
                .display_with_offset(time.0.offset())
                .to_string())
        });
        methods.add_method("ToRFC2822", |_, time, ()| {
            rfc2822::to_string(&time.0).map_err(error)
        });
        methods.add_method("Format", |_, time, pattern: String| {
            strtime::format(&pattern, &time.0).map_err(error)
        });
        methods.add_method("ToTable", |lua, time, ()| {
            let table = lua.create_table()?;
            table.set("Year", time.0.year())?;
            table.set("Month", time.0.month())?;
            table.set("Day", time.0.day())?;
            table.set("Hour", time.0.hour())?;
            table.set("Minute", time.0.minute())?;
            table.set("Second", time.0.second())?;
            table.set("Millisecond", time.0.millisecond())?;
            table.set("Weekday", time.0.weekday().to_monday_one_offset())?;
            table.set("YearDay", time.0.day_of_year())?;
            table.set("Zone", zone_name(&time.0))?;
            table.set("Offset", time.0.offset().seconds())?;
            Ok(table)
        });

        methods.add_method("InZone", |_, time, name: String| {
            Ok(to_object(time.0.with_time_zone(zone(&name)?)))
        });
        methods.add_method("ToUTC", |_, time, ()| {
            Ok(to_object(time.0.with_time_zone(TimeZone::UTC)))
        });
        methods.add_method("Add", |_, time, value: LuaValue| {
            Ok(to_object(shift(&time.0, value, 1)?))
        });
        methods.add_method("Subtract", |_, time, value: LuaValue| {
            Ok(to_object(shift(&time.0, value, -1)?))
        });
        methods.add_method("With", |_, time, fields: LuaTable| {
            let mut builder = time.0.with();
            if let Some(year) = fields.get::<Option<i16>>("Year")? {
                builder = builder.year(year);
            }
            if let Some(month) = fields.get::<Option<i8>>("Month")? {
                builder = builder.month(month);
            }
            if let Some(day) = fields.get::<Option<i8>>("Day")? {
                builder = builder.day(day);
            }
            if let Some(hour) = fields.get::<Option<i8>>("Hour")? {
                builder = builder.hour(hour);
            }
            if let Some(minute) = fields.get::<Option<i8>>("Minute")? {
                builder = builder.minute(minute);
            }
            if let Some(second) = fields.get::<Option<i8>>("Second")? {
                builder = builder.second(second);
            }
            if let Some(millisecond) = fields.get::<Option<i16>>("Millisecond")? {
                builder = builder.millisecond(millisecond);
            }
            Ok(to_object(builder.build().map_err(error)?))
        });
        methods.add_method("StartOf", |_, time, name: String| {
            Ok(to_object(start_of(&time.0, &name)?))
        });
        methods.add_method("EndOf", |_, time, name: String| {
            Ok(to_object(end_of(&time.0, &name)?))
        });

        methods.add_method(
            "Since",
            |_, time, (earlier, name): (LuaAnyUserData, Option<String>)| {
                let earlier = other(&earlier)?;
                let Some(name) = name else {
                    return Ok(LuaValue::Integer(
                        time.0.timestamp().as_millisecond() - earlier.timestamp().as_millisecond(),
                    ));
                };
                let unit = unit(&name)?;
                let span = time.0.since((unit, &earlier)).map_err(error)?;
                Ok(LuaValue::Number(
                    span.total((unit, &earlier)).map_err(error)?,
                ))
            },
        );
        methods.add_method("Relative", |_, time, now: Option<LuaAnyUserData>| {
            let now = match now {
                Some(now) => other(&now)?,
                None => Zoned::now(),
            };
            Ok(relative(&time.0, &now))
        });
        methods.add_method("IsBefore", |_, time, value: LuaAnyUserData| {
            Ok(time.0.timestamp() < other(&value)?.timestamp())
        });
        methods.add_method("IsAfter", |_, time, value: LuaAnyUserData| {
            Ok(time.0.timestamp() > other(&value)?.timestamp())
        });

        methods.add_meta_method(LuaMetaMethod::Eq, |_, time, value: LuaAnyUserData| {
            Ok(time.0.timestamp() == other(&value)?.timestamp())
        });
        methods.add_meta_method(LuaMetaMethod::Lt, |_, time, value: LuaAnyUserData| {
            Ok(time.0.timestamp() < other(&value)?.timestamp())
        });
        methods.add_meta_method(LuaMetaMethod::Le, |_, time, value: LuaAnyUserData| {
            Ok(time.0.timestamp() <= other(&value)?.timestamp())
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, time, ()| {
            Ok(time.0.to_string())
        });
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "Now",
        lua.create_function(|_, name: Option<String>| {
            Ok(to_object(Zoned::now().with_time_zone(zone_option(name)?)))
        })?,
    )?;
    module.set(
        "Unix",
        lua.create_function(|_, ()| Ok(Timestamp::now().as_millisecond() as f64 / 1000.0))?,
    )?;
    module.set(
        "UnixMs",
        lua.create_function(|_, ()| Ok(Timestamp::now().as_millisecond()))?,
    )?;
    module.set(
        "FromUnix",
        lua.create_function(|_, (seconds, name): (f64, Option<String>)| {
            let timestamp =
                Timestamp::from_millisecond((seconds * 1000.0).round() as i64).map_err(error)?;
            Ok(to_object(timestamp.to_zoned(zone_option(name)?)))
        })?,
    )?;
    module.set(
        "FromUnixMs",
        lua.create_function(|_, (milliseconds, name): (i64, Option<String>)| {
            let timestamp = Timestamp::from_millisecond(milliseconds).map_err(error)?;
            Ok(to_object(timestamp.to_zoned(zone_option(name)?)))
        })?,
    )?;
    module.set(
        "Date",
        lua.create_function(|_, (fields, name): (LuaTable, Option<String>)| {
            let date = Date::new(
                fields.get("Year")?,
                fields.get::<Option<i8>>("Month")?.unwrap_or(1),
                fields.get::<Option<i8>>("Day")?.unwrap_or(1),
            )
            .map_err(error)?;
            let time = Time::new(
                fields.get::<Option<i8>>("Hour")?.unwrap_or(0),
                fields.get::<Option<i8>>("Minute")?.unwrap_or(0),
                fields.get::<Option<i8>>("Second")?.unwrap_or(0),
                i32::from(fields.get::<Option<i16>>("Millisecond")?.unwrap_or(0)) * 1_000_000,
            )
            .map_err(error)?;
            Ok(to_object(
                date.to_datetime(time)
                    .to_zoned(zone_option(name)?)
                    .map_err(error)?,
            ))
        })?,
    )?;
    module.set(
        "Parse",
        lua.create_function(|lua, (text, options): (String, Options)| {
            match parse(&text, &options)? {
                Ok(zoned) => to_object(zoned).into_lua_multi(lua),
                Err(message) => (LuaNil, message).into_lua_multi(lua),
            }
        })?,
    )?;
    module.set(
        "IsDateTime",
        lua.create_function(|_, value: LuaValue| {
            Ok(matches!(value, LuaValue::UserData(userdata) if userdata.is::<DateTime>()))
        })?,
    )?;

    module.set(
        "IsValidZone",
        lua.create_function(|_, name: String| Ok(zone(&name).is_ok()))?,
    )?;
    module.set(
        "GetSystemZone",
        lua.create_function(|_, ()| {
            Ok(TimeZone::system().iana_name().unwrap_or("UTC").to_owned())
        })?,
    )?;
    module.set(
        "GetZones",
        lua.create_function(|lua, ()| {
            let mut names: Vec<String> = jiff::tz::db()
                .available()
                .map(|name| name.to_string())
                .collect();
            names.sort();
            lua.create_sequence_from(names)
        })?,
    )?;
    Ok(())
}
