//! ClientMCP for Rust applications: expose a running application's functions
//! as MCP tools.
//!
//! The application keeps one outbound WebSocket to a ClientMCP bridge; AI
//! agents call its tools through the bridge's standard MCP endpoint. The same
//! protocol and behaviour as the JS SDK (`sdk/js`), see `SPEC.md`:
//!
//! - online once the bridge acknowledges the registration (§3.2);
//! - a ping every 25 s, the socket closed when 10 s pass without any message;
//! - reconnecting with jittered backoff up to 30 s, except after a close
//!   reconnecting cannot help (1009, 4400, 4409; 4401 with a fixed token);
//! - a call the agent cancels, or one in flight when the connection ends, is
//!   cancelled — its handler's future is dropped, so the work stops (§9);
//! - a result over 4 MiB becomes a tool error;
//! - `completed` / `accepted` / `rejected` answer with the outcome
//!   conventions (§9.2), whose own members data may not carry.
//!
//! ```no_run
//! use clientmcp_client::{Client, Options, Reply, Risk, Tool};
//!
//! # async fn run() {
//! let client = Client::new(Options::new("wss://bridge.example.com/connect", "client-token"));
//! client.expose(
//!     "greet",
//!     Tool::new(|args, _ctx| async move {
//!         let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("you");
//!         Ok(Reply::text(format!("Hello, {name}!")))
//!     })
//!     .description("Greets someone.")
//!     .risk(Risk::Observe),
//! );
//! client.connect().await.expect("online");
//! # }
//! ```

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde_json::{Map, Value, json};
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior, interval_at, sleep, sleep_until};
use tokio_tungstenite::Connector;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::HeaderValue;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
pub use tokio_util::sync::CancellationToken;

/// The WebSocket subprotocol of this protocol version.
pub const SUBPROTOCOL: &str = "clientmcp.v1";
/// The `_meta` key a tool's risk is published under (SPEC §9.1).
pub const RISK_META: &str = "io.github.clientmcp/risk";
/// Largest message on the client channel (SPEC §6).
pub const MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;

const PING: &str = r#"{"type":"ping"}"#;
const PING_INTERVAL: Duration = Duration::from_secs(25);
const PONG_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_BACKOFF: Duration = Duration::from_secs(30);
const UNAUTHORIZED: u16 = 4401;

/// Close codes after which reconnecting cannot help (SPEC §3.3).
fn fatal(code: u16) -> Option<&'static str> {
    match code {
        1009 => Some("message too large"),
        4400 => Some("protocol error"),
        4409 => Some("replaced by a newer connection with the same client ID"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Tools and their answers
// ---------------------------------------------------------------------------

/// How much a tool's action matters, on the application's own scale (SPEC
/// §9.1). The bridge does not read it: the application decides what each
/// level needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Risk {
    /// Changes nothing.
    Observe,
    /// Ordinary, reversible use.
    Operate,
    /// Security-relevant or hard to undo.
    Manage,
    /// Moves value or signs.
    Transact,
}

impl Risk {
    fn name(self) -> &'static str {
        match self {
            Risk::Observe => "observe",
            Risk::Operate => "operate",
            Risk::Manage => "manage",
            Risk::Transact => "transact",
        }
    }

    /// The MCP annotations the risk implies, unless the tool sets its own.
    fn hints(self) -> Map<String, Value> {
        let mut hints = Map::new();
        hints.insert("readOnlyHint".into(), Value::Bool(self == Risk::Observe));
        if self != Risk::Observe {
            hints.insert(
                "destructiveHint".into(),
                Value::Bool(matches!(self, Risk::Manage | Risk::Transact)),
            );
        }
        hints
    }
}

/// What a call is about, for its handler.
#[derive(Clone, Debug)]
pub struct CallContext {
    /// The tool being called.
    pub name: String,
    /// Cancelled when the agent cancels the call or the connection ends. The
    /// handler's future is dropped then as well; this is for work it handed
    /// elsewhere.
    pub cancelled: CancellationToken,
}

/// What a handler answers.
#[derive(Clone, Debug, PartialEq)]
pub enum Reply {
    /// Nothing to say: an empty result.
    Empty,
    /// Text, sent as one text item.
    Text(String),
    /// A JSON value, sent as text and — for an object — as `structuredContent`.
    Json(Value),
    /// A complete MCP `CallToolResult`, sent unchanged.
    Result(Value),
}

impl Reply {
    pub fn text(text: impl Into<String>) -> Self {
        Reply::Text(text.into())
    }

    fn into_result(self) -> Value {
        match self {
            Reply::Empty => json!({ "content": [] }),
            Reply::Text(text) => json!({ "content": [{ "type": "text", "text": text }] }),
            Reply::Json(value) => {
                let mut result =
                    json!({ "content": [{ "type": "text", "text": value.to_string() }] });
                // Only objects: MCP revisions before 2026-07-28 require it.
                if value.is_object() {
                    result["structuredContent"] = value;
                }
                result
            }
            Reply::Result(result) => result,
        }
    }
}

/// What a handler returns: a reply, or an error the model can read.
pub type HandlerResult = Result<Reply, String>;
type BoxFuture = Pin<Box<dyn Future<Output = HandlerResult> + Send>>;
type Handler = Arc<dyn Fn(Value, CallContext) -> BoxFuture + Send + Sync>;

/// A tool: an MCP tool definition plus the function that runs it.
///
/// The handler receives arguments written by a language model: validate them.
#[derive(Clone)]
pub struct Tool {
    title: Option<String>,
    description: Option<String>,
    input_schema: Option<Value>,
    output_schema: Option<Value>,
    annotations: Option<Map<String, Value>>,
    icons: Option<Value>,
    meta: Option<Map<String, Value>>,
    risk: Option<Risk>,
    handler: Handler,
}

impl Tool {
    pub fn new<F, Fut>(handler: F) -> Self
    where
        F: Fn(Value, CallContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HandlerResult> + Send + 'static,
    {
        Tool {
            title: None,
            description: None,
            input_schema: None,
            output_schema: None,
            annotations: None,
            icons: None,
            meta: None,
            risk: None,
            handler: Arc::new(move |args, ctx| Box::pin(handler(args, ctx))),
        }
    }

    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    #[must_use]
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// A JSON Schema for the arguments; defaults to `{ "type": "object" }`.
    #[must_use]
    pub fn input_schema(mut self, schema: Value) -> Self {
        self.input_schema = Some(schema);
        self
    }

    #[must_use]
    pub fn output_schema(mut self, schema: Value) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// MCP annotations; they win over those the risk implies.
    #[must_use]
    pub fn annotations(mut self, annotations: Map<String, Value>) -> Self {
        self.annotations = Some(annotations);
        self
    }

    #[must_use]
    pub fn icons(mut self, icons: Value) -> Self {
        self.icons = Some(icons);
        self
    }

    #[must_use]
    pub fn meta(mut self, meta: Map<String, Value>) -> Self {
        self.meta = Some(meta);
        self
    }

    /// Published in `_meta` and as the annotations it implies (SPEC §9.1).
    #[must_use]
    pub fn risk(mut self, risk: Risk) -> Self {
        self.risk = Some(risk);
        self
    }

    fn definition(&self, name: &str) -> Value {
        let mut tool = Map::new();
        tool.insert("name".into(), name.into());
        if let Some(title) = &self.title {
            tool.insert("title".into(), title.clone().into());
        }
        if let Some(description) = &self.description {
            tool.insert("description".into(), description.clone().into());
        }
        tool.insert(
            "inputSchema".into(),
            self.input_schema
                .clone()
                .unwrap_or_else(|| json!({ "type": "object" })),
        );
        if let Some(schema) = &self.output_schema {
            tool.insert("outputSchema".into(), schema.clone());
        }
        let annotations = match (self.risk, &self.annotations) {
            (Some(risk), own) => {
                let mut merged = risk.hints();
                merged.extend(own.clone().unwrap_or_default());
                Some(merged)
            }
            (None, own) => own.clone(),
        };
        if let Some(annotations) = annotations {
            tool.insert("annotations".into(), Value::Object(annotations));
        }
        if let Some(icons) = &self.icons {
            tool.insert("icons".into(), icons.clone());
        }
        let meta = match (self.risk, &self.meta) {
            (Some(risk), own) => {
                let mut meta = own.clone().unwrap_or_default();
                meta.insert(RISK_META.into(), risk.name().into());
                Some(meta)
            }
            (None, own) => own.clone(),
        };
        if let Some(meta) = meta {
            tool.insert("_meta".into(), Value::Object(meta));
        }
        Value::Object(tool)
    }
}

/// Data that tried to carry one of the outcome's own members (SPEC §9.2).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutcomeError(pub String);

impl std::fmt::Display for OutcomeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "outcome data cannot carry \"{}\": the outcome's own member (SPEC §9.2)",
            self.0
        )
    }
}

impl std::error::Error for OutcomeError {}

const OUTCOME_MEMBERS: [&str; 3] = ["status", "operation", "reason"];

fn merged(data: Map<String, Value>) -> Result<Map<String, Value>, OutcomeError> {
    match OUTCOME_MEMBERS.iter().find(|key| data.contains_key(**key)) {
        Some(taken) => Err(OutcomeError((*taken).to_owned())),
        None => Ok(data),
    }
}

fn outcome(value: Map<String, Value>, is_error: bool) -> Reply {
    let value = Value::Object(value);
    let mut result = json!({
        "content": [{ "type": "text", "text": value.to_string() }],
        "structuredContent": value,
    });
    if is_error {
        result["isError"] = Value::Bool(true);
    }
    Reply::Result(result)
}

/// The change holds (the application has seen it). `data` is merged in.
pub fn completed(data: Map<String, Value>) -> Result<Reply, OutcomeError> {
    let mut value = merged(data)?;
    value.insert("status".into(), "completed".into());
    Ok(outcome(value, false))
}

/// It started, and goes on as `operation` (its id). `data` is merged in.
pub fn accepted(operation: &str, data: Map<String, Value>) -> Result<Reply, OutcomeError> {
    let mut value = merged(data)?;
    value.insert("status".into(), "accepted".into());
    value.insert("operation".into(), operation.into());
    Ok(outcome(value, false))
}

/// Nothing changed. `code` is for the agent to act on; `detail` for a reader.
#[must_use]
pub fn rejected(code: &str, detail: Option<&str>) -> Reply {
    let mut reason = Map::new();
    reason.insert("code".into(), code.into());
    if let Some(detail) = detail {
        reason.insert("detail".into(), detail.into());
    }
    let mut value = Map::new();
    value.insert("status".into(), "rejected".into());
    value.insert("reason".into(), Value::Object(reason));
    outcome(value, true)
}

// ---------------------------------------------------------------------------
// The connection
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Connecting,
    Online,
    Offline,
    Closed,
}

/// How a connection was closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Close {
    pub code: u16,
    pub reason: String,
}

impl std::fmt::Display for Close {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ClientMCP connection closed: {} ({})",
            self.reason, self.code
        )
    }
}

impl std::error::Error for Close {}

type TokenFuture = Pin<Box<dyn Future<Output = Result<String, String>> + Send>>;
type StatusFn = Arc<dyn Fn(Status, Option<&Close>) + Send + Sync>;
type DialFuture = Pin<Box<dyn Future<Output = std::io::Result<tokio::net::TcpStream>> + Send>>;
type DialFn = Arc<dyn Fn(String, u16) -> DialFuture + Send + Sync>;

/// Where the client token comes from.
#[derive(Clone)]
pub enum TokenSource {
    Fixed(String),
    /// A fresh token for every connection; an unauthorized close is retried.
    Fresh(Arc<dyn Fn() -> TokenFuture + Send + Sync>),
}

#[derive(Clone)]
pub struct Options {
    /// The bridge's client endpoint, e.g. `wss://bridge.example.com/connect`.
    pub url: String,
    pub token: TokenSource,
    /// Guidance for the model on how to use the tools.
    pub instructions: Option<String>,
    /// Called whenever the status changes, with the close that caused it.
    pub on_status: Option<StatusFn>,
    /// Opens the TCP connection to the bridge's host and port, e.g. through
    /// the application's proxy; TLS and the WebSocket handshake follow on it.
    /// By default, a direct connection.
    pub dial: Option<DialFn>,
}

impl Options {
    pub fn new(url: impl Into<String>, token: impl Into<String>) -> Self {
        Options {
            url: url.into(),
            token: TokenSource::Fixed(token.into()),
            instructions: None,
            on_status: None,
            dial: None,
        }
    }

    /// Dial the bridge with `f(host, port)`, e.g. through a proxy.
    #[must_use]
    pub fn dial<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn(String, u16) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = std::io::Result<tokio::net::TcpStream>> + Send + 'static,
    {
        self.dial = Some(Arc::new(move |host, port| Box::pin(f(host, port))));
        self
    }

    #[must_use]
    pub fn instructions(mut self, text: impl Into<String>) -> Self {
        self.instructions = Some(text.into());
        self
    }

    #[must_use]
    pub fn on_status(mut self, f: impl Fn(Status, Option<&Close>) + Send + Sync + 'static) -> Self {
        self.on_status = Some(Arc::new(f));
        self
    }

    /// A fresh token for every connection.
    #[must_use]
    pub fn fresh_token<F, Fut>(mut self, f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String, String>> + Send + 'static,
    {
        self.token = TokenSource::Fresh(Arc::new(move || Box::pin(f())));
        self
    }
}

/// What `connect` waits on.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Phase {
    Pending,
    Online,
    Closed(Close),
}

struct State {
    tools: Vec<(String, Tool)>,
    instructions: Option<String>,
    status: Status,
    attempt: u32,
    /// The running connection loop, while there is one: its serial and stop.
    run: Option<(u64, CancellationToken)>,
    /// Calls in flight, by id: their cancellation and a serial to tell a call
    /// from a later one reusing its id.
    calls: HashMap<String, (CancellationToken, u64)>,
    serial: u64,
}

struct Shared {
    options: Options,
    state: Mutex<State>,
    /// Bumped when the tools or the instructions change.
    declared: watch::Sender<u64>,
    phase: watch::Sender<Phase>,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Sets the status of connection loop `run`, unless that loop has been
    /// closed (or replaced) meanwhile.
    fn set_status(&self, run: u64, status: Status, detail: Option<&Close>) {
        {
            let mut state = self.state();
            let current = state.run.as_ref().is_some_and(|(id, _)| *id == run);
            if !current || (state.status == status && detail.is_none()) {
                return;
            }
            state.status = status;
        }
        self.notify(status, detail);
    }

    /// Tells the application, outside the lock: its callback may call back.
    fn notify(&self, status: Status, detail: Option<&Close>) {
        if let Some(f) = &self.options.on_status {
            f(status, detail);
        }
    }

    fn redeclare(&self) {
        self.declared.send_modify(|n| *n = n.wrapping_add(1));
    }

    fn registration(&self) -> String {
        let state = self.state();
        let tools: Vec<Value> = state
            .tools
            .iter()
            .map(|(name, tool)| tool.definition(name))
            .collect();
        let mut message = json!({ "type": "register", "tools": tools });
        if let Some(instructions) = &state.instructions {
            message["instructions"] = instructions.clone().into();
        }
        message.to_string()
    }

    fn abort_calls(state: &mut State) {
        for (_, (token, _)) in state.calls.drain() {
            token.cancel();
        }
    }

    /// Ends the connection loop `run` (any loop for `None`: the application's
    /// own close) for good.
    fn stop(&self, run: Option<u64>, detail: Close) {
        {
            let mut state = self.state();
            match (&state.run, run) {
                // A loop ending after the application closed it, or replaced
                // it with a new one, has nothing left to close.
                (None, Some(_)) => return,
                (Some((current, _)), Some(id)) if *current != id => return,
                _ => {}
            }
            if let Some((_, stop)) = state.run.take() {
                stop.cancel();
            }
            Self::abort_calls(&mut state);
            state.status = Status::Closed;
            self.phase.send_replace(Phase::Closed(detail.clone()));
        }
        self.notify(Status::Closed, Some(&detail));
    }
}

/// A running application's end of a ClientMCP bridge.
///
/// Dropping it does not disconnect: the connection runs until [`Client::close`].
#[derive(Clone)]
pub struct Client {
    shared: Arc<Shared>,
}

impl Client {
    #[must_use]
    pub fn new(options: Options) -> Self {
        let instructions = options.instructions.clone();
        Client {
            shared: Arc::new(Shared {
                options,
                state: Mutex::new(State {
                    tools: Vec::new(),
                    instructions,
                    status: Status::Offline,
                    attempt: 0,
                    run: None,
                    calls: HashMap::new(),
                    serial: 0,
                }),
                declared: watch::Sender::new(0),
                phase: watch::Sender::new(Phase::Pending),
            }),
        }
    }

    #[must_use]
    pub fn status(&self) -> Status {
        self.shared.state().status
    }

    /// Exposes a tool (replacing one of the same name).
    pub fn expose(&self, name: impl Into<String>, tool: Tool) {
        let name = name.into();
        {
            let mut state = self.shared.state();
            match state.tools.iter_mut().find(|(n, _)| *n == name) {
                Some(entry) => entry.1 = tool,
                None => state.tools.push((name, tool)),
            }
        }
        self.shared.redeclare();
    }

    pub fn unexpose(&self, name: &str) {
        let removed = {
            let mut state = self.shared.state();
            let before = state.tools.len();
            state.tools.retain(|(n, _)| n != name);
            state.tools.len() != before
        };
        if removed {
            self.shared.redeclare();
        }
    }

    pub fn set_instructions(&self, text: Option<String>) {
        self.shared.state().instructions = text;
        self.shared.redeclare();
    }

    /// Connects, and keeps reconnecting. Resolves once online; fails with the
    /// close if the connection is closed for good first. Must be called within
    /// a tokio runtime.
    ///
    /// # Errors
    /// The close after which reconnecting cannot help (or [`Client::close`]).
    pub async fn connect(&self) -> Result<(), Close> {
        let mut phase = self.shared.phase.subscribe();
        let start = {
            let mut state = self.shared.state();
            if state.run.is_some() {
                None
            } else {
                state.serial += 1;
                let run = (state.serial, CancellationToken::new());
                state.run = Some(run.clone());
                self.shared.phase.send_replace(Phase::Pending);
                Some(run)
            }
        };
        if let Some((id, stop)) = start {
            tokio::spawn(run(Arc::clone(&self.shared), id, stop));
        }
        loop {
            match phase.borrow_and_update().clone() {
                Phase::Online => return Ok(()),
                Phase::Closed(close) => return Err(close),
                Phase::Pending => {}
            }
            // The sender lives in `shared`, which `self` holds.
            let _ = phase.changed().await;
        }
    }

    /// Disconnects for good. Calls in flight are cancelled.
    pub fn close(&self) {
        self.shared.stop(
            None,
            Close {
                code: 1000,
                reason: "closed by the application".into(),
            },
        );
    }
}

async fn run(shared: Arc<Shared>, id: u64, stop: CancellationToken) {
    loop {
        shared.set_status(id, Status::Connecting, None);
        let opened = tokio::select! {
            biased;
            () = stop.cancelled() => return,
            opened = async {
                let token = match &shared.options.token {
                    TokenSource::Fixed(token) => token.clone(),
                    TokenSource::Fresh(source) => source().await?,
                };
                open(&shared.options.url, &token, shared.options.dial.as_ref()).await
            } => opened,
        };
        let detail = match opened {
            Err(_) => None,
            Ok(socket) => session(&shared, id, &stop, socket).await,
        };
        if stop.is_cancelled() {
            return; // the application closed it, and has cancelled the calls
        }
        Shared::abort_calls(&mut shared.state());
        if let Some(close) = &detail {
            let fixed = matches!(shared.options.token, TokenSource::Fixed(_));
            let why = fatal(close.code)
                .or((close.code == UNAUTHORIZED && fixed).then_some("unauthorized"));
            if let Some(why) = why {
                let reason = if close.reason.is_empty() {
                    why.to_owned()
                } else {
                    close.reason.clone()
                };
                shared.stop(
                    Some(id),
                    Close {
                        code: close.code,
                        reason,
                    },
                );
                return;
            }
        }
        shared.set_status(id, Status::Offline, detail.as_ref());
        let attempt = {
            let mut state = shared.state();
            let attempt = state.attempt;
            state.attempt = state.attempt.saturating_add(1);
            attempt
        };
        let base = Duration::from_millis(500)
            .saturating_mul(2u32.saturating_pow(attempt.min(16)))
            .min(MAX_BACKOFF);
        let delay = base.mul_f64(0.5 + rand::random::<f64>() / 2.0);
        tokio::select! {
            () = sleep(delay) => {}
            () = stop.cancelled() => return,
        }
    }
}

type Socket =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

/// TLS with the platform's roots and *ring*, set up once. A library does not
/// install a process-wide crypto provider: the application may have its own.
fn tls() -> Result<Arc<rustls::ClientConfig>, String> {
    static CONFIG: OnceLock<Result<Arc<rustls::ClientConfig>, String>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = rustls::RootCertStore::empty();
            roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
            let config = rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map_err(|e| e.to_string())?
            .with_root_certificates(roots)
            .with_no_client_auth();
            Ok(Arc::new(config))
        })
        .clone()
}

async fn open(url: &str, token: &str, dial: Option<&DialFn>) -> Result<Socket, String> {
    let mut request = url.into_client_request().map_err(|e| e.to_string())?;
    let protocols =
        HeaderValue::from_str(&format!("{SUBPROTOCOL}, {token}")).map_err(|e| e.to_string())?;
    request
        .headers_mut()
        .insert("Sec-WebSocket-Protocol", protocols);
    let secure = request.uri().scheme_str() == Some("wss");
    let connector = if secure {
        Some(Connector::Rustls(tls()?))
    } else {
        None
    };
    let (socket, _) = match dial {
        None => tokio_tungstenite::connect_async_tls_with_config(request, None, false, connector)
            .await
            .map_err(|e| e.to_string())?,
        Some(dial) => {
            let host = request.uri().host().unwrap_or_default();
            // `[::1]` in a URL is `::1` to a socket.
            let host = host
                .trim_start_matches('[')
                .trim_end_matches(']')
                .to_owned();
            let port = request
                .uri()
                .port_u16()
                .unwrap_or(if secure { 443 } else { 80 });
            let stream = dial(host, port).await.map_err(|e| e.to_string())?;
            tokio_tungstenite::client_async_tls_with_config(request, stream, None, connector)
                .await
                .map_err(|e| e.to_string())?
        }
    };
    Ok(socket)
}

/// One connection, until it closes (with what it said) or `stop`.
async fn session(
    shared: &Arc<Shared>,
    run: u64,
    stop: &CancellationToken,
    socket: Socket,
) -> Option<Close> {
    let (mut sink, mut stream) = socket.split();
    let (out_tx, mut out_rx) = mpsc::unbounded_channel::<String>();
    // Only changes after this registration need another.
    let mut declared = shared.declared.subscribe();
    if sink
        .send(Message::text(shared.registration()))
        .await
        .is_err()
    {
        return None;
    }
    let mut ping = interval_at(Instant::now() + PING_INTERVAL, PING_INTERVAL);
    ping.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut pong_deadline: Option<Instant> = None;
    loop {
        tokio::select! {
            biased;
            () = stop.cancelled() => {
                let frame = CloseFrame { code: CloseCode::Normal, reason: "closed by the application".into() };
                let _ = sink.send(Message::Close(Some(frame))).await;
                return None;
            }
            incoming = stream.next() => {
                pong_deadline = None;
                match incoming {
                    Some(Ok(Message::Text(text))) => receive(shared, run, &out_tx, text.as_str()),
                    Some(Ok(Message::Close(frame))) => {
                        return Some(frame.map_or(
                            Close { code: 1005, reason: String::new() },
                            |f| Close { code: f.code.into(), reason: f.reason.to_string() },
                        ));
                    }
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => return Some(Close { code: 1006, reason: String::new() }),
                }
            }
            Some(text) = out_rx.recv() => {
                if sink.send(Message::text(text)).await.is_err() {
                    return None;
                }
            }
            Ok(()) = declared.changed() => {
                declared.borrow_and_update();
                if sink.send(Message::text(shared.registration())).await.is_err() {
                    return None;
                }
            }
            _ = ping.tick() => {
                if sink.send(Message::text(PING)).await.is_err() {
                    return None;
                }
                pong_deadline.get_or_insert(Instant::now() + PONG_TIMEOUT);
            }
            () = sleep_until(pong_deadline.unwrap_or_else(Instant::now)), if pong_deadline.is_some() => {
                // Nothing for too long: this connection is gone.
                let _ = sink.close().await;
                return None;
            }
        }
    }
}

fn receive(shared: &Arc<Shared>, run: u64, out: &mpsc::UnboundedSender<String>, text: &str) {
    let Ok(message) = serde_json::from_str::<Value>(text) else {
        return;
    };
    match message.get("type").and_then(Value::as_str) {
        Some("registered") => {
            {
                let mut state = shared.state();
                let current = state.run.as_ref().is_some_and(|(id, _)| *id == run);
                if !current || state.status == Status::Online {
                    return;
                }
                state.attempt = 0;
                state.status = Status::Online;
                shared.phase.send_replace(Phase::Online);
            }
            shared.notify(Status::Online, None);
        }
        Some("call") => {
            let Some(id) = message.get("id").and_then(Value::as_str).map(str::to_owned) else {
                return;
            };
            let name = match message.get("name") {
                Some(Value::String(name)) => name.clone(),
                Some(other) => other.to_string(),
                None => String::new(),
            };
            let args = message
                .get("arguments")
                .filter(|args| !args.is_null())
                .cloned()
                .unwrap_or_else(|| json!({}));
            call(shared, out, id, name, args);
        }
        Some("cancel") => {
            if let Some(id) = message.get("id").and_then(Value::as_str)
                && let Some((token, _)) = shared.state().calls.remove(id)
            {
                token.cancel();
            }
        }
        _ => {}
    }
}

fn call(
    shared: &Arc<Shared>,
    out: &mpsc::UnboundedSender<String>,
    id: String,
    name: String,
    args: Value,
) {
    let token = CancellationToken::new();
    let (tool, serial) = {
        let mut state = shared.state();
        state.serial += 1;
        let serial = state.serial;
        state.calls.insert(id.clone(), (token.clone(), serial));
        let tool = state
            .tools
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, tool)| tool.clone());
        (tool, serial)
    };
    let shared = Arc::clone(shared);
    let out = out.clone();
    tokio::spawn(async move {
        let reply = match tool {
            None => Err(format!("Unknown tool: {name}")),
            Some(tool) => {
                let ctx = CallContext {
                    name: name.clone(),
                    cancelled: token.clone(),
                };
                tokio::select! {
                    biased;
                    // Cancelled, or the connection ended: the work stops here.
                    () = token.cancelled() => return,
                    reply = (tool.handler)(args, ctx) => reply,
                }
            }
        };
        {
            let mut state = shared.state();
            match state.calls.get(&id) {
                Some((_, s)) if *s == serial => {
                    state.calls.remove(&id);
                }
                _ => return,
            }
        }
        let result = match reply {
            Ok(reply) => reply.into_result(),
            Err(text) => json!({ "content": [{ "type": "text", "text": text }], "isError": true }),
        };
        let mut message = json!({ "type": "result", "id": id, "result": result }).to_string();
        if message.len() > MAX_MESSAGE_BYTES {
            let text = format!(
                "The tool result is too large to deliver (over {MAX_MESSAGE_BYTES} bytes)."
            );
            message = json!({
                "type": "result",
                "id": id,
                "result": { "content": [{ "type": "text", "text": text }], "isError": true },
            })
            .to_string();
        }
        let _ = out.send(message);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_risk_is_published_and_implies_annotations_the_tool_may_override() {
        let mut own = Map::new();
        own.insert("title".into(), "Wipe".into());
        let tool = Tool::new(|_, _| async { Ok(Reply::Empty) })
            .risk(Risk::Manage)
            .annotations(own);
        let definition = tool.definition("wipe");
        assert_eq!(
            definition["annotations"],
            json!({ "readOnlyHint": false, "destructiveHint": true, "title": "Wipe" })
        );
        assert_eq!(definition["_meta"], json!({ RISK_META: "manage" }));
        assert_eq!(definition["inputSchema"], json!({ "type": "object" }));
        let look = Tool::new(|_, _| async { Ok(Reply::Empty) }).risk(Risk::Observe);
        assert_eq!(
            look.definition("look")["annotations"],
            json!({ "readOnlyHint": true })
        );
    }

    #[test]
    fn outcomes_have_their_shape_and_their_members_are_reserved() {
        let mut data = Map::new();
        data.insert("transfer".into(), json!({ "id": "op-1" }));
        let Ok(Reply::Result(paid)) = accepted("op-1", data) else {
            panic!("an outcome")
        };
        assert_eq!(
            paid["structuredContent"],
            json!({ "transfer": { "id": "op-1" }, "status": "accepted", "operation": "op-1" })
        );
        let mut clash = Map::new();
        clash.insert("operation".into(), json!({}));
        assert_eq!(
            accepted("op-2", clash),
            Err(OutcomeError("operation".into()))
        );
        let mut status = Map::new();
        status.insert("status".into(), "done".into());
        assert!(completed(status).is_err());
        let Reply::Result(no) = rejected("denied", Some("the user said no")) else {
            panic!("an outcome")
        };
        assert_eq!(no["isError"], true);
        assert_eq!(
            no["structuredContent"],
            json!({ "status": "rejected", "reason": { "code": "denied", "detail": "the user said no" } })
        );
    }

    #[test]
    fn a_reply_becomes_a_call_tool_result_as_the_js_sdk_makes_it() {
        assert_eq!(Reply::Empty.into_result(), json!({ "content": [] }));
        assert_eq!(
            Reply::text("hi").into_result(),
            json!({ "content": [{ "type": "text", "text": "hi" }] })
        );
        let object = Reply::Json(json!({ "a": 1 })).into_result();
        assert_eq!(object["structuredContent"], json!({ "a": 1 }));
        let list = Reply::Json(json!([1, 2])).into_result();
        assert!(list.get("structuredContent").is_none(), "only objects");
    }
}
