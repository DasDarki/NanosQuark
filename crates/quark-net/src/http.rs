use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes as HttpBytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::header::{CONTENT_TYPE, HeaderName, HeaderValue};
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use mlua::prelude::*;
use quark_core::{
    Binary, Bytes, Emitter, Options, create_object, create_userdata, object_state, runtime,
};
use tokio::sync::oneshot;

use crate::connection::{Connected, Connection, add_connection_methods, bind_tcp};
use crate::stream;
use crate::tls;

const SOURCE: &str = include_str!("http.lua");
const DEFAULT_MAX_BODY_SIZE: usize = 1024 * 1024;
const DEFAULT_TIMEOUT: u64 = 30_000;
const DEFAULT_MAX_CONNECTIONS: usize = 1024;
const DEFAULT_HANDSHAKE_TIMEOUT: u64 = 10_000;
const ROUTE_METHODS: &[(&str, &str)] = &[
    ("Get", "GET"),
    ("Post", "POST"),
    ("Put", "PUT"),
    ("Patch", "PATCH"),
    ("Delete", "DELETE"),
    ("Options", "OPTIONS"),
    ("All", "*"),
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

    let runtime: LuaTable = lua.load(SOURCE).set_name("=quark/http").call((
        quark_core::promise(lua)?,
        report,
        decode_component,
        decode_json,
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
}

pub struct HttpResponse {
    sender: Option<oneshot::Sender<ResponseData>>,
    status: u16,
    headers: Vec<(HeaderName, HeaderValue)>,
    head: bool,
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

    fn has_header(&self, name: &HeaderName) -> bool {
        self.headers.iter().any(|(existing, _)| existing == name)
    }

    fn send(
        &mut self,
        body: Vec<u8>,
        content_type: Option<&str>,
        status: Option<u16>,
    ) -> LuaResult<bool> {
        let Some(sender) = self.sender.take() else {
            return Err(LuaError::runtime("the response was already sent"));
        };
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
            .send(ResponseData {
                status: self.status,
                headers: std::mem::take(&mut self.headers),
                body,
            })
            .is_ok())
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

#[derive(Clone, Copy)]
struct Settings {
    max_body_size: usize,
    timeout: Duration,
}

fn listen(lua: &Lua, (port, options): (u16, Options)) -> LuaResult<Result<LuaAnyUserData, String>> {
    let host = options.get_or("Host", "0.0.0.0".to_owned())?;
    let max_connections = options.get_or("MaxConnections", DEFAULT_MAX_CONNECTIONS)?;
    let settings = Settings {
        max_body_size: options.get_or("MaxBodySize", DEFAULT_MAX_BODY_SIZE)?,
        timeout: Duration::from_millis(options.get_or("Timeout", DEFAULT_TIMEOUT)?),
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
                                async move { Ok::<_, Infallible>(handle(request, remote, emitter, settings).await) }
                            });
                            let _ = http1::Builder::new()
                                .keep_alive(true)
                                .serve_connection(TokioIo::new(io), service)
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

async fn handle(
    request: Request<Incoming>,
    remote: SocketAddr,
    emitter: Emitter,
    settings: Settings,
) -> Response<Full<HttpBytes>> {
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
                existing.push_str(", ");
                existing.push_str(&value);
            }
            None => headers.push((name.as_str().to_owned(), value)),
        }
    }

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
        let header_table = lua.create_table()?;
        for (name, value) in headers {
            header_table.set(name, value)?;
        }
        request.set("Headers", header_table)?;
        request.set("Body", Bytes(body))?;
        request.set("RemoteAddress", remote.ip().to_string())?;

        let response = create_userdata(
            lua,
            HttpResponse {
                sender: Some(sender),
                status: 200,
                headers: Vec::new(),
                head: method == "HEAD",
            },
        )?;

        let dispatch: LuaFunction = http_runtime(lua)?.get("dispatch")?;
        dispatch.call::<()>((object_state(server)?, request, response))
    });

    match tokio::time::timeout(settings.timeout, receiver).await {
        Ok(Ok(data)) => {
            let mut response = Response::new(Full::new(HttpBytes::from(data.body)));
            *response.status_mut() =
                StatusCode::from_u16(data.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
            for (name, value) in data.headers {
                response.headers_mut().insert(name, value);
            }
            response
        }
        Ok(Err(_)) => plain(StatusCode::SERVICE_UNAVAILABLE, "Service Unavailable"),
        Err(_) => plain(StatusCode::GATEWAY_TIMEOUT, "Gateway Timeout"),
    }
}
