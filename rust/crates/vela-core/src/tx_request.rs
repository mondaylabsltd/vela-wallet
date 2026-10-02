//! One reading of a transaction request's calls (spec 096 F1).
//!
//! The real-dApp pass of 2026-10-02 found every dApp call that sends native
//! coin — PancakeSwap's `value: "0xaa87bee538000"`, Aave's `depositETH` —
//! dying on the web before a signature: the submit stripped the `0x`, and the
//! gas floor then read the bare digits with `BigInt`, which reads hex only
//! behind its prefix. Behind that one throw sat a reader of `value` per shell,
//! no two alike:
//!
//! | reader                          | `"1000"`        | `1000` (number) | `"0x"` | `"0X1f"` |
//! |---------------------------------|-----------------|-----------------|--------|----------|
//! | signing card (RC4)              | refused         | refused         | 0      | refused  |
//! | web fee quote (`weiOf`)         | 1000 (decimal)  | 1000            | 0      | 0        |
//! | web submit                      | 0x1000 (hex)    | throws          | 0      | throws   |
//! | desktop submit (`wei_of`)       | 1000 (decimal)  | 1000            | 0      | 31       |
//! | iOS submit (`callsOf`)          | 0x1000 (hex)    | 0               | leg dropped | leg dropped |
//! | Android submit (`callsOf`)      | 0x1000 (hex)    | refused         | 0      | refused  |
//!
//! — so one request could be priced as one amount and signed as another, and
//! the same request signed as different amounts on different shells.
//!
//! Here the value is read ONCE, by the rule the signing card already states
//! amounts by (spec 082 RC4), so what the card says is what every shell sends:
//!
//! - absent, JSON `null`, `""` and `"0x"` are zero;
//! - `0x` and hex digits (any case) are that exact number, up to `2^256 − 1`;
//! - zero written without the prefix — `"0"`, `"00"`, the JSON number `0` —
//!   is zero: it reads the same in every base, and dApps send it;
//! - anything else — decimal or bare-hex text, any other JSON number, a sign,
//!   whitespace, `"0X"`, an overflow — is refused, never guessed.
//!
//! At arrival (`sign_request`) a request whose value breaks the rule is
//! refused `-32602` before any sheet ([`canonical_params_json`]), and a
//! readable one not already written canonically (`0x`, lower-case, no leading
//! zeros) is rewritten so — every later reader holds the same text. The
//! calls a submit sends are [`calls_of`], for every shell.

use alloy_primitives::U256;
use serde::Serialize;
use serde_json::Value;

/// `2^256 − 1` has 64 hex digits.
const U256_HEX_DIGITS: usize = 64;

/// The two methods whose params carry calls.
#[must_use]
pub fn is_transaction_method(method: &str) -> bool {
    matches!(method, "eth_sendTransaction" | "wallet_sendCalls")
}

/// A call's `value` text as exact decimal wei, or `None` when the rule (see
/// the module doc) refuses it. `None` given is zero.
#[must_use]
pub fn value_wei_text(value: Option<&str>) -> Option<String> {
    let value = match value {
        None => return Some("0".to_owned()),
        Some(v) if v.is_empty() || v == "0x" || v.bytes().all(|b| b == b'0') => {
            return Some("0".to_owned())
        }
        Some(v) => v,
    };
    let body = value.strip_prefix("0x")?;
    if !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let significant = body.trim_start_matches('0');
    if significant.len() > U256_HEX_DIGITS {
        return None;
    }
    if significant.is_empty() {
        return Some("0".to_owned());
    }
    U256::from_str_radix(significant, 16)
        .ok()
        .map(|wei| wei.to_string())
}

/// A call's `value` field as exact decimal wei, or `None` when refused.
/// Absent and JSON `null` are zero; a JSON number is refused unless it is `0`.
#[must_use]
pub fn value_wei(value: Option<&Value>) -> Option<String> {
    match value {
        None | Some(Value::Null) => Some("0".to_owned()),
        Some(Value::String(text)) => value_wei_text(Some(text)),
        Some(Value::Number(n)) if n.as_u64() == Some(0) => Some("0".to_owned()),
        Some(_) => None,
    }
}

/// The canonical text of a readable value: `0x`, lower-case, no leading
/// zeros, `"0x0"` for zero. `None` when the rule refuses it.
#[must_use]
pub fn canonical_value(value: &Value) -> Option<String> {
    value_wei(Some(value))?;
    let body = value
        .as_str()
        .and_then(|text| text.strip_prefix("0x"))
        .unwrap_or("")
        .trim_start_matches('0')
        .to_ascii_lowercase();
    Some(if body.is_empty() {
        "0x0".to_owned()
    } else {
        format!("0x{body}")
    })
}

/// Why a transaction request is refused at arrival (`-32602`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TxRequestError {
    /// A call's `value` is not a hex quantity. `leg` is the call's index in a
    /// `wallet_sendCalls` batch, `None` for an `eth_sendTransaction`.
    BadValue { leg: Option<usize> },
}

impl TxRequestError {
    /// The fixed English message the dApp is answered with.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Self::BadValue { leg: None } => {
                "Invalid transaction value: expected a 0x-prefixed hex quantity".to_owned()
            }
            Self::BadValue { leg: Some(leg) } => {
                format!("Invalid value in call {leg}: expected a 0x-prefixed hex quantity")
            }
        }
    }
}

/// The request as every later reader must hold it: `Ok(None)` when it is
/// already canonical (or carries no calls to read — the params are left for
/// their own readers to refuse), `Ok(Some(params_json))` when a value was
/// rewritten to its canonical text, `Err` when a value breaks the rule.
///
/// Only `value` is touched, and only when it is not already canonical, so the
/// common request (viem, ethers and web3 all write canonical quantities)
/// passes byte for byte.
///
/// # Errors
/// [`TxRequestError::BadValue`] for the first call whose value is refused.
pub fn canonical_params_json(
    method: &str,
    params_json: &str,
) -> Result<Option<String>, TxRequestError> {
    if !is_transaction_method(method) {
        return Ok(None);
    }
    let Ok(mut params) = serde_json::from_str::<Value>(params_json) else {
        return Ok(None);
    };
    let Some(first) = params.get_mut(0) else {
        return Ok(None);
    };
    let mut changed = false;
    if method == "wallet_sendCalls" {
        if let Some(calls) = first.get_mut("calls").and_then(Value::as_array_mut) {
            for (leg, call) in calls.iter_mut().enumerate() {
                changed |= canonicalize(call, Some(leg))?;
            }
        }
    } else {
        changed |= canonicalize(first, None)?;
    }
    Ok(changed.then(|| params.to_string()))
}

/// One call's `value` made canonical in place; `true` when it changed.
fn canonicalize(call: &mut Value, leg: Option<usize>) -> Result<bool, TxRequestError> {
    let Some(value) = call.as_object_mut().and_then(|call| call.get_mut("value")) else {
        return Ok(false);
    };
    if value.is_null() {
        return Ok(false);
    }
    let canonical = canonical_value(value).ok_or(TxRequestError::BadValue { leg })?;
    if value.as_str() == Some(canonical.as_str()) {
        return Ok(false);
    }
    *value = Value::String(canonical);
    Ok(true)
}

/// One call a submit sends: base units as a DECIMAL string (the
/// `FeeCall` / `UserOpCall` convention), `0x`-hex data, `"0x"` for none.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RequestCall {
    pub to: String,
    pub value: String,
    pub data: String,
}

/// The calls a request sends — `params[0].calls` of a `wallet_sendCalls`,
/// else `params[0]` itself — every one or none: a call with no recipient, an
/// unreadable value or non-text data refuses the whole request (a batch sent
/// without one of its legs is a batch the page never asked for). An empty
/// batch is not a batch.
#[must_use]
pub fn calls_of(method: &str, params_json: &str) -> Option<Vec<RequestCall>> {
    let params: Value = serde_json::from_str(params_json).ok()?;
    let first = params.get(0)?;
    let calls = if method == "wallet_sendCalls" {
        first
            .get("calls")?
            .as_array()?
            .iter()
            .map(call_of)
            .collect::<Option<Vec<_>>>()?
    } else {
        vec![call_of(first)?]
    };
    (!calls.is_empty()).then_some(calls)
}

fn call_of(raw: &Value) -> Option<RequestCall> {
    let to = raw.get("to")?.as_str()?;
    if to.is_empty() {
        return None;
    }
    let data = match raw.get("data") {
        None | Some(Value::Null) => "0x",
        Some(Value::String(text)) if text.is_empty() => "0x",
        Some(Value::String(text)) => text.as_str(),
        Some(_) => return None,
    };
    Some(RequestCall {
        to: to.to_owned(),
        value: value_wei(raw.get("value"))?,
        data: data.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const ROUTER: &str = "0x13f4EA83D0bd40E75C8222255bc855a974568Dd4";

    /// The shared table every shell's own reading is pinned against (spec
    /// 096): the text a dApp writes → the wei the core reads, `None` refused.
    pub(crate) const VALUE_TABLE: &[(&str, Option<&str>)] = &[
        // PancakeSwap's BNB → USDC, the request that failed on 2026-10-02.
        ("\"0xaa87bee538000\"", Some("3000000000000000")),
        ("\"0xAA87BEE538000\"", Some("3000000000000000")),
        ("\"0x0\"", Some("0")),
        ("\"0x\"", Some("0")),
        ("\"\"", Some("0")),
        ("null", Some("0")),
        ("\"0\"", Some("0")),
        ("\"000\"", Some("0")),
        ("0", Some("0")),
        (
            "\"0x0000000000000000000000000000000000000000000000000000000000000001\"",
            Some("1"),
        ),
        (
            "\"0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff\"",
            Some("115792089237316195423570985008687907853269984665640564039457584007913129639935"),
        ),
        // 2^256 does not fit.
        (
            "\"0x10000000000000000000000000000000000000000000000000000000000000000\"",
            None,
        ),
        // Decimal text, bare hex, numbers: ambiguous, refused.
        ("\"1000\"", None),
        ("\"aa87bee538000\"", None),
        ("1000", None),
        ("1.5", None),
        ("\"0X1f\"", None),
        ("\" 0x1\"", None),
        ("\"-0x1\"", None),
        ("\"0x1g\"", None),
        ("true", None),
        ("{}", None),
    ];

    #[test]
    fn the_value_table() {
        for (json_text, wei) in VALUE_TABLE {
            let value: Value = serde_json::from_str(json_text).unwrap_or_else(|_| unreachable!());
            assert_eq!(
                value_wei(Some(&value)).as_deref(),
                *wei,
                "value {json_text}"
            );
        }
        assert_eq!(value_wei(None).as_deref(), Some("0"), "absent");
    }

    #[test]
    fn the_card_reads_text_by_the_same_rule() {
        for (json_text, wei) in VALUE_TABLE {
            if let Ok(Value::String(text)) = serde_json::from_str::<Value>(json_text) {
                assert_eq!(value_wei_text(Some(&text)).as_deref(), *wei, "{text}");
            }
        }
    }

    #[test]
    fn a_canonical_request_passes_byte_for_byte() {
        let params = format!(
            r#"[{{"from":"0x88cCA0eedbf2C4426110bBfC998f048689266894","to":"{ROUTER}","value":"0xaa87bee538000","data":"0x3593564c"}}]"#
        );
        assert_eq!(
            canonical_params_json("eth_sendTransaction", &params),
            Ok(None)
        );
    }

    #[test]
    fn a_readable_value_is_rewritten_canonically() {
        let params = format!(r#"[{{"to":"{ROUTER}","value":"0x000AA87BEE538000"}}]"#);
        let rewritten = canonical_params_json("eth_sendTransaction", &params)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let read: Value = serde_json::from_str(&rewritten).unwrap_or_else(|_| unreachable!());
        assert_eq!(read[0]["value"], json!("0xaa87bee538000"));
        assert_eq!(read[0]["to"], json!(ROUTER));
        for zero in [r#""0x""#, r#""""#, r#""0""#, "0", r#""0x00""#] {
            let params = format!(r#"[{{"to":"{ROUTER}","value":{zero}}}]"#);
            let rewritten = canonical_params_json("eth_sendTransaction", &params)
                .unwrap_or_else(|_| unreachable!())
                .unwrap_or_else(|| unreachable!());
            assert!(
                rewritten.contains(r#""value":"0x0""#),
                "{zero}: {rewritten}"
            );
        }
    }

    #[test]
    fn an_unreadable_value_refuses_the_request() {
        let params = format!(r#"[{{"to":"{ROUTER}","value":"aa87bee538000"}}]"#);
        let refused = canonical_params_json("eth_sendTransaction", &params);
        assert_eq!(refused, Err(TxRequestError::BadValue { leg: None }));
        assert!(TxRequestError::BadValue { leg: None }
            .message()
            .contains("0x-prefixed hex quantity"));

        let batch = format!(
            r#"[{{"calls":[{{"to":"{ROUTER}","value":"0x1"}},{{"to":"{ROUTER}","value":1000}}]}}]"#
        );
        assert_eq!(
            canonical_params_json("wallet_sendCalls", &batch),
            Err(TxRequestError::BadValue { leg: Some(1) })
        );
    }

    #[test]
    fn a_batch_leg_is_rewritten_in_place_and_null_stays() {
        let batch = format!(
            r#"[{{"calls":[{{"to":"{ROUTER}","value":"0x"}},{{"to":"{ROUTER}","value":null}},{{"to":"{ROUTER}"}}]}}]"#
        );
        let rewritten = canonical_params_json("wallet_sendCalls", &batch)
            .unwrap_or_else(|_| unreachable!())
            .unwrap_or_else(|| unreachable!());
        let read: Value = serde_json::from_str(&rewritten).unwrap_or_else(|_| unreachable!());
        assert_eq!(read[0]["calls"][0]["value"], json!("0x0"));
        assert_eq!(read[0]["calls"][1]["value"], Value::Null);
        assert!(read[0]["calls"][2].get("value").is_none());
    }

    #[test]
    fn other_methods_and_shapeless_params_are_left_alone() {
        assert_eq!(
            canonical_params_json("personal_sign", r#"["0x1","0x2"]"#),
            Ok(None)
        );
        assert_eq!(
            canonical_params_json("eth_sendTransaction", "not json"),
            Ok(None)
        );
        assert_eq!(canonical_params_json("eth_sendTransaction", "[]"), Ok(None));
        assert_eq!(
            canonical_params_json("wallet_sendCalls", r#"[{"calls":"x"}]"#),
            Ok(None)
        );
    }

    #[test]
    fn the_calls_a_submit_sends() {
        // PancakeSwap's swap, Aave's depositETH, a batch with a native leg.
        let swap =
            format!(r#"[{{"to":"{ROUTER}","value":"0xaa87bee538000","data":"0x3593564c"}}]"#);
        assert_eq!(
            calls_of("eth_sendTransaction", &swap),
            Some(vec![RequestCall {
                to: ROUTER.to_owned(),
                value: "3000000000000000".to_owned(),
                data: "0x3593564c".to_owned(),
            }])
        );
        let batch = format!(
            r#"[{{"calls":[{{"to":"{ROUTER}","data":"0x095ea7b3"}},{{"to":"{ROUTER}","value":"0xaa87bee538000","data":null}}]}}]"#
        );
        let calls = calls_of("wallet_sendCalls", &batch).unwrap_or_else(|| unreachable!());
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].value, "0");
        assert_eq!(calls[1].value, "3000000000000000");
        assert_eq!(calls[1].data, "0x");
    }

    #[test]
    fn every_call_or_none() {
        for batch in [
            // An unreadable value, a missing recipient, data that is not text.
            format!(
                r#"[{{"calls":[{{"to":"{ROUTER}","value":"0x1"}},{{"to":"{ROUTER}","value":"1"}}]}}]"#
            ),
            format!(r#"[{{"calls":[{{"to":"{ROUTER}","value":"0x1"}},{{"value":"0x1"}}]}}]"#),
            format!(
                r#"[{{"calls":[{{"to":"{ROUTER}","value":"0x1"}},{{"to":"{ROUTER}","data":12}}]}}]"#
            ),
            format!(r#"[{{"calls":[{{"to":"{ROUTER}"}},{{"to":""}}]}}]"#),
            r#"[{"calls":[]}]"#.to_owned(),
        ] {
            assert_eq!(calls_of("wallet_sendCalls", &batch), None, "{batch}");
        }
        assert_eq!(calls_of("eth_sendTransaction", "[]"), None);
        assert_eq!(calls_of("eth_sendTransaction", "{}"), None);
    }
}
