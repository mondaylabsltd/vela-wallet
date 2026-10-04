//! The in-app browser's request record and its words (spec 099 R3, R5).
//!
//! A dApp tab is six layers deep, and when something does not work the first
//! question is which one stopped:
//!
//! ```text
//! browser   the tab and its page          loading · ready · crashed · navigated away
//! provider  the wallet offered to it      offered · insecure origin · script did not run
//! wallet    the wallet's own rules        not connected · unsupported · unknown chain · not compatible · …
//! network   the chain's RPC               no endpoint · timed out · rate limited · its error
//! relay     Vela's relay                  refused · unreachable · not confirmed yet
//! sheet     the person, on Vela's sheet   rejected
//! signer    the passkey                   unavailable · not discoverable · failed
//! ```
//!
//! [`super::dapp_browser`] gives every request a page sends a row when it
//! routes it and closes the row where the answer is made, naming the layer and
//! the reason. The words are the corpus's (`componentsUi.browserStatus.*`);
//! this module owns the vocabulary, the one log line and the copyable report,
//! so every client says and logs the same thing.
//!
//! Never recorded: params, results, signatures, addresses, paths — a row is a
//! method name, a class, times and an outcome; a tab is named by its origin.

use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::sign_request::{SignErrorKind, NOT_CONFIRMED_MESSAGE, REVERTED_MESSAGE};

/// Rows a tab keeps, open and settled, oldest dropped first.
pub const REQUEST_RECORD_CAP: usize = 200;
/// Every read is answered by then (spec 099 FR-008): the shell's executor
/// races it, and a read nobody answered in time is `network / timed_out`.
pub const READ_DEADLINE_MS: f64 = 30_000.0;
/// How far back the status entry looks for trouble.
pub const RECENT_ROWS: usize = 20;

/// What kind of request a row is — which layers it can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DbrRequestClass {
    /// Answered by the wallet at once: accounts, chain id, permissions,
    /// switch chain, an add for a chain the wallet has, capabilities.
    Local,
    /// A connect (granted already, or the consent sheet), or an add-network
    /// request that opened its sheet (spec 100).
    Consent,
    /// A node read through the person's own endpoints.
    Read,
    /// A read of Vela's relay: receipts, operation status, batch status.
    RelayRead,
    Signing,
}

/// Which layer decided how a request ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DbrLayer {
    Browser,
    Provider,
    Wallet,
    Network,
    Relay,
    Sheet,
    Signer,
}

/// Why a request ended in an error — one corpus line each.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DbrReason {
    /// The page went to another document (4900).
    NavigatedAway,
    /// The page's web content process died (4900).
    PageCrashed,
    /// The tab was closed (4900).
    TabClosed,
    /// Settings' debug mode went off and the page is no longer offered the
    /// wallet (4900, spec 091).
    WalletWithdrawn,
    /// Signing on a public http page (4100).
    InsecureOrigin,
    NotConnected,
    /// The request names an account this site was not shown (4100).
    AccountMismatch,
    /// A connect with no wallet account (4001).
    NoAccount,
    /// Another request's sheet is open: another site's connect (4001), or
    /// any other add-network request (-32002, spec 100).
    ConsentBusy,
    UnsupportedMethod,
    UnknownChain,
    BadParams,
    /// The tab's read queue is full (−32005).
    TooManyReads,
    /// `wallet_getCallsStatus` for a batch this wallet did not send.
    UnknownBatch,
    /// Any other refusal of the wallet's own (unlimited approval, a call
    /// that changes who controls the account, an unsupported capability…).
    WalletRefused,
    /// No endpoint answered.
    NoEndpoint,
    /// No answer within [`READ_DEADLINE_MS`].
    TimedOut,
    /// The endpoint said to slow down (HTTP 429, −32005).
    RateLimited,
    /// The endpoint's own JSON-RPC error, passed through.
    EndpointError,
    /// The operation was included and reverted.
    Reverted,
    RejectedByPerson,
    /// The relay refused the operation; nothing was sent.
    RelayRefused,
    /// The relay could not be reached; nothing was sent.
    RelayUnreachable,
    /// The relay has not reported a transaction yet; it may still land.
    NotConfirmedYet,
    RelayFailed,
    /// No passkey can be used here (e.g. an unsigned build).
    SignerUnavailable,
    /// A passkey that would never be offered at sign-in.
    SignerNotDiscoverable,
    SignerFailed,
    /// A network a page asked Vela to add lacks Vela's contracts (4902,
    /// spec 100).
    NotCompatible,
    /// A network a page asked Vela to add is not in Vela's catalog, and no
    /// RPC the page gave answers for it (-32602, spec 100).
    BadRpc,
}

impl DbrReason {
    /// The corpus line that says it — `componentsUi.browserStatus.reason.*`.
    /// The signing sheet says a signer failure with the same line.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::NavigatedAway => "componentsUi.browserStatus.reason.navigatedAway",
            Self::PageCrashed => "componentsUi.browserStatus.reason.pageCrashed",
            // Never shown — a closed tab has no status panel — so it has no
            // line of its own.
            Self::TabClosed => "componentsUi.browserStatus.reason.navigatedAway",
            Self::WalletWithdrawn => "componentsUi.browserStatus.reason.walletWithdrawn",
            Self::InsecureOrigin => "componentsUi.browserStatus.reason.insecureOrigin",
            Self::NotConnected => "componentsUi.browserStatus.reason.notConnected",
            Self::AccountMismatch => "componentsUi.browserStatus.reason.accountMismatch",
            Self::NoAccount => "componentsUi.browserStatus.reason.noAccount",
            Self::ConsentBusy => "componentsUi.browserStatus.reason.consentBusy",
            Self::UnsupportedMethod => "componentsUi.browserStatus.reason.unsupportedMethod",
            Self::UnknownChain => "componentsUi.browserStatus.reason.unknownChain",
            Self::BadParams => "componentsUi.browserStatus.reason.badParams",
            Self::TooManyReads => "componentsUi.browserStatus.reason.tooManyReads",
            Self::UnknownBatch => "componentsUi.browserStatus.reason.unknownBatch",
            Self::WalletRefused => "componentsUi.browserStatus.reason.walletRefused",
            Self::NoEndpoint => "componentsUi.browserStatus.reason.noEndpoint",
            Self::TimedOut => "componentsUi.browserStatus.reason.timedOut",
            Self::RateLimited => "componentsUi.browserStatus.reason.rateLimited",
            Self::EndpointError => "componentsUi.browserStatus.reason.endpointError",
            Self::Reverted => "componentsUi.browserStatus.reason.reverted",
            Self::RejectedByPerson => "componentsUi.browserStatus.reason.rejectedByPerson",
            Self::RelayRefused => "componentsUi.browserStatus.reason.relayRefused",
            Self::RelayUnreachable => "componentsUi.browserStatus.reason.relayUnreachable",
            Self::NotConfirmedYet => "componentsUi.browserStatus.reason.notConfirmedYet",
            Self::RelayFailed => "componentsUi.browserStatus.reason.relayFailed",
            Self::SignerUnavailable => "componentsUi.browserStatus.reason.signerUnavailable",
            Self::SignerNotDiscoverable => {
                "componentsUi.browserStatus.reason.signerNotDiscoverable"
            }
            Self::SignerFailed => "componentsUi.browserStatus.reason.signerFailed",
            // Settings' own verdict line, word for word.
            Self::NotCompatible => "addToken.errorNotCompatible",
            Self::BadRpc => "componentsUi.browserStatus.reason.badRpc",
        }
    }

    /// Worth the person's attention in the status entry: a failure the page
    /// did not cause by leaving and the person did not choose.
    #[must_use]
    pub fn is_issue(self) -> bool {
        !matches!(
            self,
            Self::NavigatedAway | Self::TabClosed | Self::RejectedByPerson
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DbrOutcome {
    #[default]
    Open,
    Answered,
    Failed,
}

/// One request a page sent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DbrRequestRow {
    /// The page's JSON-RPC id.
    pub id: String,
    pub method: String,
    pub class: DbrRequestClass,
    /// The shell's clock; `0` = unknown.
    pub started_ms: f64,
    pub ended_ms: Option<f64>,
    pub outcome: DbrOutcome,
    /// The EIP-1193 / JSON-RPC error code, for a failure.
    pub code: Option<i64>,
    pub layer: Option<DbrLayer>,
    pub reason: Option<DbrReason>,
}

impl DbrRequestRow {
    #[must_use]
    pub fn is_issue(&self) -> bool {
        self.outcome == DbrOutcome::Failed && self.reason.is_some_and(DbrReason::is_issue)
    }

    /// Milliseconds it took, when both ends are known.
    #[must_use]
    pub fn duration_ms(&self) -> Option<f64> {
        let ended = self.ended_ms?;
        (self.started_ms > 0.0 && ended >= self.started_ms).then_some(ended - self.started_ms)
    }
}

/// The status entry's one line about the latest trouble.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DbrFailureNote {
    pub layer: DbrLayer,
    pub reason: DbrReason,
    pub method: String,
    /// The line that says it ([`DbrReason::key`]).
    pub key: String,
}

/// The browser layer, as the wallet sees it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DbrPageState {
    /// Nothing loaded yet (the start page, a tab before its first load).
    #[default]
    Blank,
    Loading,
    Ready,
    Crashed,
}

impl DbrPageState {
    /// `componentsUi.browserStatus.page.*`.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Blank => "componentsUi.browserStatus.page.blank",
            Self::Loading => "componentsUi.browserStatus.page.loading",
            Self::Ready => "componentsUi.browserStatus.page.ready",
            Self::Crashed => "componentsUi.browserStatus.page.crashed",
        }
    }
}

/// The provider layer: was the wallet offered to the page showing now?
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DbrProviderState {
    /// A load is under way (or none has finished): not known yet.
    #[default]
    Pending,
    /// The page's document said hello: the wallet is there.
    Offered,
    /// Not offered: the origin is not a secure context (spec 088).
    InsecureOrigin,
    /// Not offered: the load finished and no document said hello — the
    /// script did not run (an error page, a page that blocks it).
    NoHello,
}

impl DbrProviderState {
    /// `componentsUi.browserStatus.provider.*`.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Pending => "componentsUi.browserStatus.provider.pending",
            Self::Offered => "componentsUi.browserStatus.provider.offered",
            Self::InsecureOrigin => "componentsUi.browserStatus.provider.insecureOrigin",
            Self::NoHello => "componentsUi.browserStatus.provider.noHello",
        }
    }
}

/// Why a read came back with no body.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DbrReadFailure {
    NoEndpoint,
    TimedOut,
    RateLimited,
}

/// The inspected tab, whole: the panel and a bug report draw this.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DbrInspectorView {
    pub tab: String,
    pub origin: Option<String>,
    pub page: DbrPageState,
    pub provider: DbrProviderState,
    pub connected: bool,
    pub chain_id: u32,
    /// Oldest first.
    pub rows: Vec<DbrRequestRow>,
    /// The record as plain text — what "copy" copies and a bug report
    /// carries, the same on every client.
    pub report: String,
}

/// A layer and a reason: how a request ended.
pub type Why = (DbrLayer, DbrReason);

/// How a signing request's error answer ended it — the signing machine's
/// kind, and for a failed submission its fixed sentence.
#[must_use]
pub fn sign_error_why(kind: SignErrorKind, detail: Option<&str>) -> Why {
    use DbrLayer::{Network, Relay, Sheet, Signer, Wallet};
    match kind {
        SignErrorKind::UserRejected | SignErrorKind::FundingCancelled => {
            (Sheet, DbrReason::RejectedByPerson)
        }
        SignErrorKind::UnsupportedChain => (Wallet, DbrReason::UnknownChain),
        SignErrorKind::UnauthorizedAccount => (Wallet, DbrReason::AccountMismatch),
        SignErrorKind::InvalidParams => (Wallet, DbrReason::BadParams),
        SignErrorKind::WalletSwitchedChains
        | SignErrorKind::UnsupportedCapability
        | SignErrorKind::UnlimitedApproval
        | SignErrorKind::SelfCallBlocked
        | SignErrorKind::StaleFeeQuote => (Wallet, DbrReason::WalletRefused),
        SignErrorKind::SignerUnavailable => (Signer, DbrReason::SignerUnavailable),
        SignErrorKind::SignerNotDiscoverable => (Signer, DbrReason::SignerNotDiscoverable),
        SignErrorKind::SignerFailed => (Signer, DbrReason::SignerFailed),
        SignErrorKind::SubmitFailed => {
            let detail = detail.unwrap_or_default();
            if detail == crate::user_op::REFUSED_DAPP_DETAIL {
                (Relay, DbrReason::RelayRefused)
            } else if detail == crate::user_op::NOT_SENT_DAPP_DETAIL {
                (Relay, DbrReason::RelayUnreachable)
            } else if detail.starts_with(REVERTED_MESSAGE) {
                (Network, DbrReason::Reverted)
            } else if detail.starts_with(NOT_CONFIRMED_MESSAGE) {
                (Relay, DbrReason::NotConfirmedYet)
            } else {
                (Relay, DbrReason::RelayFailed)
            }
        }
    }
}

/// How a read with no body ended.
#[must_use]
pub fn read_failure_why(class: DbrRequestClass, failure: Option<DbrReadFailure>) -> Why {
    let reason = match failure {
        Some(DbrReadFailure::TimedOut) => DbrReason::TimedOut,
        Some(DbrReadFailure::RateLimited) => DbrReason::RateLimited,
        Some(DbrReadFailure::NoEndpoint) | None => DbrReason::NoEndpoint,
    };
    (read_layer(class), reason)
}

/// How a read the endpoint answered with an error ended.
#[must_use]
pub fn endpoint_error_why(class: DbrRequestClass, code: Option<i64>) -> Why {
    let reason = match code {
        Some(429 | -32005) => DbrReason::RateLimited,
        _ => DbrReason::EndpointError,
    };
    (read_layer(class), reason)
}

fn read_layer(class: DbrRequestClass) -> DbrLayer {
    if class == DbrRequestClass::RelayRead {
        DbrLayer::Relay
    } else {
        DbrLayer::Network
    }
}

/// A wire name: what `#[serde(rename_all = "snake_case")]` writes.
fn name<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

/// The one log line for a settled row (spec 099 FR-015).
#[must_use]
pub fn row_log_line(tab: &str, row: &DbrRequestRow) -> String {
    let mut line = format!(
        "dapp tab={tab} req={} method={} class={} outcome={}",
        row.id,
        row.method,
        name(row.class),
        name(row.outcome)
    );
    if let Some(code) = row.code {
        let _ = write!(line, " code={code}");
    }
    if let (Some(layer), Some(reason)) = (row.layer, row.reason) {
        let _ = write!(line, " layer={} reason={}", name(layer), name(reason));
    }
    if let Some(ms) = row.duration_ms() {
        let _ = write!(line, " ms={ms:.0}");
    }
    line
}

/// The one log line for a tab whose page or provider state changed.
#[must_use]
pub fn tab_log_line(
    tab: &str,
    origin: Option<&str>,
    page: DbrPageState,
    provider: DbrProviderState,
) -> String {
    format!(
        "dapp tab={tab} origin={} page={} provider={}",
        origin.unwrap_or("-"),
        name(page),
        name(provider)
    )
}

/// `HH:MM:SS.mmm` UTC of a shell clock reading, or `--:--:--` unknown.
fn clock(ms: f64) -> String {
    if ms <= 0.0 || !ms.is_finite() {
        return "--:--:--.---".to_owned();
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive finite epoch reading in ms"
    )]
    let ms = ms as u64;
    let day = ms % 86_400_000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        day / 3_600_000,
        day / 60_000 % 60,
        day / 1000 % 60,
        day % 1000
    )
}

/// The copyable record of one tab (module doc: nothing private in it).
#[must_use]
pub fn report(
    tab: &str,
    origin: Option<&str>,
    page: DbrPageState,
    provider: DbrProviderState,
    connected: bool,
    chain_id: u32,
    rows: &[DbrRequestRow],
) -> String {
    let mut text = format!(
        "Vela dApp browser — tab {tab}\norigin: {}\npage: {} · wallet: {} · connected: {} · chain: {chain_id}\nrequests (UTC, oldest first): {}\n",
        origin.unwrap_or("-"),
        name(page),
        name(provider),
        if connected { "yes" } else { "no" },
        rows.len()
    );
    for row in rows {
        let _ = write!(
            text,
            "{} {} {} {}",
            clock(row.started_ms),
            row.method,
            name(row.class),
            name(row.outcome)
        );
        if let Some(code) = row.code {
            let _ = write!(text, " {code}");
        }
        if let (Some(layer), Some(reason)) = (row.layer, row.reason) {
            let _ = write!(text, " {}/{}", name(layer), name(reason));
        }
        if let Some(ms) = row.duration_ms() {
            let _ = write!(text, " {ms:.0}ms");
        }
        text.push('\n');
    }
    text
}
