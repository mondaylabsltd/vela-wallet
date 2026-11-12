//! Pure policy — may the signing confirm be tapped, and if not, why not
//! (spec 099 R7).
//!
//! Four machines have a say: the request ([`super::sign_request`], its own
//! gate and why it is shut — and, since PR 3, whether its simulation's verdict
//! is on the sheet yet), the approval guard, the clear-signing reading and
//! the fee. Every client used to AND them itself, each with rules of its own
//! on top — a message has no fee to wait for, another speed's figure is not
//! this speed's (issue 681), a request not yet read is not signable (096 F7) —
//! and the copies drifted: iOS had two, one without the last two rules. And a
//! disabled confirm said nothing: the person could not tell a held failure from a
//! fee being measured.
//!
//! [`confirm_state`] is the one gate. It names the first part that is shut,
//! in the order a person would fix them, and every client draws the
//! [`ConfirmBlock`]'s line (`componentsUi.signing.confirmBlock.*`) under the
//! disabled confirm, with the action that opens it — unless the sheet already says
//! it: a short fee coin is said under the fee, where the other coins are
//! (issue #438: "No token can pay this fee" twice, red over the fee and grey
//! under the confirm).
//!
//! ## How hard the confirm is (R22, review 1-3)
//!
//! [`posture`] is the one rule for the confirm's friction, and the gate
//! carries it ([`ConfirmState::anyway`], [`ConfirmState::danger`],
//! [`ConfirmState::hold_ms`]):
//!
//! - **"… anyway"** whenever anything Vela checks could not be read: a call
//!   or message nobody reads (the blind surfaces, a batch with one such leg),
//!   and — for a transaction — a simulation that did not come back, that a
//!   node did not offer, that nobody independent confirmed, or that says it
//!   fails. A revert no longer signs with a plain press.
//! - **Hold to confirm** ([`HOLD_TO_CONFIRM_MS`]) for every "… anyway" and every
//!   danger. A shell whose person uses assistive technology (a screen
//!   reader, switch control, the keyboard) offers the accessible equivalent:
//!   the first activation arms the confirm and says
//!   [`CONFIRM_AGAIN_KEY`]; a second one within [`CONFIRM_AGAIN_WINDOW_MS`]
//!   confirms (WCAG 2.5.1 / 2.5.7: no path-based or held gesture is the only
//!   way).
//! - **"Refuse blind signing"** (the person's preference,
//!   `prefs::keys::REFUSE_BLIND_SIGNING`): every "… anyway" is refused
//!   instead ([`ConfirmBlock::BlindRefused`]).

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::approval_guard::{GuardSurface, GuardView};
use super::clear_signing::{ClearRisk, ClearSignType, ClearSigningView, ClearSurface};
use super::fee_policy::{FeeTier, FeeView};
use super::sign_request::SignView;
use super::sim_outcome::SimVerdict;

/// Which part of the gate is shut.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ConfirmBlock {
    /// No request on the sheet.
    NoRequest,
    /// Refused by the wallet (spec 081): only Dismiss.
    Refused,
    /// Signing or submitting now: the sheet says which.
    InFlight,
    /// The gas-account funding sheet is up.
    Funding,
    /// The site's account is being switched to (§12.1.6).
    AccountSwitching,
    /// This request was answered, or its failure is held: Try again (when
    /// `SignView.failure_retryable`) or close.
    Answered,
    /// The account's previous transaction on this network is still going
    /// through ([`super::tx_tracker::InFlightOp`]): this one would take the
    /// same nonce, so it waits, and the confirm opens by itself once the
    /// first is final or has made no progress for ten minutes
    /// ([`super::tx_tracker::IN_FLIGHT_STALL_MS`]). One plain line for as long
    /// as it holds — it sits ahead of every fee block, so a re-quote never
    /// swaps it out. The send confirm says the same line.
    PreviousPending,
    /// The request is still being read.
    Reading,
    /// An approval needs its amount chosen.
    ApprovalChoice,
    /// A batch's calls are still being checked.
    BatchUnsettled,
    /// The fee is being worked out (or re-worked for another speed).
    FeeMeasuring,
    /// The fee could not be worked out. Its line is the fee view's own
    /// ([`super::fee_policy::FeeFailureView::footer_key`]): "Retrying…" while
    /// the machine asks again by itself — through the re-ask too — and "Tap
    /// it to retry" only when a tap is the one way.
    FeeFailed,
    /// The coin chosen for the fee is short. No line under the confirm: the
    /// fee section says it already, where the other coins are — "Insufficient
    /// ETH for gas fees", or, when no coin on offer can pay
    /// ([`FeeView::no_coin_pays`], issue #408), "No token can pay this fee".
    /// Every client draws that sentence whenever this block holds (the coin in
    /// force is then provably `insufficient`), so a second copy under the confirm
    /// only repeated it (issue #438).
    FeeShort,
    /// Everything else is ready and the wallet's own simulation has not
    /// given its verdict yet (PR 3, [`SignView::sim_checking`]). The balance
    /// changes are the one part of the sheet the site being signed for
    /// cannot write, so the confirm waits for them — at most
    /// [`super::sign_request::SIM_VERDICT_WAIT_MS`], after which the sheet
    /// says it could not check and the confirm opens. Last in the order: a
    /// request still being read, an amount to choose or a fee being worked
    /// out says its own line first, and this one stands only when the
    /// verdict is all that is missing.
    SimChecking,
    /// A message is drawn taller than the sheet's window and its end has not
    /// been in sight yet (cs-10): "The message continues — scroll to read
    /// all of it". The shell observes the scroll and says so
    /// ([`ConfirmInput::message_unread`]); the gate is this. Asked only on
    /// [`ClearSurface::MessageSign`], and only once everything else is open.
    MessageUnread,
    /// R22: the request signs only "… anyway" and the person turned on
    /// "Refuse blind signing": refused, with the line that says why and where
    /// the preference is ([`BLIND_REFUSED_KEY`]). Asked last.
    BlindRefused,
}

/// The line while the account's previous transaction on this network is
/// still going through — under a held confirm, the dApp sheet's and Send's.
pub const PREVIOUS_PENDING_KEY: &str = "componentsUi.signing.confirmBlock.previousPending";

/// The line under a confirm that waits for the simulation's verdict (PR 3,
/// [`ConfirmBlock::SimChecking`]): "Checking what this transaction does…" —
/// the present tense of the sentence the verdict's place says when nothing
/// could be checked (`simUnavailableWarning`).
pub const SIM_CHECKING_KEY: &str = "componentsUi.signing.confirmBlock.simChecking";

/// The line under a confirm held until a message has been read to its end
/// ([`ConfirmBlock::MessageUnread`]).
pub const MESSAGE_UNREAD_KEY: &str = "componentsUi.signing.confirmBlock.messageUnread";

/// R22: the line under a confirm the "Refuse blind signing" preference shut
/// ([`ConfirmBlock::BlindRefused`]).
pub const BLIND_REFUSED_KEY: &str = "componentsUi.signing.confirmBlock.blindRefused";

/// R22: how long a "… anyway" or a danger confirm is held before it acts, in
/// ms. 1.5 s: long enough that a tap, a double tap or a press made while the
/// sheet is still arriving cannot sign (a deliberate long press, which the
/// platforms put at 0.5 s, is not enough either), short enough not to read as
/// broken — the range hold-to-confirm controls use (1–2 s).
pub const HOLD_TO_CONFIRM_MS: u32 = 1_500;

/// R22: the accessible equivalent of the hold — a second explicit activation
/// within this window confirms; past it the confirm disarms.
pub const CONFIRM_AGAIN_WINDOW_MS: u32 = 5_000;

/// "Hold to confirm" — said on and beside a held confirm (and to a screen
/// reader as its hint).
pub const HOLD_KEY: &str = "componentsUi.signing.holdToConfirm";

/// "Press again to confirm" — the armed state of the accessible equivalent.
pub const CONFIRM_AGAIN_KEY: &str = "componentsUi.signing.confirmAgain";

/// The title when the relay would not take an operation because the
/// account's previous one on this network still holds the nonce, at submit
/// (`RelayRejection::NonceHeld`): "Not sent yet". Nothing failed and nothing
/// was sent, so it is said calmly — never "Failed", never in the failure's
/// colour — on the signing sheet ([`super::sign_request::SignView::failure_not_sent`])
/// and on Send's confirm (`SendTxErrorKey::PreviousPending`), with Try again.
pub const NOT_SENT_TITLE_KEY: &str = "componentsUi.signing.notSentTitle";

/// The sentence under [`NOT_SENT_TITLE_KEY`]: "Your previous transaction on
/// this network is still being processed. Try again once it's done." The
/// held confirm's own line ([`PREVIOUS_PENDING_KEY`], "Waiting for your last
/// transaction…") is a wait in progress; this one follows a submit that was
/// turned back, and says what to do.
pub const NOT_SENT_BODY_KEY: &str = "componentsUi.signing.notSentBody";

/// What the gate decides from: the four views as the shell last had them,
/// and the speed in force (`None` — no speed control).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfirmInput {
    pub sign: SignView,
    pub guard: GuardView,
    pub clear: ClearSigningView,
    /// `None` — no fee session (a message).
    pub fee: Option<FeeView>,
    #[serde(default)]
    pub speed_tier: Option<FeeTier>,
    /// The shell's observation (cs-10): the message on the sheet runs past
    /// the sheet's window and its last line has not been in sight yet. A
    /// shell that does not observe it leaves it `false`, and nothing is held.
    #[serde(default)]
    pub message_unread: bool,
    /// R22: the sheet's own simulation verdict as the shell holds it
    /// (`sim_outcome::verdict_of_envelope`); `None` before it lands or for a
    /// message. A transaction without one is unread.
    #[serde(default)]
    pub sim: Option<SimVerdict>,
    /// R22: the person's "Refuse blind signing" preference.
    #[serde(default)]
    pub refuse_blind: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ConfirmState {
    pub enabled: bool,
    /// Why not, when not.
    pub block: Option<ConfirmBlock>,
    /// The line under the disabled confirm (`componentsUi.signing.confirmBlock.*`),
    /// or `None` where the sheet already says it in its own way (no request,
    /// signing in progress, the funding sheet, a refusal's own panel, a short
    /// fee coin's line under the fee).
    pub key: Option<String>,
    /// R22: the confirm is "… anyway" ("Sign anyway", in the danger style):
    /// something Vela checks could not be read ([`posture`]).
    #[serde(default)]
    pub anyway: bool,
    /// R22: the request is a danger (a drain's shape, an unlimited grant, a
    /// revert, `eth_sign`): drawn in the danger style.
    #[serde(default)]
    pub danger: bool,
    /// R22: hold the confirm this long before it acts ([`HOLD_TO_CONFIRM_MS`];
    /// `0`: a plain press), or confirm twice through the accessible path.
    #[serde(default)]
    pub hold_ms: u32,
}

/// R22: how hard the confirm is ([`posture`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct Posture {
    pub anyway: bool,
    pub danger: bool,
    pub hold_ms: u32,
}

/// R22 (review 1-3): the confirm's friction (module doc, "How hard the
/// confirm is"). Pure; the same on every client.
#[must_use]
pub fn posture(sign: &SignView, clear: &ClearSigningView, sim: Option<&SimVerdict>) -> Posture {
    let transaction = !off_chain(clear) && clear.surface != ClearSurface::None;
    // A transaction's own simulation: anything but a checked answer (the
    // verdict's `deltas`) — not yet back past its wait, not offered, nobody
    // independent confirmed it, it fails — is a check Vela could not read.
    // While its verdict is still out (`sim_checking`, the confirm is shut
    // "Checking…") nothing is judged yet: the confirm does not read
    // "… anyway" over a check that may come back plain.
    let sim_unread = transaction
        && !sign.sim_checking
        && (sign.sim_waited_out_key.is_some()
            || sim.is_none_or(|verdict| verdict.kind != "deltas"));
    let reverts = transaction && sim.is_some_and(|verdict| verdict.kind == "reverts");
    let anyway = clear.sign_anyway || sim_unread;
    let reading_danger = clear
        .result
        .as_ref()
        .is_some_and(|result| result.risk == ClearRisk::Danger)
        || clear
            .batch
            .as_ref()
            .is_some_and(|batch| batch.risk == ClearRisk::Danger);
    let danger =
        reverts || reading_danger || clear.danger_haptic || clear.surface == ClearSurface::EthSign;
    Posture {
        anyway,
        danger,
        hold_ms: if anyway || danger {
            HOLD_TO_CONFIRM_MS
        } else {
            0
        },
    }
}

impl ConfirmBlock {
    /// The line for this block; `retryable` picks "try again" over "close"
    /// for a held failure (`SignView::failure_retryable`).
    #[must_use]
    pub fn key(self, retryable: bool) -> Option<&'static str> {
        Some(match self {
            Self::NoRequest | Self::Refused | Self::InFlight | Self::Funding | Self::FeeShort => {
                return None;
            }
            Self::AccountSwitching => "componentsUi.signing.confirmBlock.accountSwitching",
            Self::Answered if retryable => "componentsUi.signing.confirmBlock.answeredRetry",
            Self::Answered => "componentsUi.signing.confirmBlock.answered",
            Self::PreviousPending => PREVIOUS_PENDING_KEY,
            Self::Reading => "componentsUi.signing.confirmBlock.reading",
            Self::ApprovalChoice => "componentsUi.signing.confirmBlock.approvalChoice",
            Self::BatchUnsettled => "componentsUi.signing.confirmBlock.batchUnsettled",
            Self::FeeMeasuring => "componentsUi.signing.confirmBlock.feeMeasuring",
            Self::FeeFailed => "componentsUi.signing.confirmBlock.feeFailed",
            Self::SimChecking => SIM_CHECKING_KEY,
            Self::MessageUnread => MESSAGE_UNREAD_KEY,
            Self::BlindRefused => BLIND_REFUSED_KEY,
        })
    }
}

/// The request is a signature, not a transaction: no fee to wait for.
#[must_use]
pub fn off_chain(clear: &ClearSigningView) -> bool {
    clear
        .result
        .as_ref()
        .is_some_and(|result| result.sign_type == ClearSignType::Signature)
        || matches!(
            clear.surface,
            ClearSurface::MessageSign | ClearSurface::EthSign | ClearSurface::BlindTypedData
        )
}

/// The figure on the sheet is another speed's (issue 681): between a tap and
/// that speed's own figure landing, `confirm_fee_ready` is still true on the
/// speed just left.
#[must_use]
pub fn fee_of_another_tier(fee: &FeeView, speed_tier: Option<FeeTier>) -> bool {
    let (Some(estimate), Some(tier)) = (&fee.fee, speed_tier) else {
        return false;
    };
    super::fee_speed::offered(estimate.tier) != super::fee_speed::offered(tier)
}

/// The figure on the sheet is another coin's gas (the fee-coin switch): the
/// coin in force changed after the gas was measured with the other coin's fee
/// leg, and the machine is measuring it again (`FeeView::provisional`). The
/// fee machine already holds `confirm_fee_ready` meanwhile; this is the gate's
/// own second line, as [`fee_of_another_tier`] is for a speed.
#[must_use]
pub fn fee_of_another_coin(fee: &FeeView) -> bool {
    fee.provisional
}

/// The one gate (module doc), with its friction ([`posture`]) and the
/// "Refuse blind signing" refusal, asked last.
#[must_use]
pub fn confirm_state(input: &ConfirmInput) -> ConfirmState {
    let state = confirm_state_of(
        &input.sign,
        &input.guard,
        &input.clear,
        input.fee.as_ref(),
        input.speed_tier,
    );
    let state = message_read_gate(state, &input.clear, input.message_unread);
    with_posture(
        state,
        guarded_posture(
            posture(&input.sign, &input.clear, input.sim.as_ref()),
            &input.guard,
        ),
        input.refuse_blind,
    )
}

/// §39 (R22 follow-up, an R22 observation against §37): an unbounded grant
/// is a danger however it arrives — a top-level `approve`, a batch leg, one
/// carried inside `multicall` / Multicall3 / `multiSend`, or an off-chain
/// permit (EIP-2612, Permit2, its witness transfer). The reading's own risk
/// grades only what a descriptor reads, so a Multicall3-carried approval or
/// a witness permit was a plain Confirm while a two-leg batch was held. The
/// approval guard's one verdict (`GuardView::unlimited_warning`, the same
/// red sentence every shell draws) now makes the confirm a held danger.
#[must_use]
pub fn guarded_posture(posture: Posture, guard: &GuardView) -> Posture {
    if !guard.unlimited_warning {
        return posture;
    }
    Posture {
        danger: true,
        hold_ms: HOLD_TO_CONFIRM_MS,
        ..posture
    }
}

/// R22: the gate's state with its friction, and — the person refusing blind
/// signing — every "… anyway" refused, once everything else is open.
#[must_use]
pub fn with_posture(state: ConfirmState, posture: Posture, refuse_blind: bool) -> ConfirmState {
    let state = ConfirmState {
        anyway: posture.anyway,
        danger: posture.danger,
        hold_ms: posture.hold_ms,
        ..state
    };
    if !state.enabled || !refuse_blind || !posture.anyway {
        return state;
    }
    ConfirmState {
        enabled: false,
        block: Some(ConfirmBlock::BlindRefused),
        key: Some(BLIND_REFUSED_KEY.to_owned()),
        ..state
    }
}

/// cs-10: a message is signed only once the person could have read all of
/// it. Asked last — any other line names its own reason first — and only of
/// a message, whatever a shell reports for another surface.
#[must_use]
pub fn message_read_gate(
    state: ConfirmState,
    clear: &ClearSigningView,
    message_unread: bool,
) -> ConfirmState {
    if !state.enabled || !message_unread || clear.surface != ClearSurface::MessageSign {
        return state;
    }
    ConfirmState {
        enabled: false,
        block: Some(ConfirmBlock::MessageUnread),
        key: ConfirmBlock::MessageUnread.key(false).map(str::to_owned),
        ..state
    }
}

/// [`confirm_state`] over borrowed views — for a shell that links the core
/// and holds them already.
#[must_use]
pub fn confirm_state_of(
    sign: &SignView,
    guard: &GuardView,
    clear: &ClearSigningView,
    fee: Option<&FeeView>,
    speed_tier: Option<FeeTier>,
) -> ConfirmState {
    let shut = |block: ConfirmBlock| ConfirmState {
        enabled: false,
        block: Some(block),
        key: block.key(sign.failure_retryable).map(str::to_owned),
        ..ConfirmState::default()
    };
    if !sign.confirm_gate_open {
        return shut(sign.confirm_block.unwrap_or(ConfirmBlock::NoRequest));
    }
    if sign.is_signing || sign.is_submitting {
        return shut(ConfirmBlock::InFlight);
    }
    // Spec 096 F7: a confirm that was live under "Loading…" signed what nobody had
    // been shown yet.
    if clear.resolving || clear.surface == ClearSurface::Loading {
        return shut(ConfirmBlock::Reading);
    }
    if !guard.confirm_allowed {
        return shut(if guard.surface == GuardSurface::Batch {
            ConfirmBlock::BatchUnsettled
        } else {
            ConfirmBlock::ApprovalChoice
        });
    }
    if !off_chain(clear) {
        let Some(fee) = fee else {
            return shut(ConfirmBlock::FeeMeasuring);
        };
        // The failure says the same thing here as on the row (PR 2 note 1):
        // "retrying" while the machine asks again by itself — through the
        // re-ask too, so the line does not flip to "working out" and back
        // every few seconds — and "tap it" only when a tap is the one way.
        if let Some(failure) = fee.failure.as_ref() {
            return ConfirmState {
                enabled: false,
                block: Some(ConfirmBlock::FeeFailed),
                key: Some(failure.footer_key.clone()),
                ..ConfirmState::default()
            };
        }
        if fee.busy || fee_of_another_tier(fee, speed_tier) || fee_of_another_coin(fee) {
            return shut(ConfirmBlock::FeeMeasuring);
        }
        if let Some(failed) = fee.failed {
            // A view from a core that predates `failure`.
            return ConfirmState {
                key: Some(super::fee_policy::FeeFailureView::of(failed, false).footer_key),
                ..shut(ConfirmBlock::FeeFailed)
            };
        }
        if !fee.confirm_fee_ready {
            // Priced, not busy, not failed, and still not ready: the coin
            // that pays is short (fee_policy's only other refusal). Not
            // priced yet: still being worked out.
            if fee.fee.is_none() {
                return shut(ConfirmBlock::FeeMeasuring);
            }
            return shut(ConfirmBlock::FeeShort);
        }
    }
    // PR 3: the simulation's verdict is still out. Asked last, and of every
    // request alike — a message never has `sim_checking` set, because no
    // simulation is ever started for one.
    if sign.sim_checking {
        return shut(ConfirmBlock::SimChecking);
    }
    ConfirmState {
        enabled: true,
        block: None,
        key: None,
        ..ConfirmState::default()
    }
}

/// [`confirm_state`] over the four views as the shell last received them, in
/// JSON (UniFFI, wasm), the speed in force as its wire name, and whether the
/// message on the sheet is still unread ([`ConfirmInput::message_unread`]).
/// `None` when a view does not read — the shell keeps the confirm disabled.
#[must_use]
#[allow(clippy::too_many_arguments)]
pub fn confirm_state_json(
    sign_json: &str,
    guard_json: &str,
    clear_json: &str,
    fee_json: Option<&str>,
    speed_tier: Option<&str>,
    message_unread: bool,
    sim_json: Option<&str>,
    refuse_blind: bool,
) -> Option<String> {
    let input = ConfirmInput {
        sign: serde_json::from_str(sign_json).ok()?,
        guard: serde_json::from_str(guard_json).ok()?,
        clear: serde_json::from_str(clear_json).ok()?,
        fee: match fee_json {
            Some(json) => Some(serde_json::from_str(json).ok()?),
            None => None,
        },
        speed_tier: match speed_tier {
            Some(tier) => {
                Some(serde_json::from_value(serde_json::Value::String(tier.to_owned())).ok()?)
            }
            None => None,
        },
        message_unread,
        // R22: a verdict that does not read is no verdict — the transaction
        // is then unread, "… anyway".
        sim: sim_json.and_then(|json| serde_json::from_str(json).ok()),
        refuse_blind,
    };
    serde_json::to_string(&confirm_state(&input)).ok()
}

// ---------------------------------------------------------------------------
// Spec 102 D4 — the hand-off card's fee row
// ---------------------------------------------------------------------------

/// The hand-off card's compact fee + speed row (spec 102, D4; core round 5).
///
/// When an account reviews and signs on a trusted page, the app's sheet gives
/// way to the hand-off card — and the fee is still the app's: it is chosen on
/// the sheet (coin, speed) BEFORE the page opens, because the page signs the
/// operation the app assembled, fee leg included. So the card keeps one quiet
/// row of what was chosen, read from the two views the sheet already drives:
///
/// - the fee is [`FeeView::fee`] of the session in force, drawn exactly as the
///   sheet's folded fee row draws it (the same formatter, the same coin);
/// - the speed is [`super::fee_speed::FeeSpeedView::tier`], named by
///   `send.gasTier.<tier>`.
///
/// No control is drawn on it: once the page is open the operation is fixed,
/// and a different fee is a different operation (close the page, change it on
/// the sheet, open again). And the card's Open IS the sheet's confirm: enabled
/// only when [`confirm_state`] is AND the page's integrity line opens.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct HandoffFee {
    /// The quote in force, settled for [`Self::tier`].
    pub fee: super::fee_policy::FeeEstimateView,
    /// The speed it was priced at and will be submitted at — `None` where
    /// there is nothing to restate: no speed control, or a network with one
    /// speed (`FeeSpeedView::single`).
    pub tier: Option<FeeTier>,
    /// `send.gasTier.<tier>`, when `tier` is set.
    pub tier_key: Option<String>,
}

/// The hand-off card's fee row, from the fee session in force and the speed
/// control (`None` for a surface without one) — or `None`, and the card draws
/// no row: a message has no fee, and a fee that is not settled for the speed
/// in force (measuring, failed, another speed's figure, a coin that is short)
/// is not one the person chose; the confirm gate keeps Open shut until it is.
#[must_use]
pub fn handoff_fee(
    fee: Option<&FeeView>,
    speed: Option<&super::fee_speed::FeeSpeedView>,
) -> Option<HandoffFee> {
    let fee = fee?;
    let speed_tier = speed.map(|speed| speed.tier);
    if fee.busy
        || fee.failed.is_some()
        || !fee.confirm_fee_ready
        || fee_of_another_tier(fee, speed_tier)
        || fee_of_another_coin(fee)
    {
        return None;
    }
    let estimate = fee.fee.clone()?;
    let tier = speed
        .filter(|speed| !speed.single)
        .map(|speed| super::fee_speed::offered(speed.tier));
    Some(HandoffFee {
        fee: estimate,
        tier_key: tier.map(|tier| format!("send.gasTier.{}", tier_name(tier))),
        tier,
    })
}

const fn tier_name(tier: FeeTier) -> &'static str {
    match tier {
        FeeTier::Slow => "slow",
        FeeTier::Standard => "standard",
        FeeTier::Rapid => "rapid",
        FeeTier::Fast => "fast",
    }
}

/// [`handoff_fee`] over the views as the shell last received them, in JSON
/// (UniFFI, wasm): a `HandoffFee` JSON, or `None` — no row (or a view that
/// does not read).
#[must_use]
pub fn handoff_fee_json(fee_json: Option<&str>, speed_json: Option<&str>) -> Option<String> {
    let fee: FeeView = serde_json::from_str(fee_json?).ok()?;
    let speed: Option<super::fee_speed::FeeSpeedView> = match speed_json {
        Some(json) => Some(serde_json::from_str(json).ok()?),
        None => None,
    };
    serde_json::to_string(&handoff_fee(Some(&fee), speed.as_ref())?).ok()
}
