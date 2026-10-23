use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes as HttpBytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::header::{
    ACCEPT_ENCODING, CONTENT_LENGTH, CONTENT_TYPE, HOST, HeaderName, HeaderValue, LOCATION,
    USER_AGENT,
};
use hyper::{Method, Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use mlua::prelude::*;
use quark_core::{Binary, Bytes, Emitter, Options, create_object, options_and_callback, runtime};
use url::Url;

use crate::connection::{Connected, Connection, add_connection_methods};
use crate::encoding;
use crate::http::http_runtime;
use crate::stream;
use crate::tls;

const DEFAULT_TIMEOUT: u64 = 30_000;
const DEFAULT_MAX_BODY_SIZE: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_REDIRECTS: usize = 5;
const AGENT: &str = concat!("Quark/", env!("CARGO_PKG_VERSION"));

#[derive(Clone)]
struct Spec {
    method: Method,
    url: Url,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Vec<u8>,
    timeout: Duration,
    max_body_size: usize,
    max_redirects: usize,
    ca: Option<String>,
    identity: Option<tls::Identity>,
    http2: bool,
    decompress: bool,
}

enum Sender {
    Http1(hyper::client::conn::http1::SendRequest<Full<HttpBytes>>),
    Http2(hyper::client::conn::http2::SendRequest<Full<HttpBytes>>),
}

impl Sender {
    async fn send(
        &mut self,
        request: Request<Full<HttpBytes>>,
    ) -> hyper::Result<Response<Incoming>> {
        match self {
            Sender::Http1(sender) => sender.send_request(request).await,
            Sender::Http2(sender) => sender.send_request(request).await,
        }
    }
}

enum RequestBody {
    Raw(Vec<u8>),
    Json(LuaValue),
}

pub struct Headers(Vec<(String, String)>);

impl IntoLua for Headers {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        for (name, value) in self.0 {
            match table.raw_get::<Option<String>>(name.as_str())? {
                Some(existing) => table.raw_set(name, format!("{existing}, {value}"))?,
                None => table.raw_set(name, value)?,
            }
        }
        Ok(LuaValue::Table(table))
    }
}

pub struct ClientResponse {
    status: u16,
    version: &'static str,
    headers: Headers,
    body: Vec<u8>,
    url: String,
}

impl IntoLua for ClientResponse {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        table.set("Status", self.status)?;
        table.set("Version", self.version)?;
        table.set("Headers", self.headers)?;
        table.set("Body", Bytes(self.body))?;
        table.set("URL", self.url)?;
        let metatable: LuaTable = http_runtime(lua)?.get("ClientResponse")?;
        table.set_metatable(Some(metatable))?;
        Ok(LuaValue::Table(table))
    }
}

fn spec(
    lua: &Lua,
    url: String,
    method: Option<String>,
    body: Option<RequestBody>,
    options: &Options,
) -> LuaResult<Spec> {
    let mut url = Url::parse(&url)
        .map_err(|error| LuaError::runtime(format!("invalid URL '{url}': {error}")))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(LuaError::runtime(
            "only http:// and https:// URLs are supported",
        ));
    }
    if let Some(query) = options.get::<LuaTable>("Query")? {
        let mut pairs = Vec::new();
        for pair in query.pairs::<String, LuaValue>() {
            let (key, value) = pair?;
            pairs.push((key, value.to_string()?));
        }
        pairs.sort();
        url.query_pairs_mut().extend_pairs(pairs);
    }

    let method = method
        .or(options.get::<String>("Method")?)
        .unwrap_or_else(|| "GET".to_owned())
        .to_ascii_uppercase();
    let method = Method::from_bytes(method.as_bytes())
        .map_err(|_| LuaError::runtime(format!("'{method}' is not a valid HTTP method")))?;

    let mut headers = Vec::new();
    if let Some(table) = options.get::<LuaTable>("Headers")? {
        for pair in table.pairs::<String, String>() {
            let (name, value) = pair?;
            let header = HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| LuaError::runtime(format!("'{name}' is not a valid header name")))?;
            let value = HeaderValue::from_str(&value).map_err(|_| {
                LuaError::runtime(format!("the value of the header '{name}' is invalid"))
            })?;
            headers.push((header, value));
        }
    }

    let body = match body {
        Some(body) => Some(body),
        None => match options.get::<LuaValue>("Json")? {
            Some(value) => Some(RequestBody::Json(value)),
            None => options
                .get::<Binary>("Body")?
                .map(|data| RequestBody::Raw(data.0)),
        },
    };
    let body = match body {
        None => Vec::new(),
        Some(RequestBody::Raw(data)) => data,
        Some(RequestBody::Json(value)) => {
            let value: serde_json::Value = lua
                .from_value_with(
                    value,
                    LuaDeserializeOptions::new()
                        .sort_keys(true)
                        .deny_recursive_tables(true),
                )
                .map_err(|error| {
                    LuaError::runtime(format!(
                        "cannot encode the JSON body: {}",
                        quark_core::describe(&error)
                    ))
                })?;
            if !headers.iter().any(|(name, _)| *name == CONTENT_TYPE) {
                headers.push((CONTENT_TYPE, HeaderValue::from_static("application/json")));
            }
            serde_json::to_vec(&value).map_err(LuaError::external)?
        }
    };

    Ok(Spec {
        method,
        url,
        headers,
        body,
        timeout: Duration::from_millis(options.get_or("Timeout", DEFAULT_TIMEOUT)?),
        max_body_size: options.get_or("MaxBodySize", DEFAULT_MAX_BODY_SIZE)?,
        max_redirects: if options.get_or("FollowRedirects", true)? {
            options.get_or("MaxRedirects", DEFAULT_MAX_REDIRECTS)?
        } else {
            0
        },
        ca: options.get("CA")?,
        identity: tls::identity(options)?,
        http2: options.get_or("HTTP2", true)?,
        decompress: options.get_or("Decompress", true)?,
    })
}

async fn send(mut spec: Spec) -> Result<(Response<Incoming>, Url), String> {
    let mut redirects = 0;
    loop {
        let host = spec
            .url
            .host_str()
            .ok_or_else(|| format!("the URL {} has no host", spec.url))?
            .trim_start_matches('[')
            .trim_end_matches(']')
            .to_owned();
        let port = spec.url.port_or_known_default().unwrap_or(80);
        let alpn: &[&[u8]] = if spec.http2 {
            &[tls::ALPN_HTTP2, tls::ALPN_HTTP1]
        } else {
            &[tls::ALPN_HTTP1]
        };
        let tls = match spec.url.scheme() {
            "https" => Some(tls::client_for(
                &host,
                spec.ca.as_deref(),
                spec.identity.as_ref(),
                alpn,
            )?),
            _ => None,
        };

        let connected = stream::connect(&host, port, tls, spec.timeout).await?;
        let http2 = connected.alpn.as_deref() == Some(tls::ALPN_HTTP2);
        let handshake_failed =
            |error: hyper::Error| format!("the HTTP handshake with {host} failed: {error}");
        let mut sender = if http2 {
            let (sender, connection) = hyper::client::conn::http2::handshake(
                TokioExecutor::new(),
                TokioIo::new(connected.io),
            )
            .await
            .map_err(handshake_failed)?;
            runtime().spawn(async move {
                let _ = connection.await;
            });
            Sender::Http2(sender)
        } else {
            let (sender, connection) =
                hyper::client::conn::http1::handshake(TokioIo::new(connected.io))
                    .await
                    .map_err(handshake_failed)?;
            runtime().spawn(async move {
                let _ = connection.await;
            });
            Sender::Http1(sender)
        };

        let target = match (http2, spec.url.query()) {
            (true, _) => spec.url.to_string(),
            (false, Some(query)) => format!("{}?{query}", spec.url.path()),
            (false, None) => spec.url.path().to_owned(),
        };
        let mut request = Request::builder()
            .method(spec.method.clone())
            .uri(target)
            .body(Full::new(HttpBytes::from(spec.body.clone())))
            .map_err(|error| format!("cannot build the request: {error}"))?;
        let headers = request.headers_mut();
        let authority = match spec.url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host.clone(),
        };
        if !http2 {
            headers.insert(
                HOST,
                HeaderValue::from_str(&authority).map_err(|error| error.to_string())?,
            );
        }
        headers.insert(USER_AGENT, HeaderValue::from_static(AGENT));
        if !spec.body.is_empty()
            || matches!(spec.method, Method::POST | Method::PUT | Method::PATCH)
        {
            headers.insert(CONTENT_LENGTH, HeaderValue::from(spec.body.len()));
        }
        if spec.decompress {
            headers.insert(
                ACCEPT_ENCODING,
                HeaderValue::from_static(encoding::ACCEPT_ENCODING),
            );
        }
        for (name, value) in &spec.headers {
            headers.insert(name.clone(), value.clone());
        }

        let response = tokio::time::timeout(spec.timeout, sender.send(request))
            .await
            .map_err(|_| format!("the request to {} timed out", spec.url))?
            .map_err(|error| format!("the request to {} failed: {error}", spec.url))?;

        let status = response.status().as_u16();
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        match (status, location) {
            (301 | 302 | 303 | 307 | 308, Some(location)) if redirects < spec.max_redirects => {
                redirects += 1;
                spec.url = spec
                    .url
                    .join(&location)
                    .map_err(|error| format!("invalid redirect location '{location}': {error}"))?;
                if status == 303 || (matches!(status, 301 | 302) && spec.method == Method::POST) {
                    spec.method = Method::GET;
                    spec.body.clear();
                    spec.headers.retain(|(name, _)| *name != CONTENT_TYPE);
                }
            }
            _ => return Ok((response, spec.url)),
        }
    }
}

fn headers_of(response: &Response<Incoming>) -> Headers {
    Headers(
        response
            .headers()
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_owned(),
                    String::from_utf8_lossy(value.as_bytes()).into_owned(),
                )
            })
            .collect(),
    )
}

async fn fetch(spec: Spec) -> Result<ClientResponse, String> {
    let timeout = spec.timeout;
    let max_body_size = spec.max_body_size;
    let decompress = spec.decompress
        && !spec
            .headers
            .iter()
            .any(|(name, _)| *name == ACCEPT_ENCODING);
    let (response, url) = send(spec).await?;
    let status = response.status().as_u16();
    let version = crate::http::version_name(response.version());
    let mut headers = headers_of(&response);

    let mut body = response.into_body();
    let mut collected = Vec::new();
    let deadline = tokio::time::Instant::now() + timeout;
    while let Some(frame) = tokio::time::timeout_at(deadline, body.frame())
        .await
        .map_err(|_| format!("reading the response of {url} timed out"))?
    {
        let frame =
            frame.map_err(|error| format!("reading the response of {url} failed: {error}"))?;
        if let Some(data) = frame.data_ref() {
            if collected.len() + data.len() > max_body_size {
                return Err(format!(
                    "the response of {url} is larger than {max_body_size} bytes"
                ));
            }
            collected.extend_from_slice(data);
        }
    }

    let content_encoding = headers
        .0
        .iter()
        .find(|(name, _)| name == "content-encoding")
        .map(|(_, value)| value.clone());
    if decompress && let Some(content_encoding) = content_encoding {
        let input = std::mem::take(&mut collected);
        collected = tokio::task::spawn_blocking(move || {
            encoding::decompress(&content_encoding, input, max_body_size)
        })
        .await
        .map_err(|_| "decoding the response failed".to_owned())?
        .map_err(|error| format!("{error} ({url})"))?;
        headers
            .0
            .retain(|(name, _)| name != "content-encoding" && name != "content-length");
    }

    Ok(ClientResponse {
        status,
        version,
        headers,
        body: collected,
        url: url.to_string(),
    })
}

pub enum StreamCommand {
    Close,
}

pub struct HttpStream {
    connection: Arc<Connection<StreamCommand>>,
}

impl Connected for HttpStream {
    type Command = StreamCommand;

    fn connection(&self) -> &Arc<Connection<StreamCommand>> {
        &self.connection
    }
}

impl LuaUserData for HttpStream {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        add_connection_methods(methods);
        methods.add_method("Close", |_, stream, ()| {
            stream.connection.close_with(StreamCommand::Close);
            Ok(())
        });
        methods.add_meta_method(LuaMetaMethod::ToString, |_, stream, ()| {
            Ok(format!("HTTPStream({})", stream.connection.state_name()))
        });
    }
}

fn open_stream(lua: &Lua, options: Options) -> LuaResult<LuaAnyUserData> {
    let url = options
        .get::<String>("URL")?
        .ok_or_else(|| LuaError::runtime("the URL option is required"))?;
    let mut spec = spec(lua, url, None, None, &options)?;
    spec.decompress = false;
    let (connection, mut commands) = Connection::new(|| StreamCommand::Close);
    let object = create_object(
        lua,
        HttpStream {
            connection: Arc::clone(&connection),
        },
    )?;
    let emitter = Emitter::new(lua, &object)?;
    connection.track(lua, &emitter)?;

    runtime().spawn(async move {
        let reason = tokio::select! {
            result = async {
                let (response, url) = send(spec).await?;
                connection.open(None, None);
                emitter.emit("Response", (response.status().as_u16(), headers_of(&response), url.to_string()));
                let mut body = response.into_body();
                while let Some(frame) = body.frame().await {
                    let frame = frame.map_err(|error| format!("reading the response failed: {error}"))?;
                    if let Ok(data) = frame.into_data() {
                        emitter.emit("Data", Bytes(data.to_vec()));
                    }
                }
                emitter.emit("End", ());
                Ok::<_, String>("finished".to_owned())
            } => match result {
                Ok(reason) => reason,
                Err(error) => {
                    emitter.emit("Error", error.clone());
                    error
                }
            },
            _ = commands.recv() => "closed".to_owned(),
        };
        connection.closed();
        emitter.emit("Close", reason);
    });

    Ok(object)
}

pub fn register(lua: &Lua, module: &LuaTable) -> LuaResult<()> {
    module.set(
        "Request",
        lua.create_function(|lua, (options, callback): (Options, Option<LuaFunction>)| {
            let url = options
                .get::<String>("URL")?
                .ok_or_else(|| LuaError::runtime("the URL option is required"))?;
            let spec = spec(lua, url, None, None, &options)?;
            quark_core::run_async(lua, callback, fetch(spec))
        })?,
    )?;

    module.set(
        "Get",
        lua.create_function(
            |lua, (url, options, callback): (String, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                let spec = spec(lua, url, Some("GET".to_owned()), None, &options)?;
                quark_core::run_async(lua, callback, fetch(spec))
            },
        )?,
    )?;

    module.set(
        "Post",
        lua.create_function(
            |lua, (url, body, options, callback): (String, LuaValue, LuaValue, Option<LuaFunction>)| {
                let (options, callback) = options_and_callback(options, callback)?;
                let body = match body {
                    LuaNil => None,
                    LuaValue::Table(_) => Some(RequestBody::Json(body)),
                    other => Some(RequestBody::Raw(Binary::from_lua(other, lua)?.0)),
                };
                let spec = spec(lua, url, Some("POST".to_owned()), body, &options)?;
                quark_core::run_async(lua, callback, fetch(spec))
            },
        )?,
    )?;

    module.set("Stream", lua.create_function(open_stream)?)?;
    Ok(())
}
