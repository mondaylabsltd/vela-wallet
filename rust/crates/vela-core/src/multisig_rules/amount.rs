//! An amount the core hands a shell (r12-1) — with the token's decimals and
//! symbol it was read in, or an explicit "decimals unknown" with its base
//! units. Never a bare base-unit string a shell has to scale itself.
//!
//! The round-12 bug: the "would fail" line of a multisig's 0.01 USDC transfer
//! on Arc Testnet read "the multisig holds 0, and this needs <0.000001". The
//! shortfall carried `have` / `need` in base units and the token as a bare
//! address; Android and iOS looked for decimals that were not there and fell
//! back to 18 (10,000 base units of a 6-decimal token is 0.01, of an
//! 18-decimal one 10^-14). The shape below makes that impossible: the number
//! is the core's text, said in the decimals it knows — or a corpus key
//! ([`DECIMALS_UNKNOWN_KEY`]) with the base units, when it knows none.
//!
//! Door: `amount.of {base_units, chain_id, token?, symbol?, decimals?}` →
//! [`Amount`]. Vectors: `tests/vectors/multisig/amounts.json`.

use serde::{Deserialize, Serialize};
#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::{arg, to_json};
use crate::multisig_link::format_units;
use crate::CoreError;

/// "{{raw}} base units · decimals unknown" — an amount whose token never said
/// its decimals (the review's own words for it).
pub const DECIMALS_UNKNOWN_KEY: &str = "home.multisig.review.reading.decimalsUnknown";

/// How an [`Amount`] is said — exactly one of two shapes, so a shell cannot
/// draw a number the core did not write.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigAmountSaid"))]
pub enum AmountSaid {
    /// The decimals are known: draw `text` as it is ("0.01 USDC"; "0.01"
    /// when no symbol is known). `number` is the figure alone.
    Known {
        text: String,
        number: String,
        symbol: Option<String>,
        decimals: u32,
    },
    /// Nobody said the token's decimals: draw `key` with `{{raw}}` = `raw`
    /// (its base units) — "10000 base units · decimals unknown". Never a
    /// guess at 18.
    DecimalsUnknown { key: String, raw: String },
}

/// An amount of one token, as a shell draws it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigAmount"))]
pub struct Amount {
    /// The exact amount in the token's base units (decimal digits).
    pub base_units: String,
    /// The token's contract, lowercase; `None` for the network's own coin.
    pub token: Option<String>,
    pub said: AmountSaid,
}

/// `base_units` of `token` (`None`: the network's coin, 18 decimals and the
/// chain's own symbol when the wallet ships the chain) with what is known of
/// it. A token the registry names takes the registry's decimals and symbol
/// whatever `symbol` / `decimals` say.
#[must_use]
pub fn of(
    base_units: &str,
    chain_id: u64,
    token: Option<&str>,
    symbol: Option<&str>,
    decimals: Option<u32>,
) -> Amount {
    let base_units = digits(base_units);
    let token = token.map(str::to_ascii_lowercase);
    let chain = u32::try_from(chain_id).ok();
    let (symbol, decimals) = match token.as_deref() {
        None => (
            chain
                .and_then(crate::app::network_admin::builtin_native_symbol)
                .map(str::to_owned)
                .or_else(|| symbol.map(str::to_owned)),
            Some(18),
        ),
        Some(address) => {
            match chain.and_then(|c| crate::app::token_registry::registry_token(c, address)) {
                Some(known) => (Some(known.symbol.to_owned()), Some(known.decimals)),
                None => (symbol.map(str::to_owned), decimals.filter(|d| *d <= 36)),
            }
        }
    };
    let symbol = symbol
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty());
    let said = match decimals {
        Some(decimals) => {
            let number = format_units(&base_units, decimals).unwrap_or_else(|_| base_units.clone());
            let text = match &symbol {
                Some(symbol) => format!("{number} {symbol}"),
                None => number.clone(),
            };
            AmountSaid::Known {
                text,
                number,
                symbol,
                decimals,
            }
        }
        None => AmountSaid::DecimalsUnknown {
            key: DECIMALS_UNKNOWN_KEY.to_owned(),
            raw: base_units.clone(),
        },
    };
    Amount {
        base_units,
        token,
        said,
    }
}

/// A base-unit amount as decimal digits: a `0x` quantity read as hex, a
/// decimal kept, anything else `0`.
fn digits(raw: &str) -> String {
    let raw = raw.trim();
    let value = match raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        Some(hex) if !hex.is_empty() => u128::from_str_radix(hex, 16).ok().map(|v| v.to_string()),
        Some(_) => Some("0".to_owned()),
        None if !raw.is_empty() && raw.chars().all(|c| c.is_ascii_digit()) => {
            Some(raw.trim_start_matches('0').to_owned())
        }
        None => None,
    };
    match value {
        Some(v) if v.is_empty() => "0".to_owned(),
        Some(v) => v,
        None => "0".to_owned(),
    }
}

pub(super) fn call(method: &str, args: &serde_json::Value) -> Result<String, CoreError> {
    match method {
        "of" => {
            let base: serde_json::Value = arg(args, "base_units")?;
            let base = match base {
                serde_json::Value::String(s) => s,
                serde_json::Value::Number(n) => n.to_string(),
                _ => return Err(CoreError::Multisig("argument base_units: a string".into())),
            };
            let chain_id: u64 = arg(args, "chain_id")?;
            let token: Option<String> = arg(args, "token")?;
            let symbol: Option<String> = arg(args, "symbol")?;
            let decimals: Option<u32> = arg(args, "decimals")?;
            to_json(&of(
                &base,
                chain_id,
                token.as_deref(),
                symbol.as_deref(),
                decimals,
            ))
        }
        other => Err(CoreError::Multisig(format!("amount.{other}: no such rule"))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::Value;

    /// The shared vectors every shell replays through the door (r12-1).
    #[test]
    fn the_amount_vectors_hold() {
        let path = format!(
            "{}/tests/vectors/multisig/amounts.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let cases = v["cases"].as_array().unwrap();
        assert!(cases.len() >= 8);
        for case in cases {
            let got: Value = serde_json::from_str(&call("of", &case["args"]).unwrap()).unwrap();
            assert_eq!(got, case["expect"], "{}", case["name"]);
        }
    }

    /// r12 S4: the confirm's fee row, through the same door.
    #[test]
    fn the_fee_row_vectors_hold() {
        let path = format!(
            "{}/tests/vectors/fee/confirm-row.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        for case in v["cases"].as_array().unwrap() {
            let got = crate::multisig_rules::call_json("fee.row", &case["args"])
                .unwrap()
                .unwrap();
            let got: Value = serde_json::from_str(&got).unwrap();
            assert_eq!(got, case["expect"], "{}", case["name"]);
        }
    }

    #[test]
    fn the_door_knows_the_area() {
        let args = serde_json::json!({"base_units": "1", "chain_id": 1, "token": null, "symbol": null, "decimals": null});
        assert!(crate::multisig_rules::call_json("amount.of", &args).is_some_and(|r| r.is_ok()));
    }
}
