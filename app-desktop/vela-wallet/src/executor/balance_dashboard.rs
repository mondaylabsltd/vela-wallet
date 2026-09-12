//! The only place the `balance_dashboard` machine touches the outside world.
//!
//! Seven operations: the multi-chain fetch (delegated whole to
//! [`crate::executor::balances`]), the 24-hour total cache, a retry timer and
//! the privacy flag.
//!
//! ## The TTL is the shell's, the write gate is the core's
//!
//! The core's own words on `ReadBalanceCache`: "The shell applies the 24h TTL —
//! absent or expired answers `None`." And on `WriteBalanceCache`: "The CORE
//! decides when this may happen — the complete-results-only write gate."
//!
//! That split is worth respecting exactly. Caching a total assembled from a
//! partial fetch is how a wallet remembers a number that was never true; the
//! core refuses to ask for that write, and this file never writes uninvited.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::App;
use serde_json::{Value, json};

use vela_core::app::balance_dashboard::{
    AUTO_REFRESH_MS, BalanceCacheEntry, BalanceDashboard, BalanceOperation, BalanceShellResult,
    Event,
};

use crate::executor::{balances, storage};
use crate::resident::{self, Answer, Machine};
use crate::session;

/// `vela.balanceCache` — `{ address: { usd, at } }`, the Expo bytes.
const CACHE_KEY: &str = "vela.balanceCache";
/// 24 hours (`balance-cache.ts:11`).
const CACHE_TTL_MS: f64 = 24.0 * 60.0 * 60.0 * 1000.0;
/// `vela.balanceHidden` — `'1'` / `'0'`, best effort.
const PRIVACY_KEY: &str = "vela.balanceHidden";

/// A cached total, if one is present and still inside the TTL.
fn cached_usd(address: &str, now_ms: f64) -> Option<f64> {
    let entry = storage::read_value(CACHE_KEY).ok().flatten()?;
    let record = entry.get(address)?;
    let at = record.get("at").and_then(Value::as_f64)?;
    // Expired reads as absent, not as stale-but-usable: the hero would rather
    // show a skeleton than a figure from a different day.
    (now_ms - at <= CACHE_TTL_MS).then(|| record.get("usd").and_then(Value::as_f64))?
}

fn write_cached_usd(address: &str, usd: f64, now_ms: f64) {
    let mut map = match storage::read_value(CACHE_KEY) {
        Ok(Some(Value::Object(map))) => map,
        _ => serde_json::Map::new(),
    };
    map.insert(address.to_owned(), json!({ "usd": usd, "at": now_ms }));
    let _ = storage::write_value(CACHE_KEY, Value::Object(map));
}

impl Machine for BalanceDashboard {
    const LABEL: &'static str = "balance_dashboard";

    fn boot_event(cx: &App) -> Event {
        // The core resets its state and paints the hero from cache on this
        // event, so it is the boot: an address, or the empty string, which is
        // what "no account yet" looks like on the way from Welcome.
        Event::AccountChanged {
            address: session::view(cx).address,
        }
    }

    fn perform(operation: &BalanceOperation) -> Answer<BalanceShellResult, Self::Event> {
        match operation {
            BalanceOperation::FetchTokens {
                address,
                force,
                pull,
            } => {
                let (address, pull) = (address.clone(), *pull);
                // `force` bypasses a 5-minute shell TTL this cut does not keep:
                // every fetch is live. Recorded rather than silently ignored —
                // adding the TTL later changes no core rule, because the core
                // already tells us when it wants one bypassed.
                let _ = force;
                // Streaming, because this is the one operation the core says
                // streams: "while in flight the shell streams
                // `Event::ChainAssetsArrived` snapshots; the operation itself
                // settles exactly once". `FetchAccountAssets` below is the
                // same fan-out and deliberately does NOT stream — it fills a
                // switcher row for somebody else's account, and its snapshots
                // would be merged into the active one.
                Answer::Streaming(Box::new(move |sink| {
                    let sink = sink.clone();
                    let streamed_for = address.clone();
                    let arrived: std::sync::Arc<balances::ChainSink> =
                        std::sync::Arc::new(move |tokens| {
                            // The address rides along so the core can drop a
                            // stream that belongs to an account the person has
                            // already switched away from (its invariant ⑤).
                            sink.send(Event::ChainAssetsArrived {
                                address: streamed_for.clone(),
                                tokens,
                            });
                        });
                    let (tokens, failed) = balances::fetch_all_streaming(&address, &arrived);
                    BalanceShellResult::FetchSettled {
                        address,
                        pull,
                        tokens,
                        failed_chain_ids: failed,
                        // The pool's rate-limit classification is a separate
                        // question this cut does not yet ask it. Empty is the
                        // honest answer, and it degrades to "failed" rather
                        // than inventing a transient.
                        rate_limited_chain_ids: Vec::new(),
                        now_ms: crate::executor::now_ms(),
                    }
                }))
            }

            BalanceOperation::FetchAccountAssets { address } => {
                let address = address.clone();
                Answer::Blocking(Box::new(move || {
                    let (tokens, failed) = balances::fetch_all(&address);
                    BalanceShellResult::AccountAssetsFetched {
                        address,
                        // Best effort: a row that could not be read keeps its
                        // cached value rather than showing a wrong one.
                        tokens: failed.is_empty().then_some(tokens),
                    }
                }))
            }

            BalanceOperation::ReadBalanceCache { address } => {
                Answer::Now(BalanceShellResult::CachedTotalLoaded {
                    address: address.clone(),
                    usd: cached_usd(address, crate::executor::now_ms()),
                })
            }

            BalanceOperation::ReadBalanceCacheMany { addresses } => {
                let now = crate::executor::now_ms();
                Answer::Now(BalanceShellResult::CachedBalancesLoaded {
                    balances: addresses
                        .iter()
                        .filter_map(|address| {
                            cached_usd(address, now).map(|usd| BalanceCacheEntry {
                                address: address.clone(),
                                usd,
                            })
                        })
                        .collect(),
                })
            }

            BalanceOperation::WriteBalanceCache { address, usd } => {
                write_cached_usd(address, *usd, crate::executor::now_ms());
                Answer::Now(BalanceShellResult::BalanceCacheWritten)
            }

            BalanceOperation::StartRetryTimer { ms, timer_id } => Answer::After(
                Duration::from_millis(u64::from(*ms)),
                BalanceShellResult::RetryElapsed {
                    timer_id: *timer_id,
                },
            ),

            BalanceOperation::WritePrivacy { hidden } => {
                let _ = storage::write_value(
                    PRIVACY_KEY,
                    Value::String(if *hidden { "1" } else { "0" }.to_owned()),
                );
                Answer::Now(BalanceShellResult::PrivacyWritten)
            }
        }
    }
}

/// The person's stored answer to "hide my balance", read at boot.
///
/// `WritePrivacy` has been writing this file since spec 030 and **nothing has
/// ever read it back**: the flag survived a restart on disk and not on screen.
/// First-write-wins is the core's (its invariant ⑧), which is why this is an
/// event rather than a field set before boot.
fn hydrate_privacy() -> Option<Event> {
    let stored = storage::read_value(PRIVACY_KEY).ok().flatten()?;
    let hidden = match &stored {
        Value::String(text) => text == "1",
        Value::Bool(flag) => *flag,
        _ => return None,
    };
    Some(Event::PrivacyHydrated { hidden })
}

static TICKING: AtomicBool = AtomicBool::new(false);
/// Set when something invalidated what the hero is showing, from a place with
/// no `cx` to say so directly (a `perform` runs without one). Drained by the
/// page on its next frame — which is the same frame the change caused.
static INVALIDATED: AtomicBool = AtomicBool::new(false);

/// "What you are showing is out of date." Cheap, idempotent, and safe to call
/// from an operation.
pub fn invalidate() {
    INVALIDATED.store(true, Ordering::SeqCst);
}

/// Drain the flag. `true` means somebody should force a read.
pub fn take_invalidation() -> bool {
    INVALIDATED.swap(false, Ordering::SeqCst)
}

/// Boot the hero and keep it honest for the life of the process.
///
/// Until this existed the desktop dispatched exactly one balance event ever —
/// `AccountChanged` at boot — so **the total on screen was the total at launch**.
/// Money could arrive, a send could settle, and the figure would not move until
/// the app was restarted. The core names the two cadences it does not own
/// (`AUTO_REFRESH_MS`, and `AppFocused` when the window comes back) and both are
/// wired here.
///
/// `force: false` is deliberate: the core drops an unforced refresh while the
/// window is in the background, which is exactly what an interval poll should
/// do and what the web's own timer relies on.
pub fn start_ticks(cx: &mut App) {
    let _ = resident::resident::<BalanceDashboard>(cx);
    if let Some(event) = hydrate_privacy() {
        resident::resident::<BalanceDashboard>(cx)
            .update(cx, |resident, cx| resident.dispatch(event, cx));
    }
    if TICKING.swap(true, Ordering::SeqCst) {
        return;
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a ten-minute constant in f64 ms is exactly representable"
    )]
    let period = Duration::from_millis(AUTO_REFRESH_MS as u64);
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(period).await;
            // Re-fetched each tick: a sign-out drops every resident, and the
            // next sign-in's hero is the one that must get the ticks.
            let balance = cx.update(|cx| resident::resident::<BalanceDashboard>(cx));
            balance.update(cx, |resident, cx| {
                resident.dispatch(
                    Event::RefreshRequested {
                        force: false,
                        pull: false,
                    },
                    cx,
                );
            });
        }
    })
    .detach();
}

/// One event into the hero, from a screen that made something stale.
pub fn dispatch(event: Event, cx: &mut App) {
    resident::resident::<BalanceDashboard>(cx).update(cx, |resident, cx| {
        resident.dispatch(event, cx);
    });
}

/// A forced read, for the moments a screen KNOWS the answer changed — a token
/// added, a network added, an RPC repaired. The web forces on exactly these,
/// and forcing matters: those all happen while somebody is looking.
pub fn refresh(cx: &mut App) {
    dispatch(
        Event::RefreshRequested {
            force: true,
            pull: false,
        },
        cx,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perform(operation: BalanceOperation) -> BalanceShellResult {
        match BalanceDashboard::perform(&operation) {
            Answer::Now(result) => result,
            _ => unreachable!("this operation is local"),
        }
    }

    /// The privacy flag survives a restart on SCREEN, not only on disk.
    ///
    /// `WritePrivacy` has written this file since spec 030 and nothing read it
    /// back, so hiding your balance lasted exactly as long as the process. Both
    /// spellings are accepted because both clients have written it: the Expo
    /// app's `'1'`/`'0'` strings, and a boolean is what a JSON store would hold
    /// if one ever wrote it that way.
    #[test]
    fn the_stored_privacy_choice_is_read_back_at_boot() {
        storage::tests::with_temp_state("balance-privacy-hydrate", || {
            assert!(hydrate_privacy().is_none(), "nothing stored, nothing said");

            if storage::write_value(PRIVACY_KEY, Value::String("1".to_owned())).is_err() {
                unreachable!("could not seed");
            }
            assert!(matches!(
                hydrate_privacy(),
                Some(Event::PrivacyHydrated { hidden: true })
            ));

            if storage::write_value(PRIVACY_KEY, Value::String("0".to_owned())).is_err() {
                unreachable!("could not seed");
            }
            assert!(matches!(
                hydrate_privacy(),
                Some(Event::PrivacyHydrated { hidden: false })
            ));

            if storage::write_value(PRIVACY_KEY, Value::Bool(true)).is_err() {
                unreachable!("could not seed");
            }
            assert!(matches!(
                hydrate_privacy(),
                Some(Event::PrivacyHydrated { hidden: true })
            ));
        });
    }

    /// Hiding the balance outlives the process.
    ///
    /// Two halves that were both present and never met: the core has always
    /// asked for `WritePrivacy`, and nothing read the file back. This drives
    /// the real machine through the tap, performs the write it asks for, and
    /// then reads it the way a fresh launch does.
    ///
    /// Only the privacy operation is performed — `AccountChanged` also asks for
    /// a twelve-chain fetch, and a unit test has no business making one.
    #[test]
    fn hiding_the_balance_survives_a_restart() {
        use crate::core_host::CoreHost;

        storage::tests::with_temp_state("balance-privacy-roundtrip", || {
            let mut host = CoreHost::<BalanceDashboard>::new();
            let _ = host.dispatch(Event::AccountChanged {
                address: "0xabc".to_owned(),
            });

            let toggle = |host: &mut CoreHost<BalanceDashboard>| {
                for next in host.dispatch(Event::PrivacyToggled) {
                    if matches!(next.operation, BalanceOperation::WritePrivacy { .. }) {
                        let result = perform(next.operation.clone());
                        let _ = host.resolve(next.id, result);
                    }
                }
            };

            toggle(&mut host);
            assert!(host.view().hidden, "the tap hid it");
            assert!(
                matches!(
                    hydrate_privacy(),
                    Some(Event::PrivacyHydrated { hidden: true })
                ),
                "and the next launch would start hidden"
            );

            toggle(&mut host);
            assert!(!host.view().hidden, "the second tap shows it again");
            assert!(matches!(
                hydrate_privacy(),
                Some(Event::PrivacyHydrated { hidden: false })
            ));
        });
    }

    /// Invalidation is a one-shot: the frame that drains it forces one read,
    /// and the frame after it does not force another.
    #[test]
    fn an_invalidation_is_drained_once() {
        assert!(!take_invalidation(), "nothing to drain");
        invalidate();
        invalidate();
        assert!(take_invalidation(), "one drain reports it");
        assert!(!take_invalidation(), "and the next frame does not re-read");
    }

    /// A total written today comes back; one written two days ago does not.
    ///
    /// Expiry reads as **absent**, not as a stale figure: the hero would rather
    /// show a skeleton than yesterday's number presented as today's.
    #[test]
    fn a_cached_total_expires_rather_than_going_stale() {
        storage::tests::with_temp_state("balance-cache-ttl", || {
            const ADDR: &str = "0xabc";
            let now = 1_756_000_000_000.0;

            assert_eq!(cached_usd(ADDR, now), None, "nothing cached yet");

            write_cached_usd(ADDR, 42.5, now);
            assert_eq!(cached_usd(ADDR, now), Some(42.5));
            assert_eq!(
                cached_usd(ADDR, now + CACHE_TTL_MS - 1.0),
                Some(42.5),
                "still inside the window"
            );
            assert_eq!(
                cached_usd(ADDR, now + CACHE_TTL_MS + 1.0),
                None,
                "expired must read as absent, not as stale"
            );
        });
    }

    /// The stored shape is the one every other client reads.
    #[test]
    fn the_cache_uses_the_shared_key_and_shape() {
        storage::tests::with_temp_state("balance-cache-shape", || {
            write_cached_usd("0xabc", 12.25, 1_756_000_000_000.0);
            let raw = storage::read_value(CACHE_KEY)
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("nothing written"));
            let record = raw
                .get("0xabc")
                .unwrap_or_else(|| unreachable!("the address is the key"));
            assert_eq!(record.get("usd").and_then(Value::as_f64), Some(12.25));
            assert!(record.get("at").is_some(), "`at` is what the TTL reads");
        });
    }

    /// The switcher only ever offers rows it can vouch for.
    #[test]
    fn only_unexpired_rows_reach_the_switcher() {
        storage::tests::with_temp_state("balance-cache-many", || {
            // The REAL clock, because the operation reads it: `ReadBalanceCache*`
            // takes `now` from the shell, which is the right design and means a
            // test seeding a fixed past timestamp writes two expired rows and
            // proves nothing. (It cost me one confusing red.)
            let now = crate::executor::now_ms();
            write_cached_usd("0xfresh", 1.0, now);
            write_cached_usd("0xstale", 2.0, now - CACHE_TTL_MS - 1.0);

            let result = perform(BalanceOperation::ReadBalanceCacheMany {
                addresses: vec![
                    "0xfresh".to_owned(),
                    "0xstale".to_owned(),
                    "0xnone".to_owned(),
                ],
            });
            match result {
                BalanceShellResult::CachedBalancesLoaded { balances } => {
                    assert_eq!(balances.len(), 1, "only the fresh row may be offered");
                    assert_eq!(balances[0].address, "0xfresh");
                }
                other => unreachable!("wrong variant: {other:?}"),
            }
        });
    }

    /// Privacy persists as the '1'/'0' string the other clients wrote.
    #[test]
    fn privacy_persists_as_the_shared_flag() {
        storage::tests::with_temp_state("balance-privacy", || {
            perform(BalanceOperation::WritePrivacy { hidden: true });
            assert_eq!(
                storage::read_value(PRIVACY_KEY)
                    .ok()
                    .flatten()
                    .as_ref()
                    .and_then(Value::as_str),
                Some("1")
            );
            perform(BalanceOperation::WritePrivacy { hidden: false });
            assert_eq!(
                storage::read_value(PRIVACY_KEY)
                    .ok()
                    .flatten()
                    .as_ref()
                    .and_then(Value::as_str),
                Some("0")
            );
        });
    }
}
