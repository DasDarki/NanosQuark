use mlua::prelude::*;
use quark_core::Options;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

const DEFAULT_THRESHOLD: f64 = 0.5;

fn graphemes(text: &str) -> Vec<&str> {
    text.graphemes(true).collect()
}

fn grapheme_range(length: usize, start: i64, end: Option<i64>) -> (usize, usize) {
    let length = length as i64;
    let resolve = |position: i64| {
        if position < 0 {
            length + position + 1
        } else {
            position
        }
    };
    let start = resolve(start).max(1);
    let end = resolve(end.unwrap_or(-1)).min(length);
    if start > end {
        return (0, 0);
    }
    (start as usize - 1, end as usize)
}

fn similarity(first: &str, second: &str, algorithm: &str) -> LuaResult<f64> {
    Ok(match algorithm {
        "JaroWinkler" => strsim::jaro_winkler(first, second),
        "Jaro" => strsim::jaro(first, second),
        "Levenshtein" => strsim::normalized_levenshtein(first, second),
        "DamerauLevenshtein" => strsim::normalized_damerau_levenshtein(first, second),
        "SorensenDice" => strsim::sorensen_dice(first, second),
        other => {
            return Err(LuaError::runtime(format!(
                "unknown algorithm '{other}', expected JaroWinkler, Jaro, Levenshtein, DamerauLevenshtein or SorensenDice"
            )));
        }
    })
}

fn fuzzy_score(query: &str, candidate: &str) -> f64 {
    if query.is_empty() {
        return 0.0;
    }
    if candidate == query {
        return 1.0;
    }
    let length_ratio = query.chars().count() as f64 / candidate.chars().count().max(1) as f64;
    if candidate.starts_with(query) {
        return 0.9 + 0.09 * length_ratio;
    }
    if candidate.contains(query) {
        return 0.8 + 0.09 * length_ratio;
    }
    let jaro = strsim::jaro_winkler(query, candidate);
    let edits = strsim::normalized_damerau_levenshtein(query, candidate);
    0.79 * jaro.max(edits)
}

fn fuzzy_find(
    lua: &Lua,
    query: &str,
    candidates: &LuaTable,
    options: &Options,
) -> LuaResult<LuaTable> {
    let threshold: f64 = options.get_or("Threshold", DEFAULT_THRESHOLD)?;
    let limit: Option<usize> = options.get("Limit")?;
    let key: Option<String> = options.get("Key")?;
    let case_sensitive: bool = options.get_or("CaseSensitive", false)?;
    let prepare = |text: &str| {
        if case_sensitive {
            text.to_owned()
        } else {
            text.to_lowercase()
        }
    };
    let query = prepare(query);

    let mut results = Vec::new();
    for (index, item) in candidates.sequence_values::<LuaValue>().enumerate() {
        let item = item?;
        let text = match (&item, &key) {
            (LuaValue::Table(table), Some(key)) => table.get::<Option<String>>(key.as_str())?,
            (LuaValue::String(text), _) => Some(text.to_str()?.to_owned()),
            (LuaValue::Table(_), None) => {
                return Err(LuaError::runtime(
                    "tables can only be searched with the Key option",
                ));
            }
            _ => None,
        };
        let Some(text) = text else {
            continue;
        };
        let score = fuzzy_score(&query, &prepare(&text));
        if score >= threshold {
            results.push((score, index + 1, item));
        }
    }

    results.sort_by(|left, right| right.0.total_cmp(&left.0).then(left.1.cmp(&right.1)));
    if let Some(limit) = limit {
        results.truncate(limit);
    }

    let table = lua.create_table_with_capacity(results.len(), 0)?;
    for (score, index, item) in results {
        let entry = lua.create_table()?;
        entry.set("Item", item)?;
        entry.set("Score", score)?;
        entry.set("Index", index)?;
        table.raw_push(entry)?;
    }
    Ok(table)
}

fn slugify(text: &str, separator: &str) -> String {
    let ascii = deunicode::deunicode(text).to_lowercase();
    let mut slug = String::with_capacity(ascii.len());
    let mut pending = false;
    for character in ascii.chars() {
        if character.is_ascii_alphanumeric() {
            if pending && !slug.is_empty() {
                slug.push_str(separator);
            }
            pending = false;
            slug.push(character);
        } else {
            pending = true;
        }
    }
    slug
}

fn display_width(text: &str) -> usize {
    text.graphemes(true).count()
}

fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        let mut line_width = 0;
        for word in paragraph.split_whitespace() {
            let mut word = word.to_owned();
            let mut word_width = display_width(&word);
            while word_width > width {
                if line_width > 0 {
                    lines.push(std::mem::take(&mut line));
                    line_width = 0;
                }
                let parts = graphemes(&word);
                lines.push(parts[..width].concat());
                word = parts[width..].concat();
                word_width -= width;
            }
            if word.is_empty() {
                continue;
            }
            if line_width > 0 && line_width + 1 + word_width > width {
                lines.push(std::mem::take(&mut line));
                line_width = 0;
            }
            if line_width > 0 {
                line.push(' ');
                line_width += 1;
            }
            line.push_str(&word);
            line_width += word_width;
        }
        lines.push(line);
    }
    lines
}

fn split(lua: &Lua, text: &str, separator: &str, options: &Options) -> LuaResult<LuaTable> {
    let trim: bool = options.get_or("Trim", false)?;
    let remove_empty: bool = options.get_or("RemoveEmpty", false)?;
    let limit: Option<usize> = options.get("Limit")?;

    let parts: Vec<&str> = match (separator.is_empty(), limit) {
        (true, _) => graphemes(text),
        (false, Some(limit)) => text.splitn(limit.max(1), separator).collect(),
        (false, None) => text.split(separator).collect(),
    };
    let result = lua.create_table_with_capacity(parts.len(), 0)?;
    for part in parts {
        let part = if trim { part.trim() } else { part };
        if remove_empty && part.is_empty() {
            continue;
        }
        result.raw_push(part)?;
    }
    Ok(result)
}

fn pad(text: &str, width: usize, options: &Options) -> LuaResult<String> {
    let fill: String = options.get_or("Char", " ".to_owned())?;
    if display_width(&fill) != 1 {
        return Err(LuaError::runtime(
            "the padding Char has to be a single character",
        ));
    }
    let align: String = options.get_or("Align", "Left".to_owned())?;
    let missing = width.saturating_sub(display_width(text));
    let (left, right) = match align.as_str() {
        "Left" => (0, missing),
        "Right" => (missing, 0),
        "Center" => (missing / 2, missing - missing / 2),
        other => {
            return Err(LuaError::runtime(format!(
                "unknown alignment '{other}', expected Left, Right or Center"
            )));
        }
    };
    Ok(format!("{}{text}{}", fill.repeat(left), fill.repeat(right)))
}

fn capitalize(text: &str) -> String {
    let mut words = String::with_capacity(text.len());
    for word in text.split_word_bounds() {
        let mut characters = word.chars();
        match characters.next() {
            Some(first) if first.is_alphabetic() => {
                words.extend(first.to_uppercase());
                words.push_str(&characters.as_str().to_lowercase());
            }
            _ => words.push_str(word),
        }
    }
    words
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "Upper",
        lua.create_function(|_, text: String| Ok(text.to_uppercase()))?,
    )?;
    module.set(
        "Lower",
        lua.create_function(|_, text: String| Ok(text.to_lowercase()))?,
    )?;
    module.set(
        "Capitalize",
        lua.create_function(|_, text: String| Ok(capitalize(&text)))?,
    )?;
    module.set(
        "Normalize",
        lua.create_function(|_, (text, form): (String, Option<String>)| {
            Ok(match form.as_deref().unwrap_or("NFC") {
                "NFC" => text.nfc().collect::<String>(),
                "NFD" => text.nfd().collect(),
                "NFKC" => text.nfkc().collect(),
                "NFKD" => text.nfkd().collect(),
                other => {
                    return Err(LuaError::runtime(format!(
                        "unknown normalization form '{other}', expected NFC, NFD, NFKC or NFKD"
                    )));
                }
            })
        })?,
    )?;
    module.set(
        "ToAscii",
        lua.create_function(|_, text: String| Ok(deunicode::deunicode(&text)))?,
    )?;
    module.set(
        "Slugify",
        lua.create_function(|_, (text, separator): (String, Option<String>)| {
            Ok(slugify(&text, separator.as_deref().unwrap_or("-")))
        })?,
    )?;
    module.set(
        "IsValid",
        lua.create_function(
            |_, text: LuaString| Ok(std::str::from_utf8(&text.as_bytes()).is_ok()),
        )?,
    )?;

    module.set(
        "Length",
        lua.create_function(|_, text: String| Ok(display_width(&text)))?,
    )?;
    module.set(
        "Graphemes",
        lua.create_function(|lua, text: String| lua.create_sequence_from(graphemes(&text)))?,
    )?;
    module.set(
        "Words",
        lua.create_function(|lua, text: String| lua.create_sequence_from(text.unicode_words()))?,
    )?;
    module.set(
        "Sub",
        lua.create_function(|_, (text, start, end): (String, i64, Option<i64>)| {
            let parts = graphemes(&text);
            let (start, end) = grapheme_range(parts.len(), start, end);
            Ok(parts[start..end].concat())
        })?,
    )?;
    module.set(
        "Reverse",
        lua.create_function(|_, text: String| Ok(text.graphemes(true).rev().collect::<String>()))?,
    )?;
    module.set(
        "Truncate",
        lua.create_function(
            |_, (text, length, suffix): (String, usize, Option<String>)| {
                let parts = graphemes(&text);
                if parts.len() <= length {
                    return Ok(text);
                }
                let suffix = suffix.unwrap_or_else(|| "...".to_owned());
                let suffix_width = display_width(&suffix);
                if suffix_width >= length {
                    return Ok(parts[..length].concat());
                }
                let keep = length - suffix_width;
                Ok(format!("{}{suffix}", parts[..keep].concat().trim_end()))
            },
        )?,
    )?;
    module.set(
        "Trim",
        lua.create_function(|_, text: String| Ok(text.trim().to_owned()))?,
    )?;
    module.set(
        "Split",
        lua.create_function(
            |lua, (text, separator, options): (String, String, Options)| {
                split(lua, &text, &separator, &options)
            },
        )?,
    )?;
    module.set(
        "Pad",
        lua.create_function(|_, (text, width, options): (String, usize, Options)| {
            pad(&text, width, &options)
        })?,
    )?;
    module.set(
        "Wrap",
        lua.create_function(|lua, (text, width): (String, usize)| {
            if width == 0 {
                return Err(LuaError::runtime("the width has to be at least 1"));
            }
            lua.create_sequence_from(wrap(&text, width))
        })?,
    )?;

    module.set(
        "Levenshtein",
        lua.create_function(|_, (first, second): (String, String)| {
            Ok(strsim::levenshtein(&first, &second))
        })?,
    )?;
    module.set(
        "DamerauLevenshtein",
        lua.create_function(|_, (first, second): (String, String)| {
            Ok(strsim::damerau_levenshtein(&first, &second))
        })?,
    )?;
    module.set(
        "Similarity",
        lua.create_function(
            |_, (first, second, algorithm): (String, String, Option<String>)| {
                similarity(
                    &first,
                    &second,
                    algorithm.as_deref().unwrap_or("JaroWinkler"),
                )
            },
        )?,
    )?;
    module.set(
        "FuzzyFind",
        lua.create_function(
            |lua, (query, candidates, options): (String, LuaTable, Options)| {
                fuzzy_find(lua, &query, &candidates, &options)
            },
        )?,
    )?;
    module.set(
        "Closest",
        lua.create_function(
            |lua, (query, candidates, options): (String, LuaTable, Options)| {
                let results = fuzzy_find(lua, &query, &candidates, &options)?;
                match results.raw_get::<Option<LuaTable>>(1)? {
                    Some(best) => (best.get::<LuaValue>("Item")?, best.get::<f64>("Score")?)
                        .into_lua_multi(lua),
                    None => LuaNil.into_lua_multi(lua),
                }
            },
        )?,
    )?;

    crate::filter::register(lua, &module)?;
    Ok(module)
}
