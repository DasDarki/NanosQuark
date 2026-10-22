use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::thread;

use jiff::Zoned;
use mlua::prelude::*;
use quark_core::{Dispatcher, Object, Options, Resource, package_environment};
use serde_json::{Map, Value};

const DEFAULT_MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
const DEFAULT_MAX_FILES: usize = 5;
const LEVELS: &[&str] = &["Debug", "Info", "Warn", "Error"];

#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Level {
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Clone, Copy, PartialEq)]
enum Format {
    Text,
    Json,
}

impl Level {
    fn parse(name: &str) -> LuaResult<Self> {
        match name {
            "Debug" => Ok(Level::Debug),
            "Info" => Ok(Level::Info),
            "Warn" => Ok(Level::Warn),
            "Error" => Ok(Level::Error),
            other => Err(LuaError::runtime(format!(
                "unknown log level '{other}', expected Debug, Info, Warn or Error"
            ))),
        }
    }

    fn name(self) -> &'static str {
        LEVELS[self as usize]
    }
}

enum Command {
    Line(String),
    Flush(Sender<()>),
}

struct FileSink {
    sender: Mutex<Option<Sender<Command>>>,
}

impl Resource for FileSink {
    fn close(&self) {
        if let Ok(mut sender) = self.sender.lock() {
            sender.take();
        }
    }
}

impl FileSink {
    fn write(&self, line: String) {
        if let Ok(sender) = self.sender.lock()
            && let Some(sender) = sender.as_ref()
        {
            let _ = sender.send(Command::Line(line));
        }
    }

    fn flush(&self) {
        let (done, wait) = mpsc::channel();
        let sent = match self.sender.lock() {
            Ok(sender) => sender
                .as_ref()
                .is_some_and(|sender| sender.send(Command::Flush(done)).is_ok()),
            Err(_) => false,
        };
        if sent {
            let _ = wait.recv();
        }
    }
}

struct Writer {
    path: PathBuf,
    max_size: u64,
    max_files: usize,
    file: Option<BufWriter<File>>,
    size: u64,
}

impl Writer {
    fn open(&mut self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        self.size = file.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        self.file = Some(BufWriter::new(file));
        Ok(())
    }

    fn rotated(&self, index: usize) -> PathBuf {
        let mut name = self.path.as_os_str().to_owned();
        name.push(format!(".{index}"));
        PathBuf::from(name)
    }

    fn rotate(&mut self) -> std::io::Result<()> {
        if let Some(mut file) = self.file.take() {
            file.flush()?;
        }
        if self.max_files == 0 {
            fs::remove_file(&self.path)?;
        } else {
            let _ = fs::remove_file(self.rotated(self.max_files));
            for index in (1..self.max_files).rev() {
                let from = self.rotated(index);
                if from.exists() {
                    fs::rename(&from, self.rotated(index + 1))?;
                }
            }
            fs::rename(&self.path, self.rotated(1))?;
        }
        self.open()
    }

    fn write(&mut self, line: &str) -> std::io::Result<()> {
        if self.file.is_none() {
            self.open()?;
        }
        let length = line.len() as u64 + 1;
        if self.size > 0 && self.size + length > self.max_size {
            self.rotate()?;
        }
        if let Some(file) = self.file.as_mut() {
            file.write_all(line.as_bytes())?;
            file.write_all(b"\n")?;
            self.size += length;
        }
        Ok(())
    }

    fn run(mut self, receiver: Receiver<Command>) {
        let mut reported = false;
        while let Ok(command) = receiver.recv() {
            let mut pending = vec![command];
            while let Ok(command) = receiver.try_recv() {
                pending.push(command);
            }
            for command in pending {
                match command {
                    Command::Line(line) => {
                        if let Err(error) = self.write(&line)
                            && !reported
                        {
                            eprintln!(
                                "[Quark] failed to write the log file {}: {error}",
                                self.path.display()
                            );
                            reported = true;
                        }
                    }
                    Command::Flush(done) => {
                        if let Some(file) = self.file.as_mut() {
                            let _ = file.flush();
                        }
                        let _ = done.send(());
                    }
                }
            }
            if let Some(file) = self.file.as_mut() {
                let _ = file.flush();
            }
        }
    }
}

fn sinks() -> &'static Mutex<HashMap<PathBuf, Weak<FileSink>>> {
    static SINKS: OnceLock<Mutex<HashMap<PathBuf, Weak<FileSink>>>> = OnceLock::new();
    SINKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn open_sink(path: &Path, max_size: u64, max_files: usize) -> LuaResult<Arc<FileSink>> {
    let key = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let mut sinks = sinks()
        .lock()
        .map_err(|_| LuaError::runtime("the log file registry is poisoned"))?;
    if let Some(sink) = sinks.get(&key).and_then(Weak::upgrade)
        && sink.sender.lock().is_ok_and(|sender| sender.is_some())
    {
        return Ok(sink);
    }

    let (sender, receiver) = mpsc::channel();
    let writer = Writer {
        path: key.clone(),
        max_size,
        max_files,
        file: None,
        size: 0,
    };
    thread::Builder::new()
        .name("quark-log".to_owned())
        .spawn(move || writer.run(receiver))
        .map_err(|error| LuaError::runtime(format!("failed to start the log writer: {error}")))?;

    let sink = Arc::new(FileSink {
        sender: Mutex::new(Some(sender)),
    });
    sinks.retain(|_, sink| sink.strong_count() > 0);
    sinks.insert(key, Arc::downgrade(&sink));
    Ok(sink)
}

struct Shared {
    name: Option<String>,
    format: Format,
    console: bool,
    file: Option<Arc<FileSink>>,
}

pub struct Logger {
    shared: Arc<Shared>,
    level: Arc<Mutex<Level>>,
    fields: Map<String, Value>,
}

fn to_json(lua: &Lua, value: LuaValue) -> LuaResult<Value> {
    Ok(match value {
        LuaNil => Value::Null,
        LuaValue::Boolean(value) => Value::Bool(value),
        LuaValue::Integer(value) => Value::from(value),
        LuaValue::Number(value) => serde_json::Number::from_f64(value)
            .map(Value::Number)
            .unwrap_or(Value::Null),
        LuaValue::String(value) => Value::String(value.to_string_lossy()),
        value @ LuaValue::Table(_) => lua
            .from_value_with(
                value.clone(),
                LuaDeserializeOptions::new()
                    .deny_unsupported_types(false)
                    .deny_recursive_tables(false),
            )
            .unwrap_or_else(|_| Value::String(value.to_string().unwrap_or_default())),
        other => Value::String(other.to_string()?),
    })
}

fn read_fields(
    lua: &Lua,
    table: Option<LuaTable>,
    target: &mut Map<String, Value>,
) -> LuaResult<()> {
    if let Some(table) = table {
        for pair in table.pairs::<LuaValue, LuaValue>() {
            let (key, value) = pair?;
            target.insert(key.to_string()?, to_json(lua, value)?);
        }
    }
    Ok(())
}

fn text_value(value: &Value) -> String {
    match value {
        Value::String(text) if !text.is_empty() && !text.contains([' ', '=', '"', '\n', '\t']) => {
            text.clone()
        }
        Value::String(text) => Value::String(text.clone()).to_string(),
        other => other.to_string(),
    }
}

fn console_function(lua: &Lua, level: Level) -> LuaResult<Option<LuaFunction>> {
    let console =
        match package_environment(lua).map(|environment| environment.get::<LuaValue>("Console")) {
            Some(Ok(LuaValue::Table(console))) => console,
            _ => match lua.globals().get::<LuaValue>("Console")? {
                LuaValue::Table(console) => console,
                _ => return Ok(None),
            },
        };
    let name = match level {
        Level::Debug | Level::Info => "Log",
        Level::Warn => "Warn",
        Level::Error => "Error",
    };
    console.get(name)
}

impl Logger {
    fn log(
        &self,
        lua: &Lua,
        level: Level,
        message: String,
        fields: Option<LuaTable>,
    ) -> LuaResult<()> {
        let current = self.level.lock().map(|level| *level).unwrap_or(Level::Info);
        if level < current {
            return Ok(());
        }
        let mut all = self.fields.clone();
        read_fields(lua, fields, &mut all)?;

        let time = Zoned::now();
        let line = |with_time: bool| -> String {
            match self.shared.format {
                Format::Json => {
                    let mut entry = Map::new();
                    entry.insert(
                        "time".to_owned(),
                        Value::String(
                            time.timestamp()
                                .display_with_offset(time.offset())
                                .to_string(),
                        ),
                    );
                    entry.insert(
                        "level".to_owned(),
                        Value::String(level.name().to_lowercase()),
                    );
                    if let Some(name) = &self.shared.name {
                        entry.insert("logger".to_owned(), Value::String(name.clone()));
                    }
                    entry.insert("message".to_owned(), Value::String(message.clone()));
                    for (key, value) in &all {
                        entry.entry(key.clone()).or_insert_with(|| value.clone());
                    }
                    Value::Object(entry).to_string()
                }
                Format::Text => {
                    let mut text = String::new();
                    if with_time {
                        text.push_str(&time.strftime("%Y-%m-%dT%H:%M:%S%.3f%:z ").to_string());
                        text.push_str(&format!("{:<5} ", level.name().to_uppercase()));
                    } else if level == Level::Debug {
                        text.push_str("[DEBUG] ");
                    }
                    if let Some(name) = &self.shared.name {
                        text.push_str(&format!("[{name}] "));
                    }
                    text.push_str(&message);
                    let mut keys: Vec<&String> = all.keys().collect();
                    keys.sort();
                    for key in keys {
                        text.push_str(&format!(" {key}={}", text_value(&all[key])));
                    }
                    text
                }
            }
        };

        if let Some(file) = &self.shared.file {
            file.write(line(true));
        }
        if self.shared.console
            && let Some(function) = console_function(lua, level)?
        {
            function.call::<()>(line(false))?;
        }
        Ok(())
    }
}

fn message(value: LuaValue) -> LuaResult<String> {
    match value {
        LuaValue::String(text) => Ok(text.to_string_lossy()),
        other => other.to_string(),
    }
}

impl LuaUserData for Logger {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        for (name, level) in [
            ("Debug", Level::Debug),
            ("Info", Level::Info),
            ("Warn", Level::Warn),
            ("Error", Level::Error),
        ] {
            methods.add_method(
                name,
                move |lua, logger, (text, fields): (LuaValue, Option<LuaTable>)| {
                    logger.log(lua, level, message(text)?, fields)
                },
            );
        }
        methods.add_method(
            "Log",
            |lua, logger, (level, text, fields): (String, LuaValue, Option<LuaTable>)| {
                logger.log(lua, Level::parse(&level)?, message(text)?, fields)
            },
        );
        methods.add_method("With", |lua, logger, fields: LuaTable| {
            let mut all = logger.fields.clone();
            read_fields(lua, Some(fields), &mut all)?;
            Ok(Object(Logger {
                shared: Arc::clone(&logger.shared),
                level: Arc::new(Mutex::new(
                    logger
                        .level
                        .lock()
                        .map(|level| *level)
                        .unwrap_or(Level::Info),
                )),
                fields: all,
            }))
        });
        methods.add_method("SetLevel", |_, logger, level: String| {
            let level = Level::parse(&level)?;
            if let Ok(mut current) = logger.level.lock() {
                *current = level;
            }
            Ok(())
        });
        methods.add_method("GetLevel", |_, logger, ()| {
            Ok(logger
                .level
                .lock()
                .map(|level| level.name())
                .unwrap_or("Info"))
        });
        methods.add_method("IsEnabled", |_, logger, level: String| {
            let level = Level::parse(&level)?;
            Ok(logger.level.lock().is_ok_and(|current| level >= *current))
        });
        methods.add_method("Flush", |_, logger, ()| {
            if let Some(file) = &logger.shared.file {
                file.flush();
            }
            Ok(())
        });
    }
}

fn create(lua: &Lua, options: &Options) -> LuaResult<Logger> {
    let format = match options.get::<String>("Format")?.as_deref() {
        None | Some("Text") => Format::Text,
        Some("Json") => Format::Json,
        Some(other) => {
            return Err(LuaError::runtime(format!(
                "unknown log format '{other}', expected Text or Json"
            )));
        }
    };
    let file = match options.get::<String>("File")? {
        Some(path) => {
            let max_size: u64 = options.get_or("MaxFileSize", DEFAULT_MAX_FILE_SIZE)?;
            if max_size == 0 {
                return Err(LuaError::runtime("MaxFileSize has to be greater than 0"));
            }
            let sink = open_sink(
                Path::new(&path),
                max_size,
                options.get_or("MaxFiles", DEFAULT_MAX_FILES)?,
            )?;
            let weak: Weak<dyn Resource> = Arc::downgrade(&sink) as Weak<dyn Resource>;
            if let Ok(dispatcher) = Dispatcher::get(lua) {
                dispatcher.track(lua, weak)?;
            }
            Some(sink)
        }
        None => None,
    };
    let mut fields = Map::new();
    read_fields(lua, options.get("Fields")?, &mut fields)?;

    Ok(Logger {
        shared: Arc::new(Shared {
            name: options.get("Name")?,
            format,
            console: options.get_or("Console", true)?,
            file,
        }),
        level: Arc::new(Mutex::new(Level::parse(
            &options.get_or("Level", "Info".to_owned())?,
        )?)),
        fields,
    })
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set(
        "New",
        lua.create_function(|lua, options: Options| Ok(Object(create(lua, &options)?)))?,
    )?;

    let default = lua.create_userdata(create(lua, &Options::default())?)?;
    for name in LEVELS {
        let logger = default.clone();
        module.set(
            *name,
            lua.create_function(move |lua, (text, fields): (LuaValue, Option<LuaTable>)| {
                logger
                    .borrow::<Logger>()?
                    .log(lua, Level::parse(name)?, message(text)?, fields)
            })?,
        )?;
    }
    Ok(module)
}
