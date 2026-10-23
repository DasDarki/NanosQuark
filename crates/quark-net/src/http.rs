use std::convert::Infallible;
use std::net::SocketAddr;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes as HttpBytes;
use futures_util::FutureExt;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::header::{
    ACCEPT_ENCODING, CACHE_CONTROL, CONNECTION, CONTENT_ENCODING, CONTENT_TYPE, HeaderName,
    HeaderValue, SET_COOKIE, UPGRADE, VARY,
};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use mlua::prelude::*;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use quark_core::{
    Binary, Bytes, Emitter, Options, create_object, create_userdata, object_state, runtime,
};
use tokio::sync::oneshot;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::Role;

use crate::connection::{Connected, Connection, add_connection_methods, bind_tcp};
use crate::encoding::{self, DEFAULT_ENCODINGS, Encoding};
use crate::stream::{self, BoxedIo};
use crate::tls;

const SOURCE: &str = include_str!("http.lua");
const DEFAULT_MAX_BODY_SIZE: usize = 1024 * 1024;
const DEFAULT_TIMEOUT: u64 = 30_000;
const DEFAULT_MAX_CONNECTIONS: usize = 1024;
const DEFAULT_HANDSHAKE_TIMEOUT: u64 = 10_000;
const DEFAULT_MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;
const DEFAULT_COMPRESSION_MIN_SIZE: usize = 1024;
const COOKIE_VALUE: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');
const MIME_TYPES: &[(&str, &str)] = &[
    ("html", "text/html; charset=utf-8"),
    ("htm", "text/html; charset=utf-8"),
    ("css", "text/css; charset=utf-8"),
    ("js", "text/javascript; charset=utf-8"),
    ("mjs", "text/javascript; charset=utf-8"),
    ("json", "application/json"),
    ("map", "application/json"),
    ("txt", "text/plain; charset=utf-8"),
    ("csv", "text/csv; charset=utf-8"),
    ("xml", "application/xml"),
    ("svg", "image/svg+xml"),
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("ico", "image/x-icon"),
    ("wasm", "application/wasm"),
    ("woff", "font/woff"),
    ("woff2", "font/woff2"),
    ("ttf", "font/ttf"),
    ("mp3", "audio/mpeg"),
    ("ogg", "audio/ogg"),
    ("wav", "audio/wav"),
    ("mp4", "video/mp4"),
    ("webm", "video/webm"),
    ("pdf", "application/pdf"),
    ("zip", "application/zip"),
];
const ROUTE_METHODS: &[(&str, &str)] = &[
    ("Get", "GET"),
    ("Post", "POST"),
    ("Put", "PUT"),
    ("Patch", "PATCH"),
    ("Delete", "DELETE"),
    ("Options", "OPTIONS"),
    ("All", "*"),
    ("WebSocket", "WEBSOCKET"),
];

struct Runtime(LuaTable);

pub enum ServerCommand {
    Close,
}

pub struct HttpServer {
    connection: Arc<Connection<ServerCommand>>,
    port: u16,
}

impl Connected for HttpServer {
    type Command = ServerCommand;

    fn connection(&self) -> &Arc<Connection<ServerCommand>> {
        &self.connection
    }
}

pub(crate) fn http_runtime(lua: &Lua) -> LuaResult<LuaTable> {
    if let Some(runtime) = lua.app_data_ref::<Runtime>() {
        return Ok(runtime.0.clone());
    }

    let report = lua.create_function(|lua, message: String| {
        let error = LuaError::runtime(message);
        quark_core::report_error(lua, &error);
        Ok(())
    })?;
    let decode_component = lua.create_function(|lua, text: String| {
        let decoded =
            url::form_urlencoded::parse(format!("x={}", text.replace('+', "%2B")).as_bytes())
                .next()
                .map(|(_, value)| value.into_owned())
                .unwrap_or(text);
        lua.create_string(decoded)
    })?;
    let decode_json = lua.create_function(|lua, body: LuaString| {
        match serde_json::from_slice::<serde_json::Value>(&body.as_bytes()) {
            Ok(value) => (
                lua.to_value_with(
                    &value,
                    LuaSerializeOptions::new()
                        .serialize_none_to_null(false)
                        .serialize_unit_to_null(false)
                        .set_array_metatable(true),
                )?,
                LuaNil,
            )
                .into_lua_multi(lua),
            Err(error) => (LuaNil, format!("invalid JSON: {error}")).into_lua_multi(lua),
        }
    })?;

    let parse_form =
        lua.create_function(|lua, (body, content_type): (LuaString, Option<String>)| {
            match parse_form(lua, &body.as_bytes(), content_type.as_deref()) {
                Ok((fields, files)) => (fields, files).into_lua_multi(lua),
                Err(error) => (LuaNil, error).into_lua_multi(lua),
            }
        })?;
    let resolve_static = lua.create_function(
        |_, (directory, relative, index): (String, String, String)| {
            Ok(resolve_static(&directory, &relative, &index)
                .map(|path| path.to_string_lossy().into_owned()))
        },
    )?;

    let runtime: LuaTable = lua.load(SOURCE).set_name("=quark/http").call((
        quark_core::promise(lua)?,
        report,
        decode_component,
        decode_json,
        parse_form,
        resolve_static,
    ))?;
    lua.set_app_data(Runtime(runtime.clone()));
    Ok(runtime)
}

impl LuaUserData for HttpServer {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        add_connection_methods(methods);
        methods.add_method("GetPort", |_, server, ()| Ok(server.port));
        methods.add_method("Close", |_, server, ()| {
            server.connection.close_with(ServerCommand::Close);
            Ok(())
        });

        methods.add_function(
            "Use",
            |lua, (this, middleware): (LuaAnyUserData, LuaValue)| {
                let add: LuaFunction = http_runtime(lua)?.get("add_middleware")?;
                add.call::<()>((object_state(&this)?, middleware))?;
                Ok(this)
            },
        );

        methods.add_function(
            "Route",
            |lua, (this, method, pattern, handler): (LuaAnyUserData, String, LuaValue, LuaValue)| {
                let add: LuaFunction = http_runtime(lua)?.get("add_route")?;
                add.call::<()>((object_state(&this)?, method, pattern, handler))?;
                Ok(this)
            },
        );

        for (name, method) in ROUTE_METHODS {
            methods.add_function(
                *name,
                move |lua, (this, pattern, handler): (LuaAnyUserData, LuaValue, LuaValue)| {
                    let add: LuaFunction = http_runtime(lua)?.get("add_route")?;
                    add.call::<()>((object_state(&this)?, *method, pattern, handler))?;
                    Ok(this)
                },
            );
        }

        methods.add_function(
            "Static",
            |lua, (this, prefix, directory, options): (LuaAnyUserData, LuaValue, LuaValue, LuaValue)| {
                let add: LuaFunction = http_runtime(lua)?.get("add_static")?;
                add.call::<()>((object_state(&this)?, prefix, directory, options))?;
                Ok(this)
            },
        );

        methods.add_meta_method(LuaMetaMethod::ToString, |_, server, ()| {
            Ok(format!(
                "HTTPServer({}, port {})",
                server.connection.state_name(),
                server.port
            ))
        });
    }
}

struct ResponseData {
    status: u16,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Vec<u8>,
    compress: bool,
}

enum Reply {
    Data(ResponseData),
    Upgrade {
        handler: LuaRegistryKey,
        request: LuaRegistryKey,
        headers: Vec<(HeaderName, HeaderValue)>,
    },
}

pub struct HttpResponse {
    sender: Option<oneshot::Sender<Reply>>,
    status: u16,
    headers: Vec<(HeaderName, HeaderValue)>,
    head: bool,
    compress: bool,
}

impl HttpResponse {
    fn set_status(&mut self, status: u16) -> LuaResult<()> {
        if !(100..=599).contains(&status) {
            return Err(LuaError::runtime(
                "HTTP status codes have to be between 100 and 599",
            ));
        }
        self.status = status;
        Ok(())
    }

    fn set_header(&mut self, name: &str, value: &str) -> LuaResult<()> {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| LuaError::runtime(format!("'{name}' is not a valid header name")))?;
        let value = HeaderValue::from_str(value).map_err(|_| {
            LuaError::runtime(format!("the value of the header '{name}' is invalid"))
        })?;
        self.headers.retain(|(existing, _)| *existing != name);
        self.headers.push((name, value));
        Ok(())
    }

    fn append_header(&mut self, name: HeaderName, value: &str) -> LuaResult<()> {
        let value = HeaderValue::from_str(value).map_err(|_| {
            LuaError::runtime(format!("the value of the header '{name}' is invalid"))
        })?;
        self.headers.push((name, value));
        Ok(())
    }

    fn take_sender(&mut self) -> LuaResult<oneshot::Sender<Reply>> {
        self.sender
            .take()
            .ok_or_else(|| LuaError::runtime("the response was already sent"))
    }

    fn has_header(&self, name: &HeaderName) -> bool {
        self.headers.iter().any(|(existing, _)| existing == name)
    }

    fn send(
        &mut self,
        body: Vec<u8>,
        content_type: Option<&str>,
        status: Option<u16>,
    ) -> LuaResult<bool> {
        let sender = self.take_sender()?;
        if let Some(status) = status {
            self.set_status(status)?;
        }
        if let Some(content_type) = content_type {
            self.set_header("content-type", content_type)?;
        } else if !body.is_empty() && !self.has_header(&CONTENT_TYPE) {
            let default = if std::str::from_utf8(&body).is_ok() {
                "text/plain; charset=utf-8"
            } else {
                "application/octet-stream"
            };
            self.set_header("content-type", default)?;
        }

        let body = if self.head { Vec::new() } else { body };
        Ok(sender
            .send(Reply::Data(ResponseData {
                status: self.status,
                headers: std::mem::take(&mut self.headers),
                body,
                compress: self.compress,
            }))
            .is_ok())
    }

    fn cookie(&mut self, name: &str, value: &str, options: &Options, clear: bool) -> LuaResult<()> {
        let valid_name = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte));
        if !valid_name {
            return Err(LuaError::runtime(format!(
                "'{name}' is not a valid cookie name"
            )));
        }

        let mut cookie = format!("{name}={}", utf8_percent_encode(value, COOKIE_VALUE));
        let path = options.get_or("Path", "/".to_owned())?;
        cookie.push_str(&format!("; Path={path}"));
        if let Some(domain) = options.get::<String>("Domain")? {
            cookie.push_str(&format!("; Domain={domain}"));
        }
        if clear {
            cookie.push_str("; Max-Age=0");
        } else if let Some(max_age) = options.get::<i64>("MaxAge")? {
            cookie.push_str(&format!("; Max-Age={max_age}"));
        }

        let secure = options.get_or("Secure", false)?;
        let same_site = options.get_or("SameSite", "Lax".to_owned())?;
        let same_site = match same_site.to_ascii_lowercase().as_str() {
            "lax" => "Lax",
            "strict" => "Strict",
            "none" if secure => "None",
            "none" => {
                return Err(LuaError::runtime(
                    "cookies with SameSite None have to be Secure",
                ));
            }
            other => {
                return Err(LuaError::runtime(format!(
                    "unknown SameSite value '{other}', expected Lax, Strict or None"
                )));
            }
        };
        cookie.push_str(&format!("; SameSite={same_site}"));
        if secure {
            cookie.push_str("; Secure");
        }
        if options.get_or("HttpOnly", true)? {
            cookie.push_str("; HttpOnly");
        }
        self.append_header(SET_COOKIE, &cookie)
    }
}

impl LuaUserData for HttpResponse {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("Status", |_, (this, status): (LuaAnyUserData, u16)| {
            this.borrow_mut::<HttpResponse>()?.set_status(status)?;
            Ok(this)
        });
        methods.add_function(
            "Header",
            |_, (this, name, value): (LuaAnyUserData, String, String)| {
                this.borrow_mut::<HttpResponse>()?
                    .set_header(&name, &value)?;
                Ok(this)
            },
        );
        methods.add_function(
            "Compress",
            |_, (this, enabled): (LuaAnyUserData, Option<bool>)| {
                this.borrow_mut::<HttpResponse>()?.compress = enabled.unwrap_or(true);
                Ok(this)
            },
        );
        methods.add_method("GetStatus", |_, response, ()| Ok(response.status));
        methods.add_method("IsSent", |_, response, ()| Ok(response.sender.is_none()));
        methods.add_method_mut(
            "Send",
            |_, response, (body, content_type, status): (Option<Binary>, Option<String>, Option<u16>)| {
                response.send(body.map(|body| body.0).unwrap_or_default(), content_type.as_deref(), status)
            },
        );
        methods.add_method_mut(
            "Text",
            |_, response, (text, status): (String, Option<u16>)| {
                response.send(text.into_bytes(), Some("text/plain; charset=utf-8"), status)
            },
        );
        methods.add_method_mut(
            "Html",
            |_, response, (html, status): (String, Option<u16>)| {
                response.send(html.into_bytes(), Some("text/html; charset=utf-8"), status)
            },
        );
        methods.add_method_mut(
            "Json",
            |lua, response, (value, status): (LuaValue, Option<u16>)| {
                let value: serde_json::Value = lua
                    .from_value_with(
                        value,
                        LuaDeserializeOptions::new()
                            .sort_keys(true)
                            .deny_recursive_tables(true),
                    )
                    .map_err(|error| {
                        LuaError::runtime(format!(
                            "cannot encode the response as JSON: {}",
                            quark_core::describe(&error)
                        ))
                    })?;
                let body = serde_json::to_vec(&value).map_err(LuaError::external)?;
                response.send(body, Some("application/json"), status)
            },
        );
        methods.add_method_mut(
            "Redirect",
            |_, response, (location, status): (String, Option<u16>)| {
                response.set_header("location", &location)?;
                response.send(Vec::new(), None, Some(status.unwrap_or(302)))
            },
        );
        methods.add_function(
            "Cookie",
            |_, (this, name, value, options): (LuaAnyUserData, String, String, Options)| {
                this.borrow_mut::<HttpResponse>()?
                    .cookie(&name, &value, &options, false)?;
                Ok(this)
            },
        );
        methods.add_function(
            "ClearCookie",
            |_, (this, name, options): (LuaAnyUserData, String, Options)| {
                this.borrow_mut::<HttpResponse>()?
                    .cookie(&name, "", &options, true)?;
                Ok(this)
            },
        );
        methods.add_method_mut(
            "SendFile",
            |_, response, (path, options): (String, Options)| {
                let content_type = match options.get::<String>("ContentType")? {
                    Some(content_type) => content_type,
                    None => mime_type(Path::new(&path)).to_owned(),
                };
                if let Some(max_age) = options.get::<u64>("MaxAge")? {
                    response.set_header("cache-control", &format!("public, max-age={max_age}"))?;
                } else if !response.has_header(&CACHE_CONTROL) {
                    response.set_header("cache-control", "no-cache")?;
                }
                response.set_header("content-type", &content_type)?;

                let sender = response.take_sender()?;
                let status = response.status;
                let headers = std::mem::take(&mut response.headers);
                let head = response.head;
                let compress = response.compress;
                let max_size = options.get_or("MaxFileSize", DEFAULT_MAX_FILE_SIZE)?;
                runtime().spawn(async move {
                    let reply = match read_file(&path, max_size).await {
                        Ok(body) => ResponseData {
                            status,
                            headers,
                            body: if head { Vec::new() } else { body },
                            compress,
                        },
                        Err(status) => ResponseData {
                            status,
                            headers: vec![(
                                CONTENT_TYPE,
                                HeaderValue::from_static("text/plain; charset=utf-8"),
                            )],
                            body: if status == 404 {
                                b"Not Found".to_vec()
                            } else {
                                b"Internal Server Error".to_vec()
                            },
                            compress: false,
                        },
                    };
                    let _ = sender.send(Reply::Data(reply));
                });
                Ok(true)
            },
        );
        methods.add_method_mut(
            "Upgrade",
            |lua, response, (handler, request): (LuaFunction, LuaTable)| {
                if !request.get::<bool>("IsWebSocket")? {
                    return Err(LuaError::runtime("the request is not a WebSocket upgrade"));
                }
                let sender = response.take_sender()?;
                let reply = Reply::Upgrade {
                    handler: lua.create_registry_value(handler)?,
                    request: lua.create_registry_value(request)?,
                    headers: std::mem::take(&mut response.headers),
                };
                Ok(sender.send(reply).is_ok())
            },
        );
        methods.add_meta_method(LuaMetaMethod::ToString, |_, response, ()| {
            Ok(format!("HTTPResponse({})", response.status))
        });
    }
}

pub fn build(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;
    module.set("Listen", lua.create_function(listen)?)?;
    module.set("Cors", http_runtime(lua)?.get::<LuaFunction>("cors")?)?;
    crate::http_client::register(lua, &module)?;
    Ok(module)
}

struct CompressionSettings {
    encodings: Vec<Encoding>,
    min_size: usize,
    level: Option<u32>,
}

#[derive(Clone)]
struct Settings {
    max_body_size: usize,
    max_message_size: usize,
    timeout: Duration,
    compression: Option<Arc<CompressionSettings>>,
}

fn compression_settings(options: &Options) -> LuaResult<Option<Arc<CompressionSettings>>> {
    let table = match options.get::<LuaValue>("Compression")? {
        None | Some(LuaValue::Boolean(false)) => return Ok(None),
        Some(LuaValue::Boolean(true)) => None,
        Some(LuaValue::Table(table)) => Some(table),
        Some(other) => {
            return Err(LuaError::runtime(format!(
                "the Compression option has to be a boolean or a table, got {}",
                other.type_name()
            )));
        }
    };
    let options = table.map(Options::from_table).unwrap_or_default();
    let encodings = match options.get::<Vec<String>>("Encodings")? {
        None => DEFAULT_ENCODINGS.to_vec(),
        Some(names) => names
            .iter()
            .map(|name| {
                Encoding::parse(name).ok_or_else(|| {
                    LuaError::runtime(format!(
                        "unknown encoding '{name}', expected br, zstd or gzip"
                    ))
                })
            })
            .collect::<LuaResult<Vec<_>>>()?,
    };
    if encodings.is_empty() {
        return Err(LuaError::runtime("at least one encoding is needed"));
    }
    Ok(Some(Arc::new(CompressionSettings {
        encodings,
        min_size: options.get_or("MinSize", DEFAULT_COMPRESSION_MIN_SIZE)?,
        level: options.get("Level")?,
    })))
}

fn listen(lua: &Lua, (port, options): (u16, Options)) -> LuaResult<Result<LuaAnyUserData, String>> {
    let host = options.get_or("Host", "0.0.0.0".to_owned())?;
    let max_connections = options.get_or("MaxConnections", DEFAULT_MAX_CONNECTIONS)?;
    let settings = Settings {
        max_body_size: options.get_or("MaxBodySize", DEFAULT_MAX_BODY_SIZE)?,
        max_message_size: options
            .get_or("MaxMessageSize", crate::framing::DEFAULT_MAX_FRAME_SIZE)?,
        timeout: Duration::from_millis(options.get_or("Timeout", DEFAULT_TIMEOUT)?),
        compression: compression_settings(&options)?,
    };

    let acceptor = tls::server(&options)?;

    let listener = match bind_tcp(&host, port) {
        Ok(listener) => listener,
        Err(error) => return Ok(Err(error)),
    };
    let local = listener.local_addr().ok();
    let (connection, mut commands) = Connection::new(|| ServerCommand::Close);
    connection.open(local, None);

    let object = create_object(
        lua,
        HttpServer {
            connection: Arc::clone(&connection),
            port: local.map(|address| address.port()).unwrap_or(port),
        },
    )?;
    let state = object_state(&object)?;
    state.raw_set("routes", lua.create_table()?)?;
    state.raw_set("middleware", lua.create_table()?)?;
    http_runtime(lua)?;

    let emitter = Emitter::new(lua, &object)?;
    connection.track(lua, &emitter)?;

    let active = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    runtime().spawn(async move {
        loop {
            tokio::select! {
                accepted = listener.accept() => match accepted {
                    Ok((socket, remote)) => {
                        use std::sync::atomic::Ordering;
                        if active.load(Ordering::Acquire) >= max_connections {
                            drop(socket);
                            continue;
                        }
                        active.fetch_add(1, Ordering::AcqRel);
                        let emitter = emitter.clone();
                        let active = Arc::clone(&active);
                        let acceptor = acceptor.clone();
                        let settings = settings.clone();
                        runtime().spawn(async move {
                            let io = match stream::accept(socket, acceptor, Duration::from_millis(DEFAULT_HANDSHAKE_TIMEOUT)).await {
                                Ok(io) => io,
                                Err(_) => {
                                    active.fetch_sub(1, Ordering::AcqRel);
                                    return;
                                }
                            };
                            let service = service_fn(move |request| {
                                let emitter = emitter.clone();
                                let settings = settings.clone();
                                async move { Ok::<_, Infallible>(handle(request, remote, emitter, settings).await) }
                            });
                            let _ = http1::Builder::new()
                                .keep_alive(true)
                                .serve_connection(TokioIo::new(io), service)
                                .with_upgrades()
                                .await;
                            active.fetch_sub(1, Ordering::AcqRel);
                        });
                    }
                    Err(error) => {
                        emitter.emit("Error", error.to_string());
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                },
                _ = commands.recv() => break,
            }
        }
        connection.closed();
        emitter.emit("Close", "closed".to_owned());
    });

    Ok(Ok(object))
}

fn plain(status: StatusCode, text: &str) -> Response<Full<HttpBytes>> {
    let mut response = Response::new(Full::new(HttpBytes::from(text.to_owned())));
    *response.status_mut() = status;
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );
    response
}

fn websocket_key(request: &Request<Incoming>) -> Option<String> {
    let headers = request.headers();
    let has_token = |name: HeaderName, token: &str| {
        headers.get_all(name).iter().any(|value| {
            value
                .to_str()
                .map(|value| {
                    value
                        .split(',')
                        .any(|part| part.trim().eq_ignore_ascii_case(token))
                })
                .unwrap_or(false)
        })
    };
    let upgrade = request.method() == hyper::Method::GET
        && has_token(CONNECTION, "upgrade")
        && has_token(UPGRADE, "websocket")
        && headers
            .get("sec-websocket-version")
            .and_then(|value| value.to_str().ok())
            == Some("13");
    if !upgrade {
        return None;
    }
    headers
        .get("sec-websocket-key")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

async fn handle(
    mut request: Request<Incoming>,
    remote: SocketAddr,
    emitter: Emitter,
    settings: Settings,
) -> Response<Full<HttpBytes>> {
    let websocket_key = websocket_key(&request);
    let on_upgrade = websocket_key
        .as_ref()
        .map(|_| hyper::upgrade::on(&mut request));

    let accept_encoding = request
        .headers()
        .get_all(ACCEPT_ENCODING)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .collect::<Vec<_>>()
        .join(",");
    let (parts, body) = request.into_parts();
    let body = match Limited::new(body, settings.max_body_size).collect().await {
        Ok(collected) => collected.to_bytes().to_vec(),
        Err(_) => return plain(StatusCode::PAYLOAD_TOO_LARGE, "Payload Too Large"),
    };

    let method = parts.method.as_str().to_owned();
    let path = parts.uri.path().to_owned();
    let query = parts.uri.query().unwrap_or_default().to_owned();
    let mut headers: Vec<(String, String)> = Vec::new();
    for (name, value) in &parts.headers {
        let value = String::from_utf8_lossy(value.as_bytes()).into_owned();
        match headers
            .iter_mut()
            .find(|(existing, _)| existing == name.as_str())
        {
            Some((_, existing)) => {
                let separator = if name == hyper::header::COOKIE {
                    "; "
                } else {
                    ", "
                };
                existing.push_str(separator);
                existing.push_str(&value);
            }
            None => headers.push((name.as_str().to_owned(), value)),
        }
    }
    let is_websocket = websocket_key.is_some();

    let (sender, receiver) = oneshot::channel();
    emitter.with(move |lua, server| {
        let request = lua.create_table()?;
        request.set("Method", method.as_str())?;
        request.set("Path", path)?;
        request.set("RawQuery", query.as_str())?;
        let parameters = lua.create_table()?;
        for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
            parameters.set(key.as_ref(), value.as_ref())?;
        }
        request.set("Query", parameters)?;

        let cookies = lua.create_table()?;
        let header_table = lua.create_table()?;
        for (name, value) in headers {
            if name == "cookie" {
                for pair in value.split(';') {
                    if let Some((key, value)) = pair.trim().split_once('=') {
                        let decoded =
                            percent_decode_str(value.trim().trim_matches('"')).decode_utf8_lossy();
                        cookies.set(key.trim(), decoded.as_ref())?;
                    }
                }
            }
            header_table.set(name, value)?;
        }
        request.set("Headers", header_table)?;
        request.set("Cookies", cookies)?;
        request.set("Body", Bytes(body))?;
        request.set("RemoteAddress", remote.ip().to_string())?;
        request.set("IsWebSocket", is_websocket)?;

        let response = create_userdata(
            lua,
            HttpResponse {
                sender: Some(sender),
                status: 200,
                headers: Vec::new(),
                head: method == "HEAD",
                compress: true,
            },
        )?;

        let dispatch: LuaFunction = http_runtime(lua)?.get("dispatch")?;
        dispatch.call::<()>((object_state(server)?, request, response))
    });

    let reply = match tokio::time::timeout(settings.timeout, receiver).await {
        Ok(Ok(reply)) => reply,
        Ok(Err(_)) => return plain(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable"),
        Err(_) => return plain(StatusCode::GATEWAY_TIMEOUT, "Gateway Timeout"),
    };

    match reply {
        Reply::Data(mut data) => {
            if let Some(compression) = &settings.compression {
                encode_body(&mut data, compression, &accept_encoding).await;
            }
            let mut response = Response::new(Full::new(HttpBytes::from(data.body)));
            *response.status_mut() =
                StatusCode::from_u16(data.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            for (name, value) in data.headers {
                response.headers_mut().append(name, value);
            }
            response
        }
        Reply::Upgrade {
            handler,
            request,
            headers,
        } => {
            let (Some(key), Some(on_upgrade)) = (websocket_key, on_upgrade) else {
                return plain(StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error");
            };

            let config = crate::websocket::server_config(settings.max_message_size);
            runtime().spawn(async move {
                let Ok(upgraded) = on_upgrade.await else {
                    return;
                };
                let io: BoxedIo = Box::new(TokioIo::new(upgraded));
                let socket = WebSocketStream::from_raw_socket(io, Role::Server, Some(config)).await;
                emitter.with(move |lua, _| {
                    let socket = crate::websocket::adopt(lua, socket, Some(remote))?;
                    let handler: LuaFunction = lua.registry_value(&handler)?;
                    let request: LuaTable = lua.registry_value(&request)?;
                    let accept: LuaFunction = http_runtime(lua)?.get("accept_websocket")?;
                    accept.call::<()>((handler, socket, request))
                });
            });

            let mut response = Response::new(Full::new(HttpBytes::new()));
            *response.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
            let response_headers = response.headers_mut();
            for (name, value) in headers {
                response_headers.append(name, value);
            }
            response_headers.insert(UPGRADE, HeaderValue::from_static("websocket"));
            response_headers.insert(CONNECTION, HeaderValue::from_static("Upgrade"));
            if let Ok(accept) = HeaderValue::from_str(&derive_accept_key(key.as_bytes())) {
                response_headers.insert("sec-websocket-accept", accept);
            }
            response
        }
    }
}

async fn encode_body(data: &mut ResponseData, compression: &CompressionSettings, accept: &str) {
    let header = |name: &HeaderName| {
        data.headers
            .iter()
            .find(|(existing, _)| existing == name)
            .and_then(|(_, value)| value.to_str().ok())
            .map(str::to_owned)
    };
    let eligible = data.compress
        && data.body.len() >= compression.min_size
        && !matches!(data.status, 204 | 206 | 304)
        && header(&CONTENT_ENCODING).is_none()
        && header(&CONTENT_TYPE).is_some_and(|content_type| encoding::compressible(&content_type));
    data.headers
        .push((VARY, HeaderValue::from_static("accept-encoding")));
    if !eligible {
        return;
    }
    let Some(encoding) = encoding::choose(accept, &compression.encodings) else {
        return;
    };
    let body = std::mem::take(&mut data.body);
    let level = compression.level;
    let result = tokio::task::spawn_blocking(move || {
        let compressed = encoding::compress(encoding, &body, level);
        (body, compressed)
    })
    .await;
    match result {
        Ok((_, Ok(compressed))) => {
            data.body = compressed;
            data.headers
                .push((CONTENT_ENCODING, HeaderValue::from_static(encoding.name())));
        }
        Ok((body, Err(_))) => data.body = body,
        Err(_) => {}
    }
}

fn mime_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    MIME_TYPES
        .iter()
        .find(|(candidate, _)| *candidate == extension)
        .map(|(_, mime)| *mime)
        .unwrap_or("application/octet-stream")
}

async fn read_file(path: &str, max_size: u64) -> Result<Vec<u8>, u16> {
    let metadata = tokio::fs::metadata(path).await.map_err(|_| 404u16)?;
    if !metadata.is_file() {
        return Err(404);
    }
    if metadata.len() > max_size {
        return Err(500);
    }
    tokio::fs::read(path).await.map_err(|_| 500u16)
}

fn resolve_static(directory: &str, relative: &str, index: &str) -> Option<PathBuf> {
    let base = std::fs::canonicalize(directory).ok()?;
    let mut path = base.clone();
    for part in relative.split(['/', '\\']) {
        match Path::new(part).components().next() {
            None => {}
            Some(Component::Normal(_)) if !part.contains(':') => path.push(part),
            Some(Component::CurDir) => {}
            _ => return None,
        }
    }
    if path.is_dir() {
        path.push(index);
    }
    let canonical = std::fs::canonicalize(&path).ok()?;
    (canonical.starts_with(&base) && canonical.is_file()).then_some(canonical)
}

fn parse_form(
    lua: &Lua,
    body: &[u8],
    content_type: Option<&str>,
) -> Result<(LuaTable, LuaTable), String> {
    let fields = lua.create_table().map_err(|error| error.to_string())?;
    let files = lua.create_table().map_err(|error| error.to_string())?;
    let content_type = content_type.unwrap_or("application/x-www-form-urlencoded");
    let essence = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();

    match essence.as_str() {
        "application/x-www-form-urlencoded" => {
            for (key, value) in url::form_urlencoded::parse(body) {
                fields
                    .set(key.as_ref(), value.as_ref())
                    .map_err(|error| error.to_string())?;
            }
        }
        "multipart/form-data" => {
            let boundary = multer::parse_boundary(content_type)
                .map_err(|error| format!("invalid multipart form: {error}"))?;
            let data = HttpBytes::copy_from_slice(body);
            let stream = futures_util::stream::once(async move { Ok::<_, Infallible>(data) });
            let mut multipart = multer::Multipart::new(stream, boundary);
            loop {
                let field = multipart
                    .next_field()
                    .now_or_never()
                    .ok_or("invalid multipart form: the body ended early")?
                    .map_err(|error| format!("invalid multipart form: {error}"))?;
                let Some(field) = field else {
                    break;
                };
                let name = field.name().unwrap_or_default().to_owned();
                let file_name = field.file_name().map(str::to_owned);
                let field_type = field.content_type().map(|mime| mime.to_string());
                let data = field
                    .bytes()
                    .now_or_never()
                    .ok_or("invalid multipart form: the body ended early")?
                    .map_err(|error| format!("invalid multipart form: {error}"))?;

                let store = || -> LuaResult<()> {
                    match file_name {
                        Some(file_name) => {
                            let file = lua.create_table()?;
                            file.set("Field", name)?;
                            file.set("FileName", file_name)?;
                            file.set("ContentType", field_type)?;
                            file.set("Data", Bytes(data.to_vec()))?;
                            files.raw_push(file)
                        }
                        None => fields.set(name, Bytes(data.to_vec())),
                    }
                };
                store().map_err(|error| error.to_string())?;
            }
        }
        other => return Err(format!("unsupported form content type '{other}'")),
    }
    Ok((fields, files))
}
