//! The only place the `receive_watch` machine touches the outside world.
//!
//! Three operations, and the machine behind them is the one that notices money
//! arriving while somebody is looking at their QR code.
//!
//! The shell's whole job is: fetch a snapshot, wait, and buzz. Which snapshot
//! counts as "a deposit landed" — the baseline, the comparison, the debounce —
//! is the core's, and it is why this file is short.

use std::time::Duration;

use gpui::App;

use vela_core::app::receive_watch::{
    Event, ReceiveWatch, ReceiveWatchOperation, ReceiveWatchShellResult, TokenSnapshot,
};

use crate::executor::balances;
use crate::resident::{Answer, Machine};

impl Machine for ReceiveWatch {
    const LABEL: &'static str = "receive_watch";

    fn boot_event(_cx: &App) -> Event {
        Event::Start
    }

    fn perform(operation: &ReceiveWatchOperation) -> Answer<ReceiveWatchShellResult, Self::Event> {
        match operation {
            ReceiveWatchOperation::FetchTokens => Answer::Blocking(Box::new(|| {
                // The watcher polls; a failure is a beat missed, not an error to
                // show. The core keeps its baseline and tries again.
                let address = current_address();
                if address.is_empty() {
                    return ReceiveWatchShellResult::Inactive;
                }
                let (tokens, failed) = balances::fetch_all(&address);
                if tokens.is_empty() && !failed.is_empty() {
                    return ReceiveWatchShellResult::FetchFailed {
                        now_ms: crate::executor::now_ms(),
                    };
                }
                ReceiveWatchShellResult::TokensFetched {
                    tokens: tokens.iter().map(snapshot).collect(),
                    now_ms: crate::executor::now_ms(),
                }
            })),

            ReceiveWatchOperation::Wait { ms } => Answer::After(
                Duration::from_millis(u64::from(*ms)),
                ReceiveWatchShellResult::Waited {
                    now_ms: crate::executor::now_ms(),
                },
            ),

            // The buzz. A desktop has no haptics, so the signal is the visual
            // one the screen already draws — answered, never skipped.
            ReceiveWatchOperation::SignalDeposit => Answer::Now(ReceiveWatchShellResult::Signalled),
        }
    }
}

/// The watcher follows the signed-in account, and there is no session on the
/// gpui side of a background fetch — so it reads the same store the session
/// machine does.
fn current_address() -> String {
    let accounts = crate::executor::storage::load_accounts().unwrap_or_default();
    let index = crate::executor::storage::load_active_index();
    accounts
        .get(index)
        .map(|account| account.address.clone())
        .unwrap_or_default()
}

/// A balance row as the watcher's baseline sees it.
///
/// The balance becomes an `f64` here, and that is safe in a way it would not be
/// for money: this number is only ever COMPARED against an earlier snapshot of
/// itself to answer "did it go up". It is never rendered and never summed.
fn snapshot(token: &vela_core::app::balance_dashboard::BalanceToken) -> TokenSnapshot {
    #[allow(clippy::cast_precision_loss, reason = "compared, never displayed")]
    let balance = token.balance.parse::<u128>().unwrap_or(0) as f64
        / 10_f64.powi(i32::try_from(token.decimals).unwrap_or(18));
    TokenSnapshot {
        id: token.token_address.clone().map_or_else(
            || format!("{}_native", token.chain_id),
            |address| format!("{}_{address}", token.chain_id),
        ),
        symbol: token.symbol.clone(),
        chain_id: token.chain_id,
        balance,
        price_usd: token.price_usd,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::balance_dashboard::BalanceToken;

    fn token(chain_id: u32, balance: &str, decimals: u32) -> BalanceToken {
        BalanceToken {
            chain_id,
            symbol: "xDAI".to_owned(),
            name: "xDAI".to_owned(),
            balance: balance.to_owned(),
            decimals,
            token_address: None,
            price_usd: None,
            spam: false,
        }
    }

    /// A native row gets a stable per-chain id, so two chains' native coins are
    /// never the same baseline entry.
    #[test]
    fn a_native_snapshot_is_keyed_by_its_chain() {
        assert_eq!(snapshot(&token(100, "0", 18)).id, "100_native");
        assert_ne!(
            snapshot(&token(1, "0", 18)).id,
            snapshot(&token(100, "0", 18)).id
        );
    }

    /// The raw quantity is scaled, because the baseline compares magnitudes.
    #[test]
    fn the_balance_is_scaled_for_comparison() {
        let snap = snapshot(&token(100, "1500000000000000000", 18));
        assert!(
            (snap.balance - 1.5).abs() < 1e-9,
            "expected 1.5, got {}",
            snap.balance
        );
    }
}
