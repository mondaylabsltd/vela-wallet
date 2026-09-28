//! The signing column once the person has approved (spec 079 US1).
//!
//! The owner, on the phone: "签完后，回到签名提示框，似乎没有任何提示". The desktop
//! kept the whole form — and a slide dimmed to 45% — for up to ninety seconds
//! after the passkey, and only then, once the core had closed the sheet, drew a
//! receipt that read "submitted" for every tracker status but two. From the
//! approval on, the column is the send receipt instead: the same stages, the
//! same words, the same ring, so a dApp transaction and a send land the same
//! way (Android's `SigningLive.receipt`, word for word).
//!
//! Two moments, one vocabulary:
//!
//! - [`approved`] — the request is still open in the core: signing, then
//!   submitting, then submitted and counting against the chain's usual time,
//!   or failed with the send flow's sentence — for an operation that
//!   reverted on chain, the send receipt's failure with its transaction
//!   (083). The column stays here past the old 90 s: the page is not
//!   answered until the transaction is known, and "still confirming" is the
//!   tracker's word meanwhile — also after the core's cap has told the page
//!   "not confirmed yet", since the operation may still land.
//! - [`ended`] — the core has answered the page and closed its sheet: the
//!   tick (a message signed, a transaction landed), or — when an answer was
//!   the operation hash, which the desktop no longer gives (083) — "not
//!   landed yet, Vela keeps checking" for as long as the tracker follows it,
//!   and "unknown" past its 24 h line. Never "failed" on time alone: a
//!   timeout is not a failure (the tracker's money rule).
//!
//! Nothing here decides anything about the request. Which close answers the
//! page, and with what, is `sign_request`'s; whether an operation landed is
//! `tx_tracker`'s. This maps their views onto the receipt's words.

use gpui::SharedString;

use vela_core::app::sign_request::{
    SignErrorKind, SignMethodKind, SignResponsePayload, SignSwipeAction, SignView, method_kind,
    reverted_transaction,
};
use vela_core::app::tx_tracker::{TrackEntryView, TrackOutcome, TrackStatus};

use crate::executor::sign_request::PhoneStop;
use crate::flows::fixtures::ReceiptStage;
use crate::signing::SigningStrings;
use crate::signing::fixtures::Block;

/// Where an approved request stands, as the column draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct SigningReceipt {
    pub stage: ReceiptStage,
    pub title: SharedString,
    pub captions: Vec<SharedString>,
    /// The ring round the disc while it lands (`flows::live::ring_progress`).
    pub progress: Option<f32>,
    /// `(label, the whole hash)` — drawn short, copied whole.
    pub hash: Option<(SharedString, String)>,
    /// The transaction the explorer link opens — only once there is one.
    pub explorer_tx: Option<String>,
    pub cta: SharedString,
    pub cta_accent: bool,
}

/// How a request ended when the core answered the page and closed its sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SigningEnding {
    /// A message was signed.
    Signed,
    /// The operation landed inside the wait; the page got this hash.
    Landed { tx_hash: String },
    /// The wait ran out; the page got the operation hash and the tracker
    /// keeps following it.
    StillConfirming { user_op_hash: String },
}

/// The chain's clock, for the ring and the "~9s remaining" line.
pub struct Clock {
    pub now_ms: f64,
    /// The chain's usual inclusion time (`network_admin`'s table).
    pub typical_s: Option<u32>,
    pub chain_name: String,
    /// When this column first saw the operation accepted — the ring's start
    /// until the tracker has its own `submitted_at_ms`.
    pub seen_submitted_ms: Option<f64>,
}

/// Where this approval's signature stands (083 W11) — the executor's word,
/// since the core's `Submitting` spans building the operation, the passkey
/// and the submission alike.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Signature {
    /// Nothing has asked for it yet: the funding check, the nonce, the
    /// deployment read.
    #[default]
    NotYet,
    /// A prompt is up (or the Trusted Signer's page is asking).
    Asked,
    /// The prompt has signed. Not merely answered: a prompt that said no
    /// signed nothing, and the column's close reads `Given` as "the
    /// operation goes on" (083 review).
    Given,
}

/// Does this method go to the network? A message never does: it is signing,
/// then signed — never "submitting" (device-found on the Xiaomi, spec 079).
#[must_use]
pub fn on_chain(method: &str) -> bool {
    matches!(
        method_kind(method),
        SignMethodKind::Transaction | SignMethodKind::Batch
    )
}

/// When the column's close reaches the core (083). What it answers stays the
/// core's (`swipe_action`); this is only whether it may be told NOW.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClosePlan {
    /// Tell the core now: it refuses a request not yet approved (4001),
    /// cancels a top-up, or stops watching an operation on its way.
    Now,
    /// The core would only stop watching — but the signature is still to
    /// come, and a prompt that then came back unsigned answered the page
    /// nothing, ever (083 review; before 083 a scan that ran out answered
    /// -32603 after 90 s). So the prompt is stopped, and the core is told
    /// once the ceremony is back: a refusal if nothing was signed, the
    /// ordinary close if the phone had already signed.
    AfterCancel,
    /// A close already waits on the ceremony. A second one must add nothing:
    /// told to the core as a dismiss, it left the request unanswered.
    Ignore,
}

/// Where a request stands, as far as closing its column goes (083).
#[derive(Clone, Copy, Debug)]
pub struct CloseFacts {
    /// What the core does with a close right now.
    pub swipe: SignSwipeAction,
    /// A pipeline runs (the core's `is_signing || is_submitting`).
    pub running: bool,
    /// An operation went out (`pending_op_hash`).
    pub submitted: bool,
    /// The core already failed it — and answered the page.
    pub failed: bool,
    pub signature: Signature,
    /// A close already waits on the ceremony.
    pub closing: bool,
}

/// [`ClosePlan`] for these facts.
#[must_use]
pub fn close_plan(facts: &CloseFacts) -> ClosePlan {
    if facts.closing {
        return ClosePlan::Ignore;
    }
    let signature_ahead = facts.swipe == SignSwipeAction::Dismiss
        && facts.running
        && !facts.submitted
        && !facts.failed
        && facts.signature != Signature::Given;
    if signature_ahead {
        ClosePlan::AfterCancel
    } else {
        ClosePlan::Now
    }
}

/// A close that waited on the ceremony may be told to the core: the ceremony
/// came back empty (nothing runs — the core now refuses), or an operation
/// went out (the phone had signed — the core now stops watching it).
#[must_use]
pub fn waited_close_due(running: bool, submitted: bool) -> bool {
    !running || submitted
}

/// Esc on the signing column (083, the owner's D1): it closes only where
/// closing answers nothing — an operation already on its way (the core only
/// stops watching it), or no request at all. A refusal (4001), a funding
/// cancel, or a blocked request's answer is the ✕'s alone: on Windows Esc is
/// also the key that leaves the address bar, and one press refused the page.
/// Nor does it close while the signature is still to come: that close ends
/// in a refusal too, once the prompt comes back unsigned.
#[must_use]
pub fn escape_closes(swipe: SignSwipeAction, plan: ClosePlan) -> bool {
    plan == ClosePlan::Now && matches!(swipe, SignSwipeAction::Dismiss | SignSwipeAction::None)
}

/// The title and body keys of the card a phone's stop is told with (083). A
/// connection that never came up is the onboarding's own pair — "the network
/// connection is unstable", "the request never arrived — check your network":
/// nothing reached the phone, and the network is what a person can check
/// before trying again. One that dropped once the phone was asked has the
/// same title, but the request DID arrive, so the body is what is true
/// whatever the phone did (H4 review): a transaction was not submitted and
/// the funds are safe; a message went nowhere — `on_chain` says which. The
/// title is a card's sentence case, as the timeout's is: the connection list's
/// "Connection Failed" is a status label, title-cased in en, id and tr.
#[must_use]
pub const fn phone_stop_words(stop: PhoneStop, on_chain: bool) -> (&'static str, &'static str) {
    const LINK: &str = "onboarding.common.networkTitle";
    match stop {
        PhoneStop::ScanExpired => (
            "onboarding.common.timeoutTitle",
            "onboarding.common.timeoutBody",
        ),
        PhoneStop::LinkFailed => (LINK, "onboarding.common.networkBody"),
        PhoneStop::Dropped if on_chain => (LINK, "send.txErrorGeneric"),
        PhoneStop::Dropped => (LINK, "connect.detail.offChainNote"),
    }
}

/// The ending an answer stands for, or `None` when there is nothing to show —
/// a refusal (the page was told why; the column said so) or an empty answer.
/// `submitted` is the operation this request handed the tracker: an answer
/// that IS that hash means the wait ran out before a receipt.
#[must_use]
pub fn ending_of(
    method: &str,
    payload: &SignResponsePayload,
    submitted: Option<&str>,
) -> Option<SigningEnding> {
    let SignResponsePayload::Ok {
        result: Some(result),
    } = payload
    else {
        return None;
    };
    if result.trim().is_empty() {
        return None;
    }
    if !on_chain(method) {
        return Some(SigningEnding::Signed);
    }
    Some(match submitted {
        Some(op) if op.eq_ignore_ascii_case(result) => SigningEnding::StillConfirming {
            user_op_hash: op.to_owned(),
        },
        _ => SigningEnding::Landed {
            tx_hash: result.clone(),
        },
    })
}

/// The request in one line, from the blocks the column already drew: what it
/// is ("发送", "授权") and its figure — so the receipt still says WHAT is
/// landing once the form has gone.
#[must_use]
pub fn summary_of(blocks: &[Block]) -> Option<SharedString> {
    let intent = blocks.iter().find_map(|block| match block {
        Block::Intent { text, .. } if !text.trim().is_empty() => Some(text.to_string()),
        _ => None,
    });
    let figure = blocks.iter().find_map(|block| match block {
        Block::Amount { line, .. } => Some(
            format!("{}{} {}", line.sign, line.value, line.symbol)
                .trim()
                .to_owned(),
        ),
        Block::Swap { pay, receive } => Some(format!(
            "{} {} → {} {}",
            pay.value, pay.symbol, receive.value, receive.symbol
        )),
        _ => None,
    });
    let line = [intent, figure]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    (!line.is_empty()).then(|| SharedString::from(line))
}

/// The column after the approval, while the core still holds the request —
/// `None` while it is still a request (the form is drawn).
///
/// `track` is the tracker's entry for the operation this request submitted,
/// when there is one; `signature` is where its passkey stands.
#[must_use]
pub fn approved(
    sign: &SignView,
    on_chain: bool,
    summary: Option<&SharedString>,
    track: Option<&TrackEntryView>,
    clock: &Clock,
    signature: Signature,
    s: &SigningStrings,
) -> Option<SigningReceipt> {
    let lead = || summary.cloned().into_iter().collect::<Vec<_>>();
    if !on_chain && (sign.is_signing || sign.is_submitting) {
        return Some(receipt(
            ReceiptStage::Submitting,
            s.status_signing.clone(),
            lead(),
            s.close.clone(),
            false,
        ));
    }
    // A refusal AFTER the approval — the submission failed. The page already
    // has its error; this is the send flow's sentence for it. A refusal
    // before (a 4902, the unlimited gate) stays on the form, where the reason
    // is drawn beside what was asked.
    if let Some(error) = sign.error.as_ref()
        && error.kind != SignErrorKind::UserRejected
        && (sign.pending_op_hash.is_some() || error.kind == SignErrorKind::SubmitFailed)
    {
        // 083: an operation that reverted on chain — the tracker holds the
        // transaction that carried it, so this is the send receipt's own
        // failure for that: its sentence, the hash, and the explorer, which
        // says why. Never 已确认.
        if let Some(op) = sign.pending_op_hash.as_deref()
            && let Some(entry) = track.filter(|entry| {
                entry.user_op_hash.eq_ignore_ascii_case(op) && entry.status == TrackStatus::Dropped
            })
        {
            return Some(following(op, Some(entry), lead(), clock, s));
        }
        // The core's own word that it reverted, before the tracker has it:
        // the same receipt at once. "Couldn't be submitted — your funds are
        // safe" was false here: it was included, and a fee may have gone.
        if on_chain
            && let Some(tx) = sign
                .error
                .as_ref()
                .and_then(|error| error.detail.as_deref())
                .and_then(reverted_transaction)
        {
            let mut captions = lead();
            captions.push(s.receipt_failed_hint.clone());
            let mut out = receipt(
                ReceiptStage::Failed,
                s.receipt_failed.clone(),
                captions,
                s.receipt_done.clone(),
                true,
            );
            out.hash = Some((s.receipt_tx_hash.clone(), tx.to_owned()));
            out.explorer_tx = Some(tx.to_owned());
            return Some(out);
        }
        let mut captions = lead();
        // A message goes nowhere: "the transaction couldn't be submitted" is
        // not what failed (083 H4), and nothing went on chain.
        captions.push(if on_chain {
            s.error_generic.clone()
        } else {
            s.error_off_chain.clone()
        });
        return Some(receipt(
            ReceiptStage::Failed,
            s.receipt_failed.clone(),
            captions,
            s.receipt_done.clone(),
            true,
        ));
    }
    if let Some(op) = sign.pending_op_hash.as_deref() {
        let track = track.filter(|entry| entry.user_op_hash.eq_ignore_ascii_case(op));
        return Some(following(op, track, lead(), clock, s));
    }
    // Submitting once the signature is given — the core's `Submitting` also
    // covers building the operation and the prompt, which are not (083 W11).
    if sign.is_submitting && (!sign.is_signing || signature == Signature::Given) {
        let mut captions = lead();
        captions.push(s.tx_background_hint.clone());
        return Some(receipt(
            ReceiptStage::Submitting,
            s.tx_submitting.clone(),
            captions,
            s.tx_close_background.clone(),
            false,
        ));
    }
    if sign.is_signing {
        // "Waiting for biometric" only while the passkey (or the Trusted
        // Signer's page — its own dialog stands over this one) is asking.
        // Before it, the funding check, the nonce and the deployment read
        // are the wallet preparing; the owner saw the biometric line for
        // seconds with no prompt anywhere (083 W11).
        let title = if signature == Signature::Asked {
            s.tx_signing.clone()
        } else {
            s.tx_preparing.clone()
        };
        return Some(receipt(
            ReceiptStage::Submitting,
            title,
            lead(),
            s.close.clone(),
            false,
        ));
    }
    None
}

/// The ending of a request whose sheet the core has closed.
#[must_use]
pub fn ended(
    ending: &SigningEnding,
    summary: Option<&SharedString>,
    track: Option<&TrackEntryView>,
    clock: &Clock,
    s: &SigningStrings,
) -> SigningReceipt {
    let lead = || summary.cloned().into_iter().collect::<Vec<_>>();
    match ending {
        SigningEnding::Signed => receipt(
            ReceiptStage::Confirmed,
            s.signed.clone(),
            lead(),
            s.receipt_done.clone(),
            true,
        )
        .with_progress(Some(1.)),
        SigningEnding::Landed { tx_hash } => landed(tx_hash, lead(), clock, s),
        SigningEnding::StillConfirming { user_op_hash } => {
            let track = track.filter(|entry| entry.user_op_hash.eq_ignore_ascii_case(user_op_hash));
            following(user_op_hash, track, lead(), clock, s)
        }
    }
}

/// An operation the relay accepted, as the tracker has it — every status.
fn following(
    op: &str,
    track: Option<&TrackEntryView>,
    mut captions: Vec<SharedString>,
    clock: &Clock,
    s: &SigningStrings,
) -> SigningReceipt {
    let op_hash = Some((s.receipt_op_hash.clone(), op.to_owned()));
    if let Some(entry) = track {
        match entry.status {
            TrackStatus::Confirmed => {
                if let Some(tx) = entry.tx_hash.as_deref() {
                    return landed(tx, captions, clock, s);
                }
            }
            // Reverted on chain: the send receipt's own sentence for it, and
            // the explorer, which says why.
            TrackStatus::Dropped => {
                captions.push(s.receipt_failed_hint.clone());
                let mut out = receipt(
                    ReceiptStage::Failed,
                    s.receipt_failed.clone(),
                    captions,
                    s.receipt_done.clone(),
                    true,
                );
                out.hash = entry
                    .tx_hash
                    .clone()
                    .map(|tx| (s.receipt_tx_hash.clone(), tx))
                    .or(op_hash);
                out.explorer_tx = entry.tx_hash.clone();
                return out;
            }
            // The relay refused it: nothing was sent.
            TrackStatus::Rejected => {
                captions.push(s.error_generic.clone());
                let mut out = receipt(
                    ReceiptStage::Failed,
                    s.receipt_failed.clone(),
                    captions,
                    s.receipt_done.clone(),
                    true,
                );
                out.hash = op_hash;
                return out;
            }
            _ => {}
        }
    }
    let outcome = track.map_or(TrackOutcome::Landing, |entry| entry.outcome);
    let held = track.is_some_and(|entry| entry.status == TrackStatus::FeeHeld);
    let mut out = receipt(
        ReceiptStage::Submitted,
        s.tx_submitted_title.clone(),
        Vec::new(),
        s.tx_close_background.clone(),
        false,
    );
    match outcome {
        // Past the 24 h line: nobody asks any more, and nobody knows.
        TrackOutcome::Unknown => captions.push(s.unknown_outcome.clone()),
        // Past the window: handed to the network, not landed, still watched.
        TrackOutcome::StillConfirming => captions.push(s.still_confirming.clone()),
        TrackOutcome::Landing | TrackOutcome::Final => {
            captions.push(if held {
                s.tx_held_fees.clone()
            } else {
                s.tx_waiting_confirm.clone()
            });
            // Count, don't spin (spec 038 #D3): the chain's usual time, then
            // what is left of it, then "almost there", then "taking longer".
            let since = track
                .and_then(|entry| entry.submitted_at_ms)
                .or(clock.seen_submitted_ms);
            if let (Some(since), Some(typical)) = (since, clock.typical_s) {
                let elapsed = ((clock.now_ms - since) / 1000.).max(0.) as u64;
                let typical = u64::from(typical);
                captions.push(SharedString::from(crate::signing::fill(
                    &s.tx_typical_time,
                    &[
                        ("chainName", &clock.chain_name),
                        ("estSecs", &typical.to_string()),
                    ],
                )));
                captions.push(if elapsed < typical {
                    SharedString::from(crate::signing::fill(
                        &s.tx_remaining,
                        &[("remaining", &(typical - elapsed).to_string())],
                    ))
                } else if elapsed < typical * 2 {
                    SharedString::from(crate::signing::fill(
                        &s.tx_elapsed,
                        &[("elapsed", &elapsed.to_string())],
                    ))
                } else {
                    s.tx_slow_confirm.clone()
                });
                out.progress = crate::flows::live::ring_progress(elapsed, typical);
            }
        }
    }
    out.captions = captions;
    // The OPERATION hash: there is no transaction until it lands, and
    // labelling one as the other sends a person to search an explorer for
    // nothing.
    out.hash = op_hash;
    out
}

fn landed(
    tx_hash: &str,
    mut captions: Vec<SharedString>,
    clock: &Clock,
    s: &SigningStrings,
) -> SigningReceipt {
    if !clock.chain_name.is_empty() {
        captions.push(SharedString::from(clock.chain_name.clone()));
    }
    let mut out = receipt(
        ReceiptStage::Confirmed,
        s.receipt_confirmed.clone(),
        captions,
        s.receipt_done.clone(),
        true,
    )
    .with_progress(Some(1.));
    out.hash = Some((s.receipt_tx_hash.clone(), tx_hash.to_owned()));
    out.explorer_tx = Some(tx_hash.to_owned());
    out
}

fn receipt(
    stage: ReceiptStage,
    title: SharedString,
    captions: Vec<SharedString>,
    cta: SharedString,
    cta_accent: bool,
) -> SigningReceipt {
    SigningReceipt {
        stage,
        title,
        captions,
        progress: None,
        hash: None,
        explorer_tx: None,
        cta,
        cta_accent,
    }
}

impl SigningReceipt {
    fn with_progress(mut self, progress: Option<f32>) -> Self {
        self.progress = progress;
        self
    }

    /// A tick is seen, then it goes: nobody should have to close a success.
    /// How long it stays — a message's tick is shorter than a landing, which
    /// carries a hash somebody may want to read (Android's 1.4 s / 2.6 s).
    #[must_use]
    pub fn closes_after_ms(&self, ending: &SigningEnding) -> Option<u64> {
        (self.stage == ReceiptStage::Confirmed).then_some(match ending {
            SigningEnding::Signed => 1_400,
            _ => 2_600,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::sign_request::{SignErrorNotice, SignSurface};

    use crate::signing::Tone;
    use crate::signing::fixtures::AmountLine;

    const OP: &str = "0xace642c724834405c56919fb198d6a6389cc2867ff3f3e3585b59d1921fe681d";
    const TX: &str = "0x09c35478682ddce770c02e4aed27c9f0260f7b2f54ae6a7a194f50b70d604c9f";

    fn strings() -> SigningStrings {
        SigningStrings::resolve(&crate::loc::Loc::from_env())
    }

    /// The core's own pristine view, with the fields a test turns on.
    fn view(edit: impl FnOnce(&mut SignView)) -> SignView {
        let mut view =
            crate::core_host::CoreHost::<vela_core::app::sign_request::SignRequest>::new().view();
        view.surface = SignSurface::Sheet;
        edit(&mut view);
        view
    }

    fn entry(status: TrackStatus, outcome: TrackOutcome, tx: Option<&str>) -> TrackEntryView {
        TrackEntryView {
            user_op_hash: OP.to_owned(),
            chain_id: 100,
            record_ids: Vec::new(),
            status,
            tx_hash: tx.map(str::to_owned),
            polling: !matches!(outcome, TrackOutcome::Final | TrackOutcome::Unknown),
            submitted_at_ms: Some(1_000.),
            outcome,
        }
    }

    fn clock(now_ms: f64) -> Clock {
        Clock {
            now_ms,
            typical_s: Some(5),
            chain_name: "Gnosis".to_owned(),
            seen_submitted_ms: None,
        }
    }

    fn summary() -> SharedString {
        SharedString::from("Send · −0.001 XDAI")
    }

    #[test]
    fn a_request_not_yet_approved_is_still_the_form() {
        let s = strings();
        assert!(
            approved(
                &view(|_| {}),
                true,
                None,
                None,
                &clock(0.),
                Signature::NotYet,
                &s
            )
            .is_none()
        );
    }

    /// The passkey, the submission and the wait are each named in the send's
    /// own words, and the slide is gone from the first of them.
    #[test]
    fn signing_submitting_and_waiting_read_as_the_send_receipt() {
        let s = strings();
        let signing = approved(
            &view(|v| v.is_signing = true),
            true,
            Some(&summary()),
            None,
            &clock(0.),
            Signature::Asked,
            &s,
        )
        .unwrap_or_else(|| unreachable!("approved"));
        assert_eq!(signing.stage, ReceiptStage::Submitting);
        assert_eq!(signing.title, s.tx_signing);
        assert_eq!(signing.captions, vec![summary()]);

        let submitting = approved(
            &view(|v| v.is_submitting = true),
            true,
            None,
            None,
            &clock(0.),
            Signature::NotYet,
            &s,
        )
        .unwrap_or_else(|| unreachable!("approved"));
        assert_eq!(submitting.title, s.tx_submitting);
        assert_eq!(submitting.cta, s.tx_close_background);

        let waiting = approved(
            &view(|v| v.pending_op_hash = Some(OP.to_owned())),
            true,
            Some(&summary()),
            Some(&entry(TrackStatus::Pending, TrackOutcome::Landing, None)),
            &clock(3_000.),
            Signature::NotYet,
            &s,
        )
        .unwrap_or_else(|| unreachable!("approved"));
        assert_eq!(waiting.stage, ReceiptStage::Submitted);
        assert_eq!(waiting.title, s.tx_submitted_title);
        assert_eq!(waiting.captions[0], summary());
        assert_eq!(waiting.captions[1], s.tx_waiting_confirm);
        assert!(
            waiting.captions[2].contains("Gnosis"),
            "the chain's usual time"
        );
        assert!(
            waiting.captions[3].contains('3'),
            "5 s usual, 2 s gone: ~3 s left"
        );
        assert!(waiting.progress.is_some(), "the ring counts");
        assert_eq!(
            waiting.hash,
            Some((s.receipt_op_hash.clone(), OP.to_owned())),
            "the OPERATION hash — there is no transaction yet"
        );
        assert!(waiting.explorer_tx.is_none());
    }

    /// Submitted outranks signing: once there is a hash the ceremony is over.
    #[test]
    fn a_hash_outranks_a_ceremony_still_marked_as_running() {
        let s = strings();
        let both = approved(
            &view(|v| {
                v.is_signing = true;
                v.pending_op_hash = Some(OP.to_owned());
            }),
            true,
            None,
            None,
            &clock(0.),
            Signature::NotYet,
            &s,
        )
        .unwrap_or_else(|| unreachable!("approved"));
        assert_eq!(both.stage, ReceiptStage::Submitted);
    }

    /// 083 W11: the funding check, the nonce and the deployment read are the
    /// wallet preparing; "waiting for biometric" is said only while a prompt
    /// is up, and "submitting" once it has answered.
    #[test]
    fn preparing_is_not_waiting_for_biometric() {
        let s = strings();
        let title = |signing: bool, submitting: bool, signature: Signature| {
            approved(
                &view(|v| {
                    v.is_signing = signing;
                    v.is_submitting = submitting;
                }),
                true,
                None,
                None,
                &clock(0.),
                signature,
                &s,
            )
            .map(|receipt| receipt.title)
        };
        let precheck = title(true, false, Signature::NotYet);
        assert_eq!(
            precheck.as_ref(),
            Some(&s.tx_preparing),
            "the funding check"
        );
        assert_ne!(s.tx_preparing, s.tx_signing);
        assert_eq!(
            title(true, true, Signature::NotYet).as_ref(),
            Some(&s.tx_preparing),
            "building the operation"
        );
        assert_eq!(
            title(true, true, Signature::Asked).as_ref(),
            Some(&s.tx_signing)
        );
        assert_eq!(
            title(true, true, Signature::Given).as_ref(),
            Some(&s.tx_submitting)
        );
        assert_eq!(
            title(false, true, Signature::NotYet).as_ref(),
            Some(&s.tx_submitting),
            "the relay's own top-up after a signature"
        );
    }

    /// Facts for [`close_plan`]: a transaction past the commitment point
    /// (a close is a dismiss), nothing submitted, nothing failed.
    fn committed(signature: Signature) -> CloseFacts {
        CloseFacts {
            swipe: SignSwipeAction::Dismiss,
            running: true,
            submitted: false,
            failed: false,
            signature,
            closing: false,
        }
    }

    /// 083 (review): the close is told to the core now, except while the
    /// signature is still to come — then only once the ceremony is back —
    /// and a second close while one waits adds nothing.
    #[test]
    fn a_close_waits_only_for_a_signature_still_to_come() {
        use ClosePlan::{AfterCancel, Ignore, Now};
        let table: [(&str, CloseFacts, ClosePlan); 10] = [
            (
                "preparing: nonce, deployment",
                committed(Signature::NotYet),
                AfterCancel,
            ),
            (
                "a prompt or the QR is up",
                committed(Signature::Asked),
                AfterCancel,
            ),
            (
                "the phone signed: on its way",
                committed(Signature::Given),
                Now,
            ),
            (
                "an operation went out",
                CloseFacts {
                    submitted: true,
                    ..committed(Signature::Asked)
                },
                Now,
            ),
            (
                "a failure, already answered",
                CloseFacts {
                    failed: true,
                    ..committed(Signature::NotYet)
                },
                Now,
            ),
            (
                "the funding check (the core refuses and stops it)",
                CloseFacts {
                    swipe: SignSwipeAction::Reject,
                    ..committed(Signature::NotYet)
                },
                Now,
            ),
            (
                "the form",
                CloseFacts {
                    swipe: SignSwipeAction::Reject,
                    running: false,
                    ..committed(Signature::NotYet)
                },
                Now,
            ),
            (
                "the top-up",
                CloseFacts {
                    swipe: SignSwipeAction::FundingCancel,
                    running: false,
                    ..committed(Signature::NotYet)
                },
                Now,
            ),
            (
                "a close already waits",
                CloseFacts {
                    closing: true,
                    ..committed(Signature::Asked)
                },
                Ignore,
            ),
            (
                "…whatever else is true",
                CloseFacts {
                    closing: true,
                    swipe: SignSwipeAction::Reject,
                    running: false,
                    ..committed(Signature::NotYet)
                },
                Ignore,
            ),
        ];
        for (case, facts, plan) in table {
            assert_eq!(close_plan(&facts), plan, "{case}");
        }
    }

    /// The close that waited goes to the core once nothing runs, or once an
    /// operation is out — never while the ceremony is still going.
    #[test]
    fn a_waited_close_goes_once_the_ceremony_is_back() {
        assert!(!waited_close_due(true, false), "still asking");
        assert!(waited_close_due(false, false), "came back empty");
        assert!(waited_close_due(true, true), "the phone signed");
    }

    /// 083 (D1): Esc never answers a request — only the ✕ refuses one. It
    /// still closes a column whose close answers nothing, and never one whose
    /// close waits for the prompt to come back unsigned — a refusal too.
    #[test]
    fn escape_never_refuses_a_request() {
        use ClosePlan::{AfterCancel, Ignore, Now};
        assert!(escape_closes(SignSwipeAction::None, Now));
        assert!(escape_closes(SignSwipeAction::Dismiss, Now));
        assert!(!escape_closes(SignSwipeAction::Reject, Now));
        assert!(!escape_closes(SignSwipeAction::FundingCancel, Now));
        assert!(!escape_closes(SignSwipeAction::Dismiss, AfterCancel));
        assert!(!escape_closes(SignSwipeAction::Dismiss, Ignore));
        assert!(!escape_closes(SignSwipeAction::None, Ignore));
    }

    /// A message is signing, then signed — never "submitting".
    #[test]
    fn a_message_never_says_submitting() {
        let s = strings();
        let out = approved(
            &view(|v| v.is_submitting = true),
            false,
            None,
            None,
            &clock(0.),
            Signature::NotYet,
            &s,
        )
        .unwrap_or_else(|| unreachable!("approved"));
        assert_eq!(out.title, s.status_signing);
        let signed = ended(&SigningEnding::Signed, None, None, &clock(0.), &s);
        assert_eq!(signed.stage, ReceiptStage::Confirmed);
        assert_eq!(signed.title, s.signed);
        assert_eq!(signed.closes_after_ms(&SigningEnding::Signed), Some(1_400));
    }

    /// A submission that failed says so in the send flow's words; a refusal
    /// the person made, or one before the approval, stays on the form.
    #[test]
    fn a_failed_submission_is_a_failed_receipt_and_a_refusal_is_not() {
        let s = strings();
        let failed = approved(
            &view(|v| {
                v.error = Some(SignErrorNotice {
                    kind: SignErrorKind::SubmitFailed,
                    detail: Some("relay unreachable".to_owned()),
                });
            }),
            true,
            Some(&summary()),
            None,
            &clock(0.),
            Signature::NotYet,
            &s,
        )
        .unwrap_or_else(|| unreachable!("approved"));
        assert_eq!(failed.stage, ReceiptStage::Failed);
        assert_eq!(failed.captions, vec![summary(), s.error_generic.clone()]);
        assert!(
            !failed.captions.iter().any(|line| line.contains("relay")),
            "no raw relay text on a screen"
        );
        for kind in [
            SignErrorKind::UserRejected,
            SignErrorKind::UnsupportedChain,
            SignErrorKind::UnlimitedApproval,
        ] {
            let form = approved(
                &view(|v| {
                    v.error = Some(SignErrorNotice { kind, detail: None });
                }),
                true,
                None,
                None,
                &clock(0.),
                Signature::NotYet,
                &s,
            );
            assert!(form.is_none(), "{kind:?} is not a submission that failed");
        }
    }

    /// 083 S2: an operation that REVERTED on chain. The core answered the
    /// page an error and failed the record; the column shows the send
    /// receipt's own failure for it — its sentence, the hash and the
    /// explorer — at once, from the core's error, and the same once the
    /// tracker has the transaction. Never the tick, never 已确认, and never
    /// "couldn't be submitted, your funds are safe" (receipts review).
    #[test]
    fn a_reverted_operation_is_a_failure_receipt_with_its_transaction() {
        let s = strings();
        let reverted = view(|v| {
            v.pending_op_hash = Some(OP.to_owned());
            v.error = Some(SignErrorNotice {
                kind: SignErrorKind::SubmitFailed,
                detail: Some(format!(
                    "{} ({TX})",
                    vela_core::app::sign_request::REVERTED_MESSAGE
                )),
            });
        });
        let at = |track: Option<&TrackEntryView>| {
            approved(
                &reverted,
                true,
                Some(&summary()),
                track,
                &clock(20_000.),
                Signature::Given,
                &s,
            )
            .unwrap_or_else(|| unreachable!("approved"))
        };

        let at_once = at(None);
        assert_eq!(at_once.stage, ReceiptStage::Failed);
        assert_eq!(at_once.title, s.receipt_failed);
        assert!(at_once.captions.contains(&s.receipt_failed_hint));
        assert!(!at_once.captions.contains(&s.error_generic));
        assert_eq!(
            at_once.hash,
            Some((s.receipt_tx_hash.clone(), TX.to_owned()))
        );
        assert_eq!(at_once.explorer_tx.as_deref(), Some(TX));
        assert!(
            !at_once
                .captions
                .iter()
                .any(|line| line.contains(vela_core::app::sign_request::REVERTED_MESSAGE)),
            "the page's developer sentence stays off the screen (the column's own hint may say reverted)"
        );

        let dropped = entry(TrackStatus::Dropped, TrackOutcome::Final, Some(TX));
        let with_tx = at(Some(&dropped));
        assert_eq!(with_tx.stage, ReceiptStage::Failed);
        assert_eq!(with_tx.title, s.receipt_failed);
        assert!(with_tx.captions.contains(&s.receipt_failed_hint));
        assert_eq!(
            with_tx.hash,
            Some((s.receipt_tx_hash.clone(), TX.to_owned()))
        );
        assert_eq!(
            with_tx.explorer_tx.as_deref(),
            Some(TX),
            "the explorer says why"
        );

        // Even with a tracker that has not caught up — or says otherwise —
        // the core's failure stands.
        let pending = entry(TrackStatus::Pending, TrackOutcome::Landing, None);
        assert_eq!(at(Some(&pending)).stage, ReceiptStage::Failed);
        assert_eq!(at(Some(&pending)).explorer_tx.as_deref(), Some(TX));

        // Any other submit failure is still the generic one.
        let refused = view(|v| {
            v.error = Some(SignErrorNotice {
                kind: SignErrorKind::SubmitFailed,
                detail: Some("relay said no".to_owned()),
            });
        });
        let generic = approved(
            &refused,
            true,
            Some(&summary()),
            None,
            &clock(0.),
            Signature::Given,
            &s,
        )
        .unwrap_or_else(|| unreachable!("approved"));
        assert!(generic.captions.contains(&s.error_generic));
        assert_eq!(generic.explorer_tx, None);
    }

    /// 083 S3: at the core's cap the page was told "not confirmed yet" — but
    /// the column does not call it failed: the operation may still land, and
    /// it says "still confirming" until the tracker knows, then lands.
    #[test]
    fn past_the_cap_the_column_still_follows_the_operation() {
        let s = strings();
        let answered = view(|v| {
            v.pending_op_hash = Some(OP.to_owned());
            v.tracker_handoff = None;
        });
        let at = |status, outcome, tx| {
            approved(
                &answered,
                true,
                None,
                Some(&entry(status, outcome, tx)),
                &clock(700_000.),
                Signature::Given,
                &s,
            )
            .unwrap_or_else(|| unreachable!("approved"))
        };
        let still = at(
            TrackStatus::AcceptedNotLanded,
            TrackOutcome::StillConfirming,
            None,
        );
        assert_eq!(still.stage, ReceiptStage::Submitted);
        assert!(still.captions.contains(&s.still_confirming));
        assert_eq!(
            still.hash,
            Some((s.receipt_op_hash.clone(), OP.to_owned())),
            "the OPERATION hash, labelled as one"
        );
        let landed = at(TrackStatus::Confirmed, TrackOutcome::Final, Some(TX));
        assert_eq!(landed.stage, ReceiptStage::Confirmed);
    }

    /// 083 H4: a message whose signature failed is not a transaction that
    /// could not be submitted — it went nowhere, and says so.
    #[test]
    fn a_failed_message_is_not_a_failed_transaction() {
        let s = strings();
        let failed = approved(
            &view(|v| {
                v.error = Some(SignErrorNotice {
                    kind: SignErrorKind::SubmitFailed,
                    detail: Some("personal_sign carried nothing this wallet could sign".to_owned()),
                });
            }),
            false,
            Some(&summary()),
            None,
            &clock(0.),
            Signature::Asked,
            &s,
        )
        .unwrap_or_else(|| unreachable!("approved"));
        assert_eq!(failed.stage, ReceiptStage::Failed);
        assert_eq!(failed.captions, vec![summary(), s.error_off_chain.clone()]);
        assert_ne!(s.error_off_chain, s.error_generic);
        assert_eq!(failed.cta, s.receipt_done, "the close stays");
    }

    /// 083: the phone stops are told apart — a scan nobody answered is its
    /// window; a phone that scanned and never connected is the connection
    /// and the network; one that dropped once asked (H4 review) is the
    /// connection, and not "the request never arrived", which was false
    /// there: its body is the transaction's "not submitted, funds safe" or
    /// the message's "nothing went on chain" — each in words every
    /// catalogue has, checked in every one of them (not in whatever language
    /// the machine running the test is set to), and titled in a card's
    /// sentence case, as the timeout is.
    #[test]
    fn a_phone_stop_says_which_stop_it_was() {
        for on_chain in [true, false] {
            let scan = phone_stop_words(PhoneStop::ScanExpired, on_chain);
            let link = phone_stop_words(PhoneStop::LinkFailed, on_chain);
            let dropped = phone_stop_words(PhoneStop::Dropped, on_chain);
            assert_ne!(scan.0, link.0);
            assert_ne!(scan.1, link.1);
            assert_eq!(dropped.0, link.0, "the connection, both times");
            assert_ne!(dropped.1, link.1, "but the request did arrive");
            assert_ne!(dropped.1, scan.1);
            for (language, loc) in crate::loc::Loc::every_language() {
                for key in [scan.0, scan.1, link.0, link.1, dropped.0, dropped.1] {
                    assert_ne!(
                        loc.t(key).as_ref(),
                        key,
                        "{language}: `{key}` echoed the key"
                    );
                }
            }
        }
        let en = crate::loc::Loc::for_language("en");
        assert_eq!(
            en.t(phone_stop_words(PhoneStop::LinkFailed, true).0)
                .as_ref(),
            "Network connection is unstable",
            "a card's sentence case, not the list's \"Connection Failed\""
        );
        for (language, loc) in crate::loc::Loc::every_language() {
            let s = SigningStrings::resolve(&loc);
            let body = |on_chain| loc.t(phone_stop_words(PhoneStop::Dropped, on_chain).1);
            assert_eq!(body(true), s.error_generic, "{language}: not submitted");
            assert_eq!(
                body(false),
                s.error_off_chain,
                "{language}: nothing on chain"
            );
        }
    }

    /// Every tracker status has its own words — the column used to read
    /// "submitted" for all but two.
    #[test]
    fn every_tracker_status_is_mapped() {
        let s = strings();
        let at = |status, outcome, tx| {
            approved(
                &view(|v| v.pending_op_hash = Some(OP.to_owned())),
                true,
                None,
                Some(&entry(status, outcome, tx)),
                &clock(60_000.),
                Signature::NotYet,
                &s,
            )
            .unwrap_or_else(|| unreachable!("approved"))
        };
        let confirmed = at(TrackStatus::Confirmed, TrackOutcome::Final, Some(TX));
        assert_eq!(confirmed.stage, ReceiptStage::Confirmed);
        assert_eq!(confirmed.explorer_tx.as_deref(), Some(TX));

        let dropped = at(TrackStatus::Dropped, TrackOutcome::Final, Some(TX));
        assert_eq!(dropped.stage, ReceiptStage::Failed);
        assert!(dropped.captions.contains(&s.receipt_failed_hint));
        assert_eq!(
            dropped.explorer_tx.as_deref(),
            Some(TX),
            "the explorer says why"
        );

        let rejected = at(TrackStatus::Rejected, TrackOutcome::Final, None);
        assert_eq!(rejected.stage, ReceiptStage::Failed);
        assert!(rejected.captions.contains(&s.error_generic));

        let held = at(TrackStatus::FeeHeld, TrackOutcome::Landing, None);
        assert_eq!(held.stage, ReceiptStage::Submitted);
        assert!(held.captions.contains(&s.tx_held_fees));

        for status in [TrackStatus::AcceptedNotLanded, TrackStatus::Unreachable] {
            let still = at(status, TrackOutcome::StillConfirming, None);
            assert_eq!(
                still.stage,
                ReceiptStage::Submitted,
                "{status:?} never fails"
            );
            assert!(still.captions.contains(&s.still_confirming));
            assert!(still.progress.is_none(), "no ring promising a landing");
        }

        let unknown = at(TrackStatus::AcceptedNotLanded, TrackOutcome::Unknown, None);
        assert_eq!(
            unknown.stage,
            ReceiptStage::Submitted,
            "unknown is not failed"
        );
        assert!(unknown.captions.contains(&s.unknown_outcome));
    }

    /// The core's answer, read back: a message signed, a transaction landed
    /// inside the wait, or the operation hash because the wait ran out.
    #[test]
    fn the_answer_says_how_it_ended() {
        let ok = |result: &str| SignResponsePayload::Ok {
            result: Some(result.to_owned()),
        };
        assert_eq!(
            ending_of("personal_sign", &ok("0xsig"), None),
            Some(SigningEnding::Signed)
        );
        assert_eq!(
            ending_of("eth_sendTransaction", &ok(TX), Some(OP)),
            Some(SigningEnding::Landed {
                tx_hash: TX.to_owned()
            })
        );
        assert_eq!(
            ending_of("wallet_sendCalls", &ok(&OP.to_uppercase()), Some(OP)),
            Some(SigningEnding::StillConfirming {
                user_op_hash: OP.to_owned()
            })
        );
        let refused = SignResponsePayload::Err {
            code: 4001,
            kind: SignErrorKind::UserRejected,
            message: None,
        };
        assert_eq!(ending_of("eth_sendTransaction", &refused, Some(OP)), None);
        assert_eq!(
            ending_of(
                "eth_sendTransaction",
                &SignResponsePayload::Ok { result: None },
                None
            ),
            None
        );
    }

    /// After the core closed the sheet: a landing ticks and goes; a wait that
    /// ran out says so until the tracker sees it land — and then it ticks.
    #[test]
    fn an_ending_follows_the_tracker() {
        let s = strings();
        let still = SigningEnding::StillConfirming {
            user_op_hash: OP.to_owned(),
        };
        let waiting = ended(
            &still,
            None,
            Some(&entry(
                TrackStatus::AcceptedNotLanded,
                TrackOutcome::StillConfirming,
                None,
            )),
            &clock(200_000.),
            &s,
        );
        assert!(waiting.captions.contains(&s.still_confirming));
        assert_eq!(waiting.closes_after_ms(&still), None, "stays until closed");
        let landed_later = ended(
            &still,
            None,
            Some(&entry(
                TrackStatus::Confirmed,
                TrackOutcome::Final,
                Some(TX),
            )),
            &clock(210_000.),
            &s,
        );
        assert_eq!(landed_later.stage, ReceiptStage::Confirmed);
        assert_eq!(
            landed_later.hash,
            Some((s.receipt_tx_hash.clone(), TX.to_owned()))
        );
        let landed = SigningEnding::Landed {
            tx_hash: TX.to_owned(),
        };
        let tick = ended(&landed, None, None, &clock(0.), &s);
        assert_eq!(tick.explorer_tx.as_deref(), Some(TX));
        assert_eq!(tick.closes_after_ms(&landed), Some(2_600));
        // Another operation's entry is not this one's.
        let mut other = entry(TrackStatus::Confirmed, TrackOutcome::Final, Some(TX));
        other.user_op_hash = "0xother".to_owned();
        let unmoved = ended(&still, None, Some(&other), &clock(0.), &s);
        assert_eq!(unmoved.stage, ReceiptStage::Submitted);
    }

    #[test]
    fn the_summary_is_the_intent_and_its_figure() {
        let blocks = vec![
            Block::Intent {
                text: SharedString::from("Send"),
                tone: Tone::Neutral,
            },
            Block::Amount {
                line: AmountLine {
                    sign: SharedString::from("−"),
                    value: SharedString::from("0.001"),
                    symbol: SharedString::from("XDAI"),
                    token: None,
                    fiat: None,
                    caption: None,
                    tone: Tone::Neutral,
                },
                card: false,
                note: None,
                compact: false,
            },
        ];
        assert_eq!(
            summary_of(&blocks),
            Some(SharedString::from("Send · −0.001 XDAI"))
        );
        assert_eq!(summary_of(&[]), None);
    }
}
