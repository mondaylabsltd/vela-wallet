//! Rules of the send machine, one test per rule.
//!
//! Inventory invariants ①–⑮ each have at least one test named after the rule;
//! the pure-helper vectors mirror the TS jest suites (`batch-send.test.ts`,
//! `send-utils`, `fiat-convert`) and the reentry-lock contract (issue #91).
//!
//! Most tests drive the machine through [`support::DomainDriver`] exactly the
//! way the shell will: dispatch an event, answer the operations oldest-first.
//! A small local driver (`Flex`) exists solely for the out-of-order races the
//! FIFO driver cannot express (the 15s timeout firing before the estimate).

#![cfg(feature = "crux")]

mod support;

use std::collections::VecDeque;

use crux_core::{Core, Request};
use support::DomainDriver;
use vela_core::app::fee_policy::{to_base_units, FeeAssetView, FeeEstimateView, FeeTier};
use vela_core::app::money::{DenominatedAmount, TokenPrice};
use vela_core::app::send::{
    build_multi_token_calls, build_split_calls, duplicate_recipient_rows, is_valid_address,
    recipients_are_valid, split_row_issues, sum_split_base_units, Event, ReentryLock, Send,
    SendAccountRef, SendAddNetworkOutcome, SendAlertKind, SendAmountWarning, SendChainInfo,
    SendDisplayContext, SendEstimateFailure, SendFeeOutcome, SendHapticKind, SendHoldReason,
    SendLockError, SendOpenParams, SendOperation as Op, SendReceiptKind, SendReceiptOutcome,
    SendReceiptStatus, SendRecipientDraft, SendRowFieldState, SendScan, SendShellResult as Res,
    SendStage, SendSubmitFailure, SendTimerTag, SendToken, SendTokenMeta, SendTreasuryAsset,
    SendTreasuryProbe, SendTreasuryStatus, SendTxErrorKey, SendTxStatus, SendUnitIssue, SendView,
    BATCH_MAX_RECIPIENTS,
};

type Sut = DomainDriver<Send>;

const ACCOUNT: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const RECIPIENT: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const RECIPIENT_B: &str = "0xcccccccccccccccccccccccccccccccccccccccc";
const USDC: &str = "0x2222222222222222222222222222222222222222";
const DAI: &str = "0x3333333333333333333333333333333333333333";
const FEE_COLLECTOR: &str = "0x1111111111111111111111111111111111111111";
const PK: &str = "04deadbeef";
const HASH: &str = "0xhash";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn acct() -> SendAccountRef {
    SendAccountRef {
        id: "cred-1".to_owned(),
        address: ACCOUNT.to_owned(),
        name: Some("Ann".to_owned()),
    }
}

fn display() -> SendDisplayContext {
    SendDisplayContext {
        code: "USD".to_owned(),
        rate: Some(1.0),
        fiat_decimals: 2,
    }
}

/// A display currency that IS priced, and is not USD — the case where the
/// figure on screen and the rate must agree about which currency they mean.
fn cny_display() -> SendDisplayContext {
    SendDisplayContext {
        code: "CNY".to_owned(),
        rate: Some(7.17),
        fiat_decimals: 2,
    }
}

/// The display context for a currency NO source could price — the state
/// `display_currency` commits as `{code, None}`.
fn unpriced_display() -> SendDisplayContext {
    SendDisplayContext {
        code: "CNY".to_owned(),
        rate: None,
        fiat_decimals: 2,
    }
}

fn chains() -> Vec<SendChainInfo> {
    vec![
        SendChainInfo {
            chain_id: 1,
            network: "ethereum".to_owned(),
            native_symbol: "ETH".to_owned(),
        },
        SendChainInfo {
            chain_id: 137,
            network: "polygon".to_owned(),
            native_symbol: "POL".to_owned(),
        },
    ]
}

fn eth(balance: &str) -> SendToken {
    SendToken {
        network: "ethereum".to_owned(),
        chain_id: 1,
        symbol: "ETH".to_owned(),
        balance: balance.to_owned(),
        decimals: 18,
        token_address: None,
        price_usd: Some(2000.0),
        logo_urls: vec!["eth.png".to_owned()],
        spam: false,
    }
}

fn usdc(balance: &str) -> SendToken {
    SendToken {
        network: "ethereum".to_owned(),
        chain_id: 1,
        symbol: "USDC".to_owned(),
        balance: balance.to_owned(),
        decimals: 6,
        token_address: Some(USDC.to_owned()),
        price_usd: Some(1.0),
        logo_urls: vec![],
        spam: false,
    }
}

fn dai(balance: &str) -> SendToken {
    SendToken {
        network: "ethereum".to_owned(),
        chain_id: 1,
        symbol: "DAI".to_owned(),
        balance: balance.to_owned(),
        decimals: 18,
        token_address: Some(DAI.to_owned()),
        price_usd: Some(1.0),
        logo_urls: vec![],
        spam: false,
    }
}

/// The same asset on ANOTHER chain — the row a one-chain sweep must not take.
fn polygon_usdc(balance: &str) -> SendToken {
    SendToken {
        network: "polygon".to_owned(),
        chain_id: 137,
        symbol: "USDC".to_owned(),
        balance: balance.to_owned(),
        decimals: 6,
        token_address: Some(USDC.to_owned()),
        price_usd: Some(1.0),
        logo_urls: vec![],
        spam: false,
    }
}

fn loaded(tokens: Vec<SendToken>) -> Res {
    Res::TokensLoaded {
        tokens: Some(tokens),
        chains: chains(),
    }
}

fn native_fee(chain_id: u32, total_wei: u128) -> FeeEstimateView {
    FeeEstimateView {
        chain_id,
        total_wei: total_wei.to_string(),
        max_fee_per_gas: "1000".to_owned(),
        network_fee_per_gas: "1000000000".to_owned(),
        relayer_fee_per_gas: "0".to_owned(),
        bundler_gas_price: "1000000000".to_owned(),
        in_band_gas_basis: "1000000000".to_owned(),
        effective_gas_price: None,
        max_gas_price: None,
        total_gas: "450000".to_owned(),
        deployed: true,
        tier: FeeTier::Fast,
        quoted: true,
        fee_asset: FeeAssetView::Native,
        fee_recipient: Some(FEE_COLLECTOR.to_owned()),
    }
}

fn usdc_fee(chain_id: u32, amount: u128) -> FeeEstimateView {
    FeeEstimateView {
        chain_id,
        total_wei: "0".to_owned(),
        max_fee_per_gas: "0".to_owned(),
        network_fee_per_gas: "1000000000".to_owned(),
        relayer_fee_per_gas: "0".to_owned(),
        bundler_gas_price: "1000000000".to_owned(),
        in_band_gas_basis: "1000000000".to_owned(),
        effective_gas_price: None,
        max_gas_price: None,
        total_gas: "450000".to_owned(),
        deployed: true,
        tier: FeeTier::Fast,
        quoted: true,
        fee_asset: FeeAssetView::Erc20 {
            token: USDC.to_owned(),
            decimals: 6,
            amount: amount.to_string(),
            symbol: Some("USDC".to_owned()),
        },
        fee_recipient: Some(FEE_COLLECTOR.to_owned()),
    }
}

fn fee_ok(view: FeeEstimateView) -> Res {
    Res::FeeEstimated {
        outcome: SendFeeOutcome::Ok { estimate: view },
    }
}

fn treasury_status() -> SendTreasuryStatus {
    SendTreasuryStatus {
        chain_id: 1,
        address: FEE_COLLECTOR.to_owned(),
        asset: SendTreasuryAsset::Native,
        balance: "1".to_owned(),
        floor: "10".to_owned(),
        bootstrap_needed: true,
        // The shell never judges this; the core fills it when it publishes the
        // sheet (spec 060).
        operator_served: false,
    }
}

fn low_float() -> Res {
    Res::TreasuryProbed {
        probe: SendTreasuryProbe::LowFloat {
            status: treasury_status(),
        },
    }
}

fn covered() -> Res {
    Res::TreasuryProbed {
        probe: SendTreasuryProbe::Covered,
    }
}

fn credential(pk: Option<&str>) -> Res {
    Res::AccountCredential {
        public_key_hex: pk.map(str::to_owned),
    }
}

fn submitted(hash: &str) -> Res {
    Res::Submitted {
        user_op_hash: hash.to_owned(),
        now_ms: 1_754_000_000_500.0,
    }
}

fn open_event(params: SendOpenParams) -> Event {
    Event::Open {
        account: Some(acct()),
        params,
        display: display(),
    }
}

/// Mount and answer the token load.
fn boot(tokens: Vec<SendToken>) -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(open_event(SendOpenParams::default()));
    assert_eq!(
        ops,
        vec![Op::FetchTokens {
            address: ACCOUNT.to_owned()
        }]
    );
    let ops = sut.resolve(loaded(tokens));
    assert!(ops.is_empty(), "plain load routes nowhere: {ops:?}");
    sut
}

/// Select ETH and settle the credential prefetch — and the warm quote it
/// starts (spec 028 Phase 10). The warm-up is answered FAILED here so every
/// rule below still meets the form exactly as it did before the warm-up
/// existed: no fee in hand, the pipeline idle.
fn select_eth(sut: &mut Sut) {
    let ops = sut.dispatch(Event::SelectToken {
        token_id: eth("2").id(),
    });
    assert_eq!(
        ops,
        vec![Op::LoadAccountCredential {
            account_id: "cred-1".to_owned()
        }]
    );
    settle_warm_quote(sut);
}

/// The credential lands, the warm quote is asked, and is refused — the
/// swallowed failure that leaves the model as a plain selection would.
fn settle_warm_quote(sut: &mut Sut) {
    let ops = sut.resolve(credential(Some(PK)));
    assert!(
        matches!(
            ops.as_slice(),
            [Op::EstimateFee {
                tx: None,
                batch: None,
                ..
            }]
        ),
        "a picked token warms a transfer-sized quote: {ops:?}"
    );
    let ops = sut.resolve(Res::FeeEstimated {
        outcome: SendFeeOutcome::Failed {
            kind: SendEstimateFailure::QuoteUnavailable,
        },
    });
    assert!(ops.is_empty(), "a refused warm-up is swallowed: {ops:?}");
}

/// Select USDC (6 decimals, priced at 1 USD) and settle the credential
/// prefetch — the token the CNY overpayment was originally reported against.
fn select_usdc(sut: &mut Sut) {
    let ops = sut.dispatch(Event::SelectToken {
        token_id: usdc("9000").id(),
    });
    assert_eq!(
        ops,
        vec![Op::LoadAccountCredential {
            account_id: "cred-1".to_owned()
        }]
    );
    settle_warm_quote(sut);
}

fn set_recipient(sut: &mut Sut, addr: &str) {
    let ops = sut.dispatch(Event::SetRecipient {
        recipient: addr.to_owned(),
    });
    if ops
        .iter()
        .any(|op| matches!(op, Op::ResolveIdentity { .. }))
    {
        assert!(sut
            .resolve(Res::IdentityResolved { identity: None })
            .is_empty());
    }
}

/// The form's own quote (spec 028 Phase 9, T490) arms a debounce whenever the
/// form is complete. Runs that walk the pre-check's FIFO drop it unanswered —
/// the timer never fires in those runs, exactly like the 15s one — so the
/// next `resolve` answers the pre-check, not the debounce.
fn drain_form_quote(sut: &mut Sut) {
    sut.drop_matching(|op| {
        matches!(
            op,
            Op::StartTimer {
                tag: SendTimerTag::FormEstimate,
                ..
            }
        )
    });
}

/// `ops` without the form quote's debounce, for assertions about the rest.
fn without_form_quote(ops: Vec<Op>) -> Vec<Op> {
    ops.into_iter()
        .filter(|op| {
            !matches!(
                op,
                Op::StartTimer {
                    tag: SendTimerTag::FormEstimate,
                    ..
                }
            )
        })
        .collect()
}

/// Run Continue to a settled confirm step with the given estimate.
fn continue_to_confirm(sut: &mut Sut, fee: FeeEstimateView) {
    let ops = sut.dispatch(Event::Continue);
    drain_form_quote(sut);
    assert!(
        matches!(
            ops.as_slice(),
            [
                Op::EstimateFee { .. },
                Op::ProbeTreasury { .. },
                Op::StartTimer {
                    ms: 15_000,
                    tag: SendTimerTag::EstimateTimeout
                }
            ]
        ),
        "pre-check trio expected, got {ops:?}"
    );
    assert!(
        sut.resolve(fee_ok(fee)).is_empty(),
        "waits for the treasury"
    );
    let probes = sut.resolve(covered());
    // Entering confirm starts the risk/sim probes (recipient-dependent).
    sut.drop_oldest(); // the 15s timer never fires in this run
    for op in probes {
        match op {
            Op::ResolveRisk { .. } => {
                assert!(sut.resolve(Res::RiskResolved { risk: None }).is_empty());
            }
            Op::SimulateCalls { .. } => {
                assert!(sut.resolve(Res::SimResolved { sim_json: None }).is_empty());
            }
            other => panic!("unexpected confirm probe {other:?}"),
        }
    }
    assert_eq!(sut.view().stage, SendStage::Confirm);
}

/// Slide through the pre-sign treasury recheck into the in-flight submit.
fn slide_to_submit(sut: &mut Sut) -> Op {
    let ops = sut.dispatch(Event::SlideConfirm);
    assert!(
        matches!(ops.as_slice(), [Op::ProbeTreasury { .. }]),
        "pre-sign treasury recheck expected, got {ops:?}"
    );
    let ops = sut.resolve(covered());
    assert_eq!(ops.len(), 1, "exactly one submit: {ops:?}");
    let op = ops.into_iter().next().unwrap();
    assert!(matches!(op, Op::SubmitUserOp { .. }), "got {op:?}");
    op
}

/// Settle the post-submit persistence chain, asserting invariant ⑥ ordering.
fn settle_persistence(sut: &mut Sut) -> (Vec<String>, Op) {
    // Haptic, ClearTokenCache, PersistTxRecords — oldest first.
    assert!(sut.resolve(Res::HapticPlayed).is_empty());
    assert!(sut.resolve(Res::TokenCacheCleared).is_empty());
    let ops = sut.resolve(Res::RecordsPersisted);
    assert_eq!(ops.len(), 1, "tracker hand-off after persist: {ops:?}");
    let op = ops.into_iter().next().unwrap();
    let ids = match &op {
        Op::TrackSubmitted { record_ids, .. } => record_ids.clone(),
        other => panic!("expected TrackSubmitted, got {other:?}"),
    };
    (ids, op)
}

// ---------------------------------------------------------------------------
// A flexible driver for out-of-order races (timer before estimate)
// ---------------------------------------------------------------------------

struct Flex {
    core: Core<Send>,
    pending: VecDeque<Request<Op>>,
}

impl Flex {
    fn new() -> Self {
        Self {
            core: Core::new(),
            pending: VecDeque::new(),
        }
    }

    fn dispatch(&mut self, event: Event) -> Vec<Op> {
        let effects = self.core.process_event(event);
        self.collect(effects)
    }

    /// Answer the oldest outstanding operation matching `predicate`.
    fn resolve_where(&mut self, predicate: impl Fn(&Op) -> bool, result: Res) -> Vec<Op> {
        let index = self
            .pending
            .iter()
            .position(|request| predicate(&request.operation))
            .expect("no outstanding operation matches");
        let mut request = self.pending.remove(index).expect("index from position()");
        match self.core.resolve(&mut request, result) {
            Ok(effects) => self.collect(effects),
            Err(_) => Vec::new(),
        }
    }

    fn view(&self) -> SendView {
        self.core.view()
    }

    fn collect(&mut self, effects: Vec<vela_core::app::send::SendEffect>) -> Vec<Op> {
        let mut operations = Vec::new();
        for effect in effects {
            if let Some(request) = effect.into_shell() {
                operations.push(request.operation.clone());
                self.pending.push_back(request);
            }
        }
        operations
    }
}

// ===========================================================================
// Ported pure helpers
// ===========================================================================

/// What the shell now does at every call site, spelled out: the figure names
/// the currency it was typed in, the price names the currency it is quoted in,
/// and the two are compared.
///
/// This used to be `resolve_token_amount(amount, in_fiat, ..)` — a free
/// function with no code parameter, which therefore had to label both halves
/// with the same `const ANY: &str = ""`. That made the comparison `"" == ""`,
/// true always, so the one guard that catches a figure meeting another
/// currency's price was switched off for every caller of the only helper
/// anybody called. The helper is gone; this fixture is what replaced it, and
/// the code is a parameter now.
fn token_units(
    amount: &str,
    fiat_code: Option<&str>,
    price_usd: Option<f64>,
    decimals: u32,
    rate: Option<f64>,
    quoted_in: &str,
) -> String {
    let figure = match fiat_code {
        Some(code) => DenominatedAmount::fiat(amount, code),
        None => DenominatedAmount::token(amount),
    };
    figure.to_token_units(
        TokenPrice::new(price_usd, rate, quoted_in).as_ref(),
        decimals,
    )
}

/// The half of the guard the placeholder used to disable: a figure typed in one
/// currency is not converted by another currency's price. `display_changed` can
/// swap the whole context in a single event, so this pairing is reachable.
///
/// Mutation proof: drop the `p.code() == code` filter in
/// `DenominatedAmount::to_token_units` and the first line below returns
/// "697.35007" — 5000 CNY paid out as if it were 5000 USD.
#[test]
fn a_figure_is_never_converted_by_another_currencys_price() {
    assert_eq!(
        token_units("5000", Some("CNY"), Some(1.0), 6, Some(7.17), "USD"),
        "0"
    );
    assert_eq!(
        token_units("5000", Some("USD"), Some(1.0), 6, Some(7.17), "CNY"),
        "0"
    );
    // Same currency on both halves: the conversion happens.
    assert_eq!(
        token_units("5000", Some("CNY"), Some(1.0), 6, Some(7.17), "CNY"),
        "697.35007"
    );
}

#[test]
fn resolve_token_amount_passes_token_mode_through_untouched() {
    assert_eq!(
        token_units("1.5", None, Some(2000.0), 18, Some(1.0), "USD"),
        "1.5"
    );
    // …and keeps passing it through when nothing can price the token, because
    // a token-denominated figure needs no conversion at all.
    assert_eq!(token_units("1.5", None, None, 18, Some(1.0), "USD"), "1.5");
    assert_eq!(token_units("1.5", None, Some(0.0), 18, None, "USD"), "1.5");
    // Not even a price quoted in a currency the figure has never heard of.
    assert_eq!(
        token_units("1.5", None, Some(2000.0), 18, Some(7.17), "CNY"),
        "1.5"
    );
}

/// An unpriced TOKEN is the same refusal as an unpriceable CURRENCY: both are
/// a missing factor in `price_usd × rate`, and a fiat figure has no token twin
/// without both. This used to pass "7" straight through — so a price feed that
/// dropped ETH while "7" was on screen in USD turned it into 7 whole ETH.
///
/// Mutation proof: make `DenominatedAmount::to_token_units` fall back to
/// `self.value` when the price is absent and every assertion here flips to "7".
#[test]
fn resolve_token_amount_refuses_an_unpriced_token_in_fiat_mode() {
    assert_eq!(
        token_units("7", Some("USD"), None, 18, Some(1.0), "USD"),
        "0"
    );
    assert_eq!(
        token_units("7", Some("USD"), Some(0.0), 18, Some(1.0), "USD"),
        "0"
    );
    assert_eq!(
        token_units("7", Some("USD"), Some(f64::NAN), 18, Some(1.0), "USD"),
        "0"
    );
}

#[test]
fn resolve_token_amount_divides_fiat_by_the_display_price() {
    // $70 at $7/token → 10 tokens.
    assert_eq!(
        token_units("70", Some("USD"), Some(7.0), 18, Some(1.0), "USD"),
        "10"
    );
    // Display rate 2: 70 fiat = $35 → 5 tokens.
    assert_eq!(
        token_units("70", Some("XXX"), Some(7.0), 18, Some(2.0), "XXX"),
        "5"
    );
    // Garbage / non-positive fiat → '0'.
    assert_eq!(
        token_units("abc", Some("USD"), Some(7.0), 18, Some(1.0), "USD"),
        "0"
    );
    assert_eq!(
        token_units("", Some("USD"), Some(7.0), 18, Some(1.0), "USD"),
        "0"
    );
}

/// An unknown display rate converts NOTHING in fiat mode — and costs token
/// mode nothing at all.
///
/// The tempting shape is `rate.unwrap_or(1.0)`, which is also what
/// `token_price_in_fiat`'s own `else { 1.0 }` branch does if it is ever
/// reached with 0. Either way a "5000" typed in an unpriceable CNY resolves to
/// 5000 whole tokens — the batch importer's 7x payout, one screen over.
///
/// Mutation proof: change the `let Some(rate) = rate.filter(..)` guard to
/// `rate.unwrap_or(1.0)` and the fiat lines below return "5000" / "714.28...";
/// delete the `is_finite()`/`> 0.0` filter and the 0.0 and NaN lines do the
/// same. The token-mode lines stay green under every one of those mutations,
/// which is the point: the refusal is narrow.
#[test]
fn resolve_token_amount_refuses_an_unknown_display_rate() {
    // 5000 CNY of USDT, with nothing able to price CNY.
    assert_eq!(
        token_units("5000", Some("CNY"), Some(1.0), 6, None, "CNY"),
        "0"
    );
    // A source that answered nonsense is no better than one that failed.
    assert_eq!(
        token_units("5000", Some("CNY"), Some(1.0), 6, Some(0.0), "CNY"),
        "0"
    );
    assert_eq!(
        token_units("5000", Some("CNY"), Some(1.0), 6, Some(-7.17), "CNY"),
        "0"
    );
    assert_eq!(
        token_units("5000", Some("CNY"), Some(1.0), 6, Some(f64::NAN), "CNY"),
        "0"
    );

    // What the fallback used to pay: the fiat figure, one token for one yuan.
    assert_eq!(
        token_units("5000", Some("CNY"), Some(1.0), 6, Some(1.0), "CNY"),
        "5000"
    );
    // And what it is actually worth once CNY can be priced.
    assert_eq!(
        token_units("5000", Some("CNY"), Some(1.0), 6, Some(7.17), "CNY"),
        "697.35007"
    );

    // TOKEN mode never reads the rate: sending 5 USDT still works with no
    // rate at all. Blocking the conversion must not block the send screen.
    assert_eq!(token_units("5", None, Some(1.0), 6, None, "CNY"), "5");
    assert_eq!(
        token_units("0.25", None, Some(2000.0), 18, None, "CNY"),
        "0.25"
    );
}

#[test]
fn recipients_are_valid_needs_address_and_positive_amount_each() {
    let row = |addr: &str, amount: &str| SendRecipientDraft {
        id: "r".to_owned(),
        address: addr.to_owned(),
        amount: amount.to_owned(),
        name: None,
    };
    assert!(!recipients_are_valid(&[]));
    assert!(recipients_are_valid(&[row(RECIPIENT, "1.5")]));
    // Trimmed address is fine; bad address or zero amount is not.
    assert!(recipients_are_valid(&[row(
        &format!(" {RECIPIENT} "),
        "0.1"
    )]));
    assert!(!recipients_are_valid(&[row("0x123", "1")]));
    assert!(!recipients_are_valid(&[row(RECIPIENT, "0")]));
    assert!(!recipients_are_valid(&[
        row(RECIPIENT, "1"),
        row(RECIPIENT_B, "")
    ]));
}

/// Issue 203: the importer has refused a repeated payee since it shipped, but
/// rows typed in, picked from the book or added one at a time never met that
/// rule — the same address could take two lines of one batch with nothing
/// anywhere saying so.
#[test]
fn duplicate_recipient_rows_names_every_repeat_and_the_row_it_repeats() {
    let row = |id: &str, addr: &str| SendRecipientDraft {
        id: id.to_owned(),
        address: addr.to_owned(),
        amount: "1".to_owned(),
        name: None,
    };
    // Nothing repeats.
    assert_eq!(
        duplicate_recipient_rows(&[row("a", RECIPIENT), row("b", RECIPIENT_B)]),
        vec![]
    );
    // The reporter's batch: rows 5 and 6 repeat rows 2 and 1. The FIRST
    // occurrence is never flagged — it is the row the repeat repeats.
    let flagged = duplicate_recipient_rows(&[
        row("r1", RECIPIENT),
        row("r2", RECIPIENT_B),
        row("r3", "0x0000000000000000000000000000000000000003"),
        row("r4", "0x0000000000000000000000000000000000000004"),
        row("r5", RECIPIENT_B),
        row("r6", RECIPIENT),
    ]);
    assert_eq!(
        flagged
            .iter()
            .map(|d| (d.id.as_str(), d.first_ordinal))
            .collect::<Vec<_>>(),
        vec![("r5", 2), ("r6", 1)]
    );
    // The importer's matching, exactly: trimmed and case-insensitive, so a
    // checksummed address and its lowercase twin are one payee.
    let mixed = duplicate_recipient_rows(&[
        row("a", RECIPIENT),
        row("b", &format!(" {} ", RECIPIENT.to_lowercase())),
    ]);
    assert_eq!(mixed.len(), 1);
    assert_eq!(mixed[0].first_ordinal, 1);
    // A row that is not an address yet repeats nothing — an empty second row
    // is what "+ add recipient" hands you, and two of them are not a warning.
    assert_eq!(
        duplicate_recipient_rows(&[row("a", ""), row("b", ""), row("c", "0x123")]),
        vec![]
    );
}

/// The screen's warning and the rule are one thing, and it never fires outside
/// a split: the same repeat is a warning in the editor and nothing at all in a
/// single send, which has one recipient by construction.
#[test]
fn the_view_flags_repeated_payees_without_refusing_the_batch() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    assert_eq!(sut.view().split_duplicates, vec![]);

    sut.dispatch(Event::EnterSplitMode);
    let row = |id: &str, to: &str| SendRecipientDraft {
        id: id.to_owned(),
        address: to.to_owned(),
        amount: "0.25".to_owned(),
        name: None,
    };
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            row("rcpt_1", RECIPIENT),
            row("rcpt_2", RECIPIENT_B),
            row("rcpt_3", RECIPIENT),
        ],
    });
    let view = sut.view();
    assert_eq!(view.split_duplicates.len(), 1);
    assert_eq!(view.split_duplicates[0].id, "rcpt_3");
    assert_eq!(view.split_duplicates[0].first_ordinal, 1);
    // Warned, not refused: the batch is exactly what was asked for, and both
    // payments are built.
    assert!(view.can_continue);
    let ops = sut.dispatch(Event::Continue);
    assert!(
        !ops.iter().any(|op| matches!(op, Op::ShowAlert { .. })),
        "a repeat is a warning, not a refusal: {ops:?}"
    );
    // …and both payments to the repeated payee are in the batch that is quoted.
    assert!(ops.iter().any(|op| matches!(
        op,
        Op::EstimateFee {
            batch: Some(calls),
            ..
        } if calls.len() == 3
    )));

    // Fix the repeat and the warning goes with it.
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            row("rcpt_1", RECIPIENT),
            row("rcpt_2", RECIPIENT_B),
            row("rcpt_3", "0x0000000000000000000000000000000000000009"),
        ],
    });
    assert_eq!(sut.view().split_duplicates, vec![]);
}

#[test]
fn sum_split_base_units_is_exact_and_refuses_garbage() {
    let row = |amount: &str| SendRecipientDraft {
        id: "r".to_owned(),
        address: RECIPIENT.to_owned(),
        amount: amount.to_owned(),
        name: None,
    };
    assert_eq!(
        sum_split_base_units(&[row("1.5"), row("0.25")], 6),
        Some(1_750_000)
    );
    // Where TS toBaseUnits throws, the port refuses.
    assert_eq!(sum_split_base_units(&[row("1,5")], 6), None);
}

#[test]
fn build_split_calls_shapes_erc20_transfers_and_refuses_zero() {
    let rows = vec![
        SendRecipientDraft {
            id: "a".to_owned(),
            address: RECIPIENT.to_owned(),
            amount: "1.5".to_owned(),
            name: None,
        },
        SendRecipientDraft {
            id: "b".to_owned(),
            address: format!(" {RECIPIENT_B} "), // trimmed at build time
            amount: "0.5".to_owned(),
            name: None,
        },
    ];
    let calls = build_split_calls(Some(USDC), 6, &rows).expect("builds");
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].to, USDC);
    assert_eq!(calls[0].value, "0");
    assert!(calls[0].data.starts_with("0xa9059cbb"));
    assert!(calls[0].data.contains(&RECIPIENT[2..].to_lowercase()));
    // Zero amount → the whole batch refuses (TS throws BatchSendError).
    let zero = vec![SendRecipientDraft {
        id: "a".to_owned(),
        address: RECIPIENT.to_owned(),
        amount: "0".to_owned(),
        name: None,
    }];
    assert_eq!(build_split_calls(Some(USDC), 6, &zero), None);
    assert_eq!(build_multi_token_calls(RECIPIENT, &[]), None);
}

#[test]
fn is_valid_address_is_the_exact_regex() {
    assert!(is_valid_address(RECIPIENT));
    assert!(!is_valid_address(&RECIPIENT[..41]));
    assert!(!is_valid_address(&format!("{RECIPIENT} ")));
    assert!(!is_valid_address(
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    ));
}

/// The reentry-lock contract (issue #91), ported verbatim.
#[test]
fn reentry_lock_stale_end_never_releases_a_newer_lock() {
    let mut lock = ReentryLock::default();
    let first = lock.begin().expect("acquires");
    assert_eq!(lock.begin(), None, "single flight");
    // Cancel force-releases and invalidates the holder…
    lock.cancel();
    assert!(!lock.busy());
    let second = lock.begin().expect("retry acquires");
    // …so the cancelled promise's finally is a no-op against the new lock.
    assert!(!lock.end(first), "stale end must not release");
    assert!(lock.busy());
    assert!(lock.end(second));
    assert!(!lock.busy());
}

// ===========================================================================
// Boot, tokens, preselection
// ===========================================================================

#[test]
fn loaded_tokens_filter_zero_balances_and_sort_by_usd_value() {
    let sut = boot(vec![usdc("5"), eth("2"), dai("0")]);
    let view = sut.view();
    // dai has zero balance → gone; ETH ($4000) outranks USDC ($5).
    let symbols: Vec<&str> = view.tokens.iter().map(|t| t.symbol.as_str()).collect();
    assert_eq!(symbols, vec!["ETH", "USDC"]);
    assert!(!view.loading);
    assert_eq!(view.stage, SendStage::SelectToken);
}

#[test]
fn token_load_failure_alerts_and_stops_loading() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams::default()));
    let ops = sut.resolve(Res::TokensLoaded {
        tokens: None,
        chains: chains(),
    });
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::LoadTokensFailed
        }]
    );
    assert!(!sut.view().loading);
}

#[test]
fn progressive_chunks_paint_early_but_never_after_the_load_settled() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams::default()));
    sut.dispatch(Event::TokensPartial {
        tokens: vec![eth("2")],
    });
    let view = sut.view();
    assert_eq!(view.tokens.len(), 1);
    assert!(!view.loading, "first chunk ends the skeleton");
    sut.resolve(loaded(vec![eth("2"), usdc("5")]));
    // A late stray chunk after the load settled must not clobber the list.
    sut.dispatch(Event::TokensPartial { tokens: vec![] });
    assert_eq!(sut.view().tokens.len(), 2);
}

/// A prefilled recipient (the address book's 转账, a scanned address) is the
/// recipient from the first frame — before the token list answers, and even
/// if it never does (spec 028 US5). The web hand-off found the old order: the
/// form opened on nobody while the fetch was out.
#[test]
fn a_prefilled_recipient_is_shown_before_the_tokens_arrive() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        prefilled_recipient: Some(RECIPIENT.to_owned()),
        ..SendOpenParams::default()
    }));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails, "optimistic step");
    assert_eq!(view.recipient, RECIPIENT, "known before any token is");
    assert!(view.selected_token.is_none());

    // The tokens land: the highest-value one is picked, the recipient stays.
    sut.resolve(loaded(vec![eth("2"), usdc("5")]));
    let view = sut.view();
    assert_eq!(view.recipient, RECIPIENT);
    assert!(view.selected_token.is_some());
}

/// Issue #209: a hand-off from the address book to an account that holds
/// NOTHING must not leave a form standing. `open` steps to the form for the
/// recipient's sake; when the list comes back with no token to send, the form
/// has no balance, no chain and no Max to show — so the picker is what is
/// showing, and it says so out of the same empty list.
#[test]
fn a_prefilled_recipient_with_nothing_to_send_falls_back_to_the_picker() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        prefilled_recipient: Some(RECIPIENT.to_owned()),
        ..SendOpenParams::default()
    }));
    assert_eq!(sut.view().stage, SendStage::EnterDetails, "optimistic step");

    sut.resolve(loaded(vec![]));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken, "no token, no form");
    assert!(view.selected_token.is_none());
    assert!(
        view.tokens.is_empty(),
        "the picker says the account is empty"
    );
    // Who the money is for survives: picking a token later lands on the form
    // with the person still filled in.
    assert_eq!(view.recipient, RECIPIENT);
}

/// The same rule for the two other hand-offs that step to the form early: a
/// token whose symbol is no longer held, and multi ids that match no row.
#[test]
fn a_preselection_that_resolves_to_nothing_falls_back_to_the_picker() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        preselected_symbol: Some("USDC".to_owned()),
        preselected_network: Some("ethereum".to_owned()),
        ..SendOpenParams::default()
    }));
    sut.resolve(loaded(vec![eth("2")]));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken, "USDC is not held");
    assert!(view.selected_token.is_none());

    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        preselected_multi: Some("ethereum_native_WETH".to_owned()),
        ..SendOpenParams::default()
    }));
    sut.resolve(loaded(vec![eth("2")]));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken, "no id matched");
    assert!(!view.multi_select_mode);
}

#[test]
fn preselected_symbol_and_network_land_on_enter_details() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        preselected_symbol: Some("USDC".to_owned()),
        preselected_network: Some("ethereum".to_owned()),
        ..SendOpenParams::default()
    }));
    assert_eq!(sut.view().stage, SendStage::EnterDetails, "optimistic step");
    sut.resolve(loaded(vec![eth("2"), usdc("5")]));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails);
    assert_eq!(
        view.selected_token.as_ref().map(|t| t.symbol.as_str()),
        Some("USDC")
    );
}

#[test]
fn prefilled_recipient_quick_send_picks_the_most_valuable_token() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        prefilled_recipient: Some(RECIPIENT.to_owned()),
        ..SendOpenParams::default()
    }));
    let ops = sut.resolve(loaded(vec![usdc("5"), eth("2")]));
    assert!(
        matches!(
            ops.as_slice(),
            [Op::ResolveIdentity { .. }, Op::LoadAccountCredential { .. }]
        ),
        "recipient prefill resolves identity and warms a quote (Phase 10): {ops:?}"
    );
    let view = sut.view();
    assert_eq!(
        view.selected_token.as_ref().map(|t| t.symbol.as_str()),
        Some("ETH")
    );
    assert_eq!(view.recipient, RECIPIENT);
    assert_eq!(view.stage, SendStage::EnterDetails);
}

#[test]
fn preselected_multi_hand_off_lands_in_multi_select_and_warms_an_estimate() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        preselected_multi: Some(format!("{},{}", eth("2").id(), usdc("5").id())),
        ..SendOpenParams::default()
    }));
    let ops = sut.resolve(loaded(vec![eth("2"), usdc("5")]));
    assert!(matches!(ops.as_slice(), [Op::LoadAccountCredential { .. }]));
    let ops = sut.resolve(credential(Some(PK)));
    assert!(
        matches!(
            ops.as_slice(),
            [Op::EstimateFee {
                tx: None,
                batch: None,
                ..
            }]
        ),
        "rough warm estimate: {ops:?}"
    );
    assert!(sut.resolve(fee_ok(native_fee(1, 1_000))).is_empty());
    let view = sut.view();
    assert!(view.multi_select_mode);
    assert_eq!(view.multi_selected_ids.len(), 2);
    assert_eq!(view.stage, SendStage::EnterDetails);
    assert!(view.fee.is_some(), "warm estimate landed");
}

// ===========================================================================
// EIP-681 locked requests
// ===========================================================================

fn locked_params(token: Option<&str>, amount_base: Option<&str>, chain: &str) -> SendOpenParams {
    SendOpenParams {
        prefilled_recipient: Some(RECIPIENT.to_owned()),
        prefilled_chain_id: Some(chain.to_owned()),
        prefilled_token_address: token.map(str::to_owned),
        prefilled_amount_base: amount_base.map(str::to_owned),
        locked: true,
        ..SendOpenParams::default()
    }
}

#[test]
fn locked_request_resolves_a_held_token_with_its_real_decimals() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(locked_params(Some(USDC), Some("1500000"), "1")));
    let ops = without_form_quote(sut.resolve(loaded(vec![eth("2"), usdc("5")])));
    assert!(matches!(ops.as_slice(), [Op::ResolveIdentity { .. }]));
    let view = sut.view();
    // ⑫: 1_500_000 base units at the token's ON-CHAIN 6 decimals = 1.5.
    assert_eq!(view.amount, "1.5");
    assert_eq!(view.recipient, RECIPIENT);
    assert!(view.amount_locked);
    assert_eq!(view.stage, SendStage::EnterDetails);
}

#[test]
fn locked_unknown_token_restores_the_amount_with_resolved_decimals() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(locked_params(Some(DAI), Some("1500000"), "1")));
    let ops = sut.resolve(loaded(vec![eth("2")])); // DAI not held
    assert_eq!(
        ops,
        vec![Op::ResolveTokenMetadata {
            chain_id: 1,
            address: DAI.to_lowercase(),
        }]
    );
    // ⑫: the link may claim any decimals — only the chain's answer counts.
    let ops = sut.resolve(Res::TokenMetadata {
        meta: Some(SendTokenMeta {
            symbol: "DAI".to_owned(),
            decimals: 6,
        }),
    });
    let ops = without_form_quote(ops);
    assert!(matches!(ops.as_slice(), [Op::ResolveIdentity { .. }]));
    let view = sut.view();
    assert_eq!(view.amount, "1.5");
    let tok = view.selected_token.expect("synthetic token");
    assert_eq!(tok.balance, "0");
    assert_eq!(tok.decimals, 6);
    assert_eq!(tok.token_address.as_deref(), Some(DAI));
}

#[test]
fn locked_unknown_token_without_metadata_is_the_token_exception() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(locked_params(Some(DAI), None, "1")));
    sut.resolve(loaded(vec![eth("2")]));
    sut.resolve(Res::TokenMetadata { meta: None });
    let view = sut.view();
    assert_eq!(view.lock_error, Some(SendLockError::Token));
    assert_eq!(view.stage, SendStage::LockError);
}

#[test]
fn locked_native_request_synthesizes_a_zero_balance_placeholder() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(locked_params(
        None,
        Some("1000000000000000000"),
        "137",
    )));
    sut.resolve(loaded(vec![eth("2")])); // no POL held
    let view = sut.view();
    let tok = view.selected_token.expect("synthetic native");
    assert_eq!(tok.symbol, "POL");
    assert_eq!(tok.balance, "0");
    assert_eq!(tok.token_address, None);
    assert_eq!(view.amount, "1");
}

#[test]
fn locked_unsupported_chain_offers_add_network_and_retries_after_adding() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(locked_params(None, None, "999")));
    sut.resolve(loaded(vec![eth("2")]));
    assert_eq!(
        sut.view().lock_error,
        Some(SendLockError::Network { chain_id: 999 })
    );
    let ops = sut.dispatch(Event::AddNetworkTapped { chain_id: 999 });
    assert_eq!(ops, vec![Op::AddNetwork { chain_id: 999 }]);
    assert!(sut.view().adding_network);
    // Added → resolution re-runs against a fresh token load.
    let ops = sut.resolve(Res::NetworkAdded {
        outcome: SendAddNetworkOutcome::Added,
    });
    assert!(matches!(ops.as_slice(), [Op::FetchTokens { .. }]));
    let mut with_new_chain = chains();
    with_new_chain.push(SendChainInfo {
        chain_id: 999,
        network: "customnet".to_owned(),
        native_symbol: "CUST".to_owned(),
    });
    sut.resolve(Res::TokensLoaded {
        tokens: Some(vec![eth("2")]),
        chains: with_new_chain,
    });
    let view = sut.view();
    assert_eq!(view.lock_error, None);
    assert_eq!(
        view.selected_token.as_ref().map(|t| t.symbol.as_str()),
        Some("CUST")
    );
}

#[test]
fn locked_bad_chain_param_just_ends_resolution_ported_verbatim() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(locked_params(None, None, "not-a-number")));
    sut.resolve(loaded(vec![eth("2")]));
    let view = sut.view();
    assert_eq!(view.lock_error, None);
    assert!(!view.resolving_lock);
}

// ===========================================================================
// Amount warnings (derived, `useSendController.ts:326-398`)
// ===========================================================================

#[test]
fn native_amount_over_balance_warns_not_enough_token() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    sut.dispatch(Event::SetAmount {
        amount: "2.5".to_owned(),
    });
    assert_eq!(
        sut.view().amount_warning,
        Some(SendAmountWarning::NotEnoughToken {
            symbol: "ETH".to_owned()
        })
    );
}

#[test]
fn native_amount_plus_quoted_fee_over_balance_warns_insufficient_for_gas() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    // 2 ETH balance, 0.1 ETH quoted reimbursement.
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 100_000_000_000_000_000),
    });
    sut.dispatch(Event::SetAmount {
        amount: "1.95".to_owned(),
    });
    assert_eq!(
        sut.view().amount_warning,
        Some(SendAmountWarning::InsufficientForGas {
            symbol: Some("ETH".to_owned())
        })
    );
    // Exactly balance − fee passes.
    sut.dispatch(Event::SetAmount {
        amount: "1.9".to_owned(),
    });
    assert_eq!(sut.view().amount_warning, None);
}

#[test]
fn sending_the_fee_token_reserves_its_own_fee() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    let ops = sut.dispatch(Event::SelectToken {
        token_id: usdc("5").id(),
    });
    assert_eq!(ops.len(), 1);
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 1_000_000), // $1 fee in USDC
    });
    sut.dispatch(Event::SetAmount {
        amount: "4.5".to_owned(),
    });
    assert_eq!(
        sut.view().amount_warning,
        Some(SendAmountWarning::InsufficientForGas {
            symbol: Some("USDC".to_owned())
        })
    );
}

#[test]
fn sending_another_token_requires_the_fee_token_balance_to_cover() {
    // Fee is paid in USDC but the user holds only $0.5 of it.
    let mut sut = boot(vec![eth("2"), usdc("0.5"), dai("100")]);
    sut.dispatch(Event::SelectToken {
        token_id: dai("100").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 1_000_000),
    });
    sut.dispatch(Event::SetAmount {
        amount: "10".to_owned(),
    });
    assert_eq!(
        sut.view().amount_warning,
        Some(SendAmountWarning::NeedGas {
            symbol: Some("USDC".to_owned())
        })
    );
}

/// Issue #211. The fee rides on the native coin and the account holds none of
/// it — a zero balance never even reaches this machine, since the holdings
/// drop it — so the fee leg would move a coin that is not there and the whole
/// op could only revert. Before this rule the ERC-20 half of the fee check was
/// the only half: nothing measured a NATIVE fee, so the screen said nothing
/// and `Continue` carried a doomed send to the passkey.
#[test]
fn an_erc20_send_paying_a_native_fee_it_cannot_afford_warns_and_refuses() {
    let mut sut = boot(vec![usdc("5")]);
    sut.dispatch(Event::SelectToken {
        token_id: usdc("5").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 100_000_000_000_000_000),
    });
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    // The symbol is the chain's, because there is no row to read it from.
    let need_gas = SendAmountWarning::NeedGas {
        symbol: Some("ETH".to_owned()),
    };
    assert_eq!(sut.view().amount_warning, Some(need_gas.clone()));
    let ops = without_form_quote(sut.dispatch(Event::Continue));
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::InsufficientBalance {
                warning: Some(need_gas)
            }
        }]
    );
    assert_eq!(sut.view().stage, SendStage::EnterDetails);
}

/// The mirror image, and the second half of the same hole: a NATIVE send whose
/// gas is paid in a stablecoin the account barely holds. That branch measured
/// only the coin being sent, so switching the fee coin said nothing at all.
#[test]
fn a_native_send_paying_an_erc20_fee_it_cannot_afford_warns_and_refuses() {
    let mut sut = boot(vec![eth("2"), usdc("0.5")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 1_000_000), // $1 in USDC, and 0.5 is held
    });
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    let need_gas = SendAmountWarning::NeedGas {
        symbol: Some("USDC".to_owned()),
    };
    assert_eq!(sut.view().amount_warning, Some(need_gas.clone()));
    let ops = without_form_quote(sut.dispatch(Event::Continue));
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::InsufficientBalance {
                warning: Some(need_gas)
            }
        }]
    );
    // …and the whole native balance is spendable when the fee is not: Max
    // reserves nothing for gas a different coin is paying.
    let mut rich = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut rich);
    rich.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 1_000_000),
    });
    rich.dispatch(Event::TapMax);
    assert_eq!(rich.view().amount, "2");
    assert_eq!(rich.view().amount_warning, None);
}

/// The other side of the same rule: a native balance that DOES cover the
/// quoted fee is not a warning. The sent token's own balance is untouched by
/// the fee — it is not the asset paying it.
#[test]
fn an_erc20_send_whose_native_balance_covers_the_fee_stays_silent() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    sut.dispatch(Event::SelectToken {
        token_id: usdc("5").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 100_000_000_000_000_000),
    });
    sut.dispatch(Event::SetAmount {
        amount: "5".to_owned(),
    });
    assert_eq!(sut.view().amount_warning, None);
}

/// A quote for ANOTHER chain is not this form's fee (invariant ①), so it can
/// neither price nor refuse this send.
#[test]
fn a_native_fee_quoted_for_another_chain_never_refuses_this_one() {
    let mut sut = boot(vec![usdc("5")]);
    sut.dispatch(Event::SelectToken {
        token_id: usdc("5").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(137, 100_000_000_000_000_000),
    });
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    assert_eq!(sut.view().amount_warning, None);
}

/// Issue #211's own repro: a split (the −0.12 pUSD to two recipients). The
/// split path asks its rows about the token's balance and asked nothing at all
/// about the fee, so this refusal has to live on the `Continue` gate itself.
#[test]
fn a_split_paying_a_native_fee_it_cannot_afford_is_refused() {
    let mut sut = boot(vec![usdc("5")]);
    sut.dispatch(Event::SelectToken {
        token_id: usdc("5").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 100_000_000_000_000_000),
    });
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            SendRecipientDraft {
                id: "rcpt_1".to_owned(),
                address: RECIPIENT.to_owned(),
                amount: "0.06".to_owned(),
                name: None,
            },
            SendRecipientDraft {
                id: "rcpt_2".to_owned(),
                address: RECIPIENT_B.to_owned(),
                amount: "0.06".to_owned(),
                name: None,
            },
        ],
    });
    let ops = without_form_quote(sut.dispatch(Event::Continue));
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::InsufficientBalance {
                warning: Some(SendAmountWarning::NeedGas {
                    symbol: Some("ETH".to_owned())
                })
            }
        }]
    );
    assert_eq!(sut.view().stage, SendStage::EnterDetails);
}

/// And a sweep: `reserve_native_gas` drops a native line it cannot reserve
/// from and keeps every ERC-20 one, so the batch still builds — with a fee leg
/// nothing can pay.
#[test]
fn a_sweep_paying_a_native_fee_it_cannot_afford_is_refused() {
    let mut sut = boot(vec![usdc("5"), dai("100")]);
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    sut.dispatch(Event::ToggleAllMultiTokens {
        visible_ids: vec![usdc("5").id(), dai("100").id()],
    });
    sut.dispatch(Event::ConfirmMultiSelection);
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 100_000_000_000_000_000),
    });
    set_recipient(&mut sut, RECIPIENT);
    let ops = without_form_quote(sut.dispatch(Event::Continue));
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::InsufficientBalance {
                warning: Some(SendAmountWarning::NeedGas {
                    symbol: Some("ETH".to_owned())
                })
            }
        }]
    );
}

#[test]
fn empty_or_zero_amounts_never_warn() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    assert_eq!(sut.view().amount_warning, None);
    sut.dispatch(Event::SetAmount {
        amount: "0".to_owned(),
    });
    assert_eq!(sut.view().amount_warning, None);
}

// ===========================================================================
// Max (invariant ⑨ — string-exact)
// ===========================================================================

#[test]
fn max_native_is_string_exact_against_the_reserve() {
    let mut sut = boot(vec![eth("1.234567891234567891")]);
    let ops = sut.dispatch(Event::SelectToken {
        token_id: eth("1.234567891234567891").id(),
    });
    assert_eq!(ops.len(), 1);
    sut.resolve(credential(Some(PK)));
    let reserve: u128 = 10_000_000_000_000_001;
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, reserve),
    });
    let ops = sut.dispatch(Event::TapMax);
    assert!(ops.is_empty(), "no estimate needed: {ops:?}");
    let view = sut.view();
    // ⑨: to_base_units(result) + reserve == balance, exactly.
    let filled = to_base_units(&view.amount, 18).expect("max parses");
    assert_eq!(
        filled + reserve,
        to_base_units("1.234567891234567891", 18).expect("balance parses")
    );
    assert_eq!(view.amount_fiat_code, None, "Max always fills token units");
    assert_eq!(
        view.amount_warning, None,
        "its own fill never trips the gate"
    );
}

#[test]
fn max_of_the_fee_token_reserves_one_and_a_half_times_the_quote() {
    let mut sut = boot(vec![usdc("5")]);
    sut.dispatch(Event::SelectToken {
        token_id: usdc("5").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 1_000_000),
    });
    sut.dispatch(Event::TapMax);
    // 5 − 1.5×1 = 3.5 USDC.
    assert_eq!(sut.view().amount, "3.5");
}

/// Issue #210: 0.00005 BNB against a 0.000332 BNB fee. `Max` is right to fill
/// `0` — and has to say why, or the screen is a zero and a dead button.
#[test]
fn max_below_the_native_fee_fills_zero_and_says_why() {
    let mut sut = boot(vec![eth("0.00005")]);
    let ops = sut.dispatch(Event::SelectToken {
        token_id: eth("0.00005").id(),
    });
    assert_eq!(ops.len(), 1);
    sut.resolve(credential(Some(PK)));
    // The fee is 6.6× the whole balance.
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 332_000_000_000_000),
    });
    let ops = sut.dispatch(Event::TapMax);
    assert!(ops.is_empty(), "no estimate needed: {ops:?}");
    let view = sut.view();
    assert_eq!(view.amount, "0");
    assert_eq!(
        view.amount_warning,
        Some(SendAmountWarning::InsufficientGas {
            symbol: Some("ETH".to_owned())
        }),
        "the zero explains itself"
    );
    assert!(!view.can_continue, "and the gate stays shut");
}

/// The same rule one wei the other side of the line: a balance that still
/// clears the reserve fills a figure and says nothing.
#[test]
fn max_one_wei_above_the_reserve_is_silent() {
    let mut sut = boot(vec![eth("0.000332000000000001")]);
    sut.dispatch(Event::SelectToken {
        token_id: eth("0.000332000000000001").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 332_000_000_000_000),
    });
    sut.dispatch(Event::TapMax);
    let view = sut.view();
    assert_eq!(view.amount, "0.000000000000000001");
    assert_eq!(view.amount_warning, None);
}

/// The fee token sent as itself: the 1.5× reserve is the line, and a balance
/// under it gets the same sentence in the fee asset's own symbol.
#[test]
fn max_of_the_fee_token_below_its_reserve_fills_zero_and_says_why() {
    let mut sut = boot(vec![usdc("1.4")]);
    sut.dispatch(Event::SelectToken {
        token_id: usdc("1.4").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 1_000_000), // reserve = 1.5 USDC
    });
    sut.dispatch(Event::TapMax);
    let view = sut.view();
    assert_eq!(view.amount, "0");
    assert_eq!(
        view.amount_warning,
        Some(SendAmountWarning::InsufficientGas {
            symbol: Some("USDC".to_owned())
        })
    );
}

/// A sponsored transfer reserves nothing, so an empty balance is an empty
/// balance — never "the fee ate it".
#[test]
fn a_zero_fee_never_blames_the_fee() {
    let mut sut = boot(vec![eth("0.000001")]);
    sut.dispatch(Event::SelectToken {
        token_id: eth("0.000001").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 0),
    });
    sut.dispatch(Event::TapMax);
    let view = sut.view();
    assert_eq!(
        view.amount, "0.000001",
        "a sponsored transfer sweeps it all"
    );
    assert_eq!(view.amount_warning, None);
}

/// Gas paid in a separate asset: the whole balance is sendable, so a zero in
/// the box is the person's own zero and the fee has nothing to answer for.
#[test]
fn a_fee_in_another_asset_never_claims_this_balance() {
    let mut sut = boot(vec![usdc("5"), dai("0.1")]);
    sut.dispatch(Event::SelectToken {
        token_id: dai("0.1").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 1_000_000),
    });
    sut.dispatch(Event::SetAmount {
        amount: "0".to_owned(),
    });
    assert_eq!(sut.view().amount_warning, None);
}

/// 078 M-01: Max while Continue's pre-check is out. It used to take the
/// pipeline for its own estimate, which dropped the pre-check's answer and
/// left `estimating_gas` set — Continue read "Estimating…" for good.
#[test]
fn max_during_the_continue_precheck_leaves_the_continue_to_finish() {
    let mut sut = boot(vec![usdc("100")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "10".to_owned(),
    });
    let ops = sut.dispatch(Event::Continue);
    drain_form_quote(&mut sut);
    assert!(
        matches!(
            ops.as_slice(),
            [
                Op::EstimateFee { .. },
                Op::ProbeTreasury { .. },
                Op::StartTimer { .. }
            ]
        ),
        "pre-check trio expected, got {ops:?}"
    );
    assert!(sut.view().estimating_gas);

    let ops = sut.dispatch(Event::TapMax);
    assert!(ops.is_empty(), "the Continue owns the form: {ops:?}");
    assert_eq!(
        sut.view().amount,
        "10",
        "the figure being checked stays the figure"
    );

    assert!(sut.resolve(fee_ok(usdc_fee(1, 1_000_000))).is_empty());
    sut.resolve(covered());
    let view = sut.view();
    assert!(!view.estimating_gas, "the check settles");
    assert_eq!(view.stage, SendStage::Confirm);
}

/// 078 M-01: Max while Continue is still loading the credential. It used to
/// replace the Continue, so the credential's answer found nobody waiting.
#[test]
fn max_during_the_continue_credential_load_does_not_cancel_the_continue() {
    let mut sut = boot(vec![eth("2")]);
    let ops = sut.dispatch(Event::SelectToken {
        token_id: eth("2").id(),
    });
    assert_eq!(ops.len(), 1);
    sut.drop_oldest(); // prefetch lost
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    let ops = sut.dispatch(Event::Continue);
    assert!(matches!(ops.as_slice(), [Op::LoadAccountCredential { .. }]));
    drain_form_quote(&mut sut);

    let ops = sut.dispatch(Event::TapMax);
    assert!(ops.is_empty(), "the Continue owns the form: {ops:?}");

    let ops = sut.resolve(credential(Some(PK)));
    assert!(
        matches!(
            ops.as_slice(),
            [
                Op::EstimateFee { .. },
                Op::ProbeTreasury { .. },
                Op::StartTimer { .. }
            ]
        ),
        "the Continue carries on to its pre-check: {ops:?}"
    );
    assert!(sut.view().estimating_gas);
}

#[test]
fn max_without_a_quote_estimates_on_demand_and_falls_back_to_full_balance() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    let ops = sut.dispatch(Event::TapMax);
    // 078 M-02: the transfer Max is about to fill, not a placeholder — no
    // payee typed yet, so the account itself; one wei of the chain's coin,
    // the value-bearing path without asking to move the whole balance.
    match ops.as_slice() {
        [Op::EstimateFee {
            tx: Some(call),
            batch: None,
            account,
            ..
        }] => {
            assert_eq!(&call.to, account, "no payee yet: the account itself");
            assert_eq!(call.value, "1");
            assert_eq!(call.data, "0x");
        }
        other => panic!("the transfer's own estimate: {other:?}"),
    }
    // Estimation failed — full balance; the pre-check still warns later.
    sut.resolve(Res::FeeEstimated {
        outcome: SendFeeOutcome::Failed {
            kind: SendEstimateFailure::QuoteUnavailable,
        },
    });
    assert_eq!(sut.view().amount, "2");
}

/// 078 M-02: a token's Max quotes `transfer(payee, whole balance)` — the call
/// Continue will quote again — and keeps that quote as the form's fee.
#[test]
fn max_without_a_quote_estimates_the_token_transfer_it_fills() {
    let mut sut = boot(vec![usdc("5")]);
    sut.dispatch(Event::SelectToken {
        token_id: usdc("5").id(),
    });
    sut.resolve(credential(Some(PK)));
    set_recipient(&mut sut, RECIPIENT);
    let ops = sut.dispatch(Event::TapMax);
    let call = ops
        .iter()
        .find_map(|op| match op {
            Op::EstimateFee { tx: Some(call), .. } => Some(call.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("an estimate of the transfer: {ops:?}"));
    assert_eq!(call.to.to_lowercase(), USDC.to_lowercase());
    assert_eq!(call.value, "0");
    // transfer(RECIPIENT, 5_000_000)
    assert!(call.data.starts_with("0xa9059cbb"), "{}", call.data);
    assert!(call.data.contains(&RECIPIENT[2..]), "{}", call.data);
    assert!(
        call.data.ends_with(&format!("{:064x}", 5_000_000u128)),
        "{}",
        call.data
    );

    sut.resolve_matching(
        |op| matches!(op, Op::EstimateFee { .. }),
        Res::FeeEstimated {
            outcome: SendFeeOutcome::Ok {
                estimate: usdc_fee(1, 1_000_000),
            },
        },
    );
    let view = sut.view();
    assert_eq!(view.amount, "3.5", "5 − 1.5 × the quote");
    assert!(
        view.fee.is_some(),
        "the quote Max used is the fee on screen"
    );
}

/// 078 M-03: after the person picks another fee coin, the quote in hand is
/// the old coin's. Max must not reserve against it — it asks again, in the
/// coin now chosen.
#[test]
fn max_after_a_fee_coin_switch_does_not_reserve_the_old_coins_fee() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    // A native quote: Max would hold 0.5 ETH back for it.
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 500_000_000_000_000_000),
    });
    // The person moves the fee to USDC; USDC's quote has not arrived.
    sut.dispatch(Event::ChooseFeeToken {
        token: Some(USDC.to_owned()),
    });
    let ops = sut.dispatch(Event::TapMax);
    assert!(
        ops.iter().any(|op| matches!(
            op,
            Op::EstimateFee {
                gas_fee_token: Some(token),
                ..
            } if token.eq_ignore_ascii_case(USDC)
        )),
        "asks again in the chosen coin: {ops:?}"
    );
    assert_ne!(sut.view().amount, "1.5", "not the old coin's reserve");
    // …and USDC pays, so the whole ETH balance is sendable.
    sut.resolve(Res::FeeEstimated {
        outcome: SendFeeOutcome::Ok {
            estimate: usdc_fee(1, 1_000_000),
        },
    });
    assert_eq!(sut.view().amount, "2");

    // Picking native back: a USDC quote no longer counts either.
    sut.dispatch(Event::ChooseFeeToken { token: None });
    let ops = sut.dispatch(Event::TapMax);
    assert!(
        ops.iter().any(|op| matches!(
            op,
            Op::EstimateFee {
                gas_fee_token: None,
                ..
            }
        )),
        "asks again in the chain's coin: {ops:?}"
    );
}

// ===========================================================================
// Fiat input toggle (ported display math)
// ===========================================================================

#[test]
fn fiat_toggle_converts_across_the_boundary_both_ways() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    sut.dispatch(Event::ToggleFiatInput);
    let view = sut.view();
    assert_eq!(view.amount_fiat_code.as_deref(), Some("USD"));
    assert_eq!(view.amount, "2000.00", "1 ETH at $2000, toFixed(2)");
    sut.dispatch(Event::ToggleFiatInput);
    let view = sut.view();
    assert_eq!(view.amount_fiat_code, None);
    assert_eq!(view.amount, "1", "round-trips through the strip regex");
}

/// Pressing ⇄ twice gives back what was typed, at a price that does not
/// divide cleanly.
///
/// The existing round-trip test uses $2000, where `1 → 2000.00 → 1` survives
/// by luck. A device found the real case: typing `1` on a coin priced at
/// 6.8123 and toggling there-and-back left **`1.000293813380387808`** in the
/// field — eighteen digits of arithmetic noise, because the fiat leg is
/// rounded to two decimals and the way back divides that by the price at the
/// token's full precision. It is not cosmetic: that field is what a signature
/// is built from, so the person would have signed an amount they never typed.
///
/// The toggle restores rather than recomputes — but only while the fiat figure
/// is still the one it produced. Edit it and the conversion is what you get,
/// which the second half pins.
#[test]
fn toggling_twice_gives_back_the_digits_that_were_typed() {
    let mut sut = boot(vec![SendToken {
        price_usd: Some(6.8123),
        ..eth("2")
    }]);
    select_eth(&mut sut);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });

    sut.dispatch(Event::ToggleFiatInput);
    assert_eq!(sut.view().amount, "6.81", "1 × 6.8123, to two places");

    sut.dispatch(Event::ToggleFiatInput);
    assert_eq!(
        sut.view().amount,
        "1",
        "what was typed, not 6.81 ÷ 6.8123 at eighteen decimals"
    );
    assert_eq!(sut.view().amount_fiat_code, None);

    // Changed in fiat: there is nothing to give back, so it converts.
    sut.dispatch(Event::ToggleFiatInput);
    sut.dispatch(Event::SetAmount {
        amount: "13.62".to_owned(),
    });
    sut.dispatch(Event::ToggleFiatInput);
    let back = sut.view().amount;
    assert!(
        back.starts_with("1.99") || back.starts_with('2'),
        "13.62 ÷ 6.8123 ≈ 2, converted rather than restored: {back}"
    );
    assert_ne!(back, "1", "the remembered figure must not come back here");
}

/// An unpriceable display currency closes the fiat-denominated input — and
/// leaves everything else on the screen working.
///
/// The ⇄ toggle is the door into typing money in the display currency, and the
/// display currency is exactly what nothing can price here. The core will not
/// open that door; it will always let someone back OUT of it (a currency can
/// go unpriceable while a fiat amount is already typed, and trapping the user
/// in a mode whose amount can never resolve would be its own bug).
///
/// Mutation proof: drop the `target.is_fiat() && price.is_none()` guard in
/// `toggle_fiat_input` and the first assertion flips to a `Some("CNY")` code
/// with `amount: "0.00"` — the ETH amount rewritten by a multiplier that does
/// not exist. The last block is the narrowness: a token-denominated send is
/// completely unaffected by the missing rate.
#[test]
fn an_unpriceable_display_currency_closes_fiat_input_but_not_the_send() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::DisplayChanged {
        display: unpriced_display(),
    });
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });

    // The door will not open.
    sut.dispatch(Event::ToggleFiatInput);
    let view = sut.view();
    assert_eq!(
        view.amount_fiat_code, None,
        "no rate, no fiat-denominated input"
    );
    assert_eq!(view.amount, "1", "and the typed amount is left alone");

    // Token mode is untouched: 1 ETH is still 1 ETH, and it is still the
    // number the confirm page and the signature are built from.
    assert_eq!(view.token_amount, "1");

    // Someone already inside fiat mode when the rate vanished can leave.
    sut.dispatch(Event::DisplayChanged { display: display() });
    sut.dispatch(Event::ToggleFiatInput);
    assert_eq!(sut.view().amount_fiat_code.as_deref(), Some("USD"));
    sut.dispatch(Event::DisplayChanged {
        display: unpriced_display(),
    });
    // While stuck there, nothing converts — 2000 "unpriceable units" buys no
    // ETH at all, rather than 2000 ETH at a defaulted rate of 1.
    assert_eq!(sut.view().token_amount, "0");
    sut.dispatch(Event::ToggleFiatInput);
    assert_eq!(
        sut.view().amount_fiat_code,
        None,
        "leaving is always allowed"
    );
}

/// **Leaving fiat mode without a rate does not smuggle the fiat digits out
/// wearing a token label.**
///
/// This is the shape of the last four defects with the arithmetic removed. No
/// `?? 1`, no `unwrap_or(1.0)`, no `|| 1` — the conversion was simply skipped
/// and the unit label changed underneath the number, which is multiplication by
/// an implicit 1 written as an assignment. 5000 CNY became 5000 USDC, and the
/// confirm slider was armed on it.
///
/// The honest outcomes when a figure cannot be restated are: refuse the unit
/// change, or drop the figure. Trapping the user in fiat mode is NOT one of
/// them (that was the previous round's mistake in the other direction), so
/// leaving still works — the field simply arrives empty, which is the one state
/// that claims nothing. `can_continue` already refuses an empty amount.
///
/// Mutation proof: replace the `.unwrap_or_else(|_| DenominatedAmount::token(""))`
/// in `toggle_fiat_input` with anything that keeps `model.amount`'s digits and
/// `token_amount` becomes "5000" — 5000 whole USDC, signable.
#[test]
fn leaving_fiat_mode_with_no_rate_drops_the_figure_instead_of_relabelling_it() {
    let mut sut = boot(vec![usdc("9000")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, RECIPIENT);

    // Priced CNY: the door opens and 5000 CNY is a real, resolvable figure.
    sut.dispatch(Event::DisplayChanged {
        display: cny_display(),
    });
    sut.dispatch(Event::ToggleFiatInput);
    sut.dispatch(Event::SetAmount {
        amount: "5000".to_owned(),
    });
    assert_eq!(sut.view().amount_fiat_code.as_deref(), Some("CNY"));
    assert_eq!(sut.view().token_amount, "697.35007");

    // CNY goes unpriceable mid-screen. Nothing converts — already covered.
    sut.dispatch(Event::DisplayChanged {
        display: unpriced_display(),
    });
    assert_eq!(sut.view().token_amount, "0");

    // Now leave. The mode flips (no trap) and the figure does NOT come along.
    sut.dispatch(Event::ToggleFiatInput);
    let view = sut.view();
    assert_eq!(view.amount_fiat_code, None, "leaving is always allowed");
    assert_eq!(view.amount, "", "5000 CNY is not 5000 USDC");
    assert_eq!(view.token_amount, "", "nothing to sign");
    assert!(!view.can_continue, "and nothing to continue with");
}

/// The same figure, but the rate that reappears belongs to a DIFFERENT
/// currency than the one it was typed in.
///
/// `display_changed` can swap the whole context in one event, so "5000" typed
/// in CNY can find itself sitting next to a USD rate. Because the figure
/// carries its own code and `TokenPrice` carries the code it is quoted in,
/// that mismatch is a refusal — not a 7x conversion at the wrong rate.
///
/// Mutation proof: drop the `p.code() == code` filter in
/// `DenominatedAmount::to_token_units` and `token_amount` becomes "5000".
#[test]
fn a_figure_typed_in_one_currency_is_not_resolved_at_another_currencys_rate() {
    let mut sut = boot(vec![usdc("9000")]);
    select_usdc(&mut sut);
    sut.dispatch(Event::DisplayChanged {
        display: cny_display(),
    });
    sut.dispatch(Event::ToggleFiatInput);
    sut.dispatch(Event::SetAmount {
        amount: "5000".to_owned(),
    });
    assert_eq!(sut.view().token_amount, "697.35007");

    // The display currency becomes USD while a CNY figure is on the field.
    sut.dispatch(Event::DisplayChanged { display: display() });
    assert_eq!(
        sut.view().amount_fiat_code.as_deref(),
        Some("USD"),
        "still a fiat-denominated figure — in the currency now on screen"
    );
    assert_eq!(
        sut.view().token_amount,
        "0",
        "a CNY figure has no USD-rate answer"
    );

    // Leaving drops it rather than calling 5000 CNY 5000 USDC.
    sut.dispatch(Event::ToggleFiatInput);
    assert_eq!(sut.view().amount, "");
}

/// Max is a token-unit fill, so it must leave a fiat-denominated field in token
/// units — and while it waits for an estimate it leaves the field EMPTY rather
/// than letting the previous fiat digits sit under a token label.
#[test]
fn max_never_leaves_a_fiat_figure_wearing_a_token_label() {
    let mut sut = boot(vec![usdc("9000")]);
    select_usdc(&mut sut);
    sut.dispatch(Event::DisplayChanged {
        display: cny_display(),
    });
    sut.dispatch(Event::ToggleFiatInput);
    sut.dispatch(Event::SetAmount {
        amount: "5000".to_owned(),
    });
    sut.dispatch(Event::TapMax);
    let view = sut.view();
    assert_eq!(view.amount_fiat_code, None, "Max always fills token units");
    assert_ne!(view.amount, "5000", "never the CNY digits relabelled");
}

/// **A ⇄ row that refuses says why it refuses.**
///
/// The previous round closed half of this: the row now dims instead of
/// silently swallowing the tap. But dimming is a refusal, not a reason, and
/// this is the ONE branch on the screen where nothing else speaks — the token
/// is priced, the display currency is not, and the figure is in TOKEN units,
/// so it resolves perfectly and `derive_amount_warning` has nothing to say.
/// A 40%-opacity control and total silence is a dead end.
///
/// Mutation proof: drop the `denom_toggle_shown && !denom_toggle_enabled`
/// guard's `.then(...)` (return `None` unconditionally) and the last-but-one
/// assertion fails with `None` — the exact state the screen was in before.
#[test]
fn a_conversion_row_that_cannot_be_pressed_says_why() {
    let mut sut = boot(vec![usdc("9000")]);
    select_usdc(&mut sut);
    sut.dispatch(Event::DisplayChanged {
        display: unpriced_display(),
    });
    sut.dispatch(Event::SetAmount {
        amount: "10".to_owned(),
    });

    let view = sut.view();
    assert!(
        view.denom_toggle_shown,
        "a priced token still offers the row"
    );
    assert!(
        !view.denom_toggle_enabled,
        "but there is no CNY rate to enter"
    );
    assert_eq!(
        view.token_amount, "10",
        "the token figure resolves perfectly…"
    );
    assert_eq!(
        view.amount_warning, None,
        "…so nothing else on the screen speaks"
    );
    assert_eq!(
        view.denom_toggle_reason,
        Some(SendUnitIssue {
            code: "CNY".to_owned(),
            symbol: "USDC".to_owned(),
        }),
        "the dimming must come with a sentence"
    );

    // And when the row works again it goes quiet: a reason for a refusal that
    // is not happening is noise.
    sut.dispatch(Event::DisplayChanged {
        display: cny_display(),
    });
    let view = sut.view();
    assert!(view.denom_toggle_enabled);
    assert_eq!(view.denom_toggle_reason, None);
}

#[test]
fn fiat_toggle_with_no_amount_only_flips_the_mode() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::ToggleFiatInput);
    let view = sut.view();
    assert_eq!(view.amount_fiat_code.as_deref(), Some("USD"));
    assert_eq!(view.amount, "");
}

// ===========================================================================
// Split mode (invariant ⑩)
// ===========================================================================

#[test]
fn enter_split_mode_seeds_the_current_recipient_plus_an_empty_row() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    sut.dispatch(Event::EnterSplitMode);
    let view = sut.view();
    assert!(view.split_mode);
    assert_eq!(view.amount_fiat_code, None);
    assert_eq!(view.recipients.len(), 2);
    assert_eq!(view.recipients[0].address, RECIPIENT);
    assert_eq!(view.recipients[0].amount, "1");
    assert_eq!(view.recipients[1].address, "");
    // Deterministic, monotonic ids — the ported makeRecipientId counter.
    assert_eq!(view.recipients[0].id, "rcpt_1");
    assert_eq!(view.recipients[1].id, "rcpt_2");
}

#[test]
fn collapsing_to_one_row_returns_to_single_mode_with_its_values() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::EnterSplitMode);
    let remaining = SendRecipientDraft {
        id: "rcpt_1".to_owned(),
        address: RECIPIENT_B.to_owned(),
        amount: "0.7".to_owned(),
        name: None,
    };
    let ops = sut.dispatch(Event::RecipientsChanged {
        recipients: vec![remaining],
    });
    // The carried address re-resolves identity (and, the form being whole
    // again, arms the form quote).
    let ops = without_form_quote(ops);
    assert!(matches!(ops.as_slice(), [Op::ResolveIdentity { .. }]));
    let view = sut.view();
    assert!(!view.split_mode);
    assert_eq!(view.recipient, RECIPIENT_B);
    assert_eq!(view.amount, "0.7");
    assert!(view.recipients.is_empty());
}

#[test]
fn seeding_split_rows_caps_at_sixty_and_closes_the_sheets() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::OpenBatchImport);
    let rows: Vec<SendRecipientDraft> = (0..70)
        .map(|i| SendRecipientDraft {
            id: String::new(),
            address: RECIPIENT.to_owned(),
            amount: format!("{}", i + 1),
            name: Some(format!("P{i}")),
        })
        .collect();
    sut.dispatch(Event::SeedSplitRecipients { recipients: rows });
    let view = sut.view();
    assert!(view.split_mode);
    assert_eq!(view.recipients.len(), BATCH_MAX_RECIPIENTS, "⑩: ≤60 rows");
    assert!(!view.show_batch_import);
    assert!(view.recipients.iter().all(|r| r.id.starts_with("rcpt_")));
}

#[test]
fn split_continue_rejects_invalid_rows_and_over_balance_totals() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    let rows = |a: &str, b: &str| {
        vec![
            SendRecipientDraft {
                id: "rcpt_1".to_owned(),
                address: RECIPIENT.to_owned(),
                amount: a.to_owned(),
                name: None,
            },
            SendRecipientDraft {
                id: "rcpt_2".to_owned(),
                address: RECIPIENT_B.to_owned(),
                amount: b.to_owned(),
                name: None,
            },
        ]
    };
    // Invalid rows → the address alert, no step change.
    sut.dispatch(Event::RecipientsChanged {
        recipients: rows("1", ""),
    });
    let ops = sut.dispatch(Event::Continue);
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::InvalidAddress
        }]
    );
    // ⑩: the total is summed in base units against the balance.
    sut.dispatch(Event::RecipientsChanged {
        recipients: rows("1.5", "0.6"),
    });
    let ops = sut.dispatch(Event::Continue);
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::SplitOverBalance
        }]
    );
    assert_eq!(sut.view().stage, SendStage::EnterDetails);
}

// ---------------------------------------------------------------------------
// The split says what it is doing (issues 204-206 follow-up)
// ---------------------------------------------------------------------------

fn draft(id: &str, address: &str, amount: &str) -> SendRecipientDraft {
    SendRecipientDraft {
        id: id.to_owned(),
        address: address.to_owned(),
        amount: amount.to_owned(),
        name: None,
    }
}

/// A dark `Continue` names its rows. One boolean over forty recipients could
/// not say which of them was unfinished, or whether it was the address or the
/// amount — so every shell either said nothing or re-derived the rule.
#[test]
fn split_row_issues_name_the_row_and_the_field() {
    let rows = vec![
        draft("a", RECIPIENT, "1"),
        draft("b", "", ""),
        draft("c", "0xbbbb", "2"),
        draft("d", RECIPIENT_B, "0"),
        draft("e", &format!("  {RECIPIENT_B}  "), " 0.5 "),
    ];
    let issues = split_row_issues(&rows, 18);
    let seen: Vec<_> = issues
        .iter()
        .map(|i| (i.id.as_str(), i.ordinal, i.address, i.amount))
        .collect();
    use SendRowFieldState::{Empty, Invalid, Ok};
    assert_eq!(
        seen,
        vec![
            ("b", 2, Empty, Empty),
            ("c", 3, Invalid, Ok),
            ("d", 4, Ok, Invalid),
        ],
        "finished rows (a, and e once trimmed) are not listed"
    );
}

/// The core gap that armed `Continue` on a batch it could not build: an amount
/// was judged by its leading digits ("1,5" reads as 1), the sum could not read
/// it, and the press returned without a word while the total went blank. The
/// gate and the row's own state are now one computation.
#[test]
fn an_amount_that_cannot_be_summed_closes_the_gate_and_says_which_row() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    for garbage in ["1,5", "1e5", "1abc", "0.0000000000000000001"] {
        sut.dispatch(Event::RecipientsChanged {
            recipients: vec![
                draft("rcpt_1", RECIPIENT, "0.5"),
                draft("rcpt_2", RECIPIENT_B, garbage),
            ],
        });
        let view = sut.view();
        assert!(!view.can_continue, "{garbage:?} must not arm Continue");
        assert_eq!(view.split_row_issues.len(), 1, "{garbage:?}");
        assert_eq!(view.split_row_issues[0].id, "rcpt_2");
        assert_eq!(view.split_row_issues[0].address, SendRowFieldState::Ok);
        assert_eq!(view.split_row_issues[0].amount, SendRowFieldState::Invalid);
    }
    // Corrected, the list empties and the gate opens — together.
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            draft("rcpt_1", RECIPIENT, "0.5"),
            draft("rcpt_2", RECIPIENT_B, "1.5"),
        ],
    });
    let view = sut.view();
    assert!(view.split_row_issues.is_empty());
    assert!(view.can_continue);
    assert_eq!(view.confirm_amount, "2");
}

/// A split can exist before a token does — the book hands recipients over while
/// the token list is still out. "No token, so no issues" would arm `Continue`
/// over rows with no amounts; the old one-boolean gate kept it shut, and so
/// must this one.
#[test]
fn a_split_with_no_token_yet_still_answers_for_its_rows() {
    let mut sut = boot(vec![eth("2")]); // tokens known, none SELECTED
    sut.dispatch(Event::AppendSplitRecipients {
        recipients: vec![draft("", RECIPIENT, ""), draft("", RECIPIENT_B, "")],
    });
    let view = sut.view();
    assert!(view.split_mode);
    assert!(view.selected_token.is_none());
    assert_eq!(view.split_row_issues.len(), 2);
    assert!(view
        .split_row_issues
        .iter()
        .all(|i| i.address == SendRowFieldState::Ok && i.amount == SendRowFieldState::Empty));
    assert!(!view.can_continue);
}

/// Outside a split there are no rows to have issues.
#[test]
fn the_single_form_reports_no_row_issues() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    let view = sut.view();
    assert!(view.split_row_issues.is_empty());
    assert_eq!(view.split_remaining, None);
}

/// "How much is left to give out" is the core's subtraction, in base units —
/// never a shell's float.
#[test]
fn split_remaining_is_the_balance_less_the_rows() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            draft("rcpt_1", RECIPIENT, "0.5"),
            draft("rcpt_2", RECIPIENT_B, ""),
        ],
    });
    assert_eq!(sut.view().split_remaining.as_deref(), Some("1.5"));

    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            draft("rcpt_1", RECIPIENT, "0.5"),
            draft("rcpt_2", RECIPIENT_B, "0.300000000000000001"),
        ],
    });
    assert_eq!(
        sut.view().split_remaining.as_deref(),
        Some("1.199999999999999999"),
        "exact to the wei"
    );

    // Over the balance there is nothing left — `split_over_balance` says that.
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            draft("rcpt_1", RECIPIENT, "1.5"),
            draft("rcpt_2", RECIPIENT_B, "0.6"),
        ],
    });
    let view = sut.view();
    assert_eq!(view.split_remaining, None);
    assert!(view.split_over_balance);

    // And a row that cannot be summed leaves nothing to subtract from.
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            draft("rcpt_1", RECIPIENT, "1,5"),
            draft("rcpt_2", RECIPIENT_B, "0.1"),
        ],
    });
    assert_eq!(sut.view().split_remaining, None);
}

/// `Max` fills the single amount, and a split has none. It used to write that
/// hidden field (and could start a fee estimate for it), so the button changed
/// nothing a person could see.
#[test]
fn max_does_nothing_in_a_split() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    let before = sut.view();
    let ops = sut.dispatch(Event::TapMax);
    assert!(
        ops.is_empty(),
        "no estimate is started for a field nobody sees"
    );
    assert_eq!(sut.view(), before, "and nothing on the view moves");
}

/// And nothing in a sweep, for a stronger reason: every row is ALREADY that
/// token's maximum (`multi_specs` — the whole balance less the fee's reserve).
/// The event only ever knew `selected_token`, the first of the pick, so a Max
/// on the second row rewrote the hidden amount of the FIRST — and
/// `derive_amount_warning`, which a sweep does not escape, then said "not
/// enough ETH for the fee" on a form showing several tokens.
#[test]
fn max_does_nothing_in_a_sweep() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    sut.dispatch(Event::ToggleAllMultiTokens {
        visible_ids: vec![eth("2").id(), usdc("5").id()],
    });
    sut.dispatch(Event::ConfirmMultiSelection);
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 100_000_000_000_000_000),
    });
    set_recipient(&mut sut, RECIPIENT);
    let before = sut.view();
    assert!(before.multi_select_mode);
    assert_eq!(
        before.multi_specs.len(),
        2,
        "the rows are the amounts, and there are two of them"
    );
    let ops = sut.dispatch(Event::TapMax);
    assert!(
        ops.is_empty(),
        "no rival estimate for a field nobody sees: {ops:?}"
    );
    let after = sut.view();
    assert_eq!(after, before, "and nothing on the view moves");
    assert_eq!(after.amount, "", "no hidden single amount is written");
    assert_eq!(
        after.amount_warning, None,
        "and no verdict about one token lands on a form showing several"
    );
}

/// The single form's live verdict does not follow the person into a split: the
/// figure it judges is left behind there.
#[test]
fn the_single_amount_warning_does_not_haunt_a_split() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "5".to_owned(),
    });
    assert!(sut.view().amount_warning.is_some(), "5 of a 2 ETH balance");
    sut.dispatch(Event::EnterSplitMode);
    let view = sut.view();
    assert_eq!(view.amount_warning, None);
    assert!(
        view.split_over_balance,
        "the split's own verdict says it instead"
    );
}

/// A list brought to a form that already has people on it is more people.
#[test]
fn appending_keeps_what_was_typed_and_drops_the_blank_rows() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "0.5".to_owned(),
    });
    sut.dispatch(Event::EnterSplitMode); // the typed row + one blank
    sut.dispatch(Event::OpenBatchImport);
    assert_eq!(
        sut.view().split_import_room,
        BATCH_MAX_RECIPIENTS as u32 - 1
    );

    sut.dispatch(Event::AppendSplitRecipients {
        recipients: vec![SendRecipientDraft {
            id: String::new(),
            address: RECIPIENT_B.to_owned(),
            amount: "0.25".to_owned(),
            name: Some("Bob".to_owned()),
        }],
    });
    let view = sut.view();
    assert!(view.split_mode);
    assert!(
        !view.show_batch_import,
        "the sheet is shut, as the seed shuts it"
    );
    let rows: Vec<_> = view
        .recipients
        .iter()
        .map(|r| (r.address.as_str(), r.amount.as_str(), r.name.as_deref()))
        .collect();
    assert_eq!(
        rows,
        vec![(RECIPIENT, "0.5", None), (RECIPIENT_B, "0.25", Some("Bob"))],
        "typed row first, blank row gone, imported row after"
    );
    assert_eq!(view.recipients[0].id, "rcpt_1", "a kept row keeps its id");
    assert!(view.recipients[1].id.starts_with("rcpt_"));
    assert_eq!(view.confirm_amount, "0.75");
}

/// The older event is untouched: every shell that still sends it gets exactly
/// what it always got.
#[test]
fn seeding_still_replaces() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::SeedSplitRecipients {
        recipients: vec![draft("", RECIPIENT_B, "0.25")],
    });
    let view = sut.view();
    assert_eq!(view.recipients.len(), 1);
    assert_eq!(view.recipients[0].address, RECIPIENT_B);
}

/// From the single form the recipient being typed becomes the first row, as
/// `EnterSplitMode` would have made it.
#[test]
fn appending_from_the_single_form_carries_the_recipient_in() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::AppendSplitRecipients {
        recipients: vec![draft("", RECIPIENT_B, "0.25")],
    });
    let view = sut.view();
    let addresses: Vec<_> = view.recipients.iter().map(|r| r.address.as_str()).collect();
    assert_eq!(addresses, vec![RECIPIENT, RECIPIENT_B]);
    assert_eq!(
        view.recipients[0].amount, "",
        "no amount was typed, so none is invented"
    );
}

/// A repeat between a typed row and an appended one is WARNED, like any other
/// repeat between rows entered on purpose — never silently dropped.
#[test]
fn an_appended_repeat_of_a_typed_payee_is_named_not_dropped() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![draft("rcpt_1", RECIPIENT, "0.1"), draft("rcpt_2", "", "")],
    });
    sut.dispatch(Event::AppendSplitRecipients {
        recipients: vec![draft("", RECIPIENT, "0.2")],
    });
    let view = sut.view();
    assert_eq!(view.recipients.len(), 2);
    assert_eq!(view.split_duplicates.len(), 1);
    assert_eq!(view.split_duplicates[0].first_ordinal, 1);
    assert!(view.can_continue, "warned, not refused");
}

/// Two imports in a row from a shell that numbers its rows the same way each
/// time: every row still gets an id of its own.
#[test]
fn appended_rows_never_share_an_id() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    let batch = || {
        vec![
            draft("b0", RECIPIENT, "0.1"),
            draft("b1", RECIPIENT_B, "0.1"),
        ]
    };
    sut.dispatch(Event::AppendSplitRecipients {
        recipients: batch(),
    });
    sut.dispatch(Event::AppendSplitRecipients {
        recipients: batch(),
    });
    let view = sut.view();
    assert_eq!(view.recipients.len(), 4);
    let mut ids: Vec<_> = view.recipients.iter().map(|r| r.id.clone()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(
        ids.len(),
        4,
        "ids are what the warnings and the picker point at"
    );
    // …and the repeats are named against the right rows.
    let repeats: Vec<_> = view
        .split_duplicates
        .iter()
        .map(|d| (d.id.clone(), d.first_ordinal))
        .collect();
    assert_eq!(
        repeats,
        vec![
            (view.recipients[2].id.clone(), 1),
            (view.recipients[3].id.clone(), 2)
        ]
    );
}

/// Entering a split twice is not a way to lose one: a shell whose "+ add
/// recipient" sends `EnterSplitMode` while already in a split used to rebuild
/// the rows as `[the single recipient, blank]`.
#[test]
fn entering_a_split_twice_keeps_the_rows() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::SeedSplitRecipients {
        recipients: vec![
            draft("", RECIPIENT, "0.1"),
            draft("", RECIPIENT_B, "0.2"),
            draft("", RECIPIENT, "0.3"),
        ],
    });
    let before = sut.view();
    sut.dispatch(Event::EnterSplitMode);
    assert_eq!(sut.view().recipients, before.recipients);
}

/// The wire is JSON and three shells write it by hand. The old event, in the
/// shape they send today, still means what it meant; the new one is reachable
/// by its tag alone.
#[test]
fn the_seed_and_append_events_read_from_the_json_shells_send() {
    let seed: Event = serde_json::from_str(&format!(
        r#"{{"type":"seed_split_recipients","recipients":[{{"id":"b0","address":"{RECIPIENT}","amount":"1","name":null}}]}}"#
    ))
    .expect("the shape every shell sends today");
    assert!(matches!(seed, Event::SeedSplitRecipients { ref recipients } if recipients.len() == 1));

    let append: Event = serde_json::from_str(&format!(
        r#"{{"type":"append_split_recipients","recipients":[{{"id":"","address":"{RECIPIENT}","amount":"1","name":"Alice"}}]}}"#
    ))
    .expect("the new variant");
    assert!(matches!(append, Event::AppendSplitRecipients { .. }));
}

/// The cap holds across an append, and the room the importer is opened with is
/// what is actually left.
#[test]
fn appending_respects_the_cap_and_reports_the_room() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    let many = |n: usize| -> Vec<SendRecipientDraft> {
        (0..n)
            .map(|i| draft("", RECIPIENT, &format!("{}", i + 1)))
            .collect()
    };
    sut.dispatch(Event::SeedSplitRecipients {
        recipients: many(58),
    });
    assert_eq!(sut.view().split_import_room, 2);
    sut.dispatch(Event::AppendSplitRecipients {
        recipients: many(5),
    });
    let view = sut.view();
    assert_eq!(view.recipients.len(), BATCH_MAX_RECIPIENTS);
    assert_eq!(view.split_import_room, 0);
}

/// The headline on the signing page and the sum the money gates read are the
/// same number, in every mode. The shell used to add the rows up itself.
#[test]
fn the_confirm_headline_is_the_machines_own_total_in_every_mode() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "0.5".to_owned(),
    });
    // 1→1: the headline restates the resolved figure.
    assert_eq!(sut.view().confirm_amount, "0.5");
    assert_eq!(sut.view().confirm_amount, sut.view().token_amount);

    sut.dispatch(Event::EnterSplitMode);
    let row = |id: &str, to: &str, amount: &str| SendRecipientDraft {
        id: id.to_owned(),
        address: to.to_owned(),
        amount: amount.to_owned(),
        name: None,
    };
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            row("rcpt_1", RECIPIENT, "0.5"),
            row("rcpt_2", RECIPIENT_B, "0.25"),
        ],
    });
    let view = sut.view();
    // The sum, not the single-send field the rows were seeded from.
    assert_eq!(view.confirm_amount, "0.75");
    assert_eq!(
        to_base_units(&view.confirm_amount, 18),
        sum_split_base_units(&view.recipients, 18),
        "the headline and the gate's total are one number",
    );

    // A row this machine declines answers "" — never a shell exception on the
    // page the user is signing.
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            row("rcpt_1", RECIPIENT, "1,5"),
            row("rcpt_2", RECIPIENT_B, "0.25"),
        ],
    });
    assert_eq!(sut.view().confirm_amount, "");

    // multiSelect has no single headline — its rows come from `multi_specs`.
    let mut sweep = boot(vec![eth("2"), usdc("5")]);
    sweep.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    sweep.dispatch(Event::ToggleAllMultiTokens {
        visible_ids: vec![eth("2").id(), usdc("5").id()],
    });
    sweep.dispatch(Event::ConfirmMultiSelection);
    assert!(sweep.view().multi_select_mode);
    assert_eq!(sweep.view().confirm_amount, "");
}

#[test]
fn split_preview_estimate_and_signed_batch_use_the_same_calls() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            SendRecipientDraft {
                id: "rcpt_1".to_owned(),
                address: RECIPIENT.to_owned(),
                amount: "0.5".to_owned(),
                name: Some("Bob".to_owned()),
            },
            SendRecipientDraft {
                id: "rcpt_2".to_owned(),
                address: RECIPIENT_B.to_owned(),
                amount: "0.25".to_owned(),
                name: None,
            },
        ],
    });
    let ops = sut.dispatch(Event::Continue);
    let estimate_batch = match &ops[0] {
        Op::EstimateFee {
            batch: Some(batch),
            tx: None,
            ..
        } => batch.clone(),
        other => panic!("expected batched estimate, got {other:?}"),
    };
    assert!(sut.resolve(fee_ok(native_fee(1, 1_000))).is_empty());
    let probes = sut.resolve(covered());
    sut.drop_oldest(); // timer
    for op in probes {
        match op {
            Op::ResolveRisk { .. } => {
                sut.resolve(Res::RiskResolved { risk: None });
            }
            Op::SimulateCalls { calls, .. } => {
                assert_eq!(calls, estimate_batch, "sim previews the same legs");
                sut.resolve(Res::SimResolved { sim_json: None });
            }
            other => panic!("unexpected probe {other:?}"),
        }
    }
    let submit = slide_to_submit(&mut sut);
    let Op::SubmitUserOp { calls, .. } = &submit else {
        unreachable!()
    };
    // ⑩: `buildSplitCalls` is the ONE helper — preview == signed batch.
    assert_eq!(*calls, estimate_batch);
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].to, RECIPIENT);
    assert_eq!(calls[0].value, "500000000000000000");
}

// ===========================================================================
// MultiSelect (invariant ⑪)
// ===========================================================================

#[test]
fn changing_the_network_filter_clears_the_selection() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    sut.dispatch(Event::ToggleMultiToken {
        token_id: eth("2").id(),
    });
    assert_eq!(sut.view().multi_selected_ids.len(), 1);
    sut.dispatch(Event::SetMultiNetwork {
        chain_id: Some(137),
    });
    let view = sut.view();
    assert!(
        view.multi_selected_ids.is_empty(),
        "⑪: a batch is one chain"
    );
    assert_eq!(view.multi_chain_id, Some(137));
}

#[test]
fn toggle_all_selects_only_valuable_tokens_and_toggles_off_again() {
    let spam = SendToken {
        spam: true,
        ..usdc("9")
    };
    let unpriced = SendToken {
        price_usd: None,
        symbol: "MYST".to_owned(),
        ..dai("3")
    };
    let mut sut = boot(vec![eth("2"), usdc("5"), spam.clone(), unpriced.clone()]);
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    let all_on_screen: Vec<String> = [eth("2"), usdc("5"), spam, unpriced]
        .iter()
        .map(|t| t.id())
        .collect();
    assert_eq!(
        sut.view().multi_valuable_ids.len(),
        2,
        "the projection behind the master tick excludes spam/unpriced too"
    );
    sut.dispatch(Event::ToggleAllMultiTokens {
        visible_ids: all_on_screen.clone(),
    });
    let view = sut.view();
    assert_eq!(view.multi_selected_ids.len(), 2, "spam/unpriced excluded");
    sut.dispatch(Event::ToggleAllMultiTokens {
        visible_ids: all_on_screen,
    });
    assert!(sut.view().multi_selected_ids.is_empty());
}

/// The sweep is scoped to what the picker is SHOWING. A search box narrowed to
/// one row must sweep that row only — never every valuable token on the chain.
#[test]
fn toggle_all_sweeps_only_the_rows_the_picker_is_showing() {
    let mut sut = boot(vec![eth("2"), usdc("5"), dai("3")]);
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    sut.dispatch(Event::ToggleAllMultiTokens {
        visible_ids: vec![usdc("5").id()],
    });
    assert_eq!(
        sut.view().multi_selected_ids,
        vec![usdc("5").id()],
        "a filtered picker never sweeps what it is hiding"
    );
    // A second tap over the same narrowed list clears only that row.
    sut.dispatch(Event::ToggleMultiToken {
        token_id: eth("2").id(),
    });
    sut.dispatch(Event::ToggleAllMultiTokens {
        visible_ids: vec![usdc("5").id()],
    });
    assert_eq!(
        sut.view().multi_selected_ids,
        vec![eth("2").id()],
        "the hidden row keeps whatever the user did to it by hand"
    );
    // Ids the machine does not hold cannot select anything.
    sut.dispatch(Event::ToggleAllMultiTokens {
        visible_ids: vec!["1_0xdead_GHOST".to_owned()],
    });
    assert_eq!(sut.view().multi_selected_ids, vec![eth("2").id()]);
}

#[test]
fn confirming_a_single_selection_is_a_normal_amount_send() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    sut.dispatch(Event::ToggleMultiToken {
        token_id: usdc("5").id(),
    });
    let ops = sut.dispatch(Event::ConfirmMultiSelection);
    assert!(matches!(ops.as_slice(), [Op::LoadAccountCredential { .. }]));
    let view = sut.view();
    assert!(!view.multi_select_mode, "one token = amount-send");
    assert_eq!(
        view.selected_token.as_ref().map(|t| t.symbol.as_str()),
        Some("USDC")
    );
}

#[test]
fn multi_select_reserves_the_fee_asset_identically_in_preview_and_signature() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    sut.dispatch(Event::ToggleMultiToken {
        token_id: eth("2").id(),
    });
    sut.dispatch(Event::ToggleMultiToken {
        token_id: usdc("5").id(),
    });
    let ops = sut.dispatch(Event::ConfirmMultiSelection);
    assert!(matches!(ops.as_slice(), [Op::LoadAccountCredential { .. }]));
    let ops = sut.resolve(credential(Some(PK)));
    assert!(matches!(ops.as_slice(), [Op::EstimateFee { .. }]));
    // Warm quote: native fee 0.5 ETH → the native line is trimmed by it.
    assert!(sut
        .resolve(fee_ok(native_fee(1, 500_000_000_000_000_000)))
        .is_empty());
    set_recipient(&mut sut, RECIPIENT);
    let view = sut.view();
    // ⑪: the preview shows the EXACT reserved amounts.
    assert_eq!(view.multi_specs.len(), 2);
    assert_eq!(
        view.multi_specs[0].amount, "1.5",
        "native net of prefund reserve"
    );
    assert_eq!(view.multi_specs[1].amount, "5");

    let ops = sut.dispatch(Event::Continue);
    let estimate_batch = match &ops[0] {
        Op::EstimateFee {
            batch: Some(batch), ..
        } => batch.clone(),
        other => panic!("expected batch estimate, got {other:?}"),
    };
    // The estimate prices the RAW legs (no circular fee dependency).
    assert_eq!(estimate_batch[0].value, "2000000000000000000");
    assert!(sut
        .resolve(fee_ok(native_fee(1, 500_000_000_000_000_000)))
        .is_empty());
    let probes = sut.resolve(covered());
    sut.drop_oldest();
    for op in probes {
        match op {
            Op::ResolveRisk { .. } => {
                sut.resolve(Res::RiskResolved { risk: None });
            }
            Op::SimulateCalls { .. } => {
                sut.resolve(Res::SimResolved { sim_json: None });
            }
            other => panic!("unexpected probe {other:?}"),
        }
    }
    let submit = slide_to_submit(&mut sut);
    let Op::SubmitUserOp { calls, .. } = &submit else {
        unreachable!()
    };
    // ⑪: signed amounts == the previewed reserved specs.
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0].to, RECIPIENT);
    assert_eq!(calls[0].value, "1500000000000000000");
    assert_eq!(calls[1].to, USDC);
}

#[test]
fn multi_select_with_an_erc20_fee_trims_that_line_by_twice_the_quote() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    sut.dispatch(Event::ToggleMultiToken {
        token_id: eth("2").id(),
    });
    sut.dispatch(Event::ToggleMultiToken {
        token_id: usdc("5").id(),
    });
    sut.dispatch(Event::ConfirmMultiSelection);
    sut.resolve(credential(Some(PK)));
    sut.resolve(fee_ok(usdc_fee(1, 1_000_000)));
    let view = sut.view();
    // ⑪: 2× the quote is reserved from the fee-asset line only.
    assert_eq!(view.multi_specs[0].amount, "2", "native untouched");
    assert_eq!(view.multi_specs[1].amount, "3", "5 − 2×1 USDC");
}

// ===========================================================================
// Continue gate (invariant ②)
// ===========================================================================

#[test]
fn invalid_recipient_or_amount_never_reaches_confirm() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, "not-an-address");
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    let ops = sut.dispatch(Event::Continue);
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::InvalidAddress
        }]
    );
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "0".to_owned(),
    });
    let ops = sut.dispatch(Event::Continue);
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::InvalidAmount
        }]
    );
    assert_eq!(sut.view().stage, SendStage::EnterDetails);
}

#[test]
fn an_active_warning_blocks_continue_with_the_insufficient_alert() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "3".to_owned(),
    });
    let ops = sut.dispatch(Event::Continue);
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::InsufficientBalance {
                warning: Some(SendAmountWarning::NotEnoughToken {
                    symbol: "ETH".to_owned()
                })
            }
        }]
    );
}

#[test]
fn estimate_failure_surfaces_and_never_advances_with_a_fabricated_preview() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    let ops = sut.dispatch(Event::Continue);
    assert_eq!(ops.len(), 3);
    drain_form_quote(&mut sut);
    assert!(sut.view().estimating_gas);
    let ops = sut.resolve(Res::FeeEstimated {
        outcome: SendFeeOutcome::Failed {
            kind: SendEstimateFailure::QuoteUnavailable,
        },
    });
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::EstimateFailed {
                kind: SendEstimateFailure::QuoteUnavailable
            }
        }]
    );
    let view = sut.view();
    assert_eq!(
        view.stage,
        SendStage::EnterDetails,
        "② never a fake preview"
    );
    assert!(!view.estimating_gas);
    assert!(view.fee.is_none());
    // A late treasury answer for the failed run changes nothing.
    let ops = sut.resolve(covered());
    assert!(ops.is_empty());
    assert_eq!(sut.view().stage, SendStage::EnterDetails);
}

#[test]
fn estimate_timeout_stays_put_and_a_late_quote_still_lands_ported_verbatim() {
    let mut sut = Flex::new();
    sut.dispatch(open_event(SendOpenParams::default()));
    sut.resolve_where(
        |op| matches!(op, Op::FetchTokens { .. }),
        loaded(vec![eth("2")]),
    );
    sut.dispatch(Event::SelectToken {
        token_id: eth("2").id(),
    });
    sut.resolve_where(
        |op| matches!(op, Op::LoadAccountCredential { .. }),
        credential(Some(PK)),
    );
    // The warm quote (Phase 10) is refused, so the pre-check's estimate below
    // is the only `EstimateFee` left for the late answer to land on.
    sut.resolve_where(
        |op| matches!(op, Op::EstimateFee { tx: None, .. }),
        Res::FeeEstimated {
            outcome: SendFeeOutcome::Failed {
                kind: SendEstimateFailure::QuoteUnavailable,
            },
        },
    );
    sut.dispatch(Event::SetRecipient {
        recipient: RECIPIENT.to_owned(),
    });
    sut.resolve_where(
        |op| matches!(op, Op::ResolveIdentity { .. }),
        Res::IdentityResolved { identity: None },
    );
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    sut.dispatch(Event::Continue);
    // The 15s timer fires FIRST.
    let ops = sut.resolve_where(
        |op| {
            matches!(
                op,
                Op::StartTimer {
                    tag: SendTimerTag::EstimateTimeout,
                    ..
                }
            )
        },
        Res::TimerElapsed {
            tag: SendTimerTag::EstimateTimeout,
        },
    );
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::EstimateFailed {
                kind: SendEstimateFailure::Timeout
            }
        }]
    );
    let view = sut.view();
    assert_eq!(
        view.stage,
        SendStage::EnterDetails,
        "② timeout never advances"
    );
    assert!(!view.estimating_gas);
    // The raced-out treasury answer is dropped…
    let ops = sut.resolve_where(|op| matches!(op, Op::ProbeTreasury { .. }), low_float());
    assert!(ops.is_empty());
    assert!(sut.view().treasury_bootstrap.is_none());
    // …but a late successful estimate still lands (TS `setFeeEstimate` after
    // the race — ported verbatim).
    sut.resolve_where(
        |op| matches!(op, Op::EstimateFee { .. }),
        fee_ok(native_fee(1, 1_000)),
    );
    assert!(sut.view().fee.is_some());
    assert_eq!(
        sut.view().stage,
        SendStage::EnterDetails,
        "still no advance"
    );
}

#[test]
fn a_depleted_treasury_opens_the_bootstrap_sheet_instead_of_confirm() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    sut.dispatch(Event::Continue);
    drain_form_quote(&mut sut);
    assert!(sut.resolve(fee_ok(native_fee(1, 1_000))).is_empty());
    let ops = sut.resolve(low_float());
    assert!(ops.is_empty(), "no confirm probes: {ops:?}");
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails);
    assert!(view.treasury_bootstrap.is_some());
    // Retry from the sheet re-runs the whole pre-confirm flow.
    let ops = sut.dispatch(Event::RetryAfterBootstrap);
    assert!(
        matches!(
            ops.as_slice(),
            [
                Op::EstimateFee { .. },
                Op::ProbeTreasury { .. },
                Op::StartTimer { .. }
            ]
        ),
        "{ops:?}"
    );
}

// ===========================================================================
// Displayed = signed (invariant ①) and the confirm gates
// ===========================================================================

fn to_confirm_native(sut: &mut Sut, amount: &str, fee: FeeEstimateView) {
    select_eth(sut);
    set_recipient(sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: amount.to_owned(),
    });
    continue_to_confirm(sut, fee);
}

#[test]
fn the_signed_quote_is_exactly_the_displayed_estimate() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 42_000_000_000_000));
    let view = sut.view();
    let displayed = view.fee.expect("estimate displayed");
    assert_eq!(displayed.total_wei, "42000000000000");
    let submit = slide_to_submit(&mut sut);
    let Op::SubmitUserOp {
        quoted_fee: Some(quoted),
        calls,
        max_fee_per_gas,
        ..
    } = &submit
    else {
        panic!("in-band quote expected: {submit:?}");
    };
    // ①: amount + recipient are byte-identical to the displayed quote.
    assert_eq!(quoted.amount, displayed.total_wei);
    assert_eq!(quoted.recipient, FEE_COLLECTOR);
    assert_eq!(max_fee_per_gas.as_deref(), Some("1000"));
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].value, "1000000000000000000");
    assert_eq!(calls[0].data, "0x");
}

/// Spec 069: the submission names the speed the displayed fee was priced at,
/// taken from the same estimate as the amount — so the relay is told the tier
/// the person saw, and no shell keeps a second copy of which speed this is.
#[test]
fn the_signed_quote_names_the_tier_it_was_priced_at() {
    for tier in [FeeTier::Fast, FeeTier::Standard, FeeTier::Slow] {
        let mut sut = boot(vec![eth("2")]);
        let mut fee = native_fee(1, 42_000_000_000_000);
        fee.tier = tier;
        to_confirm_native(&mut sut, "1", fee);
        let submit = slide_to_submit(&mut sut);
        let Op::SubmitUserOp {
            quoted_fee: Some(quoted),
            ..
        } = &submit
        else {
            panic!("in-band quote expected: {submit:?}");
        };
        assert_eq!(quoted.tier, Some(tier));
    }
}

/// The dead `rapid` is never put on the wire: the relay refuses it with
/// -32602, and a relabelling to a neighbouring tier would name a speed this
/// fee was not priced at. It names nothing — the pre-068 wire.
#[test]
fn a_quote_at_the_dead_tier_names_no_tier() {
    let mut sut = boot(vec![eth("2")]);
    let mut fee = native_fee(1, 42_000_000_000_000);
    fee.tier = FeeTier::Rapid;
    to_confirm_native(&mut sut, "1", fee);
    let submit = slide_to_submit(&mut sut);
    let Op::SubmitUserOp {
        quoted_fee: Some(quoted),
        ..
    } = &submit
    else {
        panic!("in-band quote expected: {submit:?}");
    };
    assert_eq!(quoted.tier, None);
}

#[test]
fn a_requote_before_the_slide_signs_the_new_number_not_the_old_one() {
    let mut sut = boot(vec![usdc("100")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "10".to_owned(),
    });
    continue_to_confirm(&mut sut, usdc_fee(1, 1_000_000));
    // The GasFeeCard re-quotes: $1 → $2. The sim re-runs (it depends on the
    // estimate) — settle its probes.
    let probes = sut.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 2_000_000),
    });
    for op in probes {
        match op {
            Op::ResolveRisk { .. } => {
                sut.resolve(Res::RiskResolved { risk: None });
            }
            Op::SimulateCalls { .. } => {
                sut.resolve(Res::SimResolved { sim_json: None });
            }
            other => panic!("unexpected probe {other:?}"),
        }
    }
    let submit = slide_to_submit(&mut sut);
    let Op::SubmitUserOp {
        quoted_fee: Some(quoted),
        ..
    } = &submit
    else {
        panic!("quote expected");
    };
    assert_eq!(quoted.amount, "2000000", "① holds across requotes");
}

#[test]
fn fee_busy_or_estimating_disables_the_confirm_slide() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    assert!(sut.view().can_confirm);
    sut.dispatch(Event::FeeBusyChanged { busy: true });
    assert!(!sut.view().can_confirm, "re-quoting must disable confirm");
    sut.dispatch(Event::FeeBusyChanged { busy: false });
    assert!(sut.view().can_confirm);
}

#[test]
fn a_quote_from_another_chain_is_never_shown_or_signed() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(137, 999),
    });
    assert!(sut.view().fee.is_none(), "①: chain-guarded display");
}

// ===========================================================================
// Same-asset ceiling (invariant ⑧)
// ===========================================================================

#[test]
fn a_doomed_same_asset_batch_never_reaches_the_passkey() {
    let mut sut = boot(vec![usdc("5")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "4.5".to_owned(),
    });
    // The fee ($1, in USDC) is learned only at confirm: 4.5 + 1 > 5.
    continue_to_confirm(&mut sut, usdc_fee(1, 1_000_000));
    let view = sut.view();
    let issue = view.same_asset_fee_issue.expect("⑧: ceiling surfaced");
    assert_eq!(issue.transfer_amount, "4500000");
    assert_eq!(issue.fee_amount, "1000000");
    assert_eq!(issue.max_transfer_amount, "4000000");
    assert!(!view.can_confirm);
    // The slide routes to "edit amount" — no lock, no signing. (Back on the
    // form with its fee coin reset, the form quote re-arms; that is the one
    // op allowed here, and it is not a submit.)
    let ops = without_form_quote(sut.dispatch(Event::SlideConfirm));
    assert!(ops.is_empty(), "⑧: {ops:?}");
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails);
    assert_eq!(view.tx_status, SendTxStatus::Idle);
    assert!(!view.sending);
}

// ===========================================================================
// Pre-sign treasury recheck (invariant ⑭)
// ===========================================================================

#[test]
fn the_race_window_after_preflight_is_covered_by_a_pre_sign_recheck() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    let ops = sut.dispatch(Event::SlideConfirm);
    assert!(matches!(ops.as_slice(), [Op::ProbeTreasury { .. }]), "⑭");
    assert_eq!(sut.view().tx_status, SendTxStatus::Preparing);
    // The float fell below its floor after the preflight.
    let ops = sut.resolve(low_float());
    assert!(ops.is_empty(), "⑭: no SubmitUserOp: {ops:?}");
    let view = sut.view();
    assert!(view.treasury_bootstrap.is_some());
    assert_eq!(view.tx_status, SendTxStatus::Idle);
    assert!(!view.sending, "lock released — a retry can start");
    // And the retry does start.
    sut.dispatch(Event::DismissTreasurySheet);
    let ops = sut.dispatch(Event::SlideConfirm);
    assert!(matches!(ops.as_slice(), [Op::ProbeTreasury { .. }]));
}

// ===========================================================================
// Cancel semantics (invariants ③ and ④)
// ===========================================================================

#[test]
fn cancel_during_the_pre_sign_window_never_resurrects_the_passkey() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    let ops = sut.dispatch(Event::SlideConfirm);
    assert!(matches!(ops.as_slice(), [Op::ProbeTreasury { .. }]));
    // Cancel while the treasury recheck is still in flight (the pre-sign
    // window — TS's ~20s class of awaits).
    let ops = sut.dispatch(Event::CancelSigning);
    assert_eq!(ops, vec![Op::CancelPasskeySign]);
    let view = sut.view();
    assert_eq!(view.tx_status, SendTxStatus::Idle);
    assert!(!view.sending);
    // The recheck answers late: NOTHING may happen — no submit, no sheet.
    let ops = sut.resolve(covered());
    assert!(ops.is_empty(), "③: passkey must not resurrect: {ops:?}");
    assert_eq!(sut.view().tx_status, SendTxStatus::Idle);
}

#[test]
fn back_is_refused_while_a_transaction_is_in_progress() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    sut.dispatch(Event::SlideConfirm);
    let ops = sut.dispatch(Event::Back);
    assert!(ops.is_empty());
    assert_eq!(sut.view().stage, SendStage::Confirm, "③: back refused");
    // After the flow settles back to idle, back works again.
    sut.dispatch(Event::CancelSigning);
    sut.dispatch(Event::Back);
    assert_eq!(sut.view().stage, SendStage::EnterDetails);
}

#[test]
fn a_cancelled_runs_stale_result_never_touches_the_retrys_lock() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    // First send reaches the in-flight submit, then the user cancels during
    // signing.
    let _old_submit = slide_to_submit(&mut sut);
    sut.dispatch(Event::SigningStarted);
    assert_eq!(sut.view().tx_status, SendTxStatus::Signing);
    sut.dispatch(Event::CancelSigning);
    assert!(!sut.view().sending, "cancel releases the lock immediately");
    // Retry: a NEW send acquires a new generation.
    let ops = sut.dispatch(Event::SlideConfirm);
    assert!(matches!(ops.as_slice(), [Op::ProbeTreasury { .. }]));
    assert!(sut.view().sending);
    // The CANCELLED run's submit settles now (oldest outstanding op) — it
    // belongs to a dead generation and must not confirm anything nor release
    // the retry's lock (issue #91 / invariant ④).
    let ops = sut.resolve(Res::SubmitFailed {
        failure: SendSubmitFailure::PasskeyCancelled,
    });
    assert!(ops.is_empty());
    let view = sut.view();
    assert!(view.sending, "④: the retry still holds its lock");
    assert_eq!(view.tx_status, SendTxStatus::Preparing);
    // The fire-and-forget CancelPasskeySign from the cancel is next in the
    // FIFO; the shell never answers it.
    sut.drop_oldest();
    // The retry completes normally.
    let ops = sut.resolve(covered());
    assert!(matches!(ops.as_slice(), [Op::SubmitUserOp { .. }]));
}

/// **The confirm slider disarms when the money stops resolving — and says so.**
///
/// `can_confirm` looked only at the fee and the pipeline: the amount was
/// checked once, by `Continue`, and never again. But the confirm page is a page
/// someone can sit on, and `display_changed` lands whenever the currency
/// commits. A CNY figure cannot be restated in USD (this screen has no cross
/// rate and inventing one is the defect the whole area exists to forbid), so
/// `redenominate_to_display` drops it — leaving a fully armed slide over
/// nothing at all. This is the confirm-stage twin of the hole the previous
/// round closed on `can_continue`.
///
/// Mutation proof: remove `&& confirm_amount_ok` from `can_confirm` and the
/// `!view.can_confirm` assertion fails — the slider is armed on a `"0"` amount,
/// which is exactly a signable zero-value transfer.
#[test]
fn a_currency_commit_under_the_confirm_page_disarms_the_slide_and_says_why() {
    let mut sut = boot(vec![usdc("9000")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::DisplayChanged {
        display: cny_display(),
    });
    sut.dispatch(Event::ToggleFiatInput);
    sut.dispatch(Event::SetAmount {
        amount: "500".to_owned(),
    });
    continue_to_confirm(&mut sut, native_fee(1, 1_000));
    assert!(sut.view().can_confirm, "a resolvable figure is signable");

    // The display currency commits to USD while the review page is open.
    sut.dispatch(Event::DisplayChanged { display: display() });
    let view = sut.view();
    assert_eq!(view.stage, SendStage::Confirm, "still on the page");
    assert_eq!(view.amount, "", "the reviewed figure could not come across");
    assert_eq!(view.token_amount, "0", "and there is nothing to sign");
    assert!(!view.can_confirm, "so the slide is not armed");
    assert_eq!(
        view.confirm_amount_issue,
        Some(SendUnitIssue {
            code: "USD".to_owned(),
            symbol: "USDC".to_owned(),
        }),
        "and the refusal is not silent"
    );

    // The entry screen's gate agrees — one judgement, two pages.
    assert!(!view.can_continue);

    // And the machine does not rely on the shell honouring `can_confirm`: a
    // slide that arrives anyway signs nothing. `to_base_units("0", 6)` is a
    // perfectly valid `Some(0)`, so without this the build path would have
    // encoded a zero-value transfer and asked for a passkey over it.
    let ops = sut.dispatch(Event::SlideConfirm);
    assert!(
        !ops.iter()
            .any(|op| matches!(op, Op::SubmitUserOp { .. } | Op::ProbeTreasury { .. })),
        "a figure that does not resolve never reaches signing: {ops:?}"
    );
    assert_eq!(
        sut.view().stage,
        SendStage::EnterDetails,
        "back to the amount field — the same recovery the fee breach gets"
    );
}

/// The confirm-stage reason is scoped to the refusal: a healthy confirm page
/// says nothing, and the batch modes (whose money lives in `recipients` /
/// `multi_specs`, not in the amount field) are never accused of an empty one.
#[test]
fn a_healthy_confirm_page_and_the_batch_modes_are_never_accused_of_a_dead_amount() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    let view = sut.view();
    assert!(view.can_confirm);
    assert_eq!(view.confirm_amount_issue, None);

    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            SendRecipientDraft {
                id: "rcpt_1".to_owned(),
                address: RECIPIENT.to_owned(),
                amount: "0.5".to_owned(),
                name: None,
            },
            SendRecipientDraft {
                id: "rcpt_2".to_owned(),
                address: RECIPIENT_B.to_owned(),
                amount: "0.25".to_owned(),
                name: None,
            },
        ],
    });
    continue_to_confirm(&mut sut, native_fee(1, 1_000));
    let view = sut.view();
    assert!(view.can_confirm, "split money is not in `model.amount`");
    assert_eq!(view.confirm_amount_issue, None);
}

#[test]
fn double_slide_in_one_tick_starts_exactly_one_submit() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    let first = sut.dispatch(Event::SlideConfirm);
    assert_eq!(first.len(), 1);
    let second = sut.dispatch(Event::SlideConfirm);
    assert!(second.is_empty(), "④: the lock is synchronous");
}

// ===========================================================================
// Submission, records, receipt (invariants ⑤ ⑥ ⑦ ⑮)
// ===========================================================================

#[test]
fn a_single_send_persists_one_record_then_hands_off_to_the_tracker() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.dispatch(Event::SigningStarted);
    let ops = sut.resolve(submitted(HASH));
    // Haptic + cache clear + ONE atomic record write.
    let persist = ops
        .iter()
        .find_map(|op| match op {
            Op::PersistTxRecords { records } => Some(records.clone()),
            _ => None,
        })
        .expect("persist op present");
    assert_eq!(persist.len(), 1);
    assert_eq!(persist[0].id, HASH);
    assert_eq!(persist[0].user_op_hash, HASH);
    assert_eq!(persist[0].tx_hash, "");
    assert_eq!(persist[0].value, "1");
    assert_eq!(persist[0].usd.as_deref(), Some("$2000.00"));
    assert_eq!(persist[0].timestamp_s, 1_754_000_000.0);
    let view = sut.view();
    assert_eq!(view.tx_status, SendTxStatus::Confirmed);
    assert_eq!(view.stage, SendStage::Receipt);
    assert!(!view.sending);
    let receipt = view.receipt.expect("receipt view");
    assert_eq!(receipt.status, SendReceiptStatus::Submitted);
    assert_eq!(receipt.amount, "1");
    // ⑥: the tracker learns only AFTER the records landed.
    let (ids, op) = settle_persistence(&mut sut);
    assert_eq!(ids, vec![HASH.to_owned()]);
    assert_eq!(
        op,
        Op::TrackSubmitted {
            user_op_hash: HASH.to_owned(),
            record_ids: vec![HASH.to_owned()],
            chain_id: 1,
        }
    );
    assert!(sut.resolve(Res::TrackHandedOff).is_empty());
}

/// **A receipt reports the amount that was SIGNED, not one recomputed from
/// whatever rate is on screen now.**
///
/// `receipt_view` asked `model_token_amount` for its headline figure, which
/// re-runs the fiat↔token conversion against the CURRENT display context. That
/// is a live computation about a fact that stopped being live the instant the
/// calldata was signed: move the display currency after the payment and the
/// receipt's token amount moved with it — down to `0` once the rate was gone.
/// A number already on-chain is not the currency picker's to rewrite.
///
/// Mutation proof: put `model_token_amount(model, token)` back in
/// `receipt_view` and the post-payment assertions fail — `"69.735007"` becomes
/// `"0"` under the unpriced currency and `"500"` (the raw CNY digits, wearing a
/// USDC label) under a USD rate.
#[test]
fn a_receipt_shows_the_signed_amount_and_no_later_rate_can_restate_it() {
    // The native row is here because the fee below is native: an account with
    // no ETH cannot pay an ETH fee, and since issue #211 the slide says so
    // instead of signing an op that could only revert.
    let mut sut = boot(vec![eth("2"), usdc("9000")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::DisplayChanged {
        display: cny_display(),
    });
    sut.dispatch(Event::ToggleFiatInput);
    sut.dispatch(Event::SetAmount {
        amount: "500".to_owned(),
    });
    assert_eq!(sut.view().token_amount, "69.735007", "500 CNY at 7.17/USD");

    continue_to_confirm(&mut sut, native_fee(1, 1_000));
    let op = slide_to_submit(&mut sut);
    // What the passkey actually signed: 69_735_007 base units of a 6-decimal
    // token — `0x0428125f` in the transfer's second word.
    let Op::SubmitUserOp { calls, .. } = &op else {
        panic!("expected a submit, got {op:?}")
    };
    assert!(
        calls[0].data.starts_with("0xa9059cbb") && calls[0].data.ends_with("0428125f"),
        "signed calldata: {}",
        calls[0].data
    );

    sut.resolve(submitted(HASH));
    let receipt = sut.view().receipt.expect("receipt view");
    assert_eq!(receipt.amount, "69.735007");
    assert_eq!(receipt.usd_value, 69.735007);

    // The rate vanishes AFTER the payment. Nothing about a signature changes.
    sut.dispatch(Event::DisplayChanged {
        display: unpriced_display(),
    });
    let receipt = sut.view().receipt.expect("receipt view");
    assert_eq!(
        receipt.amount, "69.735007",
        "an on-chain transfer is not re-derived from today's rate"
    );
    assert_eq!(receipt.usd_value, 69.735007);

    // …and neither does a different currency with a perfectly good rate.
    sut.dispatch(Event::DisplayChanged { display: display() });
    assert_eq!(
        sut.view().receipt.expect("receipt view").amount,
        "69.735007"
    );
}

#[test]
fn batch_siblings_are_written_in_one_atomic_batch_with_suffixed_ids() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            SendRecipientDraft {
                id: "rcpt_1".to_owned(),
                address: RECIPIENT.to_owned(),
                amount: "0.5".to_owned(),
                name: Some("Bob".to_owned()),
            },
            SendRecipientDraft {
                id: "rcpt_2".to_owned(),
                address: RECIPIENT_B.to_owned(),
                amount: "0.25".to_owned(),
                name: None,
            },
        ],
    });
    continue_to_confirm(&mut sut, native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    let ops = sut.resolve(submitted(HASH));
    let records = ops
        .iter()
        .find_map(|op| match op {
            Op::PersistTxRecords { records } => Some(records.clone()),
            _ => None,
        })
        .expect("one atomic write");
    // ⑥: ALL siblings in the one write, each with its own `<hash>-<i>` id.
    assert_eq!(records.len(), 2);
    assert_eq!(records[0].id, format!("{HASH}-0"));
    assert_eq!(records[1].id, format!("{HASH}-1"));
    assert_eq!(records[0].to, RECIPIENT);
    assert_eq!(records[0].to_name.as_deref(), Some("Bob"));
    assert_eq!(records[1].to, RECIPIENT_B);
    let view = sut.view();
    let receipt = view.receipt.expect("receipt");
    assert_eq!(receipt.kind, Some(SendReceiptKind::Split));
    assert_eq!(receipt.transfers.len(), 2);
    let (ids, _) = settle_persistence(&mut sut);
    assert_eq!(ids, vec![format!("{HASH}-0"), format!("{HASH}-1")]);
}

#[test]
fn a_definitive_failure_stamps_the_receipt_but_never_unsubmits_the_payment() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.resolve(submitted(HASH));
    settle_persistence(&mut sut);
    sut.resolve(Res::TrackHandedOff);
    // A stale hash is ignored.
    sut.dispatch(Event::ReceiptUpdate {
        user_op_hash: "0xother".to_owned(),
        outcome: SendReceiptOutcome::Failed { rejected: false },
    });
    assert_eq!(
        sut.view().receipt.expect("receipt").status,
        SendReceiptStatus::Submitted
    );
    // The real one stamps Failed — while tx_status STAYS confirmed
    // (⑤: a submitted payment is never flipped back into an error).
    sut.dispatch(Event::ReceiptUpdate {
        user_op_hash: HASH.to_owned(),
        outcome: SendReceiptOutcome::Failed { rejected: false },
    });
    let view = sut.view();
    assert_eq!(view.tx_status, SendTxStatus::Confirmed);
    assert_eq!(view.tx_error, None);
    assert_eq!(
        view.receipt.expect("receipt").status,
        SendReceiptStatus::Failed
    );
}

#[test]
fn a_confirmed_hash_lights_the_explorer_link() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.resolve(submitted(HASH));
    settle_persistence(&mut sut);
    sut.dispatch(Event::ReceiptUpdate {
        user_op_hash: HASH.to_owned(),
        outcome: SendReceiptOutcome::Confirmed {
            tx_hash: "0xtx".to_owned(),
        },
    });
    let view = sut.view();
    assert_eq!(view.tx_hash.as_deref(), Some("0xtx"));
    assert_eq!(
        view.receipt.expect("receipt").status,
        SendReceiptStatus::Confirmed
    );
}

#[test]
fn fee_hold_is_waiting_not_failure() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.resolve(submitted(HASH));
    settle_persistence(&mut sut);
    sut.dispatch(Event::ReceiptUpdate {
        user_op_hash: HASH.to_owned(),
        outcome: SendReceiptOutcome::FeeHeld,
    });
    let view = sut.view();
    let receipt = view.receipt.expect("receipt");
    // ⑦: still submitted (pending), only the wording changes.
    assert_eq!(receipt.status, SendReceiptStatus::Submitted);
    assert_eq!(receipt.hold_reason, Some(SendHoldReason::FeeHold));
    assert_eq!(view.tx_status, SendTxStatus::Confirmed);
}

#[test]
fn raw_submit_errors_become_the_calm_semantic_key() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    let ops = sut.resolve(Res::SubmitFailed {
        failure: SendSubmitFailure::Other {
            message: Some("execution reverted: 0xdeadbeef gobbledygook".to_owned()),
        },
    });
    assert_eq!(
        ops,
        vec![Op::Haptic {
            kind: SendHapticKind::Error
        }]
    );
    let view = sut.view();
    // ⑮: the raw wording never reaches the money screen.
    assert_eq!(view.tx_status, SendTxStatus::Error);
    assert_eq!(view.tx_error, Some(SendTxErrorKey::Generic));
    assert!(!view.sending);
    // Retry resets to idle.
    sut.dispatch(Event::RetryAfterError);
    assert_eq!(sut.view().tx_status, SendTxStatus::Idle);
}

#[test]
fn passkey_cancel_is_never_an_error_state() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.dispatch(Event::SigningStarted);
    let ops = sut.resolve(Res::SubmitFailed {
        failure: SendSubmitFailure::PasskeyCancelled,
    });
    assert!(ops.is_empty(), "no haptic, no alert: {ops:?}");
    let view = sut.view();
    assert_eq!(view.tx_status, SendTxStatus::Idle);
    assert_eq!(view.tx_error, None);
    assert!(!view.sending);
}

#[test]
fn underfunded_bundler_rechecks_the_treasury_before_wording_the_error() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    let ops = sut.resolve(Res::SubmitFailed {
        failure: SendSubmitFailure::BundlerUnderfunded,
    });
    assert!(matches!(ops.as_slice(), [Op::ProbeTreasury { .. }]));
    // Healthy treasury → the bundler-fund error, NOT a personal top-up sheet.
    let ops = sut.resolve(covered());
    assert_eq!(
        ops,
        vec![Op::Haptic {
            kind: SendHapticKind::Error
        }]
    );
    let view = sut.view();
    assert_eq!(view.tx_error, Some(SendTxErrorKey::BundlerFund));
    assert!(view.treasury_bootstrap.is_none());
    assert!(!view.sending, "the finally released the lock");
}

#[test]
fn relayer_unavailable_with_a_depleted_treasury_shows_the_honest_bootstrap_ask() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    let ops = sut.resolve(Res::SubmitFailed {
        failure: SendSubmitFailure::RelayerUnavailable,
    });
    assert!(matches!(ops.as_slice(), [Op::ProbeTreasury { .. }]));
    let ops = sut.resolve(low_float());
    assert!(ops.is_empty());
    let view = sut.view();
    assert!(view.treasury_bootstrap.is_some());
    assert_eq!(view.tx_status, SendTxStatus::Idle, "not an error — an ask");
    assert_eq!(view.tx_error, None);
    assert!(!view.sending);
}

// ===========================================================================
// Leaving confirm, fee-asset reset
// ===========================================================================

#[test]
fn leaving_confirm_resets_the_fee_asset_and_clears_a_stale_erc20_estimate() {
    let mut sut = boot(vec![usdc("100")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "10".to_owned(),
    });
    continue_to_confirm(&mut sut, usdc_fee(1, 1_000_000));
    sut.dispatch(Event::ChooseFeeToken {
        token: Some(USDC.to_owned()),
    });
    assert_eq!(sut.view().gas_fee_token.as_deref(), Some(USDC));
    sut.dispatch(Event::Back);
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails);
    assert_eq!(view.gas_fee_token, None, "next entry re-quotes in native");
    // The erc20 estimate (totalWei = 0) is gone so downstream reserve math
    // never reads 0 (`useSendController.ts:467-473`).
    assert!(view.fee.is_none());
}

#[test]
fn a_native_estimate_survives_leaving_confirm() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    sut.dispatch(Event::Back);
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails);
    assert!(
        view.fee.is_some(),
        "native quote keeps gating the amount form"
    );
}

#[test]
fn back_from_enter_details_resets_single_mode_but_keeps_a_multi_selection() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    sut.dispatch(Event::Back);
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken);
    assert!(view.selected_token.is_none());
    assert_eq!(view.recipient, "");
    assert_eq!(view.amount, "");
    // Multi mode preserves the pick on back.
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    sut.dispatch(Event::ToggleMultiToken {
        token_id: eth("2").id(),
    });
    sut.dispatch(Event::ToggleMultiToken {
        token_id: usdc("5").id(),
    });
    sut.dispatch(Event::ConfirmMultiSelection);
    sut.resolve(credential(Some(PK)));
    sut.resolve(fee_ok(native_fee(1, 1_000)));
    sut.dispatch(Event::Back);
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken);
    assert_eq!(view.multi_selected_ids.len(), 2, "selection preserved");
    // Back from the first step closes the flow.
    let ops = sut.dispatch(Event::Back);
    assert_eq!(ops, vec![Op::Close]);
}

// ===========================================================================
// Scan routing (invariant ⑬)
// ===========================================================================

#[test]
fn a_row_scoped_scan_takes_only_the_address_and_spares_the_other_rows() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            SendRecipientDraft {
                id: "rcpt_1".to_owned(),
                address: RECIPIENT.to_owned(),
                amount: "0.5".to_owned(),
                name: None,
            },
            SendRecipientDraft {
                id: "rcpt_2".to_owned(),
                address: String::new(),
                amount: "0.25".to_owned(),
                name: None,
            },
        ],
    });
    // Row 2 opens the picker, then scans a FULL EIP-681 request.
    sut.dispatch(Event::OpenContactPicker {
        target: Some("rcpt_2".to_owned()),
    });
    sut.dispatch(Event::OpenScanner);
    let ops = sut.dispatch(Event::ScanResolved {
        scan: SendScan::Request {
            recipient: RECIPIENT_B.to_owned(),
            chain_id: Some(1),
            token_address: Some(USDC.to_owned()),
            amount_base_units: Some("123".to_owned()),
        },
    });
    // ⑬: no re-lock — no FetchTokens, no reset; only the row's address moves.
    assert!(ops.is_empty(), "{ops:?}");
    let view = sut.view();
    assert!(view.split_mode);
    assert_eq!(view.recipients[0].address, RECIPIENT);
    assert_eq!(view.recipients[0].amount, "0.5");
    assert_eq!(view.recipients[1].address, RECIPIENT_B);
    assert_eq!(view.recipients[1].amount, "0.25", "amount untouched");
    assert!(!view.locked);
}

/// The picker is the core's state (spec 028 US5): opening it is an event,
/// and a pick both fills the recipient and closes it — the shell never has
/// to say "and now close it" as a second sentence.
#[test]
fn a_pick_from_the_book_fills_the_recipient_and_closes_the_picker() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::OpenContactPicker { target: None });
    assert!(sut.view().show_contact_picker);
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT.to_owned(),
    });
    let view = sut.view();
    assert_eq!(view.recipient, RECIPIENT);
    assert!(!view.show_contact_picker, "the pick closes the picker");
    assert!(!view.split_mode);

    // A row-scoped pick does the same for its row.
    sut.dispatch(Event::EnterSplitMode);
    let rows = sut.view().recipients;
    let second = rows.get(1).map(|r| r.id.clone()).unwrap_or_default();
    sut.dispatch(Event::OpenContactPicker {
        target: Some(second.clone()),
    });
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT_B.to_owned(),
    });
    let view = sut.view();
    assert!(!view.show_contact_picker);
    assert!(view
        .recipients
        .iter()
        .any(|r| r.id == second && r.address == RECIPIENT_B));
}

#[test]
fn an_untargeted_full_request_relocks_the_whole_flow() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    sut.dispatch(Event::OpenScanner);
    let ops = sut.dispatch(Event::ScanResolved {
        scan: SendScan::Request {
            recipient: RECIPIENT.to_owned(),
            chain_id: Some(1),
            token_address: Some(USDC.to_owned()),
            amount_base_units: Some("1500000".to_owned()),
        },
    });
    // `router.replace` = a fresh locked mount: the token list reloads.
    assert!(
        matches!(ops.as_slice(), [Op::FetchTokens { .. }]),
        "{ops:?}"
    );
    assert!(sut.view().locked);
    let ops = without_form_quote(sut.resolve(loaded(vec![eth("2"), usdc("5")])));
    assert!(matches!(ops.as_slice(), [Op::ResolveIdentity { .. }]));
    let view = sut.view();
    assert_eq!(view.amount, "1.5");
    assert_eq!(view.recipient, RECIPIENT);
    assert_eq!(
        view.selected_token.as_ref().map(|t| t.symbol.as_str()),
        Some("USDC")
    );
}

#[test]
fn a_bare_address_scan_fills_the_recipient_field() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::OpenScanner);
    let ops = sut.dispatch(Event::ScanResolved {
        scan: SendScan::Text {
            data: RECIPIENT.to_owned(),
        },
    });
    assert!(matches!(ops.as_slice(), [Op::ResolveIdentity { .. }]));
    let view = sut.view();
    assert!(!view.show_scanner);
    assert_eq!(view.recipient, RECIPIENT);
    assert!(!view.locked);
}

/// Issue #270: "Scan to fill the address" is a row ON the contact picker. The
/// scan that filled the field is the pick — the picker goes with the scanner,
/// instead of coming back up over the address it just produced.
#[test]
fn a_scan_from_the_contact_picker_closes_the_picker_too() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::OpenContactPicker { target: None });
    sut.dispatch(Event::OpenScanner);
    assert!(sut.view().show_contact_picker);
    sut.dispatch(Event::ScanResolved {
        scan: SendScan::Text {
            data: RECIPIENT.to_owned(),
        },
    });
    let view = sut.view();
    assert!(!view.show_scanner);
    assert!(!view.show_contact_picker);
    assert_eq!(view.recipient, RECIPIENT);

    // The same from a request code without a chain: the address fills, both close.
    sut.dispatch(Event::OpenContactPicker { target: None });
    sut.dispatch(Event::OpenScanner);
    sut.dispatch(Event::ScanResolved {
        scan: SendScan::Request {
            recipient: RECIPIENT_B.to_owned(),
            chain_id: None,
            token_address: None,
            amount_base_units: None,
        },
    });
    let view = sut.view();
    assert!(!view.show_contact_picker);
    assert_eq!(view.recipient, RECIPIENT_B);
}

// ===========================================================================
// Continue credential path
// ===========================================================================

#[test]
fn a_missing_credential_alerts_account_unavailable_before_estimating() {
    let mut sut = boot(vec![eth("2")]);
    // Select without settling the prefetch (the shell never answered).
    let ops = sut.dispatch(Event::SelectToken {
        token_id: eth("2").id(),
    });
    assert_eq!(ops.len(), 1);
    sut.drop_oldest(); // prefetch lost
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    let ops = sut.dispatch(Event::Continue);
    assert!(matches!(ops.as_slice(), [Op::LoadAccountCredential { .. }]));
    drain_form_quote(&mut sut);
    let ops = sut.resolve(credential(None));
    assert_eq!(
        ops,
        vec![Op::ShowAlert {
            kind: SendAlertKind::AccountUnavailable
        }]
    );
    assert_eq!(sut.view().stage, SendStage::EnterDetails);
}

#[test]
fn the_estimate_op_carries_the_chosen_fee_token_and_the_real_call_shape() {
    let mut sut = boot(vec![usdc("100")]);
    sut.dispatch(Event::SelectToken {
        token_id: usdc("100").id(),
    });
    sut.resolve(credential(Some(PK)));
    sut.dispatch(Event::ChooseFeeToken {
        token: Some(USDC.to_owned()),
    });
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "10".to_owned(),
    });
    let ops = sut.dispatch(Event::Continue);
    match &ops[0] {
        Op::EstimateFee {
            chain_id,
            account,
            tx: Some(call),
            batch: None,
            gas_fee_token,
            public_key_hex,
        } => {
            assert_eq!(*chain_id, 1);
            assert_eq!(account, ACCOUNT);
            assert_eq!(gas_fee_token.as_deref(), Some(USDC));
            assert_eq!(public_key_hex.as_deref(), Some(PK));
            // ⑨-cousin: the REAL erc20 transfer calldata, not a padded model.
            assert_eq!(call.to, USDC);
            assert!(call.data.starts_with("0xa9059cbb"));
        }
        other => panic!("expected real-shape estimate, got {other:?}"),
    }
}

// ===========================================================================
// View wiring odds and ends
// ===========================================================================

#[test]
fn locked_amounts_are_not_editable() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(locked_params(Some(USDC), Some("1500000"), "1")));
    sut.resolve(loaded(vec![usdc("5")]));
    sut.dispatch(Event::SetAmount {
        amount: "99".to_owned(),
    });
    assert_eq!(sut.view().amount, "1.5", "locked amount stays");
}

#[test]
fn locked_recipients_are_not_editable_either() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(locked_params(Some(USDC), Some("1500000"), "1")));
    sut.resolve(loaded(vec![usdc("5")]));
    assert_eq!(sut.view().recipient, RECIPIENT);
    // The screen renders the field disabled; the machine refuses the edit too,
    // so the payee a scanned request names cannot be re-pointed.
    let ops = sut.dispatch(Event::SetRecipient {
        recipient: RECIPIENT_B.to_owned(),
    });
    assert!(ops.is_empty(), "no identity lookup for a refused edit");
    assert_eq!(sut.view().recipient, RECIPIENT, "locked recipient stays");
}

/// The owner's report, 2026-09-23: 「扫码到一个地址后无法切换发送资产，只能发送
/// 里面的默认代币」.
///
/// A scan skips the picker and takes the balance's top token — the person
/// never chose it — so the only way to a different asset was Back, and Back
/// threw the scanned address away. They scanned again, landed on the same
/// token, and read it as "the asset cannot be changed". Now the address
/// survives and the second token is one tap from the first.
#[test]
fn a_scanned_recipient_can_change_its_asset_without_being_scanned_again() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        prefilled_recipient: Some(RECIPIENT.to_owned()),
        ..SendOpenParams::default()
    }));
    // Two tokens, ETH worth more: the quick-send path picks it for the person.
    sut.resolve(loaded(vec![eth("2"), usdc("5")]));
    sut.drop_matching(|op| {
        matches!(
            op,
            Op::ResolveIdentity { .. } | Op::LoadAccountCredential { .. }
        )
    });
    let view = sut.view();
    assert_eq!(
        view.stage,
        SendStage::EnterDetails,
        "the picker was skipped"
    );
    assert_eq!(view.recipient, RECIPIENT);

    sut.dispatch(Event::Back);
    let view = sut.view();
    assert_eq!(
        view.stage,
        SendStage::SelectToken,
        "the picker is reachable"
    );
    assert_eq!(
        view.recipient, RECIPIENT,
        "and the scan is still in hand — changing the asset costs nothing"
    );
}

#[test]
fn an_unlocked_prefill_still_carries_its_recipient_across_a_token_change() {
    // The recovery path the lock must not eat: a contact tapped "Send"
    // prefills the recipient WITHOUT locking, and the field stays re-settable.
    //
    // Back used to CLEAR it, and the web shell put it back by hand
    // (`changeToken` = Back + SetRecipient) — a dance the native shells never
    // learned, which is why a scanned address could not change its asset
    // (owner, 2026-09-23). Back now keeps a handed-in recipient, and this test
    // keeps its own subject: re-setting it still works.
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        prefilled_recipient: Some(RECIPIENT.to_owned()),
        ..SendOpenParams::default()
    }));
    sut.resolve(loaded(vec![eth("2"), usdc("5")]));
    // The prefill resolved an identity and warmed a quote (Phase 10); neither
    // is this rule's subject, and a FIFO walk must not answer them by accident.
    sut.drop_matching(|op| {
        matches!(
            op,
            Op::ResolveIdentity { .. } | Op::LoadAccountCredential { .. }
        )
    });
    select_eth(&mut sut);
    sut.dispatch(Event::Back);
    assert_eq!(
        sut.view().recipient,
        RECIPIENT,
        "a recipient handed in from outside survives the trip to the picker"
    );
    sut.dispatch(Event::SetRecipient {
        recipient: RECIPIENT.to_owned(),
    });
    assert_eq!(
        sut.view().recipient,
        RECIPIENT,
        "an unlocked prefill is re-settable — the field is only READ-ONLY on screen"
    );
}

#[test]
fn a_locked_request_cannot_become_a_split_or_a_batch() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(locked_params(Some(USDC), Some("1500000"), "1")));
    sut.resolve(loaded(vec![usdc("5")]));

    sut.dispatch(Event::EnterSplitMode);
    let view = sut.view();
    assert!(!view.split_mode, "one request pays one payee");
    assert!(view.recipients.is_empty());

    sut.dispatch(Event::SeedSplitRecipients {
        recipients: vec![
            SendRecipientDraft {
                id: String::new(),
                address: RECIPIENT_B.to_owned(),
                amount: "1".to_owned(),
                name: None,
            },
            SendRecipientDraft {
                id: String::new(),
                address: RECIPIENT.to_owned(),
                amount: "1".to_owned(),
                name: None,
            },
        ],
    });
    let view = sut.view();
    assert!(
        !view.split_mode,
        "an import cannot re-target a locked request"
    );
    assert_eq!(view.recipient, RECIPIENT);
    assert_eq!(view.amount, "1.5", "and the pinned amount survives it");
}

#[test]
fn a_token_off_the_filtered_chain_cannot_join_the_sweep() {
    let mut sut = boot(vec![eth("2"), polygon_usdc("9")]);
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    sut.dispatch(Event::ToggleMultiToken {
        token_id: polygon_usdc("9").id(),
    });
    assert!(
        sut.view().multi_selected_ids.is_empty(),
        "⑪: a batch is one chain — the picker's filter is not the only guard"
    );
    // …and the rows the filter IS showing still toggle both ways.
    sut.dispatch(Event::ToggleMultiToken {
        token_id: eth("2").id(),
    });
    assert_eq!(sut.view().multi_selected_ids, vec![eth("2").id()]);
    sut.dispatch(Event::ToggleMultiToken {
        token_id: eth("2").id(),
    });
    assert!(sut.view().multi_selected_ids.is_empty(), "never stuck on");
}

#[test]
fn identity_and_risk_results_are_dropped_when_the_recipient_moved_on() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    let ops = sut.dispatch(Event::SetRecipient {
        recipient: RECIPIENT.to_owned(),
    });
    assert!(matches!(ops.as_slice(), [Op::ResolveIdentity { .. }]));
    // The recipient changes before the lookup answers.
    let ops = sut.dispatch(Event::SetRecipient {
        recipient: RECIPIENT_B.to_owned(),
    });
    assert!(matches!(ops.as_slice(), [Op::ResolveIdentity { .. }]));
    // The FIRST lookup answers now — stale, dropped.
    let ops = sut.resolve(Res::IdentityResolved {
        identity: Some(vela_core::app::send::SendRecipientIdentity {
            name: Some("Mallory".to_owned()),
            source: None,
        }),
    });
    assert!(ops.is_empty());
    assert_eq!(
        sut.view().recipient_identity,
        None,
        "stale identity dropped"
    );
    // The current one lands.
    sut.resolve(Res::IdentityResolved {
        identity: Some(vela_core::app::send::SendRecipientIdentity {
            name: Some("Bob".to_owned()),
            source: Some("vela".to_owned()),
        }),
    });
    assert_eq!(
        sut.view()
            .recipient_identity
            .as_ref()
            .and_then(|i| i.name.as_deref()),
        Some("Bob")
    );
}

#[test]
fn fee_estimates_that_do_not_parse_are_refused_not_guessed() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    let mut bad = native_fee(1, 1_000);
    bad.total_wei = "not-a-number".to_owned();
    sut.dispatch(Event::FeeUpdated { estimate: bad });
    assert!(sut.view().fee.is_none());
}

// ===========================================================================
// The form's own quote (spec 028 Phase 9, T490)
// ===========================================================================

#[test]
fn a_complete_form_quotes_once_it_sits_still_and_shows_the_fee_before_continue() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    assert!(sut.view().fee.is_none(), "no amount yet, no quote");
    let ops = sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    assert!(
        matches!(
            ops.as_slice(),
            [Op::StartTimer {
                ms: 400,
                tag: SendTimerTag::FormEstimate
            }]
        ),
        "the debounce is armed, not a quote: {ops:?}"
    );
    let ops = sut.resolve(Res::TimerElapsed {
        tag: SendTimerTag::FormEstimate,
    });
    assert!(
        matches!(
            ops.as_slice(),
            [Op::EstimateFee {
                chain_id: 1,
                tx: Some(_),
                batch: None,
                ..
            }]
        ),
        "the exact transfer is quoted: {ops:?}"
    );
    assert!(sut.resolve(fee_ok(native_fee(1, 21_000))).is_empty());
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails, "still the form");
    assert!(
        view.fee.is_some(),
        "the fee row has a figure before Continue"
    );
}

#[test]
fn typing_re_arms_the_debounce_and_a_stale_timer_asks_for_nothing() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    let first = sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    assert!(matches!(
        first.as_slice(),
        [Op::StartTimer {
            tag: SendTimerTag::FormEstimate,
            ..
        }]
    ));
    let second = sut.dispatch(Event::SetAmount {
        amount: "1.5".to_owned(),
    });
    assert!(matches!(
        second.as_slice(),
        [Op::StartTimer {
            tag: SendTimerTag::FormEstimate,
            ..
        }]
    ));
    // Oldest first: the first timer fires late, and is not the armed one.
    let ops = sut.resolve(Res::TimerElapsed {
        tag: SendTimerTag::FormEstimate,
    });
    assert!(ops.is_empty(), "a stale debounce quotes nothing: {ops:?}");
    let ops = sut.resolve(Res::TimerElapsed {
        tag: SendTimerTag::FormEstimate,
    });
    assert!(matches!(ops.as_slice(), [Op::EstimateFee { .. }]));
}

#[test]
fn continue_takes_over_from_a_pending_form_quote_and_a_landed_one_is_not_asked_twice() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    // Continue before the debounce elapsed: the pre-check owns the slot.
    let ops = sut.dispatch(Event::Continue);
    assert!(
        matches!(
            ops.as_slice(),
            [
                Op::EstimateFee { .. },
                Op::ProbeTreasury { .. },
                Op::StartTimer {
                    ms: 15_000,
                    tag: SendTimerTag::EstimateTimeout
                }
            ]
        ),
        "{ops:?}"
    );
    // The debounce fires under the pre-check: it asks for nothing.
    let late = sut.resolve_matching(
        |op| {
            matches!(
                op,
                Op::StartTimer {
                    tag: SendTimerTag::FormEstimate,
                    ..
                }
            )
        },
        Res::TimerElapsed {
            tag: SendTimerTag::FormEstimate,
        },
    );
    assert!(late.is_empty(), "{late:?}");

    // A second form, quoted once: the same payee and token asked again does
    // not re-arm the debounce while the quote in hand is about them.
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    sut.resolve(Res::TimerElapsed {
        tag: SendTimerTag::FormEstimate,
    });
    assert!(sut.resolve(fee_ok(native_fee(1, 21_000))).is_empty());
    let ops = sut.dispatch(Event::SetAmount {
        amount: "1.25".to_owned(),
    });
    assert!(
        ops.is_empty(),
        "the amount alone is not a new question: {ops:?}"
    );
    // A different fee coin is.
    let ops = sut.dispatch(Event::ChooseFeeToken {
        token: Some(USDC.to_owned()),
    });
    assert!(
        matches!(
            ops.as_slice(),
            [Op::StartTimer {
                tag: SendTimerTag::FormEstimate,
                ..
            }]
        ),
        "{ops:?}"
    );
}

/// The out-of-gas sheet has to say WHO can fix it, and the core is what knows:
/// a network Vela ships has an operator who owns that relayer, a network the
/// person added may have nobody who can hold gas on it at all. The shell sends
/// the probe unjudged; the answer is added here (spec 060).
#[test]
fn the_relayer_sheet_says_whether_the_operator_owns_this_network() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    sut.dispatch(Event::Continue);
    drain_form_quote(&mut sut);
    assert!(sut.resolve(fee_ok(native_fee(1, 1_000))).is_empty());
    // The shell reported the probe WITHOUT this verdict (`operator_served`
    // is false in `treasury_status()`); the core supplies it.
    assert!(sut.resolve(low_float()).is_empty());

    let status = sut
        .view()
        .treasury_bootstrap
        .expect("a depleted relayer opens the sheet");
    assert!(
        status.operator_served,
        "chain 1 ships with Vela, so its relayer is the operator's to refill"
    );
}
