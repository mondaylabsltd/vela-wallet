//! The wallet home's display models, built from what the cores decided.
//!
//! The sibling of `fixtures.rs`, never its replacement — the third of these
//! (settings, contacts, wallet) and the one where the rules bite hardest,
//! because every value here is somebody's money.

use gpui::SharedString;

use vela_core::app::activity_feed::{
    FeedAllowance, FeedDirection, FeedItem, FeedLine, FeedRow, FeedTxKind, FeedTxStatus, FeedView,
};
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
    /// Dollars, borrowable for as long as the process lives.
    ///
    /// Every screen that has no committed pair — signed out, and every drawn
    /// board — wants the same one, and a `&Money` that outlives the statement
    /// it was made in is what lets a builder take it by reference.
    #[must_use]
    pub fn usd() -> &'static Self {
        static USD: std::sync::OnceLock<Money> = std::sync::OnceLock::new();
        USD.get_or_init(Money::default)
    }

    /// The committed pair, straight from `display_currency`.
    #[must_use]
    pub fn new(code: &str, rate: Option<f64>) -> Self {
        Self {
            code: code.to_owned(),
            rate,
        }
    }

    /// The code money is actually DRAWN in, which is not always the code that
    /// was picked: with no rate there is nothing to convert with, so the figure
    /// — and anything labelling it — is dollars.
    #[must_use]
    pub fn code(&self) -> &str {
        if self.rate.is_some() {
            self.code.as_str()
        } else {
            "USD"
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

    // Hidden says nothing about the figure it is hiding — the web's hidden
    // hero has no status line (078 H-03).
    if view.hidden {
        return BalanceModel {
            label: s.total_balance.clone(),
            currency: SharedString::from(money.code().to_owned()),
            state: BalanceState::Hidden,
            integer: SharedString::from(BALANCE_MASK),
            decimals: None,
            live: None,
            status: None,
        };
    }

    let Some(usd) = view.display_total_usd.or(view.cached_total_usd) else {
        return BalanceModel {
            label: s.total_balance.clone(),
            currency: SharedString::from(money.code().to_owned()),
            state: BalanceState::Loading,
            // Not "$0". The core withholds the number until it has one, and the
            // shell must not fill the gap with a figure that reads as an answer.
            integer: SharedString::from(""),
            decimals: None,
            live: None,
            // A first launch with no network says so over the skeleton rather
            // than show a settled-looking zero (spec 038 finding 15) — and a
            // skeleton says nothing else: it is already "still counting".
            status: view
                .unreachable
                .then(|| (StatusKind::Warning, s.balance_unreachable.clone())),
        };
    };

    // One status line, most actionable first — the web's `liveBalance` order.
    // `banner_chain_ids` is already failed MINUS rate-limited (a rate limit
    // heals on its own), so a chain here really is unreachable and the person
    // can fix its RPC: the line names it, and opens its editor. Then a figure
    // being brought up to date — grey, because it is not wrong, only not
    // final. Then what could not be priced.
    let status = match view.banner_chain_ids.as_slice() {
        [chain_id] => Some((
            StatusKind::Warning,
            SharedString::from(crate::wallet::fill(
                &s.rpc_unavailable_single,
                "name",
                &crate::executor::custom_tokens::network_name(*chain_id),
            )),
        )),
        [_, _, ..] => Some((
            StatusKind::Warning,
            SharedString::from(crate::wallet::fill(
                &s.rpc_unavailable_multiple,
                "count",
                &view.banner_chain_ids.len().to_string(),
            )),
        )),
        [] if view.refreshing || on_cache || view.notice == Some(BalanceNotice::StillUpdating) => {
            Some((StatusKind::Refreshing, s.balance_stale.clone()))
        }
        [] if view.notice == Some(BalanceNotice::Unpriced) => {
            Some((StatusKind::Warning, s.balance_unpriced.clone()))
        }
        [] => None,
    };

    let (integer, decimals) = split_fiat(usd, locale, money);
    BalanceModel {
        label: s.total_balance.clone(),
        currency: SharedString::from(money.code().to_owned()),
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
        // The web's "listening" line under a live zero (078 H-05): a wallet
        // every chain has answered for, holding nothing, is waiting for its
        // first deposit — and says so rather than looking empty.
        live: (usd == 0.0
            && !view.balance_unknown
            && !view.balance_partial
            && view.tokens.is_empty())
        .then(|| s.live_indicator.clone()),
        status,
    }
}

/// SR3, the balance by network (078 H-03) — the web's `liveBalanceDetail`:
/// the chains still being read (rate-limited, retrying on their own) or
/// unreachable (with a Retry), the chains that settled, largest first, and the
/// tokens nothing could price. The same figures the hero sums.
pub struct BalanceDetail {
    pub summary: SharedString,
    pub pending: Vec<DetailChain>,
    pub done: Vec<DetailChain>,
    pub unpriced: Vec<DetailChain>,
}

/// One line of the breakdown: a chain (or an unpriced token on one), what it
/// says under its name, what it says at the end, and — for an unreachable
/// chain — the Retry it offers.
pub struct DetailChain {
    pub chain_id: u32,
    pub name: SharedString,
    /// Under the name; `(text, failed)` — a failed chain says so in red.
    pub status: Option<(SharedString, bool)>,
    pub amount: Option<SharedString>,
    pub retry: bool,
}

#[must_use]
pub fn balance_detail(
    view: &BalanceView,
    s: &WalletStrings,
    locale: &str,
    money: &Money,
) -> BalanceDetail {
    let name =
        |chain_id: u32| SharedString::from(crate::executor::custom_tokens::network_name(chain_id));
    let mask = || SharedString::from(crate::wallet::fixtures::MASK);

    let mut pending: Vec<DetailChain> = view
        .rate_limited_chain_ids
        .iter()
        .map(|&chain_id| DetailChain {
            chain_id,
            name: name(chain_id),
            status: Some((s.detail_retrying.clone(), false)),
            amount: None,
            retry: false,
        })
        .collect();
    for &chain_id in &view.banner_chain_ids {
        if pending.iter().any(|row| row.chain_id == chain_id) {
            continue;
        }
        pending.push(DetailChain {
            chain_id,
            name: name(chain_id),
            status: Some((s.detail_failed.clone(), true)),
            amount: None,
            retry: true,
        });
    }

    let mut per_chain: Vec<(u32, f64)> = Vec::new();
    for token in &view.tokens {
        let usd = token.balance.parse::<f64>().unwrap_or(f64::NAN) * token.price_usd.unwrap_or(0.0);
        if !usd.is_finite() {
            continue;
        }
        match per_chain.iter_mut().find(|(id, _)| *id == token.chain_id) {
            Some((_, sum)) => *sum += usd,
            None => per_chain.push((token.chain_id, usd)),
        }
    }
    per_chain.retain(|(id, _)| !pending.iter().any(|row| row.chain_id == *id));
    per_chain.sort_by(|a, b| b.1.total_cmp(&a.1));
    let done = per_chain
        .into_iter()
        .map(|(chain_id, usd)| DetailChain {
            chain_id,
            name: name(chain_id),
            status: None,
            amount: Some(if view.hidden {
                mask()
            } else {
                SharedString::from(money.text(usd, locale))
            }),
            retry: false,
        })
        .collect();

    let total = view.display_total_usd.or(view.cached_total_usd);
    let summary = crate::wallet::fill(
        &s.detail_total,
        "amount",
        &match total {
            Some(usd) if !view.hidden => money.text(usd, locale),
            _ => crate::wallet::fixtures::MASK.to_owned(),
        },
    );

    let unpriced = view
        .unpriced_tokens
        .iter()
        .map(|token| DetailChain {
            chain_id: token.chain_id,
            name: SharedString::from(token.symbol.clone()),
            status: Some((
                SharedString::from(format!(
                    "{} · {}",
                    crate::executor::custom_tokens::network_name(token.chain_id),
                    if view.hidden {
                        crate::wallet::fixtures::MASK.to_owned()
                    } else {
                        token_amount_text(&token.balance)
                    }
                )),
                false,
            )),
            amount: None,
            retry: false,
        })
        .collect();

    BalanceDetail {
        summary: SharedString::from(summary),
        pending,
        done,
        unpriced,
    }
}

/// What the account holds on each network, in the display currency (spec 079
/// FR-017, owner: "需要能看到这个网络上的余额吧") — the home screen's own
/// figures, never fetched for the picker. A network gets no figure at all
/// rather than a made-up zero when its balance is not known (it failed, or
/// only rate-limited), when nothing priced is held there, and when balances
/// are hidden: the privacy mask covers every money surface.
#[must_use]
pub fn network_balances(
    view: &BalanceView,
    locale: &str,
    money: &Money,
) -> std::collections::HashMap<u32, SharedString> {
    let mut sums: std::collections::HashMap<u32, f64> = std::collections::HashMap::new();
    if view.hidden {
        return std::collections::HashMap::new();
    }
    for token in view.tokens.iter().filter(|token| {
        !token.spam
            && !view.failed_chain_ids.contains(&token.chain_id)
            && !view.rate_limited_chain_ids.contains(&token.chain_id)
    }) {
        let Some(price) = token.price_usd else {
            continue;
        };
        let usd = token.balance.parse::<f64>().unwrap_or(f64::NAN) * price;
        if usd.is_finite() {
            *sums.entry(token.chain_id).or_default() += usd;
        }
    }
    sums.into_iter()
        // Half a cent is the smallest figure a person reads as money.
        .filter(|(_, usd)| *usd >= 0.005)
        .map(|(chain_id, usd)| (chain_id, SharedString::from(money.text(usd, locale))))
        .collect()
}

/// `$1,383.28` → `("$1,383", Some("28"))`.
///
/// The hero draws the minor units smaller than the number — the design
/// language's "subordinated symbols" rule — so the split is a render concern and
/// belongs here rather than in a formatter. Splitting on the LAST `.` is what
/// keeps a locale whose group separator is `.` from being cut in half.
fn split_fiat(usd: f64, locale: &str, money: &Money) -> (SharedString, Option<SharedString>) {
    split_at_mark(
        &money.text(usd, locale),
        crate::executor::format_prefs::current()
            .number
            .separators()
            .decimal,
    )
}

/// Split a formatted figure at the person's DECIMAL MARK — never at a fixed
/// `.`. Under `1.234,56` a `.` split drew "1" large and ".234,56" small, and
/// a comma-decimal balance was never split at all (078 H-07; the web splits by
/// `numberSeparators().decimal`). What follows the digits — a symbol written
/// after the figure, "1.234,56 €" — stays with the small part, so nothing is
/// dropped.
fn split_at_mark(formatted: &str, mark: &str) -> (SharedString, Option<SharedString>) {
    if let Some((whole, rest)) = formatted.rsplit_once(mark) {
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        // Minor units are one to three digits; anything else is not a
        // fraction (a group, when the mark doubles as one elsewhere).
        if (1..=3).contains(&digits) {
            return (
                SharedString::from(whole.to_owned()),
                Some(SharedString::from(rest.to_owned())),
            );
        }
    }
    // No minor units — a large balance drops them by product rule
    // (`drop_minor_units_above`), and a currency with zero fraction digits
    // never had them.
    (SharedString::from(formatted.to_owned()), None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_host::CoreHost;
    use vela_core::app::balance_dashboard::{BalanceDashboard, Event as BalanceEvent};

    /// The one token-amount rule, pinned with the core's own vectors
    /// (`a_max_reads_on_the_balance_lines_ladder`) — the same list web and
    /// iOS pin — ungrouped, so a balance matches the Max taken from it.
    #[test]
    fn token_amounts_read_on_the_one_ladder() {
        for (exact, shown) in [
            ("0.043790209243313861", "0.04379"),
            ("0.0439686", "0.043969"),
            ("1.22456789123456789", "1.2246"),
            ("1234.567", "1234.57"),
            ("2", "2"),
            ("0", "0"),
            ("0.9999996", "1"),
            ("999.99996", "1000"),
            ("0.0000001234", "0.00000012"),
            ("5.000000", "5"),
        ] {
            assert_eq!(token_amount_text(exact), shown, "{exact}");
        }
    }

    /// A ceiling is cut, never rounded up past what the balance covers.
    #[test]
    fn a_ceiling_is_cut_down() {
        assert_eq!(token_amount_text_down("0.0439686"), "0.043968");
        assert_eq!(token_amount_text_down("1.22456789"), "1.2245");
        assert_eq!(token_amount_text_down("999.99996"), "999.9999");
        assert_eq!(token_amount_text_down("1234.567"), "1234.56");
        assert_eq!(token_amount_text_down("0.0000001234"), "0.00000012");
        assert_eq!(token_amount_text_down("3"), "3");
    }

    /// Spec 079: the picker's figure per network — the home screen's own
    /// numbers, nothing where they are not known, nothing under the privacy
    /// mask, never a zero.
    #[test]
    fn each_network_shows_what_the_account_holds_there() {
        let token = |chain_id: u32, balance: &str, price: Option<f64>, spam: bool| BalanceToken {
            chain_id,
            symbol: "T".to_owned(),
            name: "T".to_owned(),
            balance: balance.to_owned(),
            decimals: 18,
            token_address: None,
            price_usd: price,
            spam,
        };
        let mut shown = view(Some(10.));
        shown.tokens = vec![
            token(1, "2", Some(3.), false),
            token(1, "1", Some(1.5), false),
            token(100, "0.001", Some(1.), false),
            token(8453, "5", None, false),
            token(10, "9", Some(1.), true),
            token(42161, "4", Some(1.), false),
            token(137, "4", Some(1.), false),
        ];
        shown.failed_chain_ids = vec![42161];
        shown.rate_limited_chain_ids = vec![137];
        let money = Money::usd();
        let figures = network_balances(&shown, "en", money);
        assert_eq!(
            figures.get(&1),
            Some(&SharedString::from(money.text(7.5, "en")))
        );
        for chain_id in [100, 8453, 10, 42161, 137] {
            assert!(
                !figures.contains_key(&chain_id),
                "chain {chain_id}: dust, unpriced, spam, failed and rate-limited have no figure"
            );
        }
        shown.hidden = true;
        assert!(network_balances(&shown, "en", money).is_empty());
    }

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
        ActivityFeed, Event as FeedEvent, FeedItem, FeedLine, FeedRow, FeedView,
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
            dapp_url: None,
            intent: None,
            balance_changes: None,
            calldata: None,
            call_data: None,
            summary: None,
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
            kind: if incoming {
                vela_core::app::activity_feed::FeedTxKind::Receive
            } else {
                vela_core::app::activity_feed::FeedTxKind::Send
            },
            status: vela_core::app::activity_feed::FeedTxStatus::Confirmed,
            site: None,
            counterparty_role: Default::default(),
            dapp: None,
            subtitle: Vec::new(),
        }
    }

    /// A dApp row's half as the core builds it, for a row drawn by hand:
    /// `site` and the recorded intent, nothing granted, no place.
    fn dapp_of(
        site: Option<&str>,
        intent: Option<&str>,
        intent_term: Option<vela_core::app::clear_signing::ClearTerm>,
    ) -> vela_core::app::activity_feed::FeedDapp {
        vela_core::app::activity_feed::FeedDapp {
            site: site.map(str::to_owned),
            intent: intent.map(str::to_owned),
            intent_term,
            changes: Vec::new(),
            received: None,
            estimated: false,
            contract_call: false,
            action: vela_core::app::dapp_activity::DappAction::Call,
            place: None,
            allowance: None,
            off_chain: false,
            facts: Vec::new(),
            technical: Vec::new(),
        }
    }

    fn flow_strings() -> crate::flows::FlowStrings {
        crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env())
    }

    /// The feed core over `records`, loaded the way the executor answers it.
    fn feed_of(records: Vec<vela_core::app::activity_feed::FeedTxRecord>) -> FeedView {
        crate::wallet::fixtures::core_feed(records)
    }

    /// A dApp's transaction as `sign_request::persist_record` stores it.
    fn dapp_record(
        id: &str,
        status: vela_core::app::activity_feed::FeedTxStatus,
    ) -> vela_core::app::activity_feed::FeedTxRecord {
        vela_core::app::activity_feed::FeedTxRecord {
            id: id.to_owned(),
            user_op_hash: "0xop".to_owned(),
            tx_hash: String::new(),
            from: "0xme".to_owned(),
            to: "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141".to_owned(),
            to_name: None,
            value: "0x38d7ea4c68000".to_owned(),
            symbol: "xDAI".to_owned(),
            decimals: 18,
            logo_urls: None,
            chain_id: 100,
            timestamp: 1_756_000_000.0,
            day_start_ms: 0.0,
            status,
            kind: Some(vela_core::app::activity_feed::FeedTxKind::DappTx),
            usd: None,
            dapp_url: Some("http://127.0.0.1:8137".to_owned()),
            intent: None,
            balance_changes: None,
            calldata: None,
            call_data: None,
            summary: None,
        }
    }

    /// Spec 082 L-D3 (T072, RG1–RG2), spec 093: a dApp's transaction is in
    /// Activity, titled by what it did where — a call nobody decoded, at the
    /// site that asked, "Contract interaction on 127.0.0.1:8137" — and its
    /// second line is the core's: the status of one that has not landed, then
    /// the network ("处理中 · Gnosis"); the site is already in the title.
    #[test]
    fn a_dapp_transaction_is_a_row_with_its_site_and_status() {
        use vela_core::app::activity_feed::FeedTxStatus;
        let s = strings();
        let title = crate::wallet::fill(
            &crate::wallet::fill(&s.dapp_row_title, "place", "127.0.0.1:8137"),
            "intent",
            &s.intent_contract_call,
        );
        let gnosis = crate::flows::live::chain_name(100);
        for (status, word) in [
            (FeedTxStatus::Pending, s.status_pending.clone()),
            (FeedTxStatus::Failed, s.status_failed.clone()),
        ] {
            let view = feed_of(vec![dapp_record("dapp-1-tx", status)]);
            let rows = activity_rows(&view, &s, &flow_strings(), false);
            assert_eq!(rows.len(), 1, "{status:?}: the row is in Activity");
            let row = &rows[0];
            assert!(matches!(row.kind, ActivityKind::Dapp));
            assert_eq!(row.title.as_ref(), title);
            assert_eq!(
                row.subtitle.as_ref(),
                format!("{word} · {gnosis}"),
                "{status:?}"
            );
            assert_eq!(row.amount.as_ref(), "\u{2212}0.001");
        }
        // Confirmed: the network alone.
        let view = feed_of(vec![dapp_record("dapp-2-tx", FeedTxStatus::Confirmed)]);
        let rows = activity_rows(&view, &s, &flow_strings(), false);
        assert_eq!(rows[0].subtitle.as_ref(), gnosis);
    }

    /// Spec 093, the three rows through the REAL feed core, in Chinese: a
    /// swap on Uniswap says what left and what came back, a permit says it
    /// granted an unlimited allowance — in the danger tone, and never masked:
    /// it is a risk, not a balance — and a sign-in is a row with no figure.
    /// Where the title names a protocol, the site is on the second line.
    #[test]
    fn the_cores_dapp_rows_are_drawn_in_its_words() {
        let loc = crate::loc::Loc::for_language("zh");
        let s = WalletStrings::resolve(&loc);
        let flow = crate::flows::FlowStrings::resolve(&loc);
        let view = feed_of(crate::wallet::fixtures::dapp_activity_records(
            1_756_000_000.0,
        ));
        let ethereum = crate::flows::live::chain_name(1);
        for hidden in [false, true] {
            let rows = activity_rows(&view, &s, &flow, hidden);
            assert_eq!(rows.len(), 3, "a signature is a row too");
            let drawn: Vec<(&str, &str)> = rows
                .iter()
                .map(|row| (row.title.as_ref(), row.subtitle.as_ref()))
                .collect();
            assert_eq!(
                drawn,
                vec![
                    (
                        "在 Uniswap 兑换",
                        format!("app.uniswap.org · {ethereum}").as_str()
                    ),
                    (
                        "在 Uniswap 授权签名",
                        format!("app.uniswap.org · {ethereum}").as_str()
                    ),
                    ("在 app.uniswap.org 登录", ethereum.as_str()),
                ]
            );
            assert!(rows.iter().all(|row| row.kind == ActivityKind::Dapp));
            // The swap: what left and what was expected back.
            assert_eq!(rows[0].unit.as_ref(), "USDC");
            assert!(!rows[0].danger);
            // The permit: the allowance, unlimited, in the danger tone.
            assert_eq!(
                (rows[1].amount.as_ref(), rows[1].unit.as_ref()),
                ("无限额", "USDC")
            );
            assert!(rows[1].danger);
            // The sign-in moved nothing and granted nothing.
            assert_eq!((rows[2].amount.as_ref(), rows[2].unit.as_ref()), ("", ""));
            assert!(!rows[2].danger);
            if hidden {
                assert_eq!(rows[0].amount.as_ref(), crate::wallet::fixtures::MASK);
            } else {
                assert_eq!(rows[0].amount.as_ref(), "≈ \u{2212}100");
                assert_eq!(
                    rows[0].received.as_ref().map(AsRef::as_ref),
                    Some("≈ +0.03 ETH")
                );
            }
        }
    }

    /// Spec 093: a finite allowance reads as its figure and its token, masked
    /// like any figure when balances are hidden; an unlimited one reads the
    /// sheet's own word, and is never masked.
    #[test]
    fn an_allowance_is_its_figure_or_the_unlimited_word() {
        use vela_core::app::activity_feed::FeedAllowance;
        let s = strings();
        let finite = FeedAllowance {
            symbol: "USDC".to_owned(),
            value: Some("100".to_owned()),
            decimals: Some(6),
            unlimited: false,
            token: None,
        };
        assert_eq!(allowance_text(&finite, &s, false).as_ref(), "100 USDC");
        assert_eq!(
            allowance_text(&finite, &s, true).to_string(),
            format!("{} USDC", crate::wallet::fixtures::MASK)
        );
        let unlimited = FeedAllowance {
            value: None,
            decimals: None,
            unlimited: true,
            ..finite
        };
        for hidden in [false, true] {
            assert_eq!(
                allowance_text(&unlimited, &s, hidden).to_string(),
                format!("{} USDC", s.unlimited_value)
            );
        }
        let nameless = FeedAllowance {
            symbol: String::new(),
            ..unlimited
        };
        assert_eq!(allowance_text(&nameless, &s, false), s.unlimited_value);
    }

    /// 087 F04/F05, through the real core: a dApp record from an older build
    /// — no operation hash, no tx hash — three days on is "未知 · <site>",
    /// never "处理中" for ever and never "失败"; its detail says the same,
    /// draws no hash (the record id is not one) and keeps a quiet delete.
    #[test]
    fn a_record_nothing_will_settle_reads_unknown_and_shows_no_hash() {
        use vela_core::app::activity_feed::FeedTxStatus;
        let s = strings();
        let mut legacy = dapp_record("dapp-1790500000796-tx", FeedTxStatus::Pending);
        legacy.user_op_hash = String::new();
        legacy.timestamp -= 3.0 * 86_400.0;
        let view = feed_of(vec![legacy]);
        let rows = activity_rows(&view, &s, &flow_strings(), false);
        assert_eq!(
            rows[0].subtitle.to_string(),
            format!(
                "{} · {}",
                s.status_unknown,
                crate::flows::live::chain_name(100)
            )
        );
        assert_ne!(s.status_unknown, s.status_pending);
        assert_ne!(s.status_unknown, s.status_failed);

        let flows = flow_strings();
        let detail = crate::flows::live::tx_detail(
            &view,
            "dapp-1790500000796-tx",
            &flows,
            &s,
            false,
            "en-US",
            super::Money::usd(),
        )
        .unwrap_or_else(|| unreachable!("the row exists"));
        let chip = detail
            .status
            .as_ref()
            .unwrap_or_else(|| unreachable!("a transaction wears a chip"));
        assert_eq!(chip.text, flows.status_unknown);
        assert!(matches!(
            crate::flows::panels::delete_style(chip),
            crate::flows::panels::DeleteStyle::Quiet
        ));
        assert!(
            detail
                .facts
                .iter()
                .all(|fact| fact.label != flows.detail_hash)
        );
        assert!(detail.facts.iter().all(|fact| {
            fact.copy
                .as_ref()
                .is_none_or(|copy| !copy.contains("dapp-"))
        }));
        assert!(detail.explorer_url.is_none());
        assert!(detail.delete_label.is_some());
    }

    /// Spec 093: every part of the core's second line, in the reader's
    /// words — a status in the feed's status words, whom a transfer went to
    /// or came from (a name, else the short address), a site verbatim, a
    /// network by its name — joined " · ", in the core's order. A confirmed
    /// status (which the core never sends) says nothing.
    #[test]
    fn every_part_of_the_second_line_is_worded_here() {
        use vela_core::app::activity_feed::{FeedLine, FeedTxStatus};
        let s = strings();
        let address = "0xAbCdEf0000000000000000000000000000000001".to_owned();
        let line = |lines: Vec<FeedLine>| subtitle_text(&lines, &s).to_string();
        assert_eq!(
            line(vec![
                FeedLine::Status {
                    status: FeedTxStatus::Pending
                },
                FeedLine::Site {
                    site: "app.uniswap.org".to_owned()
                },
                FeedLine::Network { chain_id: 100 },
            ]),
            format!(
                "{} · app.uniswap.org · {}",
                s.status_pending,
                crate::flows::live::chain_name(100)
            )
        );
        assert_eq!(
            line(vec![
                FeedLine::Status {
                    status: FeedTxStatus::Unknown
                },
                FeedLine::To {
                    address: address.clone(),
                    name: None
                },
            ]),
            format!(
                "{} · {}",
                s.status_unknown,
                crate::wallet::fill(&s.to_name, "name", "0xAbCd…0001")
            )
        );
        assert_eq!(
            line(vec![FeedLine::From {
                address: address.clone(),
                name: Some("Alice".to_owned())
            }]),
            crate::wallet::fill(&s.from_name, "name", "Alice")
        );
        assert_eq!(
            line(vec![
                FeedLine::Status {
                    status: FeedTxStatus::Failed
                },
                FeedLine::Status {
                    status: FeedTxStatus::Confirmed
                },
            ]),
            s.status_failed.to_string()
        );
        assert_eq!(line(Vec::new()), "");
    }

    /// Spec 082 RD10 / RG5 (T073): the home's Activity — rows, skeletons
    /// while nothing has been ruled, else the core's empty line: the plain one
    /// with its caption, or the network's own with none under a chain filter.
    #[test]
    fn the_home_activity_has_three_states_under_both_filters() {
        let s = strings();
        let mut host = CoreHost::<ActivityFeed>::new();
        let _ = host.dispatch(FeedEvent::AccountSwitched {
            address: "0xme".to_owned(),
        });
        let all = host.view();
        let _ = host.dispatch(FeedEvent::ChainFilterChanged {
            chain_id: Some(100),
        });
        let narrowed = host.view();
        for feed in [&all, &narrowed] {
            assert_eq!(home_activity(true, true, feed, &s), HomeActivity::Rows);
            assert_eq!(home_activity(true, false, feed, &s), HomeActivity::Rows);
            assert_eq!(home_activity(false, true, feed, &s), HomeActivity::Loading);
        }
        assert_eq!(
            home_activity(false, false, &all, &s),
            HomeActivity::Empty {
                title: s.empty_activity_title.clone(),
                caption: s.empty_activity_caption.clone(),
            }
        );
        assert_eq!(
            home_activity(false, false, &narrowed, &s),
            HomeActivity::Empty {
                title: s.empty_activity_network.clone(),
                caption: SharedString::default(),
            }
        );
        assert_ne!(s.empty_activity_title, s.empty_activity_network);
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
        let rows = activity_rows(
            &view,
            &strings(),
            &crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env()),
            false,
        );
        assert_eq!(rows.len(), 2, "the header is not a row here");
        assert_eq!(rows[0].unit, SharedString::from("POL"));
        assert_eq!(rows[1].unit, SharedString::from("USDT"));
        // …it rides on the first item of its day, and only there (078 H-04).
        assert!(rows[0].day.is_some(), "the day opens on its first row");
        assert_eq!(rows[1].day, None);
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
        let rows = activity_rows(
            &view,
            &strings(),
            &crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env()),
            false,
        );
        assert_eq!(rows[0].amount, SharedString::from("\u{2212}2"));
        assert!(!rows[0].positive);
        assert_eq!(rows[1].amount, SharedString::from("+120"));
        assert!(rows[1].positive);
    }

    /// A dApp's row (083 H2, spec 093): titled by the core's headline verb in
    /// the sheet's words — the descriptor's text when the wallet has no word
    /// for it — "on" the place the core named, or the verb alone when it
    /// named none; with no figure when it moved no coin.
    #[test]
    fn a_dapp_row_is_titled_by_its_verb_and_its_place() {
        use vela_core::app::clear_signing::ClearTerm;

        let dapp =
            |id: &str, place: Option<&str>, intent: Option<&str>, term: Option<ClearTerm>| {
                let mut row = item(id, false, None, "");
                row.decimals = None;
                let mut dapp = dapp_of(Some("app.uniswap.org"), intent, term);
                dapp.place = place.map(str::to_owned);
                row.dapp = Some(dapp);
                FeedRow::Item { item: row }
            };
        let view = feed_with(
            vec![
                dapp("blind", None, None, Some(ClearTerm::IntentContractCall)),
                dapp(
                    "swap",
                    Some("Uniswap"),
                    Some("Swap"),
                    Some(ClearTerm::IntentSwap),
                ),
                dapp("odd", Some("app.uniswap.org"), Some("Frobnicate"), None),
            ],
            Vec::new(),
        );
        let s = strings();
        let rows = activity_rows(
            &view,
            &s,
            &crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env()),
            false,
        );
        let on = |place: &str, verb: &str| {
            crate::wallet::fill(
                &crate::wallet::fill(&s.dapp_row_title, "place", place),
                "intent",
                verb,
            )
        };
        assert_eq!(rows[0].title, s.intent_contract_call);
        assert_eq!(rows[0].amount.as_ref(), "", "no coin moved, no figure");
        assert_eq!(rows[0].unit.as_ref(), "");
        let swap = s
            .terms
            .get(&ClearTerm::IntentSwap)
            .cloned()
            .unwrap_or_default();
        assert_eq!(rows[1].title.to_string(), on("Uniswap", &swap));
        // A descriptor's word with no translation is still better than none.
        assert_eq!(
            rows[2].title.to_string(),
            on("app.uniswap.org", "Frobnicate")
        );
        // Every headline word the core can name has a word here.
        for term in ClearTerm::HEADLINES {
            assert!(s.terms.contains_key(&term), "{term:?}");
        }
    }

    /// A counterparty that is not ASCII is drawn, never a crash (083 H2
    /// review). A page could put any text in a batch's top-level `to`; cut at
    /// byte 6, `日本語日本語` panicked on every launch while its record stayed on
    /// the disk. The core now names no such recipient — this is the shell's
    /// own half of the promise, for whatever else reaches it.
    #[test]
    fn a_counterparty_that_is_not_ascii_is_drawn_not_a_crash() {
        let row = |id: &str, to: &str, site: Option<&str>| {
            let mut row = item(id, false, None, "");
            row.decimals = None;
            row.counterparty = Some(to.to_owned());
            row.dapp = Some(dapp_of(site, None, None));
            // The second line as the core would word a recipient it named.
            row.subtitle = match site {
                Some(site) => vec![FeedLine::Site {
                    site: site.to_owned(),
                }],
                None => vec![FeedLine::To {
                    address: to.to_owned(),
                    name: None,
                }],
            };
            FeedRow::Item { item: row }
        };
        let view = feed_with(
            vec![
                row("site", "日本語日本語", Some("app.uniswap.org")),
                row("bare", "日本語日本語日本語日本語日本語", None),
            ],
            Vec::new(),
        );
        let s = strings();
        let rows = activity_rows(
            &view,
            &s,
            &crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env()),
            false,
        );
        assert_eq!(rows[0].subtitle.as_ref(), "app.uniswap.org");
        assert_eq!(
            rows[1].subtitle.to_string(),
            crate::wallet::fill(&s.to_name, "name", "日本語日本語…語日本語")
        );
        // By character, never by byte — and an address reads as it did.
        assert_eq!(super::shorten_address("日本語日本語"), "日本語日本語");
        assert_eq!(
            super::shorten_address("0x日本語日本語日本語日本語日本語"),
            "0x日本語日…語日本語"
        );
        assert_eq!(
            super::shorten_address("0xAbCd000000000000000000000000000000000001"),
            "0xAbCd…0001"
        );
        assert_eq!(super::shorten_address(""), "");
    }

    /// A dApp call that moved no coin has no figure to mask: privacy draws
    /// nothing there rather than "••••", which would claim one (083 H2
    /// review). A figure beside it is still masked.
    #[test]
    fn a_row_with_no_figure_is_not_masked() {
        let mut call = item("call", false, None, "");
        call.decimals = None;
        call.dapp = Some(dapp_of(Some("app.uniswap.org"), Some("Swap"), None));
        let mut send = item("send", false, Some("0.01"), "xDAI");
        send.dapp = call.dapp.clone();
        let view = feed_with(
            vec![FeedRow::Item { item: call }, FeedRow::Item { item: send }],
            Vec::new(),
        );
        let rows = activity_rows(
            &view,
            &strings(),
            &crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env()),
            true,
        );
        assert_eq!(rows[0].amount.as_ref(), "");
        assert_eq!(rows[0].unit.as_ref(), "");
        assert_eq!(
            rows[1].amount,
            SharedString::from(crate::wallet::fixtures::MASK)
        );
        assert_eq!(rows[1].unit.as_ref(), "xDAI");
    }

    /// 083 F1: a swap's row says what left — the figure the core took from
    /// the sheet's simulation — and, under it, the one coin expected back,
    /// marked "≈" because the chain may deliver another amount. The figure
    /// itself says "≈" when the core calls it the simulation's (an outflow it
    /// measured) and reads bare when it is what the wallet sent (083 F1
    /// review). Privacy masks both digits and keeps both units; a row with
    /// nothing expected back draws no second line.
    #[test]
    fn a_swap_row_says_what_left_and_what_was_expected_back() {
        use vela_core::app::activity_feed::{FeedDapp, FeedDappChange};

        let mut swap = item("swap", false, Some("0.1"), "USDC");
        swap.decimals = Some(6);
        swap.dapp = Some(FeedDapp {
            received: Some(FeedDappChange {
                direction: FeedDirection::In,
                verified: true,
                symbol: "ETH".to_owned(),
                value: Some("0.000037".to_owned()),
                decimals: Some(18),
                exact: false,
            }),
            estimated: true,
            contract_call: true,
            ..dapp_of(Some("app.uniswap.org"), None, None)
        });
        let mut call = swap.clone();
        call.id = "call".to_owned();
        if let Some(dapp) = call.dapp.as_mut() {
            dapp.received = None;
            // The call's own value: what the wallet sent, not an estimate.
            dapp.estimated = false;
        }
        let view = feed_with(
            vec![FeedRow::Item { item: swap }, FeedRow::Item { item: call }],
            Vec::new(),
        );
        let s = strings();
        let flow = crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env());
        let rows = activity_rows(&view, &s, &flow, false);
        assert_eq!(rows[0].title, s.intent_contract_call);
        assert_eq!(
            (rows[0].amount.as_ref(), rows[0].unit.as_ref()),
            ("≈ \u{2212}0.1", "USDC")
        );
        assert!(!rows[0].positive, "money out is plain ink");
        assert_eq!(
            rows[0].received.as_ref().map(AsRef::as_ref),
            Some("≈ +0.000037 ETH")
        );
        assert_eq!(rows[1].received, None);
        assert_eq!(
            rows[1].amount.as_ref(),
            "\u{2212}0.1",
            "what was sent reads bare"
        );

        let masked = activity_rows(&view, &s, &flow, true);
        assert_eq!(masked[0].amount.as_ref(), crate::wallet::fixtures::MASK);
        assert_eq!(masked[0].unit.as_ref(), "USDC");
        assert_eq!(
            masked[0].received.as_ref().map(AsRef::as_ref),
            Some("≈ +•••• ETH")
        );
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
        let rows = activity_rows(
            &view,
            &strings(),
            &crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env()),
            false,
        );
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
        let rows = activity_rows(
            &view,
            &strings(),
            &crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env()),
            true,
        );
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
            activity_rows(
                &view,
                &strings(),
                &crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env()),
                false
            )[0]
            .amount,
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

    /// The status line in the web's order (078 H-03): an unreachable chain by
    /// name first, then "still updating" in grey (it is not wrong, only not
    /// final), then unpriced; a hidden hero says nothing, and a skeleton only
    /// "unreachable".
    #[test]
    fn the_status_line_says_the_most_actionable_thing_first() {
        let s = strings();
        let money = Money::default();

        let mut down = view(Some(10.0));
        down.banner_chain_ids = vec![1];
        down.notice = Some(BalanceNotice::Unpriced);
        let model = balance(&down, &s, "en", &money);
        let Some((StatusKind::Warning, text)) = model.status else {
            panic!(
                "an unreachable chain is a warning: {:?}",
                model.status.map(|s| s.1)
            );
        };
        assert!(
            text.contains(&crate::executor::custom_tokens::network_name(1)),
            "the line names the chain: {text}"
        );
        down.banner_chain_ids = vec![1, 10];
        let (_, text) = balance(&down, &s, "en", &money).status.expect("a line");
        assert!(text.contains('2'), "several are counted: {text}");

        let mut updating = view(Some(10.0));
        updating.notice = Some(BalanceNotice::StillUpdating);
        assert!(matches!(
            balance(&updating, &s, "en", &money).status,
            Some((StatusKind::Refreshing, _))
        ));

        let mut unpriced = view(Some(10.0));
        unpriced.notice = Some(BalanceNotice::Unpriced);
        assert_eq!(
            balance(&unpriced, &s, "en", &money).status,
            Some((StatusKind::Warning, s.balance_unpriced.clone()))
        );

        let mut hidden = view(Some(10.0));
        hidden.hidden = true;
        hidden.banner_chain_ids = vec![1];
        assert!(balance(&hidden, &s, "en", &money).status.is_none());

        let mut loading = view(None);
        loading.refreshing = true;
        assert!(
            balance(&loading, &s, "en", &money).status.is_none(),
            "a skeleton is already 'still counting'"
        );
        loading.unreachable = true;
        assert!(matches!(
            balance(&loading, &s, "en", &money).status,
            Some((StatusKind::Warning, _))
        ));
    }

    /// SR3 splits the chains the way the web's does: rate-limited ones retry
    /// by themselves, unreachable ones offer Retry, and a chain in either
    /// list is not also counted as settled.
    #[test]
    fn the_breakdown_keeps_a_pending_chain_out_of_the_settled_ones() {
        let mut v = view(Some(10.0));
        v.rate_limited_chain_ids = vec![137];
        v.banner_chain_ids = vec![137, 10];
        let detail = balance_detail(&v, &strings(), "en", &Money::default());
        let pending: Vec<(u32, bool)> = detail
            .pending
            .iter()
            .map(|row| (row.chain_id, row.retry))
            .collect();
        assert_eq!(pending, vec![(137, false), (10, true)]);
        assert!(
            detail
                .done
                .iter()
                .all(|row| row.chain_id != 137 && row.chain_id != 10)
        );
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

    /// The hero's label names the code its figure is actually drawn in.
    ///
    /// It used to write `· USD` verbatim, so a wallet converted to ZAR read
    /// `总余额 · USD` over `ZAR 157.34` — the one line on the screen whose
    /// whole job is to say which money this is.
    #[test]
    fn the_hero_labels_the_currency_it_is_actually_drawn_in() {
        let priced = balance(
            &view(Some(100.0)),
            &strings(),
            "en-US",
            &Money::new("ZAR", Some(16.0)),
        );
        assert_eq!(priced.currency, "ZAR");
        assert!(
            priced.integer.contains("ZAR") || priced.integer.contains('R'),
            "and the figure agrees: {}",
            priced.integer
        );

        // No rate: the figure falls back to dollars, so the label does too —
        // the two must never disagree about which money is on screen.
        let unpriced = balance(
            &view(Some(100.0)),
            &strings(),
            "en-US",
            &Money::new("ZAR", None),
        );
        assert_eq!(unpriced.currency, "USD");
        assert!(unpriced.integer.contains('$'), "{}", unpriced.integer);

        // Even with nothing to draw, the label is still about a currency.
        assert_eq!(
            balance(
                &view(None),
                &strings(),
                "en-US",
                &Money::new("EUR", Some(0.9))
            )
            .currency,
            "EUR"
        );
    }

    /// The split is at the person's decimal mark, whatever their grouping.
    #[test]
    fn the_hero_splits_at_the_decimal_mark_it_was_given() {
        let split = |text: &str, mark: &str| {
            let (whole, minor) = split_at_mark(text, mark);
            (whole.to_string(), minor.map(|m| m.to_string()))
        };
        assert_eq!(
            split("$1,544.50", "."),
            ("$1,544".into(), Some("50".into()))
        );
        assert_eq!(
            split("1.234,56 €", ","),
            ("1.234".into(), Some("56 €".into()))
        );
        assert_eq!(
            split("1 234,56 €", ","),
            ("1 234".into(), Some("56 €".into()))
        );
        // A dot-grouped whole number has no minor units to split off.
        assert_eq!(split("1.234 €", ","), ("1.234 €".into(), None));
        assert_eq!(split("¥1,235", "."), ("¥1,235".into(), None));
    }

    #[test]
    fn splitting_survives_a_dot_grouped_locale() {
        let (integer, decimals) = split_fiat(1383.28, "de", &Money::default());
        assert!(
            integer.contains("1.383") || integer.contains("1,383"),
            "the whole part lost its grouping: {integer}"
        );
        // The minor units are at most two DIGITS; a symbol written after the
        // figure rides along with them.
        assert!(
            decimals
                .as_ref()
                .is_none_or(|d| d.chars().take_while(char::is_ascii_digit).count() <= 2),
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
                        kind: vela_core::app::activity_feed::FeedTxKind::Receive,
                        status: vela_core::app::activity_feed::FeedTxStatus::Confirmed,
                        site: None,
                        counterparty_role: Default::default(),
                        dapp: None,
                        subtitle: Vec::new(),
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
            // An ERC-20 names its contract, and keeps it whole for the copy;
            // the price row says there is none rather than going missing
            // (the web's `liveAssetDetail`, 078 H-06).
            assert!(
                mon.facts
                    .iter()
                    .any(|(label, _)| *label == s.label_contract)
            );
            assert!(mon.contract_copy.is_some());
            assert!(
                mon.facts
                    .iter()
                    .any(|(label, value)| *label == s.label_price && *value == s.no_price)
            );
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
            assert_eq!(chains[0].count, 3, "every holding, as the web counts");
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
                    SharedString::from(token_amount_text(&token.balance))
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
        // The holdings, as the web counts them (`liveChainRows`:
        // `view.tokens.length`) — every other row counts holdings too. This
        // counted the CHAINS, so one wallet read 23 on the web and 16 here.
        count: u32::try_from(view.tokens.len()).unwrap_or(u32::MAX),
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
    // Price is always a row — "No price" when there is none (the web's
    // `liveAssetDetail`, 078 H-06): a fact that disappears reads as a panel
    // that forgot it rather than a token nobody quotes.
    facts.push((
        s.label_price.clone(),
        match token.price_usd {
            Some(price) => SharedString::from(crate::wallet::fill(
                &crate::wallet::fill(&s.price_value, "symbol", &token.symbol),
                "value",
                &figure(price),
            )),
            None => s.no_price.clone(),
        },
    ));
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
                token_amount_text(&token.balance),
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
            .map(|item| activity_row(item, s, view.hidden))
            .collect(),
        // …and the id behind each, from the SAME walk, so row N opens
        // record N.
        activity_ids: own.iter().map(|item| item.id.clone()).collect(),
        explorer_url: token_explorer_url(token, view.address.as_deref()).map(SharedString::from),
        contract_copy: token.token_address.clone().map(SharedString::from),
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
/// `FeedView::rows` interleaves the core's day headers with the items. The
/// home used to drop them ("the mocks draw no headings") — the web files its
/// preview under its days (spec 038 #E3, 078 H-04), so a header now rides on
/// the first item of its day as `day`, and row N is still feed item N.
#[must_use]
pub fn activity_rows(
    view: &FeedView,
    s: &WalletStrings,
    flow: &crate::flows::FlowStrings,
    hidden: bool,
) -> Vec<ActivityRowModel> {
    let mut rows = Vec::new();
    let mut day = None;
    for row in &view.rows {
        match row {
            FeedRow::Header { day_start_ms, .. } => {
                day = Some(crate::flows::live::day_label(*day_start_ms, flow));
            }
            FeedRow::Item { item } => {
                let mut model = activity_row(item, s, hidden);
                model.day = day.take();
                rows.push(model);
            }
        }
    }
    rows
}

/// What the home's Activity strip shows (spec 082 RD10, RG5, G1).
#[derive(Clone, Debug, PartialEq)]
pub enum HomeActivity {
    /// Rows to draw.
    Rows,
    /// Nothing yet, and the balance core has not ruled: skeletons, never
    /// "nothing happened" before anyone looked.
    Loading,
    /// Nothing, and it is known: the core's empty line — the network's own
    /// when the sidebar narrows to one chain, with no caption then.
    Empty {
        title: SharedString,
        caption: SharedString,
    },
}

/// The home Activity's state. Which empty line is the core's
/// (`FeedView.home_empty_key`, from the filter the page told it); loading
/// versus empty is the shell's, from `BalanceView.balance_unknown` — the
/// iPhone's rows / loading / empty (`WalletLive.swift:88-103`).
#[must_use]
pub fn home_activity(
    has_rows: bool,
    balance_unknown: bool,
    feed: &FeedView,
    s: &WalletStrings,
) -> HomeActivity {
    if has_rows {
        return HomeActivity::Rows;
    }
    if balance_unknown {
        return HomeActivity::Loading;
    }
    match feed.home_empty_key.as_str() {
        "home.emptyNoActivityNetwork" => HomeActivity::Empty {
            title: s.empty_activity_network.clone(),
            caption: SharedString::default(),
        },
        _ => HomeActivity::Empty {
            title: s.empty_activity_title.clone(),
            caption: s.empty_activity_caption.clone(),
        },
    }
}

/// One feed item as a row (spec 082 RG1, RG2; spec 093) — everything it says
/// is the core's: what it is (`FeedItem.kind`; a row with `dapp` is a dApp's
/// transaction or signature), its title (`dapp_title`), its second line
/// (`FeedItem.subtitle`, worded by `subtitle_text`) and its figure — the
/// money it moved, or the allowance a grant states. The desktop's own
/// subtitle rule is gone.
pub(crate) fn activity_row(item: &FeedItem, s: &WalletStrings, hidden: bool) -> ActivityRowModel {
    let incoming = item.direction == FeedDirection::In;
    let kind = match (item.dapp.is_some(), item.kind) {
        (true, _)
        | (false, FeedTxKind::DappTx | FeedTxKind::SignMessage | FeedTxKind::SignTypedData) => {
            ActivityKind::Dapp
        }
        (false, FeedTxKind::Receive) => ActivityKind::Received,
        (false, FeedTxKind::Send) => ActivityKind::Sent,
        // Connections never become rows (the core's `accept()`); should one
        // arrive, it is drawn by its direction.
        (false, FeedTxKind::Connect) => {
            if incoming {
                ActivityKind::Received
            } else {
                ActivityKind::Sent
            }
        }
    };
    // A grant states its allowance where a figure would be (spec 093) — the
    // core sends one only on a row that moved no money of its own.
    let allowance = item.dapp.as_ref().and_then(|dapp| dapp.allowance.as_ref());
    let (amount, unit) = match allowance {
        Some(allowance) => allowance_figure(allowance, s, hidden),
        // Privacy masks the FIGURE and keeps the unit — H5's rule, and the
        // same mask the hero uses, because a leak in one surface defeats it
        // everywhere (the core's invariant ④ on the balance side). A row with
        // no figure has nothing to mask, and "••••" would claim one.
        None => (
            if moved_nothing(item) {
                SharedString::from("")
            } else if hidden {
                SharedString::from(crate::wallet::fixtures::MASK)
            } else {
                amount_text(item, incoming)
            },
            SharedString::from(item.symbol.clone()),
        ),
    };

    ActivityRowModel {
        kind,
        title: match (kind, item.dapp.as_ref()) {
            (_, Some(dapp)) => dapp_title(dapp, s),
            (ActivityKind::Sent, None) => s.label_sent.clone(),
            (ActivityKind::Received, None) => s.label_received.clone(),
            (ActivityKind::Dapp, None) => s.label_dapp.clone(),
        },
        subtitle: subtitle_text(&item.subtitle, s),
        // The chain this happened on, drawn as its logo where the endpoint has
        // one (§8.3; issue 201): the badge used to be a colour, and a colour
        // is not a network anyone can name.
        badge_logo: crate::marks::chain_logo_url(item.chain_id),
        amount,
        unit,
        positive: incoming,
        // An allowance with no limit is the one standing risk a row states.
        danger: allowance.is_some_and(|allowance| allowance.unlimited),
        badge: badge(item.chain_id),
        day: None,
        // A swap's one coin back, beside what left (083 F1) — the sheet's
        // expectation, so it says "≈", and masked like the figure.
        received: item
            .dapp
            .as_ref()
            .and_then(|dapp| dapp.received.as_ref())
            .map(|change| {
                SharedString::from(format!(
                    "{} {}",
                    change_figure(change, hidden),
                    change.symbol
                ))
            }),
    }
}

/// A row's second line (spec 093): the core's parts, in its order, each in
/// the reader's words and joined " · " — a status in the feed's status words
/// ("处理中 · …": a pending or failed operation must never read like one
/// that landed), whom a transfer went to or came from (a name, else the
/// short address), a site verbatim, a network by its name, a day as the
/// history's headers say it.
pub(crate) fn subtitle_text(lines: &[FeedLine], s: &WalletStrings) -> SharedString {
    let parts: Vec<String> = lines
        .iter()
        .filter_map(|line| match line {
            FeedLine::Status { status } => match status {
                FeedTxStatus::Pending => Some(s.status_pending.to_string()),
                FeedTxStatus::Failed => Some(s.status_failed.to_string()),
                FeedTxStatus::Unknown => Some(s.status_unknown.to_string()),
                // The core never says it: a confirmed row leads with nothing.
                FeedTxStatus::Confirmed => None,
            },
            FeedLine::To { address, name } => Some(crate::wallet::fill(
                &s.to_name,
                "name",
                &name.clone().unwrap_or_else(|| shorten_address(address)),
            )),
            FeedLine::From { address, name } => Some(crate::wallet::fill(
                &s.from_name,
                "name",
                &name.clone().unwrap_or_else(|| shorten_address(address)),
            )),
            FeedLine::Site { site } => Some(site.clone()),
            FeedLine::Network { chain_id } => Some(crate::flows::live::chain_name(*chain_id)),
            // A contact's rows have no day headers: the day is on the line,
            // worded as the history's headers word it.
            FeedLine::Day { day_start_ms } => Some(
                crate::flows::live::day_word(*day_start_ms, &s.today, &s.yesterday).to_string(),
            ),
        })
        .collect();
    SharedString::from(parts.join(" · "))
}

/// A grant's allowance as a figure and its unit (spec 093): the sheet's own
/// "Unlimited" — never masked, it is a standing risk and not a balance — or
/// the cap in the person's number format, masked like any figure when
/// balances are hidden. The unit is the token's symbol (empty when nobody
/// could name it).
pub(crate) fn allowance_figure(
    allowance: &FeedAllowance,
    s: &WalletStrings,
    hidden: bool,
) -> (SharedString, SharedString) {
    let figure = if allowance.unlimited {
        s.unlimited_value.clone()
    } else if hidden {
        SharedString::from(crate::wallet::fixtures::MASK)
    } else {
        let value = allowance.value.as_deref().unwrap_or_default();
        SharedString::from(value.parse::<f64>().map_or_else(
            |_| value.to_owned(),
            |amount| {
                format_token_amount(
                    amount,
                    crate::executor::format_prefs::current().number,
                    false,
                )
            },
        ))
    };
    (figure, SharedString::from(allowance.symbol.clone()))
}

/// [`allowance_figure`] as one phrase — "无限额 USDC", "100 USDC" — for the
/// detail's header and its spending-cap fact.
pub(crate) fn allowance_text(
    allowance: &FeedAllowance,
    s: &WalletStrings,
    hidden: bool,
) -> SharedString {
    let (figure, unit) = allowance_figure(allowance, s, hidden);
    SharedString::from(format!("{figure} {unit}").trim().to_owned())
}

/// One of a dApp transaction's balance changes as a figure (083 F1), in the
/// signing sheet's own form with one addition: "≈" on every figure the wallet
/// cannot vouch for — "≈ +0.000037" expected back (slippage), "≈ −0.1" for
/// an outflow the simulation measured (an exact-output swap may spend
/// another amount, and nothing says which kind was signed; 083 F1 review).
/// Only the coin the transaction itself sent reads bare ("−0.0001"). A token
/// the sheet could not verify keeps its direction and never a number ("+"),
/// as it did on the sheet. Privacy masks the digits and keeps the rest.
pub(crate) fn change_figure(
    change: &vela_core::app::activity_feed::FeedDappChange,
    hidden: bool,
) -> String {
    let incoming = change.direction == FeedDirection::In;
    let sign = if incoming { "+" } else { "\u{2212}" };
    let figure = change
        .value
        .as_deref()
        .and_then(|value| value.parse::<f64>().ok())
        .filter(|_| change.verified);
    let Some(amount) = figure else {
        return sign.to_owned();
    };
    let digits = if hidden {
        crate::wallet::fixtures::MASK.to_owned()
    } else {
        format_token_amount(
            amount,
            crate::executor::format_prefs::current().number,
            false,
        )
    };
    if change.exact {
        format!("{sign}{digits}")
    } else {
        format!("≈ {sign}{digits}")
    }
}

/// What a dApp interaction did and where, as a title (083 H2, spec 093):
/// the core's headline verb in the reader's words — the signing sheet's own
/// (`componentsUi.signing.<intent_term>`), else the descriptor's text — and,
/// when the core names a place (a protocol it knows, else the site),
/// "{verb} on {place}" (`history.dappRowTitle`; 「在 Uniswap 兑换」).
pub(crate) fn dapp_title(
    dapp: &vela_core::app::activity_feed::FeedDapp,
    s: &WalletStrings,
) -> SharedString {
    let verb = dapp
        .intent_term
        .and_then(|term| s.terms.get(&term).cloned())
        .or_else(|| dapp.intent.clone().map(SharedString::from))
        .unwrap_or_else(|| s.intent_contract_call.clone());
    match dapp.place.as_deref() {
        // The place first: a host or a built-in protocol name carries no
        // braces, while a descriptor's verb is text somebody else wrote.
        Some(place) => SharedString::from(crate::wallet::fill(
            &crate::wallet::fill(&s.dapp_row_title, "place", place),
            "intent",
            &verb,
        )),
        None => verb,
    }
}

/// `+120` / `−2`. The minus is U+2212, not a hyphen — the mocks use it and it
/// is what aligns under a digit.
/// The same signed amount the feed row shows, mask included.
///
/// Privacy masks the FIGURE and keeps the unit, on every surface together —
/// the detail panel is one of them, and reading a second flag is how one ends
/// up out of step.
/// A token amount as every balance surface reads it — the asset list rows, a
/// token's page, the Send picker and card, the confirm and the receipt — ONE
/// rule on all four shells: the core's ladder (`send::max_figure`, the very
/// figure `Max` writes — 6 places under 1, 4 under 1000, 2 above, half up), in
/// the person's decimal mark and UNGROUPED like the field `Max` fills, so a
/// balance and the Max taken from it read the same digits (web
/// `tokenAmountText`, iOS `WalletLive.tokenAmountText`).
#[must_use]
pub fn token_amount_text(value: &str) -> String {
    with_decimal_mark(vela_core::app::send::max_figure(value))
}

/// The same ladder cut DOWN — for a ceiling the person may type back ("you
/// can send up to", what is left): rounded up, it would be a figure the
/// balance cannot cover.
#[must_use]
pub fn token_amount_text_down(value: &str) -> String {
    let exact = value.trim();
    let (int_raw, frac_raw) = exact.split_once('.').unwrap_or((exact, ""));
    if !int_raw.bytes().all(|b| b.is_ascii_digit()) || !frac_raw.bytes().all(|b| b.is_ascii_digit())
    {
        return exact.to_owned();
    }
    let int_digits = int_raw.trim_start_matches('0');
    let places = match int_digits.len() {
        0 => 6,
        1..=3 => 4,
        _ => 2,
    };
    let cut: String = frac_raw.chars().take(places).collect();
    // Below the last place, the half-up rule's two significant digits are
    // already a cut; anything else is the truncation itself.
    if int_digits.is_empty() && cut.trim_end_matches('0').is_empty() {
        return token_amount_text(exact);
    }
    let int_part = if int_digits.is_empty() {
        "0"
    } else {
        int_digits
    };
    let frac = cut.trim_end_matches('0');
    with_decimal_mark(if frac.is_empty() {
        int_part.to_owned()
    } else {
        format!("{int_part}.{frac}")
    })
}

fn with_decimal_mark(figure: String) -> String {
    let decimal = crate::executor::format_prefs::current()
        .number
        .separators()
        .decimal;
    if decimal == "." {
        figure
    } else {
        figure.replace('.', decimal)
    }
}

pub(crate) fn amount_text_of(item: &FeedItem, incoming: bool, hidden: bool) -> SharedString {
    // A dApp call that moved no coin has no figure (083 H2) — and nothing to
    // mask either: "••••" would say there is one (083 H2 review).
    if moved_nothing(item) {
        return SharedString::from("");
    }
    if hidden {
        return SharedString::from(crate::wallet::fixtures::MASK);
    }
    let amount = amount_text(item, incoming);
    // Nothing, not a stray space where a figure would be.
    if amount.is_empty() {
        return amount;
    }
    SharedString::from(format!("{amount} {}", item.symbol))
}

/// A dApp's transaction that moved no coin (083 H2): the core sends no
/// figure for it, and no surface draws one — or masks one. A multi-token
/// batch also has no single figure, but it has a total, and privacy keeps
/// masking that.
pub(crate) fn moved_nothing(item: &FeedItem) -> bool {
    item.dapp.is_some() && item.value.is_none()
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
    // A dApp row whose figure is the simulation's, not the value the wallet
    // sent (083 F1 review): what it expected, so it says so.
    let about = if item.dapp.as_ref().is_some_and(|dapp| dapp.estimated) {
        "≈ "
    } else {
        ""
    };
    SharedString::from(format!(
        "{about}{}{formatted}",
        if incoming { "+" } else { "\u{2212}" }
    ))
}

/// `0x1234…abcd`. Counted in characters, never bytes: what reaches here is
/// not always an address a wallet wrote — a dApp record's recipient, a
/// clear-signing field — and cutting text like `日本語日本語` at byte 6
/// panics, which took the desktop down on every launch while the row stayed
/// on the disk (083 H2 review). An address is ASCII, so it reads as before.
pub(crate) fn shorten_address(address: &str) -> String {
    let count = address.chars().count();
    if count <= 14 {
        return address.to_owned();
    }
    let head: String = address.chars().take(6).collect();
    let tail: String = address.chars().skip(count - 4).collect();
    format!("{head}…{tail}")
}
