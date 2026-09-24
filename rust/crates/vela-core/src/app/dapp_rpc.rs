//! Pure — what a page may ask a Vela wallet, and the wire it asks over
//! (spec 070).
//!
//! Before this module the routing table existed four times — the extension's
//! `lib/protocol.js`, the desktop's `executor/dapp_rpc.rs`, iOS
//! `DappRpc.swift`, Android `DappRpc.kt` — each pinned to the JS by its own
//! parity test and each answering the edges differently (`eth_sign` 4200 in
//! the extension, 4900 "disconnected" on native; `wallet_addEthereumChain`
//! switching in one and silently not switching in three). The in-app browsers
//! now route through [`classify`] inside the `dapp_browser` machine; the
//! extension's service worker keeps its JS table (it cannot run the core on
//! every page load) and a web unit test replays it against this one over wasm.
//!
//! Also here, because every in-app browser needs exactly the same bytes:
//!
//! - [`provider_script`] — THE page-side provider (`provider/inpage.js`, the
//!   file the extension bundles too) plus the ONE bridge. Hosts differ only
//!   in the function that posts a string to native code.
//! - [`parse_page_message`] — the bridge's envelope, validated the way the
//!   extension's `isWellFormedRequest` validates it.
//! - [`result_json`] / [`error_json`] / [`event_json`] — what is delivered
//!   back, addressed to one document.
//! - [`browser_input`] — address-bar text → the URL to load.
//! - [`sign_error_message`] — the words a page reads for a signing refusal,
//!   formerly a table in every shell's signing executor.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::sign_request::SignErrorKind;

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// The postMessage channel tag every page-side message carries.
pub const CHANNEL: &str = "vela-1193";

/// THE page-side provider, classic script (spec 070 research R3).
pub const PROVIDER_JS: &str = include_str!("../../provider/inpage.js");

/// The largest params payload a page may send (`protocol.js`
/// `MAX_REQUEST_BYTES`). Typed data and batched calls fit comfortably; a
/// page sending more is trying its luck with the wallet's memory.
pub const MAX_REQUEST_BYTES: usize = 512 * 1024;
/// Request ids are the page's own (`SESSION_UUID:seq`), never longer.
pub const MAX_ID_CHARS: usize = 128;
pub const MAX_METHOD_CHARS: usize = 100;

// ---------------------------------------------------------------------------
// The table — byte-equal to `extension/lib/protocol.js`
// ---------------------------------------------------------------------------

/// The node reads the app itself advertises (`READ_ONLY_RPC_METHODS`).
pub const READ_ONLY_RPC_METHODS: [&str; 21] = [
    "eth_call",
    "eth_estimateGas",
    "eth_getBalance",
    "eth_getCode",
    "eth_getStorageAt",
    "eth_getTransactionCount",
    "eth_getTransactionByHash",
    "eth_getTransactionReceipt",
    "eth_getLogs",
    "eth_blockNumber",
    "eth_getBlockByNumber",
    "eth_getBlockByHash",
    "eth_feeHistory",
    "eth_gasPrice",
    "eth_maxPriorityFeePerGas",
    "eth_newFilter",
    "eth_newBlockFilter",
    "eth_getFilterChanges",
    "eth_uninstallFilter",
    "eth_sendRawTransaction",
    "eth_syncing",
];

/// Routed to the ERC-4337 bundler, not the node (`BUNDLER_METHODS`).
pub const BUNDLER_METHODS: [&str; 5] = [
    "eth_sendUserOperation",
    "eth_estimateUserOperationGas",
    "eth_getUserOperationReceipt",
    "eth_getUserOperationByHash",
    "pimlico_getUserOperationGasPrice",
];

/// The rest of `READ_PROXY_METHODS`: proxied, never advertised.
pub const EXTRA_READ_METHODS: [&str; 9] = [
    "eth_getBlockReceipts",
    "eth_getProof",
    "eth_createAccessList",
    "eth_getFilterLogs",
    "eth_getTransactionByBlockHashAndIndex",
    "eth_getTransactionByBlockNumberAndIndex",
    "eth_getBlockTransactionCountByHash",
    "eth_getBlockTransactionCountByNumber",
    "web3_clientVersion",
];

/// Who answers a request.
///
/// An ALLOWLIST, not a denylist: a method nothing below names is
/// [`Route::Unsupported`], never forwarded. Denylist routing fails OPEN —
/// `eth_signTransaction` is not a signing method by any test here, so a
/// catch-all read bucket would hand it to a public node and turn the wallet
/// into an open RPC relay for any site it renders.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DappRoute"))]
pub enum Route {
    /// `eth_requestAccounts`, `wallet_requestPermissions` — may open consent.
    Connect,
    /// `eth_accounts` — the grant, never a prompt.
    Accounts,
    /// `eth_coinbase` — the first granted account or `null`.
    Coinbase,
    /// `wallet_getPermissions` — EIP-2255, from the grant.
    Permissions,
    /// `wallet_revokePermissions` — the site disconnects itself.
    RevokePermissions,
    ChainId,
    NetVersion,
    SwitchChain,
    /// `wallet_addEthereumChain` — a switch when the wallet has the chain;
    /// networks are added in Settings, by a person, never by a page.
    AddChain,
    /// `wallet_watchAsset` — `false`: tokens join through Vela's own trust
    /// rules, not because a page asked.
    WatchAsset,
    /// Needs a signature: the signing sheet.
    Sign,
    /// A node (or bundler) read on the site's chain.
    Read {
        bundler: bool,
    },
    /// Refused — 4200. `eth_sign` lands here on purpose (a blind digest).
    Unsupported,
}

/// "This needs a passkey" — the one predicate (`protocol.js`
/// `isSigningMethod`, minus `eth_sign`, which [`classify`] refuses before
/// asking).
pub fn is_signing_method(method: &str) -> bool {
    method == "eth_sendTransaction"
        || method == "wallet_sendCalls"
        || method == "personal_sign"
        || method.contains("signTypedData")
}

pub fn classify(method: &str) -> Route {
    if method == "eth_sign" {
        return Route::Unsupported;
    }
    if is_signing_method(method) {
        return Route::Sign;
    }
    match method {
        "eth_requestAccounts" | "wallet_requestPermissions" => Route::Connect,
        "eth_accounts" => Route::Accounts,
        "eth_coinbase" => Route::Coinbase,
        "wallet_getPermissions" => Route::Permissions,
        "wallet_revokePermissions" => Route::RevokePermissions,
        "eth_chainId" => Route::ChainId,
        "net_version" => Route::NetVersion,
        "wallet_switchEthereumChain" => Route::SwitchChain,
        "wallet_addEthereumChain" => Route::AddChain,
        "wallet_watchAsset" => Route::WatchAsset,
        _ if BUNDLER_METHODS.contains(&method) => Route::Read { bundler: true },
        _ if READ_ONLY_RPC_METHODS.contains(&method) || EXTRA_READ_METHODS.contains(&method) => {
            Route::Read { bundler: false }
        }
        _ => Route::Unsupported,
    }
}

/// The chain `wallet_switchEthereumChain` / `wallet_addEthereumChain` names:
/// `[{ chainId: "0x64" }]`, `"100"` or `100`. `None` = nothing usable (the
/// page gets -32602). Zero is not a chain.
pub fn chain_param(params: &Value) -> Option<u32> {
    let value = params.as_array()?.first()?.as_object()?.get("chainId")?;
    let parsed = match value {
        Value::Number(n) => n.as_u64(),
        Value::String(text) => {
            if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                if hex.is_empty() || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return None;
                }
                u64::from_str_radix(hex, 16).ok()
            } else if !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit()) {
                text.parse::<u64>().ok()
            } else {
                None
            }
        }
        _ => None,
    }?;
    u32::try_from(parsed).ok().filter(|chain| *chain > 0)
}

/// The account a signing request asks to act as, when it names one:
/// `eth_sendTransaction` / `wallet_sendCalls` → `params[0].from`;
/// `personal_sign` → `params[1]`; `eth_signTypedData_v3`/`_v4` →
/// `params[0]`; `eth_signTypedData`/`_v1` → `params[1]` (their order is
/// `[data, address]`). Only a well-formed address counts.
pub fn requested_address(method: &str, params: &Value) -> Option<String> {
    let list = params.as_array()?;
    let candidate = match method {
        "eth_sendTransaction" | "wallet_sendCalls" => list.first()?.get("from")?.as_str()?,
        "personal_sign" | "eth_signTypedData" | "eth_signTypedData_v1" => list.get(1)?.as_str()?,
        m if m.contains("signTypedData") => list.first()?.as_str()?,
        _ => return None,
    };
    let is_address = candidate.len() == 42
        && candidate.starts_with("0x")
        && candidate[2..].bytes().all(|b| b.is_ascii_hexdigit());
    is_address.then(|| candidate.to_owned())
}

/// `1` → `"0x1"`.
pub fn hex_chain_id(chain_id: u32) -> String {
    format!("0x{chain_id:x}")
}

// ---------------------------------------------------------------------------
// The page → native envelope
// ---------------------------------------------------------------------------

/// One validated request.
#[derive(Clone, Debug, PartialEq)]
pub struct PageRequest {
    pub doc: String,
    pub id: String,
    pub method: String,
    /// A JSON array (absent params arrive as `[]`) — or, for
    /// `wallet_watchAsset` alone, the object EIP-747 specifies.
    pub params: Value,
}

/// What a bridge message turned out to be.
#[derive(Clone, Debug, PartialEq)]
pub enum PageMessage {
    /// A document started — sent before the page's own scripts run (and again
    /// when a back-forward-cache restore brings it back). `href` is the
    /// document's own URL: it is what ties a hello to the navigation that
    /// produced it when the platform reports that navigation late.
    Hello {
        doc: String,
        href: Option<String>,
    },
    Request(PageRequest),
    /// Refused on shape. `id` is present when the page can be told; without
    /// one there is nobody to answer and the message is dropped.
    Invalid {
        doc: Option<String>,
        id: Option<String>,
        code: i64,
        message: &'static str,
    },
    /// Not ours (no `t`, or an unknown `t`). Dropped silently.
    Ignored,
}

/// Parse and validate one bridge message (`{t:"hello",doc}` or
/// `{t:"req",doc,id,method,params}`), the extension's `isWellFormedRequest`
/// rules plus the document id.
pub fn parse_page_message(message_json: &str) -> PageMessage {
    // A payload far past the params cap is refused before it is even parsed.
    if message_json.len() > MAX_REQUEST_BYTES + 64 * 1024 {
        return PageMessage::Invalid {
            doc: None,
            id: None,
            code: -32602,
            message: "Request too large",
        };
    }
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(message_json) else {
        return PageMessage::Ignored;
    };
    let doc = object
        .get("doc")
        .and_then(Value::as_str)
        .filter(|doc| !doc.is_empty() && doc.len() <= MAX_ID_CHARS)
        .map(str::to_owned);
    match object.get("t").and_then(Value::as_str) {
        Some("hello") => match doc {
            Some(doc) => PageMessage::Hello {
                doc,
                href: object
                    .get("href")
                    .and_then(Value::as_str)
                    .filter(|href| href.len() <= 8 * 1024)
                    .map(str::to_owned),
            },
            None => PageMessage::Ignored,
        },
        Some("req") => {
            let id = object
                .get("id")
                .and_then(Value::as_str)
                .filter(|id| !id.is_empty() && id.chars().count() <= MAX_ID_CHARS)
                .map(str::to_owned);
            let invalid = |code: i64, message: &'static str| PageMessage::Invalid {
                doc: doc.clone(),
                id: id.clone(),
                code,
                message,
            };
            let (Some(doc_id), Some(request_id)) = (doc.clone(), id.clone()) else {
                return invalid(-32600, "Invalid request");
            };
            let method = match object.get("method").and_then(Value::as_str) {
                Some(method)
                    if !method.is_empty() && method.chars().count() <= MAX_METHOD_CHARS =>
                {
                    method.to_owned()
                }
                _ => return invalid(-32600, "Invalid request"),
            };
            // The bridge marks params it could not serialise (cyclic, BigInt).
            if object.get("bad").and_then(Value::as_bool) == Some(true) {
                return invalid(-32602, "Invalid params");
            }
            let params = match object.get("params") {
                None | Some(Value::Null) => Value::Array(Vec::new()),
                Some(params @ Value::Array(_)) => params.clone(),
                // EIP-747 is the one method whose params are an OBJECT
                // (`{type, options}`); refusing it broke every "add this token
                // to your wallet" button (found on the device, spec 070).
                Some(params @ Value::Object(_)) if method == "wallet_watchAsset" => params.clone(),
                Some(_) => return invalid(-32602, "Expected params to be an array"),
            };
            if params.to_string().len() > MAX_REQUEST_BYTES {
                return invalid(-32602, "Request too large");
            }
            PageMessage::Request(PageRequest {
                doc: doc_id,
                id: request_id,
                method,
                params,
            })
        }
        _ => PageMessage::Ignored,
    }
}

// ---------------------------------------------------------------------------
// The native → page wire (what `__velaDeliver` receives)
// ---------------------------------------------------------------------------

/// A successful answer to request `id` of document `doc`.
pub fn result_json(doc: &str, id: &str, result: &Value) -> String {
    json!({ "doc": doc, "dir": "res", "id": id, "result": result }).to_string()
}

/// An EIP-1193 error answer.
pub fn error_json(doc: &str, id: &str, code: i64, message: &str) -> String {
    json!({ "doc": doc, "dir": "res", "id": id, "error": { "code": code, "message": message } })
        .to_string()
}

/// A JSON-RPC error body passed through from a node (a revert is an answer the
/// page asked for), keeping `data` when there is one.
pub fn error_body_json(doc: &str, id: &str, error: &Value) -> String {
    let code = error.get("code").and_then(Value::as_i64).unwrap_or(-32603);
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .filter(|m| !m.is_empty())
        .unwrap_or("The node refused this call");
    let mut body = json!({ "code": code, "message": message });
    if let Some(data) = error.get("data") {
        body["data"] = data.clone();
    }
    json!({ "doc": doc, "dir": "res", "id": id, "error": body }).to_string()
}

/// The error answer for a SIGNING refusal: the same envelope, plus the core's
/// own `kind` beside the code.
///
/// **Why `kind` is on the wire.** `SignErrorKind` is the vocabulary the core
/// refuses in, and a page — or the shell hosting it — has to be able to act on
/// WHICH refusal this was without matching on English. The web shell has sent
/// it since spec 081 (`sign-executor.ts`: `{code, kind, message}`) and reads it
/// back (`DappRequestHost`: a `self_call_blocked` sheet stays up to explain
/// itself instead of leaving on the grace timer). Spec 070 moved the in-app
/// browser's answer in here, and this restores that half of the contract for
/// it: the message is for a developer to read, `kind` is for code to branch on.
pub fn sign_error_json(
    doc: &str,
    id: &str,
    code: i64,
    kind: SignErrorKind,
    detail: Option<&str>,
) -> String {
    json!({
        "doc": doc,
        "dir": "res",
        "id": id,
        "error": {
            "code": code,
            "kind": sign_error_kind_name(kind),
            "message": sign_error_words(kind, detail),
        },
    })
    .to_string()
}

/// The snake_case name of a kind — what `#[serde(rename_all = "snake_case")]`
/// writes, stated once so the wire cannot drift from the enum.
pub fn sign_error_kind_name(kind: SignErrorKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        // Unreachable for a unit variant, and a refusal must still be
        // answerable if it ever were: an empty kind is a missing field, not a
        // lie about which refusal this was.
        .unwrap_or_default()
}

/// What a page is told when signing ended in a refusal the core named.
///
/// A `detail` is a FACT, not a sentence — the refused function, the capability
/// that is not supported, the token an unlimited approval was for. Putting it
/// on the wire ALONE is the defect spec 081 found on a device: a page asked for
/// `addOwnerWithThreshold` and was answered `addOwnerWithThreshold`, which
/// reads as a label rather than an answer. So the sentence says what happened
/// and the detail is named inside it, which is what the web shell does
/// (`signErrorMessage`).
///
/// `SubmitFailed` is the exception and keeps its detail verbatim: there the
/// detail IS the message — the node's or the shell's own words for why a
/// submission failed — and no sentence of ours improves on it.
pub fn sign_error_words(kind: SignErrorKind, detail: Option<&str>) -> String {
    let detail = detail.map(str::trim).filter(|d| !d.is_empty());
    match (kind, detail) {
        (SignErrorKind::SubmitFailed, Some(detail)) => detail.to_owned(),
        (kind, Some(detail)) => format!("{} ({detail})", sign_error_message(kind)),
        (kind, None) => sign_error_message(kind).to_owned(),
    }
}

/// An EIP-1193 event (`accountsChanged`, `chainChanged`, `disconnect`).
pub fn event_json(doc: &str, event: &str, data: Option<Value>) -> String {
    let mut body = json!({ "doc": doc, "dir": "evt", "event": event });
    if let Some(data) = data {
        body["data"] = data;
    }
    body.to_string()
}

/// The words a page reads when signing ends in an error the core named by
/// kind. They are for the dApp's developer — EIP-1193 messages are not UI.
pub fn sign_error_message(kind: SignErrorKind) -> &'static str {
    match kind {
        SignErrorKind::UserRejected => "User rejected the request",
        SignErrorKind::WalletSwitchedChains => "Cancelled: the wallet switched chains",
        SignErrorKind::UnsupportedChain => "Unsupported chain",
        SignErrorKind::UnauthorizedAccount => "Unauthorized account",
        SignErrorKind::InvalidParams => "Invalid params",
        SignErrorKind::UnsupportedCapability => "Unsupported capability",
        SignErrorKind::UnlimitedApproval => "Unlimited approvals are disabled",
        // Spec 081's self-call guard, brought in by the 075 merge. The NOTICE
        // carries the refused function in `detail` — that is what a page is
        // told, because it is the specific fact — and this is the sentence for
        // the case with no detail to give. Never 4001: the person did not
        // reject it, the wallet refused it.
        SignErrorKind::SelfCallBlocked => {
            "The wallet refused a call that would change who controls the account"
        }
        SignErrorKind::FundingCancelled => "Gas account funding cancelled",
        SignErrorKind::SubmitFailed => "The transaction could not be submitted",
        SignErrorKind::StaleFeeQuote => "The fee quote expired",
    }
}

// ---------------------------------------------------------------------------
// The address bar
// ---------------------------------------------------------------------------

/// Where searches go (spec 070 research R6): no account, no search history.
pub const SEARCH_URL_PREFIX: &str = "https://duckduckgo.com/?q=";

/// Address-bar text → the URL to load. `None` for blank input.
///
/// - `http(s)://…` loads as typed.
/// - A host (`app.uniswap.org`, `app.uniswap.org/swap`, `localhost:5173`,
///   `192.168.1.4:8137`) gets `https://` — `http://` only for loopback and
///   private-network hosts, which is where a person runs a local dApp.
/// - Anything else — a word, a sentence, another scheme (`javascript:`,
///   `file:`) — is searched, never loaded.
pub fn browser_input(text: &str) -> Option<String> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        if super::dapp_permissions::origin_of(text).is_some() {
            return Some(text.to_owned());
        }
        return Some(search_url(text));
    }
    if text.chars().any(char::is_whitespace) {
        return Some(search_url(text));
    }
    let host_port = text.split(['/', '?', '#']).next().unwrap_or("");
    let host = host_port
        .rsplit_once(':')
        .filter(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
        .map(|(host, _)| host)
        .unwrap_or(host_port);
    if host.is_empty() || host.contains(['@', ':']) {
        return Some(search_url(text));
    }
    let looks_like_host = host.eq_ignore_ascii_case("localhost")
        || is_ipv4(host)
        || (host.contains('.')
            && host.split('.').all(|label| {
                !label.is_empty()
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            })
            && host
                .rsplit('.')
                .next()
                .is_some_and(|tld| tld.len() >= 2 && tld.bytes().any(|b| b.is_ascii_alphabetic())));
    if !looks_like_host {
        return Some(search_url(text));
    }
    let local = format!("http://{host_port}");
    if !super::dapp_permissions::is_insecure_public_origin(&local) {
        return Some(format!("http://{text}"));
    }
    Some(format!("https://{text}"))
}

fn is_ipv4(host: &str) -> bool {
    let parts: Vec<&str> = host.split('.').collect();
    parts.len() == 4
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.len() <= 3
                && part.bytes().all(|b| b.is_ascii_digit())
                && part.parse::<u16>().is_ok_and(|n| n <= 255)
        })
}

fn search_url(query: &str) -> String {
    let mut out = String::from(SEARCH_URL_PREFIX);
    for byte in query.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The injected script
// ---------------------------------------------------------------------------

/// Which in-app browser the script is for. The ONLY difference is how the
/// bridge hands a string to native code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DappProviderHost"))]
pub enum ProviderHost {
    /// `WebViewCompat.addWebMessageListener(…, "VelaHost", …)`.
    Android,
    /// `WKUserContentController.add(_, contentWorld: .page, name: "VelaHost")`.
    Ios,
    /// wry's IPC handler.
    Desktop,
}

impl ProviderHost {
    fn post_expression(self) -> &'static str {
        match self {
            Self::Android => "(s) => VelaHost.postMessage(s)",
            Self::Ios => "(s) => window.webkit.messageHandlers.VelaHost.postMessage(s)",
            Self::Desktop => "(s) => window.ipc.postMessage(s)",
        }
    }
}

/// The bridge: one document id, one hello, requests relayed, answers accepted
/// only when addressed to THIS document.
const BRIDGE_JS: &str = r#"
	if (window.__velaBridge) return;
	const CHANNEL = 'vela-1193';
	const DOC =
		(window.crypto && window.crypto.randomUUID && window.crypto.randomUUID()) ||
		Date.now().toString(16) + Math.random().toString(16).slice(2);
	const hostPost = __HOST_POST__;
	const send = (message) => {
		try {
			hostPost(JSON.stringify(message));
		} catch (error) {
			console.error('[Vela] bridge could not reach the wallet', error);
		}
	};
	Object.defineProperty(window, '__velaBridge', { value: DOC });
	send({ t: 'hello', doc: DOC, href: location.href });
	// A page restored from the back-forward cache is an OLD document coming
	// back without re-running this script: it says hello again so the wallet
	// makes it current and answers it.
	window.addEventListener('pageshow', (ev) => {
		if (ev.persisted) send({ t: 'hello', doc: DOC, href: location.href });
	});
	window.addEventListener('message', (ev) => {
		if (ev.source !== window) return;
		const d = ev.data;
		if (!d || d.ch !== CHANNEL || d.dir !== 'req') return;
		let encoded = null;
		try {
			encoded = JSON.stringify({ t: 'req', doc: DOC, id: d.id, method: d.method, params: d.params });
		} catch {
			encoded = JSON.stringify({ t: 'req', doc: DOC, id: d.id, method: d.method, bad: true });
		}
		try {
			hostPost(encoded);
		} catch (error) {
			console.error('[Vela] bridge could not reach the wallet', error);
		}
	});
	Object.defineProperty(window, '__velaDeliver', {
		value: (json) => {
			let m = null;
			try {
				m = JSON.parse(json);
			} catch {
				return;
			}
			if (!m || m.doc !== DOC) return;
			delete m.doc;
			window.postMessage(Object.assign({ ch: CHANNEL }, m), window.location.origin);
		}
	});
"#;

/// The whole document-start script for `host`: the bridge, then the
/// provider, both skipped in any frame but the top one — an iframe gets no
/// provider rather than one that can never be answered.
pub fn provider_script(host: ProviderHost) -> String {
    format!(
        "(function () {{\n\tif (window.top !== window) return;\n{bridge}\n{provider}\n}})();\n",
        bridge = BRIDGE_JS.replace("__HOST_POST__", host.post_expression()),
        provider = PROVIDER_JS,
    )
}

#[cfg(test)]
mod tests {

    /// Spec 081, device-found: a refusal must say what happened AND carry the
    /// kind. The page used to be answered with the refused function's name on
    /// its own, which reads as a label.
    #[test]
    fn a_refused_request_is_answered_with_a_sentence_and_a_kind() {
        let answer = sign_error_json(
            "doc-1",
            "rid-1",
            -32603,
            SignErrorKind::SelfCallBlocked,
            Some("addOwnerWithThreshold"),
        );
        let answer: Value = serde_json::from_str(&answer).unwrap_or_default();
        let error = &answer["error"];
        assert_eq!(error["code"], -32603);
        assert_eq!(error["kind"], "self_call_blocked");
        let message = error["message"].as_str().unwrap_or_default();
        // The sentence, with the refused function named inside it — not instead
        // of it.
        assert!(
            message.starts_with("The wallet refused a call that would change who controls"),
            "{message}"
        );
        assert!(message.contains("addOwnerWithThreshold"), "{message}");
        // The envelope is the same one every other answer rides.
        assert_eq!(answer["doc"], "doc-1");
        assert_eq!(answer["dir"], "res");
        assert_eq!(answer["id"], "rid-1");
    }

    /// The detail is folded in where it is a fact, and kept verbatim where it
    /// IS the message.
    #[test]
    fn a_refusal_with_nothing_to_add_is_just_the_sentence() {
        assert_eq!(
            sign_error_words(SignErrorKind::UserRejected, None),
            "User rejected the request"
        );
        // Empty is not a detail.
        assert_eq!(
            sign_error_words(SignErrorKind::UserRejected, Some("  ")),
            "User rejected the request"
        );
        // A submission failure's detail is the node's own words; ours would
        // only get in the way.
        assert_eq!(
            sign_error_words(SignErrorKind::SubmitFailed, Some("AA21 didn't pay prefund")),
            "AA21 didn't pay prefund"
        );
        assert_eq!(
            sign_error_words(
                SignErrorKind::UnsupportedCapability,
                Some("paymasterService")
            ),
            "Unsupported capability (paymasterService)"
        );
    }

    /// Every kind's wire name is the serde one, so a kind added next year
    /// cannot reach a page under a name nothing branches on.
    #[test]
    fn a_kinds_wire_name_is_the_one_serde_writes() {
        for kind in [
            SignErrorKind::UserRejected,
            SignErrorKind::WalletSwitchedChains,
            SignErrorKind::UnsupportedChain,
            SignErrorKind::UnauthorizedAccount,
            SignErrorKind::InvalidParams,
            SignErrorKind::UnsupportedCapability,
            SignErrorKind::UnlimitedApproval,
            SignErrorKind::SelfCallBlocked,
            SignErrorKind::FundingCancelled,
            SignErrorKind::SubmitFailed,
            SignErrorKind::StaleFeeQuote,
        ] {
            let name = sign_error_kind_name(kind);
            assert_eq!(
                serde_json::to_string(&kind).unwrap_or_default(),
                format!("\"{name}\""),
                "{kind:?}"
            );
            assert!(!name.is_empty(), "{kind:?}");
            assert!(!name.contains(char::is_uppercase), "{kind:?} → {name}");
        }
    }
    use super::*;

    #[test]
    fn eth_sign_and_unknowns_are_unsupported() {
        assert_eq!(classify("eth_sign"), Route::Unsupported);
        assert_eq!(classify("eth_signTransaction"), Route::Unsupported);
        assert_eq!(classify("debug_traceCall"), Route::Unsupported);
    }

    #[test]
    fn typed_data_versions_are_signing() {
        for method in [
            "eth_signTypedData",
            "eth_signTypedData_v1",
            "eth_signTypedData_v3",
            "eth_signTypedData_v4",
        ] {
            assert_eq!(classify(method), Route::Sign, "{method}");
        }
    }

    #[test]
    fn chain_params() {
        assert_eq!(chain_param(&json!([{ "chainId": "0x64" }])), Some(100));
        assert_eq!(chain_param(&json!([{ "chainId": "100" }])), Some(100));
        assert_eq!(chain_param(&json!([{ "chainId": 8453 }])), Some(8453));
        assert_eq!(chain_param(&json!([{ "chainId": "0x" }])), None);
        assert_eq!(chain_param(&json!([{ "chainId": "0x0" }])), None);
        assert_eq!(chain_param(&json!([{ "chainId": "abc" }])), None);
        assert_eq!(chain_param(&json!([{ "chainId": "0x100000000" }])), None);
        assert_eq!(chain_param(&json!([])), None);
        assert_eq!(chain_param(&json!({})), None);
    }

    #[test]
    fn address_bar() {
        assert_eq!(
            browser_input("app.uniswap.org").as_deref(),
            Some("https://app.uniswap.org")
        );
        assert_eq!(
            browser_input("  https://app.aave.com/markets  ").as_deref(),
            Some("https://app.aave.com/markets")
        );
        assert_eq!(
            browser_input("127.0.0.1:8137/").as_deref(),
            Some("http://127.0.0.1:8137/")
        );
        assert_eq!(
            browser_input("localhost:5173").as_deref(),
            Some("http://localhost:5173")
        );
        assert_eq!(
            browser_input("uniswap").as_deref(),
            Some("https://duckduckgo.com/?q=uniswap")
        );
        assert_eq!(
            browser_input("aave v3 markets").as_deref(),
            Some("https://duckduckgo.com/?q=aave+v3+markets")
        );
        assert_eq!(
            browser_input("javascript:alert(1)").as_deref(),
            Some("https://duckduckgo.com/?q=javascript%3Aalert%281%29")
        );
        assert_eq!(browser_input("   "), None);
        assert_eq!(browser_input("8.8.8.8").as_deref(), Some("https://8.8.8.8"));
        assert_eq!(
            browser_input("什么是以太坊").as_deref(),
            Some(
                "https://duckduckgo.com/?q=%E4%BB%80%E4%B9%88%E6%98%AF%E4%BB%A5%E5%A4%AA%E5%9D%8A"
            )
        );
    }

    #[test]
    fn page_messages() {
        assert_eq!(
            parse_page_message(r#"{"t":"hello","doc":"d1"}"#),
            PageMessage::Hello {
                doc: "d1".to_owned(),
                href: None
            }
        );
        assert_eq!(
            parse_page_message(r#"{"t":"req","doc":"d1","id":"x:1","method":"eth_chainId"}"#),
            PageMessage::Request(PageRequest {
                doc: "d1".to_owned(),
                id: "x:1".to_owned(),
                method: "eth_chainId".to_owned(),
                params: json!([]),
            })
        );
        assert!(matches!(
            parse_page_message(
                r#"{"t":"req","doc":"d1","id":"x:1","method":"eth_call","params":{"a":1}}"#
            ),
            PageMessage::Invalid {
                code: -32602,
                id: Some(_),
                ..
            }
        ));
        assert!(matches!(
            parse_page_message(r#"{"t":"req","doc":"d1","method":"eth_call"}"#),
            PageMessage::Invalid { id: None, .. }
        ));
        assert!(matches!(
            parse_page_message(
                r#"{"t":"req","doc":"d1","id":"x:1","method":"eth_call","bad":true}"#
            ),
            PageMessage::Invalid { code: -32602, .. }
        ));
        assert_eq!(parse_page_message("not json"), PageMessage::Ignored);
        assert_eq!(parse_page_message(r#"{"id":"x"}"#), PageMessage::Ignored);
        let big = format!(
            r#"{{"t":"req","doc":"d1","id":"x:1","method":"eth_call","params":["{}"]}}"#,
            "a".repeat(MAX_REQUEST_BYTES + 1)
        );
        assert!(matches!(
            parse_page_message(&big),
            PageMessage::Invalid {
                code: -32602,
                id: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn watch_asset_takes_the_object_eip_747_specifies() {
        assert!(matches!(
            parse_page_message(
                r#"{"t":"req","doc":"d1","id":"x:2","method":"wallet_watchAsset","params":{"type":"ERC20"}}"#
            ),
            PageMessage::Request(_)
        ));
    }

    #[test]
    fn script_is_one_classic_script_per_host() {
        for host in [
            ProviderHost::Android,
            ProviderHost::Ios,
            ProviderHost::Desktop,
        ] {
            let script = provider_script(host);
            assert!(script.starts_with("(function () {\n\tif (window.top !== window) return;"));
            assert!(!script.contains("__HOST_POST__"));
            assert!(!script
                .lines()
                .any(|line| line.trim_start().starts_with("import ")
                    || line.trim_start().starts_with("export ")));
            assert!(script.contains("eip6963:announceProvider"));
        }
        assert!(provider_script(ProviderHost::Android).contains("VelaHost.postMessage(s)"));
        assert!(provider_script(ProviderHost::Ios).contains("messageHandlers.VelaHost"));
        assert!(provider_script(ProviderHost::Desktop).contains("window.ipc.postMessage"));
    }
}
