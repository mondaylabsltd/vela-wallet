//! The approval guard's reads — spec 022's never-unlimited gate, fed.
//!
//! Three questions, all `eth_call`, all about ONE thing: what is the person
//! actually agreeing to let a contract move?
//!
//! - **metadata** — a symbol and decimals per token. Without decimals an
//!   allowance renders at the wrong magnitude, which on an approval screen is
//!   not a cosmetic error: `1000000` reads as a thousand tokens or as one,
//!   depending on a number this call is the only source of.
//! - **allowance** — what the spender may already move. `increaseAllowance`
//!   ADDS, so the resulting total is existing + increment; showing only the
//!   increment understates the agreement.
//! - **balance** — the one-tap finite cap (issue #86). A person told "never
//!   unlimited" needs a number to put there, and their own balance is the one
//!   they can reason about.
//!
//! Every one of them may fail, and the core distinguishes the failures rather
//! than flattening them: `None` metadata is "the whole read failed" while a
//! token merely missing from the list "was not resolvable", and they produce
//! different fallbacks. A failed allowance still warns that the increment adds
//! to something rather than hiding the fact.

use crate::executor::{abi, pool};
use crate::resident::{Answer, Machine};

use vela_core::app::approval_guard::{
    ApprovalGuard, Event, GuardOperation, GuardShellResult, GuardTokenMetaEntry,
};

impl Machine for ApprovalGuard {
    const LABEL: &'static str = "approval_guard";

    /// No operation, and provably so: a pristine model has no editor, and
    /// `set_bool_pick` returns `done()` for anything that is not
    /// `Editor::Boolean`. The guard starts working on its first
    /// `ApprovalDetected`, which the sheet dispatches — the same shape
    /// `fee_policy` uses for the same reason (it is owned by a screen, not by
    /// the process).
    fn boot_event(_cx: &gpui::App) -> Event {
        Event::RevokeChosen
    }

    fn perform(operation: &GuardOperation) -> Answer<GuardShellResult, Self::Event> {
        match operation {
            GuardOperation::ReadTokenMetadata { chain_id, tokens } => {
                let (chain_id, tokens) = (*chain_id, tokens.clone());
                Answer::Blocking(Box::new(move || GuardShellResult::MetaResolved {
                    metas: token_metas(chain_id, &tokens),
                }))
            }

            GuardOperation::ReadErc20Allowance {
                chain_id,
                token,
                owner,
                spender,
            } => {
                let (chain_id, token) = (*chain_id, token.clone());
                let data = abi::enc_allowance(owner, spender);
                Answer::Blocking(Box::new(move || GuardShellResult::AllowanceRead {
                    allowance: eth_call(chain_id, &token, &data)
                        .and_then(|hex| abi::dec_u256(&hex)),
                }))
            }

            GuardOperation::ReadErc20Balance {
                chain_id,
                token,
                owner,
            } => {
                let (chain_id, token) = (*chain_id, token.clone());
                let data = abi::enc_balance_of(owner);
                Answer::Blocking(Box::new(move || GuardShellResult::BalanceRead {
                    balance: eth_call(chain_id, &token, &data).and_then(|hex| abi::dec_u256(&hex)),
                }))
            }
        }
    }
}

/// One Multicall3 round trip for every token's symbol and decimals.
///
/// `None` means the batch itself failed — the pool could not be reached at
/// all. A token that answered nothing is simply left OUT of the list, which is
/// the other verdict the core keys different fallbacks off.
fn token_metas(chain_id: u32, tokens: &[String]) -> Option<Vec<GuardTokenMetaEntry>> {
    if tokens.is_empty() {
        return Some(Vec::new());
    }
    let mut calls = Vec::with_capacity(tokens.len() * 2);
    for token in tokens {
        calls.push(abi::Call3 {
            target: token.clone(),
            call_data: abi::enc_symbol(),
        });
        calls.push(abi::Call3 {
            target: token.clone(),
            call_data: abi::enc_decimals(),
        });
    }
    let raw = eth_call(chain_id, abi::MULTICALL3, &abi::enc_aggregate3(&calls))?;
    let results = abi::dec_aggregate3(&raw);
    if results.len() != calls.len() {
        // A batch that came back the wrong shape is not a batch that answered
        // "no token resolved" — it is one nobody can read, and the core's two
        // fallbacks differ.
        return None;
    }
    let metas = tokens
        .iter()
        .enumerate()
        .filter_map(|(i, token)| {
            let symbol = results.get(i * 2).and_then(|r| abi::dec_string(&r.data))?;
            let decimals = results.get(i * 2 + 1).and_then(|r| abi::dec_u8(&r.data))?;
            Some(GuardTokenMetaEntry {
                token: token.to_lowercase(),
                symbol,
                decimals: u32::from(decimals),
            })
        })
        .collect();
    Some(metas)
}

/// One routed `eth_call`, unwrapped.
///
/// `pool::call` answers the whole JSON-RPC ENVELOPE, not the result — reading
/// the envelope as a string silently answers `None` for a call that worked
/// perfectly, which is how the first version of this file turned three good
/// reads into "the batch failed".
fn eth_call(chain_id: u32, to: &str, data: &str) -> Option<String> {
    pool::call(
        chain_id,
        "eth_call",
        serde_json::json!([{ "to": to, "data": data }, "latest"]),
    )
    .ok()?
    .get("result")
    .and_then(serde_json::Value::as_str)
    .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing to read is not a failed read.
    ///
    /// The core keys two different fallbacks off `None` versus an empty list,
    /// so an empty request must answer `Some(vec![])` — and it must do it
    /// without a round trip, because a Multicall3 with no calls is a request
    /// for nothing.
    #[test]
    fn no_tokens_answers_an_empty_list_rather_than_a_failure() {
        assert_eq!(token_metas(100, &[]), Some(Vec::new()));
    }

    /// A real approval's three reads, against Gnosis.
    ///
    /// USDC on Gnosis with the golden Safe as owner: the metadata says which
    /// magnitude an allowance is in, and the balance is the number the
    /// one-tap finite cap offers instead of "unlimited". Ignored by default —
    /// it reads the chain.
    #[test]
    #[ignore = "reads a real chain"]
    fn live_a_real_token_answers_its_symbol_decimals_and_balance() {
        crate::executor::storage::tests::with_temp_state("guard-live", live_reads);
    }

    fn live_reads() {
        const GNOSIS: u32 = 100;
        const USDC: &str = "0x2a22f9c3b484c3629090FeED35F17Ff8F88f76F0";
        const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

        let metas = token_metas(GNOSIS, &[USDC.to_owned()])
            .unwrap_or_else(|| unreachable!("the batch answered"));
        let meta = metas
            .first()
            .unwrap_or_else(|| unreachable!("USDC resolved: {metas:?}"));
        assert_eq!(meta.token, USDC.to_lowercase(), "the map key is lowercased");
        assert!(!meta.symbol.is_empty(), "a symbol: {meta:?}");
        assert_eq!(meta.decimals, 6, "USDC is six decimals: {meta:?}");

        // A balance the finite cap can offer. Zero is a fine answer; `None`
        // would mean the read failed and no preset may be shown.
        let balance = eth_call(GNOSIS, USDC, &abi::enc_balance_of(GOLDEN))
            .and_then(|hex| abi::dec_u256(&hex));
        assert!(balance.is_some(), "the balance read answered");
    }
}
