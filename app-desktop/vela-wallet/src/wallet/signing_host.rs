//! The signing panel's journey — four machines, one column.
//!
//! `sign_request` owns the request's life; `clear_signing` says what the
//! transaction DOES; `approval_guard` decides what an approval may be edited
//! to; `fee_policy` prices it. The sheet reads all four, which is why they are
//! born and die together here rather than as residents: a request that ended
//! must not leave a decoded intent or a half-edited allowance behind for the
//! next one to inherit.
//!
//! The shape is `SendHost`'s, deliberately — the send column already proved
//! that a journey across machines wants one owner with one pump per machine,
//! and the correlation rules it got wrong four times (spec 032 lesson 2, the
//! fee session that must be ONE session) are not worth rediscovering.
//!
//! ## Who answers the dApp
//!
//! `SendResponse` is `SignAnswer::Screen` in the executor because only this
//! layer knows the transport a request arrived on. A site's answer does NOT
//! go to the page from here: it is queued ([`SigningHost::take_answers`]) and
//! the page hands it to the browser machine as `signing_answered`, which knows
//! which document asked, whether it is still there, and which request waits
//! in line behind this one (spec 070). The wallet's own requests ride
//! [`WALLET_TRANSPORT`] and have no page to tell.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use futures::StreamExt as _;
use gpui::Context;

use vela_core::app::Account;
use vela_core::app::approval_guard::{
    ApprovalGuard, Event as GuardEvent, GuardOperation, GuardShellResult, GuardView,
};
use vela_core::app::clear_signing::{
    ClearOperation, ClearShellResult, ClearSigning, ClearSigningView, Event as ClearEvent,
};
use vela_core::app::fee_policy::{FeeAssetView, FeeCall, FeeFailure, FeeTier, FeeView};
use vela_core::app::fee_speed::FeeSpeedView;
use vela_core::app::fee_tier_pref::FeeTierPref;
use vela_core::app::sign_pref::SignPref;
use vela_core::app::sign_request::{
    Event as SignEvent, SignAccountRef, SignApproveOpts, SignOperation, SignQuotedFee, SignRequest,
    SignShellResult, SignView,
};

use crate::ceremony::CeremonyChannel;
use crate::core_host::{CoreHost, Pending};
use crate::diag::{short, vlog};
use crate::executor::now_ms;
use crate::executor::passkey::WindowHandle;
use crate::executor::sign_request::{self as sign_executor, SignAnswer, SignContext};
use crate::executor::trusted_signer;
use crate::resident::{Answer, Machine, Sink};

use super::speed_control::{self, SpeedControl, SpeedHost};

impl SpeedHost for SigningHost {
    fn speed_control(&mut self) -> &mut SpeedControl {
        &mut self.speed
    }

    /// Spec 079: a quote that failed for a reason that can pass is asked
    /// again on the core's schedule.
    fn in_force_changed(&mut self, cx: &mut Context<Self>) {
        self.schedule_requote(cx);
    }

    fn unreadable(&mut self, cx: &mut Context<Self>) {
        self.schedule_requote(cx);
    }
}

/// Why the fee in force needs asking again, if it does: the core's failure,
/// or — the deployment read got no answer, so nothing reached the core — the
/// chain read's (spec 082 RJ13, G48): the chain's node, rate-limited or out
/// of reach, never "Vela" and never the person's network.
fn requote_failure(fee: &FeeView, chain_read: Option<FeeFailure>) -> Option<FeeFailure> {
    fee.failed.or(chain_read)
}

/// When the fee in force is asked again by itself (spec 079 FR-008, 082
/// RJ12, G47) — pure, on the host's clock, so the cadence is provable.
///
/// The waits are the core's (`fee_policy::requote_delay_ms`: 3 s, 6 s, then
/// every 8 s) and each is counted from the START of the re-quote before it,
/// never from its end: an ask is bounded by `REQUOTE_TIMEOUT_MS` (6 s), which
/// is shorter than every wait after the first, so an ask that hangs — a
/// black-holed relay holds its connection long after the relay is back — is
/// superseded by the next one, and the fee is back within 8 + 6 = 14 s of
/// the relay returning (SC-003). The device pass measured 15.8–19 s with the
/// waits counted from each failure and a hung ask waited out.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Requoter {
    /// Automatic re-quotes fired since the last good quote.
    fired: u32,
    /// When the next one is due (epoch ms).
    due_ms: Option<f64>,
    /// When the one out now began.
    out_since_ms: Option<f64>,
    /// Why the fee last failed — the schedule's step and the log's cause.
    last_failure: Option<FeeFailure>,
    /// The re-quote whose failure was last logged — one line each.
    reported: u32,
}

/// What the host does next about the fee in force.
#[derive(Clone, Debug, PartialEq)]
pub enum RequoteStep {
    /// Set the one timer for `due_ms`.
    Schedule { due_ms: f64 },
    /// Log it: `fee: quote failed chain=… cause=… re-quote #n in N ms`.
    Failed {
        cause: String,
        attempt: u32,
        in_ms: f64,
    },
    /// Ask again now — over a measurement still out, when `supersede` (the
    /// last re-quote ran past its bound).
    Fire { attempt: u32, supersede: bool },
    /// Log it: `fee: quote back chain=… after n re-quotes`.
    Back { after: u32 },
}

impl Requoter {
    /// The fee in force moved: a failure (`failure`), a measurement out, a
    /// good quote — or the sheet stopped taking a fee (`on_form` false).
    #[must_use]
    pub fn observe(
        &mut self,
        failure: Option<FeeFailure>,
        measuring: bool,
        on_form: bool,
        now_ms: f64,
    ) -> Vec<RequoteStep> {
        if !on_form {
            // Nothing re-prices under a slide that has gone.
            *self = Self::default();
            return Vec::new();
        }
        if measuring {
            return Vec::new();
        }
        let Some(failure) = failure else {
            let after = self.fired;
            *self = Self::default();
            return if after > 0 {
                vec![RequoteStep::Back { after }]
            } else {
                Vec::new()
            };
        };
        self.last_failure = Some(failure);
        let attempt = self.fired + 1;
        let mut steps = Vec::new();
        let due = match self.due_ms {
            Some(due) => due,
            None => {
                let Some(wait) = vela_core::app::fee_policy::requote_delay_ms(failure, attempt)
                else {
                    // No retry fixes it (no public key, a calculation that
                    // cannot come out): the row keeps its dash.
                    return Vec::new();
                };
                let due = (self.out_since_ms.unwrap_or(now_ms) + f64::from(wait)).max(now_ms);
                self.due_ms = Some(due);
                steps.push(RequoteStep::Schedule { due_ms: due });
                due
            }
        };
        if self.reported != attempt {
            self.reported = attempt;
            steps.push(RequoteStep::Failed {
                cause: format!("{failure:?}"),
                attempt,
                in_ms: (due - now_ms).max(0.0),
            });
        }
        steps
    }

    /// The timer set by the last `Schedule` fired at `now_ms`.
    #[must_use]
    pub fn due(
        &mut self,
        failure: Option<FeeFailure>,
        measuring: bool,
        on_form: bool,
        now_ms: f64,
    ) -> Vec<RequoteStep> {
        match self.due_ms {
            // Cancelled or moved: a stale timer.
            Some(due) if now_ms + 1.0 >= due => self.due_ms = None,
            _ => return Vec::new(),
        }
        if !on_form || (!measuring && failure.is_none()) {
            return self.observe(failure, measuring, on_form, now_ms);
        }
        let mut steps = Vec::new();
        let supersede = measuring;
        if measuring {
            if self.out_since_ms.is_none() {
                // A measurement the person started, not one of these: its
                // own answer reschedules.
                return Vec::new();
            }
            // The last re-quote is still out past its bound (RJ12).
            let cause = "timeout".to_owned();
            steps.push(RequoteStep::Failed {
                cause,
                attempt: self.fired,
                in_ms: 0.0,
            });
        }
        self.fired += 1;
        self.out_since_ms = Some(now_ms);
        steps.push(RequoteStep::Fire {
            attempt: self.fired,
            supersede,
        });
        // The next one, counted from this start: an answer cancels it.
        let failure = failure
            .or(self.last_failure)
            .unwrap_or(FeeFailure::QuoteUnavailable);
        if let Some(wait) = vela_core::app::fee_policy::requote_delay_ms(failure, self.fired + 1) {
            let due = now_ms + f64::from(wait);
            self.due_ms = Some(due);
            steps.push(RequoteStep::Schedule { due_ms: due });
        }
        steps
    }
}

/// The simulation's deltas as the fee machine reads them (spec 083 fee):
/// what the operation moves of each asset, native or by contract. The fee
/// machine weighs only its fee coins by them, each by that coin's OWN
/// `Transfer` logs (the contract that emitted them) or the node's trace of
/// native value — nothing a site's contract can emit on a coin's behalf.
pub(crate) fn fee_balance_changes(
    deltas: &[vela_core::app::token_trust::TrustAssetDelta],
) -> Vec<vela_core::app::fee_policy::FeeBalanceChange> {
    use vela_core::app::token_trust::TrustDeltaKind;
    deltas
        .iter()
        .filter_map(|delta| {
            let token = match delta.kind {
                TrustDeltaKind::Native => None,
                // A token move with no contract names no coin: never read as
                // the native one.
                TrustDeltaKind::Erc20 => Some(delta.token.clone()?),
            };
            Some(vela_core::app::fee_policy::FeeBalanceChange {
                token,
                delta: delta.delta.clone(),
            })
        })
        .collect()
}

/// Where a request stands, as far as closing its column goes (083): the
/// core's view, where its signature is, and whether a close already waits.
fn close_facts_of(
    view: &SignView,
    signature: crate::signing::status::Signature,
    closing: bool,
) -> crate::signing::status::CloseFacts {
    crate::signing::status::CloseFacts {
        swipe: view.swipe_action,
        running: view.is_signing || view.is_submitting,
        submitted: view.pending_op_hash.is_some(),
        failed: view.error.is_some(),
        signature,
        closing,
    }
}

/// The core kept a request open after its pipeline stopped — the ceremony
/// came back unsigned (dismissed, a QR nobody scanned) — so the form is the
/// person's again: the sheet up, nothing running, nothing failed, submitted
/// or answered (083).
fn back_on_form_of(view: &SignView, responded: bool) -> bool {
    view.surface == vela_core::app::sign_request::SignSurface::Sheet
        && !view.is_signing
        && !view.is_submitting
        && view.error.is_none()
        && view.pending_op_hash.is_none()
        && !responded
}

/// How often the column looks at its ceremony while a pipeline runs — the
/// cadence the send and onboarding already poll theirs at (083).
const CEREMONY_TICK_MS: u64 = 120;

/// The transport id of a request the WALLET made of itself. Its answer has no
/// page to go to; the column closing is the whole acknowledgement.
pub const WALLET_TRANSPORT: &str = "wallet";

/// The transport of a request the in-app browser forwarded. One browser, one
/// transport; the id must be stable for the life of the column, because it is
/// what `TransportDropped` names when the page that asked goes away.
pub const BROWSER_TRANSPORT: &str = "browser";

/// One dApp request, as the shell received it.
pub struct IncomingRequest {
    pub id: String,
    pub method: String,
    /// Raw JSON-RPC params, verbatim and untrusted.
    pub params_json: String,
    /// Read from the TRANSPORT, never from the page (`webview.rs`'s rule).
    pub origin: String,
    pub transport_id: String,
    /// The SITE's chain, as the browser machine keeps it per origin.
    pub chain_id: u32,
    /// The address the site was shown. The signer is pinned to it: a
    /// mismatch is refused (4100) rather than signed by whichever account is
    /// active. `None` for the wallet's own requests, which no site granted.
    pub granted_address: Option<String>,
}

/// A site's answer, for the browser machine to deliver.
pub struct TransportAnswer {
    pub id: String,
    pub payload: vela_core::app::sign_request::SignResponsePayload,
    /// Set when the answer IS a user-operation hash, so the page's later
    /// receipt lookups for it can be translated by the core.
    pub user_op_hash: Option<String>,
}

pub struct SigningHost {
    /// The transport the request arrived on — [`WALLET_TRANSPORT`] is the
    /// wallet asking itself, which is not a site and is not headed like one.
    pub transport_id: String,
    /// The request this column is for — what a `cancel_signing` names.
    pub request_id: String,
    /// The request has been answered. The column may still be showing (a
    /// receipt, an error), but the request is over, so another may take the
    /// column's place.
    pub responded: bool,
    /// Answers for a site, drained by the page.
    answers: Vec<TransportAnswer>,
    /// The fee coin list, open in the sheet (the web's `feeOpen`).
    pub fee_open: bool,
    sign: CoreHost<SignRequest>,
    pub view: SignView,
    clear: CoreHost<ClearSigning>,
    pub clear_view: ClearSigningView,
    guard: CoreHost<ApprovalGuard>,
    pub guard_view: GuardView,
    /// The fourth machine, and the speed control over it (spec 069) — the
    /// very one the send column runs. ONE session in force for the whole
    /// request: the sheet renders its view and the approve hands back the
    /// quote FROM it, so the figure somebody agreed to and the figure that
    /// gets signed cannot be two different numbers (this cut's second lesson,
    /// which the web recorded four failed integrations of). The other speeds'
    /// previews sit beside it, and a tapped one is promoted, not re-asked.
    speed: SpeedControl,
    /// WHO is asking and on WHAT chain, kept from the request that opened
    /// this host. The sheet's header is drawn from these — a signing screen
    /// naming the wrong site is the worst thing it can get wrong, and until
    /// spec 032 phase 22 it named the MOCK's site on every live request.
    pub origin: String,
    pub chain_id: u32,
    /// What the sheet can still say when the ladder decodes nothing. Parsed
    /// ONCE, here, from the same params the machines were told about: a second
    /// reading of an untrusted payload is a second answer to "what am I
    /// signing", and only one of them would be on screen.
    pub facts: crate::signing::live::RequestFacts,
    /// The request exactly as it arrived — its method and its params, kept so
    /// the sheet's "view raw data" can show what is being signed rather than
    /// only what somebody decoded from it. Never re-parsed for a second
    /// reading: `facts` is the one reading, and this is the text.
    pub raw: (String, String),
    ctx: SignContext,
    /// What the ceremony says to the screen: the phone's QR, the "check your
    /// phone" or "touch your key" prompt. The column draws them itself (083
    /// W19) — until 083 nothing read this, so a request signing by phone ran
    /// a 90 s scan behind "签名中…" with no code on screen to scan.
    channel: Arc<CeremonyChannel>,
    /// The ceremony watch is running (see [`Self::ensure_watcher`]).
    watching: bool,
    /// What the column last drew of the ceremony, so the watch redraws on a
    /// change rather than eight times a second. The whole touch prompt, not
    /// whether there is one: a key's "select" becoming "touch", or another
    /// key's name, is a change too (083 review).
    ceremony_seen: (
        crate::signing::status::Signature,
        Option<String>,
        Option<crate::ctap::usb::TouchRequest>,
    ),
    /// The phone's QR ran its whole window with nobody scanning (083 W19),
    /// the phone scanned and its connection never came up (H4), or it
    /// dropped once the phone was asked (H4 review). The request is still
    /// open — nothing was answered — and the column says which, with Retry,
    /// until the person chooses.
    pub phone_stop: Option<sign_executor::PhoneStop>,
    /// The column was closed while the signature was still to come
    /// ([`crate::signing::status::ClosePlan::AfterCancel`]): whatever the
    /// ceremony waits behind is stopped, and the close is made once the core
    /// has the ceremony back — a refusal (4001) if nothing was signed. Before
    /// 083 the scan ran on unseen for up to 90 s and the page then got
    /// "-32603 no phone answered…" (W19); told to the core at once as a
    /// dismiss, a prompt that came back unsigned answered nothing, ever
    /// (083 review).
    close_after_cancel: bool,
    /// The Trusted Signer's page asked for this approval's signature (083
    /// W11): once it stops asking, the signature is given.
    page_asked: bool,
    /// The core asked to close the column.
    pub closed: bool,
    /// What the CHAIN says this request would move, judged by `token_trust`.
    ///
    /// Empty until the simulation answers, and empty forever when it cannot —
    /// the sheet says so rather than showing an empty list as "nothing moves".
    pub sim: Vec<vela_core::app::token_trust::TrustSimJudgment>,
    /// What the simulation's answer says beside its balances (spec 082 RG6,
    /// the core's `sim_outcome::notice`): a revert is a danger, a node that
    /// could not check is a caution — a different sentence from "it ran and
    /// found nothing", which says nothing here.
    pub sim_notice: Option<vela_core::app::sim_outcome::SimNotice>,
    /// The submission already handed to the tracker: the records it closes
    /// (the key it is deduped by — spec 082: a may-have-been-sent op is
    /// handed over under its local hash, and the dedupe must not hang on a
    /// hash) and the hash it went under. The handoff stays on the view after
    /// it is taken; handing the same submission over on every render is a
    /// poll nobody asked for.
    handed_off: Option<(Vec<String>, String)>,
    /// The last hand-off fed to the tracker, whole (spec 082 RJ1): the
    /// write-ahead hands the op over (`maybe_sent`), and the relay taking it
    /// hands the SAME hash and records over again with `admitted`. Keyed on
    /// the records alone, that second one was never fed — the tracker never
    /// learnt the relay held the op, and two `not_found` answers past the
    /// grace ended an accepted payment "not sent".
    handoff_fed: Option<HandoffKey>,
    /// The last write-ahead withdrawal fed to the tracker (RJ1), once each.
    withdraw_fed: Option<vela_core::app::sign_request::SignTrackerWithdraw>,
    /// The tracker's entry for the in-flight op as last told to the core
    /// (spec 082 RJ4): `(op hash, status, tx hash)`.
    tracked_fed: Option<Tracked>,
    /// Spec 079: how the request ended, read off the answer the core sent —
    /// a message signed, a transaction landed, or the operation hash because
    /// the wait ran out. The page keeps it on screen after the core closes
    /// the column (the tick, or "not landed yet").
    pub ending: Option<vela_core::app::sign_request::SignEnding>,
    /// Spec 079: when this column first saw the operation accepted — the
    /// receipt's ring starts here until the tracker has its own clock.
    pub seen_submitted_ms: Option<f64>,
    /// The person approved this request (the slide, or the button that opens
    /// the Trusted Signer's page). A close after this refuses nothing.
    pub approved: bool,
    /// Spec 079: automatic re-quotes since the last good quote, and the
    /// number of the one scheduled — a newer schedule, an approval or a good
    /// quote makes an older timer a no-op.
    requoter: Requoter,
    /// The timer the requoter's last `Schedule` set; an older one is a no-op.
    requote_seq: u64,
    /// Submits running on a worker (the `Streaming` arm): counted from the
    /// dispatch until the core has their result. A request whose page left
    /// keeps its machines only while one runs (spec 082 RB2).
    streaming: u32,
}

/// Whether a request's machines keep running, unseen, once its column goes
/// (spec 079 FR-002, 082 RB2). Nothing is owed once the page has its answer.
/// A page that LEFT is owed nothing either — but a submit still running for
/// it may already have POSTed, and its `OpSubmitted` must reach the core so
/// the operation is recorded and followed; once that run ends, nothing is
/// left to do. Otherwise a request closed after its approval runs on until
/// the page has its answer.
#[must_use]
pub fn runs_unseen(
    approved: bool,
    responded: bool,
    asker_gone: bool,
    submit_running: bool,
) -> bool {
    if responded {
        return false;
    }
    if asker_gone {
        return submit_running;
    }
    approved
}

impl SigningHost {
    /// The Trusted Signer's waiting sheet and its last word (spec 071).
    pub fn trusted_signer(&self) -> Arc<trusted_signer::Channel> {
        Arc::clone(&self.ctx.trusted_signer)
    }

    /// Something on the Trusted Signer's channel changed: hand the browser the
    /// page if a ceremony asked for it, and redraw.
    fn trusted_signer_changed(&mut self, cx: &mut Context<Self>) {
        // The column was closed before this signature (083 review): its
        // channel is closed (`close`), so no wait goes on and none starts —
        // and a page asked for in that breath is not opened either.
        if self.close_after_cancel {
            let _unopened = self.ctx.trusted_signer.take_page();
            cx.notify();
            return;
        }
        if let Some(url) = self.ctx.trusted_signer.take_page() {
            cx.open_url(&url);
        }
        if self.ctx.trusted_signer.waiting() {
            self.page_asked = true;
        }
        cx.notify();
    }

    pub fn open(
        account: &Account,
        request: IncomingRequest,
        window_handle: WindowHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let channel = CeremonyChannel::new();
        let mut ctx = SignContext::new(account, channel.ceremony(window_handle));
        // A site's request is told to the Trusted Signer as the site's; the
        // wallet's own (the key backup) as the wallet's own send.
        ctx.site = (request.transport_id != WALLET_TRANSPORT).then(|| request.origin.clone());
        let (trusted_signer, changed) = trusted_signer::Channel::new();
        ctx.trusted_signer = trusted_signer;
        let sign = CoreHost::<SignRequest>::new();
        let clear = CoreHost::<ClearSigning>::new();
        let guard = CoreHost::<ApprovalGuard>::new();
        // The cores' own pristine views rather than a `Default` they do not
        // have: the shell must never invent a starting shape for a machine.
        let (view, clear_view, guard_view) = (sign.view(), clear.view(), guard.view());
        let mut host = Self {
            sim: Vec::new(),
            sim_notice: None,
            transport_id: request.transport_id.clone(),
            request_id: request.id.clone(),
            responded: false,
            answers: Vec::new(),
            fee_open: false,
            origin: request.origin.clone(),
            chain_id: request.chain_id,
            facts: facts_of(&request),
            raw: (request.method.clone(), request.params_json.clone()),
            sign,
            view,
            clear,
            clear_view,
            guard,
            guard_view,
            speed: SpeedControl::new(),
            ctx,
            channel,
            watching: false,
            ceremony_seen: Default::default(),
            phone_stop: None,
            close_after_cancel: false,
            page_asked: false,
            closed: false,
            handed_off: None,
            handoff_fed: None,
            withdraw_fed: None,
            tracked_fed: None,
            ending: None,
            seen_submitted_ms: None,
            approved: false,
            requoter: Requoter::default(),
            requote_seq: 0,
            streaming: 0,
        };
        // The stored default speed (spec 069), read now and followed while
        // the sheet is up: the column sits beside Settings, which may change it.
        let preference = crate::resident::resident::<FeeTierPref>(cx);
        cx.observe(&preference, |host, _, cx| {
            speed_control::configure(host, cx)
        })
        .detach();
        speed_control::reset(&mut host, cx);
        // Where this request signs is the account's sign-in route (founder,
        // 2026-09-26); the Trusted Signer's page, when it is one, as Settings
        // names it right now.
        let page = crate::resident::resident::<SignPref>(cx)
            .read(cx)
            .view()
            .signer_url;
        host.ctx.follow_sign_in(&page);
        // Spec 082 RJ4: the answer to the page follows what the tracker knows
        // of the op this request submitted.
        let tracker = crate::resident::resident::<vela_core::app::tx_tracker::TxTracker>(cx);
        cx.observe(&tracker, |host, _, cx| host.forward_tracked(cx))
            .detach();
        // The Trusted Signer's channel speaks up whenever a ceremony waits,
        // ends, or wants the page opened. The stream ends with the host.
        cx.spawn(async move |host, cx| {
            let mut changed = changed;
            while changed.next().await.is_some() {
                if host
                    .update(cx, |host, cx| host.trusted_signer_changed(cx))
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        host.begin(&request, &account.address, cx);
        host
    }

    /// One request, told to all three machines.
    ///
    /// They are told SEPARATELY and none of them waits for the others: the
    /// decode is a network round trip and the approval editor is not, so a
    /// sheet that opened only when the slowest finished would sit blank
    /// through every descriptor fetch.
    fn begin(&mut self, request: &IncomingRequest, wallet: &str, cx: &mut Context<Self>) {
        let now = now_ms();

        // The machine has to know the world before it can judge a request
        // against it. Without these two it refuses every transaction with
        // 4902 — "that chain is not added" — because as far as it knows, none
        // are. Found by running it: no test could, since the whole point is
        // what the machine is NOT told.
        self.dispatch_sign(
            SignEvent::NetworksChanged {
                chain_ids: known_chain_ids(),
            },
            cx,
        );
        self.dispatch_sign(
            SignEvent::AccountsChanged {
                accounts: vec![SignAccountRef {
                    address: wallet.to_owned(),
                    credential_id: self
                        .ctx
                        .keys
                        .first()
                        .map(|key| key.credential_id.clone())
                        .unwrap_or_default(),
                }],
                active_index: 0,
            },
            cx,
        );

        self.dispatch_sign(
            SignEvent::RequestArrived {
                id: request.id.clone(),
                method: request.method.clone(),
                params_json: request.params_json.clone(),
                origin: request.origin.clone(),
                transport_id: request.transport_id.clone(),
                dedicated_transport: true,
                per_request_chain: Some(request.chain_id),
                dapp: None,
                granted_address: request.granted_address.clone(),
                requested_address: None,
                request_ts_ms: None,
                now_ms: now,
            },
            cx,
        );

        // What it does. A transaction decodes from its call; typed data and a
        // plain message are their own rungs of the same ladder.
        let clear = clear_kickoff(
            &request.method,
            &request.params_json,
            request.chain_id,
            // The BROWSER's fact about who is asking, never the page's claim
            // — an empty origin is no origin, which is what the SIWE binding
            // check treats as unbindable rather than as a match.
            Some(request.origin.clone()).filter(|origin| !origin.is_empty()),
        );
        if let Some(event) = clear {
            self.dispatch_clear(event, cx);
        }

        // What it costs. Only a transaction has a fee.
        if let Some(calls) =
            crate::executor::sign_request::calls_of(&request.method, &request.params_json)
        {
            // And what it would DO. Asked of the chain, off the main thread,
            // and judged by `token_trust` before anything reaches the screen —
            // a simulation is untrusted input, so which deltas may carry a
            // confident number is never this file's call.
            self.simulate(request.chain_id, wallet.to_owned(), calls.clone(), cx);
            self.request_quote(request.chain_id, wallet.to_owned(), calls, cx);
        }

        // What it may be edited to. `read_only` is false: this sheet is the
        // one place the never-unlimited gate can be satisfied, and a guard
        // that cannot be edited would leave "unlimited" as the only option.
        self.dispatch_guard(
            GuardEvent::ApprovalDetected {
                method: request.method.clone(),
                params_json: request.params_json.clone(),
                chain_id: request.chain_id,
                wallet_address: Some(wallet.to_owned()),
                read_only: false,
                now_ms: now,
            },
            cx,
        );
    }

    /// Price this request.
    ///
    /// Only a transaction has a fee: a `personal_sign` costs nothing, and
    /// quoting one would put a network fee on a signature that never touches
    /// a chain.
    fn request_quote(
        &mut self,
        chain_id: u32,
        account: String,
        calls: Vec<FeeCall>,
        cx: &mut Context<Self>,
    ) {
        if calls.is_empty() {
            return;
        }
        let public_key_available = !self.ctx.keys.is_empty();
        // HOW FAST is the speed control's to say (spec 069): the person's
        // stored default — `fast` for everybody who never chose — until the
        // sheet's own control picks another. The quoted fee carries it to the
        // relay beside the amount.
        //
        // An indeterminate deployment read never reaches the core: guessing
        // "deployed" ships an operation without initCode, and guessing
        // "undeployed" attaches one to a live account. No quote is better
        // than a wrong one — the slide stays shut, which is what
        // `confirm_fee_ready: false` means.
        speed_control::ask(
            self,
            chain_id,
            account,
            public_key_available,
            calls,
            None,
            // Nobody chose the fee coin for this request yet: the fee machine
            // pays in one that can, and the approve carries the view's
            // `fee_token` — the coin it picked — exactly as it carries a tap.
            true,
            cx,
        );
    }

    /// Approve, carrying the fee THIS sheet displayed.
    ///
    /// The quote is read off `fee_view` — the same view the confirm card
    /// rendered — rather than re-asked. A second question would produce a
    /// second number, and the figure somebody agreed to would not be the
    /// figure that gets signed.
    /// Ask the chain what these calls would move, then ask the core which of
    /// the answer may be shown as a number.
    ///
    /// Both halves are blocking — an RPC round trip and a metadata multicall —
    /// so both happen on the background executor and each lands back here
    /// through the entity, the way every other slow answer in this shell does.
    /// The fee machine hears what the calls move as soon as the first half
    /// answers (spec 083 fee review): which coin can pay does not wait on the
    /// token names, and a quote priced before it arrives costs the relay one
    /// more simulation (USDC refused, then ETH).
    fn simulate(
        &mut self,
        chain_id: u32,
        wallet: String,
        calls: Vec<FeeCall>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |host, cx| {
            let fee_calls = calls.clone();
            let sim_wallet = wallet.clone();
            let outcome = cx
                .background_executor()
                .spawn(async move { crate::executor::sim::simulate(&sim_wallet, &calls, chain_id) })
                .await;
            let notice = vela_core::app::sim_outcome::notice(&outcome);
            let vela_core::app::sim_outcome::SimOutcome::Deltas { deltas } = outcome else {
                // A revert moves nothing, and a node that could not check says
                // nothing about what moves. NOT "nothing moves" — the notice is
                // the sentence for each.
                host.update(cx, |host, cx| {
                    host.sim = Vec::new();
                    host.sim_notice = notice;
                    cx.notify();
                })
                .ok();
                return;
            };
            // Spec 083 fee: what the operation moves decides which coins can
            // still pay its fee — a swap of all of the USDC cannot also pay
            // in USDC. Told before the tokens are judged, not after.
            let changes = fee_balance_changes(&deltas);
            let told = host.update(cx, |host, cx| {
                speed_control::balance_changes(host, fee_calls, changes, cx);
                cx.notify();
            });
            if told.is_err() {
                return;
            }
            let judgments = cx
                .background_executor()
                .spawn(
                    async move { crate::executor::token_trust::judge(&wallet, chain_id, deltas) },
                )
                .await;
            host.update(cx, |host, cx| {
                host.sim = judgments;
                host.sim_notice = notice;
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The fee row, tapped: with another coin to pay in, the list opens (or
    /// closes) here in the sheet — the web's `onfee` — even over a failed
    /// quote (spec 083 fee: the relay refused the operation with USDC paying,
    /// and ETH could pay); a failed quote with nothing else to choose is
    /// asked again. One coin and a quote: nothing to choose.
    pub fn fee_tapped(&mut self, cx: &mut Context<Self>) {
        use crate::signing::live::{FeeTap, fee_tap};
        match fee_tap(self.speed.fee_view()) {
            FeeTap::Coins => {
                self.fee_open = !self.fee_open;
                cx.notify();
            }
            // Measured again for real — the held readings dropped first.
            FeeTap::Requote => speed_control::refresh(self, cx),
            FeeTap::Nothing => {}
        }
    }

    /// A coin picked from the list (`None` = the native coin). The pick is a
    /// quote PARAMETER: the operation is re-priced in that coin — every speed
    /// of it, so a speed picked next is still paid in the coin chosen — and
    /// the approve carries `fee_token` from the same view.
    pub fn pick_fee(&mut self, token: Option<String>, cx: &mut Context<Self>) {
        self.fee_open = false;
        speed_control::choose_fee_token(self, token, cx);
    }

    pub fn approve(&mut self, cx: &mut Context<Self>) {
        self.approved = true;
        // The dApp's answer window starts at the tap (spec 082 RA12).
        self.ctx.approved_at_ms = Some(now_ms());
        self.phone_stop = None;
        // Nothing re-prices under a slide that has gone.
        self.requoter = Requoter::default();
        self.requote_seq += 1;
        let mut opts = approve_opts(self.speed.fee_view(), &self.clear_view, &self.guard_view);
        // 083 F1: the lines the sheet drew under "Balance changes" — none when
        // the simulation reverted or could not run (its notice stood there).
        opts.balance_changes = approved_changes(&self.sim, self.sim_notice.is_some());
        self.dispatch_sign(SignEvent::ApproveTapped { opts }, cx);
    }

    // -- the ceremony, in the column (083) -----------------------------------

    /// The phone's QR to draw, while a scan waits for it.
    pub fn qr_showing(&self) -> Option<String> {
        self.channel.qr_showing()
    }

    /// What a key is waiting for — or the phone, from the moment its advert
    /// is found: connecting, and then its prompt (083 H5).
    pub fn touch_waiting(&self) -> Option<crate::ctap::usb::TouchRequest> {
        self.channel.touch_waiting()
    }

    /// Where this approval's signature stands (083 W11): the Trusted
    /// Signer's page asking, then the passkey's own two flags. `Given` only
    /// for a signature that exists — a page that stopped asking because it
    /// was refused has signed nothing (083 review: the column's close reads
    /// `Given` as "the operation goes on").
    pub fn signature(&self) -> crate::signing::status::Signature {
        use crate::signing::status::Signature;
        let signer = &self.ctx.trusted_signer;
        if signer.waiting() {
            Signature::Asked
        } else if self.ctx.signature_done.load(Ordering::SeqCst)
            || (self.page_asked && signer.ended().is_none())
        {
            Signature::Given
        } else if self.ctx.signing_started.load(Ordering::SeqCst) || self.page_asked {
            Signature::Asked
        } else {
            Signature::NotYet
        }
    }

    /// The QR card's Cancel: the scan stops, and the request goes back to its
    /// form, unanswered — the core hears the passkey dismissed.
    pub fn cancel_qr(&mut self, cx: &mut Context<Self>) {
        self.channel.cancel_qr();
        cx.notify();
    }

    /// The touch card's Cancel, where the prompt offers one.
    pub fn cancel_touch(&mut self, cx: &mut Context<Self>) {
        self.channel.cancel_touch();
        cx.notify();
    }

    /// The phone card's Retry — a scan that ran out, a connection that never
    /// came up or one that dropped once the phone was asked (083): the same
    /// approval again, with a new QR.
    /// When the sheet could not be approved as it stands now (a quote gone
    /// stale into a failure), the form comes back instead, to say why.
    pub fn retry(&mut self, cx: &mut Context<Self>) {
        self.phone_stop = None;
        let armed = crate::signing::live::confirm_enabled(
            &self.view,
            &self.guard_view,
            &self.clear_view,
            self.speed.fee_view(),
            Some(self.speed.view().tier),
        );
        if armed {
            self.approve(cx);
        } else {
            cx.notify();
        }
    }

    /// Where this request stands, as far as closing its column goes.
    fn close_facts(&self) -> crate::signing::status::CloseFacts {
        close_facts_of(&self.view, self.signature(), self.close_after_cancel)
    }

    /// A close waits on the ceremony: the column is gone for the person, and
    /// must not come back while the ceremony winds down (083 review).
    pub fn closing(&self) -> bool {
        self.close_after_cancel
    }

    /// The column's close — its ✕ and the receipt's button. The core decides
    /// what it answers (a request not yet approved is refused, one already on
    /// its way is only no longer watched); this decides only WHEN it is told
    /// ([`crate::signing::status::close_plan`], 083). While the signature is
    /// still to come the core would take the close as the second — and a
    /// prompt that then came back unsigned answered the page nothing, ever
    /// (the review's case: ✕ over "正在准备交易...", then a QR in a column
    /// nobody drew, 90 s, silence). So the prompt is stopped, or never
    /// opened, and the close is made once the core has the ceremony back. A
    /// second close while one waits adds nothing.
    pub fn close(&mut self, cx: &mut Context<Self>) {
        use crate::signing::status::{ClosePlan, close_plan};
        match close_plan(&self.close_facts()) {
            ClosePlan::Ignore => {}
            ClosePlan::AfterCancel => {
                self.close_after_cancel = true;
                // No passkey prompt opens after this (`around_prompt`)…
                self.ctx.abandoned.store(true, Ordering::SeqCst);
                // …nor a Trusted Signer page: its channel is closed rather
                // than cancelled — an attempt not yet begun clears a cancel
                // as it starts, while a closed channel refuses it before any
                // page is asked for, and ends one already waiting. Closed is
                // what the column's Drop does anyway: this request ends with
                // this close, refused or on its way.
                self.ctx.trusted_signer.close();
                self.stop_prompts();
                // Watched until the ceremony is back: a QR posted after this
                // is taken down too.
                self.ensure_watcher(cx);
                cx.notify();
            }
            ClosePlan::Now => {
                self.phone_stop = None;
                self.dispatch_sign(SignEvent::SwipeDismissed, cx);
            }
        }
    }

    /// Whatever the passkey ceremony waits behind, stopped for a close (083
    /// review): the scan — and a key's exchange, which polls the same flag —
    /// even before a QR is up; a key's prompt that can be stopped; a PIN or
    /// wallet question the column never draws. Run again on every watch tick
    /// while the close waits: a QR posted after it clears the stop flag as it
    /// goes up. (The Trusted Signer's channel is closed once, in `close`.)
    fn stop_prompts(&self) {
        self.channel.cancel_qr();
        if self
            .channel
            .touch_waiting()
            .is_some_and(|waiting| waiting.cancellable)
        {
            self.channel.cancel_touch();
        }
        if self.channel.pending_pin().is_some() {
            self.channel.answer_pin(None);
        }
        if self.channel.pending_choice().is_some() {
            self.channel.answer_choice(None);
        }
    }

    /// Esc on the column (083, the owner's D1): never an answer to the page.
    /// Over the QR or a key prompt it is that card's Cancel — the request
    /// stays open, back on its form. Otherwise it reports whether the
    /// column may close: only where closing answers nothing, now
    /// ([`crate::signing::status::escape_closes`]) — never while a close
    /// already waits, nor while the signature is still to come (that close
    /// ends in a refusal too).
    pub fn escape(&mut self, cx: &mut Context<Self>) -> bool {
        let plan = crate::signing::status::close_plan(&self.close_facts());
        if plan == crate::signing::status::ClosePlan::Ignore {
            return false;
        }
        if self.channel.qr_showing().is_some() {
            self.cancel_qr(cx);
            return false;
        }
        if let Some(waiting) = self.channel.touch_waiting() {
            if waiting.cancellable {
                self.cancel_touch(cx);
            }
            return false;
        }
        crate::signing::status::escape_closes(self.view.swipe_action, plan)
    }

    /// The ceremony runs on a background thread that cannot wake the column:
    /// the QR going up, a prompt opening, the signature given. Watched every
    /// 120 ms (the cadence the send and onboarding use) while a pipeline runs
    /// and nothing has been submitted, and redrawn only on a change (083).
    fn ensure_watcher(&mut self, cx: &mut Context<Self>) {
        if self.watching {
            return;
        }
        self.watching = true;
        cx.spawn(async move |host, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(CEREMONY_TICK_MS))
                    .await;
                let go_on = host
                    .update(cx, |host, cx| host.watch_ceremony(cx))
                    .unwrap_or(false);
                if !go_on {
                    break;
                }
            }
        })
        .detach();
    }

    /// One look at the ceremony. Returns whether to keep looking.
    fn watch_ceremony(&mut self, cx: &mut Context<Self>) -> bool {
        // A close waits: whatever came up since is stopped too.
        if self.close_after_cancel {
            self.stop_prompts();
        }
        let seen = (
            self.signature(),
            self.channel.qr_showing(),
            self.channel.touch_waiting(),
        );
        if seen != self.ceremony_seen {
            self.ceremony_seen = seen;
            cx.notify();
        }
        let go_on = self.ceremony_ahead();
        if !go_on {
            self.watching = false;
        }
        go_on
    }

    fn pipeline_running(&self) -> bool {
        self.view.is_signing || self.view.is_submitting
    }

    /// A pipeline runs on a column still showing it, and nothing has been
    /// submitted: a prompt may still open.
    fn ceremony_ahead(&self) -> bool {
        self.pipeline_running() && !self.closed && self.view.pending_op_hash.is_none()
    }

    /// Back on the form with the request still open: nothing signing, nothing
    /// submitted, failed or answered.
    fn back_on_form(&self) -> bool {
        back_on_form_of(&self.view, self.responded) && !self.closed
    }

    /// The refresh control (spec 079): measure again, now — the held
    /// readings dropped first, so it is a new measurement.
    pub fn refresh_fee(&mut self, cx: &mut Context<Self>) {
        if self.on_form() {
            speed_control::refresh(self, cx);
        }
    }

    /// A measurement is out on the fee in force.
    pub fn fee_measuring(&self) -> bool {
        self.speed.measuring()
    }

    /// The fee in force was never asked of the core: its deployment read got
    /// no answer (spec 082 RJ13) — the chain read's failure, and the chain it
    /// names, for the fee row's words.
    pub fn fee_unanswered(&self) -> Option<crate::signing::live::ChainReadFailure> {
        self.speed
            .chain_read()
            .map(|failure| crate::signing::live::ChainReadFailure {
                failure,
                chain_id: self.chain_id,
            })
    }

    /// The person can still decide: the sheet is up, nothing is signing,
    /// and nothing has been approved or answered.
    fn on_form(&self) -> bool {
        self.view.surface == vela_core::app::sign_request::SignSurface::Sheet
            && !self.view.is_signing
            && !self.view.is_submitting
            && !self.approved
            && !self.responded
            && !self.closed
    }

    /// Spec 079 FR-008, 082 RJ12: a quote that failed for a reason that can
    /// pass is asked again by itself while the sheet is open and unapproved,
    /// on the requoter's clock. The device pass had the row read "点击重试"
    /// with the relay down and stay that way after it came back, and then
    /// (082) come back 15.8–19 s after the relay did, with no log line.
    fn schedule_requote(&mut self, cx: &mut Context<Self>) {
        let failure = requote_failure(self.speed.fee_view(), self.speed.chain_read());
        let steps =
            self.requoter
                .observe(failure, self.speed.measuring(), self.on_form(), now_ms());
        self.run_requote(steps, cx);
    }

    /// The requoter's timer fired.
    fn requote_due(&mut self, seq: u64, cx: &mut Context<Self>) {
        if seq != self.requote_seq {
            return;
        }
        let failure = requote_failure(self.speed.fee_view(), self.speed.chain_read());
        let steps = self
            .requoter
            .due(failure, self.speed.measuring(), self.on_form(), now_ms());
        self.run_requote(steps, cx);
    }

    /// Carry out what the requoter decided, and say it in the log (FR-018:
    /// the chain, the cause, which re-quote and when — never an address).
    fn run_requote(&mut self, steps: Vec<RequoteStep>, cx: &mut Context<Self>) {
        let chain = self.chain_id;
        for step in steps {
            match step {
                RequoteStep::Schedule { due_ms } => {
                    self.requote_seq += 1;
                    let seq = self.requote_seq;
                    #[allow(
                        clippy::cast_possible_truncation,
                        clippy::cast_sign_loss,
                        reason = "a wait of seconds"
                    )]
                    let wait = (due_ms - now_ms()).max(0.0) as u64;
                    cx.spawn(async move |host, cx| {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(wait))
                            .await;
                        host.update(cx, |host, cx| host.requote_due(seq, cx)).ok();
                    })
                    .detach();
                }
                RequoteStep::Failed {
                    cause,
                    attempt,
                    in_ms,
                } => vlog!(
                    "fee",
                    "quote failed chain={chain} cause={cause} re-quote #{attempt} in {in_ms:.0} ms"
                ),
                RequoteStep::Fire { attempt, supersede } => {
                    vlog!(
                        "fee",
                        "re-quote #{attempt} chain={chain}{}",
                        if supersede {
                            " (the last one ran past its bound)"
                        } else {
                            ""
                        }
                    );
                    if supersede {
                        speed_control::reask(self, cx);
                    } else {
                        speed_control::refresh(self, cx);
                    }
                }
                RequoteStep::Back { after } => vlog!(
                    "fee",
                    "quote back chain={chain} after {after} re-quote{}",
                    if after == 1 { "" } else { "s" }
                ),
            }
        }
    }

    // -- the speed control (spec 069) ----------------------------------------

    /// The fee in force, as the sheet's fee row reads it.
    pub fn fee_view(&self) -> &FeeView {
        self.speed.fee_view()
    }

    /// The speed control, as the core decided it.
    pub fn speed_view(&self) -> &FeeSpeedView {
        self.speed.view()
    }

    /// Every (tier, view) the options can format a fee with.
    pub fn speed_tier_views(&self) -> Vec<(FeeTier, FeeView)> {
        self.speed.tier_views()
    }

    /// Fold or unfold the control.
    pub fn toggle_speed(&mut self, cx: &mut Context<Self>) {
        speed_control::toggle(self, cx);
    }

    /// A tap on an option — one-shot, never the stored preference.
    pub fn pick_speed(&mut self, tier: FeeTier, cx: &mut Context<Self>) {
        speed_control::pick(self, tier, cx);
    }

    /// The page that asked is gone (spec 082 RB2): a submit already running
    /// stops before its ceremony, when the ceremony answers, or right before
    /// the relay POST — whichever it reaches next.
    pub fn asker_left(&self) {
        self.ctx.asker_left();
    }

    /// Has the page that asked gone?
    #[must_use]
    pub fn asker_gone(&self) -> bool {
        !self.ctx.still_asked()
    }

    /// Is a submit running for this request?
    #[must_use]
    pub fn submit_running(&self) -> bool {
        self.streaming > 0
    }

    /// [`runs_unseen`] for this request.
    #[must_use]
    pub fn owed_unseen(&self) -> bool {
        runs_unseen(
            self.approved,
            self.responded,
            self.asker_gone(),
            self.submit_running(),
        )
    }

    pub fn dispatch_sign(&mut self, event: SignEvent, cx: &mut Context<Self>) {
        // A pipeline (re)starting has asked for no signature yet — reset
        // BEFORE the core hands the work to a background thread that may
        // raise the flags at once (083 W11).
        if matches!(
            event,
            SignEvent::ApproveTapped { .. } | SignEvent::FundingCompleteTapped
        ) {
            self.ctx.prompt_reset();
            self.page_asked = false;
            // …and a key prompt dismissed last time must not cancel this
            // one: a ceremony built anew starts undismissed.
            self.ctx.ceremony = self.channel.ceremony(self.ctx.ceremony.window);
        }
        let pending = self.sign.dispatch(event);
        self.pump_sign(pending, cx);
    }

    pub fn dispatch_clear(&mut self, event: ClearEvent, cx: &mut Context<Self>) {
        let pending = self.clear.dispatch(event);
        self.pump_clear(pending, cx);
    }

    pub fn dispatch_guard(&mut self, event: GuardEvent, cx: &mut Context<Self>) {
        let pending = self.guard.dispatch(event);
        self.pump_guard(pending, cx);
    }

    fn resolve_sign(&mut self, id: u64, result: SignShellResult, cx: &mut Context<Self>) {
        // The dApp's record is on disk now — opened pending, or closed. The
        // feed re-reads at once rather than at its next tick, so Activity
        // shows the operation the moment the site's is on its way (083 H2).
        //
        // A re-read and nothing else: `ReconcileCompleted` reads the store
        // and never celebrates. `FocusTick` would also sweep every chain for
        // incoming transfers, twice per transaction and once per signature
        // (083 H2 review).
        let wrote_record = matches!(
            result,
            SignShellResult::RecordPersisted | SignShellResult::RecordUpdated
        );
        let pending = self.sign.resolve(id, result);
        self.pump_sign(pending, cx);
        if wrote_record {
            crate::executor::activity_feed::reconciled(1, cx);
        }
    }

    fn resolve_clear(&mut self, id: u64, result: ClearShellResult, cx: &mut Context<Self>) {
        let pending = self.clear.resolve(id, result);
        self.pump_clear(pending, cx);
    }

    fn resolve_guard(&mut self, id: u64, result: GuardShellResult, cx: &mut Context<Self>) {
        let pending = self.guard.resolve(id, result);
        self.pump_guard(pending, cx);
    }

    fn pump_sign(&mut self, pending: Vec<Pending<SignOperation>>, cx: &mut Context<Self>) {
        let was_running = self.pipeline_running();
        for effect in pending {
            let id = effect.id;
            match sign_executor::perform(&effect.operation, &self.ctx) {
                SignAnswer::Now(result) => self.resolve_sign(id, result, cx),
                SignAnswer::Blocking(work) => {
                    cx.spawn(async move |host, cx| {
                        let result = cx.background_executor().spawn(async move { work() }).await;
                        host.update(cx, |host, cx| host.resolve_sign(id, result, cx))
                            .ok();
                    })
                    .detach();
                }
                SignAnswer::Streaming(work) => {
                    self.streaming += 1;
                    let (tx, mut rx) = futures::channel::mpsc::unbounded();
                    cx.spawn(async move |host, cx| {
                        let work = cx
                            .background_executor()
                            .spawn(async move { work(&Sink::new(tx)) });
                        // The submitted hash reaches the core BEFORE the
                        // submit settles — that ordering is the whole reason
                        // this arm exists (spec 032 phase 12).
                        while let Some(event) = rx.next().await {
                            host.update(cx, |host, cx| host.dispatch_sign(event, cx))
                                .ok();
                        }
                        let result = work.await;
                        host.update(cx, |host, cx| {
                            // Over before the core hears it, so the redraw
                            // its answer causes sees nothing running.
                            host.streaming = host.streaming.saturating_sub(1);
                            host.resolve_sign(id, result, cx);
                        })
                        .ok();
                    })
                    .detach();
                }
                SignAnswer::Screen => self.answer_transport(id, &effect.operation, cx),
            }
        }
        self.view = self.sign.view();
        if self.view.pending_op_hash.is_some() && self.seen_submitted_ms.is_none() {
            self.seen_submitted_ms = Some(now_ms());
        }
        // A free upgrade is decided only while the person can still choose —
        // never under a slide that has already gone.
        let on_form = self.view.surface == vela_core::app::sign_request::SignSurface::Sheet
            && !self.view.is_signing
            && !self.view.is_submitting;
        speed_control::stage(self, on_form, cx);
        // The tracker, the moment the core has something to hand it.
        //
        // Its own words: "the shell feeds this to `tx_tracker::Event::Submitted`
        // the moment it appears (idempotent — the tracker merges by hash)".
        // Nobody was feeding it, so a dApp's transaction was submitted and then
        // FORGOTTEN: no pending row settling, no confirmation, nothing on the
        // next launch. The send column has done this since phase 4; this is the
        // same promise for the path a dApp drives.
        //
        // Spec 082 RJ1: once per hand-off VALUE — the write-ahead's, then the
        // relay's admission of the same op — and each withdrawal once.
        if let Some(handoff) = self.view.tracker_handoff.clone()
            && self.handoff_fed.as_ref() != Some(&handoff_key(&handoff))
        {
            self.handoff_fed = Some(handoff_key(&handoff));
            self.handed_off = Some((handoff.record_ids.clone(), handoff.user_op_hash.clone()));
            crate::executor::tracker::submitted(
                crate::executor::tracker::Handoff {
                    user_op_hash: handoff.user_op_hash,
                    record_ids: handoff.record_ids,
                    chain_id: handoff.chain_id,
                    maybe_sent: handoff.maybe_sent,
                    submit_block: handoff.submit_block,
                    admitted: handoff.admitted,
                },
                cx,
            );
        }
        if let Some(withdraw) = self.view.tracker_withdraw.clone()
            && self.withdraw_fed.as_ref() != Some(&withdraw)
        {
            self.withdraw_fed = Some(withdraw.clone());
            crate::executor::tracker::withdrawn(&withdraw.user_op_hash, &withdraw.record_ids, cx);
        }
        // `Hidden` is the core saying the request is over — the column closes
        // on the machine's word, never on a click this file interpreted.
        self.closed = self.view.surface == vela_core::app::sign_request::SignSurface::Hidden;
        // The tracker may already know how the op just handed over ended.
        self.forward_tracked(cx);
        // The ceremony came back with no signature — dismissed, a QR nobody
        // scanned, a phone that never connected — and the core kept the
        // request, unanswered (083).
        // The form is the person's again: not approved, and a slide that can
        // be slid, where the committed knob used to stay stuck at the end.
        if was_running && self.back_on_form() {
            self.approved = false;
            crate::signing::components::reset_slide();
            if let Some(stop) = self.ctx.take_phone_stop() {
                self.phone_stop = Some(stop);
            }
        }
        if self.ceremony_ahead() {
            self.ensure_watcher(cx);
        }
        cx.notify();
        // The close that waited, now that the ceremony is back: a refusal if
        // it came back empty, or — when the phone had already signed — the
        // ordinary close of an operation on its way.
        if self.close_after_cancel
            && crate::signing::status::waited_close_due(
                self.pipeline_running(),
                self.view.pending_op_hash.is_some(),
            )
        {
            self.close_after_cancel = false;
            self.phone_stop = None;
            self.dispatch_sign(SignEvent::SwipeDismissed, cx);
        }
    }

    /// Spec 082 RJ4 (G37): tell the core what the tracker knows of the op in
    /// flight — the answer to the page follows it (a landed op's tx hash, a
    /// refusal, a proven "not sent") instead of waiting out the receipt poll.
    /// Each change once; which change answers what is the core's.
    fn forward_tracked(&mut self, cx: &mut Context<Self>) {
        let Some(op) = self.view.pending_op_hash.clone() else {
            return;
        };
        let tracker = crate::resident::resident::<vela_core::app::tx_tracker::TxTracker>(cx);
        let entries = tracker.read(cx).view().entries;
        let Some((fed, event)) = tracked_event(&op, &entries, self.tracked_fed.as_ref(), now_ms())
        else {
            return;
        };
        vlog!(
            "dapp",
            "op={} tracker says {:?}{}",
            short(&op),
            fed.1,
            fed.2
                .as_deref()
                .map(|tx| format!(" tx={}", short(tx)))
                .unwrap_or_default()
        );
        self.tracked_fed = Some(fed);
        self.dispatch_sign(event, cx);
    }

    /// What the page takes away, exactly once.
    pub fn take_answers(&mut self) -> Vec<TransportAnswer> {
        std::mem::take(&mut self.answers)
    }

    /// The answer is the user operation this sheet submitted — the hash the
    /// tracker was handed, or the one the core is still waiting on. A
    /// transaction hash or a signature is not, and is not claimed to be.
    fn user_op_hash_of(
        &self,
        payload: &vela_core::app::sign_request::SignResponsePayload,
    ) -> Option<String> {
        let vela_core::app::sign_request::SignResponsePayload::Ok {
            result: Some(result),
        } = payload
        else {
            return None;
        };
        let submitted = [
            self.handed_off.as_ref().map(|(_, hash)| hash.as_str()),
            self.view.pending_op_hash.as_deref(),
            self.view
                .tracker_handoff
                .as_ref()
                .map(|handoff| handoff.user_op_hash.as_str()),
        ];
        submitted
            .into_iter()
            .flatten()
            .any(|hash| hash.eq_ignore_ascii_case(result))
            .then(|| result.clone())
    }

    /// The one screen-owned operation: answering the site.
    fn answer_transport(&mut self, id: u64, operation: &SignOperation, cx: &mut Context<Self>) {
        if let SignOperation::SendResponse {
            transport_id,
            id: request_id,
            payload,
        } = operation
        {
            self.responded = true;
            // RJ4: a receipt wait still running for this request stops — the
            // page has its one answer.
            self.ctx.core_answered();
            // Spec 079: what this answer says about how the request ended —
            // read BEFORE the sheet is cleared, while the hash this column
            // submitted is still known.
            let submitted = self.handed_off.clone().map(|(_, hash)| hash).or_else(|| {
                self.view.pending_op_hash.clone().or_else(|| {
                    self.view
                        .tracker_handoff
                        .as_ref()
                        .map(|handoff| handoff.user_op_hash.clone())
                })
            });
            self.ending =
                crate::signing::status::ending_of(&self.raw.0, payload, submitted.as_deref());
            // The wallet's own requests (the Ethereum backup, spec 062) ride
            // their own transport: there is no page to tell.
            if transport_id != WALLET_TRANSPORT {
                self.answers.push(TransportAnswer {
                    id: request_id.clone(),
                    payload: payload.clone(),
                    user_op_hash: self.user_op_hash_of(payload),
                });
            }
        }
        // Answered either way: the core sequences record-then-respond off this
        // acknowledgement, and withholding it would strand the request.
        self.resolve_sign(id, SignShellResult::Responded, cx);
    }

    fn pump_clear(&mut self, pending: Vec<Pending<ClearOperation>>, cx: &mut Context<Self>) {
        for effect in pending {
            let id = effect.id;
            match ClearSigning::perform(&effect.operation) {
                Answer::Now(result) => self.resolve_clear(id, result, cx),
                Answer::Blocking(work) => {
                    cx.spawn(async move |host, cx| {
                        let result = cx.background_executor().spawn(async move { work() }).await;
                        host.update(cx, |host, cx| host.resolve_clear(id, result, cx))
                            .ok();
                    })
                    .detach();
                }
                Answer::After(delay, result) => {
                    cx.spawn(async move |host, cx| {
                        cx.background_executor().timer(delay).await;
                        host.update(cx, |host, cx| host.resolve_clear(id, result, cx))
                            .ok();
                    })
                    .detach();
                }
                Answer::Streaming(_) => unreachable!("clear_signing reports nothing mid-flight"),
            }
        }
        self.clear_view = self.clear.view();
        cx.notify();
    }

    fn pump_guard(&mut self, pending: Vec<Pending<GuardOperation>>, cx: &mut Context<Self>) {
        for effect in pending {
            let id = effect.id;
            match ApprovalGuard::perform(&effect.operation) {
                Answer::Now(result) => self.resolve_guard(id, result, cx),
                Answer::Blocking(work) => {
                    cx.spawn(async move |host, cx| {
                        let result = cx.background_executor().spawn(async move { work() }).await;
                        host.update(cx, |host, cx| host.resolve_guard(id, result, cx))
                            .ok();
                    })
                    .detach();
                }
                Answer::After(delay, result) => {
                    cx.spawn(async move |host, cx| {
                        cx.background_executor().timer(delay).await;
                        host.update(cx, |host, cx| host.resolve_guard(id, result, cx))
                            .ok();
                    })
                    .detach();
                }
                Answer::Streaming(_) => unreachable!("approval_guard reports nothing mid-flight"),
            }
        }
        self.guard_view = self.guard.view();
        cx.notify();
    }
}

impl Drop for SigningHost {
    fn drop(&mut self) {
        release(&self.ctx.trusted_signer, &self.channel);
    }
}

/// One hand-off as the tracker must hear it — every field that can change
/// between two hand-offs of the same op (spec 082 RJ1, the review's key):
/// its hash, its records, whether it may have been sent, and whether the
/// relay has taken it.
type HandoffKey = (String, Vec<String>, bool, bool);

fn handoff_key(handoff: &vela_core::app::sign_request::SignTrackerHandoff) -> HandoffKey {
    (
        handoff.user_op_hash.to_lowercase(),
        handoff.record_ids.clone(),
        handoff.maybe_sent,
        handoff.admitted,
    )
}

/// What the core was last told of the op in flight: its hash, the tracker's
/// status and the tx hash it named (spec 082 RJ4).
type Tracked = (
    String,
    vela_core::app::tx_tracker::TrackStatus,
    Option<String>,
);

/// The tracker's entry for `op` as the core's `OpTracked`, when it changed
/// since `fed` (spec 082 RJ4) — `None` while the tracker has no entry for it
/// or nothing moved.
fn tracked_event(
    op: &str,
    entries: &[vela_core::app::tx_tracker::TrackEntryView],
    fed: Option<&Tracked>,
    now_ms: f64,
) -> Option<(Tracked, SignEvent)> {
    let entry = entries
        .iter()
        .find(|entry| entry.user_op_hash.eq_ignore_ascii_case(op))?;
    let now = (op.to_lowercase(), entry.status, entry.tx_hash.clone());
    if fed == Some(&now) {
        return None;
    }
    let event = SignEvent::OpTracked {
        user_op_hash: entry.user_op_hash.clone(),
        status: entry.status,
        tx_hash: entry.tx_hash.clone(),
        now_ms,
    };
    Some((now, event))
}

/// The column is gone: a Trusted Signer still waiting stops now rather than
/// holding a port for five minutes nobody can see — and so does a phone scan,
/// and a PIN or wallet question nobody can answer (083 W19).
fn release(trusted_signer: &trusted_signer::Channel, channel: &CeremonyChannel) {
    trusted_signer.close();
    channel.close();
}

/// Every chain this wallet can act on: the built-ins plus whatever was added.
///
/// The same list the network settings show, because a request for a chain the
/// settings say is present must not be refused as absent.
pub fn known_chain_ids() -> Vec<u32> {
    let mut ids: Vec<u32> = vela_core::app::network_admin::BUILTIN_CHAINS
        .iter()
        .map(|chain| chain.chain_id)
        .collect();
    for custom in crate::executor::network_admin::read_store_custom_chain_ids() {
        if !ids.contains(&custom) {
            ids.push(custom);
        }
    }
    ids
}

/// The first call's `to` / `data` / `value`, for the decoder.
///
/// What the confirm signs, assembled from the three views on screen.
///
/// A pure function so the one rule that matters here can be tested without a
/// window: **invariant ⑨** — when `approval_guard` rewrote the request, those
/// params are what gets signed, submitted and recorded. This carried `None`
/// until spec 032 phase 30, which meant a cap somebody chose would have been
/// discarded and the site's original ask signed instead.
///
/// The quote comes from the fee view that RENDERED the confirm card rather
/// than from a fresh question, for the same reason spelled out in this cut's
/// second lesson: a second question produces a second number, and the figure
/// somebody agreed to would not be the figure that gets signed.
fn approve_opts(fee: &FeeView, clear: &ClearSigningView, guard: &GuardView) -> SignApproveOpts {
    SignApproveOpts {
        max_fee_per_gas: fee
            .fee
            .as_ref()
            .map(|estimate| estimate.max_fee_per_gas.clone()),
        bundler_cost_wei: None,
        // The coin the sheet quoted in, from the SAME view: an operation
        // priced in USDC and submitted without its fee leg is a different
        // operation from the one somebody agreed to.
        gas_fee_token: fee.fee_token.clone(),
        // In the paying coin's own units — the send core's rule
        // (`submit_user_op`): an ERC-20 fee is its `amount`, never
        // `total_wei`, which is 0 for one.
        quoted_fee: fee.fee.as_ref().map(|estimate| SignQuotedFee {
            amount: match &estimate.fee_asset {
                FeeAssetView::Erc20 { amount, .. } => amount.clone(),
                FeeAssetView::Native => estimate.total_wei.clone(),
            },
            recipient: estimate.fee_recipient.clone().unwrap_or_default(),
            // The speed this very estimate was priced at — named on the wire
            // beside the amount (spec 069); the core drops a `rapid`.
            tier: Some(estimate.tier),
        }),
        fee_collector: None,
        params_override_json: guard.rewritten_params_json.clone(),
        intent: recorded_intent(clear),
        // The guard showed an unbounded amount and it was kept as the site
        // asked — the submit guard's only waiver, copied from the view that
        // drew it, never decided here.
        unlimited_approved: guard.unlimited_consented,
        // The simulation is the column's, not these views': `approve` adds
        // it (`approved_changes`).
        balance_changes: None,
    }
}

/// What the record keeps of the sheet's "Balance changes" (083 F1): the
/// judgments this column drew there (`sim_blocks`), exactly as they stood
/// when the slide fired — the one account of the operation's money that no
/// page wrote, and the one Activity shows for it. Nothing when the
/// simulation could not answer, or had not yet: the sheet showed no lines,
/// so the record keeps none and the row draws as it always has.
fn approved_changes(
    sim: &[vela_core::app::token_trust::TrustSimJudgment],
    unavailable: bool,
) -> Option<Vec<vela_core::app::token_trust::TrustSimJudgment>> {
    (!unavailable && !sim.is_empty()).then(|| sim.to_vec())
}

/// The intent the record keeps, which Activity shows as the row's title in
/// the wallet's own voice (083 H2).
///
/// Only a reading the sheet did not mark best-effort: a function name
/// recovered from the public selector database is whatever the contract's
/// deployer called it, and the sheet showed it under a caution. A plain title
/// in Activity would drop that caution, so such a call records no intent and
/// reads "Contract interaction" (083 H2 review). A plain native send has no
/// reading at all; the core records "Send" for it.
fn recorded_intent(clear: &ClearSigningView) -> Option<String> {
    clear
        .result
        .as_ref()
        .filter(|result| result.verified || !result.best_effort)
        .map(|result| result.intent.clone())
}

/// The blind rung's two facts: who it goes to, and how many bytes of calldata
/// nobody could read — a lone call's (or a one-call batch's, drawn as that
/// call). A longer batch names each call's own in its `ClearBatchCall`.
fn facts_of(request: &IncomingRequest) -> crate::signing::live::RequestFacts {
    let call = first_call(&request.method, &request.params_json);
    let data = call.as_ref().and_then(|c| c.1.clone()).unwrap_or_default();
    crate::signing::live::RequestFacts {
        to: call.and_then(|c| c.0),
        // Hex, so two characters per byte; an odd tail is a malformed payload
        // and rounds DOWN rather than claiming a byte that is not there.
        data_bytes: data.trim_start_matches("0x").len() / 2,
        native_symbol: vela_core::app::network_admin::builtin_native_symbol(request.chain_id)
            .unwrap_or("—")
            .to_owned(),
    }
}

/// Which rung of the clear-signing ladder a request climbs. A batch is read
/// call by call by the core and drawn as the per-call panorama (CS26, 089 S1).
///
/// Four methods, three surfaces — and the fourth, `personal_sign`, was the one
/// this shell never named. The comment above the old match said "typed data
/// and a plain message are their own rungs of the same ladder" while the match
/// had no arm for the message: a dApp asking somebody to sign a login had its
/// text, its SIWE fields, its domain binding and its danger class all computed
/// by the core and then thrown away, because nothing started the machine.
///
/// `eth_sign` is deliberately its own method and NOT the calm message view:
/// it signs an OPAQUE hash, and the core gives it the hard-warning surface
/// (its own note, from `SigningSheet.tsx:465-470`).
///
/// A function rather than an inline match so the mapping can be tested without
/// a window, a transport or a dApp.
#[must_use]
pub fn clear_kickoff(
    method: &str,
    params_json: &str,
    chain_id: u32,
    origin: Option<String>,
) -> Option<ClearEvent> {
    use vela_core::app::clear_signing::{ClearLocale, ClearSignMethod};
    match method {
        // 089 S1: a batch goes over whole — the core reads EVERY call, so the
        // sheet can never describe call 1 while signing them all.
        "wallet_sendCalls" => Some(ClearEvent::ResolveBatch {
            params_json: params_json.to_owned(),
            chain_id,
            locale: ClearLocale::default(),
        }),
        "eth_sendTransaction" => {
            let call = first_call(method, params_json);
            Some(ClearEvent::ResolveTransaction {
                to: call.as_ref().and_then(|c| c.0.clone()),
                data: call.as_ref().and_then(|c| c.1.clone()),
                value: call.and_then(|c| c.2),
                chain_id,
                locale: ClearLocale::default(),
            })
        }
        "eth_signTypedData_v4"
        | "eth_signTypedData_v3"
        | "eth_signTypedData_v1"
        | "eth_signTypedData" => Some(ClearEvent::ResolveTypedData {
            typed_data_json: typed_data_of(method, params_json),
            chain_id,
            locale: ClearLocale::default(),
        }),
        "personal_sign" | "eth_sign" => Some(ClearEvent::MessagePresented {
            method: if method == "eth_sign" {
                ClearSignMethod::EthSign
            } else {
                ClearSignMethod::PersonalSign
            },
            // The params AS SENT. The core does the hex/text split, the SIWE
            // parse and the binding check; a shell that pre-decoded here would
            // be deciding what the person is being shown.
            params: string_params(params_json),
            request_origin: origin,
        }),
        _ => None,
    }
}

/// A request's params as the strings the site sent, in order.
///
/// Anything that is not a string is dropped rather than stringified: the
/// message machine reads `params[0]`/`params[1]` positionally, and an object
/// coerced into that list would shift the message and the address by one.
fn string_params(params_json: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(params_json)
        .ok()
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| item.as_str().map(str::to_owned))
        .collect()
}

/// The call the decode and the blind rung read: a batch's first leg, or a
/// transaction's own call — chosen by METHOD, the way the executor's
/// `calls_of` chooses what gets submitted.
///
/// It used to follow a `calls` key whatever the method (083 review): an
/// `eth_sendTransaction` carrying a stray `"calls":[{…harmless…}]` beside a
/// malicious top-level call had its headline decoded from the harmless one
/// while the malicious one was signed. And a `wallet_sendCalls` with no
/// `calls` is nothing — its envelope is never submitted, so it is not read.
fn first_call(
    method: &str,
    params_json: &str,
) -> Option<(Option<String>, Option<String>, Option<String>)> {
    let params: serde_json::Value = serde_json::from_str(params_json).ok()?;
    let first = params.get(0)?;
    let call = if method == "wallet_sendCalls" {
        first.get("calls")?.get(0)?
    } else {
        first
    };
    let field = |name: &str| {
        call.get(name)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    };
    Some((field("to"), field("data"), value_text(call.get("value"))))
}

/// The first call's `value` as the core is told it (spec 082 RC6): a string
/// as written, a present non-string (a JSON number) as its text — so the core
/// refuses it rather than reading it as absent, which drew a calm "0" over a
/// call that moves 1000 wei — and nothing for an absent or `null` one.
fn value_text(value: Option<&serde_json::Value>) -> Option<String> {
    match value? {
        serde_json::Value::Null => None,
        serde_json::Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

/// The ONE document the request is read as — the core's reading, the same
/// bytes the passkey's digest covers. The audit of 2026-10-01: reading
/// `params[1]` here previewed the benign half of a legacy
/// `[malicious, benign]` while the passkey signed the malicious one. Empty
/// when the request is not a valid typed-data request — the core refuses it
/// before a sheet.
fn typed_data_of(method: &str, params_json: &str) -> String {
    vela_core::typed_data_request::document_json_of(method, params_json).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::fee_policy::FeePolicy;

    /// Spec 079 FR-002 and 082 RB2: which requests keep their machines,
    /// unseen, once their column goes. One closed after the approval keeps
    /// them until the page has its answer. One whose page LEFT keeps them only
    /// while its submit is still running — a POST that already went must
    /// reach the core, so it is recorded and handed to the tracker, never
    /// dropped with the column — and lets them go once that run ends.
    #[test]
    fn a_request_runs_unseen_only_while_something_is_owed() {
        // approved, responded, asker gone, submit running
        assert!(
            runs_unseen(true, false, false, false),
            "FR-002: the answer is owed"
        );
        assert!(
            !runs_unseen(true, true, false, true),
            "answered: nothing owed"
        );
        assert!(!runs_unseen(false, false, false, false), "never approved");
        assert!(
            runs_unseen(true, false, true, true),
            "the page left mid-submit: the op may be on its way"
        );
        assert!(
            !runs_unseen(true, false, true, false),
            "the page left and nothing runs: nothing to record, nobody to answer"
        );
    }

    /// The requoter against a relay that is down until `up_at` and then
    /// answers `answer_ms` after each ask; while down, an ask either fails
    /// at once (refused) or hangs for good (black-holed — a connection made
    /// while the relay was down is not revived by its return). Answers when
    /// the fee is back, on a fake clock.
    fn fee_back_at(up_at: f64, answer_ms: f64, hangs: bool) -> f64 {
        let mut requoter = Requoter::default();
        let mut timer: Option<f64> = None;
        // The quote out now: (asked at, answers at — `None` hangs, ok?).
        let mut out: Option<(f64, Option<f64>, bool)> = None;
        let mut failed_at: Option<f64> = Some(0.0);
        let failure = Some(FeeFailure::QuoteUnavailable);
        let apply = |steps: Vec<RequoteStep>,
                     now: f64,
                     timer: &mut Option<f64>,
                     out: &mut Option<(f64, Option<f64>, bool)>| {
            for step in steps {
                match step {
                    RequoteStep::Schedule { due_ms } => *timer = Some(due_ms),
                    RequoteStep::Fire { .. } => {
                        *out = Some(if now >= up_at {
                            (now, Some(now + answer_ms), true)
                        } else if hangs {
                            (now, None, false)
                        } else {
                            (now, Some(now + 300.0), false)
                        });
                    }
                    RequoteStep::Failed { .. } | RequoteStep::Back { .. } => {}
                }
            }
        };
        for _ in 0..200 {
            let answer_at = out.and_then(|(_, at, _)| at);
            let next = [failed_at, timer, answer_at]
                .into_iter()
                .flatten()
                .fold(f64::INFINITY, f64::min);
            assert!(
                next.is_finite(),
                "nothing left to happen: the fee never came back"
            );
            if failed_at == Some(next) {
                failed_at = None;
                let steps = requoter.observe(failure, false, true, next);
                apply(steps, next, &mut timer, &mut out);
            } else if answer_at == Some(next) {
                let (_, _, ok) = out.take().unwrap_or((0.0, None, false));
                if ok {
                    let steps = requoter.observe(None, false, true, next);
                    assert!(
                        matches!(steps.as_slice(), [RequoteStep::Back { .. }]),
                        "{steps:?}"
                    );
                    return next;
                }
                let steps = requoter.observe(failure, false, true, next);
                apply(steps, next, &mut timer, &mut out);
            } else {
                timer = None;
                let measuring = out.is_some();
                let steps = requoter.due(failure, measuring, true, next);
                apply(steps, next, &mut timer, &mut out);
            }
        }
        unreachable!("the fee never came back")
    }

    /// Spec 082 RJ12 (G47, SC-003): the fee is back within 14 s of the relay
    /// returning — whenever it returns, however long its answer takes up to
    /// the 6 s bound, and even when the ask out at that moment hangs. The
    /// device pass measured 15.8–19 s (the 12 s and 15 s steps, counted from
    /// each failure, and a hung ask waited out).
    #[test]
    fn the_fee_is_back_within_fourteen_seconds_of_the_relay() {
        for hangs in [false, true] {
            for answer_ms in [200.0, 2_000.0, 5_000.0, 6_000.0] {
                let mut up_at = 0.0;
                while up_at < 60_000.0 {
                    let back = fee_back_at(up_at, answer_ms, hangs);
                    assert!(
                        back - up_at <= 14_000.0,
                        "relay back at {up_at} ms, fee at {back} ms \
                         (answer {answer_ms} ms, hangs {hangs})"
                    );
                    up_at += 250.0;
                }
            }
        }
    }

    /// The core's waits (3 s, 6 s, then 8 s), each from the start of the
    /// re-quote before it; one line per failure; the fee's return logged
    /// with the count.
    #[test]
    fn a_failed_quote_is_asked_again_on_the_cores_schedule() {
        let failure = Some(FeeFailure::QuoteUnavailable);
        let mut requoter = Requoter::default();
        let steps = requoter.observe(failure, false, true, 1_000.0);
        assert_eq!(
            steps,
            vec![
                RequoteStep::Schedule { due_ms: 4_000.0 },
                RequoteStep::Failed {
                    cause: "QuoteUnavailable".to_owned(),
                    attempt: 1,
                    in_ms: 3_000.0,
                },
            ]
        );
        assert!(
            requoter.observe(failure, false, true, 1_500.0).is_empty(),
            "one line per failure"
        );
        assert_eq!(
            requoter.due(failure, false, true, 4_000.0),
            vec![
                RequoteStep::Fire {
                    attempt: 1,
                    supersede: false,
                },
                RequoteStep::Schedule { due_ms: 10_000.0 },
            ]
        );
        // Re-quote #1 fails half a second in: #2 is still 6 s from its start.
        assert_eq!(
            requoter.observe(failure, false, true, 4_500.0),
            vec![RequoteStep::Failed {
                cause: "QuoteUnavailable".to_owned(),
                attempt: 2,
                in_ms: 5_500.0,
            }]
        );
        let _ = requoter.due(failure, false, true, 10_000.0);
        // #2 hangs past its bound: #3 supersedes it 8 s after it began.
        let steps = requoter.due(failure, true, true, 18_000.0);
        assert_eq!(
            steps,
            vec![
                RequoteStep::Failed {
                    cause: "timeout".to_owned(),
                    attempt: 2,
                    in_ms: 0.0,
                },
                RequoteStep::Fire {
                    attempt: 3,
                    supersede: true,
                },
                RequoteStep::Schedule { due_ms: 26_000.0 },
            ]
        );
        assert_eq!(
            requoter.observe(None, false, true, 19_000.0),
            vec![RequoteStep::Back { after: 3 }]
        );
        assert_eq!(requoter, Requoter::default(), "the count starts over");
        // The timer set for #4 finds nothing to do.
        assert!(requoter.due(None, false, true, 26_000.0).is_empty());
    }

    /// Never under a slide that has gone, never over a measurement the
    /// person started, never for a failure no retry fixes, and nothing to do
    /// when the quote is good.
    #[test]
    fn the_requote_stops_where_it_must() {
        let unreachable = Some(FeeFailure::QuoteUnavailable);
        let mut requoter = Requoter::default();
        assert!(
            requoter.observe(unreachable, false, false, 0.0).is_empty(),
            "approved"
        );
        assert!(
            requoter.observe(unreachable, true, true, 0.0).is_empty(),
            "measuring"
        );
        for unfixable in [FeeFailure::MissingPublicKey, FeeFailure::CalculationFailed] {
            assert!(
                Requoter::default()
                    .observe(Some(unfixable), false, true, 0.0)
                    .is_empty(),
                "{unfixable:?}"
            );
        }
        assert!(
            Requoter::default()
                .observe(None, false, true, 0.0)
                .is_empty()
        );

        // Scheduled, then the slide goes: the timer finds nothing to do.
        let mut requoter = Requoter::default();
        let _ = requoter.observe(unreachable, false, true, 0.0);
        assert!(requoter.due(unreachable, false, false, 3_000.0).is_empty());
        assert!(requoter.due(unreachable, false, true, 3_000.0).is_empty());

        // A refresh the person tapped is out when the timer fires: it is
        // theirs to finish; its own answer schedules the next.
        let mut requoter = Requoter::default();
        let _ = requoter.observe(unreachable, false, true, 0.0);
        assert!(requoter.due(unreachable, true, true, 3_000.0).is_empty());
        assert!(
            !requoter
                .observe(unreachable, false, true, 3_500.0)
                .is_empty()
        );
    }

    /// Spec 083 fee: the simulation's deltas reach the fee machine as what the
    /// operation moves of each coin — native as `None`, a token by its
    /// contract — and a token move that names no contract is never read as
    /// the native coin.
    #[test]
    fn the_simulated_deltas_are_the_fee_machines_balance_changes() {
        use vela_core::app::fee_policy::FeeBalanceChange;
        use vela_core::app::token_trust::{TrustAssetDelta, TrustDeltaKind};
        let usdc = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";
        let changes = fee_balance_changes(&[
            TrustAssetDelta {
                kind: TrustDeltaKind::Erc20,
                token: Some(usdc.to_owned()),
                delta: "-271741".to_owned(),
            },
            TrustAssetDelta {
                kind: TrustDeltaKind::Native,
                token: None,
                delta: "101000000000000".to_owned(),
            },
            TrustAssetDelta {
                kind: TrustDeltaKind::Erc20,
                token: None,
                delta: "-1".to_owned(),
            },
        ]);
        assert_eq!(
            changes,
            vec![
                FeeBalanceChange {
                    token: Some(usdc.to_owned()),
                    delta: "-271741".to_owned(),
                },
                FeeBalanceChange {
                    token: None,
                    delta: "101000000000000".to_owned(),
                },
            ]
        );
    }

    /// 083 F1: the approve carries exactly the lines the sheet drew under
    /// "Balance changes" — the outflow with its figure, an unverified inflow
    /// with none — and nothing when there were none to draw.
    #[test]
    fn the_approve_carries_the_balance_changes_the_sheet_drew() {
        use vela_core::app::token_trust::TrustSimJudgment as J;
        let drawn = vec![
            J::Erc20Trusted {
                token: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_owned(),
                delta: "-100000".to_owned(),
                symbol: "USDC".to_owned(),
                decimals: 6,
                in_trusted_set: true,
            },
            J::Native {
                delta: "37000000000000".to_owned(),
            },
            J::Erc20Unverified {
                token: Some("0xbad".to_owned()),
                delta: "1000000000000000000000".to_owned(),
            },
        ];
        assert_eq!(approved_changes(&drawn, false), Some(drawn.clone()));
        // "Could not check" drew a warning, not lines; nothing measured yet
        // drew nothing.
        assert_eq!(approved_changes(&drawn, true), None);
        assert_eq!(approved_changes(&[], false), None);
    }

    /// Spec 082 RJ13 (G48): a deployment read that got no answer never
    /// reached the core. It is the chain read's failure — rate-limited or
    /// not, from the pool's own signal — and asked again on the same
    /// schedule; never the relay's `QuoteUnavailable`.
    #[test]
    fn an_unanswered_read_is_a_chain_read() {
        let quiet = crate::core_host::CoreHost::<FeePolicy>::new().view();
        assert_eq!(requote_failure(&quiet, None), None);
        let limited = FeeFailure::ChainRead { rate_limited: true };
        assert_eq!(requote_failure(&quiet, Some(limited)), Some(limited));
        assert_eq!(
            vela_core::app::fee_policy::failure_reason_key(limited),
            Some("home.balanceDetailStatusRetrying")
        );
        assert!(
            !Requoter::default()
                .observe(Some(limited), false, true, 0.0)
                .is_empty(),
            "asked again by itself"
        );
        let failed = FeeView {
            failed: Some(FeeFailure::EstimateFailed),
            ..quiet
        };
        assert_eq!(
            requote_failure(&failed, Some(limited)),
            Some(FeeFailure::EstimateFailed),
            "the core's own reason first"
        );
    }

    /// Spec 082 RJ1 (the review's key): the write-ahead hands the op over
    /// "may have been sent", and the relay taking it hands the SAME hash and
    /// records over again, admitted. Keyed on the records alone the second
    /// was never fed, and an accepted payment could end "not sent".
    #[test]
    fn the_admitted_hand_off_is_a_new_one() {
        use vela_core::app::sign_request::SignTrackerHandoff;
        let ahead = SignTrackerHandoff {
            user_op_hash: "0xOP".to_owned(),
            record_ids: vec!["dapp-1-tx".to_owned()],
            chain_id: 100,
            maybe_sent: true,
            submit_block: Some(48_487_620),
            admitted: false,
        };
        let admitted = SignTrackerHandoff {
            maybe_sent: false,
            admitted: true,
            ..ahead.clone()
        };
        assert_ne!(handoff_key(&ahead), handoff_key(&admitted));
        assert_eq!(
            handoff_key(&ahead),
            handoff_key(&SignTrackerHandoff {
                user_op_hash: "0xop".to_owned(),
                ..ahead.clone()
            }),
            "the same hand-off, whatever the hash's case"
        );
    }

    /// Spec 082 RJ4 (G37): the tracker's entry for the op in flight reaches
    /// the core once per change — and a Rejected one is answered -32603
    /// "refused", once, the late receipt wait's own result dropped (DX-W3:
    /// the page got ok + op hash 100 s after the relay had rejected it).
    #[test]
    fn the_tracker_s_verdict_answers_the_page_once() {
        use vela_core::app::sign_request::{
            SignAccountRef, SignApproveOpts, SignResponsePayload, SignSubmitOutcome,
        };
        use vela_core::app::tx_tracker::{TrackEntryView, TrackOutcome, TrackStatus};
        const OP: &str = "0xa974c5dd0a00000000000000000000000000000000000000000000000000beef";
        const ME: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

        let entry = |status: TrackStatus| TrackEntryView {
            user_op_hash: OP.to_owned(),
            chain_id: 100,
            record_ids: vec!["dapp-1-tx".to_owned()],
            status,
            tx_hash: None,
            polling: status == TrackStatus::Pending,
            submitted_at_ms: Some(1_000.0),
            outcome: TrackOutcome::Landing,
            relay_tx_hash: None,
        };
        let pending = [entry(TrackStatus::Pending)];
        let (fed, event) = tracked_event(OP, &pending, None, 1.0)
            .unwrap_or_else(|| unreachable!("the first word is told"));
        assert!(
            tracked_event(OP, &pending, Some(&fed), 2.0).is_none(),
            "once"
        );
        assert!(tracked_event("0xother", &pending, None, 2.0).is_none());

        // The core, past OpSubmitted (accepted), waiting on its receipt.
        let mut host = crate::core_host::CoreHost::<SignRequest>::new();
        host.dispatch(SignEvent::NetworksChanged {
            chain_ids: vec![100],
        });
        host.dispatch(SignEvent::AccountsChanged {
            accounts: vec![SignAccountRef {
                address: ME.to_owned(),
                credential_id: "cred0".to_owned(),
            }],
            active_index: 0,
        });
        host.dispatch(SignEvent::RequestArrived {
            id: "rid-w3".to_owned(),
            method: "eth_sendTransaction".to_owned(),
            params_json:
                r#"[{"to":"0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83","data":"0xa9059cbb"}]"#
                    .to_owned(),
            origin: "http://127.0.0.1:8141".to_owned(),
            transport_id: BROWSER_TRANSPORT.to_owned(),
            dedicated_transport: true,
            per_request_chain: Some(100),
            dapp: None,
            granted_address: Some(ME.to_owned()),
            requested_address: None,
            request_ts_ms: None,
            now_ms: 1_000.0,
        });
        let ops = host.dispatch(SignEvent::ApproveTapped {
            opts: SignApproveOpts::default(),
        });
        let precheck = ops[0].id;
        let ops = host.resolve(precheck, SignShellResult::PreCheck { funding: None });
        let submit = ops[0].id;
        let ops = host.dispatch(SignEvent::OpSubmitted {
            id: "rid-w3".to_owned(),
            user_op_hash: OP.to_owned(),
            now_ms: 5_000.0,
            maybe_sent: false,
            submit_block: None,
        });
        for op in ops {
            if matches!(op.operation, SignOperation::PersistRecord { .. }) {
                let _ = host.resolve(op.id, SignShellResult::RecordPersisted);
            }
        }
        let _ = event;
        let rejected = [entry(TrackStatus::Rejected)];
        let (_, verdict) = tracked_event(OP, &rejected, Some(&fed), 20_000.0)
            .unwrap_or_else(|| unreachable!("a change is told"));
        let told: Vec<_> = host
            .dispatch(verdict)
            .into_iter()
            .filter_map(|op| match op.operation {
                SignOperation::SendResponse { payload, .. } => Some(payload),
                _ => None,
            })
            .collect();
        assert_eq!(told.len(), 1, "{told:?}");
        assert!(matches!(
            &told[0],
            SignResponsePayload::Err { code: -32603, message: Some(message), .. }
                if message == vela_core::user_op::REFUSED_DAPP_DETAIL
        ));
        // The receipt wait's own late result can answer nothing more.
        let late: Vec<_> = host
            .resolve(
                submit,
                SignShellResult::Submit {
                    outcome: SignSubmitOutcome::ReceiptPending {
                        user_op_hash: OP.to_owned(),
                    },
                    now_ms: 120_000.0,
                },
            )
            .into_iter()
            .filter(|op| matches!(op.operation, SignOperation::SendResponse { .. }))
            .collect();
        assert!(late.is_empty(), "one answer only");
    }

    /// Invariant ⑨: the capped params are the ones that get signed.
    ///
    /// This carried `None` until spec 032 phase 30. With the editor wired, a
    /// `None` here would mean somebody picks a cap, watches the sheet show it,
    /// and the site's ORIGINAL unlimited ask is what reaches the chain.
    #[test]
    fn a_chosen_cap_is_what_gets_signed() {
        let fee = crate::core_host::CoreHost::<FeePolicy>::new().view();
        let clear = crate::core_host::CoreHost::<ClearSigning>::new().view();
        let mut guard = crate::core_host::CoreHost::<ApprovalGuard>::new().view();

        // Nothing rewritten: the request stands as it arrived.
        assert_eq!(
            approve_opts(&fee, &clear, &guard).params_override_json,
            None
        );

        let capped = r#"[{"to":"0xtoken","data":"0x095ea7b3capped"}]"#;
        guard.rewritten_params_json = Some(capped.to_owned());
        assert_eq!(
            approve_opts(&fee, &clear, &guard)
                .params_override_json
                .as_deref(),
            Some(capped),
            "the cap the person chose was dropped on the way to the signer"
        );
    }

    /// The other half of the same sentence: an unlimited amount kept as the
    /// site asked reaches the submit guard as the guard's consent, and only
    /// when the guard gave it — the core refuses it otherwise.
    #[test]
    fn a_kept_unlimited_approval_carries_the_guards_consent() {
        let fee = crate::core_host::CoreHost::<FeePolicy>::new().view();
        let clear = crate::core_host::CoreHost::<ClearSigning>::new().view();
        let mut guard = crate::core_host::CoreHost::<ApprovalGuard>::new().view();
        assert!(!approve_opts(&fee, &clear, &guard).unlimited_approved);

        guard.unlimited_consented = true;
        let opts = approve_opts(&fee, &clear, &guard);
        assert!(opts.unlimited_approved);
        assert_eq!(
            opts.params_override_json, None,
            "kept as asked: the site's own bytes, nothing rewritten"
        );
    }

    /// The record keeps the intent Activity will print as a title, so only a
    /// reading that is not best-effort: a selector-database name is the
    /// deployer's word, shown on the sheet under a caution, and Activity has
    /// no caution to show it under (083 H2 review).
    #[test]
    fn only_a_trusted_reading_names_the_recorded_intent() {
        use vela_core::app::clear_signing::{
            ClearProvenance, ClearRisk, ClearSignResult, ClearSignType,
        };

        let fee = crate::core_host::CoreHost::<FeePolicy>::new().view();
        let guard = crate::core_host::CoreHost::<ApprovalGuard>::new().view();
        let mut clear = crate::core_host::CoreHost::<ClearSigning>::new().view();
        assert_eq!(
            approve_opts(&fee, &clear, &guard).intent,
            None,
            "nothing read: the core decides (\"Send\" for a plain send)"
        );

        let reading = |verified: bool, best_effort: bool| ClearSignResult {
            intent: "Claim".to_owned(),
            intent_term: None,
            contract_name: None,
            owner: None,
            fields: Vec::new(),
            risk: ClearRisk::Caution,
            contract_address: None,
            verified,
            provenance: ClearProvenance::Standard,
            sign_type: ClearSignType::Transaction,
            partial: false,
            best_effort,
            to_own_token: false,
        };
        clear.result = Some(reading(false, true));
        assert_eq!(
            approve_opts(&fee, &clear, &guard).intent,
            None,
            "a guessed function name is not recorded"
        );
        clear.result = Some(reading(false, false));
        assert_eq!(
            approve_opts(&fee, &clear, &guard).intent.as_deref(),
            Some("Claim")
        );
        clear.result = Some(reading(true, true));
        assert_eq!(
            approve_opts(&fee, &clear, &guard).intent.as_deref(),
            Some("Claim")
        );
    }

    /// A transaction decodes from its call. A batch's first leg is still what
    /// the blind rung's facts read for a ONE-call batch (drawn as that call);
    /// the decode itself is handed the whole batch (089 S1, below).
    #[test]
    fn the_decoder_is_handed_the_call_and_not_the_envelope() {
        let single = r#"[{"from":"0xaaa","to":"0xbbb","data":"0xabcd","value":"0x1"}]"#;
        assert_eq!(
            first_call("eth_sendTransaction", single),
            Some((
                Some("0xbbb".to_owned()),
                Some("0xabcd".to_owned()),
                Some("0x1".to_owned())
            ))
        );

        let batch = r#"[{"calls":[{"to":"0x1","data":"0xdead"},{"to":"0x2"}]}]"#;
        let (to, data, value) = first_call("wallet_sendCalls", batch)
            .unwrap_or_else(|| unreachable!("a batch has a first leg"));
        assert_eq!(to.as_deref(), Some("0x1"));
        assert_eq!(data.as_deref(), Some("0xdead"));
        assert_eq!(value, None, "an absent value stays absent, never a zero");

        assert_eq!(
            first_call("wallet_sendCalls", r#"[{"to":"0xbbb","data":"0xabcd"}]"#),
            None,
            "a batch with no calls submits nothing, so its envelope is not decoded"
        );
    }

    /// 083 review: an `eth_sendTransaction` is decoded from its OWN call.
    ///
    /// A stray `calls` key is not a batch for this method — the executor's
    /// `calls_of` submits the top-level call — so a harmless-looking leg
    /// beside a malicious call must not become the headline. Checked where
    /// the decode starts (`clear_kickoff`) and where the blind rung reads its
    /// facts (`facts_of`), not only in the helper.
    #[test]
    fn a_stray_calls_key_does_not_hide_the_signed_call() {
        const SIGNED: &str = "0x1111111111111111111111111111111111111111";
        const DECOY: &str = "0x2222222222222222222222222222222222222222";
        let params = format!(
            r#"[{{"to":"{SIGNED}","data":"0x095ea7b3ffff","value":"0x0","calls":[{{"to":"{DECOY}","value":"0x1"}}]}}]"#
        );

        assert_eq!(
            first_call("eth_sendTransaction", &params),
            Some((
                Some(SIGNED.to_owned()),
                Some("0x095ea7b3ffff".to_owned()),
                Some("0x0".to_owned())
            ))
        );
        let submitted = crate::executor::sign_request::calls_of("eth_sendTransaction", &params)
            .unwrap_or_else(|| unreachable!("a transaction has its call"));
        assert_eq!(submitted.len(), 1);
        assert_eq!(submitted[0].to, SIGNED, "what the executor signs");

        let Some(ClearEvent::ResolveTransaction { to, data, .. }) =
            clear_kickoff("eth_sendTransaction", &params, 1, None)
        else {
            unreachable!("a transaction starts the decode")
        };
        assert_eq!(
            to.as_deref(),
            Some(SIGNED),
            "the decode reads the signed call"
        );
        assert_eq!(data.as_deref(), Some("0x095ea7b3ffff"));

        let facts = facts_of(&IncomingRequest {
            id: "1".to_owned(),
            method: "eth_sendTransaction".to_owned(),
            params_json: params,
            origin: "https://example.com".to_owned(),
            transport_id: BROWSER_TRANSPORT.to_owned(),
            chain_id: 1,
            granted_address: None,
        });
        assert_eq!(facts.to.as_deref(), Some(SIGNED));
        assert_eq!(facts.data_bytes, 6);
    }

    /// The sheet decodes the request's ONE document, where its method carries
    /// it — the core's reading, the same bytes the passkey signs (audit
    /// 2026-10-01). Two documents, in either order, give the sheet nothing
    /// and sign nothing.
    #[test]
    fn the_sheet_decodes_the_one_document_the_core_signs() {
        let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let doc = r#"{"types":{"EIP712Domain":[],"Mail":[{"name":"contents","type":"string"}]},"primaryType":"Mail","domain":{},"message":{"contents":"hi"}}"#;
        let quoted = serde_json::to_string(doc).unwrap();
        for (method, params) in [
            ("eth_signTypedData_v4", format!(r#"["{account}",{quoted}]"#)),
            ("eth_signTypedData_v3", format!(r#"["{account}",{doc}]"#)),
            ("eth_signTypedData", format!(r#"[{quoted},"{account}"]"#)),
            ("eth_signTypedData_v1", format!(r#"[{doc},"{account}"]"#)),
        ] {
            let shown = typed_data_of(method, &params);
            assert!(shown.contains(r#""Mail""#), "{method}: {shown}");
            assert_eq!(
                vela_core::eip712::hash_typed_data(&shown).ok(),
                crate::executor::sign_request::message_hash(method, &params),
                "{method}: the sheet's document is the signed one"
            );
        }
        for method in ["eth_signTypedData_v4", "eth_signTypedData"] {
            let two = format!("[{doc},{doc}]");
            assert_eq!(typed_data_of(method, &two), "", "{method}");
            assert_eq!(
                crate::executor::sign_request::message_hash(method, &two),
                None,
                "{method}"
            );
        }
        assert_eq!(
            typed_data_of("eth_signTypedData_v4", "[]"),
            "",
            "nothing to decode is not a panic"
        );
    }
}

#[cfg(test)]
mod kickoff_tests {
    use super::*;
    use vela_core::app::clear_signing::ClearSignMethod;

    /// Every method this sheet can be opened with reaches its own rung.
    ///
    /// `personal_sign` is the one that did not. The core computes a message's
    /// text, its SIWE fields, its domain binding and its danger class — and
    /// this shell drew all of it (`ClearSurface::MessageSign` has rendered
    /// since spec 022) while never starting the machine that fills it. A
    /// person asked to sign a login saw the raw request instead of the
    /// analysis of it.
    #[test]
    fn a_plain_message_reaches_the_message_rung() {
        let params = r#"["0x48656c6c6f","0xabc"]"#;
        match clear_kickoff(
            "personal_sign",
            params,
            1,
            Some("https://app.uniswap.org".into()),
        ) {
            Some(ClearEvent::MessagePresented {
                method,
                params,
                request_origin,
            }) => {
                assert_eq!(method, ClearSignMethod::PersonalSign);
                // As sent, in order: the machine reads them positionally.
                assert_eq!(params, vec!["0x48656c6c6f", "0xabc"]);
                assert_eq!(request_origin.as_deref(), Some("https://app.uniswap.org"));
            }
            other => unreachable!("a message, not {other:?}"),
        }
    }

    /// 089 S1: a batch is handed to the core WHOLE, so every call of it is
    /// read — the decode never starts from call 1 alone.
    #[test]
    fn a_batch_is_handed_to_the_decoder_whole() {
        let params =
            r#"[{"calls":[{"to":"0x1","value":"0x1"},{"to":"0x2","value":"0xde0b6b3a7640000"}]}]"#;
        match clear_kickoff("wallet_sendCalls", params, 100, None) {
            Some(ClearEvent::ResolveBatch {
                params_json,
                chain_id,
                ..
            }) => {
                assert_eq!(params_json, params);
                assert_eq!(chain_id, 100);
            }
            other => unreachable!("the whole batch, not {other:?}"),
        }
    }

    /// `eth_sign` signs an OPAQUE hash, so it is its own method and gets the
    /// hard warning — never the calm message view.
    #[test]
    fn eth_sign_is_not_the_calm_view() {
        match clear_kickoff("eth_sign", r#"["0xabc","0xdead"]"#, 1, None) {
            Some(ClearEvent::MessagePresented { method, .. }) => {
                assert_eq!(method, ClearSignMethod::EthSign);
            }
            other => unreachable!("a message, not {other:?}"),
        }
    }

    /// The other rungs still go where they went, and an unknown method starts
    /// nothing rather than guessing a surface.
    #[test]
    fn the_other_rungs_are_unchanged() {
        assert!(matches!(
            clear_kickoff("eth_sendTransaction", "[]", 100, None),
            Some(ClearEvent::ResolveTransaction { .. })
        ));
        assert!(matches!(
            clear_kickoff("eth_signTypedData_v4", "[]", 1, None),
            Some(ClearEvent::ResolveTypedData { .. })
        ));
        assert!(clear_kickoff("eth_chainId", "[]", 1, None).is_none());
    }

    /// A params list that is not all strings loses the non-strings rather than
    /// stringifying them: the machine reads `params[0]` and `params[1]` by
    /// POSITION, and an object coerced into that list would shift the message
    /// and the address by one.
    #[test]
    fn only_strings_survive_the_params_list() {
        let mixed = r#"["0x48656c6c6f",{"junk":1},"0xabc"]"#;
        match clear_kickoff("personal_sign", mixed, 1, None) {
            Some(ClearEvent::MessagePresented { params, .. }) => {
                assert_eq!(params, vec!["0x48656c6c6f", "0xabc"]);
            }
            other => unreachable!("a message, not {other:?}"),
        }
        // Not a list at all: no params, still a message — the core's own
        // refusal is better than a shell that declines to open the sheet.
        match clear_kickoff("personal_sign", "{}", 1, None) {
            Some(ClearEvent::MessagePresented { params, .. }) => assert!(params.is_empty()),
            other => unreachable!("a message, not {other:?}"),
        }
    }
}

#[cfg(test)]
mod approve_tests {
    use super::*;
    use vela_core::app::fee_policy::FeePolicy;

    /// The approve carries the fee the sheet DISPLAYED.
    ///
    /// Read off `fee_view` — the same view the confirm card rendered — rather
    /// than re-asked. A second question produces a second number, and then
    /// the figure somebody agreed to is not the figure that gets signed.
    /// This is the desktop's version of the rule four web integrations failed
    /// on (this cut's second lesson).
    #[test]
    fn the_quote_that_is_signed_is_the_quote_that_was_shown() {
        use vela_core::app::fee_policy::{FeeAssetView, FeeEstimateView};

        let shown = FeeEstimateView {
            chain_id: 100,
            total_wei: "10000000000000000".to_owned(),
            max_fee_per_gas: "1500000000".to_owned(),
            network_fee_per_gas: "1000000000".to_owned(),
            relayer_fee_per_gas: "500000000".to_owned(),
            bundler_gas_price: "1000000000".to_owned(),
            in_band_gas_basis: "21000".to_owned(),
            effective_gas_price: None,
            max_gas_price: None,
            total_gas: "21000".to_owned(),
            deployed: true,
            tier: vela_core::app::fee_policy::FeeTier::Fast,
            quoted: true,
            fee_asset: FeeAssetView::Native,
            fee_recipient: Some("0xee2c".to_owned()),
        };

        // What `approve` would put in the opts, from that view alone.
        let quoted = Some(SignQuotedFee {
            amount: shown.total_wei.clone(),
            recipient: shown.fee_recipient.clone().unwrap_or_default(),
            tier: Some(shown.tier),
        });
        let opts = SignApproveOpts {
            max_fee_per_gas: Some(shown.max_fee_per_gas.clone()),
            bundler_cost_wei: None,
            gas_fee_token: None,
            quoted_fee: quoted,
            fee_collector: None,
            params_override_json: None,
            intent: None,
            unlimited_approved: false,
            balance_changes: None,
        };

        let signed = opts
            .quoted_fee
            .as_ref()
            .unwrap_or_else(|| unreachable!("a priced sheet approves with its price"));
        assert_eq!(signed.amount, shown.total_wei, "the figure on the screen");
        assert_eq!(signed.recipient, "0xee2c");
        // …and the speed it was priced at, named beside it (spec 069).
        assert_eq!(signed.tier, Some(shown.tier));
        assert_eq!(opts.max_fee_per_gas.as_deref(), Some("1500000000"));
    }

    /// A fee paid in an ERC-20 is approved AS that coin: the fee token rides
    /// along, and the quoted amount is the coin's own figure — `total_wei` is
    /// 0 for an ERC-20 quote, and signing 0 would be a fee claim of nothing.
    #[test]
    fn an_erc20_fee_is_approved_in_its_own_coin() {
        use vela_core::app::fee_policy::FeeEstimateView;

        let usdc = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_owned();
        let mut fee = crate::core_host::CoreHost::<FeePolicy>::new().view();
        fee.fee_token = Some(usdc.clone());
        fee.fee = Some(FeeEstimateView {
            chain_id: 1,
            total_wei: "0".to_owned(),
            max_fee_per_gas: "2000000000".to_owned(),
            network_fee_per_gas: "1".to_owned(),
            relayer_fee_per_gas: "1".to_owned(),
            bundler_gas_price: "1".to_owned(),
            in_band_gas_basis: "21000".to_owned(),
            effective_gas_price: None,
            max_gas_price: None,
            total_gas: "21000".to_owned(),
            deployed: true,
            tier: FeeTier::Fast,
            quoted: true,
            fee_asset: FeeAssetView::Erc20 {
                token: usdc.clone(),
                decimals: 6,
                amount: "1250000".to_owned(),
                symbol: Some("USDC".to_owned()),
            },
            fee_recipient: Some("0xee2c".to_owned()),
        });
        let clear = crate::core_host::CoreHost::<ClearSigning>::new().view();
        let guard = crate::core_host::CoreHost::<ApprovalGuard>::new().view();

        let opts = approve_opts(&fee, &clear, &guard);
        assert_eq!(opts.gas_fee_token.as_deref(), Some(usdc.as_str()));
        let quoted = opts
            .quoted_fee
            .unwrap_or_else(|| unreachable!("a priced sheet approves with its price"));
        assert_eq!(quoted.amount, "1250000", "the coin's figure, not total_wei");
        assert_eq!(quoted.recipient, "0xee2c");

        // The native coin: no fee token, and the quote is `total_wei`.
        fee.fee_token = None;
        if let Some(estimate) = fee.fee.as_mut() {
            estimate.fee_asset = FeeAssetView::Native;
            estimate.total_wei = "91000000000000".to_owned();
        }
        let opts = approve_opts(&fee, &clear, &guard);
        assert_eq!(opts.gas_fee_token, None);
        assert_eq!(
            opts.quoted_fee.map(|quoted| quoted.amount).as_deref(),
            Some("91000000000000")
        );
    }

    /// A personal_sign on the machine the column drives, approved as far as
    /// its signature: `SignAndSubmit` is out, and its id is returned.
    fn approved_to_its_signature(sign: &mut CoreHost<SignRequest>) -> u64 {
        let wallet = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        sign.dispatch(SignEvent::NetworksChanged {
            chain_ids: vec![100],
        });
        sign.dispatch(SignEvent::AccountsChanged {
            accounts: vec![SignAccountRef {
                address: wallet.to_owned(),
                credential_id: "cred0".to_owned(),
            }],
            active_index: 0,
        });
        sign.dispatch(SignEvent::RequestArrived {
            id: "7".to_owned(),
            method: "personal_sign".to_owned(),
            params_json: format!(r#"["0x48656c6c6f","{wallet}"]"#),
            origin: "https://app.uniswap.org".to_owned(),
            transport_id: BROWSER_TRANSPORT.to_owned(),
            dedicated_transport: true,
            per_request_chain: Some(100),
            dapp: None,
            granted_address: None,
            requested_address: None,
            request_ts_ms: None,
            now_ms: 0.0,
        });
        let signing = sign.dispatch(SignEvent::ApproveTapped {
            opts: SignApproveOpts::default(),
        });
        match signing.as_slice() {
            [
                Pending {
                    id,
                    operation: SignOperation::SignAndSubmit { .. },
                },
            ] => *id,
            other => unreachable!("approve signs at once: {}", other.len()),
        }
    }

    /// The answers among `ops`, as (id, error code) — `None` for a success.
    fn answers_in(ops: &[Pending<SignOperation>]) -> Vec<(String, Option<i32>)> {
        use vela_core::app::sign_request::SignResponsePayload;
        ops.iter()
            .filter_map(|pending| match &pending.operation {
                SignOperation::SendResponse { id, payload, .. } => Some((
                    id.clone(),
                    match payload {
                        SignResponsePayload::Err { code, .. } => Some(*code),
                        SignResponsePayload::Ok { .. } => None,
                    },
                )),
                _ => None,
            })
            .collect()
    }

    /// 083 (review): the close while the signature is still to come, against
    /// the machine the column drives, with the column's own decisions in the
    /// order it makes them. The close waits (Esc does not close at all), a
    /// second close adds nothing, the prompt comes back unsigned, and the
    /// waited close is told: exactly one answer, the person's refusal.
    #[test]
    fn a_close_before_the_signature_refuses_exactly_once() {
        use crate::signing::status::{
            ClosePlan, Signature, close_plan, escape_closes, waited_close_due,
        };
        use vela_core::app::sign_request::{SignSubmitOutcome, SignSurface};

        let mut sign = CoreHost::<SignRequest>::new();
        let submit = approved_to_its_signature(&mut sign);

        let plan = close_plan(&close_facts_of(&sign.view(), Signature::NotYet, false));
        assert_eq!(plan, ClosePlan::AfterCancel, "preparing: the close waits");
        assert!(
            !escape_closes(sign.view().swipe_action, plan),
            "and Esc leaves the request alone (D1)"
        );
        let plan = close_plan(&close_facts_of(&sign.view(), Signature::Asked, false));
        assert_eq!(plan, ClosePlan::AfterCancel, "the QR is up: the same");
        assert_eq!(
            close_plan(&close_facts_of(&sign.view(), Signature::Asked, true)),
            ClosePlan::Ignore,
            "a second close while the first waits"
        );

        let view = sign.view();
        assert!(!waited_close_due(
            view.is_signing || view.is_submitting,
            view.pending_op_hash.is_some()
        ));
        let back = sign.resolve(
            submit,
            SignShellResult::Submit {
                outcome: SignSubmitOutcome::PasskeyCancelled,
                now_ms: 1.0,
            },
        );
        assert!(answers_in(&back).is_empty(), "the core kept the request");
        let view = sign.view();
        assert!(waited_close_due(
            view.is_signing || view.is_submitting,
            view.pending_op_hash.is_some()
        ));

        let refused = sign.dispatch(SignEvent::SwipeDismissed);
        assert_eq!(answers_in(&refused), vec![("7".to_owned(), Some(4001))]);
        assert_eq!(sign.view().surface, SignSurface::Hidden);
        assert!(answers_in(&sign.dispatch(SignEvent::SwipeDismissed)).is_empty());
    }

    /// 083 (review): once the phone has signed, the close is the ordinary
    /// one, at once — the operation goes on and the page still gets its
    /// answer (spec 079 FR-002); nothing is refused. And if the pipeline
    /// then comes back unsigned after all (a second prompt, dismissed), the
    /// core's own safety net answers the closed request once: 4001, never
    /// silence.
    #[test]
    fn a_close_after_the_signature_refuses_nothing() {
        use crate::signing::status::{ClosePlan, Signature, close_plan};
        use vela_core::app::sign_request::SignSubmitOutcome;

        let mut sign = CoreHost::<SignRequest>::new();
        let submit = approved_to_its_signature(&mut sign);
        assert_eq!(
            close_plan(&close_facts_of(&sign.view(), Signature::Given, false)),
            ClosePlan::Now
        );
        let dismissed = sign.dispatch(SignEvent::SwipeDismissed);
        assert!(answers_in(&dismissed).is_empty(), "no refusal");

        let back = sign.resolve(
            submit,
            SignShellResult::Submit {
                outcome: SignSubmitOutcome::PasskeyCancelled,
                now_ms: 1.0,
            },
        );
        assert_eq!(answers_in(&back), vec![("7".to_owned(), Some(4001))]);
        assert!(answers_in(&sign.dispatch(SignEvent::SwipeDismissed)).is_empty());
    }

    /// 083: a ceremony that comes back unsigned gives the form back — the
    /// column then un-approves and resets the slide, which stayed stuck at
    /// the end — and a failure does not: the page has its answer.
    #[test]
    fn only_an_unsigned_ceremony_gives_the_form_back() {
        use vela_core::app::sign_request::SignSubmitOutcome;

        let mut sign = CoreHost::<SignRequest>::new();
        let submit = approved_to_its_signature(&mut sign);
        assert!(!back_on_form_of(&sign.view(), false), "still signing");
        let back = sign.resolve(
            submit,
            SignShellResult::Submit {
                outcome: SignSubmitOutcome::PasskeyCancelled,
                now_ms: 1.0,
            },
        );
        assert!(answers_in(&back).is_empty());
        assert!(back_on_form_of(&sign.view(), false), "the form is back");
        assert!(sign.view().confirm_gate_open, "and can be approved again");

        let mut sign = CoreHost::<SignRequest>::new();
        let submit = approved_to_its_signature(&mut sign);
        let failed = sign.resolve(
            submit,
            SignShellResult::Submit {
                outcome: SignSubmitOutcome::Failed {
                    message: "the relay said no".to_owned(),
                    refused: false,
                },
                now_ms: 1.0,
            },
        );
        assert_eq!(answers_in(&failed), vec![("7".to_owned(), Some(-32603))]);
        assert!(
            !back_on_form_of(&sign.view(), true),
            "a failure is an answer"
        );
        assert!(
            !back_on_form_of(&sign.view(), false),
            "…and the core says so on its own"
        );
    }

    /// 083 W19: a column that goes away stops what its ceremony waits
    /// behind — the phone's scan, a PIN question nobody can answer, the
    /// Trusted Signer's page — instead of leaving a 90 s scan or a blocked
    /// thread behind a column nobody can see.
    #[test]
    fn a_column_that_goes_away_stops_its_ceremony() {
        let channel = CeremonyChannel::new();
        let ceremony = channel.ceremony(0);
        let (signer, _changed) = trusted_signer::Channel::new();
        (ceremony.qr)(Some("FIDO:/083".to_owned()));
        assert!(!(ceremony.cancelled)());

        // A PIN question, blocking the ceremony's thread until a screen answers.
        let pin = Arc::clone(&ceremony.pin);
        let asking = std::thread::spawn(move || {
            pin(crate::executor::passkey::PinRequest {
                product: "YubiKey 5C NFC".to_owned(),
                device: "hid-1".to_owned(),
                retries: Some(8),
                retry: false,
            })
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while channel.pending_pin().is_none() && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(channel.pending_pin().is_some(), "the question is up");

        release(&signer, &channel);

        assert_eq!(
            asking.join().ok(),
            Some(None),
            "the question is let go, unanswered"
        );
        assert_eq!(channel.qr_showing(), None, "the QR comes down");
        assert!((ceremony.cancelled)(), "and the scan behind it is told");
        assert!(signer.stopped(), "no Trusted Signer wait goes on or starts");
    }

    /// A request's context, as the column fixes it when the request opens.
    fn sign_context() -> SignContext {
        let account = Account {
            id: "cred0".to_owned(),
            name: "savings".to_owned(),
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
            public_key_hex: "04aa".to_owned(),
            created_at_iso: String::new(),
            keys: vec![vela_core::app::AccountKey {
                credential_id: "cred0".to_owned(),
                public_key_hex: "04aa".to_owned(),
                name: String::new(),
                transports: "hybrid".to_owned(),
                signer_origin: None,
            }],
            signed_in_with: None,
        };
        SignContext::new(&account, CeremonyChannel::new().ceremony(0))
    }

    /// 083 H4 (review): a phone stop, through the column, answers the page
    /// exactly once. The prompt comes back with the phone's connection gone
    /// — never up, or dropped once asked — and the executor tells the core a
    /// dismissal and the column which stop: the core keeps the request, the
    /// form is back, and the column takes the stop for its card
    /// (`pump_sign`). The card's 关闭 is the column's close, told at once
    /// (`close`), and refuses once; a second adds nothing. Its 重试 is the
    /// approval again (`retry` → `approve` → `dispatch_sign`, which resets
    /// the prompt): nothing is told the page until that one ends, and then
    /// its answer is the only one.
    ///
    /// Driven the way the tests beside it drive the column: the core machine
    /// and the column's own decisions, in the order `pump_sign`, `close` and
    /// `retry` make them — a `SigningHost` is a gpui entity, and this crate
    /// builds gpui without its test support.
    #[test]
    fn a_phone_stop_card_answers_the_page_exactly_once() {
        use crate::ctap::cable::{HybridError, PHONE_DROPPED};
        use crate::executor::passkey::PasskeyFailure;
        use crate::executor::sign_request::{PhoneStop, around_prompt};
        use crate::signing::status::{ClosePlan, Signature, close_plan};
        use vela_core::app::FailureKind;
        use vela_core::app::sign_request::{SignSubmitOutcome, SignSurface};

        let ctx = sign_context();
        let stops = [
            (
                HybridError::Tunnel("cannot reach cable.auth.com".to_owned()).to_string(),
                PhoneStop::LinkFailed,
            ),
            (
                format!("{PHONE_DROPPED}: the phone stopped answering"),
                PhoneStop::Dropped,
            ),
        ];
        // The prompt comes back as the phone left it; the pipeline (user_op)
        // reports a dismissed prompt as `PasskeyCancelled`.
        let stopped = |sign: &mut CoreHost<SignRequest>, submit: u64, error: &str| {
            ctx.prompt_reset();
            let prompt = around_prompt(&ctx, || Err(PasskeyFailure::other(error)));
            assert!(prompt.is_err_and(|failure| failure.kind == FailureKind::Cancelled));
            let back = sign.resolve(
                submit,
                SignShellResult::Submit {
                    outcome: SignSubmitOutcome::PasskeyCancelled,
                    now_ms: 1.0,
                },
            );
            assert!(answers_in(&back).is_empty(), "the core kept the request");
            assert!(back_on_form_of(&sign.view(), false), "the form is back");
        };

        for (error, stop) in &stops {
            // 关闭.
            let mut sign = CoreHost::<SignRequest>::new();
            let submit = approved_to_its_signature(&mut sign);
            stopped(&mut sign, submit, error);
            assert_eq!(ctx.take_phone_stop(), Some(*stop), "the card: {error}");
            assert_eq!(
                close_plan(&close_facts_of(&sign.view(), Signature::Asked, false)),
                ClosePlan::Now,
                "the card's close is told at once"
            );
            let refused = sign.dispatch(SignEvent::SwipeDismissed);
            assert_eq!(answers_in(&refused), vec![("7".to_owned(), Some(4001))]);
            assert_eq!(sign.view().surface, SignSurface::Hidden);
            assert!(answers_in(&sign.dispatch(SignEvent::SwipeDismissed)).is_empty());

            // 重试: the approval again, which signs this time.
            let mut sign = CoreHost::<SignRequest>::new();
            let submit = approved_to_its_signature(&mut sign);
            stopped(&mut sign, submit, error);
            assert!(sign.view().confirm_gate_open, "the slide can go again");
            // A stop the column had not taken yet is not carried into the
            // new approval: `dispatch_sign` resets the prompt first.
            ctx.prompt_reset();
            assert_eq!(ctx.take_phone_stop(), None, "an approval starts clean");
            let again = sign.dispatch(SignEvent::ApproveTapped {
                opts: SignApproveOpts::default(),
            });
            assert!(answers_in(&again).is_empty(), "a retry answers nothing");
            let submit = match again.as_slice() {
                [
                    Pending {
                        id,
                        operation: SignOperation::SignAndSubmit { .. },
                    },
                ] => *id,
                other => unreachable!("a retry signs at once: {}", other.len()),
            };
            // Record, then respond — the core's order.
            let mut ops = sign.resolve(
                submit,
                SignShellResult::Submit {
                    outcome: SignSubmitOutcome::Succeeded {
                        result: "0x1626ba7e".to_owned(),
                    },
                    now_ms: 2.0,
                },
            );
            let mut answered = answers_in(&ops);
            while let Some(record) = ops.iter().find_map(|pending| match pending.operation {
                SignOperation::PersistRecord { .. } => {
                    Some((pending.id, SignShellResult::RecordPersisted))
                }
                SignOperation::UpdateRecord { .. } => {
                    Some((pending.id, SignShellResult::RecordUpdated))
                }
                _ => None,
            }) {
                ops = sign.resolve(record.0, record.1);
                answered.extend(answers_in(&ops));
            }
            assert_eq!(answered, vec![("7".to_owned(), None)], "the retry's answer");
            assert!(answers_in(&sign.dispatch(SignEvent::SwipeDismissed)).is_empty());
        }

        // The phone hung up while it connected (review): the person's own
        // cancel on the phone. The form is back with no card — nothing
        // blames the network — and the column's close from the form refuses
        // once, as it does after any dismissal.
        let mut sign = CoreHost::<SignRequest>::new();
        let submit = approved_to_its_signature(&mut sign);
        ctx.prompt_reset();
        let hung_up = around_prompt(&ctx, || {
            Err(PasskeyFailure::classified(
                FailureKind::Cancelled,
                crate::ctap::cable::PHONE_ENDED,
            ))
        });
        assert!(hung_up.is_err_and(|failure| failure.kind == FailureKind::Cancelled));
        let back = sign.resolve(
            submit,
            SignShellResult::Submit {
                outcome: SignSubmitOutcome::PasskeyCancelled,
                now_ms: 1.0,
            },
        );
        assert!(answers_in(&back).is_empty(), "the core kept the request");
        assert!(back_on_form_of(&sign.view(), false), "the form is back");
        assert_eq!(ctx.take_phone_stop(), None, "no card");
        let refused = sign.dispatch(SignEvent::SwipeDismissed);
        assert_eq!(answers_in(&refused), vec![("7".to_owned(), Some(4001))]);
    }

    /// An unpriced sheet approves with no quote rather than a zero.
    ///
    /// It cannot be reached — the slide is shut without `confirm_fee_ready` —
    /// but a zero here would be a fee claim, and the submit would sign it.
    #[test]
    fn an_unpriced_sheet_carries_no_quote_at_all() {
        let none: Option<&vela_core::app::fee_policy::FeeEstimateView> = None;
        let quoted = none.map(|estimate| SignQuotedFee {
            amount: estimate.total_wei.clone(),
            recipient: estimate.fee_recipient.clone().unwrap_or_default(),
            tier: Some(estimate.tier),
        });
        assert!(quoted.is_none());
    }
}
