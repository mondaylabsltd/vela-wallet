//! What an agent reads of a dApp's signing request, and the opinion it may
//! give on it (spec 103: `vela_review`, `vela_watch_reviews`, `vela_advise`).
//!
//! The review says, line by line, who wrote what the sheet shows: Vela's own
//! decode of the bytes is CERTAIN; a descriptor (built in, or fetched from a
//! host with no signature over it), a 4-byte name or a shape match is an
//! interpretation; a site's name, a token's symbol its own contract answered,
//! a message's text are the requester's words. Every such word reaches the
//! agent wrapped as `{ "untrusted_text": …, "from": … }` — data, never an
//! instruction.
//!
//! The review carries a `review_digest`: a hash over exactly what will be
//! signed (the params after the approval guard's cap, the chain, the
//! account) and what the sheet shows of it. An opinion is bound to one
//! digest: the sheet draws it as current only while the review it was given
//! for is the one on screen ([`advice`]).
//!
//! The digest leaves the fee out — a fee re-prices every few seconds, and
//! that is not a new request. An opinion is bound to the fee too, but
//! separately and with room to move ([`advice_standing`]): it records the
//! fee on the sheet when it arrived, and stays current while the fee is in
//! the same coin and within [`ADVICE_FEE_BAND_PERCENT`] of that figure (or
//! a small fixed amount, for a tiny fee).
//!
//! Nothing here changes what is signed, a button, or a risk: the shells draw
//! the opinion in its own card, under Vela's own lines.

use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::approval_guard::{GuardChoice, GuardDetectedApproval, GuardEditorView, GuardView};
use super::clear_signing::{
    ClearBatchView, ClearBlindTyped, ClearLineSource, ClearMessageView, ClearSignField,
    ClearSignResult, ClearSigningView,
};
use super::intent_rules::{SignFacts, World};
use super::sim_outcome::SimVerdict;
use super::token_trust::{TrustSimJudgment, TrustSimView};
use crate::primitives::{sha256, to_hex};

/// The digest's domain: a new reading of the review is a new version.
const DIGEST_DOMAIN: &str = "vela.agent_review.v1";

/// What a review digest covers — said beside it, so an agent knows a fee
/// that moves is not a new version of the request (its opinion's own fee
/// check is [`advice_standing`]).
pub const DIGEST_COVERS: &[&str] = &[
    "chain_id",
    "account",
    "method",
    "origin",
    "signed_params",
    "reading",
    "batch",
    "message",
    "typed_data",
    "approval_cap",
    "simulation",
];

/// The longest summary an opinion keeps, in characters.
pub const ADVICE_SUMMARY_MAX: usize = 280;
/// The longest point an opinion keeps, in characters.
pub const ADVICE_POINT_MAX: usize = 160;
/// How many points, and how many sources, an opinion keeps.
pub const ADVICE_LIST_MAX: usize = 5;

/// The verdicts an agent may give.
pub const ADVICE_VERDICTS: &[&str] = &["looks_fine", "caution", "danger", "unsure"];

/// How far, in percent of the fee an opinion was given for, the fee may
/// move — up or down — before the opinion reads as given for another fee.
pub const ADVICE_FEE_BAND_PERCENT: u128 = 25;
/// The band's floor for a tiny fee, as a power of ten below one coin: a
/// stablecoin fee may always move by 0.01 of it (the relay's own minimum
/// for a stablecoin), a native one by 0.00001 of the coin (the wallet's
/// own minimum native fee) — a fee of 0.00002 xDAI that becomes 0.00003 is
/// no different request.
const FEE_FLOOR_STABLE_PLACES: u32 = 2;
const FEE_FLOOR_NATIVE_PLACES: u32 = 5;
/// Significant digits of the fee figure an opinion's card shows.
const FEE_SHOWN_DIGITS: usize = 4;

/// What the signing sheet holds beside the request, as the shell's views
/// (every member optional: a shell passes what it has, and a view that does
/// not read is no reading — never the whole review lost).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct SheetFacts {
    /// The clear-signing machine's view (`ClearSigningView`).
    pub clear: Option<Value>,
    /// The approval guard's view (`GuardView`): the person's chosen cap.
    pub guard: Option<Value>,
    /// The fee in force (`FeeView`).
    pub fee: Option<Value>,
    /// The sheet's simulation once it ended: `{ verdict, judged }`.
    pub simulation: Option<Value>,
    /// A simulation is out and has not ended.
    pub simulation_pending: bool,
    /// The base URL fetched descriptors come from (the shell's
    /// Ethereum-data endpoint): its host is said beside a fetched reading.
    pub descriptor_base: Option<String>,
}

/// Requester-written text, handed over as data.
#[must_use]
pub fn untrusted(text: &str, from: &str) -> Value {
    json!({ "untrusted_text": text, "from": from })
}

/// A character a reader cannot see that can change what text reads as —
/// the core's one classifier ([`super::invisible_chars`]), so the agent and
/// the sheet flag the same characters.
#[must_use]
pub fn is_invisible(c: char) -> bool {
    super::invisible_chars::is_invisible(c)
}

/// The text holds a character that hides or reorders what it says, by the
/// sheet's own rule ([`super::invisible_chars::has_invisible`]).
#[must_use]
pub fn has_invisible_chars(text: &str) -> bool {
    super::invisible_chars::has_invisible(text)
}

/// Any string in `value` — and in JSON text inside one (typed data v4 is a
/// JSON string) — has invisible characters.
fn any_invisible(value: &Value) -> bool {
    match value {
        Value::String(text) => {
            has_invisible_chars(text)
                || serde_json::from_str::<Value>(text)
                    .ok()
                    .filter(|inner| inner.is_object() || inner.is_array())
                    .is_some_and(|inner| any_invisible(&inner))
        }
        Value::Array(items) => items.iter().any(any_invisible),
        Value::Object(map) => map
            .iter()
            .any(|(key, item)| has_invisible_chars(key) || any_invisible(item)),
        _ => false,
    }
}

/// Plain text for a card: no control, bidirectional or zero-width
/// character, runs of white space as one space, at most `max` characters
/// (cut with "…").
#[must_use]
pub fn plain_text(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if c.is_control() || is_invisible(c) {
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    if out.chars().count() <= max {
        return out;
    }
    let kept: String = out.chars().take(max.saturating_sub(1)).collect();
    format!("{}…", kept.trim_end())
}

/// The host of an `http(s)` URL, lowercased, without user or port; `None`
/// for anything else.
#[must_use]
pub fn host_of(url: &str) -> Option<String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    let host = if host.starts_with('[') {
        host.split(']').next().map(|h| format!("{h}]"))?
    } else {
        host.split(':').next().unwrap_or("").to_owned()
    };
    let ok = !host.is_empty()
        && host.len() <= 253
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '[' | ']' | ':'));
    ok.then_some(host)
}

/// `raw / 10^decimals`, exact: digits only, trailing zeros trimmed. `None`
/// when `raw` is not a non-negative decimal integer.
#[must_use]
pub fn exact_units(raw: &str, decimals: u32) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = match raw.trim_start_matches('0') {
        "" => "0",
        rest => rest,
    };
    let places = usize::try_from(decimals.min(255)).unwrap_or(255);
    let (whole, frac) = if digits.len() > places {
        let (w, f) = digits.split_at(digits.len() - places);
        (w.to_owned(), f.to_owned())
    } else {
        ("0".to_owned(), format!("{digits:0>places$}"))
    };
    let frac = frac.trim_end_matches('0');
    Some(if frac.is_empty() {
        whole
    } else {
        format!("{whole}.{frac}")
    })
}

/// A value's wire spelling (`"built_in"`), for the enums this reads.
fn wire(value: impl serde::Serialize) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// Canonical JSON: object keys sorted, no white space.
fn canonical(value: &Value, out: &mut String) {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            out.push('{');
            for (n, key) in keys.into_iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).unwrap_or_default());
                out.push(':');
                canonical(&map[key], out);
            }
            out.push('}');
        }
        Value::Array(items) => {
            out.push('[');
            for (n, item) in items.iter().enumerate() {
                if n > 0 {
                    out.push(',');
                }
                canonical(item, out);
            }
            out.push(']');
        }
        scalar => out.push_str(&serde_json::to_string(scalar).unwrap_or_default()),
    }
}

/// SHA-256 over the canonical JSON of `value`, `0x`-hex.
#[must_use]
pub fn digest_of(value: &Value) -> String {
    let mut text = String::new();
    canonical(value, &mut text);
    to_hex(&sha256(text.as_bytes()), true)
}

// ---------------------------------------------------------------------------
// Where a line came from
// ---------------------------------------------------------------------------

/// Who a line's words are: `None` for Vela's own.
fn words_from(source: ClearLineSource) -> Option<&'static str> {
    match source {
        ClearLineSource::Vela | ClearLineSource::Shape => None,
        ClearLineSource::BuiltIn | ClearLineSource::Fetched => Some("descriptor"),
        ClearLineSource::SelectorDb => Some("selector_db"),
    }
}

/// `text` as Vela's own words, or wrapped as `from`'s.
fn said(text: &str, from: Option<&str>) -> Value {
    from.map_or_else(
        || Value::String(text.to_owned()),
        |from| untrusted(text, from),
    )
}

struct Origin<'a> {
    owner: Option<&'a str>,
    path: Option<&'a str>,
    host: Option<&'a str>,
}

/// Where a line came from, as an agent reads it.
fn source_of(source: ClearLineSource, origin: &Origin<'_>) -> Value {
    let owner = origin.owner.map(|owner| untrusted(owner, "descriptor"));
    match source {
        ClearLineSource::Vela => json!({ "kind": "certain", "by": "vela" }),
        ClearLineSource::BuiltIn => json!({ "kind": "builtin_descriptor", "owner": owner }),
        ClearLineSource::Fetched => json!({
            "kind": "fetched_descriptor",
            "owner": owner,
            "host": origin.host,
            "path": origin.path,
            "authenticated": false,
        }),
        ClearLineSource::SelectorDb => json!({ "kind": "4byte" }),
        ClearLineSource::Shape => json!({ "kind": "heuristic_shape" }),
    }
}

/// A line's value as shown: Vela's writing of the bytes, or text another
/// party chose (a token's own symbol, a descriptor's enum word, a string
/// the calldata carries).
fn value_of(field: &ClearSignField) -> Value {
    let from = match field.format.as_str() {
        "tokenAmount" | "token" if field.token_named_by_contract => Some("token_contract"),
        "tokenAmount" | "token" | "addressName" | "amount" | "date" | "duration" => None,
        "enum" | "unit" => Some("descriptor"),
        _ => Some("site"),
    };
    said(&field.value, from)
}

fn line(field: &ClearSignField, origin: &Origin<'_>) -> Value {
    json!({
        "label": said(&field.label, words_from(field.source)),
        "value": value_of(field),
        "format": field.format,
        "role": wire(field.role),
        "source": source_of(field.source, origin),
        "address": field.address,
        "this_account": field.this_account,
        "token": field.token_address.as_ref().map(|address| json!({
            "address": address,
            "named_by": if field.token_named_by_contract { "token_contract" } else { "registry" },
        })),
        "bound": wire(field.bound),
        "warning": field.warning,
        "unverified": field.unverified,
        "dust": field.dust,
        "expired": field.expired,
        "detail": field.detail,
    })
}

/// One decoded call: its intent, its contract, and every line, each with
/// its source.
fn reading_of(result: &ClearSignResult, host: Option<&str>) -> Value {
    let r = &result.reading;
    let origin = Origin {
        owner: r.descriptor_owner.as_deref(),
        path: r.descriptor_path.as_deref(),
        host: (r.source == ClearLineSource::Fetched)
            .then_some(host)
            .flatten(),
    };
    let named_from = r.contract_named_by.and_then(words_from);
    json!({
        "intent": said(&result.intent, words_from(r.source)),
        "source": source_of(r.source, &origin),
        "looks_like": r.shape_intent,
        "selector_name": r.selector_name.as_deref().map(|name| untrusted(name, "selector_db")),
        "contract": {
            "address": result.contract_address,
            "name": result.contract_name.as_deref().map(|name| said(name, named_from.or(
                // Named, but nobody says by whom: not Vela's word.
                r.contract_named_by.is_none().then_some("descriptor"),
            ))),
            "named_by": wire(r.contract_named_by),
            "owner": result.owner.as_deref().map(|owner| untrusted(owner, "descriptor")),
        },
        // An approve-all's collection, as its own contract's name() says it.
        "collection_name": r.collection_name.as_deref().map(|name| untrusted(name, "token_contract")),
        "lines": result.fields.iter().map(|f| line(f, &origin)).collect::<Vec<_>>(),
        "contradictions": r.contradictions.iter().map(|c| untrusted(c, "descriptor")).collect::<Vec<_>>(),
        "goes_elsewhere": r.goes_elsewhere,
        "dust_minimum": r.dust_minimum,
        "allowance": r.allowance,
        "allowance_adds": r.allowance_adds,
        "risk": wire(result.risk),
        "partial": result.partial,
        "best_effort": result.best_effort,
        "to_own_token": result.to_own_token,
        "terms_off_chain": result.terms_off_chain,
    })
}

/// A target's name a batch row shows: the registry's, or a token's own
/// answer with its short address beside it ("USDC (0xa0b8…)").
fn to_name(name: &str) -> Value {
    said(name, name.contains(" (0x").then_some("token_contract"))
}

fn batch_of(batch: &ClearBatchView, host: Option<&str>) -> Value {
    json!({
        "calls": batch.calls.iter().map(|call| json!({
            "index": call.index,
            "surface": wire(call.surface),
            "to": call.to,
            "to_name": call.to_name.as_deref().map(to_name),
            "no_code": call.no_code,
            "value_wei": call.value_wei,
            "amount": call.amount,
            "risk": wire(call.risk),
            "reading": call.result.as_ref().map(|r| reading_of(r, host)),
            "plain_send": call.plain_send.as_ref().map(|send| json!({
                "to": send.to, "value_wei": send.value_wei, "amount": send.amount,
            })),
        })).collect::<Vec<_>>(),
        "total_value_wei": batch.total_value_wei,
        "total_amount": batch.total_amount,
        "risk": wire(batch.risk),
    })
}

fn message_of(message: &ClearMessageView, params: &Value) -> Value {
    let siwe = message.siwe.as_ref().map(|s| {
        json!({
            "domain": untrusted(&s.domain, "message"),
            "domain_host": s.domain_host.as_deref().map(|h| untrusted(h, "message")),
            "address": s.address,
            "statement": s.statement.as_deref().map(|t| untrusted(t, "message")),
            "uri": s.uri.as_deref().map(|t| untrusted(t, "message")),
            "chain_id": s.chain_id,
            "nonce": s.nonce.as_deref().map(|t| untrusted(t, "message")),
        })
    });
    let invisible = message
        .decoded_text
        .as_deref()
        .map_or_else(|| any_invisible(params), has_invisible_chars);
    json!({
        "text": message.decoded_text.as_deref().map(|t| untrusted(t, "message")),
        "is_hex": message.is_hex,
        "binary_preview": message.binary_preview,
        "non_printable": message.non_printable,
        "has_invisible_chars": invisible,
        "siwe": siwe,
        "siwe_binding": wire(message.binding),
        "danger_class": wire(message.danger_class),
    })
}

fn typed_of(blind: Option<&ClearBlindTyped>, params: &Value) -> Value {
    json!({
        "primary_type": blind.and_then(|b| b.primary_type.as_deref()).map(|t| untrusted(t, "typed_data")),
        "domain_name": blind.and_then(|b| b.domain_name.as_deref()).map(|t| untrusted(t, "typed_data")),
        "verifying_contract": blind.and_then(|b| b.verifying_contract.as_deref()),
        "fields": blind.map(|b| b.fields.iter().map(|f| json!({
            "key": untrusted(&f.key, "typed_data"),
            "value": untrusted(&f.value, "typed_data"),
        })).collect::<Vec<_>>()),
        "has_invisible_chars": any_invisible(params),
    })
}

// ---------------------------------------------------------------------------
// The person's cap, the simulation, the fee
// ---------------------------------------------------------------------------

fn choice_of(choice: Option<&GuardChoice>) -> Value {
    match choice {
        None => Value::Null,
        Some(GuardChoice::Amount { amount_raw }) => {
            json!({ "type": "amount", "amount_raw": amount_raw })
        }
        Some(other) => wire(other),
    }
}

fn approval_of(
    approval: &GuardDetectedApproval,
    editor: Option<&GuardEditorView>,
    choice: Option<&GuardChoice>,
    symbol: &str,
    decimals: u32,
    verified: bool,
) -> Value {
    json!({
        "kind": wire(approval.kind),
        "token": approval.token_address,
        "spender": approval.spender,
        "requested_raw": approval.amount_raw,
        "requested_unbounded": approval.is_unbounded,
        "boolean_grant": approval.is_boolean_grant,
        "reducing": approval.is_reducing,
        "editable": approval.editable,
        "chosen_mode": editor.and_then(|e| e.mode.as_ref()).map(wire),
        "chosen": choice_of(choice.or_else(|| editor.and_then(|e| e.choice.as_ref()))),
        "chosen_amount_raw": editor.and_then(|e| e.display_amount_raw.as_deref()),
        "token_symbol": if verified { Value::String(symbol.to_owned()) } else { untrusted(symbol, "token_contract") },
        "decimals": decimals,
        "decimals_verified": verified,
    })
}

/// The approval guard's cap, as the person chose it.
fn cap_of(guard: &GuardView) -> Value {
    let single = guard.detected.as_ref().map(|approval| {
        approval_of(
            approval,
            guard.editor.as_ref(),
            None,
            &guard.meta.symbol,
            guard.meta.decimals,
            guard.meta.verified,
        )
    });
    let legs = guard.batch.as_ref().map(|batch| {
        batch
            .legs
            .iter()
            .enumerate()
            .filter_map(|(n, leg)| {
                let approval = leg.approval.as_ref()?;
                let mut out = approval_of(
                    approval,
                    leg.editor.as_ref(),
                    leg.choice.as_ref(),
                    &leg.meta.symbol,
                    leg.meta.decimals,
                    leg.meta.verified,
                );
                out["index"] = json!(n + 1);
                out["to"] = json!(leg.to);
                out["still_broad"] = json!(leg.grants_broad);
                out["needs_choice"] = json!(leg.needs_choice);
                Some(out)
            })
            .collect::<Vec<_>>()
    });
    if single.is_none() && legs.as_ref().is_none_or(Vec::is_empty) {
        return Value::Null;
    }
    json!({
        "approval": single,
        "legs": legs,
        "rewritten": guard.rewritten_params_json.is_some(),
        "still_unlimited": guard.unlimited_warning,
        "unlimited_consented": guard.unlimited_consented,
        "confirm_allowed": guard.confirm_allowed,
        "expired": guard.expired,
    })
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct SimEnded {
    verdict: Option<SimVerdict>,
    judged: Option<TrustSimView>,
}

fn judgment(j: &TrustSimJudgment) -> Value {
    match j {
        TrustSimJudgment::Native { delta } => json!({ "asset": "native", "delta_raw": delta }),
        TrustSimJudgment::Erc20Trusted {
            token,
            delta,
            symbol,
            decimals,
            in_trusted_set,
        } => json!({
            "asset": "erc20",
            "token": token,
            "delta_raw": delta,
            "decimals": decimals,
            "symbol": if *in_trusted_set { Value::String(symbol.clone()) } else { untrusted(symbol, "token_contract") },
            "known_token": in_trusted_set,
        }),
        TrustSimJudgment::Erc20Unverified { token, direction } => json!({
            "asset": "erc20_unverified",
            "token": token,
            "direction": wire(direction),
            "note": "an amount the token's own contract reports is not shown",
        }),
        TrustSimJudgment::Nft {
            token,
            token_id,
            standard,
            direction,
            amount,
        } => json!({
            "asset": "nft",
            "standard": standard,
            "token": token,
            "token_id": token_id,
            "direction": wire(direction),
            "amount_raw": amount,
            "note": "nobody vouches for this collection",
        }),
        TrustSimJudgment::Approval {
            token,
            spender,
            unlimited,
            token_id,
        } => json!({
            "asset": "approval",
            "token": token,
            "spender": spender,
            "unlimited": unlimited,
            "token_id": token_id,
        }),
        TrustSimJudgment::Unread { token } => json!({
            "asset": "unread",
            "emitter": token,
            "note": "a log naming the account that Vela could not read",
        }),
    }
}

/// What the sheet's own simulation said.
fn simulation_of(sheet: &SheetFacts, on_chain: bool) -> Value {
    if !on_chain {
        return json!({ "outcome": "not_applicable" });
    }
    let Some(ended) = sheet.simulation.as_ref() else {
        return json!({ "outcome": if sheet.simulation_pending { "checking" } else { "could_not_check" } });
    };
    let ended: SimEnded = serde_json::from_value(ended.clone()).unwrap_or_default();
    let source = json!({ "kind": "simulation" });
    let Some(verdict) = ended.verdict else {
        return json!({ "outcome": "could_not_check", "source": source });
    };
    match verdict.kind.as_str() {
        "reverts" => json!({
            "outcome": "reverts",
            "reason": verdict.revert_reason.as_deref().map(|r| untrusted(r, "contract")),
            "source": source,
        }),
        "deltas" => match ended.judged.filter(|j| j.ready) {
            None => json!({ "outcome": "could_not_check", "source": source }),
            Some(judged) if judged.no_change_key.is_some() => {
                json!({ "outcome": "no_change", "source": source })
            }
            Some(judged) => json!({
                "outcome": "balance_changes",
                "changes": judged.judgments.iter().map(judgment).collect::<Vec<_>>(),
                "source": source,
            }),
        },
        _ => json!({ "outcome": "could_not_check", "source": source }),
    }
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct FeeEstimate {
    total_wei: String,
    fee_asset: Value,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct FeeShown {
    busy: bool,
    stale: bool,
    failed: Option<Value>,
    fee: Option<FeeEstimate>,
}

/// The fee in force, as exact strings.
fn fee_of(fee: &Value, native_symbol: Option<&str>) -> Value {
    let fee: FeeShown = serde_json::from_value(fee.clone()).unwrap_or_default();
    let state = if fee.failed.is_some() {
        "failed"
    } else if fee.busy {
        "estimating"
    } else if fee.stale {
        "stale"
    } else {
        "quoted"
    };
    let Some(estimate) = fee.fee else {
        return json!({ "state": state, "failure": fee.failed });
    };
    let asset = &estimate.fee_asset;
    if asset.get("type").and_then(Value::as_str) == Some("erc20") {
        let raw = asset.get("amount").and_then(Value::as_str).unwrap_or("");
        let decimals = asset
            .get("decimals")
            .and_then(Value::as_u64)
            .and_then(|d| u32::try_from(d).ok())
            .unwrap_or(18);
        return json!({
            "state": state,
            "amount": exact_units(raw, decimals),
            "amount_raw": raw,
            "decimals": decimals,
            "symbol": asset.get("symbol"),
            "token": asset.get("token"),
        });
    }
    json!({
        "state": state,
        "amount": exact_units(&estimate.total_wei, 18),
        "amount_raw": estimate.total_wei,
        "decimals": 18,
        "symbol": native_symbol,
        "token": null,
    })
}

// ---------------------------------------------------------------------------
// The review
// ---------------------------------------------------------------------------

/// A request's kind by its method, for a shell that did not say it.
fn kind_of_method(method: &str) -> &'static str {
    match method {
        "personal_sign" => "personal_sign",
        "eth_sign" => "eth_sign",
        m if m.starts_with("eth_signTypedData") => "typed_data",
        "eth_sendTransaction" => "transaction",
        "wallet_sendCalls" => "batch",
        _ => "",
    }
}

fn parsed(json_text: &str) -> Value {
    serde_json::from_str::<Value>(json_text).unwrap_or_else(|_| Value::String(json_text.to_owned()))
}

/// `signing` of a `vela_review` answer: the request, what the sheet reads of
/// it with every line's source, the person's cap, the simulation, the fee,
/// and the digest an opinion is bound to.
#[must_use]
pub fn signing(
    request: &SignFacts,
    sheet: Option<&SheetFacts>,
    account: Option<&str>,
    native_symbol: Option<&str>,
) -> Value {
    let empty = SheetFacts::default();
    let sheet = sheet.unwrap_or(&empty);
    let clear: Option<ClearSigningView> = sheet
        .clear
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let guard: Option<GuardView> = sheet
        .guard
        .as_ref()
        .and_then(|v| serde_json::from_value(v.clone()).ok());
    let host = sheet.descriptor_base.as_deref().and_then(host_of);
    let host = host.as_deref();
    let kind = request
        .kind
        .as_ref()
        .and_then(Value::as_str)
        .unwrap_or_else(|| kind_of_method(&request.method));
    let on_chain = matches!(kind, "transaction" | "batch");
    let params = parsed(&request.params_json);
    let signed_params_json = guard
        .as_ref()
        .and_then(|g| g.rewritten_params_json.clone())
        .unwrap_or_else(|| request.params_json.clone());

    let reading = clear.as_ref().map_or(Value::Null, |c| {
        json!({
            "state": if c.resolving { "reading" } else if c.resolved { "read" } else { "waiting" },
            "surface": wire(c.surface),
            "call": c.result.as_ref().map(|r| reading_of(r, host)),
            "no_code": c.no_code,
            "sign_anyway": c.sign_anyway,
            "native_value": c.native_value.as_ref().map(|v| json!({ "value_wei": v.value_wei, "amount": v.amount })),
            "plain_send": c.plain_send.as_ref().map(|s| json!({ "to": s.to, "value_wei": s.value_wei, "amount": s.amount })),
        })
    });
    let batch = clear
        .as_ref()
        .and_then(|c| c.batch.as_ref())
        .map_or(Value::Null, |b| batch_of(b, host));
    let message = clear.as_ref().and_then(|c| c.message.as_ref()).map_or_else(
        || {
            matches!(kind, "personal_sign" | "eth_sign")
                .then(|| {
                    json!({
                        "text": null,
                        "has_invisible_chars": any_invisible(&params),
                    })
                })
                .unwrap_or(Value::Null)
        },
        |m| message_of(m, &params),
    );
    let typed_data = if kind == "typed_data" {
        typed_of(clear.as_ref().and_then(|c| c.blind_typed.as_ref()), &params)
    } else {
        Value::Null
    };
    let approval_cap = guard.as_ref().map_or(Value::Null, cap_of);
    let simulation = simulation_of(sheet, on_chain);
    let signer = request
        .signer_address
        .as_deref()
        .or(account)
        .map(str::to_lowercase);

    let digest = digest_of(&json!({
        "v": DIGEST_DOMAIN,
        "chain_id": request.chain_id,
        "account": signer,
        "method": request.method,
        "origin": request.origin,
        "signed_params": parsed(&signed_params_json),
        "reading": reading,
        "batch": batch,
        "message": message,
        "typed_data": typed_data,
        "approval_cap": approval_cap,
        "simulation": simulation,
    }));

    let site = request.dapp.as_ref().map(|dapp| {
        json!({
            "name": dapp.get("name").and_then(Value::as_str).map(|n| untrusted(n, "site")),
            "url": dapp.get("url").and_then(Value::as_str).map(|u| untrusted(u, "site")),
        })
    });
    let mut out = Map::new();
    out.insert("id".into(), json!(request.id));
    out.insert("method".into(), json!(request.method));
    out.insert("kind".into(), request.kind.clone().unwrap_or(Value::Null));
    out.insert("origin".into(), untrusted(&request.origin, "site"));
    out.insert("site_claims".into(), site.unwrap_or(Value::Null));
    out.insert("chain_id".into(), json!(request.chain_id));
    out.insert("signer".into(), json!(request.signer_address));
    out.insert("reading".into(), reading);
    out.insert("batch".into(), batch);
    out.insert("message".into(), message);
    out.insert("typed_data".into(), typed_data);
    out.insert("approval_cap".into(), approval_cap);
    out.insert("simulation".into(), simulation);
    out.insert(
        "fee".into(),
        sheet
            .fee
            .as_ref()
            .filter(|_| on_chain)
            .map_or(Value::Null, |fee| fee_of(fee, native_symbol)),
    );
    out.insert(
        "requested_params".into(),
        untrusted(&request.params_json, "site"),
    );
    out.insert(
        "signed_params".into(),
        untrusted(&signed_params_json, "site"),
    );
    out.insert("review_digest".into(), json!(digest));
    out.insert("digest_covers".into(), json!(DIGEST_COVERS));
    Value::Object(out)
}

// ---------------------------------------------------------------------------
// The agent's opinion
// ---------------------------------------------------------------------------

/// What `vela_advise` reads: the agent's arguments, the digest of the
/// review on the sheet now (`None` when none is open), the fee that review
/// shows (its `fee`, as `vela_review` reads it), and the world the allow
/// list is read from.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct AdviceInput {
    pub world: World,
    pub current_digest: Option<String>,
    /// The review's `fee` on the sheet now: what the opinion is recorded
    /// as given for. `None` for a message, or a fee not yet measured.
    pub current_fee: Option<Value>,
    /// `None`: only whether the agent may read this wallet's reviews at all
    /// (`vela_watch_reviews`).
    pub args: Option<Value>,
}

fn refused(code: &str, detail: &str) -> Value {
    json!({ "ok": false, "code": code, "detail": detail })
}

/// A fee's figure, as a review's `fee` carries it: the coin (`None` for
/// the network's own) and the amount in its base units.
struct FeeFigure {
    token: Option<String>,
    raw: u128,
    decimals: u32,
}

fn figure_of(fee: Option<&Value>) -> Option<FeeFigure> {
    let fee = fee?;
    let raw = fee.get("amount_raw").and_then(Value::as_str)?;
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(FeeFigure {
        token: fee
            .get("token")
            .and_then(Value::as_str)
            .map(str::to_ascii_lowercase),
        // A figure past u128 is no fee anyone pays: it is never in band.
        raw: raw.parse().unwrap_or(u128::MAX),
        decimals: fee
            .get("decimals")
            .and_then(Value::as_u64)
            .and_then(|d| u32::try_from(d).ok())
            .unwrap_or(18),
    })
}

/// The fee an opinion records: the review's figure, exact, with its coin.
/// `null` when the sheet showed none (a message, or a fee still measured).
fn advised_fee(fee: Option<&Value>) -> Value {
    let Some(fee) = fee.filter(|_| figure_of(fee).is_some()) else {
        return Value::Null;
    };
    json!({
        "amount_raw": fee.get("amount_raw"),
        "amount": fee.get("amount"),
        "decimals": fee.get("decimals"),
        "symbol": fee.get("symbol"),
        "token": fee.get("token"),
    })
}

/// `now` within the band of `given`: the same coin, and at most
/// [`ADVICE_FEE_BAND_PERCENT`] away from it — or the coin's small fixed
/// floor, when that is wider.
fn fee_in_band(given: &FeeFigure, now: &FeeFigure) -> bool {
    if given.token != now.token {
        return false;
    }
    let places = if given.token.is_some() {
        FEE_FLOOR_STABLE_PLACES
    } else {
        FEE_FLOOR_NATIVE_PLACES
    };
    let floor = 10u128
        .checked_pow(given.decimals.saturating_sub(places))
        .unwrap_or(u128::MAX);
    let band = (given.raw / 100)
        .saturating_mul(ADVICE_FEE_BAND_PERCENT)
        .saturating_add(given.raw % 100 * ADVICE_FEE_BAND_PERCENT / 100)
        .max(floor);
    given.raw.abs_diff(now.raw) <= band
}

/// `raw / 10^decimals` to [`FEE_SHOWN_DIGITS`] significant digits (cut, not
/// rounded), for the card's "Fee when given".
fn shown_units(raw: &str, decimals: u32) -> Option<String> {
    let exact = exact_units(raw, decimals)?;
    let Some((whole, frac)) = exact.split_once('.') else {
        return Some(exact);
    };
    let mut significant = if whole == "0" { 0 } else { whole.len() };
    let mut kept = String::new();
    for c in frac.chars() {
        if significant >= FEE_SHOWN_DIGITS {
            break;
        }
        if significant > 0 || c != '0' {
            significant += 1;
        }
        kept.push(c);
    }
    let kept = kept.trim_end_matches('0');
    Some(if kept.is_empty() {
        whole.to_owned()
    } else {
        format!("{whole}.{kept}")
    })
}

/// How an opinion stands against the sheet now: `current` while its digest
/// is the one on screen and the fee stayed in band; `earlier_version` once
/// the request it was given for changed; `fee_changed` once the fee moved
/// past the band or to another coin — or a figure landed where the opinion
/// saw none. A sheet with no figure now (re-pricing, failed) leaves the
/// opinion as it stood: there is nothing new to compare.
#[must_use]
pub fn standing_of(
    advice: &Value,
    current_digest: Option<&str>,
    current_fee: Option<&Value>,
) -> &'static str {
    let digest = advice
        .get("review_digest")
        .and_then(Value::as_str)
        .unwrap_or("");
    if !current_digest.is_some_and(|current| current.eq_ignore_ascii_case(digest)) {
        return "earlier_version";
    }
    let moved = match (figure_of(advice.get("fee")), figure_of(current_fee)) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some(given), Some(now)) => !fee_in_band(&given, &now),
    };
    if moved {
        "fee_changed"
    } else {
        "current"
    }
}

/// What the sheet asks of an opinion it holds ([`advice_standing`]).
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct StandingInput {
    /// The opinion as [`advice`] accepted it.
    pub advice: Value,
    pub current_digest: Option<String>,
    /// The review's `fee` on the sheet now.
    pub current_fee: Option<Value>,
}

/// The opinion's standing ([`standing_of`]) and the fee it was given for
/// as its card says it (`{ amount, symbol }`; `null` when it saw none).
#[must_use]
pub fn advice_standing(input: &StandingInput) -> Value {
    let fee = input.advice.get("fee");
    let when_given = figure_of(fee).and_then(|figure| {
        let fee = fee?;
        Some(json!({
            "amount": shown_units(fee.get("amount_raw").and_then(Value::as_str)?, figure.decimals),
            "symbol": fee.get("symbol").and_then(Value::as_str),
        }))
    });
    json!({
        "standing": standing_of(
            &input.advice,
            input.current_digest.as_deref(),
            input.current_fee.as_ref(),
        ),
        "fee_when_given": when_given,
    })
}

/// `vela_advise`: the agent's opinion, accepted as plain, capped text bound
/// to a review digest and to the fee on the sheet — or refused. `standing`
/// (and `current`) say whether it was given for the review on the sheet
/// now; an opinion for an earlier version is kept and drawn greyed, never
/// as current.
#[must_use]
pub fn advice(input: &AdviceInput) -> Value {
    if input.world.front().is_none() || !input.world.agent_may_use_active() {
        return refused(
            "wallet_not_allowed",
            "the person has not allowed the agent to use this wallet",
        );
    }
    let Some(args) = input.args.as_ref() else {
        return json!({ "ok": true });
    };
    let Some(digest) = args
        .get("review_digest")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|d| !d.is_empty())
    else {
        return refused("invalid_input", "review_digest is required");
    };
    let Some(verdict) = args
        .get("verdict")
        .and_then(Value::as_str)
        .filter(|v| ADVICE_VERDICTS.contains(v))
    else {
        return refused(
            "invalid_input",
            "verdict is one of looks_fine, caution, danger, unsure",
        );
    };
    let summary = plain_text(
        args.get("summary").and_then(Value::as_str).unwrap_or(""),
        ADVICE_SUMMARY_MAX,
    );
    if summary.is_empty() {
        return refused("invalid_input", "summary is required");
    }
    let strings = |key: &str| -> Vec<String> {
        args.get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let points: Vec<String> = strings("points")
        .iter()
        .map(|p| plain_text(p, ADVICE_POINT_MAX))
        .filter(|p| !p.is_empty())
        .take(ADVICE_LIST_MAX)
        .collect();
    let mut hosts: Vec<String> = Vec::new();
    for host in strings("sources").iter().filter_map(|s| host_of(s)) {
        if hosts.len() < ADVICE_LIST_MAX && !hosts.contains(&host) {
            hosts.push(host);
        }
    }
    let Some(current) = input.current_digest.as_deref() else {
        return refused(
            "not_found",
            "no signing review is open: read one with vela_review or vela_watch_reviews",
        );
    };
    // Given for the review on screen, the fee it shows now is the one the
    // opinion was given for; for an earlier version, that fee is unknown.
    let given_now = current.eq_ignore_ascii_case(digest);
    let advice = json!({
        "review_digest": digest,
        "verdict": verdict,
        "summary": summary,
        "points": points,
        "source_hosts": hosts,
        "fee": if given_now { advised_fee(input.current_fee.as_ref()) } else { Value::Null },
    });
    let standing = standing_of(&advice, Some(current), input.current_fee.as_ref());
    json!({
        "ok": true,
        "current": standing == "current",
        "standing": standing,
        "advice": advice,
    })
}
