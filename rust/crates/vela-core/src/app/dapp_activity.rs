//! What a dApp interaction was, as Activity states it (spec 093).
//!
//! A [`DappSummary`] is built ONCE, by the signing machine at approve time
//! ([`super::sign_request`]), from the FULL final params — before any shell
//! clips the stored request — with the same pure readers the sheet used:
//! [`super::approval_guard::detect_approval`] for allowances and permits,
//! [`super::clear_signing::analyze_message`] for Sign-In with Ethereum, and
//! [`crate::typed_data_request::canonical`] for typed data. It carries no
//! message body and no typed-data document, only the few facts a row and its
//! detail state; the request itself is kept beside it, clipped
//! ([`stored_request`]). Shells store the summary verbatim with the record and
//! hand it back to the feed untouched.
//!
//! Where a row says it happened — its "place" — is a protocol the wallet
//! itself knows by contract address ([`protocol_of`]), else the host of the
//! origin the request arrived from. Never the name a dApp gave itself: that is
//! the dApp's own claim.

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::activity_feed::FeedDappContent;
use super::approval_guard::{
    detect_approval, detect_calldata_approval, GuardApprovalKind, GuardDetectedApproval,
    GuardTokenMetaView,
};
use super::clear_signing::{
    analyze_message, known_contract, known_token_decimals, known_token_symbol, readable_message,
    ClearSignMethod, ClearSiweBinding,
};

/// The most of a request a record keeps on the disk (spec 093): 8 KB of the
/// final params' JSON, measured in UTF-8 bytes. A page chooses a request's
/// length; the wallet chooses how much of it lives on the disk. Every shell
/// stores [`super::sign_request::SignRecord::stored_request`], which is cut to
/// this here, once.
pub const STORED_REQUEST_MAX_BYTES: usize = 8 * 1024;

/// The longest EIP-712 primary type a summary keeps — a name, not a document.
pub const PRIMARY_TYPE_MAX_CHARS: usize = 32;

/// 9999-12-31T23:59:59Z. A deadline past it is no date anybody can read: the
/// "never" sentinels (`2^256-1`, `2^48-1`) land here, and read as no expiry.
const LAST_READABLE_SECS: u128 = 253_402_300_799;

/// What a dApp request did, in the words Activity titles it with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DappAction {
    /// A transaction calling one contract (or paying somebody).
    #[default]
    Call,
    /// An EIP-5792 batch of two or more calls.
    Batch,
    /// A transaction granting (or revoking) a spender's allowance.
    Approve,
    /// A signature granting a spender an allowance — ERC-2612, DAI, Permit2.
    Permit,
    /// A Sign-In with Ethereum message whose domain is the site that asked.
    SignIn,
    /// Any other `personal_sign` message (a SIWE whose domain is not the
    /// site's is one of these: it is no sign-in to that site).
    Message,
    /// EIP-712 data that grants nothing.
    TypedData,
    /// `eth_sign` — an opaque hash.
    BlindSign,
}

impl DappAction {
    /// Off-chain: nothing was sent, nothing settles, no figure moved.
    #[must_use]
    pub fn is_signature(self) -> bool {
        matches!(
            self,
            Self::Permit | Self::SignIn | Self::Message | Self::TypedData | Self::BlindSign
        )
    }

    /// The request is about an allowance, which the row states.
    #[must_use]
    pub fn grants(self) -> bool {
        matches!(self, Self::Approve | Self::Permit)
    }
}

/// The facts of one dApp request a record keeps (spec 093). Addresses are
/// lower-case; amounts are raw base units as decimal strings.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DappSummary {
    pub action: DappAction,
    /// The calls the transaction makes: 1 for a lone one, N for a batch, 0
    /// for a signature.
    #[serde(default)]
    pub calls: u32,
    /// The contract the request is about: a call's target; a batch's first
    /// call to a protocol the wallet knows; the token an on-chain approval is
    /// called on; typed data's verifying contract.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contract: Option<String>,
    /// Who an allowance lets spend.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spender: Option<String>,
    /// The token an allowance is for. `None` for several tokens at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// The token's symbol, as the sheet resolved it (or the built-in table).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u32>,
    /// The allowance in base units. `None` when it is unlimited, a
    /// grant-all, or spread over several tokens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    /// The allowance has no limit (or grants every token of a collection).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub unlimited: bool,
    /// The allowance is taken away, not granted (an approve of zero, a
    /// grant-all set to false).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub revoke: bool,
    /// When the allowance (or the permit) ends, in epoch seconds. `None` when
    /// it states no readable date — it does not end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<f64>,
    /// The sign-in's domain — only when it is the site that asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signin_domain: Option<String>,
    /// Typed data's primary type, cleaned (letters, digits, `_`; at most
    /// [`PRIMARY_TYPE_MAX_CHARS`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primary_type: Option<String>,
}

/// Summarize one request (spec 093) from its FULL final params. `origin` is
/// the origin the request arrived from — the site the row will name, which a
/// sign-in must match to be called one. `token_meta` is the approval
/// surface's resolved token (`GuardView::meta`), used only once it resolved.
#[must_use]
pub fn summarize(
    method: &str,
    params: &Value,
    origin: &str,
    token_meta: Option<&GuardTokenMetaView>,
) -> DappSummary {
    match method {
        "eth_sendTransaction" => call_summary(params.get(0).unwrap_or(&Value::Null), token_meta),
        "wallet_sendCalls" => {
            let calls = params
                .get(0)
                .and_then(|first| first.get("calls"))
                .and_then(Value::as_array)
                .map(Vec::as_slice)
                .unwrap_or_default();
            match calls {
                [] => DappSummary {
                    action: DappAction::Batch,
                    ..DappSummary::default()
                },
                // One call is that call, as its sheet reads it (089 S1).
                [only] => call_summary(only, token_meta),
                calls => batch_summary(calls),
            }
        }
        "personal_sign" => message_summary(params, origin),
        "eth_sign" => DappSummary {
            action: DappAction::BlindSign,
            ..DappSummary::default()
        },
        method if crate::typed_data_request::looks_like_typed_data(method) => {
            typed_summary(method, params, token_meta)
        }
        // Anything else that was signed is recorded as a message
        // (`record_shape`), and says no more than that.
        _ => DappSummary {
            action: DappAction::Message,
            ..DappSummary::default()
        },
    }
}

/// One transaction: an allowance it grants, or the call it makes.
fn call_summary(tx: &Value, token_meta: Option<&GuardTokenMetaView>) -> DappSummary {
    let to = address_in(tx.get("to"));
    let data = tx.get("data").and_then(Value::as_str);
    let base = DappSummary {
        action: DappAction::Call,
        calls: 1,
        contract: to.clone(),
        ..DappSummary::default()
    };
    match detect_calldata_approval(to.as_deref(), data).filter(is_grant) {
        Some(approval) => with_grant(
            DappSummary {
                action: DappAction::Approve,
                ..base
            },
            &approval,
            token_meta,
        ),
        None => base,
    }
}

/// A batch: where it happens (the first call to a protocol the wallet knows)
/// and the first allowance any call grants — the detail states it, though
/// the title is the batch's verb.
fn batch_summary(calls: &[Value]) -> DappSummary {
    let mut summary = DappSummary {
        action: DappAction::Batch,
        calls: u32::try_from(calls.len()).unwrap_or(u32::MAX),
        ..DappSummary::default()
    };
    for call in calls {
        let to = address_in(call.get("to"));
        if summary.contract.is_none() {
            summary.contract = to.clone().filter(|to| protocol_of(to).is_some());
        }
        if summary.spender.is_none() {
            let data = call.get("data").and_then(Value::as_str);
            if let Some(approval) = detect_calldata_approval(to.as_deref(), data).filter(is_grant) {
                // A leg's own token metadata never reaches the approve, so
                // only the built-in table names its token here.
                summary = with_grant(summary, &approval, None);
            }
        }
    }
    summary
}

/// `personal_sign`: a sign-in when it is a Sign-In with Ethereum message for
/// the very site that asked — read by the sheet's own reader, so the row and
/// the sheet's verified badge agree — else a message.
fn message_summary(params: &Value, origin: &str) -> DappSummary {
    let payload = params.get(0).and_then(Value::as_str).unwrap_or_default();
    let origin = Some(origin.trim()).filter(|origin| !origin.is_empty());
    let view = analyze_message(ClearSignMethod::PersonalSign, payload, origin);
    match (view.siwe, view.binding) {
        (Some(siwe), Some(ClearSiweBinding::Ok)) => DappSummary {
            action: DappAction::SignIn,
            signin_domain: siwe.domain_host.or(Some(siwe.domain)),
            ..DappSummary::default()
        },
        _ => DappSummary {
            action: DappAction::Message,
            ..DappSummary::default()
        },
    }
}

/// Typed data: a permit when it grants an allowance (the guard's reading),
/// else plain structured data — its primary type and verifying contract.
fn typed_summary(
    method: &str,
    params: &Value,
    token_meta: Option<&GuardTokenMetaView>,
) -> DappSummary {
    let document = crate::typed_data_request::canonical(method, params)
        .map(|read| read.document)
        .unwrap_or(Value::Null);
    let domain = document.get("domain").unwrap_or(&Value::Null);
    let base = DappSummary {
        action: DappAction::TypedData,
        contract: address_in(domain.get("verifyingContract")),
        primary_type: document
            .get("primaryType")
            .and_then(Value::as_str)
            .and_then(clean_primary_type),
        ..DappSummary::default()
    };
    match detect_approval(method, Some(params)).filter(is_grant) {
        Some(approval) => with_grant(
            DappSummary {
                action: DappAction::Permit,
                ..base
            },
            &approval,
            token_meta,
        ),
        None => base,
    }
}

/// An approval that is about an allowance: a decrease only lowers one, which
/// is no grant to state.
fn is_grant(approval: &GuardDetectedApproval) -> bool {
    approval.kind != GuardApprovalKind::DecreaseAllowance
}

/// The allowance's facts onto `summary`.
fn with_grant(
    mut summary: DappSummary,
    approval: &GuardDetectedApproval,
    token_meta: Option<&GuardTokenMetaView>,
) -> DappSummary {
    summary.spender = address_of(&approval.spender);
    summary.token = approval.token_address.as_deref().and_then(address_of);
    summary.unlimited = approval.is_unbounded;
    summary.revoke = approval.is_reducing && !approval.is_unbounded;
    summary.amount = approval
        .amount_raw
        .clone()
        .filter(|_| !approval.is_unbounded && !approval.is_boolean_grant);
    // The sheet's resolved metadata, once it resolved; the built-in table
    // otherwise. A token nobody could name keeps its address only.
    let resolved = token_meta.filter(|meta| meta.verified && !meta.loading);
    let token = summary.token.as_deref();
    summary.symbol = resolved
        .map(|meta| meta.symbol.trim().to_owned())
        .filter(|symbol| !symbol.is_empty())
        .or_else(|| token.and_then(known_token_symbol).map(str::to_owned));
    summary.decimals = resolved
        .map(|meta| meta.decimals)
        .or_else(|| token.and_then(known_token_decimals));
    summary.expires_at = approval.deadline.as_deref().and_then(readable_deadline);
    summary
}

/// A deadline as a date: a whole number of seconds, after the epoch and
/// before the year 10000. Zero and the "never" sentinels are no date.
fn readable_deadline(deadline: &str) -> Option<f64> {
    let secs: u128 = deadline.trim().parse().ok()?;
    #[allow(clippy::cast_precision_loss, reason = "bounded below 2^38")]
    (secs > 0 && secs <= LAST_READABLE_SECS).then_some(secs as f64)
}

/// An address-shaped JSON string, lower-cased.
fn address_in(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).and_then(address_of)
}

/// `0x` and forty hex digits, lower-cased; `None` for anything else.
fn address_of(text: &str) -> Option<String> {
    let text = text.trim();
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))?;
    (hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| format!("0x{}", hex.to_ascii_lowercase()))
}

/// A primary type as a name: its letters, digits and `_`, at most
/// [`PRIMARY_TYPE_MAX_CHARS`]. `None` when nothing is left.
#[must_use]
pub fn clean_primary_type(name: &str) -> Option<String> {
    let cleaned: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .take(PRIMARY_TYPE_MAX_CHARS)
        .collect();
    (!cleaned.is_empty()).then_some(cleaned)
}

/// The protocol a contract belongs to, when the wallet itself knows it
/// (`clear_signing`'s built-in table): "Uniswap", "PancakeSwap"…
///
/// Not every known contract is a place somebody acts ON: Permit2 serves every
/// protocol (an approval of it on PancakeSwap is no Uniswap action), and a
/// wrapped coin or a smart-wallet factory is not where anything happens. Those
/// name no place, and the row names the site.
#[must_use]
pub fn protocol_of(address: &str) -> Option<&'static str> {
    let (name, owner) = known_contract(address)?;
    (name != "Permit2" && owner != "WETH" && owner != "Coinbase").then_some(owner)
}

/// The built-in name of a known contract ("Uniswap Universal Router").
#[must_use]
pub fn contract_name_of(address: &str) -> Option<&'static str> {
    known_contract(address).map(|(name, _)| name)
}

/// The request as a record keeps it (spec 093): the final params' JSON, at
/// most [`STORED_REQUEST_MAX_BYTES`], and whether it was cut.
///
/// Within the budget it is kept verbatim. Past it, the string values —
/// calldata, a typed document, a message — are clipped, ever shorter, on a
/// character boundary, until the whole fits; the result is still the same
/// JSON shape, so a replay still reads its selector and its early words. A
/// request whose shape alone is past the budget keeps nothing (`""`). Text
/// that is not JSON (never signed: the machine refuses it) is cut on a
/// character boundary.
#[must_use]
pub fn stored_request(params_json: &str) -> (String, bool) {
    if params_json.len() <= STORED_REQUEST_MAX_BYTES {
        return (params_json.to_owned(), false);
    }
    let Ok(params) = serde_json::from_str::<Value>(params_json) else {
        return (
            cut_on_char(params_json, STORED_REQUEST_MAX_BYTES).to_owned(),
            true,
        );
    };
    for cap in [STORED_REQUEST_MAX_BYTES - 2_048, 2_048, 512, 0] {
        let text = clip_strings(&params, cap).to_string();
        if text.len() <= STORED_REQUEST_MAX_BYTES {
            return (text, true);
        }
    }
    // The structure alone is past the budget (thousands of fields): nothing
    // is kept, and the detail says the content was not recorded.
    (String::new(), true)
}

/// A record's stored request as Technical details shows it (spec 093), by
/// what it holds ([`FeedDappContent`], the detail's `content` line): typed
/// data as its document, pretty-printed; a message as its text (hex decoded
/// as UTF-8 when it is text, else the hex itself); call data as the params,
/// pretty-printed. `stored_request` is the params' JSON text exactly as the
/// record kept it ([`stored_request`]). `None` when the record kept nothing.
///
/// Read when the section opens, never with the feed: the text can be 8 KB.
#[must_use]
pub fn request_display(content: FeedDappContent, stored_request: &str) -> Option<String> {
    let text = stored_request.trim();
    if text.is_empty() {
        return None;
    }
    let Ok(params) = serde_json::from_str::<Value>(text) else {
        // Not JSON (cut by an older build): shown as kept.
        return Some(text.to_owned());
    };
    let list = params.as_array().map(Vec::as_slice).unwrap_or_default();
    let shown = match content {
        FeedDappContent::Message => message_payload(list).map(readable_message),
        FeedDappContent::TypedData => typed_document(list),
        FeedDappContent::CallData => None,
    };
    Some(shown.unwrap_or_else(|| pretty(&params)))
}

/// The signed payload of a message request: `personal_sign` carries it
/// first, `eth_sign` second (after the account) — the one whose first
/// param is an address and whose second is not.
fn message_payload(params: &[Value]) -> Option<&str> {
    let first = params.first()?.as_str()?;
    match params.get(1).and_then(Value::as_str) {
        Some(second) if address_of(first).is_some() && address_of(second).is_none() => Some(second),
        _ => Some(first),
    }
}

/// The one typed-data document among the params (an object, or a string
/// holding one), pretty-printed; a document that no longer parses (clipped)
/// as the text it is.
fn typed_document(params: &[Value]) -> Option<String> {
    params.iter().find_map(|param| match param {
        Value::Object(_) => Some(pretty(param)),
        Value::String(text) if address_of(text).is_none() => Some(
            serde_json::from_str::<Value>(text)
                .ok()
                .filter(Value::is_object)
                .map_or_else(|| text.clone(), |document| pretty(&document)),
        ),
        _ => None,
    })
}

fn pretty(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
}

/// Every string in `value` cut to at most `cap` bytes.
fn clip_strings(value: &Value, cap: usize) -> Value {
    match value {
        Value::String(text) => Value::String(cut_on_char(text, cap).to_owned()),
        Value::Array(items) => Value::Array(items.iter().map(|v| clip_strings(v, cap)).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .iter()
                .map(|(key, v)| (key.clone(), clip_strings(v, cap)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The longest prefix of `text` of at most `cap` bytes that ends on a
/// character boundary.
fn cut_on_char(text: &str, cap: usize) -> &str {
    if text.len() <= cap {
        return text;
    }
    let mut end = cap;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_short_request_is_kept_verbatim() {
        let text = r#"[{"to":"0xabc","data":"0x12"}]"#;
        assert_eq!(stored_request(text), (text.to_owned(), false));
    }

    #[test]
    fn a_long_request_keeps_its_shape_within_the_budget() {
        let data = format!("0x{}", "ab".repeat(10_000));
        let text = json!([{ "to": "0x1111111111111111111111111111111111111111", "data": data }])
            .to_string();
        let (kept, cut) = stored_request(&text);
        assert!(cut);
        assert!(kept.len() <= STORED_REQUEST_MAX_BYTES);
        let read: Value =
            serde_json::from_str(&kept).unwrap_or_else(|e| unreachable!("still JSON: {e}"));
        let data = read[0]["data"].as_str().unwrap_or_default();
        assert!(data.starts_with("0xabab"), "the selector survives");
    }

    #[test]
    fn a_cut_never_splits_a_character() {
        let message = "汉".repeat(5_000);
        let (kept, cut) = stored_request(&json!([message]).to_string());
        assert!(cut);
        assert!(kept.len() <= STORED_REQUEST_MAX_BYTES);
        let read: Value =
            serde_json::from_str(&kept).unwrap_or_else(|e| unreachable!("still JSON: {e}"));
        assert!(read[0]
            .as_str()
            .is_some_and(|s| s.chars().all(|c| c == '汉')));
        let raw = "é".repeat(STORED_REQUEST_MAX_BYTES);
        let (kept, _) = stored_request(&raw);
        assert!(kept.len() <= STORED_REQUEST_MAX_BYTES && kept.chars().all(|c| c == 'é'));
    }

    #[test]
    fn a_shape_past_the_budget_keeps_nothing() {
        let many: Vec<Value> = (0..3_000).map(|i| json!({ "n": i })).collect();
        assert_eq!(
            stored_request(&json!([many]).to_string()),
            (String::new(), true)
        );
    }

    #[test]
    fn a_message_reads_as_its_text_or_its_hex() {
        use super::FeedDappContent::{CallData, Message, TypedData};
        let hello = json!([
            "0x48656c6c6f2c20e4b896e7958c",
            "0x1111111111111111111111111111111111111111"
        ]);
        assert_eq!(
            request_display(Message, &hello.to_string()).as_deref(),
            Some("Hello, 世界")
        );
        let hash = format!("0x{}", "ab".repeat(32));
        let eth_sign = json!(["0x1111111111111111111111111111111111111111", hash]);
        assert_eq!(
            request_display(Message, &eth_sign.to_string()),
            Some(hash.clone()),
            "eth_sign signs its second param; bytes that are not text stay hex"
        );
        let typed = json!({"primaryType": "Mail", "types": {}, "domain": {}, "message": {"a": 1}});
        let shown = request_display(
            TypedData,
            &json!([
                "0x1111111111111111111111111111111111111111",
                typed.to_string()
            ])
            .to_string(),
        )
        .unwrap_or_default();
        assert!(
            shown.contains("\n  \"message\": {"),
            "pretty, not escaped: {shown}"
        );
        assert!(!shown.contains("\\\""));
        let clipped = request_display(
            TypedData,
            r#"["0x1111111111111111111111111111111111111111","{\"types\":{"]"#,
        );
        assert_eq!(clipped.as_deref(), Some(r#"{"types":{"#));
        let call = request_display(CallData, r#"[{"to":"0x1","data":"0x12"}]"#).unwrap_or_default();
        assert!(call.contains("\"data\": \"0x12\""));
        assert_eq!(request_display(CallData, "  "), None);
    }

    #[test]
    fn primary_types_are_names() {
        assert_eq!(
            clean_primary_type("PermitSingle"),
            Some("PermitSingle".into())
        );
        assert_eq!(
            clean_primary_type("<script>Order"),
            Some("scriptOrder".into())
        );
        assert_eq!(
            clean_primary_type(&"A".repeat(80)).map(|s| s.len()),
            Some(PRIMARY_TYPE_MAX_CHARS)
        );
        assert_eq!(clean_primary_type("  "), None);
    }

    #[test]
    fn infrastructure_names_no_place() {
        assert_eq!(
            protocol_of("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad"),
            Some("Uniswap")
        );
        assert_eq!(
            protocol_of("0x13F4EA83D0bd40E75C8222255bc855a974568Dd4"),
            Some("PancakeSwap")
        );
        assert_eq!(
            protocol_of("0x000000000022d473030f116ddee9f6b43ac78ba3"),
            None
        );
        assert_eq!(
            protocol_of("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"),
            None
        );
        assert_eq!(
            protocol_of("0x0000000000000000000000000000000000000001"),
            None
        );
    }

    #[test]
    fn deadlines_read_as_dates_or_not_at_all() {
        assert_eq!(readable_deadline("1767225600"), Some(1_767_225_600.0));
        assert_eq!(readable_deadline("0"), None);
        assert_eq!(readable_deadline("281474976710655"), None);
        assert_eq!(
            readable_deadline(
                "115792089237316195423570985008687907853269984665640564039457584007913129639935"
            ),
            None
        );
    }
}
