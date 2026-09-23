//! The wallet home's display models, built from what the cores decided.
//!
//! The sibling of `fixtures.rs`, never its replacement — the third of these
//! (settings, contacts, wallet) and the one where the rules bite hardest,
//! because every value here is somebody's money.

use gpui::SharedString;

use vela_core::app::activity_feed::{FeedDirection, FeedItem, FeedRow, FeedTxKind, FeedView};
use vela_core::app::balance_dashboard::{BalanceNotice, BalanceToken, BalanceView};
use vela_core::l10n::currency::format_fiat;
use vela_core::l10n::number::format_token_amount;

use crate::wallet::WalletStrings;
use crate::wallet::fixtures::{
    ActivityKind, ActivityRowModel, AssetDetailModel, AssetRowModel, BALANCE_MASK, BalanceModel,
    BalanceState, ChainRowModel, Fiat, StatusKind,
};

/// The currency every fiat figure on these screens is drawn in.
///
/// Until 2026-09-23 the desktop drew money in `"USD"`/`"$"` verbatim, so a
/// person who picked ZAR saw ZAR on one settings row and dollars everywhere
/// else. The rule here is the web's (`wallet/live.ts::moneyParts`), and its
/// point is the case it refuses: **a code with no rate is drawn as USD, not as
/// that code at rate 1.** A rate of 1 is a claim — "1 USD = 1 CNY" — and the
/// core answers `rate: None` rather than 1 for exactly this reason.
#[derive(Clone, Debug, PartialEq)]
pub struct Money {
    code: String,
    rate: Option<f64>,
}

impl Default for Money {
    /// What a signed-out screen and every fixture board draw in.
    fn default() -> Self {
        Self {
            code: "USD".to_owned(),
            rate: Some(1.0),
        }
    }
}

impl Money {
    /// The committed pair, straight from `display_currency`.
    #[must_use]
    pub fn new(code: &str, rate: Option<f64>) -> Self {
        Self {
            code: code.to_owned(),
            rate,
        }
    }

    /// A USD figure, in this currency — or in USD when it cannot be converted.
    #[must_use]
    pub fn text(&self, usd: f64, locale: &str) -> String {
        let (code, amount) = match self.rate {
            Some(rate) => (self.code.as_str(), usd * rate),
            None => ("USD", usd),
        };
        format_fiat(
            amount,
            code,
            crate::settings::live::symbol_for(code),
            locale,
            crate::executor::format_prefs::fiat_options(),
        )
    }
}

/// The balance hero.
///
/// Four states, and the two that look like edge cases are the ones the core
/// went to trouble over:
///
/// - **hidden** — the fiat value is withheld *by construction*, not masked
///   downstream. The core's invariant ⑧: a leak in one surface defeats the mask
///   everywhere, so `display_total_usd` is already `None` and there is nothing
///   here to accidentally print.
/// - **unknown** — `None` renders a skeleton, **never a fake `$0`** (invariant
///   ②). A wallet that shows zero while it is still counting has told the person
///   their money is gone.
#[must_use]
pub fn balance(view: &BalanceView, s: &WalletStrings, locale: &str, money: &Money) -> BalanceModel {
    // The last-known total paints first while the core withholds the live
    // one; live replaces it (max(live, cached) is the core's rule — this only
    // chooses what to show meanwhile). The web's `liveBalance`.
    let on_cache = view.display_total_usd.is_none() && view.cached_total_usd.is_some();
    let status = if view.unreachable {
        // A first launch with no network: say so, over the skeleton, rather
        // than show a settled-looking zero (spec 038 finding 15).
        Some((StatusKind::Warning, s.balance_unreachable.clone()))
    } else if view.refreshing || on_cache {
        // A cached figure is a figure being brought up to date — said so, so
        // yesterday's total never reads as today's.
        Some((StatusKind::Refreshing, s.balance_stale.clone()))
    } else {
        view.notice.map(|notice| {
            (
                StatusKind::Warning,
                match notice {
                    // Partial and still retrying — the figure is real but not
                    // final, which is a different thing from wrong.
                    BalanceNotice::StillUpdating => s.balance_stale.clone(),
                    BalanceNotice::Unpriced => s.balance_unpriced.clone(),
                },
            )
        })
    };

    if view.hidden {
        return BalanceModel {
            label: s.total_balance.clone(),
            state: BalanceState::Hidden,
            integer: SharedString::from(BALANCE_MASK),
            decimals: None,
            live: None,
            status,
        };
    }

    let Some(usd) = view.display_total_usd.or(view.cached_total_usd) else {
        return BalanceModel {
            label: s.total_balance.clone(),
            state: BalanceState::Loading,
            // Not "$0". The core withholds the number until it has one, and the
            // shell must not fill the gap with a figure that reads as an answer.
            integer: SharedString::from(""),
            decimals: None,
            live: None,
            status,
        };
    };

    let (integer, decimals) = split_fiat(usd, locale, money);
    BalanceModel {
        label: s.total_balance.clone(),
        // A zero is "live" only once EVERY chain has answered: a partial zero
        // (some chain unreachable), or a cached one, is an unknown wallet,
        // not a listening one.
        state: if usd == 0.0
            && !view.balance_unknown
            && !view.balance_partial
            && view.tokens.is_empty()
        {
            BalanceState::ZeroLive
        } else {
            BalanceState::Normal
        },
        integer,
        decimals,
        live: None,
        status,
    }
}

/// `$1,383.28` → `("$1,383", Some("28"))`.
///
/// The hero draws the minor units smaller than the number — the design
/// language's "subordinated symbols" rule — so the split is a render concern and
/// belongs here rather than in a formatter. Splitting on the LAST `.` is what
/// keeps a locale whose group separator is `.` from being cut in half.
fn split_fiat(usd: f64, locale: &str, money: &Money) -> (SharedString, Option<SharedString>) {
    let formatted = money.text(usd, locale);
    match formatted.rsplit_once('.') {
        Some((whole, minor)) if minor.chars().all(|c| c.is_ascii_digit()) => (
            SharedString::from(whole.to_owned()),
            Some(SharedString::from(minor.to_owned())),
        ),
        // No minor units — a large balance drops them by product rule
        // (`drop_minor_units_above`), and a currency with zero fraction digits
        // never had them.
        _ => (SharedString::from(formatted), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_host::CoreHost;
    use vela_core::app::balance_dashboard::{BalanceDashboard, Event as BalanceEvent};

    /// A real `BalanceView` with the total substituted.
    ///
    /// Taken from a booted core rather than hand-written: this view has a dozen
    /// fields with their own invariants, and a literal I typed would be a guess
    /// about them that drifts the first time one changes. (It already did —
    /// two of my guessed field names did not exist.)
    fn view(usd: Option<f64>) -> BalanceView {
        let mut host = CoreHost::<BalanceDashboard>::new();
        let _ = host.dispatch(BalanceEvent::AccountChanged {
            address: "0xabc".to_owned(),
        });
        BalanceView {
            display_total_usd: usd,
            balance_unknown: usd.is_none(),
            ..host.view()
        }
    }

    /// Drive the real machine to a settled view, performing every operation
    /// it asks for on this thread.
    ///
    /// `Answer::After` is resolved immediately rather than slept: the only
    /// timer this machine sets is the partial-fetch retry, and a test that
    /// honoured its backoff would spend minutes proving nothing.
    fn settle(address: &str) -> BalanceView {
        settle_reporting(address).1
    }

    /// The same drive, keeping what the fetch said ON THE WAY: the view as it
    /// stood after each per-chain report. Every one of those is a state the
    /// hero could have drawn before the settle.
    ///
    /// Not just the first: an address holds money on the chains it holds money
    /// on, and eleven empty reports before the one that matters is the normal
    /// case, not a failure.
    fn settle_reporting(address: &str) -> (Vec<BalanceView>, BalanceView) {
        use crate::resident::{Answer, Machine};

        let mut per_report: Vec<BalanceView> = Vec::new();
        let mut host = CoreHost::<BalanceDashboard>::new();
        let mut pending = host.dispatch(BalanceEvent::AccountChanged {
            address: address.to_owned(),
        });
        // A cap, not a timeout: a machine that kept asking would otherwise hang
        // the suite, and 64 operations is far past what one settle needs.
        for _ in 0..64 {
            let Some(next) = pending.pop() else {
                break;
            };
            let result = match BalanceDashboard::perform(&next.operation) {
                Answer::Now(result) | Answer::After(_, result) => result,
                Answer::Blocking(work) => work(),
                // The reports a streaming operation makes on its way. Dispatched
                // BEFORE its result, which is the order the async pump
                // guarantees and the order `balance_dashboard` depends on.
                Answer::Streaming(work) => {
                    let (streamed, result) = crate::resident::run_streaming(work);
                    for report in streamed {
                        pending.extend(host.dispatch(report));
                        per_report.push(host.view());
                    }
                    result
                }
            };
            pending.extend(host.resolve(next.id, result));
        }
        (per_report, host.view())
    }

    fn strings() -> WalletStrings {
        WalletStrings::resolve(&crate::loc::Loc::from_env())
    }

    use vela_core::app::activity_feed::{
        ActivityFeed, Event as FeedEvent, FeedItem, FeedRow, FeedView,
    };

    fn feed_with(
        rows: Vec<FeedRow>,
        transactions: Vec<vela_core::app::activity_feed::FeedTxRecord>,
    ) -> FeedView {
        let mut host = CoreHost::<ActivityFeed>::new();
        let _ = host.dispatch(FeedEvent::AccountSwitched {
            address: "0xme".to_owned(),
        });
        FeedView {
            rows,
            transactions,
            ..host.view()
        }
    }

    /// A celebration, produced the way the machine produces one: a first pass
    /// that spends the backlog gate, then a tick whose scan found something.
    ///
    /// Hand-writing a `FeedView` with a toast in it would prove only that this
    /// module can read a struct I filled in. What has to be true is that the
    /// toast a REAL sequence arms says what the row says, and the sequence is
    /// four steps long for reasons the core is emphatic about (the first pass
    /// never celebrates; only the read the sync named may).
    fn celebrated(value: &str, symbol: &str) -> CoreHost<ActivityFeed> {
        use vela_core::app::activity_feed::{FeedOperation, FeedShellResult, FeedTxRecord};

        const ME: &str = "0xme";
        let record = FeedTxRecord {
            id: "r1".to_owned(),
            user_op_hash: String::new(),
            tx_hash: "0xdead".to_owned(),
            from: "0xAbCdEf0000000000000000000000000000000001".to_owned(),
            to: ME.to_owned(),
            to_name: None,
            value: value.to_owned(),
            symbol: symbol.to_owned(),
            decimals: 6,
            logo_urls: None,
            chain_id: 100,
            timestamp: 1_756_000_000.0,
            day_start_ms: 0.0,
            status: vela_core::app::activity_feed::FeedTxStatus::Confirmed,
            kind: Some(vela_core::app::activity_feed::FeedTxKind::Receive),
            usd: None,
        };

        let mut host = CoreHost::<ActivityFeed>::new();
        let mut pending = host.dispatch(FeedEvent::AccountSwitched {
            address: ME.to_owned(),
        });
        // Two passes: the first spends the backlog gate (nothing new), the
        // second finds one receipt and is therefore allowed to celebrate.
        for (pass, new_count) in [(0, 0u32), (1, 1u32)] {
            if pass == 1 {
                pending.extend(host.dispatch(FeedEvent::FocusTick));
            }
            for _ in 0..16 {
                let Some(next) = pending.pop() else {
                    break;
                };
                let result = match &next.operation {
                    FeedOperation::ReadTxStore { read_id, .. } => FeedShellResult::StoreLoaded {
                        records: if pass == 0 {
                            Vec::new()
                        } else {
                            vec![record.clone()]
                        },
                        now_ms: 1_756_000_000_000.0,
                        read_id: *read_id,
                    },
                    FeedOperation::ScanIncomingTransfers { .. } => {
                        FeedShellResult::SyncCompleted { new_count }
                    }
                    FeedOperation::ResolveRecipientIdentity { addr } => {
                        FeedShellResult::AliasResolved {
                            addr: addr.clone(),
                            name: None,
                        }
                    }
                    // The countdown is NOT run: answering it here would expire
                    // the toast inside the fixture that exists to look at it.
                    FeedOperation::Timer { .. } => continue,
                    FeedOperation::DeleteTxRecord { id } => {
                        FeedShellResult::DeleteCommitted { id: id.clone() }
                    }
                    FeedOperation::Haptic => FeedShellResult::HapticPlayed,
                };
                pending.extend(host.resolve(next.id, result));
            }
        }
        host
    }

    /// The celebration says what landed, in the corpus's sentence, with the
    /// same number formatter the row underneath it uses.
    #[test]
    fn a_landed_receipt_is_celebrated_with_its_own_amount() {
        let host = celebrated("120", "USDT");
        let view = host.view();
        let s = strings();

        let toast = receipt_toast(&view, &s).unwrap_or_else(|| unreachable!("a toast was armed"));
        assert!(toast.contains("120"), "the amount: {toast}");
        assert!(toast.contains("USDT"), "the coin: {toast}");
        assert!(!toast.contains("{{"), "an unfilled template: {toast}");

        // And the row it is about is the one the glow names, so the two
        // surfaces cannot celebrate different transactions.
        assert_eq!(view.new_item_id.as_deref(), Some("r1"));
        assert_eq!(
            history_item_ids(&view).first().map(String::as_str),
            Some("r1")
        );
    }

    /// Privacy is the one case where the shell must draw NOTHING — and the
    /// core is what withholds it, so this is really a test that the shell
    /// asked. A masked hero beside a pill reading "120 USDT received" would
    /// undo the mask on the surface that spells the number out in full.
    #[test]
    fn a_masked_hero_gets_no_celebration() {
        let mut host = celebrated("120", "USDT");
        let _ = host.dispatch(FeedEvent::PrivacyChanged { hidden: true });
        let view = host.view();
        assert!(receipt_toast(&view, &strings()).is_none());
        // The glow survives it: the core withholds the NUMBER, not the news
        // that something landed.
        assert_eq!(view.new_item_id.as_deref(), Some("r1"));
    }

    /// An amount that will not parse is no celebration at all — never a pill
    /// with a hole where the money goes.
    #[test]
    fn an_unreadable_amount_is_not_celebrated() {
        let host = celebrated("not-a-number", "USDT");
        assert!(host.view().toast.is_some(), "the core armed one");
        assert!(receipt_toast(&host.view(), &strings()).is_none());
    }

    fn item(id: &str, incoming: bool, value: Option<&str>, symbol: &str) -> FeedItem {
        FeedItem {
            id: id.to_owned(),
            direction: if incoming {
                FeedDirection::In
            } else {
                FeedDirection::Out
            },
            counterparty: Some("0xAbCdEf0000000000000000000000000000000001".to_owned()),
            alias: None,
            value: value.map(str::to_owned),
            symbol: symbol.to_owned(),
            decimals: Some(18),
            usd_value: 0.0,
            chain_id: 100,
            timestamp: 1_756_000_000.0,
            day_start_ms: 0.0,
            tx_hash: None,
            batch: None,
        }
    }

    /// Day headers are dropped for the home preview — the mocks draw a flat
    /// short list there — and the items keep the core's order.
    #[test]
    fn headers_are_dropped_and_items_keep_their_order() {
        let view = feed_with(
            vec![
                FeedRow::Header {
                    id: "day-0".to_owned(),
                    day_start_ms: 0.0,
                    timestamp: 1_756_000_000.0,
                },
                FeedRow::Item {
                    item: item("a", false, Some("2"), "POL"),
                },
                FeedRow::Item {
                    item: item("b", true, Some("120"), "USDT"),
                },
            ],
            Vec::new(),
        );
        let rows = activity_rows(&view, &strings(), false);
        assert_eq!(rows.len(), 2, "the header is not a row here");
        assert_eq!(rows[0].unit, SharedString::from("POL"));
        assert_eq!(rows[1].unit, SharedString::from("USDT"));
    }

    /// The sign is the direction's, and the minus is U+2212 — a hyphen does
    /// not align under a digit, which is why the mocks use the real one.
    #[test]
    fn direction_drives_the_sign() {
        let view = feed_with(
            vec![
                FeedRow::Item {
                    item: item("a", false, Some("2"), "POL"),
                },
                FeedRow::Item {
                    item: item("b", true, Some("120"), "USDT"),
                },
            ],
            Vec::new(),
        );
        let rows = activity_rows(&view, &strings(), false);
        assert_eq!(rows[0].amount, SharedString::from("\u{2212}2"));
        assert!(!rows[0].positive);
        assert_eq!(rows[1].amount, SharedString::from("+120"));
        assert!(rows[1].positive);
    }

    /// `value` is the human amount already. Scaling it by `decimals` would
    /// print every figure 10^18 times too large — and it would look deliberate.
    #[test]
    fn the_amount_is_not_scaled_by_decimals() {
        let view = feed_with(
            vec![FeedRow::Item {
                item: item("a", true, Some("1.5"), "xDAI"),
            }],
            Vec::new(),
        );
        let rows = activity_rows(&view, &strings(), false);
        assert_eq!(rows[0].amount, SharedString::from("+1.5"));
    }

    /// Privacy masks the FIGURE and keeps the unit — H5's rule — and it comes
    /// from the balance view so every money surface masks together.
    #[test]
    fn hiding_masks_the_figure_and_keeps_the_unit() {
        let view = feed_with(
            vec![FeedRow::Item {
                item: item("a", true, Some("120"), "USDT"),
            }],
            Vec::new(),
        );
        let rows = activity_rows(&view, &strings(), true);
        assert_eq!(rows[0].unit, SharedString::from("USDT"), "the unit stays");
        assert!(
            !rows[0].amount.contains("120"),
            "the figure must not survive the mask: {:?}",
            rows[0].amount
        );
    }

    /// A batch with mixed tokens has no sum, and the core says so by sending no
    /// value. Inventing one would be arithmetic nobody asked for.
    #[test]
    fn a_batch_without_a_sum_prints_no_figure() {
        let view = feed_with(
            vec![FeedRow::Item {
                item: item("a", false, None, ""),
            }],
            Vec::new(),
        );
        assert_eq!(
            activity_rows(&view, &strings(), false)[0].amount,
            SharedString::from("")
        );
    }

    /// The figure the mock draws, from a real total.
    #[test]
    fn a_known_total_renders_split_for_the_hero() {
        let model = balance(&view(Some(1383.28)), &strings(), "en", &Money::default());
        assert_eq!(model.state, BalanceState::Normal);
        assert_eq!(model.integer, SharedString::from("$1,383"));
        assert_eq!(model.decimals, Some(SharedString::from("28")));
    }

    /// Invariant ②. A wallet that shows $0 while it is still counting has told
    /// somebody their money is gone.
    #[test]
    fn an_unknown_total_is_a_skeleton_and_never_a_zero() {
        let model = balance(&view(None), &strings(), "en", &Money::default());
        assert_eq!(model.state, BalanceState::Loading);
        assert!(
            !model.integer.contains('0'),
            "a skeleton must not print a figure: {:?}",
            model.integer
        );
        assert_eq!(model.decimals, None);
    }

    /// A real zero is a different fact from an unknown one, and gets its own
    /// state — the mocks draw them differently on purpose.
    #[test]
    fn a_real_zero_is_not_the_same_as_unknown() {
        let model = balance(&view(Some(0.0)), &strings(), "en", &Money::default());
        assert_eq!(model.state, BalanceState::ZeroLive);
        assert_eq!(model.integer, SharedString::from("$0"));
    }

    /// The last-known total paints first — with the refreshing line, so it
    /// never reads as today's figure — and live replaces it (the web's
    /// `liveBalance`). A skeleton only when there is nothing known at all.
    #[test]
    fn a_cached_total_paints_first_and_says_it_is_refreshing() {
        let mut cached = view(None);
        cached.cached_total_usd = Some(42.5);
        let model = balance(&cached, &strings(), "en", &Money::default());
        assert_eq!(model.state, BalanceState::Normal);
        assert_eq!(model.integer, SharedString::from("$42"));
        assert_eq!(model.decimals, Some(SharedString::from("50")));
        assert!(
            matches!(model.status, Some((StatusKind::Refreshing, _))),
            "a cached figure must say it is being brought up to date"
        );

        // Live wins over the cache the moment it is there.
        let mut live = view(Some(40.0));
        live.cached_total_usd = Some(42.5);
        let model = balance(&live, &strings(), "en", &Money::default());
        assert_eq!(model.integer, SharedString::from("$40"));
        assert!(model.status.is_none(), "{:?}", model.status.map(|s| s.1));
    }

    /// A zero is "live" only once every chain answered: a partial zero, or a
    /// cached one, is an unknown wallet — not a listening one.
    #[test]
    fn a_partial_or_cached_zero_is_not_live() {
        let mut partial = view(Some(0.0));
        partial.balance_partial = true;
        assert_eq!(
            balance(&partial, &strings(), "en", &Money::default()).state,
            BalanceState::Normal
        );

        let mut cached = view(None);
        cached.cached_total_usd = Some(0.0);
        assert_eq!(
            balance(&cached, &strings(), "en", &Money::default()).state,
            BalanceState::Normal
        );
    }

    /// Invariant ⑧: the value is withheld by construction. The core already
    /// nulls `display_total_usd` when hidden, so there is nothing here that
    /// could leak — this asserts the shell does not reintroduce it.
    #[test]
    fn hiding_masks_and_carries_no_figure() {
        let mut hidden = view(None);
        hidden.hidden = true;
        let model = balance(&hidden, &strings(), "en", &Money::default());
        assert_eq!(model.state, BalanceState::Hidden);
        assert_eq!(model.integer, SharedString::from(BALANCE_MASK));
        assert_eq!(model.decimals, None);
    }

    /// A locale whose group separator is `.` must not be cut in half.
    /// The rule the whole type exists for: a currency nobody could price is
    /// drawn as USD, **never** as that currency at rate 1. "A rate of 1 is a
    /// claim (1 USD = 1 CNY)", and the core answers `None` rather than 1 for
    /// exactly this reason — a shell that quietly filled in 1 would put a ¥
    /// in front of a dollar figure.
    #[test]
    fn an_unpriceable_currency_is_drawn_in_dollars() {
        let priced = Money::new("EUR", Some(0.92));
        let text = priced.text(1000.0, "en-US");
        assert!(text.contains("920"), "converted: {text}");
        assert!(!text.contains('$'), "and wearing its own symbol: {text}");

        let unpriced = Money::new("EUR", None);
        let text = unpriced.text(1000.0, "en-US");
        assert!(text.contains('$'), "USD, symbol and all: {text}");
        assert!(text.contains("1,000"), "the figure is untouched: {text}");

        // Zero-decimal currencies keep the core's own rule about minor units.
        let yen = Money::new("JPY", Some(157.0));
        let text = yen.text(10.0, "en-US");
        assert!(!text.contains('.'), "a yen figure has no cents: {text}");

        assert_eq!(
            Money::default().text(1.5, "en-US"),
            Money::new("USD", Some(1.0)).text(1.5, "en-US"),
            "the default IS dollars"
        );
    }

    #[test]
    fn splitting_survives_a_dot_grouped_locale() {
        let (integer, decimals) = split_fiat(1383.28, "de", &Money::default());
        assert!(
            integer.contains("1.383") || integer.contains("1,383"),
            "the whole part lost its grouping: {integer}"
        );
        assert!(
            decimals.as_ref().is_none_or(|d| d.len() <= 2),
            "the split took too much: {decimals:?}"
        );
    }

    /// The asset detail is about the holding that was clicked, and it says
    /// "no price" rather than "$0.00" when nobody could price it.
    #[test]
    fn the_asset_detail_is_about_the_row_that_was_opened() {
        crate::executor::storage::tests::with_temp_state("asset-detail", || {
            use vela_core::app::activity_feed::{
                ActivityFeed, Event as FeedEvent, FeedDirection, FeedItem,
            };
            use vela_core::app::balance_dashboard::BalanceToken;

            let mut held = view(Some(100.0));
            held.tokens = vec![
                BalanceToken {
                    chain_id: 100,
                    symbol: "xDAI".to_owned(),
                    name: "xDai".to_owned(),
                    balance: "0.75897".to_owned(),
                    decimals: 18,
                    token_address: None,
                    price_usd: Some(1.0),
                    spam: false,
                },
                BalanceToken {
                    chain_id: 143,
                    symbol: "MON".to_owned(),
                    name: "Monad".to_owned(),
                    balance: "12".to_owned(),
                    decimals: 18,
                    token_address: Some("0xAbCdEf0000000000000000000000000000000009".to_owned()),
                    price_usd: None,
                    spam: false,
                },
            ];

            let mut host = CoreHost::<ActivityFeed>::new();
            let _ = host.dispatch(FeedEvent::AccountSwitched {
                address: "0xme".to_owned(),
            });
            let feed = FeedView {
                rows: vec![FeedRow::Item {
                    item: FeedItem {
                        id: "a".to_owned(),
                        direction: FeedDirection::In,
                        counterparty: None,
                        alias: None,
                        value: Some("1.5".to_owned()),
                        symbol: "xDAI".to_owned(),
                        decimals: Some(18),
                        usd_value: 1.5,
                        chain_id: 100,
                        timestamp: 1_788_500_000.0,
                        day_start_ms: 0.0,
                        tx_hash: None,
                        batch: None,
                    },
                }],
                ..host.view()
            };
            let s = strings();

            // The SECOND row, because that is the one clicked.
            let mon = asset_detail(&held, &feed, 1, &s, "en-US", &Money::default())
                .unwrap_or_else(|| unreachable!("row 1 exists"));
            assert_eq!(mon.ticker, "MON");
            assert_eq!(mon.amount, "12 MON");
            // Unpriced: the chain, never "$0.00 · Monad".
            assert!(mon.sub.contains("Monad"));
            assert!(!mon.sub.contains('$'), "an unpriced holding is not $0.00");
            // An ERC-20 names its contract; the price row is absent because
            // there is no price to state.
            assert!(
                mon.facts
                    .iter()
                    .any(|(label, _)| *label == s.label_contract)
            );
            assert!(!mon.facts.iter().any(|(label, _)| *label == s.label_price));
            // xDAI's transaction is not MON's.
            assert!(mon.activity.is_empty());

            let xdai = asset_detail(&held, &feed, 0, &s, "en-US", &Money::default())
                .unwrap_or_else(|| unreachable!("row 0 exists"));
            assert_eq!(xdai.ticker, "xDAI");
            assert!(xdai.sub.starts_with("$0.76"));
            // A native coin has no contract, and says so in words.
            assert!(
                xdai.facts
                    .iter()
                    .any(|(label, value)| *label == s.label_contract && *value == s.native_token)
            );
            assert_eq!(xdai.activity.len(), 1, "its own transaction");
            // …and the id that row opens, from the same walk.
            assert_eq!(xdai.activity_ids, vec!["a".to_owned()]);
            assert!(mon.activity_ids.is_empty());

            // View on explorer: the token page scoped to this account for an
            // ERC-20, the account page for the chain's own coin.
            let account = held.address.clone().unwrap_or_default();
            assert!(!account.is_empty());
            assert_eq!(
                mon.explorer_url.as_deref(),
                Some(
                    format!(
                        "https://monadscan.com/token/0xAbCdEf0000000000000000000000000000000009?a={account}"
                    )
                    .as_str()
                )
            );
            assert_eq!(
                xdai.explorer_url.as_deref(),
                Some(format!("https://gnosisscan.io/address/{account}").as_str())
            );

            // 转账 from a token opens the form with THAT token chosen: its
            // symbol, and its network spelled as the send executor spells
            // `SendToken.network` — any other spelling lands on the picker.
            let params = token_send_params(&held.tokens[1]);
            assert_eq!(params.preselected_symbol.as_deref(), Some("MON"));
            assert_eq!(
                params.preselected_network,
                Some(crate::executor::send::to_send_token(&held.tokens[1]).network)
            );
            assert!(params.prefilled_recipient.is_none() && !params.locked);

            // The list moved underneath: no panel rather than the wrong one.
            assert!(asset_detail(&held, &feed, 9, &s, "en-US", &Money::default()).is_none());
        });
    }

    /// The home strip lists the core's holdings, in the core's order, and says
    /// nothing about a chain nobody holds anything on.
    #[test]
    fn the_home_strips_show_only_what_the_person_holds() {
        crate::executor::storage::tests::with_temp_state("home-strips", || {
            use vela_core::app::balance_dashboard::BalanceToken;
            let token =
                |chain_id: u32, symbol: &str, balance: &str, price: Option<f64>| BalanceToken {
                    chain_id,
                    symbol: symbol.to_owned(),
                    name: symbol.to_owned(),
                    balance: balance.to_owned(),
                    decimals: 18,
                    token_address: None,
                    price_usd: price,
                    spam: false,
                };
            let mut held = view(Some(100.0));
            held.tokens = vec![
                token(1, "ETH", "0.05", Some(2_000.0)),
                token(100, "xDAI", "0.75897", Some(1.0)),
                token(100, "USDC", "12", Some(1.0)),
            ];
            let s = strings();

            let assets = asset_rows(&held, &s, "en-US", None, &Money::default());
            assert_eq!(assets.len(), 3);
            // The core's order, kept: it already sorted by value.
            assert_eq!(assets[0].ticker, "ETH");
            assert_eq!(assets[0].chain, "Ethereum");
            assert!(matches!(&assets[0].fiat, Fiat::Value(v) if v.as_ref() == "$100.00"));

            let chains = chain_rows(&held, &s, None);
            // "All networks" plus the TWO chains held on — not the twelve the
            // wallet knows about.
            assert_eq!(chains.len(), 3);
            assert_eq!(chains[0].name, s.all_networks);
            assert_eq!(chains[0].count, 2, "the count is the chains listed");
            assert!(chains[0].dot.is_none(), "all is not a chain");
            assert!(chains[0].selected);
            assert_eq!(chains[1].name, "Ethereum");
            assert_eq!(chains[1].count, 1);
            assert_eq!(chains[2].name, "Gnosis");
            assert_eq!(chains[2].count, 2);

            // Still counting: an empty strip, never somebody else's tokens.
            let counting = view(None);
            assert!(asset_rows(&counting, &s, "en-US", None, &Money::default()).is_empty());
            assert_eq!(chain_rows(&counting, &s, None).len(), 1, "only the all row");
        });
    }

    /// Narrowing to one network: the strip shows that chain's holdings, the
    /// check moves to its row, and — the part that would be a money bug — the
    /// drawn rows still MAP to the core's own list.
    ///
    /// Row 0 of "Gnosis" is not holding 0. The asset panel is addressed by
    /// index into the unfiltered list, so a lost mapping opens the wrong
    /// holding, on a panel whose next button is 转账.
    #[test]
    fn narrowing_to_a_chain_keeps_the_rows_pointing_at_the_right_holdings() {
        crate::executor::storage::tests::with_temp_state("chain-filter", || {
            use vela_core::app::balance_dashboard::BalanceToken;
            let token = |chain_id: u32, symbol: &str| BalanceToken {
                chain_id,
                symbol: symbol.to_owned(),
                name: symbol.to_owned(),
                balance: "1".to_owned(),
                decimals: 18,
                token_address: None,
                price_usd: Some(1.0),
                spam: false,
            };
            let mut held = view(Some(3.0));
            held.tokens = vec![
                token(1, "ETH"),
                token(100, "xDAI"),
                token(100, "USDC"),
                token(56, "BNB"),
            ];
            let s = strings();

            let gnosis = Some(100);
            let rows = asset_rows(&held, &s, "en-US", gnosis, &Money::default());
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].ticker, "xDAI");
            assert_eq!(rows[1].ticker, "USDC");
            // The mapping: drawn row 0 is the core's holding 1, not 0.
            assert_eq!(visible_token_indices(&held, gnosis), vec![1, 2]);
            assert_eq!(
                visible_token_indices(&held, None),
                vec![0, 1, 2, 3],
                "no filter maps to itself"
            );

            // The check moves; the list of chains does not.
            let chains = chain_rows(&held, &s, gnosis);
            assert_eq!(chains.len(), 4);
            assert!(!chains[0].selected, "all networks is no longer the one");
            assert_eq!(chains[0].chain_id, None);
            let picked = chains
                .iter()
                .find(|row| row.chain_id == Some(100))
                .unwrap_or_else(|| unreachable!("Gnosis is held on"));
            assert!(picked.selected);
            assert_eq!(picked.count, 2);

            // A chain held on with nothing else: still a real, if short, list.
            assert_eq!(
                asset_rows(&held, &s, "en-US", Some(56), &Money::default()).len(),
                1
            );
            // And a chain nothing is held on narrows to nothing rather than
            // falling back to everything.
            assert!(asset_rows(&held, &s, "en-US", Some(137), &Money::default()).is_empty());
        });
    }

    /// SC-003's visible half: a chain that could not be reached becomes a chip,
    /// and one that is merely rate-limited does not.
    #[test]
    fn an_unreachable_chain_becomes_a_chip_and_a_rate_limited_one_does_not() {
        crate::executor::storage::tests::with_temp_state("banner-chips", || {
            // Nothing wrong: no banner at all. An empty list is not a banner
            // saying "0 networks unavailable".
            assert!(unreachable_chips(&view(Some(10.0))).is_empty());

            let mut down = view(Some(10.0));
            down.banner_chain_ids = vec![100, 137];
            let chips = unreachable_chips(&down);
            assert_eq!(chips.len(), 2);
            assert_eq!(chips[0].2, "Gnosis");
            assert_eq!(chips[1].2, "Polygon");
            assert_eq!(chips[0].0, "G");
            // The tint is the settings table's, so the chip matches the network
            // row for the same chain.
            assert_eq!(
                chips[0].1,
                crate::settings::model::chain_tint(100)
                    .unwrap_or_else(|| unreachable!("Gnosis has a tint"))
            );

            // A network the person added is named by the name they gave it —
            // which is why these strings are owned rather than `&'static str`.
            let networks = serde_json::json!([
                { "chainId": 7_777_777, "displayName": "My testnet" }
            ]);
            if crate::executor::storage::write_value(
                crate::executor::storage::KEY_CUSTOM_NETWORKS,
                networks,
            )
            .is_err()
            {
                unreachable!("could not seed");
            }
            let mut custom = view(Some(10.0));
            custom.banner_chain_ids = vec![7_777_777];
            assert_eq!(unreachable_chips(&custom)[0].2, "My testnet");
        });
    }

    /// SC-001, end to end: the golden Safe's own money reaches the hero.
    ///
    /// Not a screenshot. This drives the real `balance_dashboard` machine over
    /// the real network and renders the real hero model, so what it proves is
    /// the whole chain — pool routing, the multicall, the price ladder, the
    /// core's total, and the split into the figure the screen draws.
    /// The fetch reports money before it settles.
    ///
    /// The point of the streaming fetch is that twelve chains are read on
    /// twelve threads and the settle waits for all of them, so an unreachable
    /// RPC used to hold the screen on its skeleton (or on yesterday's cached
    /// total) for its entire timeout while eleven chains sat answered and
    /// unused.
    ///
    /// **What this test proves and what it does not.** It proves the reports
    /// exist, that each chain reports for itself, and that the first one
    /// already puts a drawable total in the core's hands — before the settle.
    /// It does NOT prove the wall-clock earliness, because the driver here is
    /// `run_streaming`, which keeps the ORDER and drops the concurrency (its
    /// own doc says so). The timing is the async pump's, and this repo has no
    /// gpui harness to drive it.
    #[test]
    #[ignore = "reads every chain for a real address"]
    fn the_fetch_reports_money_before_it_settles() {
        crate::executor::storage::tests::with_temp_state("hero-stream", || {
            crate::executor::chain_tokens::invalidate();
            crate::executor::chainlink::invalidate();
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let (per_report, settled) = settle_reporting(GOLDEN);

            assert!(
                per_report.len() > 1,
                "each chain reports for itself: {} report(s)",
                per_report.len()
            );
            // This Safe holds xDAI on Gnosis and nothing anywhere else, so
            // most reports are correctly empty — the claim is that one of
            // them, before the settle, already had a total the hero can draw.
            let funded = per_report
                .iter()
                .position(|view| {
                    !view.tokens.is_empty() && view.display_total_usd.is_some_and(|usd| usd > 0.0)
                })
                .unwrap_or_else(|| {
                    unreachable!("no report carried money before the settle: {per_report:?}")
                });
            assert!(
                funded < per_report.len(),
                "and it arrived as a report, not as the settle"
            );
            // The settle is still the complete picture — streaming adds to
            // what the screen sees early, it does not replace the settle.
            assert!(settled.tokens.len() >= per_report[funded].tokens.len());
        });
    }

    #[test]
    #[ignore = "reads every chain for a real address"]
    fn the_hero_shows_the_golden_safes_own_money() {
        crate::executor::storage::tests::with_temp_state("hero-live", || {
            crate::executor::chain_tokens::invalidate();
            crate::executor::chainlink::invalidate();
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let view = settle(GOLDEN);
            let model = balance(&view, &strings(), "en-US", &Money::default());
            println!(
                "  hero: {}{}  state={:?} notice={:?}",
                model.integer,
                model
                    .decimals
                    .as_ref()
                    .map_or_else(String::new, |d| format!(".{d}")),
                model.state,
                view.notice
            );

            // The number is REAL, not the fixture and not a skeleton. The
            // fixture total is $1,383.28, and it appearing here would mean the
            // app is showing somebody a stranger's money under their own name.
            assert_eq!(model.state, BalanceState::Normal, "still counting, or zero");
            assert!(
                model.integer.starts_with('$'),
                "no figure at all: {:?}",
                model.integer
            );
            assert_ne!(model.integer.as_ref(), "$1,383", "that is the fixture");
            let usd = view
                .display_total_usd
                .unwrap_or_else(|| unreachable!("no total"));
            // A band, not a figure: the Safe's balance moves on-chain, and a
            // test that goes red when the world changes reports the wrong
            // thing. What must hold is that the total is a plausible amount of
            // money rather than base units or a phantom chain's constant.
            assert!(usd > 0.01 && usd < 1_000.0, "implausible total ${usd}");
        });
    }
}

/// The chains a person cannot reach right now, as the banner's chips.
///
/// **This is SC-003's visible half.** The fetch already reports an unreachable
/// chain separately from an empty one, and the core already computes which of
/// those deserve a banner. Without this the verdict is correct and invisible,
/// which for the person looking at the screen is the same as absent.
///
/// The core's list, not `failed_chain_ids`: `banner_chain_ids` is failed MINUS
/// rate-limited (invariant ⑦), because a rate limit lifts on its own and a
/// "fix your RPC" banner that nags about one is telling somebody to repair
/// something that is not broken.
#[must_use]
pub fn unreachable_chips(view: &BalanceView) -> Vec<(SharedString, u32, SharedString)> {
    view.banner_chain_ids
        .iter()
        .map(|chain_id| {
            let name = crate::executor::custom_tokens::network_name(*chain_id);
            (
                crate::settings::model::lettermark(&name),
                // The same table the network rows and the activity badges read.
                // A second colour map for the same chains is how one screen's
                // Polygon stops matching another's.
                crate::settings::model::chain_tint(u64::from(*chain_id)).unwrap_or(0x8A_8F_98),
                SharedString::from(name),
            )
        })
        .collect()
}

/// The home's asset strip — the person's holdings, most valuable first.
///
/// The core already sorted and filtered them (`sortAndFilterHoldings`: zero
/// balances dropped, highest value first), so this only renders. Re-sorting
/// here would be a second opinion about which holding matters most.
///
/// Empty while the core is still counting, and empty is right: the strip
/// simply has no rows yet. The hero next to it is already saying "counting" in
/// the one place that can say it without inventing a figure.
#[must_use]
pub fn asset_rows(
    view: &BalanceView,
    s: &WalletStrings,
    locale: &str,
    filter: Option<u32>,
    money: &Money,
) -> Vec<AssetRowModel> {
    let unpriced: std::collections::BTreeSet<(u32, String)> = view
        .unpriced_tokens
        .iter()
        .map(|token| {
            (
                token.chain_id,
                token.token_address.clone().unwrap_or_default(),
            )
        })
        .collect();
    visible_token_indices(view, filter)
        .into_iter()
        .filter_map(|index| view.tokens.get(index))
        .map(|token| {
            let key = (
                token.chain_id,
                token.token_address.clone().unwrap_or_default(),
            );
            let amount = token.balance.parse::<f64>().unwrap_or(0.0);
            AssetRowModel {
                logos: crate::marks::token_logos(
                    token.chain_id,
                    &token.symbol,
                    token.token_address.as_deref(),
                    &[],
                ),
                ticker: SharedString::from(token.symbol.clone()),
                chain: SharedString::from(crate::executor::custom_tokens::network_name(
                    token.chain_id,
                )),
                badge: badge(token.chain_id),
                balance: if view.hidden {
                    SharedString::from(crate::wallet::fixtures::MASK)
                } else {
                    SharedString::from(format_token_amount(
                        amount,
                        crate::executor::format_prefs::current().number,
                        false,
                    ))
                },
                fiat: if view.hidden {
                    Fiat::Masked
                } else if unpriced.contains(&key) {
                    Fiat::NoPrice(s.no_price.clone())
                } else {
                    Fiat::Value(SharedString::from(
                        money.text(amount * token.price_usd.unwrap_or(0.0), locale),
                    ))
                },
            }
        })
        .collect()
}

/// The home's network list: every chain the person holds something on, with
/// how many assets are on it, under an "all networks" row.
///
/// Only chains with a holding. A list of twelve networks where eleven say "0"
/// is a list nobody reads — and the eleven are not wrong, they are noise. The
/// count on the "all" row is the number of chains listed, so the two halves of
/// the strip cannot disagree.
#[must_use]
pub fn chain_rows(
    view: &BalanceView,
    s: &WalletStrings,
    filter: Option<u32>,
) -> Vec<ChainRowModel> {
    let mut order: Vec<u32> = Vec::new();
    for token in &view.tokens {
        if !order.contains(&token.chain_id) {
            order.push(token.chain_id);
        }
    }
    let mut rows = vec![ChainRowModel {
        name: s.all_networks.clone(),
        // The neutral dot: "all" is not a chain and must not wear one's colour.
        dot: None,
        count: u32::try_from(order.len()).unwrap_or(u32::MAX),
        selected: filter.is_none(),
        chain_id: None,
    }];
    for chain_id in order {
        rows.push(ChainRowModel {
            name: SharedString::from(crate::executor::custom_tokens::network_name(chain_id)),
            dot: Some(badge(chain_id)),
            count: u32::try_from(
                view.tokens
                    .iter()
                    .filter(|token| token.chain_id == chain_id)
                    .count(),
            )
            .unwrap_or(u32::MAX),
            selected: filter == Some(chain_id),
            chain_id: Some(chain_id),
        });
    }
    rows
}

/// Whether the home's asset strip says "nothing here" — the web's
/// `assetsMode(..) === 'empty'`: once the core has actually looked and there
/// is nothing held, or when the sidebar's chain holds nothing while others do.
/// A blank strip under a pill reads as a list that failed to load; while the
/// core is still counting it stays blank, because the hero says "counting".
#[must_use]
pub fn assets_strip_empty(view: &BalanceView, filter: Option<u32>) -> bool {
    if view.tokens.is_empty() {
        return !view.holdings_loading && !view.balance_unknown;
    }
    filter.is_some() && visible_token_indices(view, filter).is_empty()
}

/// Which holdings the network filter leaves on screen, as indices into the
/// core's own `tokens` order.
///
/// INDICES, not rows, because the asset panel is opened by index into that same
/// list: filtering the drawn rows without carrying the mapping would open the
/// wrong asset, and the next thing somebody does on that panel is send it.
///
/// The hero total deliberately does NOT narrow — the phone's `selectedChainId`
/// semantics, which the web ported in the same words: holdings and feed narrow,
/// the total stays the total.
#[must_use]
pub fn visible_token_indices(view: &BalanceView, filter: Option<u32>) -> Vec<usize> {
    view.tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| filter.is_none_or(|chain_id| token.chain_id == chain_id))
        .map(|(index, _)| index)
        .collect()
}

/// The celebration sentence — "120 USDT received" — or nothing.
///
/// Everything about WHETHER to celebrate is already decided: the core arms the
/// toast only on a post-first-pass sync that persisted a genuinely new receipt,
/// withholds it entirely while balance privacy is on (invariant ④ — a toast
/// would print the number the hero is masking), and expires it on its own
/// 2.8-second timer. This reads what survived all that and formats it with the
/// SAME number formatter the feed row beneath it uses, so the row and the
/// celebration cannot disagree about the amount.
///
/// A value that will not parse answers `None`. The core's own words for the
/// same corner: "fail closed: no toast rather than a wrong one" — and the wrong
/// one here would be a celebration with an empty space where the money goes.
#[must_use]
pub fn receipt_toast(view: &FeedView, s: &WalletStrings) -> Option<SharedString> {
    let toast = view.toast.as_ref()?;
    let amount = toast.value.parse::<f64>().ok()?;
    let amount = format_token_amount(
        amount,
        crate::executor::format_prefs::current().number,
        false,
    );
    Some(SharedString::from(crate::wallet::fill(
        &crate::wallet::fill(&s.toast_received, "amount", &amount),
        "token",
        &toast.symbol,
    )))
}

/// The ids of the feed ITEMS, in the order the home preview draws them.
///
/// The preview drops the core's day headers, so this is the header-free walk;
/// `flows::live::history_ids` is the same list for the full panel, which keeps
/// them. Both exist because the two surfaces draw different shapes of the same
/// feed, and a row must open its own transaction on either.
#[must_use]
pub fn history_item_ids(view: &FeedView) -> Vec<String> {
    view.rows
        .iter()
        .filter_map(|row| match row {
            FeedRow::Item { item } => Some(item.id.clone()),
            FeedRow::Header { .. } => None,
        })
        .collect()
}

/// One holding, in detail — D3.
///
/// `None` when the index names nothing, which happens the moment a refresh
/// re-orders the holdings under an open panel. Drawing the row that took its
/// place would silently swap which asset somebody is looking at, and the next
/// thing they do on that panel is send it.
#[must_use]
pub fn asset_detail(
    view: &BalanceView,
    feed: &FeedView,
    index: usize,
    s: &WalletStrings,
    locale: &str,
    money: &Money,
) -> Option<AssetDetailModel> {
    let token = view.tokens.get(index)?;
    let own: Vec<&FeedItem> = feed
        .rows
        .iter()
        .filter_map(|row| match row {
            FeedRow::Item { item }
                if item.symbol == token.symbol && item.chain_id == token.chain_id =>
            {
                Some(item)
            }
            _ => None,
        })
        .collect();
    let amount = token.balance.parse::<f64>().unwrap_or(0.0);
    let chain = crate::executor::custom_tokens::network_name(token.chain_id);
    let figure = |value: f64| money.text(value, locale);

    let mut facts = vec![(s.label_name.clone(), SharedString::from(token.name.clone()))];
    if let Some(price) = token.price_usd {
        facts.push((
            s.label_price.clone(),
            SharedString::from(crate::wallet::fill(
                &crate::wallet::fill(&s.price_value, "symbol", &token.symbol),
                "value",
                &figure(price),
            )),
        ));
    }
    facts.push((
        s.label_contract.clone(),
        match token.token_address.as_ref() {
            Some(address) => SharedString::from(shorten_address(address)),
            // The chain's own coin has no contract, and the mock says so in
            // words rather than leaving the row blank.
            None => s.native_token.clone(),
        },
    ));
    facts.push((
        s.label_decimals.clone(),
        SharedString::from(token.decimals.to_string()),
    ));

    Some(AssetDetailModel {
        logos: crate::marks::token_logos(
            token.chain_id,
            &token.symbol,
            token.token_address.as_deref(),
            &[],
        ),
        ticker: SharedString::from(token.symbol.clone()),
        badge: badge(token.chain_id),
        amount: if view.hidden {
            SharedString::from(crate::wallet::fixtures::MASK)
        } else {
            SharedString::from(format!(
                "{} {}",
                format_token_amount(
                    amount,
                    crate::executor::format_prefs::current().number,
                    false
                ),
                token.symbol
            ))
        },
        sub: if view.hidden {
            SharedString::from(chain.clone())
        } else {
            match token.price_usd {
                Some(price) => SharedString::from(format!("{} · {chain}", figure(amount * price))),
                // Unpriced: the chain alone, never "$0.00 · Gnosis".
                None => SharedString::from(format!("{} · {chain}", s.no_price)),
            }
        },
        facts,
        // This asset's own transactions, from the same feed the home draws.
        // Matched on symbol AND chain: two chains' USDC are different money.
        activity: own
            .iter()
            .map(|item| activity_row(feed, item, s, view.hidden))
            .collect(),
        // …and the id behind each, from the SAME walk, so row N opens
        // record N.
        activity_ids: own.iter().map(|item| item.id.clone()).collect(),
        explorer_url: token_explorer_url(token, view.address.as_deref()).map(SharedString::from),
    })
}

/// Where "view on explorer" leads for a held token (the web's
/// `tokenExplorerURL`): the token page, scoped to this account, for an
/// ERC-20; the account page for the chain's own coin. `None` for a chain with
/// no explorer — no link rather than a wrong one.
#[must_use]
pub fn token_explorer_url(token: &BalanceToken, account: Option<&str>) -> Option<String> {
    let base = crate::executor::custom_tokens::explorer_base(token.chain_id)?;
    match token.token_address.as_deref() {
        None => account.map(|account| format!("{base}/address/{account}")),
        Some(contract) => Some(match account {
            Some(account) => format!("{base}/token/{contract}?a={account}"),
            None => format!("{base}/token/{contract}"),
        }),
    }
}

/// The send a token's own 转账 opens: that token, on its network, already
/// chosen (the web's `enter('send', { assetId })`). The network is spelled
/// the way this shell's send executor spells `SendToken.network` — the core
/// matches the two strings, and any other spelling lands on the picker.
#[must_use]
pub fn token_send_params(token: &BalanceToken) -> vela_core::app::send::SendOpenParams {
    vela_core::app::send::SendOpenParams {
        preselected_symbol: Some(token.symbol.clone()),
        preselected_network: Some(crate::executor::send::network_id(token.chain_id)),
        ..vela_core::app::send::SendOpenParams::default()
    }
}

/// The chain tint for an activity badge.
///
/// Read out of `settings::model::chain_tint` — the same table the network rows
/// use, which is the same table the mocks use. A second colour map for the same
/// chains is how one screen's Polygon stops matching another's.
pub(crate) fn badge(chain_id: u32) -> gpui::Hsla {
    gpui::rgb(crate::settings::model::chain_tint(u64::from(chain_id)).unwrap_or(0x8A_8F_98)).into()
}

/// The activity rows the home preview shows.
///
/// **Headers are dropped here, not filtered out of the core.** `FeedView::rows`
/// interleaves day headers with items because the full Activity screen draws
/// them; the home preview is a short flat list and the mocks draw no headings in
/// it. Asking the core for a different shape would move a render decision into
/// the machine.
#[must_use]
pub fn activity_rows(view: &FeedView, s: &WalletStrings, hidden: bool) -> Vec<ActivityRowModel> {
    view.rows
        .iter()
        .filter_map(|row| match row {
            FeedRow::Header { .. } => None,
            FeedRow::Item { item } => Some(activity_row(view, item, s, hidden)),
        })
        .collect()
}

/// What kind of event a row is.
///
/// `FeedItem` carries only a direction; the RECORD carries the kind, and
/// `FeedView::transactions` is the account-scoped record list the core exposes
/// beside the rows. Looking it up there keeps the dApp distinction the mocks
/// draw — a swap is not "sent", and labelling it so loses the one word that
/// explains where the money went.
pub(crate) fn kind_of(view: &FeedView, item: &FeedItem, incoming: bool) -> ActivityKind {
    let record_kind = view
        .transactions
        .iter()
        .find(|record| record.id == item.id)
        .and_then(|record| record.kind);
    match record_kind {
        Some(FeedTxKind::DappTx) => ActivityKind::Dapp,
        // A signature is not money moving, but the home preview has no row for
        // it; treating it as the direction says is the least wrong of the three
        // shapes available, and the full Activity screen draws it properly.
        _ if incoming => ActivityKind::Received,
        _ => ActivityKind::Sent,
    }
}

pub(crate) fn activity_row(
    view: &FeedView,
    item: &FeedItem,
    s: &WalletStrings,
    hidden: bool,
) -> ActivityRowModel {
    let incoming = item.direction == FeedDirection::In;
    let kind = kind_of(view, item, incoming);

    // Who it was with: the resolved alias if the core has one, else a shortened
    // address, else nothing — a batch row has no single counterparty.
    let who = item
        .alias
        .clone()
        .or_else(|| item.counterparty.as_ref().map(|a| shorten_address(a)));
    let subtitle = who.map_or_else(
        || SharedString::from(""),
        |name| {
            SharedString::from(crate::wallet::fill(
                if incoming { &s.from_name } else { &s.to_name },
                "name",
                &name,
            ))
        },
    );

    ActivityRowModel {
        kind,
        title: match kind {
            ActivityKind::Sent => s.label_sent.clone(),
            ActivityKind::Received => s.label_received.clone(),
            ActivityKind::Dapp => s.label_dapp.clone(),
        },
        subtitle,
        // The chain this happened on, drawn as its logo where the endpoint has
        // one (§8.3; issue 201): the badge used to be a colour, and a colour
        // is not a network anyone can name.
        badge_logo: crate::marks::chain_logo_url(item.chain_id),
        // Privacy masks the FIGURE and keeps the unit — H5's rule, and the same
        // mask the hero uses, because a leak in one surface defeats it
        // everywhere (the core's invariant ④ on the balance side).
        amount: if hidden {
            SharedString::from(crate::wallet::fixtures::MASK)
        } else {
            amount_text(item, incoming)
        },
        unit: SharedString::from(item.symbol.clone()),
        positive: incoming,
        badge: badge(item.chain_id),
    }
}

/// `+120` / `−2`. The minus is U+2212, not a hyphen — the mocks use it and it
/// is what aligns under a digit.
/// The same signed amount the feed row shows, mask included.
///
/// Privacy masks the FIGURE and keeps the unit, on every surface together —
/// the detail panel is one of them, and reading a second flag is how one ends
/// up out of step.
pub(crate) fn amount_text_of(item: &FeedItem, incoming: bool, hidden: bool) -> SharedString {
    if hidden {
        return SharedString::from(crate::wallet::fixtures::MASK);
    }
    SharedString::from(format!("{} {}", amount_text(item, incoming), item.symbol))
}

pub(crate) fn amount_text(item: &FeedItem, incoming: bool) -> SharedString {
    let Some(value) = item.value.as_deref() else {
        // A multi-select batch has mixed tokens and no sum; the core says so by
        // sending no value, and inventing one here would be arithmetic nobody
        // asked for.
        return SharedString::from("");
    };
    // `value` is the HUMAN amount already — "1.5", not raw wei. The web renders
    // it with `trimBalance(item.value)` and the core sums it with `parseFloat`,
    // neither of which scales by `decimals`. Scaling here would print every
    // amount 10^18 times too large, and it would look deliberate.
    let Ok(amount) = value.parse::<f64>() else {
        return SharedString::from("");
    };
    let formatted = format_token_amount(
        amount,
        crate::executor::format_prefs::current().number,
        false,
    );
    SharedString::from(format!(
        "{}{formatted}",
        if incoming { "+" } else { "\u{2212}" }
    ))
}

pub(crate) fn shorten_address(address: &str) -> String {
    if address.len() <= 14 {
        return address.to_owned();
    }
    format!("{}…{}", &address[..6], &address[address.len() - 4..])
}
