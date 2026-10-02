//! Machine — payment requests (spec `016-crux-wallet-state`, US3).
//!
//! Three orthogonal sub-machines in one module (crux tutorial §5.4):
//!
//! - the **acknowledge gate**: the per-account "understand what receiving
//!   means" overlay. The QR stays covered while the flag is loading and until
//!   the account acknowledges once; copy and save unlock only after.
//! - the **request builder**: asset + optional amount → EIP-681 URI,
//!   shareable pay-link, QR value and copy payload. The copy payload in
//!   request mode is the pay-link — a web page that bridges to any wallet —
//!   never the raw `ethereum:` URI.
//! - the **network switch** (spec 090): address mode's code is the bare
//!   address by default, because every wallet can scan that. The person may
//!   switch on "include network", and the code becomes
//!   `ethereum:<address>@<chain>` for the network on screen — which many
//!   wallets cannot read, so a calm hint rides with it. Copy stays the bare
//!   address either way: people paste addresses.
//! - the **`/pay` validator**: the untrusted landing-page query → a typed
//!   `PayRequest` or the invalid surface. Strict on purpose (research.md D8):
//!   today's page crashes on `amount=1e18` (BigInt SyntaxError mid-render)
//!   and hex-parses `amount=0x10` after zero-padding into ≈7.5×10⁴ tokens
//!   prefilled into a locked Send. What is displayed must be what is encoded.
//!
//! Base-unit conversion is decimal-string arithmetic — pad, shift, trim. No
//! floats near money, no bignum dependency, no overflow, and the builder and
//! validator share the one implementation so encode and display cannot
//! diverge (FR-012).

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "PaymentRequestOperation")
)]
pub enum PaymentRequestOperation {
    /// Read `vela.receiveWarned.{account}`. A read error answers `false`
    /// (show the gate), exactly as the screen's `.catch` does today.
    ReadAck { account: String },
    /// Best-effort persist of the acknowledgement (`'1'`).
    WriteAck { account: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "PaymentRequestShellResult")
)]
pub enum PaymentRequestShellResult {
    AckFlag { acknowledged: bool },
    AckWritten,
}

impl Operation for PaymentRequestOperation {
    type Output = PaymentRequestShellResult;
}

#[effect]
pub enum PaymentRequestEffect {
    Render(RenderOperation),
    Shell(PaymentRequestOperation),
}

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// The picked asset's facts. The catalog (what can be picked, zero-balance
/// loading) is picker UI and stays in the shell — the core only needs what
/// the choice pins down.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct Asset {
    pub chain_id: u32,
    pub token_address: Option<String>,
    pub symbol: String,
    pub decimals: u32,
    pub network_name: String,
}

impl Default for Asset {
    /// Native ETH on Ethereum — the builder's default before a pick, exactly
    /// as `defaultAsset()` has it today.
    fn default() -> Self {
        Asset {
            chain_id: 1,
            token_address: None,
            symbol: "ETH".to_owned(),
            decimals: 18,
            network_name: "Ethereum".to_owned(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ReceiveMode"))]
pub enum Mode {
    #[default]
    Address,
    Request,
}

/// A validated `/pay` request — every field already normalized, the base-unit
/// amount already computed from the SAME conversion the builder uses.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct PayRequest {
    pub recipient: String,
    pub chain_id: u32,
    pub token_address: Option<String>,
    /// The human amount exactly as displayed in the headline, or `None` for
    /// an open request.
    pub amount: Option<String>,
    /// Base units as a decimal string (never a JSON number — D9), present iff
    /// `amount` is.
    pub amount_base: Option<String>,
    pub symbol: String,
    pub decimals: u32,
    pub network_name: String,
    /// The `ethereum:` URI for the "scan with another wallet" QR.
    pub eip681_uri: String,
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "PaymentRequestEvent"))]
pub enum Event {
    /// Receive-screen session start. `base_url` is the shell's origin +
    /// `/pay` (or the public fallback on native) — the core never touches
    /// `window.location`.
    Start {
        account: String,
        recipient: String,
        base_url: String,
    },
    ModeChanged {
        mode: Mode,
    },
    /// A pick re-clamps the amount to the new asset's precision.
    AssetPicked {
        chain_id: u32,
        token_address: Option<String>,
        symbol: String,
        decimals: u32,
        network_name: String,
    },
    /// Input already dot-normalized by the shell (`parseLocaleNumber`); the
    /// core applies the sanitize rules.
    AmountChanged {
        text: String,
    },
    /// The "include network" switch under an address-mode code (spec 090).
    /// Session-scoped: every `Start` turns it back off, so a code somebody
    /// shows tomorrow is the one every wallet can read.
    IncludeNetworkChanged {
        include: bool,
    },
    /// The warning gate's confirm button.
    Acknowledge,
    /// `/pay` landing-page session: the raw query, entirely untrusted.
    LinkOpened {
        to: Option<String>,
        chain: Option<String>,
        token: Option<String>,
        amount: Option<String>,
        sym: Option<String>,
        dec: Option<String>,
        net: Option<String>,
    },
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: PaymentRequestShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Gate {
    /// The flag is still loading — the QR stays covered so a first visit
    /// never flashes it.
    #[default]
    Loading,
    Unacknowledged,
    Acknowledged,
}

#[derive(Clone, Debug, Default, PartialEq)]
enum PayParse {
    #[default]
    NotOpened,
    Invalid,
    Valid(PayRequest),
}

#[derive(Default)]
pub struct Model {
    // gate
    account: String,
    gate: Gate,
    // builder
    base_url: String,
    recipient: String,
    mode: Mode,
    asset: Asset,
    amount: String,
    include_network: bool,
    // /pay
    pay: PayParse,
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct PaymentRequestView {
    // gate — the screen maps these onto the overlay/reminder/copy/save
    pub gate_loading: bool,
    pub acknowledged: bool,
    pub can_copy: bool,
    pub can_save: bool,
    // builder
    pub mode: Mode,
    pub asset: Asset,
    pub amount: String,
    pub eip681_uri: String,
    pub pay_link: String,
    /// The QR's content: the built URI in request mode (bare recipient until
    /// one is built); in address mode the bare recipient, or
    /// `ethereum:<recipient>@<chain>` while the network switch is on (spec
    /// 090). The share image encodes this same value.
    pub qr_value: String,
    /// What the copy button copies: pay-link in request mode, the bare
    /// address otherwise (FR-015) — with or without the network switch.
    pub copy_payload: String,
    /// Spec 090: whether the "include network" switch is offered — address
    /// mode with a recipient. A request's code always names its network.
    pub network_switch: bool,
    /// The switch's position. Off by default: a bare address is the code
    /// every wallet can scan.
    pub include_network: bool,
    /// The calm line under the switch ("some wallets can't read this
    /// code"), shown exactly while the address code carries the network.
    pub network_hint: bool,
    /// Drives `summaryAmount` vs `summaryOpen` — the words stay in the shell.
    pub has_amount: bool,
    // /pay
    pub pay_valid: Option<bool>,
    pub pay: Option<PayRequest>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct PaymentRequest;

impl App for PaymentRequest {
    type Event = Event;
    type Model = Model;
    type ViewModel = PaymentRequestView;
    type Effect = PaymentRequestEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<PaymentRequestEffect, Event> {
        match event {
            Event::Start {
                account,
                recipient,
                base_url,
            } => {
                model.attempt += 1;
                model.account = account.clone();
                model.recipient = recipient;
                model.base_url = base_url;
                model.include_network = false;
                model.gate = Gate::Loading;
                request(model, PaymentRequestOperation::ReadAck { account })
            }
            Event::ModeChanged { mode } => {
                model.mode = mode;
                render()
            }
            Event::IncludeNetworkChanged { include } => {
                model.include_network = include;
                render()
            }
            Event::AssetPicked {
                chain_id,
                token_address,
                symbol,
                decimals,
                network_name,
            } => {
                model.asset = Asset {
                    chain_id,
                    token_address,
                    symbol,
                    decimals,
                    network_name,
                };
                // Re-clamp: a USDC → ETH switch keeps "1.5"; an ETH → USDC
                // switch trims "1.123456789" to six decimals.
                model.amount = sanitize_amount(&model.amount, model.asset.decimals);
                render()
            }
            Event::AmountChanged { text } => {
                model.amount = sanitize_amount(&text, model.asset.decimals);
                render()
            }
            Event::Acknowledge => {
                if model.gate != Gate::Unacknowledged {
                    return Command::done();
                }
                // Optimistic, as today: the overlay lifts immediately, the
                // write is best-effort.
                model.gate = Gate::Acknowledged;
                let account = model.account.clone();
                request(model, PaymentRequestOperation::WriteAck { account })
            }
            Event::LinkOpened {
                to,
                chain,
                token,
                amount,
                sym,
                dec,
                net,
            } => {
                model.pay = validate_pay_query(to, chain, token, amount, sym, dec, net);
                render()
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    return Command::done();
                }
                match (model.gate, result) {
                    (Gate::Loading, PaymentRequestShellResult::AckFlag { acknowledged }) => {
                        model.gate = if acknowledged {
                            Gate::Acknowledged
                        } else {
                            Gate::Unacknowledged
                        };
                        render()
                    }
                    // A best-effort write acknowledged.
                    _ => Command::done(),
                }
            }
        }
    }

    fn view(&self, model: &Model) -> PaymentRequestView {
        let acknowledged = model.gate == Gate::Acknowledged;
        let is_request = model.mode == Mode::Request;

        let (eip681_uri, pay_link) = if model.recipient.is_empty() {
            (String::new(), String::new())
        } else {
            (
                build_eip681(
                    &model.recipient,
                    model.asset.chain_id,
                    model.asset.token_address.as_deref(),
                    model.asset.decimals,
                    &model.amount,
                ),
                build_pay_link(
                    &model.base_url,
                    &model.recipient,
                    model.asset.chain_id,
                    model.asset.token_address.as_deref(),
                    &model.amount,
                    &model.asset.symbol,
                    model.asset.decimals,
                    &model.asset.network_name,
                ),
            )
        };

        let network_switch = !is_request && !model.recipient.is_empty();
        let network_hint = network_switch && model.include_network;
        let qr_value = if is_request && !eip681_uri.is_empty() {
            eip681_uri.clone()
        } else if network_hint {
            // No token, no amount: "pay me on this network" — the shape the
            // scanner reads as the network alone (issue #312).
            build_eip681(&model.recipient, model.asset.chain_id, None, 0, "")
        } else {
            model.recipient.clone()
        };
        let copy_payload = if is_request {
            pay_link.clone()
        } else {
            model.recipient.clone()
        };

        let (pay_valid, pay) = match &model.pay {
            PayParse::NotOpened => (None, None),
            PayParse::Invalid => (Some(false), None),
            PayParse::Valid(request) => (Some(true), Some(request.clone())),
        };

        PaymentRequestView {
            gate_loading: model.gate == Gate::Loading,
            acknowledged,
            can_copy: acknowledged,
            can_save: acknowledged,
            mode: model.mode,
            asset: model.asset.clone(),
            amount: model.amount.clone(),
            eip681_uri,
            pay_link,
            qr_value,
            copy_payload,
            network_switch,
            include_network: model.include_network,
            network_hint,
            has_amount: has_positive_amount(&model.amount),
            pay_valid,
            pay,
        }
    }
}

// ---------------------------------------------------------------------------
// Pure money text — shared by builder and validator (FR-012)
// ---------------------------------------------------------------------------

/// Exact port of `sanitizeAmount` (`ReceiveRequestControls.tsx:39-45`),
/// including its quirk: more than one dot returns the raw input minus its
/// last character.
pub(crate) fn sanitize_amount(text: &str, max_decimals: u32) -> String {
    let cleaned: String = text
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '.')
        .collect();
    if cleaned.matches('.').count() > 1 {
        let mut chars = text.chars();
        chars.next_back();
        return chars.as_str().to_owned();
    }
    match cleaned.split_once('.') {
        Some((int_part, frac)) if frac.len() > max_decimals as usize => {
            format!("{int_part}.{}", &frac[..max_decimals as usize])
        }
        _ => cleaned,
    }
}

/// `parseFloat(amount) > 0` as it behaves on sanitized input: is there a
/// non-zero digit in the numeric prefix?
fn has_positive_amount(amount: &str) -> bool {
    let (int_part, frac) = amount.split_once('.').unwrap_or((amount, ""));
    int_part
        .bytes()
        .chain(frac.bytes())
        .any(|b| (b'1'..=b'9').contains(&b))
}

/// Human decimal → base units, by string shifting (no floats, no overflow).
/// `""` and `"."`-only inputs yield `"0"`, matching `BigInt((int||'0')+frac)`.
pub(crate) fn to_base_units(amount: &str, decimals: u32) -> String {
    let trimmed = amount.trim();
    if trimmed.is_empty() {
        return "0".to_owned();
    }
    let (int_part, frac_part) = trimmed.split_once('.').unwrap_or((trimmed, ""));
    let mut frac: String = frac_part.chars().take(decimals as usize).collect();
    while frac.len() < decimals as usize {
        frac.push('0');
    }
    let joined = format!(
        "{}{}",
        if int_part.is_empty() { "0" } else { int_part },
        frac
    );
    let stripped = joined.trim_start_matches('0');
    if stripped.is_empty() {
        "0".to_owned()
    } else {
        stripped.to_owned()
    }
}

/// Exact port of `buildEIP681` (`eip681.ts:90-103`).
pub(crate) fn build_eip681(
    recipient: &str,
    chain_id: u32,
    token_address: Option<&str>,
    decimals: u32,
    amount: &str,
) -> String {
    let has_amount = has_positive_amount(amount);
    match token_address {
        Some(token) => {
            let mut uri = format!("ethereum:{token}@{chain_id}/transfer?address={recipient}");
            if has_amount {
                uri.push_str(&format!("&uint256={}", to_base_units(amount, decimals)));
            }
            uri
        }
        None => {
            let mut uri = format!("ethereum:{recipient}@{chain_id}");
            if has_amount {
                uri.push_str(&format!("?value={}", to_base_units(amount, decimals)));
            }
            uri
        }
    }
}

/// Exact port of `buildPayLink` (`eip681.ts:198-207`) — same params, same
/// order, same encoding.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_pay_link(
    base_url: &str,
    recipient: &str,
    chain_id: u32,
    token_address: Option<&str>,
    amount: &str,
    symbol: &str,
    decimals: u32,
    network_name: &str,
) -> String {
    let mut parts = vec![
        format!("to={}", encode_uri_component(recipient)),
        format!("chain={chain_id}"),
    ];
    if let Some(token) = token_address {
        parts.push(format!("token={}", encode_uri_component(token)));
    }
    if has_positive_amount(amount) {
        parts.push(format!("amount={}", encode_uri_component(amount)));
    }
    parts.push(format!("sym={}", encode_uri_component(symbol)));
    parts.push(format!("dec={decimals}"));
    if !network_name.is_empty() {
        parts.push(format!("net={}", encode_uri_component(network_name)));
    }
    format!("{base_url}?{}", parts.join("&"))
}

/// `encodeURIComponent`: everything percent-escaped except
/// `A–Z a–z 0–9 - _ . ! ~ * ' ( )`.
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut buf = [0u8; 4];
    for c in s.chars() {
        match c {
            'A'..='Z'
            | 'a'..='z'
            | '0'..='9'
            | '-'
            | '_'
            | '.'
            | '!'
            | '~'
            | '*'
            | '\''
            | '('
            | ')' => out.push(c),
            _ => {
                for byte in c.encode_utf8(&mut buf).as_bytes() {
                    out.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// /pay validation (strict — research.md D8)
// ---------------------------------------------------------------------------

fn is_hex_address(s: &str) -> bool {
    let s = s.trim();
    s.len() == 42 && s.starts_with("0x") && s[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// Plain non-negative decimal with at most `decimals` fractional digits.
/// Rejects everything today's page mishandles: scientific notation (crash),
/// hex (silent misparse), comma decimals, negatives, non-ASCII digits, and
/// over-precision (silent truncation ⇒ displayed ≠ encoded). Accepts every
/// shape our own builder can emit — the sanitizer allows `.5` and `1.`, and
/// links carrying them are already in the wild, so the grammar is exactly
/// "ASCII digits with at most one dot, at least one digit".
fn is_strict_amount(s: &str, decimals: u32) -> bool {
    let (int_part, frac) = match s.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (s, None),
    };
    if !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }
    let frac_ok = match frac {
        None => true,
        Some(f) => f.bytes().all(|b| b.is_ascii_digit()) && f.len() <= decimals as usize,
    };
    frac_ok && !(int_part.is_empty() && frac.is_none_or(str::is_empty))
}

fn validate_pay_query(
    to: Option<String>,
    chain: Option<String>,
    token: Option<String>,
    amount: Option<String>,
    sym: Option<String>,
    dec: Option<String>,
    net: Option<String>,
) -> PayParse {
    let recipient = to.unwrap_or_default().trim().to_owned();
    if !is_hex_address(&recipient) {
        return PayParse::Invalid;
    }

    let chain = chain.unwrap_or_default();
    let chain = chain.trim();
    if chain.is_empty() || !chain.bytes().all(|b| b.is_ascii_digit()) {
        return PayParse::Invalid;
    }
    let Ok(chain_id) = chain.parse::<u32>() else {
        return PayParse::Invalid;
    };

    // `parseInt(dec ?? '18') || 18`: unparseable OR zero falls back to 18 —
    // ported faithfully, quirk included.
    let decimals = dec
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|d| *d != 0)
        .unwrap_or(18);

    let token_address = Some(token.unwrap_or_default().trim().to_owned()).filter(|t| !t.is_empty());

    let amount = Some(amount.unwrap_or_default().trim().to_owned()).filter(|a| !a.is_empty());
    if let Some(a) = &amount {
        if !is_strict_amount(a, decimals) {
            return PayParse::Invalid;
        }
    }
    let amount_base = amount.as_deref().map(|a| to_base_units(a, decimals));

    let symbol = Some(sym.unwrap_or_default().trim().to_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "tokens".to_owned());
    let network_name = Some(net.unwrap_or_default().trim().to_owned())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| format!("Chain {chain_id}"));

    let eip681_uri = build_eip681(
        &recipient,
        chain_id,
        token_address.as_deref(),
        decimals,
        amount.as_deref().unwrap_or(""),
    );

    PayParse::Valid(PayRequest {
        recipient,
        chain_id,
        token_address,
        amount,
        amount_base,
        symbol,
        decimals,
        network_name,
        eip681_uri,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Issue one operation whose answer must match the current attempt.
fn request(
    model: &mut Model,
    operation: PaymentRequestOperation,
) -> Command<PaymentRequestEffect, Event> {
    let attempt = model.attempt;
    Command::all([
        Command::request_from_shell(operation)
            .then_send(move |result| Event::ShellCompleted { attempt, result }),
        render(),
    ])
}

impl super::SplitEffect for PaymentRequestEffect {
    type Op = PaymentRequestOperation;
    fn into_shell(self) -> Option<crux_core::Request<PaymentRequestOperation>> {
        match self {
            PaymentRequestEffect::Render(_) => None,
            PaymentRequestEffect::Shell(request) => Some(request),
        }
    }
}
