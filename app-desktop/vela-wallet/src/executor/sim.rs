//! What a transaction would DO, asked of the chain itself.
//!
//! `eth_simulateV1` runs the inner calls in a simulated block against live
//! state through this person's own RPC pool — no third-party "simulation"
//! service. What the answer means — the asset deltas, a revert and its
//! reason, or a node that could not check — is the core's `sim_outcome`
//! (spec 082 RG6); this file only asks.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/services/sim/sim-engine-rpc.ts`
//! (`rpcSimulate`).
//!
//! ## Why this is the only part of a signing sheet a site cannot author
//!
//! Everything else on that sheet starts as something the dApp said: the
//! calldata, the intent, the token it names. This starts as what the CHAIN
//! says would happen if it ran. That is why the deeper degradation rungs
//! promote it from a footnote to the protagonist — and why it is worth having
//! even when it can only answer sometimes.
//!
//! ## What it must never do
//!
//! Decide. The deltas are untrusted input by construction (the core's
//! invariants ⑤/⑥): a site can emit any `Transfer` log it likes from a
//! contract it controls. Which of them may be shown with a confident amount is
//! `token_trust`'s judgment, and this module hands them over without an
//! opinion. It also must never reach a WRITE — the founder's own ruling, from
//! the token auto-add work: admission comes from confirm-time receipt logs,
//! never from a sign-time simulation.

use serde_json::json;

use vela_core::app::fee_policy::FeeCall;
use vela_core::app::sim_outcome::{self, SimOutcome, SimReply};

/// Simulate `calls` as `from`, and what the answer means (spec 082 RG6,
/// L-D5): the pool's reply normalised into the core's [`SimReply`] and judged
/// by `sim_outcome::classify` — the deltas when every call ran, a revert with
/// its sanitised reason, "this node does not simulate", or "no node
/// answered". The desktop's own log parser is gone; every client now reads a
/// simulation by one rule.
///
/// A request with nothing to simulate (no call, or a call with no `to` — a
/// deployment) is a question that could not be asked: `NotOffered`, which
/// draws "couldn't check", never "nothing moves".
#[must_use]
pub fn simulate(from: &str, calls: &[FeeCall], chain_id: u32) -> SimOutcome {
    let Some(first) = calls.first() else {
        return SimOutcome::NotOffered;
    };
    if first.to.is_empty() {
        return SimOutcome::NotOffered;
    }
    let payload = json!({
        "blockStateCalls": [{
            "calls": calls.iter().map(|call| {
                let mut entry = json!({
                    "from": from,
                    "to": call.to,
                    "value": hex_value(&call.value),
                });
                if !call.data.is_empty() && call.data != "0x" {
                    entry["data"] = json!(call.data);
                }
                entry
            }).collect::<Vec<_>>(),
        }],
        // "What would happen", not "may this account afford it": in the 4337
        // model the Safe pays no gas, so requiring funds and a nonce would
        // refuse simulations that are going to succeed.
        "validation": false,
        "traceTransfers": true,
        "returnFullTransactions": false,
    });

    // The pool hands back the WHOLE JSON-RPC envelope — an answer, or the
    // node's error (an optional method's error is an answer, RG7) — and
    // nothing at all when no endpoint answered.
    let reply =
        match crate::executor::pool::call(chain_id, "eth_simulateV1", json!([payload, "latest"])) {
            Ok(envelope) => SimReply::from_envelope(&envelope),
            Err(_) => SimReply::Unreachable,
        };
    sim_outcome::classify(reply, from)
}

/// `value` as the hex quantity `eth_simulateV1` wants. The wire carries wei as
/// a DECIMAL string; sending it unconverted is how a 1 becomes a 0x1 that is
/// really one wei and a 10 becomes sixteen.
fn hex_value(decimal: &str) -> String {
    let trimmed = decimal.trim();
    if trimmed.is_empty() {
        return "0x0".to_owned();
    }
    trimmed
        .parse::<u128>()
        .map_or_else(|_| "0x0".to_owned(), |value| format!("{value:#x}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use vela_core::app::clear_signing::ClearRisk;
    use vela_core::app::token_trust::TrustDeltaKind;

    const ME: &str = "0x88cca0eedbf2c4426110bbfc998f048689266894";

    /// The reply as the pool hands it over, judged the way `simulate` does.
    fn judged(answer: Option<Value>) -> SimOutcome {
        let reply = answer.map_or(SimReply::Unreachable, |envelope| {
            SimReply::from_envelope(&envelope)
        });
        sim_outcome::classify(reply, ME)
    }

    /// Spec 082 L-D5: Arbitrum's public node answers `eth_simulateV1` with
    /// -32603 "method handler crashed". That is a node that cannot check —
    /// the caution, never the danger that says the transaction will fail.
    #[test]
    fn a_node_that_cannot_simulate_is_a_caution() {
        let outcome = judged(Some(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32603, "message": "method handler crashed"},
        })));
        assert_eq!(outcome, SimOutcome::NotOffered);
        let notice = sim_outcome::notice(&outcome).unwrap_or_else(|| unreachable!("a notice"));
        assert_eq!(notice.risk, ClearRisk::Caution);
        assert_eq!(notice.key, sim_outcome::KEY_UNAVAILABLE);
        // No node at all: the same caution.
        let unreachable = judged(None);
        assert_eq!(unreachable, SimOutcome::Unreachable);
        assert_eq!(
            sim_outcome::notice(&unreachable).map(|n| n.risk),
            Some(ClearRisk::Caution)
        );
    }

    /// A call the chain says fails (status 0x0) is a danger, with the
    /// sanitised `Error(string)` reason — and its logs are not netted.
    #[test]
    fn a_revert_is_a_danger_with_its_reason() {
        // Error(string) "ERC20: transfer amount exceeds balance".
        let reason = "ERC20: transfer amount exceeds balance";
        let mut data = String::from("0x08c379a0");
        data.push_str(&format!("{:064x}", 32));
        data.push_str(&format!("{:064x}", reason.len()));
        let mut bytes = reason.as_bytes().to_vec();
        bytes.resize(bytes.len().div_ceil(32) * 32, 0);
        data.push_str(&vela_core::primitives::to_hex(&bytes, false));
        let outcome = judged(Some(json!({
            "result": [{"calls": [{"status": "0x0", "returnData": data, "logs": []}]}],
        })));
        assert_eq!(
            outcome,
            SimOutcome::Reverts {
                reason: Some(reason.to_owned())
            }
        );
        let notice = sim_outcome::notice(&outcome).unwrap_or_else(|| unreachable!("a notice"));
        assert_eq!(notice.risk, ClearRisk::Danger);
        assert_eq!(notice.key, sim_outcome::KEY_WILL_FAIL_REASON);
        assert_eq!(notice.reason.as_deref(), Some(reason));
    }

    /// Every call ran: the deltas, from the core's one parser.
    #[test]
    fn a_run_simulation_is_the_cores_deltas() {
        let sentinel = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
        let topic = |address: &str| format!("0x{:0>64}", address.trim_start_matches("0x"));
        let outcome = judged(Some(json!({
            "result": [{"calls": [{"status": "0x1", "logs": [{
                "address": sentinel,
                "topics": [
                    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                    topic(ME),
                    topic("0x000000000000000000000000000000000000dead"),
                ],
                "data": format!("0x{:064x}", 1_000_000_000_000_000_u128),
            }]}]}],
        })));
        let SimOutcome::Deltas { deltas } = outcome else {
            unreachable!("deltas: {outcome:?}");
        };
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].kind, TrustDeltaKind::Native);
        assert_eq!(deltas[0].delta, "-1000000000000000");
        assert!(sim_outcome::notice(&SimOutcome::Deltas { deltas }).is_none());
    }

    /// Nothing to simulate is a question that could not be asked.
    #[test]
    fn nothing_to_simulate_could_not_be_checked() {
        assert_eq!(simulate(ME, &[], 100), SimOutcome::NotOffered);
        let deploy = FeeCall {
            to: String::new(),
            value: "0".to_owned(),
            data: "0x60".to_owned(),
        };
        assert_eq!(simulate(ME, &[deploy], 100), SimOutcome::NotOffered);
    }

    /// No copy of the delta parser is left on the desktop: the core's
    /// `derive_deltas` is the only one (spec 082 T057).
    #[test]
    fn the_delta_parser_lives_in_the_core_only() {
        let here = include_str!("sim.rs");
        let needle = ["fn derive", "_deltas"].concat();
        assert!(
            !here.contains(&needle),
            "a desktop copy of the parser is back"
        );
    }

    /// The real thing, against Gnosis: what would a 0.001 xDAI transfer from
    /// the golden Safe move?
    ///
    /// `#[ignore]` because it needs the network — the same rule the balance
    /// tests follow. Run it with
    /// `cargo test executor::sim::tests::live -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads a real chain"]
    fn live_a_native_transfer_simulates_to_its_own_delta() {
        crate::executor::storage::tests::with_temp_state("sim-live", || {
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let calls = vec![FeeCall {
                to: "0x000000000000000000000000000000000000dEaD".to_owned(),
                value: "1000000000000000".to_owned(),
                data: "0x".to_owned(),
            }];
            match simulate(GOLDEN, &calls, 100) {
                SimOutcome::Deltas { deltas } => {
                    println!("deltas: {deltas:?}");
                    let native = deltas
                        .iter()
                        .find(|delta| delta.kind == TrustDeltaKind::Native)
                        .unwrap_or_else(|| unreachable!("the coin moved"));
                    assert_eq!(native.delta, "-1000000000000000");
                }
                other => println!("the endpoint could not simulate: {other:?}"),
            }
        });
    }

    /// Wei crosses the wire as a decimal string and `eth_simulateV1` wants a
    /// hex quantity. Sending it unconverted turns 10 wei into sixteen.
    #[test]
    fn the_value_is_converted_to_a_hex_quantity() {
        assert_eq!(hex_value("0"), "0x0");
        assert_eq!(hex_value("10"), "0xa");
        assert_eq!(hex_value("1000000000000000000"), "0xde0b6b3a7640000");
        assert_eq!(hex_value(""), "0x0");
        assert_eq!(hex_value("not a number"), "0x0");
    }
}
