//! Machine — the Safari extension's App Group account snapshot and the
//! Universal-Link attestation TTL (spec `017-crux-wallet-state-complete`,
//! inventory `ext_cache`).
//!
//! ```text
//! AccountsChanged ─┬─ loading ──────► (neither write nor clear)
//!                  ├─ logged out ───► RemoveSnapshot
//!                  └─ logged in ────► ReadAttestation ─► WriteSnapshot
//! UniversalLinkOpened ─► [RequestExtensionSign?] + PersistAttestation ─► re-sync
//! SessionEnded ─► RemoveSnapshot  (invariant ⑥ — logout always clears)
//! ```
//!
//! The extension answers connect/read/state **in Safari, zero app hop**, from a
//! public snapshot the app writes into the shared App Group container. The
//! container is readable on jailbroken devices, so the projection here is a
//! security boundary: accounts are force-reprojected to exactly
//! `{ name, address }` (`app-group-account-sync.ts:148`) — no credential id, no
//! key material, nothing a dApp would not learn anyway.
//!
//! The Universal-Link attestation is fund-safety: a failed UL launch navigates
//! the dApp tab away and loses the pending sign, so the extension may use the
//! UL only while a `getvela.app/sign` open is PROVEN recent. The association
//! can silently break after attestation (e.g. "Open in Safari" preference), so
//! the proof is a timestamp with a 14-day TTL, refreshed by every successful
//! open; when opens stop landing it ages out and the extension reverts to the
//! always-safe `velawallet://` scheme. The TTL comparison lives in exactly one
//! function here ([`ul_fresh`]) — the TS duplicated it
//! (`app-group-account-sync.ts:66` and `:175`); invariant ③ makes it a single
//! point.
//!
//! The core decides; iOS executes. Whether the App Group exists
//! (`AppGroup.isSupportedSync`), file I/O, the network-catalog `chains` map the
//! shell merges into the file, and the file's exact key spelling all stay in
//! the shell — this machine only rules on TTL, gating and projection.
//!
//! Ported-verbatim quirks, kept deliberately:
//! - The raw `ul_verified_at_ms` rides the snapshot even when EXPIRED — the
//!   extension compares it against its self-heal veto timestamp, so the raw
//!   value must survive the TTL verdict.
//! - A FUTURE attestation timestamp is not fresh (`now - ts >= 0` guard) — a
//!   rolled-back clock falls back to the safe scheme.
//! - The attestation is device-level and survives logout: `SessionEnded`
//!   removes the snapshot but never the attestation.
//! - Theme normalization is strict equality: anything but exactly `"light"` or
//!   `"dark"` becomes `"auto"`.
//! - The follow-up write after an attestation persist RE-READS the flag rather
//!   than trusting the value just persisted (`writeAccountCache` re-reads on
//!   every write), so a shell whose persist failed still writes the truth.
//!
//! Documented deviation: the TS component fires `writeAccountCache` /
//! `clearAccountCache` fire-and-forget, so a write that started before a
//! logout can land AFTER the clear and resurrect the file. The core's
//! attempt guard supersedes in-flight flows instead — logout always wins
//! (invariant ⑥) and concurrent double-writes collapse into one deterministic
//! write with the latest inputs.
//!
//! The inventory lists `AttestationRead{ts}` under Events; here it is the
//! [`ExtCacheShellResult`] answering [`ExtCacheOperation::ReadAttestation`],
//! riding the house `ShellCompleted` channel (and carrying `now_ms`, the 011
//! clock-injection pattern).

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::Account;

/// The STABLE default chain the snapshot advertises (invariant ⑤). The wallet
/// has no global "current network" — each connected dApp picks/switches its own
/// chain per-origin in the extension — so this is NEVER sourced from the
/// volatile dApp-bridge chainId (the event vocabulary cannot even carry one).
pub const DEFAULT_EXT_CHAIN_ID: u32 = 1;

/// Attestation lifetime — `UL_TTL_MS` in `app-group-account-sync.ts:50`.
pub const UL_TTL_MS: f64 = 14.0 * 24.0 * 60.0 * 60.0 * 1000.0;

/// The attestation probe rid (`getvela.app/sign?rid=ul-selftest`) — not a real
/// sign, so it never drives the extension sign bus.
pub const UL_SELFTEST_RID: &str = "ul-selftest";

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// One account as the world-readable file may carry it: name and address,
/// nothing else (invariant ①). This type IS the projection — a richer
/// [`Account`] can never serialize through it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExtAccount {
    pub name: String,
    pub address: String,
}

/// The app's color-scheme PREFERENCE, riding the cache so the extension UI
/// matches the app (a forced-dark app → dark sheet even on a light system).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ExtTheme {
    #[default]
    Auto,
    Light,
    Dark,
}

/// The public snapshot the shell serializes into `vela.ext.account.json`.
/// The shell merges its network catalog (`buildChainsMap()` — names, RPC and
/// bundler URLs, chain badges) and owns the file's exact key spelling; the
/// core owns everything decided here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExtSnapshot {
    /// Active Safe address.
    pub address: String,
    /// Active account display name (may be empty — `input.name || ''`).
    pub name: String,
    /// All accounts, force-reprojected (invariant ①) for the connect sheet.
    pub accounts: Vec<ExtAccount>,
    /// Always [`DEFAULT_EXT_CHAIN_ID`] (invariant ⑤).
    pub chain_id: u32,
    /// The clock observed by the write's attestation read.
    pub updated_at_ms: f64,
    /// TTL-checked verdict — the popup's go/no-go for the UL hand-off.
    pub ul_verified: bool,
    /// Raw attestation timestamp (0 = none) — the extension compares it
    /// against its self-heal veto, so it survives the TTL verdict.
    pub ul_verified_at_ms: f64,
    pub theme: ExtTheme,
    /// Resolved display language (may be empty — `input.locale || ''`).
    pub locale: String,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. Every operation is a no-op off
/// iOS (`AppGroup.isSupportedSync` guards in the shell) and best-effort — the
/// shell swallows I/O errors exactly as the TS `catch` blocks do, and always
/// acks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ExtCacheOperation"))]
pub enum ExtCacheOperation {
    /// Merge the network catalog and write `vela.ext.account.json`.
    WriteSnapshot { snapshot: ExtSnapshot },
    /// Remove `vela.ext.account.json` → the extension shows empty-state.
    RemoveSnapshot,
    /// Read the persisted attestation timestamp (`vela.ext.ulVerifiedAt`).
    ReadAttestation,
    /// Persist a fresh attestation timestamp. Acked even when storage fails
    /// (the TS swallows it and stays unverified) — the follow-up read
    /// reflects the truth either way.
    PersistAttestation { ts: f64 },
    /// A UL carried a real rid — drive the extension sign for it (the shell's
    /// sign bus buffers it if the controller isn't up yet).
    RequestExtensionSign { rid: String },
}

/// What the shell observed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ExtCacheShellResult"))]
pub enum ExtCacheShellResult {
    /// `vela.ext.ulVerifiedAt` as persisted — 0 for never/unreadable (the
    /// shell fails closed like `getUniversalLinkVerifiedAt`). `now_ms` rides
    /// the result so the core stays a pure function of its inputs.
    AttestationRead {
        ts: f64,
        now_ms: f64,
    },
    AttestationPersisted,
    SnapshotWritten,
    SnapshotRemoved,
    SignRequested,
}

impl Operation for ExtCacheOperation {
    type Output = ExtCacheShellResult;
}

#[effect]
pub enum ExtCacheEffect {
    Render(RenderOperation),
    Shell(ExtCacheOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ExtCacheEvent"))]
#[allow(clippy::large_enum_variant)] // a wire type: the JSON shape is pinned by the generated TS
pub enum Event {
    /// Wallet state changed (or finished restoring). Replaces the headless
    /// component's latest-ref snapshot: the shell reports the whole current
    /// state, the core keeps it for foreground re-syncs. Carries
    /// `is_loading`/`has_wallet` beyond the inventory's field list because
    /// invariant ② (the loading gate) is a core decision.
    AccountsChanged {
        is_loading: bool,
        has_wallet: bool,
        accounts: Vec<Account>,
        active: Option<Account>,
        /// Raw preference string — the core normalizes (strict equality).
        theme: String,
        locale: String,
    },
    /// App came to foreground — §12.1.6: a user who installed the extension
    /// while already logged in must not have an empty cache. Re-syncs from the
    /// last reported state. (The inventory sketches `Foregrounded{now}`; the
    /// clock instead rides the `AttestationRead` result — single injection
    /// point.)
    Foregrounded,
    /// The app was opened (cold or warm) via a URL. Only an exact
    /// `https://getvela.app/sign…` Universal Link acts (invariant ④); the
    /// shell forwards every launch URL and the core rules.
    UniversalLinkOpened { url: String, now_ms: f64 },
    /// Logout hand-off from the session machine (`ClearExtensionCache`).
    /// Clears unconditionally — an explicit logout is an affirmative signal,
    /// not the ambiguous boot window invariant ② protects.
    SessionEnded,
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: ExtCacheShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// The last wallet state the shell reported — the core-side replacement for
/// `latest.current` in `AccountFileWriter.tsx:66`.
#[derive(Clone, Debug)]
struct Inputs {
    is_loading: bool,
    has_wallet: bool,
    accounts: Vec<Account>,
    active: Option<Account>,
    theme: ExtTheme,
    locale: String,
}

/// A write whose account fields were captured at decision time — the TS
/// closure captured `d` the same way, so inputs arriving while the attestation
/// read is in flight never mutate an already-decided write.
#[derive(Clone, Debug)]
struct PendingWrite {
    address: String,
    name: String,
    accounts: Vec<ExtAccount>,
    theme: ExtTheme,
    locale: String,
}

#[derive(Clone, Debug, Default)]
enum Phase {
    #[default]
    Idle,
    /// `PersistAttestation` in flight after a UL open; the ack re-syncs
    /// (TS: `await markUniversalLinkVerified(); sync(latest.current)`).
    Attesting,
    /// `ReadAttestation` in flight for a decided write.
    ReadingAttestation { pending: PendingWrite },
}

#[derive(Default)]
pub struct Model {
    /// `None` until the first `AccountsChanged` — treated exactly like the
    /// loading window (neither write nor clear), so a premature foreground
    /// can never delete a logged-in user's cache.
    inputs: Option<Inputs>,
    /// The snapshot most recently handed to the shell (`None` = cleared).
    last_snapshot: Option<ExtSnapshot>,
    /// Last attestation timestamp observed via `ReadAttestation` (0 = none).
    ul_verified_at_ms: f64,
    phase: Phase,
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// Headless in TS, diagnostic here: what the core believes the extension can
/// currently see. Public projection only — the view can never leak more than
/// the world-readable file does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ExtCacheView {
    pub snapshot: Option<ExtSnapshot>,
    pub ul_verified_at_ms: f64,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ExtCache;

impl App for ExtCache {
    type Event = Event;
    type Model = Model;
    type ViewModel = ExtCacheView;
    type Effect = ExtCacheEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<ExtCacheEffect, Event> {
        match event {
            Event::AccountsChanged {
                is_loading,
                has_wallet,
                accounts,
                active,
                theme,
                locale,
            } => {
                model.inputs = Some(Inputs {
                    is_loading,
                    has_wallet,
                    accounts,
                    active,
                    theme: normalize_theme(&theme),
                    locale,
                });
                if is_loading {
                    // Invariant ② — the boot restore window: neither write nor
                    // clear. Clearing here would permanently delete a
                    // logged-in user's cache (`AccountFileWriter.tsx:70`). An
                    // in-flight flow keeps its captured data and completes.
                    return render();
                }
                match write_intent(model) {
                    None => clear_now(model),
                    Some(pending) => {
                        if matches!(model.phase, Phase::Attesting) {
                            // The pending attestation flow re-syncs with these
                            // (latest) inputs on ack — one deterministic write
                            // instead of the TS's racing pair.
                            return render();
                        }
                        begin_write(model, pending)
                    }
                }
            }

            Event::Foregrounded => {
                let ready = matches!(model.inputs.as_ref(), Some(inputs) if !inputs.is_loading);
                if !ready {
                    // Still restoring (or never reported): TS `sync` returns
                    // without touching the file.
                    return Command::done();
                }
                match write_intent(model) {
                    // Foreground while genuinely logged out clears again —
                    // idempotent, exactly like TS `sync`.
                    None => clear_now(model),
                    Some(pending) => {
                        if matches!(model.phase, Phase::Attesting) {
                            return Command::done();
                        }
                        begin_write(model, pending)
                    }
                }
            }

            Event::UniversalLinkOpened { url, now_ms } => {
                if !is_sign_universal_link(&url) {
                    // Not the anchored apex `/sign` UL (invariant ④) — spoofs
                    // and unrelated launches change nothing.
                    return Command::done();
                }
                // TS: `new URL(url).searchParams.get('rid')`, catch → null —
                // fail-closed: no parsable rid, no sign request. The probe rid
                // attests but never drives a sign.
                let rid = query_param(&url, "rid")
                    .filter(|rid| !rid.is_empty() && rid != UL_SELFTEST_RID);
                model.attempt += 1;
                model.phase = Phase::Attesting;
                let attempt = model.attempt;
                let mut commands = Vec::new();
                if let Some(rid) = rid {
                    // Driven regardless of login state, exactly as the TS
                    // handler does — the sign bus buffers rids for a
                    // controller that isn't up yet.
                    commands.push(
                        Command::request_from_shell(ExtCacheOperation::RequestExtensionSign {
                            rid,
                        })
                        .then_send(move |result| Event::ShellCompleted { attempt, result }),
                    );
                }
                commands.push(
                    Command::request_from_shell(ExtCacheOperation::PersistAttestation {
                        ts: now_ms,
                    })
                    .then_send(move |result| Event::ShellCompleted { attempt, result }),
                );
                commands.push(render());
                Command::all(commands)
            }

            Event::SessionEnded => {
                // Invariant ⑥ — the extension cache must go with the session,
                // unconditionally (leaving it would keep serving account data
                // to Safari after logout). The attestation is device-level and
                // deliberately survives (ported verbatim).
                let (theme, locale) = model
                    .inputs
                    .as_ref()
                    .map(|inputs| (inputs.theme, inputs.locale.clone()))
                    .unwrap_or((ExtTheme::Auto, String::new()));
                model.inputs = Some(Inputs {
                    is_loading: false,
                    has_wallet: false,
                    accounts: Vec::new(),
                    active: None,
                    theme,
                    locale,
                });
                clear_now(model)
            }

            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A superseded flow — most importantly a write decided
                    // before a logout. Dropping it IS invariant ⑥ winning.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> ExtCacheView {
        ExtCacheView {
            snapshot: model.last_snapshot.clone(),
            ul_verified_at_ms: model.ul_verified_at_ms,
        }
    }
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: ExtCacheShellResult) -> Command<ExtCacheEffect, Event> {
    let phase = core::mem::take(&mut model.phase);
    match (phase, result) {
        // The attestation persisted — now re-sync, RE-READING the flag rather
        // than trusting the value just persisted (`writeAccountCache` re-reads
        // on every write; a failed persist still writes the truth).
        (Phase::Attesting, ExtCacheShellResult::AttestationPersisted) => {
            let ready = matches!(model.inputs.as_ref(), Some(inputs) if !inputs.is_loading);
            if !ready {
                return render();
            }
            match write_intent(model) {
                None => clear_now(model),
                Some(pending) => begin_write(model, pending),
            }
        }

        (
            Phase::ReadingAttestation { pending },
            ExtCacheShellResult::AttestationRead { ts, now_ms },
        ) => {
            let ts = normalize_ts(ts);
            let snapshot = ExtSnapshot {
                address: pending.address,
                name: pending.name,
                accounts: pending.accounts,
                chain_id: DEFAULT_EXT_CHAIN_ID,
                updated_at_ms: now_ms,
                ul_verified: ul_fresh(ts, now_ms),
                ul_verified_at_ms: ts,
                theme: pending.theme,
                locale: pending.locale,
            };
            model.ul_verified_at_ms = ts;
            model.last_snapshot = Some(snapshot.clone());
            request(model, ExtCacheOperation::WriteSnapshot { snapshot })
        }

        // A best-effort ack (write/remove/sign), or a result for a phase that
        // no longer expects it. Neither may change state — but an unmatched
        // result must put the phase back.
        (phase, _) => {
            model.phase = phase;
            Command::done()
        }
    }
}

// ---------------------------------------------------------------------------
// Pure policy
// ---------------------------------------------------------------------------

/// THE TTL judgment (invariant ③) — the one and only place `now - ts` is
/// compared against [`UL_TTL_MS`]. `ts > 0` = ever attested; `< TTL` = not
/// aged out; `>= 0` = not future-dated (a rolled-back clock is not fresh).
fn ul_fresh(ts: f64, now_ms: f64) -> bool {
    ts > 0.0 && now_ms - ts < UL_TTL_MS && now_ms - ts >= 0.0
}

/// `getUniversalLinkVerifiedAt` + `buildAccountCache` double guard, unified:
/// anything not a finite positive timestamp is "never" (0).
fn normalize_ts(ts: f64) -> f64 {
    if ts.is_finite() && ts > 0.0 {
        ts
    } else {
        0.0
    }
}

/// `buildAccountCache`: `theme === 'light' || theme === 'dark' ? theme :
/// 'auto'` — strict equality, ported verbatim (case-sensitive; anything else
/// is `auto`).
fn normalize_theme(theme: &str) -> ExtTheme {
    match theme {
        "light" => ExtTheme::Light,
        "dark" => ExtTheme::Dark,
        _ => ExtTheme::Auto,
    }
}

/// Invariant ① — force-reprojection to EXACTLY `{ name, address }`, so no
/// richer field (`public_key_hex`, ids, timestamps) can ever reach the shared,
/// world-readable file. Belt-and-braces in TS
/// (`app-group-account-sync.ts:148`); structural here.
fn project_accounts(accounts: &[Account]) -> Vec<ExtAccount> {
    accounts
        .iter()
        .map(|account| ExtAccount {
            name: account.name.clone(),
            address: account.address.clone(),
        })
        .collect()
}

/// What the latest inputs call for: `Some` = write this projection, `None` =
/// genuinely logged out → clear. Folds the component gate (`!hasWallet ||
/// !address`) and the service's own empty-address guard
/// (`app-group-account-sync.ts:169`) into one decision. Callers must have
/// excluded the loading window first.
fn write_intent(model: &Model) -> Option<PendingWrite> {
    let inputs = model.inputs.as_ref()?;
    if inputs.is_loading || !inputs.has_wallet {
        return None;
    }
    let active = inputs.active.as_ref()?;
    if active.address.is_empty() {
        return None;
    }
    Some(PendingWrite {
        address: active.address.clone(),
        name: active.name.clone(),
        accounts: project_accounts(&inputs.accounts),
        theme: inputs.theme,
        locale: inputs.locale.clone(),
    })
}

/// Genuinely logged out (or explicit logout): remove the file now and
/// supersede any in-flight write — the attempt bump is what closes the TS's
/// write-after-clear interleaving (module docs).
fn clear_now(model: &mut Model) -> Command<ExtCacheEffect, Event> {
    model.attempt += 1;
    model.phase = Phase::Idle;
    model.last_snapshot = None;
    request(model, ExtCacheOperation::RemoveSnapshot)
}

/// A write is due: capture the projection, then read the attestation — the
/// clock and the persisted timestamp come back together on the result.
fn begin_write(model: &mut Model, pending: PendingWrite) -> Command<ExtCacheEffect, Event> {
    model.attempt += 1;
    model.phase = Phase::ReadingAttestation { pending };
    request(model, ExtCacheOperation::ReadAttestation)
}

// ---------------------------------------------------------------------------
// Universal-Link parsing
// ---------------------------------------------------------------------------

/// `GETVELA_SIGN_UL` (`AccountFileWriter.tsx:48`), ported verbatim:
/// `^https:\/\/getvela\.app\/sign(?:[/?#]|$)` case-insensitive. Anchored to
/// the exact apex host so `evil-getvela.app` / `getvela.app.evil.com` / a path
/// merely CONTAINING the string can't spoof it, and scoped to `/sign` so the
/// attestation proves precisely the path the extension launches.
fn is_sign_universal_link(url: &str) -> bool {
    const PREFIX: &[u8] = b"https://getvela.app/sign";
    let bytes = url.as_bytes();
    if bytes.len() < PREFIX.len() || !bytes[..PREFIX.len()].eq_ignore_ascii_case(PREFIX) {
        return false;
    }
    matches!(
        bytes.get(PREFIX.len()),
        None | Some(b'/') | Some(b'?') | Some(b'#')
    )
}

/// First `key` in the query string, form-decoded — the subset of
/// `new URL(url).searchParams.get(key)` the rid extraction relies on.
/// Fragment is cut before the query is sought (a `?` inside `#…` is not a
/// query), and a URL with no query yields `None` — fail-closed, like the TS
/// `catch { /* no rid */ }`.
fn query_param(url: &str, key: &str) -> Option<String> {
    let no_fragment = match url.find('#') {
        Some(index) => &url[..index],
        None => url,
    };
    let query = &no_fragment[no_fragment.find('?')? + 1..];
    for pair in query.split('&') {
        let (raw_key, raw_value) = match pair.split_once('=') {
            Some((k, v)) => (k, v),
            None => (pair, ""),
        };
        if form_decode(raw_key) == key {
            return Some(form_decode(raw_value));
        }
    }
    None
}

/// application/x-www-form-urlencoded decoding as `URLSearchParams` does it:
/// `+` → space, valid `%hh` → byte (then lossy UTF-8), malformed sequences
/// kept literal.
fn form_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                out.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len()
                && bytes[index + 1].is_ascii_hexdigit()
                && bytes[index + 2].is_ascii_hexdigit() =>
            {
                out.push(hex_value(bytes[index + 1]) * 16 + hex_value(bytes[index + 2]));
                index += 3;
            }
            byte => {
                out.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Value of an ASCII hex digit. Callers have verified `is_ascii_hexdigit`;
/// anything else maps to 0 rather than panicking (lib code never panics).
fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

/// Issue one operation whose answer must match the current attempt.
fn request(model: &mut Model, operation: ExtCacheOperation) -> Command<ExtCacheEffect, Event> {
    let attempt = model.attempt;
    Command::all([
        Command::request_from_shell(operation)
            .then_send(move |result| Event::ShellCompleted { attempt, result }),
        render(),
    ])
}

impl super::SplitEffect for ExtCacheEffect {
    type Op = ExtCacheOperation;
    fn into_shell(self) -> Option<crux_core::Request<ExtCacheOperation>> {
        match self {
            ExtCacheEffect::Render(_) => None,
            ExtCacheEffect::Shell(request) => Some(request),
        }
    }
}
