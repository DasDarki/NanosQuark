use std::ops::Range;

use aho_corasick::{AhoCorasick, Anchored, Input, MatchKind, StartKind};
use mlua::prelude::*;
use quark_core::{Options, create_userdata};

const MAX_WORDS: usize = 100_000;

#[derive(Clone, Copy)]
struct Settings {
    whole_words: bool,
    leetspeak: bool,
    repeats: bool,
}

pub struct WordFilter {
    settings: Settings,
    words: Vec<String>,
    searcher: Option<AhoCorasick>,
}

struct Normalized {
    text: String,
    ranges: Vec<Range<usize>>,
}

fn leet(character: char) -> char {
    match character {
        '4' | '@' => 'a',
        '8' => 'b',
        '3' => 'e',
        '6' | '9' => 'g',
        '1' => 'i',
        '0' => 'o',
        '5' | '$' => 's',
        '7' => 't',
        '2' => 'z',
        other => other,
    }
}

fn normalize(text: &str, settings: Settings) -> Normalized {
    let mut normalized = Normalized {
        text: String::with_capacity(text.len()),
        ranges: Vec::with_capacity(text.len()),
    };
    let mut push = |character: char, range: Range<usize>| {
        let last = normalized.text.as_bytes().last().copied();
        if character == ' ' && (last.is_none() || last == Some(b' ')) {
            return;
        }
        if settings.repeats && character != ' ' && last == Some(character as u8) {
            if let Some(previous) = normalized.ranges.last_mut() {
                previous.end = range.end;
            }
            return;
        }
        normalized.text.push(character);
        normalized.ranges.push(range);
    };

    for (offset, character) in text.char_indices() {
        let range = offset..offset + character.len_utf8();
        let replacement = deunicode::deunicode_char(character).unwrap_or(" ");
        for converted in replacement.chars() {
            let converted = converted.to_ascii_lowercase();
            let converted = if settings.leetspeak { leet(converted) } else { converted };
            if converted.is_ascii_alphanumeric() {
                push(converted, range.clone());
            } else {
                push(' ', range.clone());
            }
        }
    }
    normalized
}

struct Found {
    word: usize,
    range: Range<usize>,
}

impl WordFilter {
    fn new(words: Vec<String>, settings: Settings) -> LuaResult<Self> {
        let mut filter = WordFilter {
            settings,
            words: Vec::new(),
            searcher: None,
        };
        filter.set_words(words)?;
        Ok(filter)
    }

    fn set_words(&mut self, words: Vec<String>) -> LuaResult<()> {
        if words.len() > MAX_WORDS {
            return Err(LuaError::runtime(format!("a word filter holds at most {MAX_WORDS} words")));
        }
        let patterns: Vec<String> = words
            .iter()
            .map(|word| normalize(word, self.settings).text.trim().to_owned())
            .collect();
        let searcher = if patterns.iter().any(|pattern| !pattern.is_empty()) {
            let usable: Vec<&str> = patterns
                .iter()
                .map(|pattern| if pattern.is_empty() { "\u{0}" } else { pattern.as_str() })
                .collect();
            Some(
                AhoCorasick::builder()
                    .match_kind(MatchKind::LeftmostLongest)
                    .start_kind(StartKind::Both)
                    .build(usable)
                    .map_err(|error| LuaError::runtime(format!("failed to build the word filter: {error}")))?,
            )
        } else {
            None
        };
        self.words = words;
        self.searcher = searcher;
        Ok(())
    }

    fn find(&self, text: &str) -> Vec<Found> {
        let Some(searcher) = &self.searcher else {
            return Vec::new();
        };
        let normalized = normalize(text, self.settings);
        let bytes = normalized.text.as_bytes();
        let mut found = Vec::new();

        if self.settings.whole_words {
            let mut start = 0;
            while start < bytes.len() {
                let input = Input::new(&normalized.text).range(start..).anchored(Anchored::Yes);
                let matched = searcher.find(input).filter(|found_match| {
                    found_match.end() == bytes.len() || bytes[found_match.end()] == b' '
                });
                let next = match matched {
                    Some(found_match) => {
                        found.push(Found {
                            word: found_match.pattern().as_usize(),
                            range: normalized.ranges[start].start..normalized.ranges[found_match.end() - 1].end,
                        });
                        found_match.end()
                    }
                    None => start,
                };
                start = match bytes[next..].iter().position(|&byte| byte == b' ') {
                    Some(space) => next + space + 1,
                    None => bytes.len(),
                };
            }
        } else {
            for found_match in searcher.find_iter(&normalized.text) {
                found.push(Found {
                    word: found_match.pattern().as_usize(),
                    range: normalized.ranges[found_match.start()].start..normalized.ranges[found_match.end() - 1].end,
                });
            }
        }
        found
    }
}

fn read_words(words: LuaTable) -> LuaResult<Vec<String>> {
    words.sequence_values::<String>().collect()
}

fn censor(text: &str, found: &[Found], replacement: &str) -> String {
    let single = replacement.chars().count() == 1;
    let mut output = String::with_capacity(text.len());
    let mut last = 0;
    for entry in found {
        if entry.range.start < last {
            continue;
        }
        output.push_str(&text[last..entry.range.start]);
        let original = &text[entry.range.clone()];
        if single {
            for character in original.chars() {
                output.push_str(if character.is_whitespace() { " " } else { replacement });
            }
        } else {
            output.push_str(replacement);
        }
        last = entry.range.end;
    }
    output.push_str(&text[last..]);
    output
}

impl LuaUserData for WordFilter {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("Contains", |_, filter, text: String| Ok(!filter.find(&text).is_empty()));
        methods.add_method("Find", |lua, filter, text: String| {
            let result = lua.create_table()?;
            for entry in filter.find(&text) {
                let item = lua.create_table()?;
                item.set("Word", filter.words[entry.word].as_str())?;
                item.set("Text", &text[entry.range.clone()])?;
                item.set("Start", entry.range.start + 1)?;
                item.set("End", entry.range.end)?;
                result.raw_push(item)?;
            }
            Ok(result)
        });
        methods.add_method("Censor", |_, filter, (text, replacement): (String, Option<String>)| {
            let found = filter.find(&text);
            Ok((censor(&text, &found, replacement.as_deref().unwrap_or("*")), found.len()))
        });
        methods.add_method_mut("Add", |_, filter, words: LuaTable| {
            let mut all = filter.words.clone();
            for word in read_words(words)? {
                if !all.contains(&word) {
                    all.push(word);
                }
            }
            filter.set_words(all)
        });
        methods.add_method_mut("Remove", |_, filter, words: LuaTable| {
            let removed = read_words(words)?;
            let remaining = filter.words.iter().filter(|word| !removed.contains(word)).cloned().collect();
            filter.set_words(remaining)
        });
        methods.add_method("GetWords", |lua, filter, ()| lua.create_sequence_from(filter.words.iter().map(String::as_str)));
    }
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "WordFilter",
        lua.create_function(|lua, (words, options): (LuaTable, Options)| {
            let settings = Settings {
                whole_words: options.get_or("WholeWords", true)?,
                leetspeak: options.get_or("Leetspeak", true)?,
                repeats: options.get_or("Repeats", true)?,
            };
            create_userdata(lua, WordFilter::new(read_words(words)?, settings)?)
        })?,
    )
}
