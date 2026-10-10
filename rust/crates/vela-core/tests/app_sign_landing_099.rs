//! Spec 099 R6–R7 — the signing sheet and the landing say what they wait for:
//! the one confirm gate and its reason, the relay's send time, the forgotten
//! op's ending, and the one countdown. One test per rule.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::approval_guard::{ApprovalGuard, GuardSurface};
use vela_core::app::clear_signing::{ClearSigning, ClearSurface};
use vela_core::app::fee_policy::{FeeAssetView, FeeEstimateView, FeeFailure, FeePolicy, FeeTier};
use vela_core::app::sign_confirm::{confirm_state, ConfirmBlock, ConfirmInput};
use vela_core::app::sign_request::SignRequest;
use vela_core::app::tx_tracker::{
    landing_pace, Event, LandingLine, TrackLifecycle, TrackOperation as Op,
    TrackShellResult as Res, TrackStatus, TxTracker, FORGOTTEN_NOT_SENT_MS, WAIT_WINDOW_MS,
};

// ---------------------------------------------------------------------------
// The confirm gate (R7)
// ---------------------------------------------------------------------------

/// The four views as real machines draw them at rest, with this machine's
/// gate opened, a calm guard, an on-chain reading and a ready fee.
fn ready() -> ConfirmInput {
    let mut sign = DomainDriver::<SignRequest>::new().view();
    sign.confirm_gate_open = true;
    sign.confirm_block = None;
    let guard = DomainDriver::<ApprovalGuard>::new().view();
    let mut clear = DomainDriver::<ClearSigning>::new().view();
    clear.resolving = false;
    clear.surface = ClearSurface::BlindTransaction;
    clear.result = None;
    let mut fee = DomainDriver::<FeePolicy>::new().view();
    fee.busy = false;
    fee.failed = None;
    fee.fee = Some(estimate(FeeTier::Fast));
    fee.confirm_fee_ready = true;
    ConfirmInput {
        sign,
        guard,
        clear,
        fee: Some(fee),
        speed_tier: Some(FeeTier::Fast),
    }
}

fn estimate(tier: FeeTier) -> FeeEstimateView {
    FeeEstimateView {
        chain_id: 100,
        total_wei: "1".to_owned(),
        max_fee_per_gas: "1".to_owned(),
        network_fee_per_gas: "1".to_owned(),
        relayer_fee_per_gas: "0".to_owned(),
        bundler_gas_price: "1".to_owned(),
        in_band_gas_basis: "1".to_owned(),
        effective_gas_price: None,
        max_gas_price: None,
        total_gas: "1".to_owned(),
        deployed: true,
        tier,
        quoted: true,
        fee_asset: FeeAssetView::Native,
        fee_recipient: None,
    }
}

fn block(input: &ConfirmInput) -> Option<ConfirmBlock> {
    let state = confirm_state(input);
    assert_eq!(state.enabled, state.block.is_none(), "enabled iff no block");
    state.block
}

#[test]
fn everything_ready_arms_the_slide() {
    assert_eq!(block(&ready()), None);
}

/// The signing machine's own gate names itself — e.g. a held failure.
#[test]
fn the_requests_own_gate_names_why() {
    let at_rest = DomainDriver::<SignRequest>::new().view();
    assert_eq!(at_rest.confirm_block, Some(ConfirmBlock::NoRequest));
    let mut input = ready();
    input.sign.confirm_gate_open = false;
    input.sign.confirm_block = Some(ConfirmBlock::Answered);
    assert_eq!(block(&input), Some(ConfirmBlock::Answered));
}

/// In the order a person would fix them: reading before the guard before
/// the fee.
#[test]
fn reading_then_guard_then_fee() {
    let mut input = ready();
    input.clear.resolving = true;
    input.guard.confirm_allowed = false;
    input.fee.as_mut().unwrap().busy = true;
    assert_eq!(block(&input), Some(ConfirmBlock::Reading));
    input.clear.resolving = false;
    assert_eq!(block(&input), Some(ConfirmBlock::ApprovalChoice));
    input.guard.surface = GuardSurface::Batch;
    assert_eq!(block(&input), Some(ConfirmBlock::BatchUnsettled));
    input.guard.confirm_allowed = true;
    assert_eq!(block(&input), Some(ConfirmBlock::FeeMeasuring));
}

/// The fee: measuring, failed, short — and another speed's figure is still
/// measuring (issue 681).
#[test]
fn the_fee_says_which() {
    let mut input = ready();
    let fee = input.fee.as_mut().unwrap();
    fee.failed = Some(FeeFailure::QuoteUnavailable);
    fee.confirm_fee_ready = false;
    assert_eq!(block(&input), Some(ConfirmBlock::FeeFailed));

    let fee = input.fee.as_mut().unwrap();
    fee.failed = None;
    assert_eq!(
        block(&input),
        Some(ConfirmBlock::FeeShort),
        "priced, not ready"
    );

    // Issue #438: a short coin is said under the fee ("Insufficient ETH for
    // gas fees"; issue #408's "No token can pay this fee" when no coin on
    // offer can), so the slide carries no second copy of it — either way.
    assert_eq!(confirm_state(&input).key, None);
    input.fee.as_mut().unwrap().no_coin_pays = true;
    let state = confirm_state(&input);
    assert!(!state.enabled);
    assert_eq!(state.block, Some(ConfirmBlock::FeeShort));
    assert_eq!(state.key, None);
    input.fee.as_mut().unwrap().no_coin_pays = false;

    let fee = input.fee.as_mut().unwrap();
    fee.fee = None;
    assert_eq!(
        block(&input),
        Some(ConfirmBlock::FeeMeasuring),
        "not priced yet"
    );

    let mut input = ready();
    let other = [FeeTier::Slow, FeeTier::Standard, FeeTier::Rapid]
        .into_iter()
        .find(|tier| {
            vela_core::app::fee_speed::offered(*tier)
                != vela_core::app::fee_speed::offered(FeeTier::Fast)
        })
        .expect("a speed other than Fast is offered");
    input.speed_tier = Some(other);
    assert_eq!(block(&input), Some(ConfirmBlock::FeeMeasuring));
}

/// A message has no fee to wait for — iOS's second gate forgot this.
#[test]
fn a_message_waits_for_no_fee() {
    let mut input = ready();
    input.clear.surface = ClearSurface::MessageSign;
    input.fee = None;
    assert_eq!(block(&input), None);
}

// ---------------------------------------------------------------------------
// The tracker (R6)
// ---------------------------------------------------------------------------

type Tracker = DomainDriver<TxTracker>;

const T0: f64 = 1_754_700_000_000.0;
const HASH: &str = "0xaaaa1111bbbb2222cccc3333dddd4444eeee5555ffff6666aaaa7777bbbb8888";
const CHAIN: u32 = 8453;
const HEAD: u64 = 1_010;

fn submitted() -> Tracker {
    let mut sut = Tracker::new();
    sut.dispatch(Event::Submitted {
        sender: None,
        user_op_hash: HASH.to_owned(),
        record_ids: vec!["rec-1".to_owned()],
        chain_id: CHAIN,
        maybe_sent: false,
        submit_block: Some(1_000),
        admitted: true,
    });
    sut.resolve_matching(|op| matches!(op, Op::Now), Res::Clock { now_ms: T0 });
    sut
}

/// Tick at `now`, answering everything that comes out: no receipt yet, the
/// relay's `status`, and a chain read to `HEAD` with no event.
fn tick(sut: &mut Tracker, now: f64, status: TrackLifecycle, tx_hash: Option<&str>) {
    sut.dispatch(Event::Tick);
    sut.resolve_matching(|op| matches!(op, Op::Now), Res::Clock { now_ms: now });
    drain(sut, now, status, tx_hash);
}

fn drain(sut: &mut Tracker, now: f64, status: TrackLifecycle, tx_hash: Option<&str>) {
    for _ in 0..50 {
        let Some(op) = sut.outstanding().into_iter().next() else {
            return;
        };
        let answer = match op {
            Op::PollReceipt { .. } => Res::ReceiptPending {
                user_op_hash: HASH.to_owned(),
                now_ms: now,
            },
            Op::PollStatus { .. } => Res::Status {
                rejection_reason: None,
                user_op_hash: HASH.to_owned(),
                status,
                stage: None,
                now_ms: now,
                tx_hash: tx_hash.map(str::to_owned),
            },
            Op::FindOpEvent { from_block, .. } => Res::OpEvent {
                user_op_hash: HASH.to_owned(),
                now_ms: now,
                logs_json: from_block.map(|_| "[]".to_owned()),
                error_json: None,
                head_block: Some(HEAD),
            },
            Op::TxReceipt { .. } => Res::TxReceipt {
                user_op_hash: HASH.to_owned(),
                now_ms: now,
                receipt_json: None,
            },
            Op::UpdateTxRecords { .. } => Res::RecordsPatched,
            // A notification: acknowledged.
            _ => Res::Notified,
        };
        sut.resolve(answer);
    }
    panic!("the tracker kept asking");
}

fn entry(sut: &Tracker) -> vela_core::app::tx_tracker::TrackEntryView {
    sut.view().entries.into_iter().next().expect("one entry")
}

/// The relay is asked 3 s in; the send time is when it first says the bundle
/// is on the network — not acceptance.
#[test]
fn the_send_time_is_the_relays_not_the_acceptance() {
    let mut sut = submitted();
    tick(&mut sut, T0 + 3_400.0, TrackLifecycle::Queued, None);
    assert_eq!(entry(&sut).relay_sent_at_ms, None, "queued: not sent yet");
    tick(
        &mut sut,
        T0 + 15_500.0,
        TrackLifecycle::Submitted,
        Some("0xbundle"),
    );
    assert_eq!(entry(&sut).relay_sent_at_ms, Some(T0 + 15_500.0));
    tick(
        &mut sut,
        T0 + 27_600.0,
        TrackLifecycle::Submitted,
        Some("0xbundle"),
    );
    assert_eq!(
        entry(&sut).relay_sent_at_ms,
        Some(T0 + 15_500.0),
        "the first time it was seen"
    );
}

/// Acknowledged, then forgotten (the relay's record expired), with the
/// chain read to its head and no event: "not sent" — after the wait, and
/// not before.
#[test]
fn a_forgotten_op_ends_not_sent_after_the_wait() {
    let mut sut = submitted();
    tick(&mut sut, T0 + 3_400.0, TrackLifecycle::Queued, None);
    let forgot = T0 + WAIT_WINDOW_MS + 80_000.0;
    tick(&mut sut, forgot, TrackLifecycle::NotFound, None);
    tick(&mut sut, forgot + 15_000.0, TrackLifecycle::NotFound, None);
    assert_ne!(
        entry(&sut).status,
        TrackStatus::NotSent,
        "not before the wait"
    );
    tick(
        &mut sut,
        forgot + FORGOTTEN_NOT_SENT_MS + 1_000.0,
        TrackLifecycle::NotFound,
        None,
    );
    assert_eq!(entry(&sut).status, TrackStatus::NotSent);
}

/// Any other answer from the relay in between: it has the op again.
#[test]
fn a_relay_that_remembers_resets_the_wait() {
    let mut sut = submitted();
    tick(&mut sut, T0 + 3_400.0, TrackLifecycle::Queued, None);
    let forgot = T0 + WAIT_WINDOW_MS + 80_000.0;
    tick(&mut sut, forgot, TrackLifecycle::NotFound, None);
    tick(&mut sut, forgot + 15_000.0, TrackLifecycle::Queued, None);
    tick(
        &mut sut,
        forgot + FORGOTTEN_NOT_SENT_MS + 1_000.0,
        TrackLifecycle::NotFound,
        None,
    );
    assert_ne!(entry(&sut).status, TrackStatus::NotSent);
}

/// A relay that named the bundle tx is not "forgotten": that tx decides.
#[test]
fn a_named_bundle_is_never_ended_by_not_found() {
    let mut sut = submitted();
    tick(
        &mut sut,
        T0 + 3_400.0,
        TrackLifecycle::Submitted,
        Some("0xbundle"),
    );
    let forgot = T0 + WAIT_WINDOW_MS + 80_000.0;
    for i in 0..4 {
        tick(
            &mut sut,
            forgot + f64::from(i) * FORGOTTEN_NOT_SENT_MS,
            TrackLifecycle::NotFound,
            None,
        );
    }
    assert_ne!(entry(&sut).status, TrackStatus::NotSent);
}

// ---------------------------------------------------------------------------
// The countdown (R6)
// ---------------------------------------------------------------------------

#[test]
fn nothing_counts_before_the_relay_sends() {
    let pace = landing_pace(None, Some(5), T0);
    assert_eq!(pace.line, LandingLine::Waiting);
    assert_eq!(pace.progress, None, "the ring roams");
    assert_eq!(landing_pace(Some(T0), None, T0).line, LandingLine::None);
}

/// Down inside the usual time, up past it, "longer" past twice it — and the
/// ring's curve as the desktop drew it (≈70% at the usual time).
#[test]
fn the_ladder_and_the_ring() {
    let at = |s: f64| landing_pace(Some(T0), Some(5), T0 + s * 1000.0);
    assert_eq!((at(0.0).line, at(0.0).seconds), (LandingLine::Remaining, 5));
    assert_eq!((at(3.2).line, at(3.2).seconds), (LandingLine::Remaining, 2));
    assert_eq!((at(5.0).line, at(5.0).seconds), (LandingLine::Elapsed, 5));
    assert_eq!((at(9.9).line, at(9.9).seconds), (LandingLine::Elapsed, 9));
    assert_eq!(at(10.0).line, LandingLine::Slow);
    let ring = at(5.0).progress.unwrap();
    assert!((ring - 0.92 * (1.0 - (-1.4f32).exp())).abs() < 1e-6);
    assert!(
        at(1_000.0).progress.unwrap() <= 0.92,
        "only a confirmation closes it"
    );
    assert_eq!(
        at(-3.0).seconds,
        5,
        "a clock behind the send counts from zero"
    );
}

/// The fee-coin switch: a figure switched to another coin is measured again
/// with that coin's leg, and the gate holds it as "measuring" — even a view
/// whose other flags read ready (the gate's own second line), and the hand-off
/// card draws no fee row for it.
#[test]
fn a_figure_switched_to_another_coin_is_still_measuring() {
    let mut input = ready();
    input.fee.as_mut().unwrap().provisional = true;
    assert_eq!(block(&input), Some(ConfirmBlock::FeeMeasuring));
    assert_eq!(
        vela_core::app::sign_confirm::handoff_fee(input.fee.as_ref(), None),
        None
    );
    input.fee.as_mut().unwrap().provisional = false;
    assert_eq!(block(&input), None);
}

/// The account's previous transaction on this network still going through
/// holds the confirm, with its own line.
#[test]
fn a_previous_transaction_in_flight_holds_the_confirm_with_its_line() {
    let mut input = ready();
    input.sign.confirm_gate_open = false;
    input.sign.confirm_block = Some(ConfirmBlock::PreviousPending);
    let state = confirm_state(&input);
    assert!(!state.enabled);
    assert_eq!(state.block, Some(ConfirmBlock::PreviousPending));
    assert_eq!(
        state.key.as_deref(),
        Some("componentsUi.signing.confirmBlock.previousPending")
    );
}

/// The held line is one line: a fee being measured again under it does not
/// swap it for "measuring the fee" and back — the hold sits ahead of every
/// fee block until the tracker lets it go.
#[test]
fn the_previous_transaction_s_line_holds_through_a_re_measure() {
    let mut input = ready();
    input.sign.confirm_gate_open = false;
    input.sign.confirm_block = Some(ConfirmBlock::PreviousPending);
    for busy in [true, false, true] {
        input.fee.as_mut().unwrap().busy = busy;
        let state = confirm_state(&input);
        assert_eq!(
            state.block,
            Some(ConfirmBlock::PreviousPending),
            "busy {busy}"
        );
        assert_eq!(
            state.key.as_deref(),
            Some("componentsUi.signing.confirmBlock.previousPending")
        );
    }
}

/// PR 2 note 1: the line under the confirm says what the fee row says — the
/// core is retrying (never "tap it" then), through the re-ask too, so the
/// line does not flip to "working out the fee" and back; "tap it" is left for
/// a failure only a tap retries.
#[test]
fn the_footer_says_what_the_fee_row_says() {
    use vela_core::app::fee_policy::{FeeFailureView, FEE_FAILED_KEY, FEE_RETRYING_KEY};
    let mut input = ready();
    let fee = input.fee.as_mut().unwrap();
    fee.failed = Some(FeeFailure::QuoteUnavailable);
    fee.confirm_fee_ready = false;
    fee.failure = Some(FeeFailureView::of(FeeFailure::QuoteUnavailable, false));
    let state = confirm_state(&input);
    assert_eq!(state.block, Some(ConfirmBlock::FeeFailed));
    assert_eq!(state.key.as_deref(), Some(FEE_RETRYING_KEY));

    // The re-ask is out: busy, `failed` gone — still the same line.
    let fee = input.fee.as_mut().unwrap();
    fee.failed = None;
    fee.busy = true;
    fee.failure = Some(FeeFailureView::of(FeeFailure::QuoteUnavailable, true));
    let state = confirm_state(&input);
    assert_eq!(state.block, Some(ConfirmBlock::FeeFailed));
    assert_eq!(state.key.as_deref(), Some(FEE_RETRYING_KEY));

    // Only a tap retries it.
    let fee = input.fee.as_mut().unwrap();
    fee.busy = false;
    fee.failed = Some(FeeFailure::MissingPublicKey);
    fee.failure = Some(FeeFailureView::of(FeeFailure::MissingPublicKey, false));
    assert_eq!(confirm_state(&input).key.as_deref(), Some(FEE_FAILED_KEY));
}
