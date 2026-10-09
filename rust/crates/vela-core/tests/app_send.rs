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
    max_figure, receipt_outcome_of, recipients_are_valid, split_row_issues, sum_split_base_units,
    Event, ReentryLock, Send, SendAccountRef, SendAddNetworkOutcome, SendAlertKind,
    SendAmountWarning, SendChainInfo, SendDisplayContext, SendEstimateFailure, SendFeeOutcome,
    SendHapticKind, SendHoldReason, SendLockError, SendNameSource, SendOpenParams,
    SendOperation as Op, SendPayee, SendReceiptKind, SendReceiptOutcome, SendReceiptStatus,
    SendRecipientDraft, SendRecipientRisk, SendRowFieldState, SendScan, SendShellResult as Res,
    SendStage, SendSubmitFailure, SendTimerTag, SendToken, SendTokenMeta, SendTreasuryAsset,
    SendTreasuryCoin, SendTreasuryProbe, SendTreasuryStatus, SendTxErrorKey, SendTxRecord,
    SendTxStatus, SendUnitIssue, SendView, BATCH_MAX_RECIPIENTS, TREASURY_WATCH_MS,
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

/// Polygon's own coin — with [`polygon_usdc`], two holdings on one network.
fn polygon_pol(balance: &str) -> SendToken {
    SendToken {
        network: "polygon".to_owned(),
        chain_id: 137,
        symbol: "POL".to_owned(),
        balance: balance.to_owned(),
        decimals: 18,
        token_address: None,
        price_usd: Some(0.5),
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
        // Nor this: the coin and its figures are the core's (issue #422).
        coin: None,
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

fn uncovered() -> Res {
    Res::TreasuryProbed {
        probe: SendTreasuryProbe::Uncovered,
    }
}

fn unknown() -> Res {
    Res::TreasuryProbed {
        probe: SendTreasuryProbe::Unknown,
    }
}

/// The treasury watch's wait elapses — answered by tag, not by age: the
/// pre-check's own 15 s timer is still outstanding, and older.
fn elapse_the_watch(sut: &mut Sut) -> Vec<Op> {
    sut.resolve_matching(
        |op| {
            matches!(
                op,
                Op::StartTimer {
                    tag: SendTimerTag::TreasuryWatch,
                    ..
                }
            )
        },
        Res::TimerElapsed {
            tag: SendTimerTag::TreasuryWatch,
        },
    )
}

/// The probe the watch started, answered by kind.
fn answer_the_watch(sut: &mut Sut, result: Res) -> Vec<Op> {
    sut.resolve_matching(|op| matches!(op, Op::ProbeTreasury { .. }), result)
}

/// The treasury sheet opened, and the only thing it asked for is its watch's
/// first wait (spec 098 §4) — no confirm probes, no submission.
#[track_caller]
fn assert_only_the_watch(ops: &[Op], why: &str) {
    assert!(
        matches!(
            ops,
            [Op::StartTimer {
                ms: TREASURY_WATCH_MS,
                tag: SendTimerTag::TreasuryWatch
            }]
        ),
        "{why}: expected only the treasury watch, got {ops:?}"
    );
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
        maybe_sent: false,
        submit_block: None,
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
    // A plain load routes nowhere — it only reads ahead the fees of the
    // chains held, for the quote a pick will start, answered at once.
    assert!(
        ops.iter().all(|op| matches!(op, Op::PrewarmFees { .. })),
        "plain load routes nowhere: {ops:?}"
    );
    for _ in &ops {
        assert!(sut.resolve(Res::FeesPrewarmed).is_empty());
    }
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
                tx: Some(_),
                batch: None,
                auto_fee_token: true,
                ..
            }]
        ),
        "a picked token warms the transfer Max fills, coin left to the fee machine: {ops:?}"
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
/// form opened on nobody while the fetch was out. Since issue #312 that frame
/// is the picker, with the recipient on it (issue #332).
#[test]
fn a_prefilled_recipient_is_shown_before_the_tokens_arrive() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        prefilled_recipient: Some(RECIPIENT.to_owned()),
        ..SendOpenParams::default()
    }));
    let view = sut.view();
    assert_eq!(
        view.stage,
        SendStage::SelectToken,
        "the asset is chosen next"
    );
    assert_eq!(view.recipient, RECIPIENT, "known before any token is");
    assert!(view.selected_token.is_none());

    // The tokens land: nothing is chosen for the person, the recipient stays.
    sut.resolve(loaded(vec![eth("2"), usdc("5")]));
    let view = sut.view();
    assert_eq!(view.recipient, RECIPIENT);
    assert!(view.selected_token.is_none());
    assert_eq!(view.tokens.len(), 2, "every holding is offered");
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
    assert_eq!(sut.view().stage, SendStage::SelectToken);

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

/// Issue #312: a scanned plain address used to open the form on the
/// balance's most valuable token — XDAI on Gnosis for a code a BNB Chain
/// wallet showed. Which asset to send is the payer's: the recipient lands on
/// the picker with every holding offered and none chosen, and the name lookup
/// and the fee read-ahead start as they do for any picker.
#[test]
fn a_prefilled_recipient_lands_on_the_picker_with_nothing_chosen_for_them() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        prefilled_recipient: Some(RECIPIENT.to_owned()),
        ..SendOpenParams::default()
    }));
    let ops = sut.resolve(loaded(vec![usdc("5"), eth("2"), polygon_usdc("3")]));
    assert!(
        matches!(
            ops.as_slice(),
            [Op::ResolveIdentity { .. }, Op::PrewarmFees { .. }]
        ),
        "the name is looked up for the picker, the fees read ahead: {ops:?}"
    );
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken);
    assert!(view.selected_token.is_none(), "no asset taken for them");
    assert_eq!(view.recipient, RECIPIENT);
    assert_eq!(view.tokens.len(), 3, "every network's holdings are offered");
    assert_eq!(view.request_chain_id, None, "no network was named");
    assert!(!view.can_change_token, "no form yet");
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
    sut.dispatch(open_event(locked_params(
        None,
        Some("1000000000000000000"),
        "999",
    )));
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
    // …and while that load is out the request is RESOLVING, never an empty
    // form with nothing chosen (078 W-04).
    assert_eq!(sut.view().stage, SendStage::LockResolving);
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
// A code that names a network, the asset left to the payer (issue #312)
// ===========================================================================

/// `ethereum:<payee>@137` — a network, no token, no amount.
fn network_only(chain: &str) -> SendOpenParams {
    locked_params(None, None, chain)
}

fn chains_of(view: &SendView) -> Vec<u32> {
    view.tokens.iter().map(|t| t.chain_id).collect()
}

/// One holding on the named network is the obvious choice: the form opens
/// on it, for the payee the code named — and the asset can still be changed,
/// within that network.
#[test]
fn a_code_naming_a_network_where_one_asset_is_held_opens_the_form_on_it() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(network_only("137")));
    assert_eq!(sut.view().request_chain_id, Some(137));
    // ETH on Ethereum is worth the most; the code is for Polygon.
    sut.resolve(loaded(vec![eth("2"), usdc("5"), polygon_usdc("3")]));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails);
    let token = view
        .selected_token
        .as_ref()
        .expect("the one Polygon holding");
    assert_eq!((token.chain_id, token.symbol.as_str()), (137, "USDC"));
    assert_eq!(view.recipient, RECIPIENT);
    assert_eq!(chains_of(&view), vec![137], "only that network is offered");
    assert!(!view.amount_locked, "the code named no amount");
    assert!(view.can_change_token);
}

/// Several holdings there: the picker, narrowed to that network, the payee
/// on it. A holding on another network cannot be picked even by id.
#[test]
fn a_code_naming_a_network_where_several_assets_are_held_offers_only_those() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(network_only("137")));
    let ops = sut.resolve(loaded(vec![
        eth("2"),
        polygon_usdc("3"),
        usdc("5"),
        polygon_pol("10"),
    ]));
    assert!(
        matches!(
            ops.as_slice(),
            [Op::ResolveIdentity { .. }, Op::PrewarmFees { chain_ids, .. }] if chain_ids == &vec![137]
        ),
        "{ops:?}"
    );
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken);
    assert!(view.selected_token.is_none());
    assert!(view.lock_error.is_none());
    assert_eq!(view.recipient, RECIPIENT);
    assert_eq!(chains_of(&view), vec![137, 137]);
    assert_eq!(view.request_chain_id, Some(137));

    // Another network's holding is not on offer, by any door.
    assert!(sut
        .dispatch(Event::SelectToken {
            token_id: eth("2").id(),
        })
        .is_empty());
    assert!(sut.view().selected_token.is_none());

    let ops = sut.dispatch(Event::SelectToken {
        token_id: polygon_pol("10").id(),
    });
    assert!(matches!(ops.as_slice(), [Op::LoadAccountCredential { .. }]));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails);
    assert_eq!(view.selected_token.map(|t| t.chain_id), Some(137));
    assert_eq!(view.recipient, RECIPIENT);
}

/// Nothing held there: the picker says so — the network named, the list
/// empty — and the send never moves to another chain (issue #264's report).
#[test]
fn a_code_naming_a_network_where_nothing_is_held_never_switches_chains() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(network_only("137")));
    sut.resolve(loaded(vec![eth("2"), usdc("5")]));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken);
    assert!(view.tokens.is_empty(), "nothing on Polygon to offer");
    assert!(view.selected_token.is_none(), "no placeholder coin either");
    assert_eq!(view.request_chain_id, Some(137));
    assert_eq!(view.lock_error, None);
    assert_eq!(view.recipient, RECIPIENT);

    // A later round of holdings that still has nothing there changes nothing.
    sut.dispatch(Event::HoldingsUpdated {
        tokens: vec![eth("3"), usdc("6")],
    });
    assert!(sut.view().tokens.is_empty());
    // One that has something there offers it.
    sut.dispatch(Event::HoldingsUpdated {
        tokens: vec![eth("3"), polygon_usdc("1")],
    });
    assert_eq!(chains_of(&sut.view()), vec![137]);
}

/// The same code scanned with Send already open re-opens it on that rule.
#[test]
fn a_network_code_scanned_inside_send_offers_that_network_only() {
    let mut sut = boot(vec![eth("2"), polygon_usdc("3"), polygon_pol("10")]);
    select_eth(&mut sut);
    let ops = sut.dispatch(Event::ScanResolved {
        scan: SendScan::Request {
            recipient: RECIPIENT.to_owned(),
            chain_id: Some(137),
            token_address: None,
            amount_base_units: None,
        },
    });
    assert!(
        matches!(ops.as_slice(), [Op::FetchTokens { .. }]),
        "{ops:?}"
    );
    sut.resolve(loaded(vec![eth("2"), polygon_usdc("3"), polygon_pol("10")]));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken);
    assert_eq!(chains_of(&view), vec![137, 137]);
    assert_eq!(view.recipient, RECIPIENT);
}

/// A network the wallet lacks is still refused with its way out; once added,
/// the payer chooses among their holdings there — and holding none, is told
/// so, rather than handed a zero-balance coin.
#[test]
fn a_named_network_the_wallet_lacks_is_refused_then_offered_once_added() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(network_only("999")));
    sut.resolve(loaded(vec![eth("2")]));
    assert_eq!(
        sut.view().lock_error,
        Some(SendLockError::Network { chain_id: 999 })
    );
    sut.dispatch(Event::AddNetworkTapped { chain_id: 999 });
    sut.resolve(Res::NetworkAdded {
        outcome: SendAddNetworkOutcome::Added,
    });
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
    assert_eq!(view.stage, SendStage::SelectToken);
    assert!(view.tokens.is_empty());
    assert!(view.selected_token.is_none());
    assert_eq!(view.request_chain_id, Some(999));
}

/// A code that names its token or its amount is that request, as before:
/// locked to it, and its token card does not open a picker.
#[test]
fn a_code_naming_its_token_or_amount_stays_that_request() {
    for params in [
        locked_params(Some(USDC), None, "1"),
        locked_params(None, Some("1000000000000000000"), "1"),
    ] {
        let mut sut = Sut::new();
        sut.dispatch(open_event(params));
        sut.resolve(loaded(vec![eth("2"), usdc("5"), polygon_usdc("3")]));
        let view = sut.view();
        assert_eq!(view.request_chain_id, None);
        assert_eq!(view.stage, SendStage::EnterDetails);
        assert_eq!(view.selected_token.as_ref().map(|t| t.chain_id), Some(1));
        assert_eq!(view.tokens.len(), 3, "the list is not narrowed");
        assert!(!view.can_change_token);
        assert!(sut.dispatch(Event::ChangeToken).is_empty());
        assert_eq!(sut.view().stage, SendStage::EnterDetails);
    }
}

// ===========================================================================
// The token card opens the picker (issue #326)
// ===========================================================================

/// The card changes WHAT is sent, never to whom: a typed recipient stays
/// (Back would have cleared it), the figure typed for the old coin goes.
#[test]
fn the_token_card_opens_the_picker_and_keeps_the_recipient() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    assert!(!sut.view().can_change_token, "no form, no card");
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    assert!(sut.view().can_change_token);

    sut.dispatch(Event::ChangeToken);
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken);
    assert!(view.selected_token.is_none());
    assert_eq!(view.amount, "", "a figure in ETH means nothing in USDC");
    assert_eq!(view.recipient, RECIPIENT, "the payee stays");
    assert!(!view.can_change_token);

    sut.dispatch(Event::SelectToken {
        token_id: usdc("5").id(),
    });
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails);
    assert_eq!(view.recipient, RECIPIENT);
    assert_eq!(
        view.selected_token.map(|t| t.symbol),
        Some("USDC".to_owned())
    );
}

/// A scanned recipient too — the case the report was about.
#[test]
fn the_token_card_keeps_a_scanned_recipient() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    sut.dispatch(Event::ScanResolved {
        scan: SendScan::Text {
            data: RECIPIENT.to_owned(),
        },
    });
    sut.drop_matching(|op| matches!(op, Op::ResolveIdentity { .. }));
    select_eth(&mut sut);
    sut.dispatch(Event::ChangeToken);
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken);
    assert_eq!(view.recipient, RECIPIENT);
}

/// Inside a code that named a network, the card's picker is that network's.
#[test]
fn the_token_card_of_a_network_code_stays_on_that_network() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(network_only("137")));
    sut.resolve(loaded(vec![eth("2"), polygon_usdc("3")]));
    sut.drop_matching(|op| matches!(op, Op::ResolveIdentity { .. }));
    assert!(sut.view().can_change_token);
    sut.dispatch(Event::ChangeToken);
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken);
    assert_eq!(chains_of(&view), vec![137]);
    assert_eq!(view.recipient, RECIPIENT);
}

/// The words every shell sends and reads for it.
#[test]
fn the_token_card_wire() {
    let event: Option<Event> = serde_json::from_str(r#"{"type":"change_token"}"#).ok();
    assert!(matches!(event, Some(Event::ChangeToken)));
    let view = serde_json::to_value(Sut::new().view()).unwrap_or_default();
    assert_eq!(view["can_change_token"], serde_json::json!(false));
    assert_eq!(view["request_chain_id"], serde_json::Value::Null);
}

/// Not in a split (its rows are figures in this coin), not in a sweep (no
/// one coin), and not while Continue's pre-check is out.
#[test]
fn the_token_card_is_inert_where_changing_the_coin_would_lose_something() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    assert!(sut.view().split_mode);
    assert!(!sut.view().can_change_token);
    assert!(sut.dispatch(Event::ChangeToken).is_empty());
    assert_eq!(sut.view().stage, SendStage::EnterDetails);

    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    sut.dispatch(Event::Continue);
    assert!(sut.view().estimating_gas);
    assert!(!sut.view().can_change_token);
    assert!(sut.dispatch(Event::ChangeToken).is_empty());
    assert_eq!(sut.view().stage, SendStage::EnterDetails);

    let mut sut = boot(vec![eth("2"), usdc("5")]);
    sut.dispatch(Event::SetMultiNetwork { chain_id: Some(1) });
    for token in [eth("2"), usdc("5")] {
        sut.dispatch(Event::ToggleMultiToken {
            token_id: token.id(),
        });
    }
    sut.dispatch(Event::ConfirmMultiSelection);
    assert!(sut.view().multi_select_mode);
    assert_eq!(sut.view().stage, SendStage::EnterDetails);
    assert!(!sut.view().can_change_token);
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
    // ⑨: to_base_units(result) + reserve == balance, exactly — for the amount
    // every gate and the signed call read…
    let filled = to_base_units(&view.token_amount, 18).expect("max parses");
    assert_eq!(
        filled + reserve,
        to_base_units("1.234567891234567891", 18).expect("balance parses")
    );
    // …while the field shows it on the balance line's ladder (4 places ≥ 1).
    assert_eq!(view.amount, "1.2246");
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

/// 078 M-02, now at the token pick: the warm-up quotes `transfer(account,
/// whole balance)` — the call Max fills — so a Max pressed while it is in
/// flight WAITS for that answer instead of racing it with a second estimate,
/// and keeps it as the form's fee.
#[test]
fn max_while_the_warm_quote_is_in_flight_waits_for_it() {
    let mut sut = boot(vec![usdc("5")]);
    sut.dispatch(Event::SelectToken {
        token_id: usdc("5").id(),
    });
    let ops = sut.resolve(credential(Some(PK)));
    let call = ops
        .iter()
        .find_map(|op| match op {
            Op::EstimateFee { tx: Some(call), .. } => Some(call.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("a warm quote of the transfer: {ops:?}"));
    assert_eq!(call.to.to_lowercase(), USDC.to_lowercase());
    assert_eq!(call.value, "0");
    // transfer(ACCOUNT, 5_000_000) — no payee yet, the account itself.
    assert!(call.data.starts_with("0xa9059cbb"), "{}", call.data);
    assert!(call.data.contains(&ACCOUNT[2..]), "{}", call.data);
    assert!(
        call.data.ends_with(&format!("{:064x}", 5_000_000u128)),
        "{}",
        call.data
    );

    // The warm quote stays outstanding while the payee is typed.
    let ops = sut.dispatch(Event::SetRecipient {
        recipient: RECIPIENT.to_owned(),
    });
    if ops
        .iter()
        .any(|op| matches!(op, Op::ResolveIdentity { .. }))
    {
        sut.resolve_matching(
            |op| matches!(op, Op::ResolveIdentity { .. }),
            Res::IdentityResolved { identity: None },
        );
    }
    let ops = sut.dispatch(Event::TapMax);
    assert!(
        !ops.iter().any(|op| matches!(op, Op::EstimateFee { .. })),
        "no second estimate while the first is on its way: {ops:?}"
    );
    assert_eq!(sut.view().amount, "", "blank until it answers");

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

/// With nothing on its way, Max asks for the transfer it fills — the payee
/// typed so far — and fills from that answer.
#[test]
fn max_with_nothing_in_flight_estimates_the_token_transfer_it_fills() {
    let mut sut = boot(vec![usdc("5")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    let ops = sut.dispatch(Event::TapMax);
    let call = ops
        .iter()
        .find_map(|op| match op {
            Op::EstimateFee { tx: Some(call), .. } => Some(call.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("an estimate of the transfer: {ops:?}"));
    assert!(call.data.contains(&RECIPIENT[2..]), "{}", call.data);
    sut.resolve_matching(
        |op| matches!(op, Op::EstimateFee { .. }),
        Res::FeeEstimated {
            outcome: SendFeeOutcome::Ok {
                estimate: usdc_fee(1, 1_000_000),
            },
        },
    );
    assert_eq!(sut.view().amount, "3.5");
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

    // Picking native back while Max is in force: a USDC quote no longer
    // counts, and the Max asks again in the chain's coin at once — the switch
    // itself, not a second press, is what moved what Max holds back.
    let ops = sut.dispatch(Event::ChooseFeeToken { token: None });
    assert!(
        ops.iter().any(|op| matches!(
            op,
            Op::EstimateFee {
                gas_fee_token: None,
                auto_fee_token: false,
                ..
            }
        )),
        "asks again in the chain's coin: {ops:?}"
    );
    let ops = sut.dispatch(Event::TapMax);
    assert!(
        ops.is_empty(),
        "a press meanwhile waits for that answer: {ops:?}"
    );
    sut.resolve(Res::FeeEstimated {
        outcome: SendFeeOutcome::Ok {
            estimate: native_fee(1, 500_000_000_000_000_000),
        },
    });
    assert_eq!(sut.view().amount, "1.5", "2 less the native fee");
}

// ===========================================================================
// The fee row's coin: one answer for the frames with no estimate
// ===========================================================================

const BSC_USDT: &str = "0x55d398326f99059ff775485246999027b3197955";
/// The same contract as a picker hands it on: checksummed.
const BSC_USDT_CHECKSUMMED: &str = "0x55d398326F99059fF775485246999027B3197955";

fn bnb(balance: &str) -> SendToken {
    SendToken {
        network: "bsc".to_owned(),
        chain_id: 56,
        symbol: "BNB".to_owned(),
        balance: balance.to_owned(),
        decimals: 18,
        token_address: None,
        price_usd: Some(600.0),
        logo_urls: vec![],
        spam: false,
    }
}

fn bsc_usdt(balance: &str) -> SendToken {
    SendToken {
        network: "bsc".to_owned(),
        chain_id: 56,
        symbol: "USDT".to_owned(),
        balance: balance.to_owned(),
        decimals: 18,
        token_address: Some(BSC_USDT.to_owned()),
        price_usd: Some(1.0),
        logo_urls: vec![],
        spam: false,
    }
}

/// Pick `token` and settle what the pick starts — the credential, and a warm
/// quote that FAILS: the form is left with no estimate in hand.
fn select_with_a_failed_quote(sut: &mut Sut, token: &SendToken) {
    let ops = sut.dispatch(Event::SelectToken {
        token_id: token.id(),
    });
    assert_eq!(
        ops,
        vec![Op::LoadAccountCredential {
            account_id: "cred-1".to_owned()
        }]
    );
    settle_warm_quote(sut);
}

#[track_caller]
fn assert_fee_coin(
    view: &SendView,
    symbol: &str,
    contract: Option<&str>,
    chain_id: u32,
    why: &str,
) {
    let coin = view
        .fee_coin
        .as_ref()
        .unwrap_or_else(|| panic!("{why}: no fee coin"));
    assert_eq!(coin.symbol, symbol, "{why}");
    assert_eq!(coin.contract.as_deref(), contract, "{why}");
    assert_eq!(coin.chain_id, chain_id, "{why}");
}

#[test]
fn with_no_chain_known_the_fee_row_names_no_coin() {
    let sut = boot(vec![eth("2"), bnb("1")]);
    assert_eq!(
        sut.view().fee_coin,
        None,
        "nothing picked: no chain to name a coin on"
    );
}

#[test]
fn a_failed_quote_names_the_chains_own_coin_never_an_empty_disc() {
    // Chain 56 is not in the shell's network list here: the registry names it.
    let mut sut = boot(vec![bsc_usdt("50"), bnb("1")]);
    select_with_a_failed_quote(&mut sut, &bnb("1"));
    let view = sut.view();
    assert!(view.fee.is_none(), "the quote failed");
    assert_fee_coin(&view, "BNB", None, 56, "failed quote, nobody chose");

    // Sending the USDT, the fee is still the chain's coin until someone says
    // otherwise — never the coin being sent.
    let mut sut = boot(vec![bsc_usdt("50"), bnb("1")]);
    select_with_a_failed_quote(&mut sut, &bsc_usdt("50"));
    assert_fee_coin(&sut.view(), "BNB", None, 56, "sending USDT, nobody chose");
}

#[test]
fn a_chosen_coin_names_the_row_before_any_estimate_of_it() {
    let mut sut = boot(vec![bsc_usdt("50"), bnb("1")]);
    select_with_a_failed_quote(&mut sut, &bnb("1"));
    sut.dispatch(Event::ChooseFeeToken {
        token: Some(BSC_USDT_CHECKSUMMED.to_owned()),
    });
    let view = sut.view();
    assert!(view.fee.is_none(), "no estimate of the chosen coin yet");
    assert_fee_coin(
        &view,
        "USDT",
        Some(BSC_USDT_CHECKSUMMED),
        56,
        "the pick, named by the holdings (contracts compared case aside)",
    );

    // Back to the chain's coin, chosen on purpose.
    sut.dispatch(Event::ChooseFeeToken { token: None });
    assert_fee_coin(&sut.view(), "BNB", None, 56, "BNB chosen");
}

#[test]
fn the_fee_cards_coin_in_force_names_the_row_when_its_quote_failed() {
    let mut sut = boot(vec![bsc_usdt("50"), bnb("0.0000001")]);
    select_with_a_failed_quote(&mut sut, &bnb("0.0000001"));
    // Nobody chose: the card picked the coin that can pay, and its quote then
    // failed. The sheet shows USDT selected; the row must not say BNB.
    sut.dispatch(Event::FeeTokenChanged {
        fee_token: Some(BSC_USDT.to_owned()),
    });
    assert_fee_coin(&sut.view(), "USDT", Some(BSC_USDT), 56, "the card's pick");

    // The person's pick is newer than the card's word until the card speaks.
    sut.dispatch(Event::ChooseFeeToken { token: None });
    assert_fee_coin(&sut.view(), "BNB", None, 56, "the person's pick");
    sut.dispatch(Event::FeeTokenChanged { fee_token: None });
    assert_fee_coin(&sut.view(), "BNB", None, 56, "the card took it up");

    // A word about the chain the form left never names this chain's coin.
    let mut sut = boot(vec![bsc_usdt("50"), eth("2")]);
    select_with_a_failed_quote(&mut sut, &bsc_usdt("50"));
    sut.dispatch(Event::FeeTokenChanged {
        fee_token: Some(BSC_USDT.to_owned()),
    });
    select_with_a_failed_quote(&mut sut, &eth("2"));
    assert_fee_coin(
        &sut.view(),
        "ETH",
        None,
        1,
        "a BSC contract is not Ethereum's coin",
    );
}

#[test]
fn a_speed_being_measured_keeps_the_coin_that_will_pay() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    // The estimate in hand is in USDC — the coin that pays.
    sut.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 1_000_000),
    });
    assert_fee_coin(&sut.view(), "USDC", Some(USDC), 1, "the estimate's coin");

    // Another speed is picked and measured: the card is busy, and asking
    // afresh it says nothing was chosen. The estimate in hand is the speed
    // just left's, and its coin is still the one that will pay — not ETH.
    sut.dispatch(Event::FeeBusyChanged { busy: true });
    sut.dispatch(Event::FeeTokenChanged { fee_token: None });
    let view = sut.view();
    assert!(view.fee_busy);
    assert_fee_coin(&view, "USDC", Some(USDC), 1, "measuring a new speed");

    // An estimate that does not spell its coin is named by the holdings.
    let mut unnamed = usdc_fee(1, 1_000_000);
    if let FeeAssetView::Erc20 { symbol, .. } = &mut unnamed.fee_asset {
        *symbol = None;
    }
    sut.dispatch(Event::FeeUpdated { estimate: unnamed });
    assert_fee_coin(&sut.view(), "USDC", Some(USDC), 1, "named by the holdings");

    // A native estimate names the chain's coin in the network list's words.
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 1_000),
    });
    assert_fee_coin(&sut.view(), "ETH", None, 1, "the chain's own coin pays");
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

/// Issue #421, through the machine: the figure a field holds is the one its
/// keys spell after the core's amount rule (`l10n::amount_text`, which every
/// shell runs in the field's change handler and writes back), so a "0" then an
/// "8" is "8" on screen, 8 under the fiat line (`token_amount`), and 8 at
/// Continue. "08" used to be stored, echoed as "08 ETH" and read as 8.
#[test]
fn a_zero_then_an_eight_is_eight_on_screen_under_it_and_at_continue() {
    use vela_core::l10n::amount_text::{clean, Entry};
    use vela_core::l10n::NumberPreset;

    /// One key at the end of what the field shows, cleaned as the shells do.
    fn key(sut: &mut Sut, preset: NumberPreset, key: &str) {
        let shown = sut.view().amount;
        let next = format!("{shown}{key}");
        let clean = clean(&next, preset, Entry::Unknown, Some(&shown))
            .unwrap_or_else(|| unreachable!("{next:?} is one key"));
        sut.dispatch(Event::SetAmount { amount: clean });
    }

    for preset in [NumberPreset::CommaDot, NumberPreset::DotComma] {
        let mut sut = boot(vec![eth("20")]);
        select_eth(&mut sut);
        set_recipient(&mut sut, RECIPIENT);
        drain_form_quote(&mut sut);

        key(&mut sut, preset, "0");
        key(&mut sut, preset, "8");
        drain_form_quote(&mut sut);
        let view = sut.view();
        assert_eq!(view.amount, "8", "the field holds 8, not 08");
        assert_eq!(view.token_amount, "8", "the fiat line and the call read 8");
        assert!(view.can_continue, "8 ETH of 20 is a send");

        // A decimal mark after the zero is how 0.8 is written — and it is 0.8
        // everywhere, under either mark the person types.
        sut.dispatch(Event::SetAmount {
            amount: String::new(),
        });
        let mark = if preset == NumberPreset::DotComma {
            ","
        } else {
            "."
        };
        key(&mut sut, preset, "0");
        key(&mut sut, preset, mark);
        key(&mut sut, preset, "8");
        drain_form_quote(&mut sut);
        let view = sut.view();
        assert_eq!(view.amount, "0.8");
        assert_eq!(view.token_amount, "0.8");
        assert!(view.can_continue);

        // A mark on an empty field reads with its zero, and the next key is a
        // decimal digit.
        sut.dispatch(Event::SetAmount {
            amount: String::new(),
        });
        key(&mut sut, preset, mark);
        assert_eq!(sut.view().amount, "0.");
        key(&mut sut, preset, "5");
        drain_form_quote(&mut sut);
        assert_eq!(sut.view().token_amount, "0.5");
    }

    // Typed in money, the zero goes the same way: $8 is 0.004 ETH at $2000,
    // never what "08" might have been read as.
    let mut sut = boot(vec![eth("20")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::ToggleFiatInput);
    assert_eq!(sut.view().amount_fiat_code.as_deref(), Some("USD"));
    key(&mut sut, NumberPreset::CommaDot, "0");
    key(&mut sut, NumberPreset::CommaDot, "8");
    drain_form_quote(&mut sut);
    let view = sut.view();
    assert_eq!(view.amount, "8");
    assert_eq!(view.token_amount, "0.004");
    assert!(view.can_continue);
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
        |op| matches!(op, Op::EstimateFee { .. }),
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
    assert_only_the_watch(&ops, "no confirm probes");
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
    // The GasFeeCard re-quotes: $1 → $2. A transfer's calls do not move with
    // the fee, so nothing is asked again
    // (`a_requote_on_confirm_asks_a_transfer_nothing_again`).
    let probes = sut.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 2_000_000),
    });
    assert!(probes.is_empty(), "{probes:?}");
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

/// What the shell's simulation said, opaque to the core.
const SIM_JSON: &str = r#"{"changes":[]}"#;

/// Answer the confirm's two probes: a first-time recipient, and a simulation.
fn answer_confirm_probes(sut: &mut Sut) {
    sut.resolve_matching(
        |op| matches!(op, Op::ResolveRisk { .. }),
        Res::RiskResolved {
            risk: Some(first_time()),
        },
    );
    sut.resolve_matching(
        |op| matches!(op, Op::SimulateCalls { .. }),
        Res::SimResolved {
            sim_json: Some(SIM_JSON.to_owned()),
        },
    );
}

fn first_time() -> SendRecipientRisk {
    SendRecipientRisk {
        is_contract: Some(false),
        first_time: Some(true),
    }
}

/// The 102 device run: a send to the person's OWN address was tagged
/// 「第一次给这个地址转账」. The shell's history check said "never sent
/// here" (true — nobody pays themselves), but a first transfer to oneself is
/// not the stranger the tag warns about. The core withholds it; the other
/// half of the risk stays.
#[test]
fn a_send_to_ones_own_address_is_never_a_first_transfer() {
    for (to, tagged) in [(RECIPIENT, true), (ACCOUNT, false)] {
        let mut sut = boot(vec![usdc("100")]);
        select_usdc(&mut sut);
        set_recipient(&mut sut, &to.to_uppercase().replacen("0X", "0x", 1));
        sut.dispatch(Event::SetAmount {
            amount: "10".to_owned(),
        });
        sut.dispatch(Event::Continue);
        drain_form_quote(&mut sut);
        sut.resolve(fee_ok(usdc_fee(1, 1_000_000)));
        drop_the_precheck_timer(&mut sut);
        sut.resolve(covered());
        assert_eq!(sut.view().stage, SendStage::Confirm);
        answer_confirm_probes(&mut sut);
        let risk = sut.view().recipient_risk.expect("the risk answered");
        assert_eq!(risk.first_time == Some(true), tagged, "to {to}");
        assert_eq!(risk.is_contract, Some(false), "the rest of it stays");
    }
}

/// The confirm's quote is priced again once a block while the page is up
/// (`fee_policy::requote_interval_ms`), and each new figure reaches this
/// machine. A transfer's calls do not move with the fee, so nothing is asked
/// again: the simulation and the recipient's risk stay on screen as they
/// were, instead of blanking and being fetched once a block for the same
/// answer — and the slide still signs the newest figure.
#[test]
fn a_requote_on_confirm_asks_a_transfer_nothing_again() {
    let mut sut = boot(vec![usdc("100")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "10".to_owned(),
    });
    let ops = sut.dispatch(Event::Continue);
    drain_form_quote(&mut sut);
    assert!(
        matches!(ops.first(), Some(Op::EstimateFee { .. })),
        "{ops:?}"
    );
    sut.resolve(fee_ok(usdc_fee(1, 1_000_000)));
    drop_the_precheck_timer(&mut sut);
    let probes = sut.resolve(covered());
    assert_eq!(sut.view().stage, SendStage::Confirm);
    assert_eq!(probes.len(), 2, "risk and simulation: {probes:?}");
    answer_confirm_probes(&mut sut);

    // Three blocks: a dearer figure, another, and the same one again.
    for amount in [1_100_000, 1_250_000, 1_250_000] {
        let ops = sut.dispatch(Event::FeeUpdated {
            estimate: usdc_fee(1, amount),
        });
        assert!(ops.is_empty(), "asked again at {amount}: {ops:?}");
        let view = sut.view();
        assert_eq!(
            view.sim_json.as_deref(),
            Some(SIM_JSON),
            "the simulation stays"
        );
        assert_eq!(view.recipient_risk, Some(first_time()), "the risk stays");
    }
    let submit = slide_to_submit(&mut sut);
    let Op::SubmitUserOp {
        quoted_fee: Some(quoted),
        ..
    } = &submit
    else {
        panic!("quote expected: {submit:?}");
    };
    assert_eq!(quoted.amount, "1250000", "① the newest figure is signed");
}

/// A sweep holds the fee back from the line in the fee's coin, so a figure
/// priced again moves the calls it signs: the simulation is asked again —
/// about exactly those calls — and the recipient's risk is not. The same
/// figure again moves nothing and asks nothing.
#[test]
fn a_requote_on_a_sweep_s_confirm_simulates_the_moved_calls_again() {
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
    sut.resolve(fee_ok(native_fee(1, 500_000_000_000_000_000)));
    set_recipient(&mut sut, RECIPIENT);
    let ops = sut.dispatch(Event::Continue);
    assert!(
        matches!(ops.first(), Some(Op::EstimateFee { .. })),
        "{ops:?}"
    );
    sut.resolve_matching(
        |op| matches!(op, Op::EstimateFee { .. }),
        fee_ok(native_fee(1, 500_000_000_000_000_000)),
    );
    drop_the_precheck_timer(&mut sut);
    let probes = sut.resolve_matching(|op| matches!(op, Op::ProbeTreasury { .. }), covered());
    assert_eq!(sut.view().stage, SendStage::Confirm);
    assert_eq!(probes.len(), 2, "risk and simulation: {probes:?}");
    answer_confirm_probes(&mut sut);

    // The next block: 0.5 → 0.6 ETH held back, so the ETH line is 1.4.
    let ops = sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 600_000_000_000_000_000),
    });
    let [Op::SimulateCalls {
        calls, chain_id, ..
    }] = ops.as_slice()
    else {
        panic!("only the simulation, asked again: {ops:?}");
    };
    assert_eq!(*chain_id, 1);
    assert_eq!(calls[0].to, RECIPIENT);
    assert_eq!(calls[0].value, "1400000000000000000", "the moved line");
    let view = sut.view();
    assert_eq!(
        view.sim_json, None,
        "the old simulation is not shown for new calls"
    );
    assert_eq!(view.recipient_risk, Some(first_time()), "the risk stays");
    sut.resolve_matching(
        |op| matches!(op, Op::SimulateCalls { .. }),
        Res::SimResolved {
            sim_json: Some(SIM_JSON.to_owned()),
        },
    );
    assert_eq!(sut.view().sim_json.as_deref(), Some(SIM_JSON));

    // The same figure again: the same calls, nothing asked.
    let ops = sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 600_000_000_000_000_000),
    });
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(sut.view().sim_json.as_deref(), Some(SIM_JSON));
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
    assert_only_the_watch(&ops, "⑭: no SubmitUserOp");
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
            sender: Some(ACCOUNT.to_owned()),
            user_op_hash: HASH.to_owned(),
            record_ids: vec![HASH.to_owned()],
            chain_id: 1,
            maybe_sent: false,
            submit_block: None,
            admitted: false,
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
        outcome: SendReceiptOutcome::Failed {
            refusal: None,
            rejected: false,
            not_sent: false,
        },
    });
    assert_eq!(
        sut.view().receipt.expect("receipt").status,
        SendReceiptStatus::Submitted
    );
    // The real one stamps Failed — while tx_status STAYS confirmed
    // (⑤: a submitted payment is never flipped back into an error).
    sut.dispatch(Event::ReceiptUpdate {
        user_op_hash: HASH.to_owned(),
        outcome: SendReceiptOutcome::Failed {
            refusal: None,
            rejected: false,
            not_sent: false,
        },
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

/// The relay topping up its gas before it sends (098 follow-up): pending,
/// its own words — and gone once the relay moves on, unlike the fee hold.
#[test]
fn the_relay_topping_up_its_gas_is_waiting_said_plainly_and_not_sticky() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.resolve(submitted(HASH));
    settle_persistence(&mut sut);
    sut.dispatch(Event::ReceiptUpdate {
        user_op_hash: HASH.to_owned(),
        outcome: SendReceiptOutcome::RelayFunding,
    });
    let receipt = sut.view().receipt.expect("receipt");
    assert_eq!(receipt.status, SendReceiptStatus::Submitted);
    assert_eq!(receipt.hold_reason, Some(SendHoldReason::RelayFunding));

    // Funded and sent: back to the ordinary words.
    sut.dispatch(Event::ReceiptUpdate {
        user_op_hash: HASH.to_owned(),
        outcome: SendReceiptOutcome::Acknowledged,
    });
    let receipt = sut.view().receipt.expect("receipt");
    assert_eq!(receipt.status, SendReceiptStatus::Submitted);
    assert_eq!(receipt.hold_reason, None);
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

/// Spec 102: a venue that cannot be used here is not a failure of the
/// network — the screen says why, with the account's domain, in the person's
/// language; the reason clears with the error.
#[test]
fn a_blocked_venue_is_said_with_its_reason() {
    use vela_core::signing_venue::VenueBlock;
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    let block = VenueBlock::AppCannotReach {
        domain: "sign.example.com".to_owned(),
    };
    let ops = sut.resolve(Res::SubmitFailed {
        failure: SendSubmitFailure::VenueBlocked {
            block: block.clone(),
        },
    });
    assert_eq!(
        ops,
        vec![Op::Haptic {
            kind: SendHapticKind::Error
        }]
    );
    let view = sut.view();
    assert_eq!(view.tx_status, SendTxStatus::Error);
    assert_eq!(view.tx_error, Some(SendTxErrorKey::VenueBlocked));
    assert_eq!(view.tx_venue_block, Some(block));
    assert!(!view.sending);
    sut.dispatch(Event::RetryAfterError);
    let view = sut.view();
    assert_eq!(view.tx_status, SendTxStatus::Idle);
    assert_eq!(view.tx_venue_block, None, "the reason goes with its error");
    // On the wire, as the web reports it.
    let wire = serde_json::to_value(SendSubmitFailure::VenueBlocked {
        block: VenueBlock::NotOnWeb,
    })
    .unwrap_or_default();
    assert_eq!(
        wire,
        serde_json::json!({"type": "venue_blocked", "block": {"type": "not_on_web"}})
    );
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
    assert_only_the_watch(&ops, "the honest ask, and nothing else");
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
    sut.dispatch(Event::OpenScanner { target: None });
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

const RECIPIENT_C: &str = "0xdddddddddddddddddddddddddddddddddddddddd";

fn split_row(id: &str, address: &str, amount: &str, name: Option<&str>) -> SendRecipientDraft {
    SendRecipientDraft {
        id: id.to_owned(),
        address: address.to_owned(),
        amount: amount.to_owned(),
        name: name.map(str::to_owned),
    }
}

/// A split's "from contacts" opens the book for the split, not for a row:
/// the pick fills the first row with no address yet — never the single
/// form's recipient, which a split hides and never pays (Android's 从通讯录
/// did exactly that; the web and iOS each added a row of their own to aim
/// it).
#[test]
fn a_targetless_pick_in_a_split_fills_the_first_row_with_no_address() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::SetRecipient {
        recipient: RECIPIENT.to_owned(),
    });
    sut.drop_matching(|op| matches!(op, Op::ResolveIdentity { .. }));
    sut.dispatch(Event::EnterSplitMode);
    let before = sut.view().recipients;
    assert_eq!(
        before.len(),
        2,
        "the split opens as [the recipient, a blank]"
    );

    sut.dispatch(Event::OpenContactPicker { target: None });
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT_B.to_owned(),
    });
    let view = sut.view();
    assert!(!view.show_contact_picker, "the pick closes the picker");
    assert!(view.split_mode);
    assert_eq!(
        view.recipients.len(),
        2,
        "no row is added while one is free"
    );
    assert_eq!(view.recipients[0], before[0], "the first row is untouched");
    assert_eq!(view.recipients[1].id, before[1].id);
    assert_eq!(view.recipients[1].address, RECIPIENT_B);
    assert_eq!(
        view.recipient, RECIPIENT,
        "the hidden single recipient is not where a split's pick goes"
    );

    // A row with a figure but no address is the free one: it is waiting
    // for its person, and it keeps its figure.
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            split_row("rcpt_1", RECIPIENT, "0.5", None),
            split_row("rcpt_2", "  ", "0.25", None),
            split_row("rcpt_3", "", "", None),
        ],
    });
    sut.dispatch(Event::OpenContactPicker { target: None });
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT_C.to_owned(),
    });
    let rows = sut.view().recipients;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].address, RECIPIENT_C);
    assert_eq!(rows[1].amount, "0.25", "its figure stays");
    assert_eq!(rows[2].address, "", "only the first free row is taken");
}

/// With every row taken, a targetless pick is a new row at the end — fresh
/// id, no figure yet — and at the split's cap, nothing (invariant ⑩).
#[test]
fn a_targetless_pick_in_a_full_split_adds_a_row_up_to_the_cap() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            split_row("rcpt_1", RECIPIENT, "0.5", None),
            split_row("rcpt_2", RECIPIENT_B, "", Some("Bea")),
        ],
    });
    sut.dispatch(Event::OpenContactPicker { target: None });
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT_C.to_owned(),
    });
    let view = sut.view();
    assert!(!view.show_contact_picker);
    assert_eq!(view.recipients.len(), 3);
    assert_eq!(
        view.recipients[0],
        split_row("rcpt_1", RECIPIENT, "0.5", None)
    );
    assert_eq!(
        view.recipients[1],
        split_row("rcpt_2", RECIPIENT_B, "", Some("Bea"))
    );
    let added = &view.recipients[2];
    assert_eq!(added.address, RECIPIENT_C);
    assert_eq!(added.amount, "");
    assert_eq!(added.name, None);
    assert!(added.id.starts_with("rcpt_"), "{}", added.id);
    assert!(
        view.recipients[..2].iter().all(|row| row.id != added.id),
        "a new row is a new id"
    );

    let full: Vec<SendRecipientDraft> = (0..BATCH_MAX_RECIPIENTS)
        .map(|i| split_row(&format!("row_{i}"), &format!("0x{:040x}", i + 1), "1", None))
        .collect();
    sut.dispatch(Event::RecipientsChanged {
        recipients: full.clone(),
    });
    sut.dispatch(Event::OpenContactPicker { target: None });
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT_C.to_owned(),
    });
    let view = sut.view();
    assert!(!view.show_contact_picker);
    assert_eq!(view.recipients, full, "⑩: never a sixty-first row");
}

/// A pick for a row that has gone since the picker opened is not lost: it
/// takes the row a targetless pick would. And a pick that changes a row's
/// address takes its name with it — the name was the old person's.
#[test]
fn a_split_pick_never_lands_nowhere_and_never_keeps_a_strangers_name() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            split_row("rcpt_1", RECIPIENT, "0.5", Some("Ann")),
            split_row("rcpt_2", "", "", None),
            split_row("rcpt_3", RECIPIENT_C, "", None),
        ],
    });
    sut.dispatch(Event::OpenContactPicker {
        target: Some("rcpt_9".to_owned()),
    });
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT_B.to_owned(),
    });
    let rows = sut.view().recipients;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].address, RECIPIENT_B, "the free row, not nowhere");

    // The same person picked again for their own row keeps their name.
    sut.dispatch(Event::OpenContactPicker {
        target: Some("rcpt_1".to_owned()),
    });
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT.to_owned(),
    });
    assert_eq!(sut.view().recipients[0].name.as_deref(), Some("Ann"));

    // The same address in another case (the book stores it checksummed) is
    // the same person, and keeps the name too.
    sut.dispatch(Event::OpenContactPicker {
        target: Some("rcpt_1".to_owned()),
    });
    sut.dispatch(Event::PickedAddress {
        address: "0xBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB".to_owned(),
    });
    assert_eq!(
        sut.view().recipients[0].name.as_deref(),
        Some("Ann"),
        "the same address in another case is the same person"
    );

    // Somebody else picked for it does not wear it.
    sut.dispatch(Event::OpenContactPicker {
        target: Some("rcpt_1".to_owned()),
    });
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT_C.to_owned(),
    });
    let rows = sut.view().recipients;
    assert_eq!(rows[0].address, RECIPIENT_C);
    assert_eq!(rows[0].name, None);
    assert_eq!(
        rows[0].amount, "0.5",
        "the figure is the row's, not the name's"
    );
}

/// The picker's scan row is a pick (issue #270), and a split's targetless
/// picker scans into the split: only the address, into the first free row —
/// a full request does not re-lock the flow over the rows (invariant ⑬),
/// and nothing goes to the hidden single recipient.
#[test]
fn a_scan_from_a_splits_targetless_picker_lands_in_the_split() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            split_row("rcpt_1", RECIPIENT, "0.5", None),
            split_row("rcpt_2", "", "0.25", None),
        ],
    });
    sut.dispatch(Event::OpenContactPicker { target: None });
    sut.dispatch(Event::OpenScanner { target: None });
    let ops = sut.dispatch(Event::ScanResolved {
        scan: SendScan::Request {
            recipient: RECIPIENT_B.to_owned(),
            chain_id: Some(1),
            token_address: Some(USDC.to_owned()),
            amount_base_units: Some("123".to_owned()),
        },
    });
    assert!(ops.is_empty(), "no re-lock: {ops:?}");
    let view = sut.view();
    assert!(view.split_mode);
    assert!(!view.locked);
    assert!(!view.show_scanner);
    assert!(!view.show_contact_picker);
    assert_eq!(
        view.recipients[0],
        split_row("rcpt_1", RECIPIENT, "0.5", None)
    );
    assert_eq!(view.recipients[1].address, RECIPIENT_B);
    assert_eq!(view.recipients[1].amount, "0.25");
    assert_ne!(view.recipient, RECIPIENT_B);

    // A plain address with every row taken: a new row.
    sut.dispatch(Event::OpenContactPicker { target: None });
    sut.dispatch(Event::OpenScanner { target: None });
    sut.dispatch(Event::ScanResolved {
        scan: SendScan::Text {
            data: RECIPIENT_C.to_owned(),
        },
    });
    let rows = sut.view().recipients;
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[2].address, RECIPIENT_C);
}

/// Issue #471: every recipient row has its own scan icon. A split row's
/// names its row, and the code lands there — whatever row is free.
#[test]
fn a_split_rows_own_scan_fills_that_row() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            split_row("rcpt_1", "", "0.5", None),
            split_row("rcpt_2", RECIPIENT, "0.25", Some("Bea")),
        ],
    });
    sut.dispatch(Event::OpenScanner {
        target: Some("rcpt_2".to_owned()),
    });
    assert_eq!(sut.view().picker_target.as_deref(), Some("rcpt_2"));
    assert!(!sut.view().show_contact_picker, "no picker on the way");
    sut.dispatch(Event::ScanResolved {
        scan: SendScan::Text {
            data: RECIPIENT_B.to_owned(),
        },
    });
    let view = sut.view();
    assert!(!view.show_scanner);
    assert_eq!(view.recipients[0], split_row("rcpt_1", "", "0.5", None));
    assert_eq!(
        view.recipients[1],
        split_row("rcpt_2", RECIPIENT_B, "0.25", None),
        "the row it was for; Bea's name went with her address"
    );
    assert_eq!(view.picker_target, None, "spent by the scan");
}

/// Issue #471's latent bug: a picker closed without a pick left its target
/// behind, and the next targetless scan overwrote that row — a row the
/// person was no longer pointing at, holding somebody else.
#[test]
fn a_stale_picker_target_does_not_steer_a_later_scan() {
    let rows = || {
        vec![
            split_row("rcpt_1", "", "0.5", None),
            split_row("rcpt_2", RECIPIENT, "0.25", None),
        ]
    };
    let scan_c = || Event::ScanResolved {
        scan: SendScan::Text {
            data: RECIPIENT_C.to_owned(),
        },
    };
    // Left by a picker closed without a pick …
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged { recipients: rows() });
    sut.dispatch(Event::OpenContactPicker {
        target: Some("rcpt_2".to_owned()),
    });
    sut.dispatch(Event::CloseContactPicker);
    assert_eq!(sut.view().picker_target, None);
    sut.dispatch(Event::OpenScanner { target: None });
    sut.dispatch(scan_c());
    let view = sut.view();
    assert_eq!(view.recipients[0].address, RECIPIENT_C, "the free row");
    assert_eq!(view.recipients[1].address, RECIPIENT, "untouched");

    // … by a row's scan closed without a code …
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged { recipients: rows() });
    sut.dispatch(Event::OpenScanner {
        target: Some("rcpt_2".to_owned()),
    });
    sut.dispatch(Event::CloseScanner);
    assert_eq!(sut.view().picker_target, None);
    sut.dispatch(Event::OpenScanner { target: None });
    sut.dispatch(scan_c());
    let view = sut.view();
    assert_eq!(view.recipients[0].address, RECIPIENT_C);
    assert_eq!(view.recipients[1].address, RECIPIENT);

    // … and by a pick that already used it.
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged { recipients: rows() });
    sut.dispatch(Event::OpenContactPicker {
        target: Some("rcpt_2".to_owned()),
    });
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT_B.to_owned(),
    });
    assert_eq!(sut.view().picker_target, None, "spent by the pick");
    sut.dispatch(Event::OpenScanner { target: None });
    sut.dispatch(scan_c());
    let view = sut.view();
    assert_eq!(view.recipients[0].address, RECIPIENT_C);
    assert_eq!(view.recipients[1].address, RECIPIENT_B);
}

/// A shell from before #471 sends `{"type":"open_scanner"}` — still the
/// targetless scan — and a row's icon adds its `target`.
#[test]
fn the_scanner_event_reads_with_and_without_a_target() {
    let bare: Event = serde_json::from_str(r#"{"type":"open_scanner"}"#).unwrap();
    assert!(matches!(bare, Event::OpenScanner { target: None }));
    let aimed: Event =
        serde_json::from_str(r#"{"type":"open_scanner","target":"rcpt_2"}"#).unwrap();
    assert!(matches!(aimed, Event::OpenScanner { target: Some(t) } if t == "rcpt_2"));
}

#[test]
fn an_untargeted_full_request_relocks_the_whole_flow() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    sut.dispatch(Event::OpenScanner { target: None });
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
    sut.dispatch(Event::OpenScanner { target: None });
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
    sut.dispatch(Event::OpenScanner { target: None });
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
    sut.dispatch(Event::OpenScanner { target: None });
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

/// Issue #332 (Android): Home → Scan opens Send on the asset picker with the
/// scanner over it, and the code arrives AFTER the open. The address it read
/// is the recipient from that moment — on the picker, where the person
/// chooses what to send — and it is still there after they pick, go back,
/// and pick again. Back used to throw it away, because only an address named
/// by the open params counted as handed in.
#[test]
fn a_code_scanned_onto_the_picker_is_the_recipient_and_survives_back() {
    for scan in [
        SendScan::Text {
            data: RECIPIENT.to_owned(),
        },
        SendScan::Request {
            recipient: RECIPIENT.to_owned(),
            chain_id: None,
            token_address: None,
            amount_base_units: None,
        },
    ] {
        let mut sut = boot(vec![eth("2"), usdc("5")]);
        sut.dispatch(Event::OpenScanner { target: None });
        let ops = sut.dispatch(Event::ScanResolved { scan });
        assert!(
            matches!(ops.as_slice(), [Op::ResolveIdentity { .. }]),
            "{ops:?}"
        );
        assert!(sut
            .resolve(Res::IdentityResolved { identity: None })
            .is_empty());
        let view = sut.view();
        assert!(!view.show_scanner, "the scan closes the scanner");
        assert_eq!(
            view.stage,
            SendStage::SelectToken,
            "the asset is chosen next"
        );
        assert_eq!(
            view.recipient, RECIPIENT,
            "with the scanned recipient in view"
        );
        assert!(view.selected_token.is_none(), "nothing is chosen for them");

        select_eth(&mut sut);
        assert_eq!(sut.view().recipient, RECIPIENT, "the form opens on them");

        sut.dispatch(Event::Back);
        let view = sut.view();
        assert_eq!(view.stage, SendStage::SelectToken);
        assert_eq!(view.recipient, RECIPIENT, "the scan survives Back");
        assert!(view.selected_token.is_none());
        assert_eq!(view.amount, "");
    }
}

/// The same for an address picked from the book on the form: it came from
/// outside the field, so changing the asset does not cost it.
#[test]
fn a_recipient_picked_from_the_book_survives_back() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::OpenContactPicker { target: None });
    sut.dispatch(Event::PickedAddress {
        address: RECIPIENT.to_owned(),
    });
    sut.drop_matching(|op| matches!(op, Op::ResolveIdentity { .. }));
    sut.dispatch(Event::Back);
    assert_eq!(sut.view().recipient, RECIPIENT);
}

/// Typed over, a scanned recipient is the person's own again — and a typed
/// recipient still goes on Back, as it always has. An echo of the same
/// address is not typing.
#[test]
fn a_scanned_recipient_typed_over_goes_on_back_like_any_typing() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::ScanResolved {
        scan: SendScan::Text {
            data: RECIPIENT.to_owned(),
        },
    });
    sut.drop_matching(|op| matches!(op, Op::ResolveIdentity { .. }));
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::Back);
    assert_eq!(sut.view().recipient, RECIPIENT, "an echo is not typing");

    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT_B);
    sut.dispatch(Event::Back);
    assert_eq!(
        sut.view().recipient,
        "",
        "typed over, it is the person's to redo"
    );
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
            auto_fee_token,
        } => {
            assert_eq!(*chain_id, 1);
            assert_eq!(account, ACCOUNT);
            assert_eq!(gas_fee_token.as_deref(), Some(USDC));
            assert!(!auto_fee_token, "a picked coin is priced as picked");
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
/// A scan skipped the picker and took the balance's top token — the person
/// never chose it — so the only way to a different asset was Back, and Back
/// threw the scanned address away. They scanned again, landed on the same
/// token, and read it as "the asset cannot be changed". The scan now lands on
/// the picker (issue #312), and the address survives every trip back to it.
#[test]
fn a_scanned_recipient_can_change_its_asset_without_being_scanned_again() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        prefilled_recipient: Some(RECIPIENT.to_owned()),
        ..SendOpenParams::default()
    }));
    sut.resolve(loaded(vec![eth("2"), usdc("5")]));
    sut.drop_matching(|op| matches!(op, Op::ResolveIdentity { .. } | Op::PrewarmFees { .. }));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::SelectToken, "the person chooses");
    assert_eq!(view.recipient, RECIPIENT);

    select_eth(&mut sut);
    assert_eq!(sut.view().stage, SendStage::EnterDetails);
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
    // The prefill resolved an identity and read the fees ahead; neither is
    // this rule's subject, and a FIFO walk must not answer them by accident.
    sut.drop_matching(|op| matches!(op, Op::ResolveIdentity { .. } | Op::PrewarmFees { .. }));
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
    assert_only_the_watch(&sut.resolve(low_float()), "the sheet opens");

    let status = sut
        .view()
        .treasury_bootstrap
        .expect("a depleted relayer opens the sheet");
    assert!(
        status.operator_served,
        "chain 1 ships with Vela, so its relayer is the operator's to refill"
    );
}

// ===========================================================================
// Max as a mode, the fee machine's coin, the asset list's holdings
// ===========================================================================

/// The figure Max writes is the balance line's: 6 places under 1, 4 under
/// 1000, 2 above, half up — never the 18-digit remainder of a fee to the wei.
#[test]
fn a_max_reads_on_the_balance_lines_ladder() {
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
        assert_eq!(max_figure(exact), shown, "{exact}");
    }
}

/// Max is a mode, not a figure written once: when the quote it reserved
/// against is replaced by the fee machine's pick in another coin, the reserve
/// goes and the whole balance is sendable.
#[test]
fn a_max_follows_the_fee_machines_pick() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 500_000_000_000_000_000),
    });
    sut.dispatch(Event::TapMax);
    assert_eq!(sut.view().amount, "1.5");
    // Nobody chose a coin; the requote comes back in USDC.
    sut.dispatch(Event::FeeUpdated {
        estimate: usdc_fee(1, 1_000_000),
    });
    let view = sut.view();
    assert_eq!(view.amount, "2", "USDC pays, so all the ETH goes");
    assert_eq!(view.token_amount, "2");
    assert_eq!(
        view.gas_fee_token.as_deref(),
        Some(USDC),
        "the view names the coin the quote is in"
    );
}

/// The fee coin the submit signs in is the one the quote named — with the
/// choice left to the fee machine, `gas_fee_token` alone would say "none" and
/// a native leg would carry a USDC amount.
#[test]
fn the_submit_pays_in_the_coin_the_quote_named() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    to_confirm_native(&mut sut, "1", usdc_fee(1, 1_000_000));
    let submit = slide_to_submit(&mut sut);
    let Op::SubmitUserOp {
        gas_fee_token,
        quoted_fee: Some(quoted),
        ..
    } = &submit
    else {
        panic!("in-band submit expected: {submit:?}");
    };
    assert_eq!(gas_fee_token.as_deref(), Some(USDC));
    assert_eq!(quoted.amount, "1000000");
}

#[test]
fn a_typed_figure_ends_the_max() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 10_000_000_000_000_001),
    });
    sut.dispatch(Event::TapMax);
    assert_eq!(sut.view().token_amount, "1.989999999999999999");
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    let view = sut.view();
    assert_eq!(view.amount, "1");
    assert_eq!(
        view.token_amount, "1",
        "what was typed, not the Max behind it"
    );
}

#[test]
fn the_swap_keeps_a_max_exact() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 10_000_000_000_000_001),
    });
    sut.dispatch(Event::TapMax);
    sut.dispatch(Event::ToggleFiatInput);
    let view = sut.view();
    assert_eq!(view.amount_fiat_code.as_deref(), Some("USD"));
    assert_eq!(view.amount, "3980.00");
    assert_eq!(view.token_amount, "1.989999999999999999", "still the Max");
    sut.dispatch(Event::ToggleFiatInput);
    let view = sut.view();
    assert_eq!(view.amount, "1.99");
    assert_eq!(view.token_amount, "1.989999999999999999");
}

/// The asset list refreshed while the form is open: the balance beside the
/// token, and a Max that depends on it, follow.
#[test]
fn the_asset_lists_holdings_move_the_form() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::FeeUpdated {
        estimate: native_fee(1, 500_000_000_000_000_000),
    });
    sut.dispatch(Event::TapMax);
    assert_eq!(sut.view().amount, "1.5");
    sut.dispatch(Event::HoldingsUpdated {
        tokens: vec![eth("3")],
    });
    let view = sut.view();
    assert_eq!(
        view.selected_token.as_ref().map(|t| t.balance.as_str()),
        Some("3")
    );
    assert_eq!(view.amount, "2.5");
    assert_eq!(view.tokens.len(), 1);
}

/// A token the update no longer lists keeps its row — a chain that did not
/// answer this round is not an emptied balance.
#[test]
fn a_token_missing_from_an_update_keeps_its_balance() {
    let mut sut = boot(vec![eth("2"), usdc("5")]);
    select_eth(&mut sut);
    sut.dispatch(Event::HoldingsUpdated {
        tokens: vec![usdc("6")],
    });
    let view = sut.view();
    assert_eq!(
        view.selected_token.as_ref().map(|t| t.balance.as_str()),
        Some("2")
    );
}

/// The first load owns the list — its answer is what a hand-off is matched
/// against — so an update that races it is left to it.
#[test]
fn an_update_during_the_first_load_waits_for_it() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams::default()));
    let ops = sut.dispatch(Event::HoldingsUpdated {
        tokens: vec![eth("9")],
    });
    assert!(ops.is_empty(), "{ops:?}");
    assert!(sut.view().tokens.is_empty());
    sut.resolve(loaded(vec![eth("2")]));
    assert_eq!(sut.view().tokens[0].balance, "2");
}

/// On the confirm page the list may move, the confirmed token may not: a Max
/// refilled there would change the signed amount after it was read.
#[test]
fn a_confirm_page_is_not_moved_by_the_asset_list() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 42_000_000_000_000));
    sut.dispatch(Event::HoldingsUpdated {
        tokens: vec![eth("3")],
    });
    let view = sut.view();
    assert_eq!(
        view.selected_token.as_ref().map(|t| t.balance.as_str()),
        Some("2")
    );
    assert_eq!(view.tokens[0].balance, "3", "the picker's list follows");
    assert_eq!(view.token_amount, "1");
}

/// The picker reads ahead the fees of the chains the person holds value on —
/// highest first, spam never, at most four — so the quote a pick starts finds
/// its relay and chain reads already done.
#[test]
fn the_picker_reads_ahead_the_fees_of_the_chains_held() {
    let on = |chain_id: u32, usd: f64, spam: bool| SendToken {
        network: format!("net{chain_id}"),
        chain_id,
        symbol: format!("T{chain_id}"),
        balance: "1".to_owned(),
        decimals: 18,
        token_address: None,
        price_usd: Some(usd),
        logo_urls: vec![],
        spam,
    };
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams::default()));
    let ops = sut.resolve(loaded(vec![
        on(10, 5.0, false),
        on(56, 9_999.0, true),
        on(1, 100.0, false),
        on(137, 50.0, false),
        on(8453, 20.0, false),
        on(42161, 1.0, false),
    ]));
    assert_eq!(
        ops,
        vec![Op::PrewarmFees {
            account: ACCOUNT.to_owned(),
            chain_ids: vec![1, 137, 8453, 10],
        }]
    );
}

/// A hand-off that lands on the form warms its own chain's quote; there is no
/// picker to read ahead for.
#[test]
fn a_hand_off_to_the_form_reads_nothing_ahead() {
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        preselected_symbol: Some("ETH".to_owned()),
        preselected_network: Some("ethereum".to_owned()),
        ..SendOpenParams::default()
    }));
    let ops = sut.resolve(loaded(vec![eth("2")]));
    assert!(
        !ops.iter().any(|op| matches!(op, Op::PrewarmFees { .. })),
        "{ops:?}"
    );
}

/// A Max pressed while the warm-up's credential read is out waits for the
/// quote that read would start. When the read finds no key, no warm quote
/// comes — the Max asks for its own instead of waiting forever on a blank field.
#[test]
fn a_max_waiting_on_a_keyless_warm_up_asks_for_its_own_quote() {
    let mut sut = boot(vec![eth("2")]);
    sut.dispatch(Event::SelectToken {
        token_id: eth("2").id(),
    });
    let ops = sut.dispatch(Event::TapMax);
    assert!(ops.is_empty(), "waits on the warm-up: {ops:?}");
    let ops = sut.resolve(credential(None));
    assert!(
        ops.iter().any(|op| matches!(op, Op::EstimateFee { .. })),
        "Max's own estimate: {ops:?}"
    );
    sut.resolve(Res::FeeEstimated {
        outcome: SendFeeOutcome::Ok {
            estimate: native_fee(1, 500_000_000_000_000_000),
        },
    });
    assert_eq!(sut.view().amount, "1.5");
}

/// The list Send reads spells a chain's coin the wallet's way too, and a
/// hand-off that still says the chain document's "XDAI" finds it.
#[test]
fn a_chains_own_coin_is_one_token_whichever_way_it_was_spelled() {
    let xdai = SendToken {
        network: "gnosis".to_owned(),
        chain_id: 100,
        symbol: "XDAI".to_owned(),
        balance: "0.5".to_owned(),
        decimals: 18,
        token_address: None,
        price_usd: Some(1.0),
        logo_urls: vec![],
        spam: false,
    };
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams {
        preselected_symbol: Some("XDAI".to_owned()),
        preselected_network: Some("gnosis".to_owned()),
        ..SendOpenParams::default()
    }));
    sut.resolve(loaded(vec![xdai]));
    let view = sut.view();
    assert_eq!(view.tokens[0].symbol, "xDAI");
    assert_eq!(
        view.selected_token.as_ref().map(|t| t.symbol.as_str()),
        Some("xDAI"),
        "the hand-off landed on the form"
    );
}

// ===========================================================================
// Spec 082 T026 (RA4, RA10) — a Send whose submit reply was lost
// ===========================================================================

const SUBMIT_BLOCK: u64 = 48_479_100;

fn submitted_maybe(hash: &str) -> Res {
    Res::Submitted {
        user_op_hash: hash.to_owned(),
        now_ms: 1_754_000_000_500.0,
        maybe_sent: true,
        submit_block: Some(SUBMIT_BLOCK),
    }
}

fn track_entry(
    status: vela_core::app::tx_tracker::TrackStatus,
    outcome: vela_core::app::tx_tracker::TrackOutcome,
    tx_hash: Option<&str>,
) -> vela_core::app::tx_tracker::TrackEntryView {
    vela_core::app::tx_tracker::TrackEntryView {
        refusal: None,
        refusal_key: None,
        sender: None,
        user_op_hash: HASH.to_owned(),
        chain_id: 1,
        record_ids: vec![HASH.to_owned()],
        status,
        tx_hash: tx_hash.map(str::to_owned),
        polling: true,
        submitted_at_ms: Some(1_754_000_000_500.0),
        outcome,
        relay_tx_hash: None,
        relay_sent_at_ms: None,
        stalled: false,
    }
}

/// A lost reply reaches the receipt as MaybeSent — recorded and tracked with
/// the flag and the head, and WITHOUT the success haptic (it has not earned
/// "sent").
#[test]
fn a_maybe_sent_send_reads_maybe_sent_and_plays_no_success_haptic() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    let ops = sut.resolve(submitted_maybe(HASH));
    assert!(
        !ops.iter().any(|op| matches!(op, Op::Haptic { .. })),
        "no haptic: {ops:?}"
    );
    let records = ops
        .iter()
        .find_map(|op| match op {
            Op::PersistTxRecords { records } => Some(records.clone()),
            _ => None,
        })
        .expect("the records are still written");
    assert!(records.iter().all(|r| r.maybe_sent));
    assert!(records.iter().all(|r| r.submit_block == Some(SUBMIT_BLOCK)));

    let view = sut.view();
    assert_eq!(view.stage, SendStage::Receipt, "never an error screen");
    assert_eq!(view.tx_error, None);
    assert_eq!(
        view.receipt.expect("receipt").status,
        SendReceiptStatus::MaybeSent
    );

    // The tracker learns it as such, after the records landed.
    assert!(sut.resolve(Res::TokenCacheCleared).is_empty());
    let ops = sut.resolve(Res::RecordsPersisted);
    assert_eq!(
        ops,
        vec![Op::TrackSubmitted {
            sender: Some(ACCOUNT.to_owned()),
            user_op_hash: HASH.to_owned(),
            record_ids: vec![HASH.to_owned()],
            chain_id: 1,
            maybe_sent: true,
            submit_block: Some(SUBMIT_BLOCK),
            admitted: false,
        }]
    );
}

/// The tracker's verdicts on a may-have-been-sent Send, through the one
/// mapping: acknowledged → the ordinary words; confirmed → Confirmed; never
/// sent → NotSent, never the fee-rejected words.
#[test]
fn the_tracker_ends_a_maybe_sent_send() {
    use vela_core::app::tx_tracker::{TrackOutcome, TrackStatus};
    let open = || {
        let mut sut = boot(vec![eth("2")]);
        to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
        slide_to_submit(&mut sut);
        sut.resolve(submitted_maybe(HASH));
        sut
    };
    let update = |sut: &mut Sut, entry| {
        if let Some(outcome) = receipt_outcome_of(&entry) {
            sut.dispatch(Event::ReceiptUpdate {
                user_op_hash: HASH.to_owned(),
                outcome,
            });
        }
    };

    // Still in doubt: nothing to say.
    let mut sut = open();
    update(
        &mut sut,
        track_entry(TrackStatus::Pending, TrackOutcome::MaybeSent, None),
    );
    assert_eq!(
        sut.view().receipt.expect("receipt").status,
        SendReceiptStatus::MaybeSent
    );
    // The relay has it: back to "submitted".
    update(
        &mut sut,
        track_entry(TrackStatus::Pending, TrackOutcome::Landing, None),
    );
    assert_eq!(
        sut.view().receipt.expect("receipt").status,
        SendReceiptStatus::Submitted
    );

    // Confirmed after MaybeSent.
    let mut sut = open();
    update(
        &mut sut,
        track_entry(TrackStatus::Confirmed, TrackOutcome::Final, Some("0xtx")),
    );
    let view = sut.view();
    assert_eq!(view.tx_hash.as_deref(), Some("0xtx"));
    assert_eq!(
        view.receipt.expect("receipt").status,
        SendReceiptStatus::Confirmed
    );

    // Never sent.
    let mut sut = open();
    update(
        &mut sut,
        track_entry(TrackStatus::NotSent, TrackOutcome::Final, None),
    );
    let view = sut.view();
    let receipt = view.receipt.expect("receipt");
    assert_eq!(receipt.status, SendReceiptStatus::NotSent);
    assert_eq!(receipt.hold_reason, None, "never the fee-rejected words");
    assert_eq!(
        view.tx_status,
        SendTxStatus::Confirmed,
        "never an error screen"
    );
}

/// Review of T026 (RA10): a fee hold is the relay's own word that it holds
/// the op (the hold stage comes only from its status), so a may-have-been-sent
/// Send that the relay parks for fees goes back to the ordinary words with the
/// hold line — not "it may have been sent" under a hold the relay announced.
#[test]
fn a_fee_hold_is_the_relay_acknowledging_a_maybe_sent_send() {
    use vela_core::app::tx_tracker::{TrackOutcome, TrackStatus};
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.resolve(submitted_maybe(HASH));
    let entry = track_entry(TrackStatus::FeeHeld, TrackOutcome::StillConfirming, None);
    let outcome = receipt_outcome_of(&entry).expect("a fee hold is news");
    sut.dispatch(Event::ReceiptUpdate {
        user_op_hash: HASH.to_owned(),
        outcome,
    });
    let receipt = sut.view().receipt.expect("receipt");
    assert_eq!(receipt.status, SendReceiptStatus::Submitted);
    assert_eq!(receipt.hold_reason, Some(SendHoldReason::FeeHold));
}

/// The one tracker → receipt mapping, row by row.
#[test]
fn receipt_outcome_of_maps_every_tracker_status() {
    use vela_core::app::tx_tracker::{TrackOutcome, TrackStatus};
    let failed = |rejected, not_sent| SendReceiptOutcome::Failed {
        rejected,
        not_sent,
        refusal: None,
    };
    let rows = [
        (
            track_entry(TrackStatus::Confirmed, TrackOutcome::Final, Some("0xtx")),
            Some(SendReceiptOutcome::Confirmed {
                tx_hash: "0xtx".to_owned(),
            }),
        ),
        (
            track_entry(TrackStatus::Confirmed, TrackOutcome::Final, None),
            None,
        ),
        (
            track_entry(TrackStatus::Dropped, TrackOutcome::Final, Some("0xtx")),
            Some(failed(false, false)),
        ),
        (
            track_entry(TrackStatus::Rejected, TrackOutcome::Final, None),
            Some(failed(true, false)),
        ),
        (
            track_entry(TrackStatus::NotSent, TrackOutcome::Final, None),
            Some(failed(false, true)),
        ),
        (
            track_entry(TrackStatus::FeeHeld, TrackOutcome::StillConfirming, None),
            Some(SendReceiptOutcome::FeeHeld),
        ),
        (
            track_entry(TrackStatus::RelayFunding, TrackOutcome::Landing, None),
            Some(SendReceiptOutcome::RelayFunding),
        ),
        (
            track_entry(TrackStatus::Pending, TrackOutcome::MaybeSent, None),
            None,
        ),
        (
            track_entry(TrackStatus::Unreachable, TrackOutcome::Unknown, None),
            None,
        ),
        (
            track_entry(TrackStatus::Pending, TrackOutcome::Landing, None),
            Some(SendReceiptOutcome::Acknowledged),
        ),
        (
            track_entry(
                TrackStatus::AcceptedNotLanded,
                TrackOutcome::StillConfirming,
                None,
            ),
            Some(SendReceiptOutcome::Acknowledged),
        ),
    ];
    for (entry, expected) in rows {
        assert_eq!(
            receipt_outcome_of(&entry),
            expected,
            "{:?}/{:?}",
            entry.status,
            entry.outcome
        );
    }
}

/// Shells that predate 082 send none of the new fields.
#[test]
fn old_send_json_without_the_new_fields_still_decodes() {
    let submitted: Option<Res> =
        serde_json::from_str(r#"{"type":"submitted","user_op_hash":"0x1","now_ms":1}"#).ok();
    assert!(matches!(
        submitted,
        Some(Res::Submitted {
            maybe_sent: false,
            submit_block: None,
            ..
        })
    ));
    let failed: Option<SendReceiptOutcome> =
        serde_json::from_str(r#"{"type":"failed","rejected":true}"#).ok();
    assert_eq!(
        failed,
        Some(SendReceiptOutcome::Failed {
            refusal: None,
            rejected: true,
            not_sent: false
        })
    );
    let record: Option<SendTxRecord> = serde_json::from_str(
        r#"{"id":"0x1","user_op_hash":"0x1","tx_hash":"","from":"0xa","to":"0xb","to_name":null,"value":"1","symbol":"ETH","decimals":18,"logo_urls":[],"chain_id":1,"timestamp_s":1,"usd":null}"#,
    )
    .ok();
    assert_eq!(
        record.map(|r| (r.maybe_sent, r.submit_block)),
        Some((false, None))
    );
}

// ===========================================================================
// Spec 082 round 2 (T189, RJ1): the wallet's own Send writes ahead too
// ===========================================================================

const LOCAL_HASH: &str = "0x5538ce6978ab8924cce04d68c11cded19c36a8773e94656be6ada6315477ba1f";
const OTHER_HASH: &str = "0xa035b480c649967d6713388a017a91c8aea0c9732ed7f4ab51caba8a99199135";
const SIGNED_BLOCK: u64 = 48_487_600;

fn is_op(kind: fn(&Op) -> bool) -> impl Fn(&Op) -> bool {
    move |op| kind(op)
}

fn is_persist_records(op: &Op) -> bool {
    matches!(op, Op::PersistTxRecords { .. })
}

fn is_submit_op(op: &Op) -> bool {
    matches!(op, Op::SubmitUserOp { .. })
}

/// A split to two recipients, slid into the in-flight submit.
fn split_submitting() -> Sut {
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
    sut
}

fn split_ids(hash: &str) -> Vec<String> {
    vec![format!("{hash}-0"), format!("{hash}-1")]
}

/// Signed, and the records written ahead — the tracker told and the POST
/// cleared, in that order, only once the records are on disk. The tracker is
/// told of the op with NO record (082 second review): a POST is about to
/// leave, and it is held off "not sent" until the POST's verdict names them.
fn signed_and_cleared(sut: &mut Sut) -> Vec<SendTxRecord> {
    let ops = sut.dispatch(Event::OpSigned {
        user_op_hash: LOCAL_HASH.to_owned(),
        submit_block: Some(SIGNED_BLOCK),
        now_ms: 1_754_000_000_200.0,
    });
    let [Op::PersistTxRecords { records }] = ops.as_slice() else {
        panic!("the records first, and nothing else: {ops:?}")
    };
    let records = records.clone();
    let ops = sut.resolve_matching(is_op(is_persist_records), Res::RecordsPersisted);
    assert_eq!(
        ops,
        vec![
            Op::TrackSubmitted {
                sender: Some(ACCOUNT.to_owned()),
                user_op_hash: LOCAL_HASH.to_owned(),
                record_ids: vec![],
                chain_id: 1,
                maybe_sent: true,
                submit_block: Some(SIGNED_BLOCK),
                admitted: false,
            },
            Op::ClearToPost {
                user_op_hash: LOCAL_HASH.to_owned()
            },
        ]
    );
    assert!(sut
        .resolve_matching(
            |op| matches!(op, Op::TrackSubmitted { .. }),
            Res::TrackHandedOff
        )
        .is_empty());
    assert!(sut
        .resolve_matching(|op| matches!(op, Op::ClearToPost { .. }), Res::PostCleared)
        .is_empty());
    records
}

/// RJ1 (G34): both split records are written — "may have been sent" — before
/// the POST is cleared; the receipt screen still waits for the verdict.
#[test]
fn a_split_send_is_written_before_it_is_cleared_to_post() {
    let mut sut = split_submitting();
    let records = signed_and_cleared(&mut sut);
    assert_eq!(
        records.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
        split_ids(LOCAL_HASH)
    );
    assert!(records.iter().all(|r| r.maybe_sent
        && r.user_op_hash == LOCAL_HASH
        && r.submit_block == Some(SIGNED_BLOCK)));
    assert_eq!(records[0].to, RECIPIENT);
    assert_eq!(records[1].to, RECIPIENT_B);
    let view = sut.view();
    assert_ne!(
        view.stage,
        SendStage::Receipt,
        "the receipt waits for the verdict"
    );
    assert_eq!(view.tx_status, SendTxStatus::Submitting);
    assert!(
        sut.outstanding().iter().any(is_submit_op),
        "still submitting"
    );
}

/// RJ1: accepted — the records are marked admitted (no second write), the
/// tracker told `admitted`, and the success haptic plays: it was sent.
#[test]
fn accepted_marks_the_written_records_admitted() {
    let mut sut = split_submitting();
    signed_and_cleared(&mut sut);
    let ops = sut.resolve_matching(is_op(is_submit_op), {
        let mut result = submitted(LOCAL_HASH);
        if let Res::Submitted { submit_block, .. } = &mut result {
            *submit_block = Some(SIGNED_BLOCK);
        }
        result
    });
    assert!(
        !ops.iter().any(is_persist_records),
        "no second record: {ops:?}"
    );
    assert!(ops.contains(&Op::MarkAdmitted {
        record_ids: split_ids(LOCAL_HASH)
    }));
    assert!(ops.contains(&Op::TrackSubmitted {
        sender: Some(ACCOUNT.to_owned()),
        user_op_hash: LOCAL_HASH.to_owned(),
        record_ids: split_ids(LOCAL_HASH),
        chain_id: 1,
        maybe_sent: false,
        submit_block: Some(SIGNED_BLOCK),
        admitted: true,
    }));
    assert!(ops.contains(&Op::Haptic {
        kind: SendHapticKind::Success
    }));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::Receipt);
    assert_eq!(
        view.receipt.expect("receipt").status,
        SendReceiptStatus::Submitted
    );
}

/// A lost reply after the write-ahead: nothing new is written, no success
/// haptic (it has not earned it), the receipt says "may have been sent" —
/// and the tracker is handed the records now, "may have been sent": the POST
/// is over, and the relay's not-found grace counts from here (082 second
/// review).
#[test]
fn a_lost_reply_after_the_write_ahead_writes_nothing_new() {
    let mut sut = split_submitting();
    signed_and_cleared(&mut sut);
    let ops = sut.resolve_matching(
        is_op(is_submit_op),
        Res::Submitted {
            user_op_hash: LOCAL_HASH.to_owned(),
            now_ms: 1_754_000_000_500.0,
            maybe_sent: true,
            submit_block: Some(SIGNED_BLOCK),
        },
    );
    assert!(
        !ops.iter().any(|op| matches!(
            op,
            Op::PersistTxRecords { .. } | Op::MarkAdmitted { .. } | Op::Haptic { .. }
        )),
        "{ops:?}"
    );
    let handoffs: Vec<&Op> = ops
        .iter()
        .filter(|op| matches!(op, Op::TrackSubmitted { .. }))
        .collect();
    assert_eq!(
        handoffs,
        vec![&Op::TrackSubmitted {
            sender: Some(ACCOUNT.to_owned()),
            user_op_hash: LOCAL_HASH.to_owned(),
            record_ids: split_ids(LOCAL_HASH),
            chain_id: 1,
            maybe_sent: true,
            submit_block: Some(SIGNED_BLOCK),
            admitted: false,
        }]
    );
    assert_eq!(
        sut.view().receipt.expect("receipt").status,
        SendReceiptStatus::MaybeSent
    );
}

/// RJ1: proven not sent — both records deleted and withdrawn from the
/// tracker, then today's error.
#[test]
fn not_sent_deletes_both_written_records() {
    let mut sut = split_submitting();
    signed_and_cleared(&mut sut);
    let ops = sut.resolve_matching(
        is_op(is_submit_op),
        Res::SubmitFailed {
            failure: SendSubmitFailure::Other {
                message: Some("relay unreachable; nothing was sent".to_owned()),
            },
        },
    );
    assert!(ops.contains(&Op::DeleteTxRecords {
        ids: split_ids(LOCAL_HASH)
    }));
    assert!(ops.contains(&Op::TrackWithdrawn {
        user_op_hash: LOCAL_HASH.to_owned(),
        record_ids: split_ids(LOCAL_HASH),
    }));
    assert!(ops.contains(&Op::Haptic {
        kind: SendHapticKind::Error
    }));
    assert!(!ops.contains(&Op::Haptic {
        kind: SendHapticKind::Success
    }));
    let view = sut.view();
    assert_eq!(view.tx_status, SendTxStatus::Error);
    assert_eq!(view.tx_error, Some(SendTxErrorKey::Generic));
}

/// A submit that fails before the write-ahead's records were even acked
/// (the shell's WRITE_AHEAD_WAIT_MS ran out): the late ack neither tells the
/// tracker nor clears a POST.
#[test]
fn a_late_ack_after_the_failure_clears_nothing() {
    let mut sut = split_submitting();
    let ops = sut.dispatch(Event::OpSigned {
        user_op_hash: LOCAL_HASH.to_owned(),
        submit_block: None,
        now_ms: 1_754_000_000_200.0,
    });
    assert!(matches!(ops.as_slice(), [Op::PersistTxRecords { .. }]));
    let ops = sut.resolve_matching(
        is_op(is_submit_op),
        Res::SubmitFailed {
            failure: SendSubmitFailure::Other { message: None },
        },
    );
    assert!(ops.contains(&Op::DeleteTxRecords {
        ids: split_ids(LOCAL_HASH)
    }));
    let late = sut.resolve_matching(is_op(is_persist_records), Res::RecordsPersisted);
    assert!(late.is_empty(), "{late:?}");
}

/// The relay answering with another hash: withdraw the write-ahead, then
/// today's records under the relay's hash.
#[test]
fn another_relay_hash_withdraws_and_writes_under_the_relay_s() {
    let mut sut = split_submitting();
    signed_and_cleared(&mut sut);
    let ops = sut.resolve_matching(is_op(is_submit_op), submitted(OTHER_HASH));
    assert!(ops.contains(&Op::DeleteTxRecords {
        ids: split_ids(LOCAL_HASH)
    }));
    assert!(ops.contains(&Op::TrackWithdrawn {
        user_op_hash: LOCAL_HASH.to_owned(),
        record_ids: split_ids(LOCAL_HASH),
    }));
    let records = ops
        .iter()
        .find_map(|op| match op {
            Op::PersistTxRecords { records } => Some(records.clone()),
            _ => None,
        })
        .expect("today's write");
    assert_eq!(
        records.iter().map(|r| r.id.clone()).collect::<Vec<_>>(),
        split_ids(OTHER_HASH)
    );
}

/// Stray `OpSigned`s: outside a submit, and a second one, are dropped.
#[test]
fn a_stray_op_signed_is_dropped() {
    let mut sut = boot(vec![eth("2")]);
    assert!(sut
        .dispatch(Event::OpSigned {
            user_op_hash: LOCAL_HASH.to_owned(),
            submit_block: None,
            now_ms: 1.0,
        })
        .is_empty());
    let mut sut = split_submitting();
    signed_and_cleared(&mut sut);
    assert!(sut
        .dispatch(Event::OpSigned {
            user_op_hash: LOCAL_HASH.to_owned(),
            submit_block: None,
            now_ms: 2.0,
        })
        .is_empty());
}

/// The round-2 send wire.
#[test]
fn the_round_2_send_wire() {
    assert_eq!(
        serde_json::to_value(Op::ClearToPost {
            user_op_hash: "0x1".to_owned()
        })
        .unwrap_or_default(),
        serde_json::json!({ "type": "clear_to_post", "user_op_hash": "0x1" })
    );
    let old: Option<Op> = serde_json::from_str(
        r#"{"type":"track_submitted","user_op_hash":"0x1","record_ids":[],"chain_id":1}"#,
    )
    .ok();
    assert!(matches!(
        old,
        Some(Op::TrackSubmitted {
            admitted: false,
            ..
        })
    ));
    let signed: Option<Event> = serde_json::from_str(
        r#"{"type":"op_signed","user_op_hash":"0x1","submit_block":null,"now_ms":1}"#,
    )
    .ok();
    assert!(matches!(signed, Some(Event::OpSigned { .. })));
}

// ===========================================================================
// Spec 082 round 2 — adversarial review of T189
// ===========================================================================

/// The tracker's `NotSent` judges an op the relay never showed it holds. The
/// write-ahead hands the op over before the POST, so a slow POST can let the
/// tracker reach it before the relay's Accepted — and a shell may forward
/// that stale entry the moment the receipt names the hash. Stamped, the
/// receipt of an ACCEPTED payment read "not sent" for good (the stamp is
/// sticky): the words that make a person pay again. A may-have-been-sent
/// send still reads "not sent" on the verdict (RA4).
#[test]
fn a_not_sent_verdict_never_stamps_an_accepted_send() {
    let not_sent = || Event::ReceiptUpdate {
        user_op_hash: LOCAL_HASH.to_owned(),
        outcome: SendReceiptOutcome::Failed {
            refusal: None,
            rejected: false,
            not_sent: true,
        },
    };
    let mut sut = split_submitting();
    signed_and_cleared(&mut sut);
    sut.resolve_matching(is_op(is_submit_op), submitted(LOCAL_HASH));
    sut.dispatch(not_sent());
    assert_eq!(
        sut.view().receipt.expect("receipt").status,
        SendReceiptStatus::Submitted,
        "an accepted payment never reads 'not sent'"
    );

    let mut maybe = split_submitting();
    signed_and_cleared(&mut maybe);
    maybe.resolve_matching(
        is_op(is_submit_op),
        Res::Submitted {
            user_op_hash: LOCAL_HASH.to_owned(),
            now_ms: 1_754_000_000_500.0,
            maybe_sent: true,
            submit_block: Some(SIGNED_BLOCK),
        },
    );
    maybe.dispatch(not_sent());
    assert_eq!(
        maybe.view().receipt.expect("receipt").status,
        SendReceiptStatus::NotSent
    );
}

// ===========================================================================
// Spec 082 round 2 — second adversarial review: a cancelled submit's
// write-ahead
// ===========================================================================

/// The op a cancelled run signed: its hash differs from the retry's (a fresh
/// fee quote, an edited amount).
const CANCELLED_HASH: &str = "0x1e0b3c5a7f8e9d0c1b2a39485766f5e4d3c2b1a0f9e8d7c6b5a4938271605f4e";

/// The ✕ during "signing" can land after the passkey returned but before
/// `OpSigned`: the shell still joins the head read and the deployment check
/// (seconds on a bad network), and the sheet still says "signing". With the
/// write-ahead the core decides whether those bytes leave (RJ1), and the
/// person withdrew them — the payment they cancelled must not go out. Before,
/// the late `OpSigned` wrote the records and cleared the POST: a cancelled
/// payment sent, the form back on screen, then a jump to the receipt.
#[test]
fn a_cancelled_submit_is_never_cleared_to_post() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.dispatch(Event::SigningStarted);
    sut.dispatch(Event::CancelSigning);
    assert_eq!(sut.view().tx_status, SendTxStatus::Idle);
    let ops = sut.dispatch(Event::OpSigned {
        user_op_hash: CANCELLED_HASH.to_owned(),
        submit_block: Some(SIGNED_BLOCK),
        now_ms: 1_754_000_000_200.0,
    });
    assert!(ops.is_empty(), "nothing written, nothing cleared: {ops:?}");
    // The shell's wait runs out and it reports "not sent": the person
    // cancelled, so no error, no haptic — the form, as they left it.
    let ops = sut.resolve_matching(
        is_op(is_submit_op),
        Res::SubmitFailed {
            failure: SendSubmitFailure::Other { message: None },
        },
    );
    assert!(ops.is_empty(), "{ops:?}");
    let view = sut.view();
    assert_eq!(view.tx_status, SendTxStatus::Idle);
    assert!(!view.sending);
    // And the next slide sends normally.
    let ops = sut.dispatch(Event::SlideConfirm);
    assert!(
        matches!(ops.as_slice(), [Op::ProbeTreasury { .. }]),
        "{ops:?}"
    );
    let ops = sut.resolve_matching(|op| matches!(op, Op::ProbeTreasury { .. }), covered());
    assert!(
        matches!(ops.as_slice(), [Op::SubmitUserOp { .. }]),
        "{ops:?}"
    );
    let records = signed_and_cleared(&mut sut);
    assert_eq!(records[0].user_op_hash, LOCAL_HASH);
}

/// The worse race: after the ✕ the person slides again, the retry reaches its
/// own submit (its passkey still up), and only then the CANCELLED run's
/// `OpSigned` arrives. `OpSigned` names no pipeline, so it was taken for the
/// retry's: the retry's lines written under the cancelled op's hash and that
/// op cleared to POST, while the retry's own `OpSigned` was dropped ("once per
/// submit"). The retry's wait then ran out ("not sent", try again), and that
/// proven "not sent" deleted the records and made the tracker forget — the
/// cancelled op landed with no record and nothing following it, and the
/// person was told to pay again. While a cancelled submit's result is still
/// owed, no `OpSigned` is cleared: both runs end "not sent", nothing left.
#[test]
fn a_cancelled_run_s_late_op_signed_never_rides_the_retry() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.dispatch(Event::SigningStarted);
    sut.dispatch(Event::CancelSigning);
    let ops = sut.dispatch(Event::SlideConfirm);
    assert!(
        matches!(ops.as_slice(), [Op::ProbeTreasury { .. }]),
        "{ops:?}"
    );
    let ops = sut.resolve_matching(|op| matches!(op, Op::ProbeTreasury { .. }), covered());
    assert!(
        matches!(ops.as_slice(), [Op::SubmitUserOp { .. }]),
        "{ops:?}"
    );
    sut.dispatch(Event::SigningStarted);
    // The cancelled run's late `OpSigned`.
    let ops = sut.dispatch(Event::OpSigned {
        user_op_hash: CANCELLED_HASH.to_owned(),
        submit_block: Some(SIGNED_BLOCK),
        now_ms: 1_754_000_000_200.0,
    });
    assert!(
        !ops.iter().any(is_persist_records),
        "never written or cleared as the retry's: {ops:?}"
    );
    assert!(
        !sut.outstanding()
            .iter()
            .any(|op| matches!(op, Op::ClearToPost { .. })),
        "{:?}",
        sut.outstanding()
    );
    // The cancelled run's wait runs out ("not sent"): silent. The retry's
    // own `OpSigned` is then written and cleared as usual.
    let ops = sut.resolve_matching(
        is_op(is_submit_op),
        Res::SubmitFailed {
            failure: SendSubmitFailure::Other { message: None },
        },
    );
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(sut.view().tx_status, SendTxStatus::Signing, "the retry's");
    let records = signed_and_cleared(&mut sut);
    assert_eq!(records[0].user_op_hash, LOCAL_HASH);
}

// ===========================================================================
// Spec 096 F12 — sending to a token's own contract is said before the slide
//
// The real-dApp pass sent WBNB to the WBNB contract and the confirm page said
// only "First time sending here". A token contract has no way to give back
// what lands on it.
// ===========================================================================

/// A token held on the network the money moves on: its contract, typed as the
/// recipient, is said — in any letter case — and an ordinary address is not.
#[test]
fn a_held_tokens_contract_as_the_recipient_is_said() {
    let mut sut = boot(vec![eth("2"), usdc("9000"), dai("5")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, DAI);
    assert!(sut.view().recipient_is_token_contract);
    let shouted = format!("0x{}", DAI[2..].to_uppercase());
    set_recipient(&mut sut, &shouted);
    assert!(sut.view().recipient_is_token_contract, "case aside");
    set_recipient(&mut sut, RECIPIENT);
    assert!(!sut.view().recipient_is_token_contract);
}

/// The token being sent, to its own contract — the WBNB → WBNB case.
#[test]
fn the_token_being_sent_to_its_own_contract_is_said() {
    let mut sut = boot(vec![eth("2"), usdc("9000")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, USDC);
    assert!(sut.view().recipient_is_token_contract);
}

/// A token's contract on ANOTHER network is an address like any other on
/// this one; and a split's rows are not asked.
#[test]
fn only_the_networks_own_tokens_count_and_not_in_a_split() {
    const POLYGON_ONLY: &str = "0x4444444444444444444444444444444444444444";
    let polygon_token = SendToken {
        symbol: "PT".to_owned(),
        token_address: Some(POLYGON_ONLY.to_owned()),
        ..polygon_usdc("10")
    };
    let mut sut = boot(vec![eth("2"), polygon_pol("10"), polygon_token]);
    select_eth(&mut sut);
    set_recipient(&mut sut, POLYGON_ONLY);
    assert!(!sut.view().recipient_is_token_contract);

    let mut sut = boot(vec![eth("2"), usdc("9000")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, USDC);
    sut.dispatch(Event::EnterSplitMode);
    let view = sut.view();
    assert!(view.split_mode);
    assert!(!view.recipient_is_token_contract);
}

#[test]
fn the_token_contract_verdict_is_on_the_wire() {
    let mut sut = boot(vec![eth("2"), usdc("9000")]);
    select_usdc(&mut sut);
    set_recipient(&mut sut, USDC);
    let json = serde_json::to_value(sut.view()).expect("serializes");
    assert_eq!(json["recipient_is_token_contract"], true);
}

// ===========================================================================
// Spec 097 F — the page that signs names the address; the receipt every coin
// ===========================================================================

/// The developer wallet of the real-money pass, which the public registry
/// calls "Wallet" (`sw-022-golden-gnosis-confirm.png`).
const DEV_WALLET: &str = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c";

fn identity(name: &str, source: Option<&str>) -> Res {
    Res::IdentityResolved {
        identity: Some(vela_core::app::send::SendRecipientIdentity {
            name: Some(name.to_owned()),
            source: source.map(str::to_owned),
        }),
    }
}

/// Type `addr` and answer its identity lookup with `answer`.
fn set_named_recipient(sut: &mut Sut, addr: &str, answer: Res) {
    let ops = sut.dispatch(Event::SetRecipient {
        recipient: addr.to_owned(),
    });
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::ResolveIdentity { .. })),
        "a whole address is looked up: {ops:?}"
    );
    assert!(sut.resolve(answer).is_empty());
}

fn payee(address: &str, name: Option<&str>, source: Option<SendNameSource>) -> SendPayee {
    SendPayee {
        address: address.to_owned(),
        name: name.map(str::to_owned),
        name_source: source,
    }
}

/// S2: the confirm's To row printed "Wallet" — a name from the public
/// registry, where anyone can register any name — and the address only
/// behind a tap on the identicon. The core now says who is paid: the address,
/// the name beside it, and that the name is the registry's, not the person's.
#[test]
fn a_registry_name_never_stands_for_the_address_on_the_confirm() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_named_recipient(&mut sut, DEV_WALLET, identity("Wallet", Some("passkey")));
    let expected = vec![payee(
        DEV_WALLET,
        Some("Wallet"),
        Some(SendNameSource::Registry),
    )];
    assert_eq!(sut.view().payees, expected, "the form's line says the same");
    sut.dispatch(Event::SetAmount {
        amount: "0.1".to_owned(),
    });
    continue_to_confirm(&mut sut, native_fee(1, 1_000));
    let view = sut.view();
    assert_eq!(
        view.payees, expected,
        "the address the slide signs, in full"
    );
    let json = serde_json::to_value(&view).expect("serializes");
    assert_eq!(json["payees"][0]["address"], DEV_WALLET);
    assert_eq!(json["payees"][0]["name_source"]["type"], "registry");
}

/// The person's own account is named in their own word, untagged — and the
/// address is still beside it.
#[test]
fn an_own_name_carries_the_address_too() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_named_recipient(&mut sut, RECIPIENT, identity("Savings", Some("self")));
    sut.dispatch(Event::SetAmount {
        amount: "0.1".to_owned(),
    });
    continue_to_confirm(&mut sut, native_fee(1, 1_000));
    assert_eq!(
        sut.view().payees,
        vec![payee(RECIPIENT, Some("Savings"), Some(SendNameSource::Own))]
    );
}

/// A name service's name is a public claim too: it says which service.
#[test]
fn a_name_service_name_says_which_service() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_named_recipient(&mut sut, RECIPIENT, identity("bob.eth", Some("ENS")));
    let json = serde_json::to_value(sut.view()).expect("serializes");
    assert_eq!(
        json["payees"][0]["name_source"],
        serde_json::json!({ "type": "service", "label": "ENS" })
    );
}

/// A name nobody can say the source of could only be drawn untagged — the
/// way the person's own names look — so it is not drawn; nor is one no
/// screen could show. The address always is.
#[test]
fn a_name_of_unknown_source_or_unprintable_is_not_drawn() {
    for answer in [
        identity("Alice", None),
        identity("Alice", Some(" ")),
        identity("Al\u{fffd}ce", Some("passkey")),
        identity("   ", Some("passkey")),
    ] {
        let mut sut = boot(vec![eth("2")]);
        select_eth(&mut sut);
        set_named_recipient(&mut sut, RECIPIENT, answer);
        assert_eq!(sut.view().payees, vec![payee(RECIPIENT, None, None)]);
    }
}

/// Nobody is named before the address is whole.
#[test]
fn a_half_typed_address_names_nobody() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::SetRecipient {
        recipient: "0x14fB1fB2".to_owned(),
    });
    assert!(sut.view().payees.is_empty());
}

/// A split row named from the person's own contacts (or the name column of
/// their own list) is their word — and its address is carried beside it.
#[test]
fn a_split_rows_own_name_carries_its_address() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    sut.dispatch(Event::EnterSplitMode);
    sut.dispatch(Event::RecipientsChanged {
        recipients: vec![
            SendRecipientDraft {
                id: "rcpt_1".to_owned(),
                address: format!(" {RECIPIENT} "),
                amount: "0.5".to_owned(),
                name: Some("Bob".to_owned()),
            },
            SendRecipientDraft {
                id: "rcpt_2".to_owned(),
                address: RECIPIENT_B.to_owned(),
                amount: "0.25".to_owned(),
                name: Some("  ".to_owned()),
            },
        ],
    });
    continue_to_confirm(&mut sut, native_fee(1, 1_000));
    assert_eq!(
        sut.view().payees,
        vec![
            payee(RECIPIENT, Some("Bob"), Some(SendNameSource::Own)),
            payee(RECIPIENT_B, None, None),
        ]
    );
}

/// Start a two-coin sweep (ETH + USDC) to `to`, its identity answered with
/// `answer`, and walk it to the confirm.
fn sweep_to_confirm(to: &str, answer: Res) -> Sut {
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
    sut.resolve(fee_ok(native_fee(1, 500_000_000_000_000_000)));
    set_named_recipient(&mut sut, to, answer);
    sut.dispatch(Event::Continue);
    drain_form_quote(&mut sut);
    sut.resolve(fee_ok(native_fee(1, 500_000_000_000_000_000)));
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
    assert_eq!(sut.view().stage, SendStage::Confirm);
    sut
}

/// The sweep's confirm (`sw-054-golden-base-confirm.png`) had the same
/// "To Wallet" row.
#[test]
fn a_sweep_names_its_one_payee_with_the_address() {
    let sut = sweep_to_confirm(DEV_WALLET, identity("Wallet", Some("passkey")));
    assert_eq!(
        sut.view().payees,
        vec![payee(
            DEV_WALLET,
            Some("Wallet"),
            Some(SendNameSource::Registry)
        )]
    );
}

/// S3: a two-coin sweep's success screen read "Sent 0.000418 ETH" although
/// 0.034929 USDC moved in the same operation (`sw-054-golden-base-status-12.png`):
/// the receipt's one figure was the first coin's. It now lists every coin,
/// and no single figure stands for the batch.
#[test]
fn a_two_coin_sweep_receipt_lists_both_coins() {
    let mut sut = sweep_to_confirm(DEV_WALLET, identity("Wallet", Some("passkey")));
    slide_to_submit(&mut sut);
    sut.resolve(submitted(HASH));
    let view = sut.view();
    assert_eq!(view.stage, SendStage::Receipt);
    let receipt = view.receipt.expect("receipt");
    assert_eq!(receipt.kind, Some(SendReceiptKind::MultiSelect));
    let coins: Vec<(&str, &str, Option<&str>)> = receipt
        .coins
        .iter()
        .map(|coin| {
            (
                coin.amount.as_str(),
                coin.symbol.as_str(),
                coin.token_address.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        coins,
        vec![("1.5", "ETH", None), ("5", "USDC", Some(USDC))],
        "every coin, at the amount signed (ETH net of the gas reserve)"
    );
    assert_eq!(receipt.amount, "", "no one coin's figure heads a sweep");
    assert_eq!(receipt.usd_value, 1.5 * 2000.0 + 5.0, "all coins together");
}

/// A split's headline was its FIRST row ("Sent 0.5 ETH" for 0.5 + 0.25). The
/// one coin it sent is listed at its total.
#[test]
fn a_split_receipt_heads_with_its_total_not_its_first_row() {
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
    sut.resolve(submitted(HASH));
    let receipt = sut.view().receipt.expect("receipt");
    assert_eq!(receipt.coins.len(), 1);
    assert_eq!(receipt.coins[0].amount, "0.75");
    assert_eq!(receipt.coins[0].symbol, "ETH");
    assert_eq!(receipt.amount, "0.75");
    assert_eq!(receipt.usd_value, 0.75 * 2000.0);
    assert_eq!(receipt.transfers.len(), 2, "the people are still listed");
}

/// A single send's receipt is one coin, its figure exactly as signed.
#[test]
fn a_single_send_receipt_is_its_one_coin() {
    let mut sut = boot(vec![eth("2")]);
    select_eth(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "0.10".to_owned(),
    });
    continue_to_confirm(&mut sut, native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.resolve(submitted(HASH));
    let receipt = sut.view().receipt.expect("receipt");
    assert_eq!(receipt.coins.len(), 1);
    assert_eq!(receipt.coins[0].amount, receipt.amount);
    assert_eq!(receipt.coins[0].symbol, "ETH");
    assert_eq!(receipt.coins[0].logo_urls, vec!["eth.png".to_owned()]);
}

// ===========================================================================
// Spec 098 — the relay reaches the network you send on
// ===========================================================================

/// Continue on a 1-ETH send, with the estimate in, waiting on the treasury.
fn to_the_treasury_answer(sut: &mut Sut) {
    select_eth(sut);
    set_recipient(sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "1".to_owned(),
    });
    sut.dispatch(Event::Continue);
    drain_form_quote(sut);
    assert!(sut.resolve(fee_ok(native_fee(1, 1_000))).is_empty());
}

/// §2: the relay said it cannot serve this chain. Before 098 this went on to
/// confirm and the passkey, and failed after the person had signed.
#[test]
fn a_relay_that_cannot_serve_the_chain_stops_the_send_before_confirm() {
    let mut sut = boot(vec![eth("2")]);
    to_the_treasury_answer(&mut sut);
    let ops = sut.resolve(uncovered());
    assert!(ops.is_empty(), "no confirm probes, no watch: {ops:?}");
    let view = sut.view();
    assert_eq!(
        view.stage,
        SendStage::EnterDetails,
        "confirm is not entered"
    );
    let sheet = view.relay_unreachable.expect("the can't-reach sheet");
    assert_eq!(sheet.chain_id, 1);
    // The core, not the shell, says whose relay it is (chain 1 ships).
    assert!(sheet.operator_served);
    assert!(view.treasury_bootstrap.is_none(), "not the funding sheet");
}

/// §2: "the relay did not answer" stays transient — the send goes on.
#[test]
fn a_relay_that_did_not_answer_is_still_transient() {
    let mut sut = boot(vec![eth("2")]);
    to_the_treasury_answer(&mut sut);
    sut.resolve(unknown());
    let view = sut.view();
    assert_eq!(view.stage, SendStage::Confirm);
    assert!(view.relay_unreachable.is_none());
}

/// §2: retry re-runs the pre-check (after the person changed the RPC or relay);
/// dismiss just closes.
#[test]
fn the_cant_reach_sheet_retries_through_the_precheck_and_dismisses_cleanly() {
    let mut sut = boot(vec![eth("2")]);
    to_the_treasury_answer(&mut sut);
    sut.resolve(uncovered());
    let ops = sut.dispatch(Event::RetryRelayUnreachable);
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
    assert!(sut.view().relay_unreachable.is_none());
    // The first run's 15 s timer is still outstanding and older: answer by kind.
    sut.resolve_matching(
        |op| matches!(op, Op::EstimateFee { .. }),
        fee_ok(native_fee(1, 1_000)),
    );
    answer_the_watch(&mut sut, uncovered());
    assert!(
        sut.view().relay_unreachable.is_some(),
        "still unreachable: shown again"
    );
    sut.dispatch(Event::DismissRelayUnreachable);
    assert!(sut.view().relay_unreachable.is_none());
}

/// §2: the pre-sign recheck stops on "can't reach" too — before the passkey.
#[test]
fn the_pre_sign_recheck_stops_on_a_relay_that_cannot_serve_the_chain() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    let ops = sut.dispatch(Event::SlideConfirm);
    assert!(matches!(ops.as_slice(), [Op::ProbeTreasury { .. }]));
    let ops = sut.resolve(uncovered());
    assert!(ops.is_empty(), "no SubmitUserOp: {ops:?}");
    let view = sut.view();
    assert!(view.relay_unreachable.is_some());
    assert_eq!(view.tx_status, SendTxStatus::Idle);
    assert!(!view.sending, "the lock is released");
}

/// §4: the open sheet asks the relay again on its own; once funded, the send
/// goes on to confirm by itself.
#[test]
fn a_funded_relay_closes_the_sheet_and_the_send_carries_on() {
    let mut sut = boot(vec![eth("2")]);
    to_the_treasury_answer(&mut sut);
    assert_only_the_watch(&sut.resolve(low_float()), "the sheet opens");

    // The wait elapses: one probe.
    let ops = elapse_the_watch(&mut sut);
    assert!(
        matches!(ops.as_slice(), [Op::ProbeTreasury { chain_id: 1 }]),
        "{ops:?}"
    );
    // Funded: the pre-check runs again, and that is what leads to confirm.
    let ops = answer_the_watch(&mut sut, covered());
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
    assert!(sut.view().treasury_bootstrap.is_none());
}

/// §4: still short — the sheet shows the new balance and waits again.
#[test]
fn a_relay_still_short_refreshes_the_balance_and_waits_again() {
    let mut sut = boot(vec![eth("2")]);
    to_the_treasury_answer(&mut sut);
    sut.resolve(low_float());
    elapse_the_watch(&mut sut);
    let mut topped = treasury_status();
    topped.balance = "7".to_owned();
    let ops = answer_the_watch(
        &mut sut,
        Res::TreasuryProbed {
            probe: SendTreasuryProbe::LowFloat { status: topped },
        },
    );
    assert_only_the_watch(&ops, "waits again");
    assert_eq!(
        sut.view().treasury_bootstrap.expect("still open").balance,
        "7"
    );
}

/// §4: on confirm, funding closes the sheet but NEVER starts the passkey —
/// that is the person's to press, and browsers refuse one nobody pressed for.
#[test]
fn on_confirm_a_funded_relay_closes_the_sheet_without_signing() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    sut.dispatch(Event::SlideConfirm);
    sut.resolve(low_float());
    elapse_the_watch(&mut sut);
    let ops = answer_the_watch(&mut sut, covered());
    assert!(ops.is_empty(), "no submission, no passkey: {ops:?}");
    let view = sut.view();
    assert!(view.treasury_bootstrap.is_none());
    assert_eq!(view.stage, SendStage::Confirm);
    assert_eq!(view.tx_status, SendTxStatus::Idle);
}

/// §4: a closed sheet stops watching — a late wait or answer changes nothing.
#[test]
fn a_dismissed_sheet_stops_watching() {
    let mut sut = boot(vec![eth("2")]);
    to_the_treasury_answer(&mut sut);
    sut.resolve(low_float());
    sut.dispatch(Event::DismissTreasurySheet);
    let ops = elapse_the_watch(&mut sut);
    assert!(ops.is_empty(), "no probe after the sheet closed: {ops:?}");
    assert_eq!(sut.view().stage, SendStage::EnterDetails);
}

// ===========================================================================
// Issue #422 — a relay stop belongs to its chain, and speaks its chain's coin
// ===========================================================================
//
// On a Xiaomi (v0.9.6), 0.8 POL on Polygon reached confirm under "Start this
// network's relayer … out of gas … Suggested contribution: 0.0001 ETH". The
// relay's Polygon treasury held 56.86 POL (`bootstrapNeeded: false`, read
// 2026-10-04), and the send went through. The stop was another chain's: an
// ETH chain whose float the relay reports empty (balance 0, floor 0.0001 —
// thirteen of the networks Vela ships answered that way that day). Nothing
// but the stop's own two buttons ever cleared it, so it rode along to the
// next coin; and the phone that drew it named its coin by a lookup that says
// "ETH" for anything it does not know.

/// Unichain: a network Vela ships, ETH its coin, and one whose relayer the
/// relay reported empty on 2026-10-04.
fn unichain_eth(balance: &str) -> SendToken {
    SendToken {
        network: "unichain".to_owned(),
        chain_id: 130,
        symbol: "ETH".to_owned(),
        balance: balance.to_owned(),
        decimals: 18,
        token_address: None,
        price_usd: Some(2000.0),
        logo_urls: vec![],
        spam: false,
    }
}

/// Tempo's pathUSD — the one treasury that is not counted in a native coin.
fn tempo_path_usd(balance: &str) -> SendToken {
    SendToken {
        network: "tempo".to_owned(),
        chain_id: 4217,
        symbol: "pathUSD".to_owned(),
        balance: balance.to_owned(),
        decimals: 6,
        token_address: Some("0x20c0000000000000000000000000000000000000".to_owned()),
        price_usd: Some(1.0),
        logo_urls: vec![],
        spam: false,
    }
}

/// A network the person added: chain 7_777_001, its coin "DEV".
fn devnet_coin(balance: &str) -> SendToken {
    SendToken {
        network: "devnet".to_owned(),
        chain_id: 7_777_001,
        symbol: "DEV".to_owned(),
        balance: balance.to_owned(),
        decimals: 18,
        token_address: None,
        price_usd: None,
        logo_urls: vec![],
        spam: false,
    }
}

/// What the relay answers for a treasury below its floor, as the shells
/// hand it over (base units, decimal; the verdicts left to the core).
fn short_treasury(chain_id: u32, asset: SendTreasuryAsset, balance: &str, floor: &str) -> Res {
    Res::TreasuryProbed {
        probe: SendTreasuryProbe::LowFloat {
            status: SendTreasuryStatus {
                chain_id,
                address: "0x3e59292e18417f814112f731e7163534c6d2fe3c".to_owned(),
                asset,
                balance: balance.to_owned(),
                floor: floor.to_owned(),
                bootstrap_needed: true,
                operator_served: false,
                coin: None,
            },
        },
    }
}

/// The relay's native floor: `0x5af3107a4000` = 10^14 base units, the same
/// on every chain it serves (vela-relay `NATIVE_TREASURY_FLOOR`).
const RELAY_NATIVE_FLOOR: &str = "100000000000000";

/// An empty relayer on `chain_id`, exactly as the relay reported it live.
fn empty_relayer(chain_id: u32) -> Res {
    short_treasury(chain_id, SendTreasuryAsset::Native, "0", RELAY_NATIVE_FLOOR)
}

/// Pick `token`, address it, type `amount`, and press Continue: the estimate
/// is answered, so the next answer is the treasury's — for `token`'s chain.
fn continue_with(sut: &mut Sut, token: &SendToken, amount: &str) {
    let ops = sut.dispatch(Event::SelectToken {
        token_id: token.id(),
    });
    assert_eq!(
        ops,
        vec![Op::LoadAccountCredential {
            account_id: "cred-1".to_owned()
        }]
    );
    settle_warm_quote(sut);
    set_recipient(sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: amount.to_owned(),
    });
    let ops = sut.dispatch(Event::Continue);
    drain_form_quote(sut);
    assert!(
        matches!(
            ops.as_slice(),
            [
                Op::EstimateFee { .. },
                Op::ProbeTreasury { chain_id },
                Op::StartTimer { .. }
            ] if *chain_id == token.chain_id
        ),
        "the pre-check asks about {}: {ops:?}",
        token.chain_id
    );
    assert!(sut
        .resolve(fee_ok(native_fee(token.chain_id, 1_000)))
        .is_empty());
}

/// The pre-check's 15 s timer, which never fires in these runs.
fn drop_the_precheck_timer(sut: &mut Sut) {
    sut.drop_matching(|op| {
        matches!(
            op,
            Op::StartTimer {
                tag: SendTimerTag::EstimateTimeout,
                ..
            }
        )
    });
}

/// The stop on `chain_id` is up on the form, and is what the view publishes.
#[track_caller]
fn assert_stop_on(sut: &Sut, chain_id: u32) -> SendTreasuryStatus {
    let view = sut.view();
    assert_eq!(
        view.stage,
        SendStage::EnterDetails,
        "confirm is not entered"
    );
    let status = view.treasury_bootstrap.expect("the treasury stop");
    assert_eq!(status.chain_id, chain_id);
    status
}

/// Answer the confirm page's own probes (risk, simulation) so nothing is left
/// pending that a later step could mistake for its own.
fn settle_confirm_probes(sut: &mut Sut, probes: Vec<Op>) {
    for op in probes {
        match op {
            Op::ResolveRisk { .. } => {
                sut.resolve_matching(
                    |op| matches!(op, Op::ResolveRisk { .. }),
                    Res::RiskResolved { risk: None },
                );
            }
            Op::SimulateCalls { .. } => {
                sut.resolve_matching(
                    |op| matches!(op, Op::SimulateCalls { .. }),
                    Res::SimResolved { sim_json: None },
                );
            }
            _ => {}
        }
    }
}

/// Cause one, as the Xiaomi met it: the stop on an empty chain, the token
/// card to POL on Polygon — funded — and Polygon's confirm under no stop.
#[test]
fn a_stop_on_one_chain_does_not_follow_the_send_to_another_422() {
    let mut sut = boot(vec![unichain_eth("1"), polygon_pol("5")]);
    continue_with(&mut sut, &unichain_eth("1"), "0.1");
    assert_only_the_watch(&sut.resolve(empty_relayer(130)), "Unichain's stop");
    assert_stop_on(&sut, 130);

    // Another coin, on another chain: the stop was about Unichain.
    sut.dispatch(Event::ChangeToken);
    let view = sut.view();
    assert!(view.treasury_bootstrap.is_none(), "Unichain's stop is gone");
    // …and so is its watch: no more asking about Unichain, and no "funded"
    // answer from it to press Continue on a Polygon form.
    assert!(
        elapse_the_watch(&mut sut).is_empty(),
        "the watch went with its stop"
    );
    // The first pre-check's 15 s timer never fires in this run.
    sut.drop_matching(|op| matches!(op, Op::StartTimer { .. }));

    continue_with(&mut sut, &polygon_pol("5"), "0.8");
    let probes = sut.resolve(covered());
    let view = sut.view();
    assert_eq!(view.stage, SendStage::Confirm, "Polygon's relayer serves");
    assert!(
        view.treasury_bootstrap.is_none(),
        "no out-of-gas notice on Polygon's confirm: {:?}",
        view.treasury_bootstrap
    );
    assert!(view.relay_unreachable.is_none());
    settle_confirm_probes(&mut sut, probes);
}

/// The same through Back (the arrow, not the card), and for the other stop:
/// a relay that cannot reach one network says nothing about the next.
#[test]
fn back_leaves_either_relay_stop_behind_with_its_chain_422() {
    let mut sut = boot(vec![unichain_eth("1"), polygon_pol("5"), devnet_coin("3")]);

    continue_with(&mut sut, &unichain_eth("1"), "0.1");
    sut.resolve(empty_relayer(130));
    assert_stop_on(&sut, 130);
    sut.dispatch(Event::Back);
    assert!(sut.view().treasury_bootstrap.is_none());
    sut.drop_matching(|op| matches!(op, Op::StartTimer { .. }));

    continue_with(&mut sut, &devnet_coin("3"), "1");
    assert!(sut.resolve(uncovered()).is_empty());
    let sheet = sut.view().relay_unreachable.expect("the can't-reach stop");
    assert_eq!(sheet.chain_id, 7_777_001);
    sut.dispatch(Event::Back);
    assert!(sut.view().relay_unreachable.is_none());
    sut.drop_matching(|op| matches!(op, Op::StartTimer { .. }));

    continue_with(&mut sut, &polygon_pol("5"), "0.8");
    let probes = sut.resolve(covered());
    let view = sut.view();
    assert_eq!(view.stage, SendStage::Confirm);
    assert!(view.treasury_bootstrap.is_none());
    assert!(view.relay_unreachable.is_none());
    settle_confirm_probes(&mut sut, probes);
}

/// A pre-check the relay passes is the latest word on the chain: whatever
/// stop stood before it, confirm opens without one. (Before #422 confirm
/// opened UNDER it, with "Confirm & Send" disabled by a stop the core had
/// just overruled.)
#[test]
fn a_precheck_the_relay_passes_opens_confirm_under_no_stop_422() {
    let mut sut = boot(vec![polygon_pol("5")]);
    continue_with(&mut sut, &polygon_pol("5"), "0.8");
    sut.resolve(empty_relayer(137));
    assert_stop_on(&sut, 137);
    // The person tops it up elsewhere and presses Continue on the form.
    drop_the_precheck_timer(&mut sut);
    let ops = sut.dispatch(Event::Continue);
    assert!(
        matches!(
            ops.as_slice(),
            [
                Op::EstimateFee { .. },
                Op::ProbeTreasury { chain_id: 137 },
                Op::StartTimer { .. }
            ]
        ),
        "{ops:?}"
    );
    sut.resolve_matching(
        |op| matches!(op, Op::EstimateFee { .. }),
        fee_ok(native_fee(137, 1_000)),
    );
    let probes = sut.resolve_matching(|op| matches!(op, Op::ProbeTreasury { .. }), covered());
    let view = sut.view();
    assert_eq!(view.stage, SendStage::Confirm);
    assert!(view.treasury_bootstrap.is_none());
    assert!(
        elapse_the_watch(&mut sut).is_empty(),
        "nothing left watching"
    );
    settle_confirm_probes(&mut sut, probes);
}

/// The issue's expectation, plainly: Polygon's relayer funded → no notice,
/// on the form or on confirm, and nothing standing in the way of the slide.
#[test]
fn a_funded_polygon_relayer_shows_no_notice_422() {
    let mut sut = boot(vec![polygon_pol("5")]);
    continue_with(&mut sut, &polygon_pol("5"), "0.8");
    let probes = sut.resolve(covered());
    let view = sut.view();
    assert_eq!(view.stage, SendStage::Confirm);
    assert!(view.treasury_bootstrap.is_none());
    assert!(view.relay_unreachable.is_none());
    settle_confirm_probes(&mut sut, probes);
    let view = sut.view();
    assert!(view.can_confirm, "the slide is open");
    // The pre-sign recheck asks about Polygon, and goes on to submit.
    let ops = sut.dispatch(Event::SlideConfirm);
    assert!(
        matches!(ops.as_slice(), [Op::ProbeTreasury { chain_id: 137 }]),
        "{ops:?}"
    );
    let ops = sut.resolve_matching(|op| matches!(op, Op::ProbeTreasury { .. }), covered());
    assert!(
        matches!(ops.as_slice(), [Op::SubmitUserOp { .. }]),
        "{ops:?}"
    );
    assert!(sut.view().treasury_bootstrap.is_none());
}

/// Cause two: the issue's other expectation. Polygon's relayer out of gas →
/// the contribution is in POL, and it is Polygon's own figure — what its
/// treasury lacks of its floor, in POL's decimals — never a constant and
/// never another chain's.
#[test]
fn an_empty_polygon_relayer_asks_in_pol_for_polygons_own_shortfall_422() {
    let mut sut = boot(vec![polygon_pol("5")]);
    continue_with(&mut sut, &polygon_pol("5"), "0.8");
    // Holding 0.00004 POL of the relay's 0.0001 POL floor.
    sut.resolve(short_treasury(
        137,
        SendTreasuryAsset::Native,
        "40000000000000",
        RELAY_NATIVE_FLOOR,
    ));
    let status = assert_stop_on(&sut, 137);
    assert!(status.operator_served, "Polygon ships with Vela");
    assert_eq!(
        status.coin,
        Some(SendTreasuryCoin {
            symbol: Some("POL".to_owned()),
            balance: "0.00004".to_owned(),
            floor: "0.0001".to_owned(),
            suggested: "0.00006".to_owned(),
        })
    );

    // The figure is the relay's for Polygon, whatever it is: a floor the
    // relay sizes for POL is asked for in POL, digit for digit.
    elapse_the_watch(&mut sut);
    answer_the_watch(
        &mut sut,
        short_treasury(
            137,
            SendTreasuryAsset::Native,
            "250000000000000000",
            "2500000000000000000",
        ),
    );
    let coin = assert_stop_on(&sut, 137).coin.expect("figures");
    assert_eq!(coin.symbol.as_deref(), Some("POL"));
    assert_eq!(
        (
            coin.balance.as_str(),
            coin.floor.as_str(),
            coin.suggested.as_str()
        ),
        ("0.25", "2.5", "2.25")
    );
}

/// Each chain's stop names that chain's coin — the one the relay's
/// `eth_getBalance` counts — and a chain the wallet cannot name gets no
/// name at all, rather than "ETH".
#[test]
fn every_stop_names_its_own_chains_coin_and_never_guesses_422() {
    // Unichain: ETH, from the built-in registry (the shell's network list
    // in this run names only Ethereum and Polygon).
    let mut sut = boot(vec![unichain_eth("1")]);
    continue_with(&mut sut, &unichain_eth("1"), "0.1");
    sut.resolve(empty_relayer(130));
    let coin = assert_stop_on(&sut, 130).coin.expect("figures");
    assert_eq!(coin.symbol.as_deref(), Some("ETH"));
    assert_eq!(coin.suggested, "0.0001");
    assert_eq!(coin.balance, "0");

    // A network the person added: its coin from the networks the shell
    // handed over.
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams::default()));
    let mut networks = chains();
    networks.push(SendChainInfo {
        chain_id: 7_777_001,
        network: "devnet".to_owned(),
        native_symbol: "DEV".to_owned(),
    });
    let ops = sut.resolve(Res::TokensLoaded {
        tokens: Some(vec![devnet_coin("3")]),
        chains: networks,
    });
    for _ in &ops {
        sut.resolve(Res::FeesPrewarmed);
    }
    continue_with(&mut sut, &devnet_coin("3"), "1");
    sut.resolve(empty_relayer(7_777_001));
    let status = assert_stop_on(&sut, 7_777_001);
    assert!(!status.operator_served, "a network the person added");
    assert_eq!(status.coin.expect("figures").symbol.as_deref(), Some("DEV"));

    // The same network unnamed by the list: the coin held there names it.
    let mut sut = boot(vec![devnet_coin("3")]);
    continue_with(&mut sut, &devnet_coin("3"), "1");
    sut.resolve(empty_relayer(7_777_001));
    assert_eq!(
        assert_stop_on(&sut, 7_777_001)
            .coin
            .expect("figures")
            .symbol
            .as_deref(),
        Some("DEV")
    );
}

/// Tempo's treasury is counted in micro-pathUSD: six decimals, not eighteen.
#[test]
fn a_tempo_stop_counts_path_usd_in_six_decimals_422() {
    let mut sut = boot(vec![tempo_path_usd("10")]);
    continue_with(&mut sut, &tempo_path_usd("10"), "1");
    // 0.1 of the relay's 0.55 pathUSD floor.
    sut.resolve(short_treasury(
        4217,
        SendTreasuryAsset::PathUsd,
        "100000",
        "550000",
    ));
    assert_eq!(
        assert_stop_on(&sut, 4217).coin,
        Some(SendTreasuryCoin {
            symbol: Some("pathUSD".to_owned()),
            balance: "0.1".to_owned(),
            floor: "0.55".to_owned(),
            suggested: "0.45".to_owned(),
        })
    );
}

/// A figure that is not a whole number of base units is not read as zero:
/// the stop stands, and names no amount.
#[test]
fn an_unreadable_treasury_figure_names_no_amount_422() {
    let mut sut = boot(vec![polygon_pol("5")]);
    continue_with(&mut sut, &polygon_pol("5"), "0.8");
    sut.resolve(short_treasury(
        137,
        SendTreasuryAsset::Native,
        "0x0",
        RELAY_NATIVE_FLOOR,
    ));
    assert_eq!(assert_stop_on(&sut, 137).coin, None);
}

/// The coin rides the published stop only: a shell's probe without it
/// still reads (the field is the core's to fill), on every wire the four
/// shells send.
#[test]
fn a_probe_without_the_coin_is_still_a_probe_422() {
    let wire = serde_json::json!({
        "type": "low_float",
        "status": {
            "chain_id": 137,
            "address": "0x3e59292e18417f814112f731e7163534c6d2fe3c",
            "asset": "native",
            "balance": "0",
            "floor": RELAY_NATIVE_FLOOR,
            "bootstrap_needed": true
        }
    });
    let probe: SendTreasuryProbe = serde_json::from_value(wire).expect("reads");
    let SendTreasuryProbe::LowFloat { status } = probe else {
        panic!("a low float");
    };
    assert_eq!(status.coin, None);
    assert!(!status.operator_served);
}

// ===========================================================================
// Issue #424 — Continue on Unichain, into a relay with no gas there
// ===========================================================================
//
// The shape the device met (Android 0.9.6, 2026-10-04): 0.0001 ETH on
// Unichain from a 0.002336 balance, a 0.000013 ETH fee, and a relay whose
// treasury on chain 130 had never been funded —
// `GET /v1/treasury/130` → `{"balance":"0x0","floor":"0x5af3107a4000",
// "bootstrapNeeded":true}`. The core stopped the send on the form and said
// why; the phones of that release drew the stop on the confirm page only, so
// the tap did nothing anyone could see.

const UNICHAIN: u32 = 130;
/// The relay's treasury, as `/v1/treasury/130` names it.
const RELAY_TREASURY: &str = "0x3e59292e18417f814112f731e7163534c6d2fe3c";

fn unichain_eth_at_2700(balance: &str) -> SendToken {
    SendToken {
        network: "unichain".to_owned(),
        chain_id: UNICHAIN,
        symbol: "ETH".to_owned(),
        balance: balance.to_owned(),
        decimals: 18,
        token_address: None,
        price_usd: Some(2_700.0),
        logo_urls: vec![],
        spam: false,
    }
}

/// `/v1/treasury/130` on 2026-10-04: nothing there, a 0.0001 ETH floor.
fn unichain_empty_treasury() -> Res {
    Res::TreasuryProbed {
        probe: SendTreasuryProbe::LowFloat {
            status: SendTreasuryStatus {
                chain_id: UNICHAIN,
                address: RELAY_TREASURY.to_owned(),
                asset: SendTreasuryAsset::Native,
                balance: "0".to_owned(),
                floor: "100000000000000".to_owned(),
                bootstrap_needed: true,
                operator_served: false,
                // The relay's probe names no coin; the core fills it when it
                // publishes the stop (#432).
                coin: None,
            },
        },
    }
}

/// The form as the screenshot shows it, Continue pressed, the estimate in:
/// waiting on the treasury.
fn unichain_to_the_treasury_answer() -> Sut {
    let token = unichain_eth_at_2700("0.002336");
    let mut sut = Sut::new();
    sut.dispatch(open_event(SendOpenParams::default()));
    let ops = sut.resolve(Res::TokensLoaded {
        tokens: Some(vec![token.clone()]),
        chains: vec![SendChainInfo {
            chain_id: UNICHAIN,
            network: "unichain".to_owned(),
            native_symbol: "ETH".to_owned(),
        }],
    });
    for _ in &ops {
        sut.resolve(Res::FeesPrewarmed);
    }
    sut.dispatch(Event::SelectToken {
        token_id: token.id(),
    });
    settle_warm_quote(&mut sut);
    set_recipient(&mut sut, RECIPIENT);
    sut.dispatch(Event::SetAmount {
        amount: "0.0001".to_owned(),
    });
    assert!(
        sut.view().can_continue,
        "the form is complete: Continue is live"
    );

    let ops = sut.dispatch(Event::Continue);
    drain_form_quote(&mut sut);
    assert!(
        matches!(
            ops.as_slice(),
            [
                Op::EstimateFee {
                    chain_id: UNICHAIN,
                    ..
                },
                Op::ProbeTreasury { chain_id: UNICHAIN },
                Op::StartTimer {
                    tag: SendTimerTag::EstimateTimeout,
                    ..
                }
            ]
        ),
        "the pre-check asks Unichain's fee and Unichain's treasury: {ops:?}"
    );
    assert!(sut
        .resolve(fee_ok(native_fee(UNICHAIN, 13_000_000_000_000)))
        .is_empty());
    sut
}

/// The core's half of #424: Continue into an empty relay is never silent. It
/// stays on the form, names the relay's treasury and what it lacks, and the
/// form's one button stays live — it is the stop's retry.
#[test]
fn unichain_continue_into_an_empty_relay_stops_on_the_form_and_says_why() {
    let mut sut = unichain_to_the_treasury_answer();
    assert_only_the_watch(
        &sut.resolve(unichain_empty_treasury()),
        "the funding stop opens and watches",
    );
    let view = sut.view();
    assert_eq!(
        view.stage,
        SendStage::EnterDetails,
        "confirm is not entered"
    );
    assert!(!view.estimating_gas, "the check is over");
    let stop = view
        .treasury_bootstrap
        .expect("the reason is in the view, on the form");
    assert_eq!(stop.chain_id, UNICHAIN);
    assert_eq!(stop.address, RELAY_TREASURY);
    assert_eq!(
        (stop.balance.as_str(), stop.floor.as_str()),
        ("0", "100000000000000")
    );
    // Unichain ships with Vela: the relay is the operator's to fund.
    assert!(stop.operator_served);
    assert!(view.relay_unreachable.is_none());
    // The button's gate is the core's one predicate, whatever the button says.
    assert!(view.can_continue, "the stop's retry is pressable");
    assert!(!view.can_confirm);
}

/// The form's button while a stop is up is Continue, and every shell may send
/// exactly that: pressing it lowers the stop and asks again. A relay funded
/// meanwhile then reaches confirm with nothing stale on it — before #424's fix
/// the old stop rode along and blocked the confirm page.
#[test]
fn continue_with_the_funding_stop_up_is_its_retry_and_leaves_nothing_stale() {
    let mut sut = unichain_to_the_treasury_answer();
    sut.resolve(unichain_empty_treasury());
    assert!(sut.view().treasury_bootstrap.is_some());

    let ops = sut.dispatch(Event::Continue);
    assert!(
        matches!(
            ops.as_slice(),
            [
                Op::EstimateFee { .. },
                Op::ProbeTreasury { chain_id: UNICHAIN },
                Op::StartTimer {
                    tag: SendTimerTag::EstimateTimeout,
                    ..
                }
            ]
        ),
        "the pre-check runs again: {ops:?}"
    );
    let view = sut.view();
    assert!(
        view.treasury_bootstrap.is_none(),
        "the stop is down while the relay is asked again"
    );
    assert!(view.estimating_gas);
    // The stop's watch went with it: its wait elapsing asks nothing.
    assert!(
        elapse_the_watch(&mut sut).is_empty(),
        "no watch outlives its stop"
    );

    sut.resolve_matching(
        |op| matches!(op, Op::EstimateFee { .. }),
        fee_ok(native_fee(UNICHAIN, 13_000_000_000_000)),
    );
    sut.resolve_matching(|op| matches!(op, Op::ProbeTreasury { .. }), covered());
    let view = sut.view();
    assert_eq!(view.stage, SendStage::Confirm, "funded: on to confirm");
    assert!(
        view.treasury_bootstrap.is_none(),
        "no stale funding stop on the confirm page"
    );
    assert!(view.can_confirm);
}

/// The same for the can't-reach stop: Continue is its retry too.
#[test]
fn continue_with_the_cant_reach_stop_up_is_its_retry_and_leaves_nothing_stale() {
    let mut sut = unichain_to_the_treasury_answer();
    assert!(sut.resolve(uncovered()).is_empty());
    assert!(sut.view().relay_unreachable.is_some());
    assert!(sut.view().can_continue, "the stop's retry is pressable");

    sut.dispatch(Event::Continue);
    assert!(sut.view().relay_unreachable.is_none());
    sut.resolve_matching(
        |op| matches!(op, Op::EstimateFee { .. }),
        fee_ok(native_fee(UNICHAIN, 13_000_000_000_000)),
    );
    sut.resolve_matching(|op| matches!(op, Op::ProbeTreasury { .. }), covered());
    let view = sut.view();
    assert_eq!(view.stage, SendStage::Confirm);
    assert!(view.relay_unreachable.is_none(), "no stale stop on confirm");
    assert!(view.can_confirm);
}

/// …and asked again while still empty, the stop comes back — said again, never
/// a silent no-op.
#[test]
fn continue_into_a_relay_still_empty_brings_the_stop_back() {
    let mut sut = unichain_to_the_treasury_answer();
    sut.resolve(unichain_empty_treasury());
    sut.dispatch(Event::Continue);
    sut.resolve_matching(
        |op| matches!(op, Op::EstimateFee { .. }),
        fee_ok(native_fee(UNICHAIN, 13_000_000_000_000)),
    );
    let ops = sut.resolve_matching(
        |op| matches!(op, Op::ProbeTreasury { .. }),
        unichain_empty_treasury(),
    );
    assert_only_the_watch(&ops, "the stop opens again, and watches again");
    let view = sut.view();
    assert_eq!(view.stage, SendStage::EnterDetails);
    assert_eq!(
        view.treasury_bootstrap.map(|stop| stop.chain_id),
        Some(UNICHAIN)
    );
}

// ===========================================================================
// Issue #466 — "Report this" files a complete report, built by the core
// ===========================================================================

/// The Unichain stop of the issue, whose "Report this" opened an empty GitHub
/// form. The core builds what it files: a title line, the relay treasury's
/// full address, what it holds against its floor in ETH, the bug form's area
/// — and a fingerprint that stays put while the watch re-reads the balance,
/// so every report of one outage lands on one issue.
#[test]
fn the_operator_stop_carries_a_complete_report_466() {
    let mut sut = unichain_to_the_treasury_answer();
    assert!(sut.view().relay_report.is_none(), "no stop, no report");
    sut.resolve(unichain_empty_treasury());
    let report = sut
        .view()
        .relay_report
        .expect("a stop on a network Vela ships has a report");
    let title = report.what.lines().next().unwrap_or_default();
    assert_eq!(title, "Relayer out of gas on Unichain (130)");
    assert!(title.chars().count() <= 80, "{title:?}");
    assert!(
        report.what.contains(&format!("Treasury: {RELAY_TREASURY}")),
        "the full address: {}",
        report.what
    );
    assert!(
        report
            .what
            .contains("Has 0 ETH of its 0.0001 ETH floor (short 0.0001 ETH)."),
        "have and floor, in the coin: {}",
        report.what
    );
    assert_eq!(report.area, "Send");
    assert_eq!(report.fingerprint, "relay-gas-130");
    assert!(
        report
            .steps
            .starts_with("1. Send on Unichain (130)\n2. Continue:"),
        "{}",
        report.steps
    );

    // The watch re-reads a balance still short: the figures follow, the key
    // does not.
    elapse_the_watch(&mut sut);
    assert_only_the_watch(
        &answer_the_watch(
            &mut sut,
            short_treasury(
                UNICHAIN,
                SendTreasuryAsset::Native,
                "40000000000000",
                RELAY_NATIVE_FLOOR,
            ),
        ),
        "still short: waits again",
    );
    let refreshed = sut.view().relay_report.expect("still up");
    assert!(
        refreshed
            .what
            .contains("Has 0.00004 ETH of its 0.0001 ETH floor (short 0.00006 ETH)."),
        "{}",
        refreshed.what
    );
    assert_eq!(
        refreshed.fingerprint, "relay-gas-130",
        "one outage, one issue"
    );

    // The stop goes, and the report with it.
    sut.dispatch(Event::DismissTreasurySheet);
    assert!(sut.view().relay_report.is_none());
}

/// Figures the core cannot put in a coin are still filed — as the relay's
/// own base units, with their unit — never left out.
#[test]
fn a_report_whose_figures_do_not_read_files_them_raw_466() {
    let mut sut = unichain_to_the_treasury_answer();
    sut.resolve(short_treasury(
        UNICHAIN,
        SendTreasuryAsset::Native,
        "12.5",
        RELAY_NATIVE_FLOOR,
    ));
    let report = sut.view().relay_report.expect("the stop is up");
    assert!(
        report
            .what
            .contains("Has 12.5 wei of its 100000000000000 wei floor."),
        "{}",
        report.what
    );
}

/// The can't-reach stop has its own report under its own key — and a stop
/// raised by the pre-sign recheck says it was the confirm.
#[test]
fn the_cant_reach_stop_carries_its_own_report_466() {
    let mut sut = unichain_to_the_treasury_answer();
    sut.resolve(uncovered());
    let report = sut.view().relay_report.expect("the can't-reach stop");
    assert_eq!(
        report.what.lines().next(),
        Some("Relay can't reach Unichain (130)")
    );
    assert_eq!(report.area, "Send");
    assert_eq!(report.fingerprint, "relay-unreachable-130");

    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    sut.dispatch(Event::SlideConfirm);
    sut.resolve(uncovered());
    let report = sut.view().relay_report.expect("the can't-reach stop");
    assert_eq!(
        report.what.lines().next(),
        Some("Relay can't reach Ethereum (1)")
    );
    assert_eq!(report.fingerprint, "relay-unreachable-1");
    assert!(
        report.steps.contains("\n2. Confirm:"),
        "the pre-sign recheck stopped it: {}",
        report.steps
    );
}

/// A network the person added has no operator to tell: neither stop carries
/// a report there (the same predicate as `operator_served`).
#[test]
fn a_stop_on_a_network_someone_added_files_no_report_466() {
    let mut sut = boot(vec![devnet_coin("3")]);
    continue_with(&mut sut, &devnet_coin("3"), "1");
    sut.resolve(empty_relayer(7_777_001));
    let view = sut.view();
    assert!(view.treasury_bootstrap.is_some(), "the stop is up");
    assert!(view.relay_report.is_none());

    let mut sut = boot(vec![devnet_coin("3")]);
    continue_with(&mut sut, &devnet_coin("3"), "1");
    sut.resolve(uncovered());
    let view = sut.view();
    assert!(view.relay_unreachable.is_some(), "the stop is up");
    assert!(view.relay_report.is_none());
}

/// The confirm page's gate is one predicate too. A relay stop opened by the
/// pre-sign recheck closes it in the core — the shells used to AND that on by
/// themselves, and not all of them did — and the slide refuses on the same
/// predicate. The stop's own retry lowers the stop and slides.
#[test]
fn on_confirm_a_relay_stop_closes_the_confirm_gate_and_the_slide_refuses() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    assert!(sut.view().can_confirm);
    sut.dispatch(Event::SlideConfirm);
    assert_only_the_watch(
        &sut.resolve(low_float()),
        "the funding stop opens on confirm",
    );
    let view = sut.view();
    assert!(view.treasury_bootstrap.is_some());
    assert_eq!(view.tx_status, SendTxStatus::Idle);
    assert!(!view.sending);
    assert!(!view.can_confirm, "the stop is up: the slide is shut");
    assert!(
        sut.dispatch(Event::SlideConfirm).is_empty(),
        "a slide from a stale frame asks for nothing"
    );
    let ops = sut.dispatch(Event::RetryAfterBootstrap);
    assert!(
        matches!(ops.as_slice(), [Op::ProbeTreasury { chain_id: 1 }]),
        "the stop's retry slides: {ops:?}"
    );
}

#[test]
fn on_confirm_the_cant_reach_stop_closes_the_confirm_gate_and_the_slide_refuses() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    sut.dispatch(Event::SlideConfirm);
    assert!(sut.resolve(uncovered()).is_empty());
    assert!(!sut.view().can_confirm);
    assert!(sut.dispatch(Event::SlideConfirm).is_empty());
    let ops = sut.dispatch(Event::RetryRelayUnreachable);
    assert!(
        matches!(ops.as_slice(), [Op::ProbeTreasury { chain_id: 1 }]),
        "{ops:?}"
    );
}

/// A refused submit shuts the slide until the error panel's retry — the
/// published gate, and the slide's own.
#[test]
fn a_refused_submit_shuts_the_slide_until_its_retry() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.resolve(Res::SubmitFailed {
        failure: SendSubmitFailure::Other { message: None },
    });
    assert_eq!(sut.view().tx_error, Some(SendTxErrorKey::Generic));
    assert!(!sut.view().can_confirm);
    assert!(sut.dispatch(Event::SlideConfirm).is_empty());
    sut.dispatch(Event::RetryAfterError);
    assert!(sut.view().can_confirm);
}

// ===========================================================================
// One transaction in flight per account and network: wait for the first
// ===========================================================================

fn in_flight(sender: &str, chain_id: u32, hash: &str) -> vela_core::app::tx_tracker::InFlightOp {
    vela_core::app::tx_tracker::InFlightOp {
        sender: sender.to_owned(),
        chain_id,
        user_op_hash: hash.to_owned(),
    }
}

/// A second send while the account's first on this chain is still going
/// through would take the same nonce — so the confirm is held, says why, and
/// opens by itself once the first is final. A stale slide asks for nothing.
#[test]
fn the_confirm_waits_for_the_account_s_previous_transaction() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    assert!(sut.view().can_confirm);

    sut.dispatch(Event::InFlightOps {
        ops: vec![in_flight(ACCOUNT, 1, "0xfirst")],
    });
    let view = sut.view();
    assert!(!view.can_confirm, "held while the first is in flight");
    let held = view.previous_pending.expect("the hold says why");
    assert_eq!(held.chain_id, 1);
    assert_eq!(held.user_op_hash, "0xfirst");
    assert_eq!(
        held.key,
        "componentsUi.signing.confirmBlock.previousPending"
    );
    assert!(
        sut.dispatch(Event::SlideConfirm).is_empty(),
        "a slide from a stale frame signs nothing"
    );

    // The first is final: the tracker no longer lists it.
    sut.dispatch(Event::InFlightOps { ops: vec![] });
    let view = sut.view();
    assert!(view.can_confirm, "opens by itself");
    assert_eq!(view.previous_pending, None);
    assert!(matches!(slide_to_submit(&mut sut), Op::SubmitUserOp { .. }));
}

/// While held, the line is one line and stays put: a fee re-measure, the
/// tracker's list sent again unchanged, its end — the hold says the same
/// sentence about the same transaction until the list lets it go (final, or
/// ten minutes with no progress: `tx_tracker::IN_FLIGHT_STALL_MS`).
#[test]
fn the_held_line_stays_one_line_until_the_hold_is_released() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    let ops = vec![in_flight(ACCOUNT, 1, "0xfirst")];
    sut.dispatch(Event::InFlightOps { ops: ops.clone() });
    let held = sut.view().previous_pending.expect("held");

    sut.dispatch(Event::FeeBusyChanged { busy: true });
    assert_eq!(
        sut.view().previous_pending.as_ref(),
        Some(&held),
        "re-measuring"
    );
    sut.dispatch(Event::FeeBusyChanged { busy: false });
    assert_eq!(sut.view().previous_pending.as_ref(), Some(&held));
    sut.dispatch(Event::InFlightOps { ops });
    assert_eq!(
        sut.view().previous_pending.as_ref(),
        Some(&held),
        "the same list again"
    );
    assert!(!sut.view().can_confirm);

    // The tracker released it (final, or stalled): the list no longer has it.
    sut.dispatch(Event::InFlightOps { ops: vec![] });
    assert_eq!(sut.view().previous_pending, None);
    assert!(sut.view().can_confirm);
}

/// Only the same account on the same chain waits: another chain, another
/// account (any casing aside) or this journey's own op hold nothing.
#[test]
fn only_the_same_account_on_the_same_chain_waits() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    sut.dispatch(Event::InFlightOps {
        ops: vec![
            in_flight(ACCOUNT, 8453, "0xbase"),
            in_flight("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", 1, "0xother"),
        ],
    });
    assert!(sut.view().can_confirm);
    sut.dispatch(Event::InFlightOps {
        ops: vec![in_flight(
            &ACCOUNT.to_uppercase().replace("0X", "0x"),
            1,
            "0xfirst",
        )],
    });
    assert!(
        !sut.view().can_confirm,
        "the address is matched in any case"
    );
}

/// The relay refused the submit because another transaction of the account
/// holds the nonce (another device, one this device could not follow):
/// nothing was sent, and it says so — not the generic failure.
#[test]
fn a_held_nonce_refusal_says_the_previous_transaction_is_pending() {
    let mut sut = boot(vec![eth("2")]);
    to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
    slide_to_submit(&mut sut);
    sut.resolve(Res::SubmitFailed {
        failure: SendSubmitFailure::PreviousPending,
    });
    let view = sut.view();
    assert_eq!(view.tx_status, SendTxStatus::Error);
    assert_eq!(view.tx_error, Some(SendTxErrorKey::PreviousPending));
}

/// A refusal is told by its reason: the fee words only when fees were the
/// reason (they were every refusal's words before — a spent nonce read as
/// "network fees stayed above the amount you approved").
#[test]
fn a_refused_send_is_told_by_its_reason() {
    use vela_core::app::tx_tracker::RefusalReason;
    for (refusal, key, fee_words) in [
        (
            Some(RefusalReason::FeeBelowMarket),
            "send.txRejectedFees",
            true,
        ),
        (
            Some(RefusalReason::NonceUsed),
            "componentsUi.signing.wentFirst",
            false,
        ),
        (
            Some(RefusalReason::SimulationFailed),
            "componentsUi.signing.refused",
            false,
        ),
        (None, "componentsUi.signing.refused", false),
    ] {
        let mut sut = boot(vec![eth("2")]);
        to_confirm_native(&mut sut, "1", native_fee(1, 1_000));
        slide_to_submit(&mut sut);
        sut.resolve(submitted(HASH));
        settle_persistence(&mut sut);
        sut.resolve(Res::TrackHandedOff);
        sut.dispatch(Event::ReceiptUpdate {
            user_op_hash: HASH.to_owned(),
            outcome: SendReceiptOutcome::Failed {
                rejected: true,
                not_sent: false,
                refusal,
            },
        });
        let receipt = sut.view().receipt.expect("receipt");
        assert_eq!(receipt.status, SendReceiptStatus::Failed, "{refusal:?}");
        assert_eq!(receipt.refusal_key.as_deref(), Some(key), "{refusal:?}");
        assert_eq!(
            receipt.hold_reason == Some(SendHoldReason::FeeRejected),
            fee_words,
            "{refusal:?}"
        );
    }
}

/// The tracker's refusal reaches the receipt outcome.
#[test]
fn the_tracker_s_reason_rides_the_receipt_outcome() {
    use vela_core::app::tx_tracker::{RefusalReason, TrackOutcome, TrackStatus};
    let mut entry = track_entry(TrackStatus::Rejected, TrackOutcome::Final, None);
    entry.refusal = Some(RefusalReason::NonceUsed);
    assert_eq!(
        vela_core::app::send::receipt_outcome_of(&entry),
        Some(SendReceiptOutcome::Failed {
            rejected: true,
            not_sent: false,
            refusal: Some(RefusalReason::NonceUsed),
        })
    );
}
