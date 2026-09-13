//! The only place the `fee_policy` machine touches the outside world.
//!
//! Six operations, six reads. Every rule that used to live in `GasFeeCard`,
//! `useSendController` and `estimateTransactionFee` — the bundler-quote
//! acceptance, the gas-price fallback, the ×1.5 padding, the 1 KiB calldata
//! cliff, the in-band pricing, the balance<fee gate — is `fee_policy.rs`'s.
//! If this file ever grows an `if` that decides what a fee IS, that decision
//! belongs in the machine.
//!
//! The readers stop at the wire (`raw_gas_signals`, `raw_bundler_quote`)
//! rather than reusing `chain_gas_price`: that one already applies the rules
//! the core also holds, and feeding the core its output would apply each rule
//! twice with neither side owning it.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/flows/core/fee-executor.ts`
//! @ `origin/main`.

use std::time::Duration;

use gpui::App;

use vela_core::app::fee_policy::{Event, FeeOperation, FeePolicy, FeeShellResult};

use crate::executor::{chain, relay, storage, user_op};
use crate::resident::{Answer, Machine};

impl Machine for FeePolicy {
    const LABEL: &'static str = "fee_policy";

    /// No operation: a pristine model ignores an expiry. The session starts
    /// working on its first `QuoteRequested`, which the surface dispatches.
    fn boot_event(_cx: &App) -> Event {
        Event::QuoteExpired
    }

    fn perform(operation: &FeeOperation) -> Answer<FeeShellResult, Self::Event> {
        match operation {
            FeeOperation::FetchGasPrice { chain_id, want_tip } => {
                let (chain_id, want_tip) = (*chain_id, *want_tip);
                Answer::Blocking(Box::new(move || {
                    let signals = chain::raw_gas_signals(chain_id, want_tip);
                    FeeShellResult::GasPrice {
                        eth_gas_price: signals.eth_gas_price,
                        base_fee: signals.base_fee,
                        priority_fee: signals.priority_fee,
                    }
                }))
            }

            FeeOperation::FetchBundlerQuote { chain_id, tier } => {
                let (chain_id, tier) = (*chain_id, *tier);
                Answer::Blocking(Box::new(move || FeeShellResult::BundlerQuote {
                    quote: relay::raw_bundler_quote(chain_id, tier),
                }))
            }

            FeeOperation::FetchInBandQuotes { chain_id, account } => {
                let (chain_id, account) = (*chain_id, account.clone());
                Answer::Blocking(Box::new(move || FeeShellResult::InBandQuotes {
                    // Every row the relay published, UNFILTERED — which rows
                    // the picker shows is the core's rule.
                    quotes: relay::in_band_quotes(chain_id, &account),
                }))
            }

            FeeOperation::FetchFeeRecipient { chain_id, account } => {
                let (chain_id, account) = (*chain_id, account.clone());
                Answer::Blocking(Box::new(move || FeeShellResult::FeeRecipient {
                    recipient: relay::account_info(chain_id, &account)
                        .and_then(|info| info.fee_recipient()),
                }))
            }

            FeeOperation::EstimateUserOpGas {
                chain_id,
                account,
                deployed,
                calls,
            } => {
                let (chain_id, account, deployed, calls) =
                    (*chain_id, account.clone(), *deployed, calls.clone());
                Answer::Blocking(Box::new(move || {
                    // A multi-key wallet's initCode derives from ALL founding
                    // keys; the stored account is the one source of them.
                    let keys = stored_key_hexes(&account);
                    FeeShellResult::UserOpGas {
                        outcome: user_op::simulate_gas(chain_id, &account, deployed, &calls, &keys),
                    }
                }))
            }

            FeeOperation::StartTtl { ms } => Answer::After(
                Duration::from_millis(u64::from(*ms)),
                FeeShellResult::TtlElapsed,
            ),
        }
    }
}

/// The founding keys of the stored account at `address`, or none — an
/// undeployed account with no keys cannot build its initCode, and the core
/// reads the empty answer as `ContextUnavailable`.
pub fn stored_key_hexes(address: &str) -> Vec<String> {
    storage::load_accounts()
        .unwrap_or_default()
        .iter()
        .find(|account| account.address.eq_ignore_ascii_case(address))
        .map(|account| user_op::key_hexes(&user_op::key_set_of(account)))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::fee_policy::FeeTier;

    /// Every network operation is a blocking answer, and the timer is a
    /// timer — nothing here runs on the frame.
    #[test]
    fn network_reads_are_blocking_and_the_ttl_is_a_timer() {
        let ops = [
            FeeOperation::FetchGasPrice {
                chain_id: 100,
                want_tip: true,
            },
            FeeOperation::FetchBundlerQuote {
                chain_id: 100,
                tier: FeeTier::Fast,
            },
            FeeOperation::FetchInBandQuotes {
                chain_id: 100,
                account: "0x0".to_owned(),
            },
            FeeOperation::FetchFeeRecipient {
                chain_id: 100,
                account: "0x0".to_owned(),
            },
            FeeOperation::EstimateUserOpGas {
                chain_id: 100,
                account: "0x0".to_owned(),
                deployed: true,
                calls: Vec::new(),
            },
        ];
        for op in &ops {
            assert!(
                matches!(FeePolicy::perform(op), Answer::Blocking(_)),
                "{op:?}"
            );
        }
        match FeePolicy::perform(&FeeOperation::StartTtl { ms: 30_000 }) {
            Answer::After(delay, FeeShellResult::TtlElapsed) => {
                assert_eq!(delay, Duration::from_secs(30));
            }
            _ => unreachable!("the TTL is a timer"),
        }
    }

    /// An account nobody stored has no keys — and the estimate of an
    /// undeployed one says so rather than guessing an initCode.
    #[test]
    fn an_unknown_account_has_no_keys() {
        crate::executor::storage::tests::with_temp_state("fee-keys", || {
            assert!(stored_key_hexes("0x0000000000000000000000000000000000000009").is_empty());
        });
    }
}
