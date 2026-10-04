//! Pure policy — may the signing slide arm, and if not, why not
//! (spec 099 R7).
//!
//! Four machines have a say: the request ([`super::sign_request`], its own
//! gate and why it is shut), the approval guard, the clear-signing reading and
//! the fee. Every client used to AND them itself, each with rules of its own
//! on top — a message has no fee to wait for, another speed's figure is not
//! this speed's (issue 681), a request not yet read is not signable (096 F7) —
//! and the copies drifted: iOS had two, one without the last two rules. And a
//! shut slide said nothing: the person could not tell a held failure from a
//! fee being measured.
//!
//! [`confirm_state`] is the one gate. It names the first part that is shut,
//! in the order a person would fix them, and every client draws the
//! [`ConfirmBlock`]'s line (`componentsUi.signing.confirmBlock.*`) under the
//! shut slide, with the action that opens it.

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
    /// The coin chosen for the fee is short: pick another.
    FeeShort,
}

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
    /// The line under the shut slide (`componentsUi.signing.confirmBlock.*`),
    /// or `None` where the sheet already says it in its own way (no request,
    /// signing in progress, the funding sheet, a refusal's own panel).
    pub key: Option<String>,
}

impl ConfirmBlock {
    /// The line for this block; `retryable` picks "try again" over "close"
    /// for a held failure (`SignView::failure_retryable`).
    #[must_use]
    pub fn key(self, retryable: bool) -> Option<&'static str> {
        Some(match self {
            Self::NoRequest | Self::Refused | Self::InFlight | Self::Funding => return None,
            Self::AccountSwitching => "componentsUi.signing.confirmBlock.accountSwitching",
            Self::Answered if retryable => "componentsUi.signing.confirmBlock.answeredRetry",
            Self::Answered => "componentsUi.signing.confirmBlock.answered",
            Self::Reading => "componentsUi.signing.confirmBlock.reading",
            Self::ApprovalChoice => "componentsUi.signing.confirmBlock.approvalChoice",
            Self::BatchUnsettled => "componentsUi.signing.confirmBlock.batchUnsettled",
            Self::FeeMeasuring => "componentsUi.signing.confirmBlock.feeMeasuring",
            Self::FeeFailed => "componentsUi.signing.confirmBlock.feeFailed",
            Self::FeeShort => "componentsUi.signing.confirmBlock.feeShort",
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
    // Spec 096 F7: a slide that armed under "Loading…" signed what nobody had
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
        if fee.busy || fee_of_another_tier(fee, speed_tier) {
            return shut(ConfirmBlock::FeeMeasuring);
        }
        if fee.failed.is_some() {
            return shut(ConfirmBlock::FeeFailed);
        }
        if !fee.confirm_fee_ready {
            // Priced, not busy, not failed, and still not ready: the coin
            // that pays is short (fee_policy's only other refusal). Not
            // priced yet: still being worked out.
            return shut(if fee.fee.is_some() {
                ConfirmBlock::FeeShort
            } else {
                ConfirmBlock::FeeMeasuring
            });
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
/// view does not read — the shell keeps the slide shut.
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
