//! Machine — how this device signs by default (spec `071-clear-signer`).
//!
//! ```text
//! refresh ─► LoadingStored ─► commit {method, signer url}
//! method_chosen ─► persist + commit at once
//! url_submitted ─┬─ an https / loopback page ─► persist + commit
//!                └─ anything else ─► refused, nothing stored, the old page stands
//! url_reset ─► remove + back to the official page
//! tunnel_submitted / tunnel_reset ─► the same, for the tunnel (spec 075)
//! ```
//!
//! Two preferences that belong together: the "Sign with" a signing sheet
//! starts at (`auto`, a place a passkey is, or the Clear Signer), and which
//! Clear Signer page the wallet opens. Shaped on [`super::fee_tier_pref`],
//! this codebase's committed-preference machine, for the same reasons: the
//! shell owns the keys (`vela.signMethod`, `vela.clearSignerUrl`, and since
//! spec 075 `vela.clearSignerTunnel` — the tunnel a pairing goes through — under the
//! `vela.` prefix that survives sign-out — how a person signs belongs to them
//! and the device, not to one account) and the words; the core decides what
//! may be stored and what shows when nothing can be.
//!
//! **A per-request choice never comes here.** The signing sheet's "Sign with"
//! row changes one request; the next starts at this default again. Only
//! Settings dispatches [`Event::MethodChosen`].

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use crate::clear_signer::{self, SignerUrlError};
use crate::wallet_keys::SIGN_METHODS;

/// The factory default: do what the wallet always did.
pub const FACTORY_METHOD: &str = "auto";

/// A stored method name, or `None` when it is not one this build offers —
/// which reads as "never chose" rather than being rewritten, so a newer
/// build's value survives a trip through an older one.
pub fn parse_method(raw: &str) -> Option<&'static str> {
    SIGN_METHODS
        .iter()
        .copied()
        .find(|method| *method == raw.trim())
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SignPrefOperation"))]
pub enum SignPrefOperation {
    /// Read `vela.signMethod`, `vela.clearSignerUrl` and
    /// `vela.clearSignerTunnel`, raw.
    ReadStored,
    /// Persist the default method (best effort).
    WriteMethod { method: String },
    /// Persist the signer page; `None` removes the key — the official page.
    WriteSignerUrl { url: Option<String> },
    /// Spec 075: persist the tunnel; `None` removes the key — the official one.
    WriteTunnelUrl { url: Option<String> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SignPrefShellResult"))]
pub enum SignPrefShellResult {
    Stored {
        method: Option<String>,
        signer_url: Option<String>,
        /// Spec 075; absent from a shell that predates the tunnel.
        #[serde(default)]
        tunnel_url: Option<String>,
    },
    Written,
}

impl Operation for SignPrefOperation {
    type Output = SignPrefShellResult;
}

#[effect]
pub enum SignPrefEffect {
    Render(RenderOperation),
    Shell(SignPrefOperation),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SignPrefEvent"))]
pub enum Event {
    /// Mount / focus — re-read. Coalesced while a read is in flight.
    Refresh,
    /// Settings: the default "Sign with". A name this build does not offer is
    /// ignored.
    MethodChosen { method: String },
    /// Settings: the Clear Signer page, as typed.
    SignerUrlSubmitted { text: String },
    /// Settings: back to the official page.
    SignerUrlReset,
    /// Spec 075 — Settings: the tunnel a pairing goes through, as typed.
    TunnelUrlSubmitted { text: String },
    /// Spec 075 — Settings: back to the official tunnel.
    TunnelUrlReset,
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: SignPrefShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model / view
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    LoadingStored,
}

#[derive(Default)]
pub struct Model {
    method: Option<&'static str>,
    /// A page the person chose, normalised. `None` ⇒ the official one.
    signer_url: Option<String>,
    /// Why the last submitted address was refused, until the next submit or
    /// reset.
    url_error: Option<SignerUrlError>,
    /// Spec 075: a tunnel the person chose, normalised. `None` ⇒ the official one.
    tunnel_url: Option<String>,
    tunnel_error: Option<SignerUrlError>,
    phase: Phase,
    attempt: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignPrefView {
    /// The "Sign with" every signing sheet starts at. Always an offered name.
    pub method: String,
    /// `false` ⇒ `method` is the factory default, not a choice.
    pub method_committed: bool,
    /// Every "Sign with" value, in the order a picker lists them.
    pub offered: Vec<String>,
    /// The Clear Signer page the wallet opens. Always usable.
    pub signer_url: String,
    /// `true` ⇒ the official page (nothing chosen, or the choice was reset).
    pub signer_url_is_default: bool,
    /// `"invalid"` | `"insecure"` — the last submitted address was refused and
    /// nothing was stored. `None` otherwise.
    pub signer_url_error: Option<String>,
    /// Whether a page at `signer_url` can use this wallet's passkeys (they are
    /// `getvela.app` keys). A page elsewhere can show a request but not sign
    /// it, and Settings says so beside the address.
    pub signer_uses_wallet_passkeys: bool,
    /// Spec 075: the tunnel a cross-device pairing goes through. Always usable.
    pub tunnel_url: String,
    /// `true` ⇒ the official tunnel.
    pub tunnel_url_is_default: bool,
    /// `"invalid"` | `"insecure"` — the last submitted tunnel was refused.
    pub tunnel_url_error: Option<String>,
}

#[derive(Default)]
pub struct SignPref;

impl App for SignPref {
    type Event = Event;
    type Model = Model;
    type ViewModel = SignPrefView;
    type Effect = SignPrefEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<SignPrefEffect, Event> {
        match event {
            Event::Refresh => {
                if model.phase != Phase::Idle {
                    return Command::done();
                }
                model.attempt += 1;
                model.phase = Phase::LoadingStored;
                shell(model, SignPrefOperation::ReadStored)
            }
            Event::MethodChosen { method } => {
                let Some(method) = parse_method(&method) else {
                    return Command::done();
                };
                model.attempt += 1;
                model.phase = Phase::Idle;
                model.method = Some(method);
                shell(
                    model,
                    SignPrefOperation::WriteMethod {
                        method: method.to_owned(),
                    },
                )
            }
            Event::SignerUrlSubmitted { text } => match clear_signer::signer_url(&text) {
                Ok(url) => {
                    model.attempt += 1;
                    model.phase = Phase::Idle;
                    model.url_error = None;
                    // Typing the official page back in is a reset, not a pin:
                    // the default may move, and this person did not choose a
                    // copy of it.
                    let chosen = (url != clear_signer::DEFAULT_SIGNER_URL).then_some(url);
                    model.signer_url.clone_from(&chosen);
                    shell(model, SignPrefOperation::WriteSignerUrl { url: chosen })
                }
                Err(error) => {
                    model.url_error = Some(error);
                    render()
                }
            },
            Event::SignerUrlReset => {
                model.attempt += 1;
                model.phase = Phase::Idle;
                model.url_error = None;
                model.signer_url = None;
                shell(model, SignPrefOperation::WriteSignerUrl { url: None })
            }
            Event::TunnelUrlSubmitted { text } => match clear_signer::tunnel_url(&text) {
                Ok(url) => {
                    model.attempt += 1;
                    model.phase = Phase::Idle;
                    model.tunnel_error = None;
                    let chosen = (url != clear_signer::DEFAULT_TUNNEL_URL).then_some(url);
                    model.tunnel_url.clone_from(&chosen);
                    shell(model, SignPrefOperation::WriteTunnelUrl { url: chosen })
                }
                Err(error) => {
                    model.tunnel_error = Some(error);
                    render()
                }
            },
            Event::TunnelUrlReset => {
                model.attempt += 1;
                model.phase = Phase::Idle;
                model.tunnel_error = None;
                model.tunnel_url = None;
                shell(model, SignPrefOperation::WriteTunnelUrl { url: None })
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // Superseded — a stored value that arrived after a choice.
                    return Command::done();
                }
                match (model.phase, result) {
                    (
                        Phase::LoadingStored,
                        SignPrefShellResult::Stored {
                            method,
                            signer_url,
                            tunnel_url,
                        },
                    ) => {
                        model.phase = Phase::Idle;
                        model.method = method.as_deref().and_then(parse_method);
                        // Validated again: a key someone else wrote (or an
                        // older rule accepted) must not open a page this build
                        // would refuse to store.
                        model.signer_url = signer_url
                            .as_deref()
                            .and_then(|raw| clear_signer::signer_url(raw).ok())
                            .filter(|url| url != clear_signer::DEFAULT_SIGNER_URL);
                        model.tunnel_url = tunnel_url
                            .as_deref()
                            .and_then(|raw| clear_signer::tunnel_url(raw).ok())
                            .filter(|url| url != clear_signer::DEFAULT_TUNNEL_URL);
                        render()
                    }
                    _ => Command::done(),
                }
            }
        }
    }

    fn view(&self, model: &Model) -> SignPrefView {
        let signer_url = model
            .signer_url
            .clone()
            .unwrap_or_else(|| clear_signer::DEFAULT_SIGNER_URL.to_owned());
        let error_name = |error: SignerUrlError| {
            match error {
                SignerUrlError::Invalid => "invalid",
                SignerUrlError::Insecure => "insecure",
            }
            .to_owned()
        };
        SignPrefView {
            method: model.method.unwrap_or(FACTORY_METHOD).to_owned(),
            method_committed: model.method.is_some(),
            offered: SIGN_METHODS.iter().map(|m| (*m).to_owned()).collect(),
            signer_uses_wallet_passkeys: clear_signer::uses_wallet_passkeys(&signer_url),
            signer_url_is_default: model.signer_url.is_none(),
            signer_url,
            signer_url_error: model.url_error.map(error_name),
            tunnel_url: model
                .tunnel_url
                .clone()
                .unwrap_or_else(|| clear_signer::DEFAULT_TUNNEL_URL.to_owned()),
            tunnel_url_is_default: model.tunnel_url.is_none(),
            tunnel_url_error: model.tunnel_error.map(error_name),
        }
    }
}

fn shell(model: &Model, operation: SignPrefOperation) -> Command<SignPrefEffect, Event> {
    let attempt = model.attempt;
    Command::all([
        Command::request_from_shell(operation)
            .then_send(move |result| Event::ShellCompleted { attempt, result }),
        render(),
    ])
}

impl super::SplitEffect for SignPrefEffect {
    type Op = SignPrefOperation;
    fn into_shell(self) -> Option<crux_core::Request<SignPrefOperation>> {
        match self {
            SignPrefEffect::Render(_) => None,
            SignPrefEffect::Shell(request) => Some(request),
        }
    }
}
