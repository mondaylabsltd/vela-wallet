//! The only place the `manage_tokens` machine touches the outside world.
//!
//! Five operations: an ERC-20 metadata read, the custom-token ledger, and a
//! cache invalidation.
//!
//! ## Three calls, not a multicall — for now
//!
//! The operation is named `MulticallErc20Meta` because that is what the web does
//! (one `aggregate3` to Multicall3). This asks `symbol()`, `name()` and
//! `decimals()` separately: three round trips instead of one, no Multicall3
//! encoding, and the same answer. The name is the core's word for *what it
//! wants*, not an instruction about how — and folding them into one call later
//! changes nothing it sees.
//!
//! What is NOT deferred is the failure rule. Metadata is all-or-nothing: a token
//! with a symbol and no decimals renders an amount at the wrong magnitude, which
//! is worse than refusing to add it. `None` unless all three answered.

use gpui::App;
use serde_json::{Value, json};

use vela_core::app::manage_tokens::{
    Event, ManageTokens, MtokOperation, MtokShellResult, MtokTokenMeta,
};

use crate::executor::{abi, custom_tokens, pool};
use crate::resident::{Answer, Machine};

/// One `eth_call`, returning the raw hex result.
fn eth_call(chain_id: u32, to: &str, data: &str) -> Option<String> {
    pool::call(
        chain_id,
        "eth_call",
        json!([{ "to": to, "data": data }, "latest"]),
    )
    .ok()?
    .get("result")
    .and_then(Value::as_str)
    .map(str::to_owned)
}

impl Machine for ManageTokens {
    const LABEL: &'static str = "manage_tokens";

    fn boot_event(_cx: &App) -> Event {
        Event::Start
    }

    fn perform(operation: &MtokOperation) -> Answer<MtokShellResult, Self::Event> {
        match operation {
            MtokOperation::MulticallErc20Meta { chain_id, address } => {
                let (chain_id, address) = (*chain_id, address.clone());
                Answer::Blocking(Box::new(move || {
                    let symbol = eth_call(chain_id, &address, &abi::enc_symbol())
                        .and_then(|r| abi::dec_string(&r));
                    let name = eth_call(chain_id, &address, &abi::enc_name())
                        .and_then(|r| abi::dec_string(&r));
                    let decimals = eth_call(chain_id, &address, &abi::enc_decimals())
                        .and_then(|r| abi::dec_u8(&r));

                    // All three, or nothing. A token with a symbol and no
                    // decimals renders an amount at the wrong magnitude, and
                    // once saved it is wrong for as long as it is in the list.
                    let meta = match (symbol, name, decimals) {
                        (Some(symbol), Some(name), Some(decimals)) => Some(MtokTokenMeta {
                            symbol,
                            name,
                            decimals,
                        }),
                        _ => None,
                    };
                    MtokShellResult::ChainMetaResolved {
                        chain_id,
                        address,
                        meta,
                    }
                }))
            }

            MtokOperation::ReadCustomTokens => Answer::Now(MtokShellResult::CustomTokensLoaded {
                tokens: custom_tokens::read()
                    .iter()
                    .filter_map(custom_tokens::StoredToken::to_mtok)
                    .collect(),
            }),

            MtokOperation::WriteCustomToken { token } => Answer::Now(
                // Replace by id rather than append: adding the same token twice
                // is a person correcting themselves, not two tokens.
                if custom_tokens::save(custom_tokens::StoredToken::from(token)) {
                    MtokShellResult::Saved
                } else {
                    MtokShellResult::SaveFailed
                },
            ),

            MtokOperation::RemoveCustomToken { id } => {
                let id = id.clone();
                Answer::Now(if custom_tokens::remove(&id) {
                    MtokShellResult::Removed { id }
                } else {
                    MtokShellResult::RemoveFailed { id }
                })
            }

            // The balance fetch reads the token list on every run, so there is
            // no separate token cache to drop on the desktop. Answered, because
            // a skipped operation leaves the core waiting.
            //
            // What the desktop DOES need is the other half of the web's
            // handler: the same moment calls `balance.refresh(true)` there,
            // because a token that was just added is a token nobody has counted
            // yet. The core times this operation exactly right — it asks for
            // the invalidation once the write has landed — so the flag is set
            // here rather than at the click.
            MtokOperation::InvalidateTokenCache => {
                crate::executor::balance_dashboard::invalidate();
                Answer::Now(MtokShellResult::CacheInvalidated)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::storage;
    use vela_core::app::manage_tokens::MtokCustomToken;

    fn token(chain_id: u32, address: &str, symbol: &str) -> MtokCustomToken {
        MtokCustomToken {
            id: format!("{chain_id}_{address}"),
            chain_id,
            contract_address: address.to_owned(),
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            decimals: 6,
            network_name: "Gnosis".to_owned(),
        }
    }

    /// Adding the same token twice is a correction, not a duplicate.
    #[test]
    fn saving_the_same_token_twice_replaces_it() {
        storage::tests::with_temp_state("mtok-replace", || {
            let save =
                |t: MtokCustomToken| match ManageTokens::perform(&MtokOperation::WriteCustomToken {
                    token: t,
                }) {
                    Answer::Now(result) => result,
                    _ => unreachable!("local"),
                };
            assert_eq!(save(token(100, "0xabc", "USDC")), MtokShellResult::Saved);
            assert_eq!(save(token(100, "0xabc", "USDC.e")), MtokShellResult::Saved);

            let tokens = custom_tokens::read();
            assert_eq!(tokens.len(), 1, "the same id must not appear twice");
            assert_eq!(tokens[0].symbol, "USDC.e", "the newer record wins");
        });
    }

    /// Removing something that is not there is a FAILURE — the row is still on
    /// the screen and the core has to know.
    #[test]
    fn removing_a_missing_token_reports_failure() {
        storage::tests::with_temp_state("mtok-remove", || {
            match ManageTokens::perform(&MtokOperation::RemoveCustomToken {
                id: "nope".to_owned(),
            }) {
                Answer::Now(MtokShellResult::RemoveFailed { id }) => assert_eq!(id, "nope"),
                _ => unreachable!("removing nothing must report failure"),
            }
        });
    }

    /// Metadata is all-or-nothing, live: a real ERC-20 answers all three.
    #[test]
    #[ignore = "reads a real ERC-20 on Gnosis"]
    fn a_real_token_resolves_its_metadata() {
        // USDC on Gnosis.
        const USDC: &str = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83";
        let Answer::Blocking(work) = ManageTokens::perform(&MtokOperation::MulticallErc20Meta {
            chain_id: 100,
            address: USDC.to_owned(),
        }) else {
            unreachable!("metadata is network work");
        };
        match work() {
            MtokShellResult::ChainMetaResolved { meta, .. } => {
                let meta = meta.unwrap_or_else(|| unreachable!("the token did not answer"));
                println!(
                    "  {} / {} / {} decimals",
                    meta.symbol, meta.name, meta.decimals
                );
                assert_eq!(meta.decimals, 6, "USDC has six decimals");
                assert!(!meta.symbol.is_empty());
            }
            other => unreachable!("wrong variant: {other:?}"),
        }
    }
}
