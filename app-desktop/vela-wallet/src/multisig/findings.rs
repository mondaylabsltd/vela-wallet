//! What the core found about who can act for a multisig (§3.5), in the
//! person's words (the web's `ui/findings.ts`): one line per reason or
//! risk, each naming the address and, where it is the chain's, the network.

use serde_json::Value;

use super::words;
use crate::loc::Loc;

fn network(chain_id: u64) -> String {
    crate::executor::custom_tokens::network_name(u32::try_from(chain_id).unwrap_or_default())
}

/// One finding (`{from: 'genesis', chainId, reason}` or `{from: 'chain',
/// chainId, risk}`), as its line reads; `fallback` names the network when
/// the finding does not.
#[must_use]
pub fn finding_words(loc: &Loc, finding: &Value, fallback: &str) -> String {
    let net = finding
        .get("chainId")
        .and_then(Value::as_u64)
        .map_or_else(|| fallback.to_owned(), network);
    let text = |v: &Value, k: &str| {
        v.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    match finding.get("from").and_then(Value::as_str) {
        Some("chain") => {
            let risk = finding.get("risk").cloned().unwrap_or(Value::Null);
            let key = match risk.get("kind").and_then(Value::as_str) {
                Some("module") => "authority.riskModule",
                Some("guard") => "authority.riskGuard",
                Some("handler") => "authority.riskHandler",
                _ => "authority.riskSingleton",
            };
            words::tv(
                loc,
                key,
                &[("address", &text(&risk, "address")), ("network", &net)],
            )
        }
        _ => {
            let reason = finding.get("reason").cloned().unwrap_or(Value::Null);
            match reason.get("kind").and_then(Value::as_str) {
                Some("payment") => words::tv(
                    loc,
                    "authority.payment",
                    &[("address", &text(&reason, "receiver"))],
                ),
                Some(kind @ ("wrongAddress" | "notSetup" | "badMembers")) => {
                    words::t(loc, &format!("authority.{kind}"))
                }
                Some(kind) => words::tv(
                    loc,
                    &format!("authority.{kind}"),
                    &[("address", &text(&reason, "address")), ("network", &net)],
                ),
                None => words::t(loc, "authority.refusedBody"),
            }
        }
    }
}

/// Every finding's line, each once.
#[must_use]
pub fn finding_lines(loc: &Loc, findings: &[Value], fallback: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for f in findings {
        let line = finding_words(loc, f, fallback);
        if !out.contains(&line) {
            out.push(line);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_chain_risk_names_its_address_and_network() {
        let loc = Loc::for_tag("en");
        let line = finding_words(
            &loc,
            &json!({"from": "chain", "chainId": 100, "risk": {"kind": "module", "address": "0xee"}}),
            "",
        );
        assert!(line.contains("0xee"), "{line}");
        assert!(line.contains("module"), "{line}");
        assert!(!line.contains("home.multisig"), "{line}");
    }

    #[test]
    fn a_genesis_refusal_is_said_once() {
        let loc = Loc::for_tag("en");
        let f = json!({"from": "genesis", "chainId": 100, "reason": {"kind": "badMembers"}});
        let lines = finding_lines(&loc, &[f.clone(), f], "");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("members"), "{}", lines[0]);
    }
}
