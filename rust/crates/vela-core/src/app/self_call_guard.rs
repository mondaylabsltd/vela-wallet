//! The self-call guard — a dApp may never ask the wallet to rewrite who
//! controls it (spec 081, FR-005).
//!
//! A Vela account is a Safe whose owners, threshold, modules, guard and
//! fallback handler are set at creation and never touched by Vela again. A
//! dApp, however, can ask for an ordinary transaction whose target happens to
//! be the account itself: `addOwnerWithThreshold`, `enableModule`,
//! `setFallbackHandler`… Signed once, any of those hands the account over as
//! completely as the payload that drained Bybit — and the old behaviour only
//! *decoded* them, leaving the user to notice.
//!
//! This kernel is pure calldata inspection, so the rule is decided once for
//! all four shells (`docs/ARCHITECTURE.md`: business rules live in the core).
//! It is deliberately narrow:
//!
//! - it applies to **dApp-originated params only** — Vela's own flows never
//!   pass through it;
//! - a self-call with **empty calldata is allowed**, because the in-band fee
//!   leg and the gas estimator use exactly that shape;
//! - it recurses through `multiSend` payloads and `execTransaction` wrappers,
//!   because a batch hides the same call one level down;
//! - any **inner `delegatecall`** inside a batch is refused whatever its
//!   target: a dApp cannot express one through `{to, value, data}`, so its
//!   presence means hand-built calldata aimed at the account's storage.

use serde_json::Value;

use crate::primitives::to_hex;

/// How deep to follow nested `multiSend` / `execTransaction` payloads. Four is
/// past anything legitimate; the cap is what stops a crafted payload from
/// costing more to inspect than to submit.
const MAX_DEPTH: u8 = 4;

/// The Safe function a blocked request would have called.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelfCallFunction {
    AddOwner,
    RemoveOwner,
    SwapOwner,
    ChangeThreshold,
    EnableModule,
    DisableModule,
    SetGuard,
    SetModuleGuard,
    SetFallbackHandler,
    Setup,
    ExecTransaction,
    ExecTransactionFromModule,
    /// Not a call: an EIP-712 `SafeTx` the dApp asked the key to authorise.
    SafeTxTypedData,
    /// A `delegatecall` leg inside a batch — the Bybit primitive.
    DelegateCall,
}

impl SelfCallFunction {
    /// Stable wire name; the shells map it to a translated sentence.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AddOwner => "addOwnerWithThreshold",
            Self::RemoveOwner => "removeOwner",
            Self::SwapOwner => "swapOwner",
            Self::ChangeThreshold => "changeThreshold",
            Self::EnableModule => "enableModule",
            Self::DisableModule => "disableModule",
            Self::SetGuard => "setGuard",
            Self::SetModuleGuard => "setModuleGuard",
            Self::SetFallbackHandler => "setFallbackHandler",
            Self::Setup => "setup",
            Self::ExecTransaction => "execTransaction",
            Self::ExecTransactionFromModule => "execTransactionFromModule",
            Self::SafeTxTypedData => "SafeTx",
            Self::DelegateCall => "delegatecall",
        }
    }
}

/// Why a request was refused, and where in it the offending call sat.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelfCallBlock {
    pub function: SelfCallFunction,
    /// `0x…` four bytes; empty for the typed-data case.
    pub selector: String,
    /// 1-based position in a `wallet_sendCalls` batch, when that is where it was.
    pub leg_index: Option<u32>,
    /// True when it was found inside a `multiSend` or `execTransaction` payload.
    pub nested: bool,
}

/// Safe v1.4.1 ownership and configuration selectors. `setModuleGuard` is
/// v1.5's; listing it now costs nothing and closes the gap the day a wallet
/// runs on one.
const BLOCKED: &[(&str, SelfCallFunction)] = &[
    ("0x0d582f13", SelfCallFunction::AddOwner),
    ("0xf8dc5dd9", SelfCallFunction::RemoveOwner),
    ("0xe318b52b", SelfCallFunction::SwapOwner),
    ("0x694e80c3", SelfCallFunction::ChangeThreshold),
    ("0x610b5925", SelfCallFunction::EnableModule),
    ("0xe009cfde", SelfCallFunction::DisableModule),
    ("0xe19a9dd9", SelfCallFunction::SetGuard),
    ("0xe068df37", SelfCallFunction::SetModuleGuard),
    ("0xf08a0323", SelfCallFunction::SetFallbackHandler),
    ("0xb63e800d", SelfCallFunction::Setup),
    ("0x6a761202", SelfCallFunction::ExecTransaction),
    ("0x468721a7", SelfCallFunction::ExecTransactionFromModule),
    ("0x5229073f", SelfCallFunction::ExecTransactionFromModule),
];

const MULTI_SEND: &str = "0x8d80ff0a";
const EXEC_TRANSACTION: &str = "0x6a761202";

fn blocked_function(selector: &str) -> Option<SelfCallFunction> {
    BLOCKED
        .iter()
        .find(|(sel, _)| sel.eq_ignore_ascii_case(selector))
        .map(|(_, f)| *f)
}

fn same_address(a: &str, b: &str) -> bool {
    a.trim().trim_start_matches("0x").eq_ignore_ascii_case(b.trim().trim_start_matches("0x"))
}

/// Raw bytes of a `0x…` string; odd or non-hex input yields what parsed, which
/// is enough to read a selector and never panics on hostile input.
fn hex_bytes(data: &str) -> Vec<u8> {
    let body = data.trim().trim_start_matches("0x");
    let usable = body.len() - (body.len() % 2);
    (0..usable)
        .step_by(2)
        .map_while(|i| u8::from_str_radix(&body[i..i + 2], 16).ok())
        .collect()
}

fn selector_of(data: &str) -> Option<String> {
    let bytes = hex_bytes(data);
    (bytes.len() >= 4).then(|| to_hex(&bytes[..4], true))
}

/// One leg of a `multiSend` payload: `operation(1) to(20) value(32) len(32) data(len)`.
struct InnerCall {
    operation: u8,
    to: String,
    data: String,
}

fn decode_multi_send(data: &str) -> Vec<InnerCall> {
    let bytes = hex_bytes(data);
    // selector(4) + head: offset(32) + length(32), then the packed payload.
    if bytes.len() < 4 + 64 {
        return Vec::new();
    }
    let len = u32_at(&bytes, 4 + 32 + 28) as usize;
    let payload = &bytes[4 + 64..];
    let payload = &payload[..len.min(payload.len())];

    let mut calls = Vec::new();
    let mut i = 0usize;
    while i + 85 <= payload.len() {
        let operation = payload[i];
        let to = to_hex(&payload[i + 1..i + 21], true);
        let data_len = u32_at(payload, i + 53 + 28) as usize;
        let start = i + 85;
        let end = start.saturating_add(data_len);
        if end > payload.len() {
            break;
        }
        calls.push(InnerCall {
            operation,
            to,
            data: to_hex(&payload[start..end], true),
        });
        i = end;
    }
    calls
}

/// The low 4 bytes of the 32-byte word starting at `at - 28`, which is where a
/// length or a small number lives in ABI encoding.
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    if at + 4 > bytes.len() {
        return 0;
    }
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// `execTransaction(address to, uint256 value, bytes data, uint8 operation, …)`
/// — the three fields that decide whether the inner call is a takeover.
fn decode_exec_transaction(data: &str) -> Option<InnerCall> {
    let bytes = hex_bytes(data);
    if bytes.len() < 4 + 32 * 4 {
        return None;
    }
    let word = |n: usize| &bytes[4 + n * 32..4 + (n + 1) * 32];
    let to = to_hex(&word(0)[12..], true);
    let operation = *word(3).last().unwrap_or(&0);
    let data_offset = u32_at(&bytes, 4 + 2 * 32 + 28) as usize;
    let inner = if 4 + data_offset + 32 <= bytes.len() {
        let len = u32_at(&bytes, 4 + data_offset + 28) as usize;
        let start = 4 + data_offset + 32;
        let end = start.saturating_add(len);
        if end <= bytes.len() {
            to_hex(&bytes[start..end], true)
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    Some(InnerCall {
        operation,
        to,
        data: inner,
    })
}

/// Inspect one `{to, data}` pair (and anything it carries) against the account.
fn inspect_call(
    to: &str,
    data: &str,
    safe: &str,
    leg_index: Option<u32>,
    nested: bool,
    depth: u8,
) -> Option<SelfCallBlock> {
    let selector = selector_of(data)?;

    if same_address(to, safe) {
        if let Some(function) = blocked_function(&selector) {
            return Some(SelfCallBlock {
                function,
                selector,
                leg_index,
                nested,
            });
        }
    }

    if depth >= MAX_DEPTH {
        return None;
    }

    if selector.eq_ignore_ascii_case(MULTI_SEND) {
        for inner in decode_multi_send(data) {
            // A delegatecall leg rewrites this account's storage with someone
            // else's code — the Bybit primitive. A dApp cannot express one
            // through `{to, value, data}`, so seeing it means crafted calldata.
            if inner.operation == 1 {
                return Some(SelfCallBlock {
                    function: SelfCallFunction::DelegateCall,
                    selector: selector_of(&inner.data).unwrap_or_default(),
                    leg_index,
                    nested: true,
                });
            }
            if let Some(block) =
                inspect_call(&inner.to, &inner.data, safe, leg_index, true, depth + 1)
            {
                return Some(block);
            }
        }
    }

    if selector.eq_ignore_ascii_case(EXEC_TRANSACTION) {
        if let Some(inner) = decode_exec_transaction(data) {
            if inner.operation == 1 {
                return Some(SelfCallBlock {
                    function: SelfCallFunction::DelegateCall,
                    selector: selector_of(&inner.data).unwrap_or_default(),
                    leg_index,
                    nested: true,
                });
            }
            if let Some(block) =
                inspect_call(&inner.to, &inner.data, safe, leg_index, true, depth + 1)
            {
                return Some(block);
            }
        }
    }

    None
}

fn str_field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

/// The verdict on a dApp request. `None` means nothing about it touches who
/// controls the account.
///
/// `safe` is the address that would sign — the account the request runs as.
pub fn detect_self_call(method: &str, params: Option<&Value>, safe: &str) -> Option<SelfCallBlock> {
    let params = params?;
    if safe.trim().is_empty() {
        return None;
    }

    if method.contains("signTypedData") {
        return detect_safe_tx_typed_data(params);
    }

    match method {
        "eth_sendTransaction" => {
            let tx = params.get(0)?;
            inspect_call(
                str_field(tx, "to")?,
                str_field(tx, "data").unwrap_or_default(),
                safe,
                None,
                false,
                0,
            )
        }
        "wallet_sendCalls" => {
            let calls = params.get(0)?.get("calls")?.as_array()?;
            for (index, call) in calls.iter().enumerate() {
                let Some(to) = str_field(call, "to") else {
                    continue;
                };
                if let Some(block) = inspect_call(
                    to,
                    str_field(call, "data").unwrap_or_default(),
                    safe,
                    Some(index as u32 + 1),
                    false,
                    0,
                ) {
                    return Some(block);
                }
            }
            None
        }
        _ => None,
    }
}

/// A `SafeTx` is an instruction to a Safe, signed for later execution by
/// anyone. Vela wraps dApp typed data in `SafeMessage(...)`, so one signed for
/// *this* account is not directly executable — but if this account is an owner
/// of another Safe, the same signature authorises that parent through
/// EIP-1271. No dApp has a legitimate reason to ask, so all of them are
/// refused.
fn detect_safe_tx_typed_data(params: &Value) -> Option<SelfCallBlock> {
    let payload = params
        .as_array()?
        .iter()
        .find_map(|entry| match entry {
            Value::String(raw) => serde_json::from_str::<Value>(raw).ok(),
            Value::Object(_) => Some(entry.clone()),
            _ => None,
        })?;

    (payload.get("primaryType").and_then(Value::as_str) == Some("SafeTx")).then(|| SelfCallBlock {
        function: SelfCallFunction::SafeTxTypedData,
        selector: String::new(),
        leg_index: None,
        nested: false,
    })
}

/// Fail-closed twin of [`detect_self_call`], for the submit chokepoint: the
/// shell may hand back rewritten params, and those are the ones that get
/// signed.
pub fn enforce_no_self_call(
    method: &str,
    params: Option<&Value>,
    safe: &str,
) -> Result<(), SelfCallBlock> {
    match detect_self_call(method, params, safe) {
        Some(block) => Err(block),
        None => Ok(()),
    }
}
