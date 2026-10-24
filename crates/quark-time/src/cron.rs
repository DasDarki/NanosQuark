use jiff::civil::DateTime as CivilDateTime;
use jiff::tz::TimeZone;
use jiff::{Span, Zoned};
use mlua::prelude::*;
use quark_core::{Options, create_userdata};

use crate::datetime::{DateTime, to_object, zone_option};

const MAX_RESULTS: usize = 1000;
const SEARCH_YEARS: i16 = 8;
const MONTHS: &[&str] = &[
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];
const WEEKDAYS: &[&str] = &["sun", "mon", "tue", "wed", "thu", "fri", "sat"];

#[derive(Clone)]
pub struct Cron {
    expression: String,
    zone: TimeZone,
    seconds: u64,
    minutes: u64,
    hours: u64,
    days: u64,
    last_day: bool,
    months: u64,
    weekdays: u64,
    days_restricted: bool,
    weekdays_restricted: bool,
}

fn value(text: &str, names: &[&str], offset: u32) -> Result<u32, String> {
    if let Some(index) = names
        .iter()
        .position(|name| name.eq_ignore_ascii_case(text))
    {
        return Ok(index as u32 + offset);
    }
    text.parse()
        .map_err(|_| format!("'{text}' is not a number"))
}

fn field(
    text: &str,
    minimum: u32,
    maximum: u32,
    names: &[&str],
    name_offset: u32,
    label: &str,
) -> Result<u64, String> {
    let mut mask = 0u64;
    for part in text.split(',') {
        let (range, step) = match part.split_once('/') {
            Some((range, step)) => {
                let step: u32 = step
                    .parse()
                    .map_err(|_| format!("invalid step '{step}' in the {label} field"))?;
                if step == 0 {
                    return Err(format!(
                        "the step in the {label} field has to be at least 1"
                    ));
                }
                (range, step)
            }
            None => (part, 1),
        };
        let (start, end) = match range {
            "*" | "?" => (minimum, maximum),
            range => match range.split_once('-') {
                Some((start, end)) => (
                    value(start, names, name_offset)?,
                    value(end, names, name_offset)?,
                ),
                None => {
                    let start = value(range, names, name_offset)?;
                    (start, if part.contains('/') { maximum } else { start })
                }
            },
        };
        if start < minimum || end > maximum || start > end {
            return Err(format!(
                "'{part}' is outside of {minimum}-{maximum} in the {label} field"
            ));
        }
        let mut current = start;
        while current <= end {
            mask |= 1 << current;
            current += step;
        }
    }
    Ok(mask)
}

impl Cron {
    pub fn parse(expression: &str, zone: TimeZone) -> Result<Self, String> {
        let expanded = match expression.trim().to_ascii_lowercase().as_str() {
            "@yearly" | "@annually" => "0 0 0 1 1 *".to_owned(),
            "@monthly" => "0 0 0 1 * *".to_owned(),
            "@weekly" => "0 0 0 * * 0".to_owned(),
            "@daily" | "@midnight" => "0 0 0 * * *".to_owned(),
            "@hourly" => "0 0 * * * *".to_owned(),
            other if other.starts_with('@') => return Err(format!("unknown cron macro '{other}'")),
            _ => expression.trim().to_owned(),
        };
        let parts: Vec<&str> = expanded.split_whitespace().collect();
        let parts: Vec<&str> = match parts.len() {
            5 => std::iter::once("0").chain(parts).collect(),
            6 => parts,
            count => return Err(format!("a cron expression has 5 or 6 fields, got {count}")),
        };

        let (days_text, last_day) = match parts[3] {
            "L" | "l" => ("*", true),
            other => (other, false),
        };
        let mut weekdays = field(parts[5], 0, 7, WEEKDAYS, 0, "weekday")?;
        if weekdays & (1 << 7) != 0 {
            weekdays = (weekdays & !(1 << 7)) | 1;
        }

        Ok(Cron {
            expression: expression.trim().to_owned(),
            zone,
            seconds: field(parts[0], 0, 59, &[], 0, "second")?,
            minutes: field(parts[1], 0, 59, &[], 0, "minute")?,
            hours: field(parts[2], 0, 23, &[], 0, "hour")?,
            days: if last_day {
                0
            } else {
                field(days_text, 1, 31, &[], 0, "day")?
            },
            last_day,
            months: field(parts[4], 1, 12, MONTHS, 1, "month")?,
            weekdays,
            days_restricted: last_day || !matches!(parts[3], "*" | "?"),
            weekdays_restricted: !matches!(parts[5], "*" | "?"),
        })
    }

    fn day_matches(&self, datetime: &CivilDateTime) -> bool {
        let date = datetime.date();
        let day = date.day() as u32;
        let by_day =
            self.days & (1 << day) != 0 || (self.last_day && date.day() == date.days_in_month());
        let by_weekday = self.weekdays & (1 << date.weekday().to_sunday_zero_offset()) != 0;
        match (self.days_restricted, self.weekdays_restricted) {
            (true, true) => by_day || by_weekday,
            (true, false) => by_day,
            (false, true) => by_weekday,
            (false, false) => true,
        }
    }

    pub fn matches(&self, zoned: &Zoned) -> bool {
        let datetime = zoned.with_time_zone(self.zone.clone()).datetime();
        self.months & (1 << datetime.month()) != 0
            && self.day_matches(&datetime)
            && self.hours & (1 << datetime.hour()) != 0
            && self.minutes & (1 << datetime.minute()) != 0
            && self.seconds & (1 << datetime.second()) != 0
    }

    pub fn next(&self, after: &Zoned) -> Option<Zoned> {
        let start = after.with_time_zone(self.zone.clone()).datetime();
        let mut current = start
            .with()
            .subsec_nanosecond(0)
            .build()
            .ok()?
            .checked_add(Span::new().seconds(1))
            .ok()?;
        let limit = start.year().saturating_add(SEARCH_YEARS);

        while current.year() <= limit {
            if self.months & (1 << current.month()) == 0 {
                current = current
                    .date()
                    .first_of_month()
                    .checked_add(Span::new().months(1))
                    .ok()?
                    .to_datetime(jiff::civil::Time::midnight());
                continue;
            }
            if !self.day_matches(&current) {
                current = current
                    .date()
                    .tomorrow()
                    .ok()?
                    .to_datetime(jiff::civil::Time::midnight());
                continue;
            }
            if self.hours & (1 << current.hour()) == 0 {
                current = current
                    .with()
                    .minute(0)
                    .second(0)
                    .build()
                    .ok()?
                    .checked_add(Span::new().hours(1))
                    .ok()?;
                continue;
            }
            if self.minutes & (1 << current.minute()) == 0 {
                current = current
                    .with()
                    .second(0)
                    .build()
                    .ok()?
                    .checked_add(Span::new().minutes(1))
                    .ok()?;
                continue;
            }
            if self.seconds & (1 << current.second()) == 0 {
                current = current.checked_add(Span::new().seconds(1)).ok()?;
                continue;
            }
            let candidate = current.to_zoned(self.zone.clone()).ok()?;
            if candidate.timestamp() > after.timestamp() {
                return Some(candidate);
            }
            if let Ok(repeated) = self.zone.to_ambiguous_zoned(current).later()
                && repeated.timestamp() > after.timestamp()
            {
                return Some(repeated);
            }
            current = current.checked_add(Span::new().seconds(1)).ok()?;
        }
        None
    }

    pub fn expression(&self) -> &str {
        &self.expression
    }
}

fn after(value: Option<LuaAnyUserData>) -> LuaResult<Zoned> {
    match value {
        Some(value) => Ok(value.borrow::<DateTime>()?.0.clone()),
        None => Ok(Zoned::now()),
    }
}

impl LuaUserData for Cron {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Next", |_, cron, start: Option<LuaAnyUserData>| {
            Ok(cron.next(&after(start)?).map(to_object))
        });
        methods.add_method(
            "NextMany",
            |lua, cron, (count, start): (usize, Option<LuaAnyUserData>)| {
                if count > MAX_RESULTS {
                    return Err(LuaError::runtime(format!(
                        "at most {MAX_RESULTS} times can be listed at once"
                    )));
                }
                let result = lua.create_table()?;
                let mut current = after(start)?;
                for _ in 0..count {
                    let Some(next) = cron.next(&current) else {
                        break;
                    };
                    result.raw_push(to_object(next.clone()))?;
                    current = next;
                }
                Ok(result)
            },
        );
        methods.add_method("Matches", |_, cron, time: LuaAnyUserData| {
            Ok(cron.matches(&time.borrow::<DateTime>()?.0))
        });
        methods.add_method("GetExpression", |_, cron, ()| Ok(cron.expression.clone()));
        methods.add_method("GetZone", |_, cron, ()| {
            Ok(cron.zone.iana_name().unwrap_or("UTC").to_owned())
        });
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "Cron",
        lua.create_function(|lua, (expression, options): (String, Options)| {
            match Cron::parse(&expression, zone_option(options.get("Zone")?)?) {
                Ok(cron) => create_userdata(lua, cron)?.into_lua_multi(lua),
                Err(message) => (LuaNil, message).into_lua_multi(lua),
            }
        })?,
    )
}
