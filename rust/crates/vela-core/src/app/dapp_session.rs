//! Machine — dApp connection session lifecycle (spec `017`, inventory
//! `### dapp_session (P2)`).
//!
//! ```text
//! input ─► classify (5-way, pure) ─┬─ walletpair: ─► prepare ─► PendingFingerprint ─► confirm ─► join ─► connected
//!                                  ├─ remote-inject ─► connect ─► connected
//!                                  ├─ full http(s) url ──┐
//!                                  ├─ bare host ─────────┴─► OpenBrowser (session untouched)
//!                                  └─ invalid ─► AlertInvalidLink
//!
//! connected ── blip ─► grace 4s (never extended) ─► "reconnecting" ─► stuck 45s prompt
//!            └ drop ──► deadline 60s error banner  +  backoff 1s·2ⁿ ≤ 30s reconnects
//! launch ─► restore: remote-inject FIRST, walletpair second ─► dead channel? drop + wipe (8s)
//! ```
//!
//! Faithful port of the TypeScript sources — behavior aligned line by line:
//!
//! - `src/models/dapp-connection.tsx:54-620, 963-1027` — the connection FSM,
//!   the 4s reconnect grace (`:64, :433-444`), the 45s stuck prompt
//!   (`:237-242`), the 120s join watchdog (`:534-552`), the mount-time
//!   restore loop and 8s `dropIfDead` (`:963-1027`, BUG-5/6), pending-pairing
//!   key release (`:461-474, :563-572`)
//! - `src/services/walletpair-transport.ts` — the 60s reconnect deadline
//!   (`RECONNECT_MAX_MS`), exponential backoff (`:484-496`), foreground/web
//!   recovery (`:388-474`), the plaintext-vs-CAIP-2 chain guard (`:50-81`)
//! - `src/services/walletpair-protocol.ts:372-437, 694-697` — read-only:
//!   fingerprint gating (`confirmJoin` refuses an unapproved pairing) and the
//!   counter-durability rule ([`DsessOperation::PersistWalletPairCounters`]
//!   sequences it at the command level; the per-seal persistence stays in the
//!   shell's live session object)
//! - `src/services/dapp-transport.ts:262-334` — `parseRemoteInjectURL`,
//!   `coerceBrowserUrl`, and the ordering contract (a remote-inject link IS an
//!   https URL, so remote-inject must be tried before the browser fallback)
//! - `src/screens/connect/ConnectScreen.tsx:46-79` — the entry decision tree
//!
//! Inventory invariants ① – ⑨, each pinned by at least one test in
//! `tests/app_dapp_session.rs`:
//!
//! ① a fingerprint never confirmed never becomes a session — the join op is
//!   reachable only through [`Event::FingerprintConfirmed`];
//! ② cancelling or replacing a pending pairing issues an explicit
//!   `DisconnectTransport` for the pending handle — the operation-level
//!   declaration that the ephemeral X25519 key must be released;
//! ③ repeated blips never extend the grace window (armed-once), and a manual
//!   reconnect bypasses it entirely;
//! ④ reconnecting never spins unbounded — 45s stuck prompt + 60s deadline;
//! ⑤ a restored session whose channel is dead is dropped AND its snapshot
//!   wiped (8s `dropIfDead`), so the next launch starts clean;
//! ⑥ remote-inject is restored before walletpair; a failed restore cleans up
//!   silently;
//! ⑦ counters are persisted before any ciphertext-producing push — the
//!   `PersistWalletPairCounters → ack → PushWalletInfo` command sequence; a
//!   failed persist never pushes (mirrors `abandonUnsafeSession`);
//! ⑧ a plaintext `chainId` disagreeing with the encrypted CAIP-2 context is
//!   rejected ([`assert_request_chain_context`], pure);
//! ⑨ classification order is fixed: walletpair → remote-inject → browser →
//!   invalid.
//!
//! Ported quirks, kept verbatim (doc'd at their sites):
//!
//! - a FRESH remote-inject connect failure leaks the transport (TS only nulls
//!   the ref; the restore path does disconnect);
//! - a failed WalletPair join/join-timeout ends `disconnected`, not `error` —
//!   the TS `setStatus('error')` is synchronously clobbered by the
//!   `disconnected` handler the `transport.disconnect()` call triggers;
//! - `errorMessage` is NOT cleared on recovery (`connected` handler never
//!   touches it) nor on `disconnectBridge`;
//! - a terminal transport drop keeps `session`/`dappInfo` in the model (only
//!   an explicit disconnect clears them);
//! - foregrounding a still-`connected` session after ≥ 20s backgrounded does
//!   NOTHING (the TS branch reconnects only from phase `'disconnected'` and
//!   skips the ping on the stale branch);
//! - a failed MANUAL reconnect schedules no backoff (`transport.reconnect()`
//!   cancelled the pending backoff timer and its catch is empty) — only an
//!   armed backoff timer or a fresh transport drop resumes auto-retry;
//! - `caip2_to_chain_id` slices 7 chars without checking the `eip155:` prefix
//!   and parses with `parseInt` leading-digits semantics.
//!
//! Deliberate deviations, all fail-closed (JS has no Rust equivalent here):
//!
//! - the shell must not deliver events for released handles; TS zombie
//!   listeners (a leaked transport still calling `setErrorMessage`) are
//!   dropped by the `session_ref` staleness guard instead;
//! - `RestoreLoaded` is single-shot and ignored while any session or pairing
//!   is active (the TS effect would silently clobber a live transport ref);
//! - the URL mini-parser rejects embedded whitespace instead of stripping
//!   tab/CR/LF the way WHATWG does, and skips punycode/percent-encoding
//!   normalization — inputs needing either are rejected, never misparsed.
//!
//! Nothing cryptographic crosses this boundary: the core holds only numeric
//! transport handles, a phase, and the remote-inject relay credentials that
//! already live in plain-JSON AsyncStorage today (`vela.remoteInjectSession`;
//! the inventory's recorded security debt). The WalletPair X25519 key pair,
//! message counters and encrypted snapshot never leave the shell.

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Constants — every timer in the densest timer module of the app
// ---------------------------------------------------------------------------

/// `RECONNECT_GRACE_MS` — a relay blip usually self-heals within ~1s; hold
/// "connected" this long before surfacing "Reconnecting…".
pub const RECONNECT_GRACE_MS: u32 = 4_000;
/// The stuck prompt: an auto-reconnect dragging past this offers manual help.
pub const RECONNECT_STUCK_MS: u32 = 45_000;
/// The join watchdog for a relay that silently drops the join message.
pub const JOIN_TIMEOUT_MS: u32 = 120_000;
/// `RECONNECT_MAX_MS` — stop pretending the transport is about to come back.
pub const RECONNECT_DEADLINE_MS: u32 = 60_000;
/// A restored session that is not live this soon after its reconnect attempt
/// has a dead channel (real reconnects settle well under; dead channels 404
/// fast) — BUG-5/6.
pub const DROP_IF_DEAD_MS: u32 = 8_000;
pub const BACKOFF_BASE_MS: u32 = 1_000;
pub const BACKOFF_CAP_MS: u32 = 30_000;
/// Web recovery throttle: `online` + `visibilitychange` firing together must
/// not double-reconnect.
pub const RECOVER_THROTTLE_MS: f64 = 3_000.0;
/// Backgrounded at least this long ⇒ the relay has almost certainly
/// idle-closed the socket.
pub const STALE_AFTER_MS: f64 = 20_000.0;

/// `Number.MAX_SAFE_INTEGER` — the JS `isSafeInteger` bound the chain-id
/// guards mirror.
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// Remote-inject relay credentials (`RemoteInjectSession`). These are plain
/// bearer credentials persisted verbatim as JSON in AsyncStorage today — NOT
/// WalletPair cryptographic material, which never enters the core.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DsessRemoteInjectSession {
    pub server_url: String,
    pub session_id: String,
    pub nonce: String,
    pub secret: String,
}

/// dApp metadata (`DAppInfo`) — name, url, optional icon.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DsessDappInfo {
    pub name: String,
    pub url: String,
    pub icon: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DsessConnectionType {
    RemoteInject,
    WalletPair,
}

/// `ConnectionStatus`, exactly today's five values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DsessStatus {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DsessTimerKind {
    Grace,
    Stuck,
    Join,
    Deadline,
    DropIfDead,
    Backoff,
}

/// Why a reconnect was requested. Echoed back on the result so the core can
/// apply the path-specific failure rule (backoff reschedules, restore drops
/// the dead channel, manual/foreground/online do nothing).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DsessReconnectCause {
    Manual,
    Backoff,
    Restore,
    Foreground,
    Online,
}

/// The error surface. Shell-reported text rides verbatim; core-originated
/// watchdogs are semantic so the shell owns the words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DsessError {
    /// A transport/connect failure message, verbatim from the shell.
    Transport { message: String },
    /// 120s join watchdog — "Connection timed out. The relay may be
    /// unavailable — try scanning again."
    JoinTimeout,
    /// 60s reconnect deadline — "Still trying to reconnect to the dApp…".
    ReconnectDeadline,
}

// ---------------------------------------------------------------------------
// Protocol — operations
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. The shell holds every live
/// transport object keyed by `session_ref`; the core never sees keys,
/// counters or snapshots. A `session_ref` the shell no longer knows is a
/// no-op (released handles must go quiet — see the module doc's deviations).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DsessOperation"))]
pub enum DsessOperation {
    /// `WalletPairTransport.prepare(uri)` — parse the pairing URI, mint the
    /// ephemeral X25519 pair (shell-side), answer with the 4-digit
    /// fingerprint and the dApp metadata.
    PrepareWalletPair {
        session_ref: u32,
        uri: String,
    },
    /// Build a `RemoteInjectTransport` for `session` and `connect()` it.
    ConnectRemoteInject {
        session_ref: u32,
        session: DsessRemoteInjectSession,
    },
    /// `confirmJoin()` — only ever issued after the user confirmed the
    /// fingerprint (invariant ①).
    ConfirmWalletPairJoin {
        session_ref: u32,
    },
    /// `transport.reconnect?.()`. The shell echoes `cause` on the result.
    ReconnectTransport {
        session_ref: u32,
        cause: DsessReconnectCause,
    },
    /// `transport.disconnect()` — for a pending pairing this is the explicit
    /// release of the ephemeral key material (invariant ②).
    DisconnectTransport {
        session_ref: u32,
    },
    /// `session.ping()` — brief-blur foreground check.
    PingTransport {
        session_ref: u32,
    },
    /// `pushWalletInfo` — the shell composes address/name/accounts from the
    /// wallet state; the core supplies the chain. For WalletPair this is only
    /// ever issued after `PersistWalletPairCounters` acknowledged (⑦).
    PushWalletInfo {
        session_ref: u32,
        chain_id: u32,
    },
    /// Persist the WalletPair session snapshot (counters included) BEFORE any
    /// ciphertext for the next nonce may be produced (invariant ⑦). The
    /// per-seal persistence inside the protocol object stays in the shell;
    /// this operation states the ordering contract at the command level.
    PersistWalletPairCounters {
        session_ref: u32,
    },
    /// `WalletPairTransport.restore()` — load + validate the snapshot,
    /// rebuild the live session shell-side.
    RestoreWalletPair {
        session_ref: u32,
    },
    /// `transport.fetchDAppInfo()`.
    FetchDappInfo {
        session_ref: u32,
    },
    /// `vela.remoteInjectSession` writes.
    SaveRemoteInjectSession {
        session: DsessRemoteInjectSession,
    },
    ClearRemoteInjectSession,
    /// `vela.walletpairSession` delete — the dead-channel wipe (⑤).
    ClearWalletPairSnapshot,
    StartTimer {
        id: u32,
        kind: DsessTimerKind,
        ms: u32,
    },
    CancelTimer {
        id: u32,
    },
    /// Route a plain web address to the in-app dApp browser.
    OpenBrowser {
        url: String,
    },
    /// `showAlert(connect.list.invalidLink…)` — the shell owns the words.
    AlertInvalidLink,
}

// ---------------------------------------------------------------------------
// Protocol — results
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DsessShellResult"))]
pub enum DsessShellResult {
    /// Fire-and-forget acknowledged (timers, disconnects, storage, pushes…).
    Ack,
    WalletPairPrepared {
        fingerprint: String,
        dapp: DsessDappInfo,
    },
    WalletPairPrepareFailed {
        message: String,
    },
    /// `connect()` resolved for the remote-inject transport.
    RemoteInjectConnectFinished,
    RemoteInjectConnectFailed {
        message: String,
    },
    /// `confirmJoin()` resolved; `connected` is `transport.connected` at that
    /// moment — `false` arms the 120s join watchdog exactly as today.
    JoinFinished {
        connected: bool,
    },
    JoinFailed {
        message: String,
    },
    ReconnectFinished {
        cause: DsessReconnectCause,
        ok: bool,
    },
    /// `restore()` outcome. `false` covers both a malformed snapshot and a
    /// key-mismatch — the shell already refused to build the session.
    WalletPairRestoreFinished {
        restored: bool,
    },
    DappInfoFetched {
        info: Option<DsessDappInfo>,
    },
    CountersPersisted {
        ok: bool,
    },
}

impl Operation for DsessOperation {
    type Output = DsessShellResult;
}

#[effect]
pub enum DsessEffect {
    Render(RenderOperation),
    Shell(DsessOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DsessEvent"))]
pub enum Event {
    /// Scan / paste / typed input. Classified in the core (invariant ⑨).
    InputSubmitted {
        raw: String,
    },
    /// The user compared and accepted the 4-digit code (invariant ①).
    FingerprintConfirmed,
    FingerprintCancelled,
    /// `disconnectBridge` — tears the session down and wipes both stores.
    DisconnectRequested,
    /// "Reconnect now" — bypasses the grace window (invariant ③).
    ManualReconnect,
    /// Mount-time restore. The shell reads BOTH stores and reports what
    /// exists; the CORE picks remote-inject first (invariant ⑥). Only the
    /// walletpair snapshot's plain dApp metadata crosses — never the snapshot.
    RestoreLoaded {
        remote_inject: Option<DsessRemoteInjectSession>,
        wallet_pair: Option<DsessDappInfo>,
    },
    /// Transport phase events, keyed by handle. Stale refs are dropped.
    TransportConnected {
        session_ref: u32,
    },
    /// Terminal drop (remote-inject SSE loss, walletpair `closed`).
    TransportDisconnected {
        session_ref: u32,
    },
    /// Transport-level drop with the session still recoverable (walletpair).
    TransportReconnecting {
        session_ref: u32,
    },
    TransportError {
        session_ref: u32,
        message: String,
    },
    TimerFired {
        id: u32,
    },
    /// Mobile foreground (`AppState` → `active`).
    AppForegrounded {
        backgrounded_ms: f64,
    },
    /// Web recovery signal (`online` OR tab became visible) — the 3s throttle
    /// lives here, which is why the epoch rides on the event.
    NetworkOnline {
        now_ms: f64,
    },
    /// Wallet state changed (account/chain/name/accounts) — push while
    /// connected, exactly today's effect.
    WalletChanged {
        chain_id: u32,
    },
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: DsessShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// The live transport's protocol-level phase, the core's proxy for
/// `session.phase` / `transport.connected`:
/// `Joining` = pre-join (`idle`/`awaiting_confirmation`, or a remote-inject
/// connect in flight), `Up` = `connected`, `Down` = `disconnected` (session
/// alive, socket gone — the only phase a reconnect is legal from).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum TransportPhase {
    #[default]
    Joining,
    Up,
    Down,
}

#[derive(Debug, Default)]
struct Timers {
    grace: Option<u32>,
    stuck: Option<u32>,
    /// `(timer id, session_ref it watches)`.
    join: Option<(u32, u32)>,
    deadline: Option<u32>,
    drop_dead: Option<(u32, u32)>,
    backoff: Option<u32>,
}

#[derive(Debug)]
pub struct Model {
    status: DsessStatus,
    error: Option<DsessError>,
    session: Option<DsessRemoteInjectSession>,
    dapp_info: Option<DsessDappInfo>,
    connection_type: Option<DsessConnectionType>,
    pending_fingerprint: Option<String>,
    reconnect_stuck: bool,
    chain_id: u32,

    live_ref: Option<u32>,
    live_kind: Option<DsessConnectionType>,
    /// A prepared-but-unconfirmed pairing — holds an ephemeral key shell-side.
    pending_wp_ref: Option<u32>,
    awaiting_prepare: Option<u32>,
    awaiting_restore: Option<u32>,
    pending_wp_restore: Option<DsessDappInfo>,
    /// Restore-path session, committed to `session` only on connect success
    /// (the fresh path commits immediately — both are today's order).
    restoring_session: Option<DsessRemoteInjectSession>,
    ri_restore: bool,
    transport_phase: TransportPhase,
    backoff_attempt: u32,
    last_recover_at: f64,
    restore_consumed: bool,

    timers: Timers,
    next_timer: u32,
    next_ref: u32,
    attempt: u64,
}

impl Default for Model {
    fn default() -> Self {
        Self {
            status: DsessStatus::Disconnected,
            error: None,
            session: None,
            dapp_info: None,
            connection_type: None,
            pending_fingerprint: None,
            reconnect_stuck: false,
            // `useState(1)` — mainnet until a request/switch says otherwise.
            chain_id: 1,
            live_ref: None,
            live_kind: None,
            pending_wp_ref: None,
            awaiting_prepare: None,
            awaiting_restore: None,
            pending_wp_restore: None,
            restoring_session: None,
            ri_restore: false,
            transport_phase: TransportPhase::Joining,
            backoff_attempt: 0,
            last_recover_at: 0.0,
            restore_consumed: false,
            timers: Timers::default(),
            next_timer: 1,
            next_ref: 1,
            attempt: 0,
        }
    }
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DsessView {
    pub status: DsessStatus,
    pub error: Option<DsessError>,
    pub session: Option<DsessRemoteInjectSession>,
    pub dapp_info: Option<DsessDappInfo>,
    pub connection_type: Option<DsessConnectionType>,
    pub pending_fingerprint: Option<String>,
    pub reconnect_stuck: bool,
    pub chain_id: u32,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

type Cmd = Command<DsessEffect, Event>;

#[derive(Default)]
pub struct DappSession;

impl App for DappSession {
    type Event = Event;
    type Model = Model;
    type ViewModel = DsessView;
    type Effect = DsessEffect;

    fn update(&self, event: Event, model: &mut Model) -> Cmd {
        match event {
            Event::InputSubmitted { raw } => on_input(model, &raw),
            Event::FingerprintConfirmed => on_confirm_fingerprint(model),
            Event::FingerprintCancelled => on_cancel_fingerprint(model),
            Event::DisconnectRequested => on_disconnect_requested(model),
            Event::ManualReconnect => on_manual_reconnect(model),
            Event::RestoreLoaded {
                remote_inject,
                wallet_pair,
            } => on_restore_loaded(model, remote_inject, wallet_pair),
            Event::TransportConnected { session_ref } => on_transport_connected(model, session_ref),
            Event::TransportDisconnected { session_ref } => {
                on_transport_disconnected(model, session_ref)
            }
            Event::TransportReconnecting { session_ref } => {
                on_transport_reconnecting(model, session_ref)
            }
            Event::TransportError {
                session_ref,
                message,
            } => on_transport_error(model, session_ref, message),
            Event::TimerFired { id } => on_timer_fired(model, id),
            Event::AppForegrounded { backgrounded_ms } => {
                on_app_foregrounded(model, backgrounded_ms)
            }
            Event::NetworkOnline { now_ms } => on_network_online(model, now_ms),
            Event::WalletChanged { chain_id } => on_wallet_changed(model, chain_id),
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A superseded flow's result — a stale prepare, a dead
                    // transport's connect. Dropping it IS the staleness rule.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> DsessView {
        DsessView {
            status: model.status,
            error: model.error.clone(),
            session: model.session.clone(),
            dapp_info: model.dapp_info.clone(),
            connection_type: model.connection_type,
            pending_fingerprint: model.pending_fingerprint.clone(),
            reconnect_stuck: model.reconnect_stuck,
            chain_id: model.chain_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Event handlers
// ---------------------------------------------------------------------------

fn on_input(model: &mut Model, raw: &str) -> Cmd {
    match classify_connect_input(raw) {
        DsessInput::WalletPair { uri } => {
            model.attempt += 1;
            let mut ops = Vec::new();
            disconnect_current(model, &mut ops);
            set_status(model, &mut ops, DsessStatus::Connecting);
            model.error = None;
            model.session = None;
            // dapp_info deliberately NOT cleared — ported verbatim (the old
            // dApp's card shows until the new prepare overwrites it).
            let session_ref = alloc_ref(model);
            model.awaiting_prepare = Some(session_ref);
            ops.push(req(
                model.attempt,
                DsessOperation::PrepareWalletPair { session_ref, uri },
            ));
            finish(ops)
        }
        DsessInput::RemoteInject { session } => {
            model.attempt += 1;
            let mut ops = Vec::new();
            disconnect_current(model, &mut ops);
            set_status(model, &mut ops, DsessStatus::Connecting);
            model.error = None;
            model.session = Some(session.clone());
            model.ri_restore = false;
            model.restoring_session = None;
            let session_ref = alloc_ref(model);
            model.live_ref = Some(session_ref);
            model.live_kind = Some(DsessConnectionType::RemoteInject);
            model.transport_phase = TransportPhase::Joining;
            ops.push(req(
                model.attempt,
                DsessOperation::ConnectRemoteInject {
                    session_ref,
                    session,
                },
            ));
            finish(ops)
        }
        // A web address never touches the live session (ConnectScreen routes
        // to /browser and returns).
        DsessInput::Browser { url } => {
            Command::all([req(model.attempt, DsessOperation::OpenBrowser { url })])
        }
        DsessInput::Invalid => Command::all([req(model.attempt, DsessOperation::AlertInvalidLink)]),
    }
}

fn on_confirm_fingerprint(model: &mut Model) -> Cmd {
    // `if (!transport) return` — no pending pairing, nothing to confirm.
    let Some(session_ref) = model.pending_wp_ref.take() else {
        return Command::done();
    };
    model.pending_fingerprint = None;
    model.live_ref = Some(session_ref);
    model.live_kind = Some(DsessConnectionType::WalletPair);
    model.transport_phase = TransportPhase::Joining;
    let ops = vec![req(
        model.attempt,
        DsessOperation::ConfirmWalletPairJoin { session_ref },
    )];
    finish(ops)
}

fn on_cancel_fingerprint(model: &mut Model) -> Cmd {
    let mut ops = Vec::new();
    // prepare() minted a wallet identity even though the relay never accepted
    // the join — explicit cancellation must release it (invariant ②).
    if let Some(session_ref) = model.pending_wp_ref.take() {
        ops.push(req(
            model.attempt,
            DsessOperation::DisconnectTransport { session_ref },
        ));
    }
    model.pending_fingerprint = None;
    set_status(model, &mut ops, DsessStatus::Disconnected);
    model.dapp_info = None;
    model.error = None;
    finish(ops)
}

fn on_disconnect_requested(model: &mut Model) -> Cmd {
    model.attempt += 1;
    let mut ops = Vec::new();
    disconnect_current(model, &mut ops);
    set_status(model, &mut ops, DsessStatus::Disconnected);
    model.connection_type = None;
    model.session = None;
    model.dapp_info = None;
    // errorMessage deliberately NOT cleared — ported verbatim.
    ops.push(req(model.attempt, DsessOperation::ClearRemoteInjectSession));
    ops.push(req(model.attempt, DsessOperation::ClearWalletPairSnapshot));
    finish(ops)
}

fn on_manual_reconnect(model: &mut Model) -> Cmd {
    let Some(session_ref) = model.live_ref else {
        return Command::done();
    };
    let mut ops = Vec::new();
    // Manual tap → show "Reconnecting…" NOW, don't wait out the grace (③).
    if let Some(id) = model.timers.grace.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if model.status == DsessStatus::Reconnecting {
        // reconnectNonce bump: re-arm the stuck timer even though the status
        // value does not change.
        model.reconnect_stuck = false;
        if let Some(id) = model.timers.stuck.take() {
            ops.push(cancel_op(model.attempt, id));
        }
        let id = start_timer(model, &mut ops, DsessTimerKind::Stuck, RECONNECT_STUCK_MS);
        model.timers.stuck = Some(id);
    } else {
        set_status(model, &mut ops, DsessStatus::Reconnecting);
    }
    ops.push(req(
        model.attempt,
        DsessOperation::ReconnectTransport {
            session_ref,
            cause: DsessReconnectCause::Manual,
        },
    ));
    finish(ops)
}

fn on_restore_loaded(
    model: &mut Model,
    remote_inject: Option<DsessRemoteInjectSession>,
    wallet_pair: Option<DsessDappInfo>,
) -> Cmd {
    // Single-shot, and never while anything is live — fail-closed deviation
    // (the TS effect would clobber a live transport ref; see module doc).
    if model.restore_consumed
        || model.live_ref.is_some()
        || model.pending_wp_ref.is_some()
        || model.awaiting_prepare.is_some()
        || model.awaiting_restore.is_some()
        || model.status != DsessStatus::Disconnected
    {
        return Command::done();
    }
    model.restore_consumed = true;
    // Remote-inject first (invariant ⑥) — when it exists, the walletpair
    // snapshot is left exactly as-is (today's early `return`).
    if let Some(session) = remote_inject {
        model.attempt += 1;
        let session_ref = alloc_ref(model);
        model.live_ref = Some(session_ref);
        model.live_kind = Some(DsessConnectionType::RemoteInject);
        model.transport_phase = TransportPhase::Joining;
        model.ri_restore = true;
        model.restoring_session = Some(session.clone());
        let ops = vec![req(
            model.attempt,
            DsessOperation::ConnectRemoteInject {
                session_ref,
                session,
            },
        )];
        return finish(ops);
    }
    if wallet_pair.is_some() {
        model.attempt += 1;
        let session_ref = alloc_ref(model);
        model.awaiting_restore = Some(session_ref);
        model.pending_wp_restore = wallet_pair;
        let ops = vec![req(
            model.attempt,
            DsessOperation::RestoreWalletPair { session_ref },
        )];
        return finish(ops);
    }
    Command::done()
}

fn on_transport_connected(model: &mut Model, session_ref: u32) -> Cmd {
    if model.live_ref != Some(session_ref) || model.live_kind.is_none() {
        return Command::done();
    }
    model.transport_phase = TransportPhase::Up;
    let mut ops = Vec::new();
    // Recovered (possibly within the grace window) — cancel the pending
    // "Reconnecting…" flip so a self-healing blip never showed at all.
    if let Some(id) = model.timers.grace.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some((id, _)) = model.timers.join.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some(id) = model.timers.deadline.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some(id) = model.timers.backoff.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    model.backoff_attempt = 0;
    set_status(model, &mut ops, DsessStatus::Connected);
    model.connection_type = model.live_kind;
    // errorMessage deliberately NOT cleared — ported verbatim.
    push_wallet_info(model, &mut ops);
    finish(ops)
}

fn on_transport_disconnected(model: &mut Model, session_ref: u32) -> Cmd {
    if model.live_ref != Some(session_ref) {
        return Command::done();
    }
    let mut ops = Vec::new();
    // session + dapp_info survive — only an explicit disconnect clears them
    // (ported verbatim).
    drop_live_transport(model, &mut ops, false, false);
    finish(ops)
}

fn on_transport_reconnecting(model: &mut Model, session_ref: u32) -> Cmd {
    if model.live_ref != Some(session_ref) {
        return Command::done();
    }
    model.transport_phase = TransportPhase::Down;
    let mut ops = Vec::new();
    // Grace: hold "connected" for 4s; an already-pending timer is left to run
    // — repeated blips never extend the window (invariant ③).
    if model.timers.grace.is_none() {
        let id = start_timer(model, &mut ops, DsessTimerKind::Grace, RECONNECT_GRACE_MS);
        model.timers.grace = Some(id);
    }
    // Deadline: already counting down this episode ⇒ don't restart.
    if model.timers.deadline.is_none() {
        let id = start_timer(
            model,
            &mut ops,
            DsessTimerKind::Deadline,
            RECONNECT_DEADLINE_MS,
        );
        model.timers.deadline = Some(id);
    }
    schedule_backoff(model, &mut ops);
    finish(ops)
}

fn on_transport_error(model: &mut Model, session_ref: u32, message: String) -> Cmd {
    if model.live_ref != Some(session_ref) {
        return Command::done();
    }
    // `setErrorMessage(msg)` only — status untouched (a 60s-deadline error
    // coexists with "reconnecting", exactly as today).
    model.error = Some(DsessError::Transport { message });
    render()
}

fn on_timer_fired(model: &mut Model, id: u32) -> Cmd {
    if model.timers.grace == Some(id) {
        model.timers.grace = None;
        let mut ops = Vec::new();
        set_status(model, &mut ops, DsessStatus::Reconnecting);
        return finish(ops);
    }
    if model.timers.stuck == Some(id) {
        model.timers.stuck = None;
        if model.status == DsessStatus::Reconnecting {
            model.reconnect_stuck = true;
            return render();
        }
        return Command::done();
    }
    if let Some((timer, watched)) = model.timers.join {
        if timer == id {
            model.timers.join = None;
            // `!transport.connected && transportRef.current === transport`
            if model.transport_phase != TransportPhase::Up && model.live_ref == Some(watched) {
                model.error = Some(DsessError::JoinTimeout);
                let mut ops = Vec::new();
                // Ends `disconnected`, not `error` — the TS `setStatus('error')`
                // is clobbered by the disconnect handler (ported verbatim).
                drop_live_transport(model, &mut ops, true, false);
                return finish(ops);
            }
            return Command::done();
        }
    }
    if model.timers.deadline == Some(id) {
        model.timers.deadline = None;
        if model.transport_phase == TransportPhase::Up {
            // Recovered in the meantime.
            return Command::done();
        }
        // Surface a recoverable error; the session is kept and the status
        // stays wherever it is — the UI merely stops promising recovery.
        model.error = Some(DsessError::ReconnectDeadline);
        return render();
    }
    if model.timers.backoff == Some(id) {
        model.timers.backoff = None;
        let (Some(session_ref), Some(DsessConnectionType::WalletPair)) =
            (model.live_ref, model.live_kind)
        else {
            return Command::done();
        };
        if model.transport_phase != TransportPhase::Down {
            return Command::done();
        }
        let mut ops = Vec::new();
        // `transport.reconnect()` emits 'reconnecting' — re-arm the grace
        // debounce if the previous window already elapsed.
        if model.timers.grace.is_none() {
            let gid = start_timer(model, &mut ops, DsessTimerKind::Grace, RECONNECT_GRACE_MS);
            model.timers.grace = Some(gid);
        }
        ops.push(req(
            model.attempt,
            DsessOperation::ReconnectTransport {
                session_ref,
                cause: DsessReconnectCause::Backoff,
            },
        ));
        return finish(ops);
    }
    if let Some((timer, watched)) = model.timers.drop_dead {
        if timer == id {
            model.timers.drop_dead = None;
            // A restored session that is not live by now can NEVER come back:
            // drop it AND clear the snapshot so the next launch starts clean
            // and a live reconnect to a dead channel can't collide with a
            // fresh pairing on the relay (BUG-5/6, invariant ⑤).
            if model.live_ref == Some(watched) && model.transport_phase != TransportPhase::Up {
                let mut ops = Vec::new();
                drop_live_transport(model, &mut ops, true, true);
                return finish(ops);
            }
            return Command::done();
        }
    }
    // A cancelled or superseded timer — stale by id, dropped.
    Command::done()
}

fn on_app_foregrounded(model: &mut Model, backgrounded_ms: f64) -> Cmd {
    // AppState recovery exists only on the WalletPair transport.
    if model.live_kind != Some(DsessConnectionType::WalletPair) {
        return Command::done();
    }
    let Some(session_ref) = model.live_ref else {
        return Command::done();
    };
    match model.transport_phase {
        // Nothing to recover before pairing has started.
        TransportPhase::Joining => Command::done(),
        TransportPhase::Down => {
            // Force an immediate reconnect: cancel the frozen backoff and
            // retry now (`transport.reconnect()` order: clear timer, emit
            // 'reconnecting', reconnect).
            let mut ops = Vec::new();
            if let Some(id) = model.timers.backoff.take() {
                ops.push(cancel_op(model.attempt, id));
            }
            if model.timers.grace.is_none() {
                let gid = start_timer(model, &mut ops, DsessTimerKind::Grace, RECONNECT_GRACE_MS);
                model.timers.grace = Some(gid);
            }
            ops.push(req(
                model.attempt,
                DsessOperation::ReconnectTransport {
                    session_ref,
                    cause: DsessReconnectCause::Foreground,
                },
            ));
            finish(ops)
        }
        TransportPhase::Up => {
            if backgrounded_ms >= STALE_AFTER_MS {
                // Ported verbatim: the stale branch only reconnects from
                // phase 'disconnected' — a still-'connected' session gets
                // neither a reconnect NOR the ping.
                Command::done()
            } else {
                let ops = vec![req(
                    model.attempt,
                    DsessOperation::PingTransport { session_ref },
                )];
                finish(ops)
            }
        }
    }
}

fn on_network_online(model: &mut Model, now_ms: f64) -> Cmd {
    if model.live_kind != Some(DsessConnectionType::WalletPair) {
        return Command::done();
    }
    let Some(session_ref) = model.live_ref else {
        return Command::done();
    };
    if model.transport_phase == TransportPhase::Joining {
        return Command::done();
    }
    // Two signals firing together (`online` + `visibilitychange`) must not
    // double-reconnect.
    if now_ms - model.last_recover_at < RECOVER_THROTTLE_MS {
        return Command::done();
    }
    model.last_recover_at = now_ms;
    match model.transport_phase {
        TransportPhase::Down => {
            // recoverNow order: emit 'reconnecting' (grace), arm the
            // deadline, then reconnect() (cancels the pending backoff).
            let mut ops = Vec::new();
            if model.timers.grace.is_none() {
                let gid = start_timer(model, &mut ops, DsessTimerKind::Grace, RECONNECT_GRACE_MS);
                model.timers.grace = Some(gid);
            }
            if model.timers.deadline.is_none() {
                let did = start_timer(
                    model,
                    &mut ops,
                    DsessTimerKind::Deadline,
                    RECONNECT_DEADLINE_MS,
                );
                model.timers.deadline = Some(did);
            }
            if let Some(id) = model.timers.backoff.take() {
                ops.push(cancel_op(model.attempt, id));
            }
            ops.push(req(
                model.attempt,
                DsessOperation::ReconnectTransport {
                    session_ref,
                    cause: DsessReconnectCause::Online,
                },
            ));
            finish(ops)
        }
        _ => {
            let ops = vec![req(
                model.attempt,
                DsessOperation::PingTransport { session_ref },
            )];
            finish(ops)
        }
    }
}

fn on_wallet_changed(model: &mut Model, chain_id: u32) -> Cmd {
    model.chain_id = chain_id;
    let mut ops = Vec::new();
    // `if (status === 'connected' && transportRef.current?.connected)`
    if model.status == DsessStatus::Connected && model.transport_phase == TransportPhase::Up {
        push_wallet_info(model, &mut ops);
    }
    finish(ops)
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: DsessShellResult) -> Cmd {
    match result {
        DsessShellResult::Ack => Command::done(),

        DsessShellResult::WalletPairPrepared { fingerprint, dapp } => {
            let Some(session_ref) = model.awaiting_prepare.take() else {
                return Command::done();
            };
            model.pending_wp_ref = Some(session_ref);
            model.pending_fingerprint = Some(fingerprint);
            model.dapp_info = Some(dapp);
            // Status stays 'connecting' while the user compares the code.
            render()
        }
        DsessShellResult::WalletPairPrepareFailed { message } => {
            if model.awaiting_prepare.take().is_none() {
                return Command::done();
            }
            let mut ops = Vec::new();
            set_status(model, &mut ops, DsessStatus::Error);
            model.error = Some(DsessError::Transport { message });
            finish(ops)
        }

        DsessShellResult::RemoteInjectConnectFinished => {
            let (Some(session_ref), Some(DsessConnectionType::RemoteInject)) =
                (model.live_ref, model.live_kind)
            else {
                return Command::done();
            };
            if model.ri_restore {
                // The restore path commits the session only on success.
                if let Some(session) = model.restoring_session.take() {
                    model.session = Some(session);
                }
                model.ri_restore = false;
            }
            let Some(session) = model.session.clone() else {
                return Command::done();
            };
            let ops = vec![
                req(model.attempt, DsessOperation::FetchDappInfo { session_ref }),
                req(
                    model.attempt,
                    DsessOperation::SaveRemoteInjectSession { session },
                ),
            ];
            finish(ops)
        }
        DsessShellResult::RemoteInjectConnectFailed { message } => {
            let (Some(session_ref), Some(DsessConnectionType::RemoteInject)) =
                (model.live_ref, model.live_kind)
            else {
                return Command::done();
            };
            if model.ri_restore {
                // Stale session — clean up silently, don't show an error.
                model.ri_restore = false;
                model.restoring_session = None;
                model.live_ref = None;
                model.live_kind = None;
                model.transport_phase = TransportPhase::Joining;
                let ops = vec![
                    req(
                        model.attempt,
                        DsessOperation::DisconnectTransport { session_ref },
                    ),
                    req(model.attempt, DsessOperation::ClearRemoteInjectSession),
                ];
                finish(ops)
            } else {
                // Fresh connect failure: error status, ref dropped — and NO
                // disconnect (TS only nulls the ref; ported verbatim).
                model.live_ref = None;
                model.live_kind = None;
                model.transport_phase = TransportPhase::Joining;
                let mut ops = Vec::new();
                set_status(model, &mut ops, DsessStatus::Error);
                model.error = Some(DsessError::Transport { message });
                finish(ops)
            }
        }

        DsessShellResult::JoinFinished { connected } => {
            let (Some(session_ref), Some(DsessConnectionType::WalletPair)) =
                (model.live_ref, model.live_kind)
            else {
                return Command::done();
            };
            if connected {
                return Command::done();
            }
            // confirmJoin resolved but the join may have been silently
            // dropped (CF Worker hibernation) — bound the wait (④).
            let mut ops = Vec::new();
            let id = start_timer(model, &mut ops, DsessTimerKind::Join, JOIN_TIMEOUT_MS);
            model.timers.join = Some((id, session_ref));
            finish(ops)
        }
        DsessShellResult::JoinFailed { message } => {
            if model.live_kind != Some(DsessConnectionType::WalletPair) || model.live_ref.is_none()
            {
                return Command::done();
            }
            model.error = Some(DsessError::Transport { message });
            let mut ops = Vec::new();
            // Ends `disconnected`, not `error` — clobber quirk, see the
            // module doc. The disconnect releases the retry loop + join key.
            drop_live_transport(model, &mut ops, true, false);
            finish(ops)
        }

        DsessShellResult::ReconnectFinished { cause, ok } => {
            let (Some(session_ref), Some(DsessConnectionType::WalletPair)) =
                (model.live_ref, model.live_kind)
            else {
                return Command::done();
            };
            if ok {
                if cause == DsessReconnectCause::Restore {
                    // Reconnect resolved — now give the channel 8s to prove
                    // it is actually live (invariant ⑤).
                    let mut ops = Vec::new();
                    let id =
                        start_timer(model, &mut ops, DsessTimerKind::DropIfDead, DROP_IF_DEAD_MS);
                    model.timers.drop_dead = Some((id, session_ref));
                    return finish(ops);
                }
                return Command::done();
            }
            match cause {
                DsessReconnectCause::Backoff => {
                    let mut ops = Vec::new();
                    schedule_backoff(model, &mut ops);
                    finish(ops)
                }
                DsessReconnectCause::Restore => {
                    // `reconnect()` threw during restore — the channel is
                    // dead right now; drop and wipe immediately.
                    if model.transport_phase == TransportPhase::Up {
                        return Command::done();
                    }
                    let mut ops = Vec::new();
                    drop_live_transport(model, &mut ops, true, true);
                    finish(ops)
                }
                // Manual/foreground/online failures do nothing: WalletPair
                // may still retry via an armed backoff; the UI stays put.
                // (A failed MANUAL reconnect schedules no new backoff —
                // ported verbatim, see the module doc.)
                _ => Command::done(),
            }
        }

        DsessShellResult::WalletPairRestoreFinished { restored } => {
            let Some(session_ref) = model.awaiting_restore.take() else {
                return Command::done();
            };
            if !restored {
                // Invalid snapshot — silent cleanup (invariant ⑥).
                model.pending_wp_restore = None;
                let ops = vec![req(model.attempt, DsessOperation::ClearWalletPairSnapshot)];
                return finish(ops);
            }
            model.live_ref = Some(session_ref);
            model.live_kind = Some(DsessConnectionType::WalletPair);
            // `restore()` leaves the session in phase 'disconnected'.
            model.transport_phase = TransportPhase::Down;
            model.dapp_info = model.pending_wp_restore.take();
            let mut ops = Vec::new();
            // `reconnect()` emits 'reconnecting' → the grace debounce arms;
            // status stays 'disconnected' until it fires (today's launch UX).
            if model.timers.grace.is_none() {
                let gid = start_timer(model, &mut ops, DsessTimerKind::Grace, RECONNECT_GRACE_MS);
                model.timers.grace = Some(gid);
            }
            ops.push(req(
                model.attempt,
                DsessOperation::ReconnectTransport {
                    session_ref,
                    cause: DsessReconnectCause::Restore,
                },
            ));
            finish(ops)
        }

        DsessShellResult::DappInfoFetched { info } => {
            if model.live_kind != Some(DsessConnectionType::RemoteInject)
                || model.live_ref.is_none()
            {
                return Command::done();
            }
            // `setDappInfo(info)` — a failed fetch sets null (verbatim).
            model.dapp_info = info;
            render()
        }

        DsessShellResult::CountersPersisted { ok } => {
            let (Some(session_ref), Some(DsessConnectionType::WalletPair)) =
                (model.live_ref, model.live_kind)
            else {
                return Command::done();
            };
            if ok {
                if model.transport_phase != TransportPhase::Up {
                    // `pushWalletInfo` guards on `_connected` — the channel
                    // dropped while persisting; nothing to push into.
                    return Command::done();
                }
                let ops = vec![req(
                    model.attempt,
                    DsessOperation::PushWalletInfo {
                        session_ref,
                        chain_id: model.chain_id,
                    },
                )];
                finish(ops)
            } else {
                // Continuing after a failed counter write risks nonce reuse
                // after restart — never produce the ciphertext (invariant ⑦;
                // mirrors `abandonUnsafeSession`: close + wipe).
                let mut ops = Vec::new();
                drop_live_transport(model, &mut ops, true, true);
                finish(ops)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Shared transitions
// ---------------------------------------------------------------------------

fn req(attempt: u64, operation: DsessOperation) -> Cmd {
    Command::request_from_shell(operation)
        .then_send(move |result| Event::ShellCompleted { attempt, result })
}

fn cancel_op(attempt: u64, id: u32) -> Cmd {
    req(attempt, DsessOperation::CancelTimer { id })
}

fn finish(mut ops: Vec<Cmd>) -> Cmd {
    ops.push(render());
    Command::all(ops)
}

fn alloc_ref(model: &mut Model) -> u32 {
    let session_ref = model.next_ref;
    model.next_ref += 1;
    session_ref
}

fn start_timer(model: &mut Model, ops: &mut Vec<Cmd>, kind: DsessTimerKind, ms: u32) -> u32 {
    let id = model.next_timer;
    model.next_timer += 1;
    ops.push(req(
        model.attempt,
        DsessOperation::StartTimer { id, kind, ms },
    ));
    id
}

/// The status transition plus the stuck-timer effect that shadows it: enter
/// "reconnecting" ⇒ arm the 45s prompt; leave it ⇒ clear the prompt.
fn set_status(model: &mut Model, ops: &mut Vec<Cmd>, status: DsessStatus) {
    if model.status == status {
        return;
    }
    model.status = status;
    model.reconnect_stuck = false;
    if let Some(id) = model.timers.stuck.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if status == DsessStatus::Reconnecting {
        let id = start_timer(
            model,
            ops_slot(ops),
            DsessTimerKind::Stuck,
            RECONNECT_STUCK_MS,
        );
        model.timers.stuck = Some(id);
    }
}

// Helper so `set_status` can call `start_timer` without a second `&mut Vec`
// binding fight (identity function; keeps the borrow local and obvious).
fn ops_slot(ops: &mut Vec<Cmd>) -> &mut Vec<Cmd> {
    ops
}

/// `min(1s · 2ⁿ, 30s)`.
fn backoff_delay(attempt: u32) -> u32 {
    if attempt >= 5 {
        BACKOFF_CAP_MS
    } else {
        (BACKOFF_BASE_MS << attempt).min(BACKOFF_CAP_MS)
    }
}

/// `scheduleReconnect` — one armed backoff at a time, only while the session
/// is transport-down.
fn schedule_backoff(model: &mut Model, ops: &mut Vec<Cmd>) {
    if model.timers.backoff.is_some() || model.transport_phase != TransportPhase::Down {
        return;
    }
    let ms = backoff_delay(model.backoff_attempt);
    model.backoff_attempt += 1;
    let id = start_timer(model, ops, DsessTimerKind::Backoff, ms);
    model.timers.backoff = Some(id);
}

/// `disconnectCurrent` — tear down whatever is pending or live so a new
/// connect flow starts clean. Storage is NOT touched here (callers decide);
/// `connection_type` follows today's asymmetry (a torn-down remote-inject
/// transport's `disconnected` handler nulls it; a walletpair one does not).
fn disconnect_current(model: &mut Model, ops: &mut Vec<Cmd>) {
    if let Some(id) = model.timers.grace.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    // An in-flight prepare's handle: fail-closed key release (the TS prepare
    // is synchronous so this window does not exist there; see module doc).
    if let Some(session_ref) = model.awaiting_prepare.take() {
        ops.push(req(
            model.attempt,
            DsessOperation::DisconnectTransport { session_ref },
        ));
    }
    // A pairing awaiting fingerprint approval owns an ephemeral X25519 pair —
    // replacing it must release the key explicitly (invariant ②).
    if let Some(session_ref) = model.pending_wp_ref.take() {
        ops.push(req(
            model.attempt,
            DsessOperation::DisconnectTransport { session_ref },
        ));
    }
    model.pending_fingerprint = None;
    // Remaining timers belong to the live transport's episode.
    if let Some(id) = model.timers.stuck.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some((id, _)) = model.timers.join.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some(id) = model.timers.deadline.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some(id) = model.timers.backoff.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some((id, _)) = model.timers.drop_dead.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some(session_ref) = model.live_ref.take() {
        if model.live_kind == Some(DsessConnectionType::RemoteInject) {
            model.connection_type = None;
        }
        ops.push(req(
            model.attempt,
            DsessOperation::DisconnectTransport { session_ref },
        ));
    }
    model.live_kind = None;
    model.transport_phase = TransportPhase::Joining;
    model.backoff_attempt = 0;
    model.ri_restore = false;
    model.restoring_session = None;
}

/// The `disconnected`-handler state change: every timer down, status
/// `disconnected`, handle released. `session`/`dapp_info` survive.
fn drop_live_transport(
    model: &mut Model,
    ops: &mut Vec<Cmd>,
    disconnect: bool,
    wipe_snapshot: bool,
) {
    if let Some(id) = model.timers.grace.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some(id) = model.timers.stuck.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some((id, _)) = model.timers.join.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some(id) = model.timers.deadline.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some(id) = model.timers.backoff.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    if let Some((id, _)) = model.timers.drop_dead.take() {
        ops.push(cancel_op(model.attempt, id));
    }
    model.reconnect_stuck = false;
    model.status = DsessStatus::Disconnected;
    model.connection_type = None;
    model.transport_phase = TransportPhase::Joining;
    model.backoff_attempt = 0;
    if let Some(session_ref) = model.live_ref.take() {
        if disconnect {
            ops.push(req(
                model.attempt,
                DsessOperation::DisconnectTransport { session_ref },
            ));
        }
    }
    model.live_kind = None;
    if wipe_snapshot {
        ops.push(req(model.attempt, DsessOperation::ClearWalletPairSnapshot));
    }
}

/// Wallet-info push. WalletPair sequences the counter persist FIRST — the
/// push op is only issued from `CountersPersisted { ok: true }` (⑦).
fn push_wallet_info(model: &mut Model, ops: &mut Vec<Cmd>) {
    let Some(session_ref) = model.live_ref else {
        return;
    };
    match model.live_kind {
        Some(DsessConnectionType::WalletPair) => ops.push(req(
            model.attempt,
            DsessOperation::PersistWalletPairCounters { session_ref },
        )),
        Some(DsessConnectionType::RemoteInject) => ops.push(req(
            model.attempt,
            DsessOperation::PushWalletInfo {
                session_ref,
                chain_id: model.chain_id,
            },
        )),
        None => {}
    }
}

// ---------------------------------------------------------------------------
// Pure policy — entry classification (invariant ⑨)
// ---------------------------------------------------------------------------

/// The five-way entry decision, ported from `ConnectScreen.handleConnect`:
/// walletpair URI → remote-inject link → full http(s) URL → bare host →
/// invalid. Order is load-bearing: a remote-inject connect link IS an
/// https:// URL (it carries `n`+`k`), so the browser fallback may only run
/// once `parse_remote_inject_url` returned `None` (ARCHITECTURE §7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DsessInput {
    WalletPair { uri: String },
    RemoteInject { session: DsessRemoteInjectSession },
    Browser { url: String },
    Invalid,
}

pub fn classify_connect_input(raw: &str) -> DsessInput {
    let trimmed = raw.trim();
    if is_wallet_pair_uri(trimmed) {
        return DsessInput::WalletPair {
            uri: trimmed.to_owned(),
        };
    }
    if let Some(session) = parse_remote_inject_url(trimmed) {
        return DsessInput::RemoteInject { session };
    }
    if let Some(url) = coerce_browser_url(trimmed) {
        return DsessInput::Browser { url };
    }
    DsessInput::Invalid
}

/// `isWalletPairURI`.
pub fn is_wallet_pair_uri(raw: &str) -> bool {
    raw.trim_start().starts_with("walletpair:")
}

/// `parseRemoteInjectURL` — both accepted shapes:
/// `https://server/s/{sessionId}?n={nonce}&k={secret}` and
/// `https://server/bridge?session={id}&n={nonce}&k={secret}`.
/// The scheme is NOT restricted to http(s) — ported verbatim.
pub fn parse_remote_inject_url(raw: &str) -> Option<DsessRemoteInjectSession> {
    let url = parse_url(raw)?;
    let server_url = format!("{}://{}", url.scheme, url.host);
    // Empty values are falsy in the TS guard — require non-empty.
    let nonce = query_param(url.query.as_deref(), "n").filter(|v| !v.is_empty())?;
    let secret = query_param(url.query.as_deref(), "k").filter(|v| !v.is_empty())?;
    if let Some(rest) = url.path.strip_prefix("/s/") {
        let end = rest.find('/').unwrap_or(rest.len());
        let session_id = &rest[..end];
        if !session_id.is_empty() {
            return Some(DsessRemoteInjectSession {
                server_url,
                session_id: session_id.to_owned(),
                nonce,
                secret,
            });
        }
    }
    let session_id = query_param(url.query.as_deref(), "session").filter(|v| !v.is_empty())?;
    Some(DsessRemoteInjectSession {
        server_url,
        session_id,
        nonce,
        secret,
    })
}

/// `coerceBrowserUrl` — a full http(s) URL as-is (normalized), a bare host
/// (`app.uniswap.org`, `uniswap.org/swap`) defaulted to https, everything
/// else `None`. A string that parses as a URL with any OTHER scheme is
/// rejected outright (never retried as a bare host) — `javascript:` and even
/// `localhost:8080` (scheme "localhost"!) return `None`, exactly as today.
pub fn coerce_browser_url(raw: &str) -> Option<String> {
    let s = raw.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(url) = parse_url(s) {
        if !url.opaque && (url.scheme == "http" || url.scheme == "https") {
            return Some(serialize_http(&url));
        }
        return None;
    }
    // Bare host: must have a dot and no whitespace; default the scheme.
    if s.chars().any(char::is_whitespace) || !s.contains('.') {
        return None;
    }
    let url = parse_url(&format!("https://{s}"))?;
    if url.hostname.contains('.') {
        Some(serialize_http(&url))
    } else {
        None
    }
}

// --- Minimal WHATWG-ish URL parsing -----------------------------------------
//
// The `url` crate is not in this crate's dependency set, so classification
// carries its own small parser covering exactly what the three entry
// functions observe: scheme, host (lowercased, default port dropped), path,
// query parameters (percent-decoded). Deviations are fail-closed and listed
// in the module doc.

struct ParsedUrl {
    scheme: String,
    userinfo: Option<String>,
    /// Lowercased `host[:port]`, default port removed.
    host: String,
    /// Lowercased, no port.
    hostname: String,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
    /// A non-special scheme with no authority (`mailto:x`, `localhost:8080`).
    opaque: bool,
}

fn is_special_scheme(scheme: &str) -> bool {
    matches!(scheme, "http" | "https" | "ws" | "wss" | "ftp")
}

fn default_port(scheme: &str) -> Option<u32> {
    match scheme {
        "http" | "ws" => Some(80),
        "https" | "wss" => Some(443),
        "ftp" => Some(21),
        _ => None,
    }
}

fn parse_url(raw: &str) -> Option<ParsedUrl> {
    // Fail-closed deviation: WHATWG strips tab/CR/LF; we reject instead.
    if raw.chars().any(char::is_whitespace) {
        return None;
    }
    let bytes = raw.as_bytes();
    let mut scheme_end = None;
    for (index, &byte) in bytes.iter().enumerate() {
        if index == 0 {
            if !byte.is_ascii_alphabetic() {
                return None;
            }
            continue;
        }
        if byte == b':' {
            scheme_end = Some(index);
            break;
        }
        if !(byte.is_ascii_alphanumeric() || byte == b'+' || byte == b'.' || byte == b'-') {
            return None;
        }
    }
    let scheme_end = scheme_end?;
    let scheme = raw[..scheme_end].to_ascii_lowercase();
    let rest = &raw[scheme_end + 1..];
    let special = is_special_scheme(&scheme);

    let after = if special {
        // Special schemes tolerate any number of slashes ("http:foo",
        // "http:///foo" both mean an authority).
        rest.trim_start_matches('/')
    } else if let Some(after) = rest.strip_prefix("//") {
        after
    } else {
        // Opaque path — enough structure for the query to be readable.
        let (path, query, fragment) = split_path_query_fragment(rest);
        return Some(ParsedUrl {
            scheme,
            userinfo: None,
            host: String::new(),
            hostname: String::new(),
            path,
            query,
            fragment,
            opaque: true,
        });
    };

    let authority_end = after.find(['/', '?', '#']).unwrap_or(after.len());
    let authority = &after[..authority_end];
    let (userinfo, host_port) = match authority.rfind('@') {
        Some(at) => (Some(authority[..at].to_owned()), &authority[at + 1..]),
        None => (None, authority),
    };
    let host_port = host_port.to_ascii_lowercase();
    let (hostname, port_text) = match host_port.rfind(':') {
        Some(colon) => (
            host_port[..colon].to_owned(),
            Some(host_port[colon + 1..].to_owned()),
        ),
        None => (host_port.clone(), None),
    };
    if hostname.is_empty() && special {
        return None; // JS URL throws: special schemes need a host.
    }
    let port = match port_text {
        None => None,
        Some(text) if text.is_empty() => None, // "host:" — empty port dropped
        Some(text) => {
            if !text.bytes().all(|b| b.is_ascii_digit()) {
                return None; // invalid port ⇒ the whole parse fails
            }
            let number: u32 = text.parse().ok()?;
            if number > 65_535 {
                return None;
            }
            if default_port(&scheme) == Some(number) {
                None
            } else {
                Some(number)
            }
        }
    };
    let host = match port {
        Some(number) => format!("{hostname}:{number}"),
        None => hostname.clone(),
    };
    let (path, query, fragment) = split_path_query_fragment(&after[authority_end..]);
    Some(ParsedUrl {
        scheme,
        userinfo,
        host,
        hostname,
        path,
        query,
        fragment,
        opaque: false,
    })
}

fn split_path_query_fragment(input: &str) -> (String, Option<String>, Option<String>) {
    let (before_fragment, fragment) = match input.find('#') {
        Some(hash) => (&input[..hash], Some(input[hash + 1..].to_owned())),
        None => (input, None),
    };
    let (path, query) = match before_fragment.find('?') {
        Some(question) => (
            before_fragment[..question].to_owned(),
            Some(before_fragment[question + 1..].to_owned()),
        ),
        None => (before_fragment.to_owned(), None),
    };
    (path, query, fragment)
}

fn serialize_http(url: &ParsedUrl) -> String {
    let mut out = format!("{}://", url.scheme);
    if let Some(userinfo) = &url.userinfo {
        out.push_str(userinfo);
        out.push('@');
    }
    out.push_str(&url.host);
    if url.path.is_empty() {
        out.push('/');
    } else {
        out.push_str(&url.path);
    }
    if let Some(query) = &url.query {
        out.push('?');
        out.push_str(query);
    }
    if let Some(fragment) = &url.fragment {
        out.push('#');
        out.push_str(fragment);
    }
    out
}

/// First value for `name`, `URLSearchParams` style (`+` is a space, `%HH`
/// decoded, malformed escapes kept literal).
fn query_param(query: Option<&str>, name: &str) -> Option<String> {
    for pair in query?.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = match pair.find('=') {
            Some(equals) => (&pair[..equals], &pair[equals + 1..]),
            None => (pair, ""),
        };
        if percent_decode(key) == name {
            return Some(percent_decode(value));
        }
    }
    None
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'+' {
            out.push(b' ');
            index += 1;
            continue;
        }
        if byte == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_digit(bytes[index + 1]), hex_digit(bytes[index + 2]))
            {
                out.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        out.push(byte);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Pure policy — plaintext chainId vs encrypted CAIP-2 context (invariant ⑧)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DsessChainContextError {
    /// The CAIP-2 context itself is unusable.
    InvalidCaip2,
    /// A declared plaintext chain id disagrees with the encrypted context.
    ChainMismatch,
}

/// `caip2ToChainId` — ported verbatim: slices 7 chars WITHOUT checking the
/// `eip155:` prefix (upstream `validateCaip2` guarantees it), and uses
/// `parseInt` leading-digit semantics (`"eip155:5x"` → 5). Requires a safe
/// integer ≥ 1.
pub fn caip2_to_chain_id(caip2: &str) -> Option<u64> {
    let rest = caip2.get(7..)?;
    let end = rest
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    let value: u64 = rest[..end].parse().ok()?;
    if !(1..=MAX_SAFE_INTEGER).contains(&value) {
        return None;
    }
    Some(value)
}

/// `declaredChainId` — number (safe non-negative integer), `0x…` hex string,
/// or decimal string. A numeric string too large for u128 collapses to a
/// sentinel that can never equal a safe-integer expectation — the same
/// reject-by-mismatch outcome as the imprecise JS float.
pub fn declared_chain_id(value: &Value) -> Option<u128> {
    if let Some(number) = value.as_u64() {
        if number <= MAX_SAFE_INTEGER {
            return Some(u128::from(number));
        }
        return None;
    }
    if let Some(float) = value.as_f64() {
        // Negative, fractional, or beyond 2^53: not a safe non-negative int.
        if float >= 0.0 && float.fract() == 0.0 && float <= MAX_SAFE_INTEGER as f64 {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            return Some(float as u128);
        }
        return None;
    }
    let text = value.as_str()?;
    if let Some(hex) = text.strip_prefix("0x") {
        if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        return Some(u128::from_str_radix(hex, 16).unwrap_or(u128::MAX));
    }
    if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
        return Some(text.parse::<u128>().unwrap_or(u128::MAX));
    }
    None
}

/// `assertRequestChainContext` — the encrypted CAIP-2 suffix is the
/// authoritative chain context; a plaintext `chainId` embedded in the request
/// body that disagrees is rejected (invariant ⑧). A typed-data STRING that
/// fails to parse as JSON passes here — the signing validator owns that
/// rejection (ported verbatim).
pub fn assert_request_chain_context(
    method: &str,
    params: &Value,
    caip2: &str,
) -> Result<(), DsessChainContextError> {
    let expected = caip2_to_chain_id(caip2).ok_or(DsessChainContextError::InvalidCaip2)?;
    let mut candidate: Option<Value> = None;
    if method == "eth_sendTransaction" || method == "wallet_sendCalls" {
        candidate = params
            .get(0)
            .and_then(|first| first.get("chainId"))
            .cloned();
    }
    if crate::typed_data_request::looks_like_typed_data(method) {
        // The request's ONE document (audit 2026-10-01). A request that is not
        // one is refused -32602 by the signing validator, so it is let past
        // here rather than answered twice.
        let Ok(read) = crate::typed_data_request::canonical(method, params) else {
            return Ok(());
        };
        candidate = read
            .document
            .get("domain")
            .and_then(|domain| domain.get("chainId"))
            .cloned();
    }
    match candidate {
        None | Some(Value::Null) => Ok(()),
        Some(value) => {
            if declared_chain_id(&value) == Some(u128::from(expected)) {
                Ok(())
            } else {
                Err(DsessChainContextError::ChainMismatch)
            }
        }
    }
}

impl super::SplitEffect for DsessEffect {
    type Op = DsessOperation;
    fn into_shell(self) -> Option<crux_core::Request<DsessOperation>> {
        match self {
            DsessEffect::Render(_) => None,
            DsessEffect::Shell(request) => Some(request),
        }
    }
}
