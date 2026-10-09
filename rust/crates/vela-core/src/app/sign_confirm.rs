//! Pure policy — may the signing confirm be tapped, and if not, why not
//! (spec 099 R7).
//!
//! Four machines have a say: the request ([`super::sign_request`], its own
//! gate and why it is shut), the approval guard, the clear-signing reading and
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

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::approval_guard::{GuardSurface, GuardView};
use super::clear_signing::{ClearSignType, ClearSigningView, ClearSurface};
use super::fee_policy::{FeeTier, FeeView};
use super::sign_request::SignView;

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
    /// first is final. The send confirm says the same line.
    PreviousPending,
    /// The request is still being read.
    Reading,
    /// An approval needs its amount chosen.
    ApprovalChoice,
    /// A batch's calls are still being checked.
    BatchUnsettled,
    /// The fee is being worked out (or re-worked for another speed).
    FeeMeasuring,
    /// The fee could not be worked out: retry.
    FeeFailed,
    /// The coin chosen for the fee is short. No line under the confirm: the
    /// fee section says it already, where the other coins are — "Insufficient
    /// ETH for gas fees", or, when no coin on offer can pay
    /// ([`FeeView::no_coin_pays`], issue #408), "No token can pay this fee".
    /// Every client draws that sentence whenever this block holds (the coin in
    /// force is then provably `insufficient`), so a second copy under the confirm
    /// only repeated it (issue #438).
    FeeShort,
}

/// The line while the account's previous transaction on this network is
/// still going through — under a held confirm, the dApp sheet's and Send's.
pub const PREVIOUS_PENDING_KEY: &str = "componentsUi.signing.confirmBlock.previousPending";

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

/// The one gate (module doc).
#[must_use]
pub fn confirm_state(input: &ConfirmInput) -> ConfirmState {
    confirm_state_of(
        &input.sign,
        &input.guard,
        &input.clear,
        input.fee.as_ref(),
        input.speed_tier,
    )
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
        if fee.busy || fee_of_another_tier(fee, speed_tier) || fee_of_another_coin(fee) {
            return shut(ConfirmBlock::FeeMeasuring);
        }
        if fee.failed.is_some() {
            return shut(ConfirmBlock::FeeFailed);
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
    ConfirmState {
        enabled: true,
        block: None,
        key: None,
    }
}

/// [`confirm_state`] over the four views as the shell last received them, in
/// JSON (UniFFI, wasm), and the speed in force as its wire name. `None` when a
/// view does not read — the shell keeps the confirm disabled.
#[must_use]
pub fn confirm_state_json(
    sign_json: &str,
    guard_json: &str,
    clear_json: &str,
    fee_json: Option<&str>,
    speed_tier: Option<&str>,
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
