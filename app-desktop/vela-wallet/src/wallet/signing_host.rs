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
/// or — the deployment read could not answer, so nothing reached the core —
/// the same thing an unreachable relay is (spec 079).
fn requote_failure(fee: &FeeView, unanswered: bool) -> Option<FeeFailure> {
    fee.failed
        .or_else(|| unanswered.then_some(FeeFailure::QuoteUnavailable))
}

/// The wait before automatic re-quote `attempt`, or `None`: only while the
/// person can still decide (open, not approved), never over a measurement
/// already out, and never for a failure no retry fixes (the core's schedule,
/// `fee_policy::requote_delay_ms`: 3 s, 6 s, 12 s, then every 15 s).
fn requote_wait(
    failure: Option<FeeFailure>,
    attempt: u32,
    on_form: bool,
    measuring: bool,
) -> Option<u32> {
    if !on_form || measuring {
        return None;
    }
    vela_core::app::fee_policy::requote_delay_ms(failure?, attempt)
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
    /// The simulation was attempted and could not answer (no `eth_simulateV1`
    /// on this endpoint, every RPC down, params refused). A different sentence
    /// from "it ran and found nothing".
    pub sim_unavailable: bool,
    /// The hash already handed to the tracker. The handoff stays on the view
    /// after it is taken, and the tracker merges by hash anyway, but handing
    /// the same submission over on every render is a poll nobody asked for.
    handed_off: Option<String>,
    /// Spec 079: how the request ended, read off the answer the core sent —
    /// a message signed, a transaction landed, or the operation hash because
    /// the wait ran out. The page keeps it on screen after the core closes
    /// the column (the tick, or "not landed yet").
    pub ending: Option<crate::signing::status::SigningEnding>,
    /// Spec 079: when this column first saw the operation accepted — the
    /// receipt's ring starts here until the tracker has its own clock.
    pub seen_submitted_ms: Option<f64>,
    /// The person approved this request (the slide, or the button that opens
    /// the Trusted Signer's page). A close after this refuses nothing.
    pub approved: bool,
    /// Spec 079: automatic re-quotes since the last good quote, and the
    /// number of the one scheduled — a newer schedule, an approval or a good
    /// quote makes an older timer a no-op.
    requote_attempt: u32,
    requote_scheduled: Option<u64>,
    requote_seq: u64,
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
            sim_unavailable: false,
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
            ending: None,
            seen_submitted_ms: None,
            approved: false,
            requote_attempt: 0,
            requote_scheduled: None,
            requote_seq: 0,
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
            let deltas = cx
                .background_executor()
                .spawn(async move { crate::executor::sim::simulate(&sim_wallet, &calls, chain_id) })
                .await;
            let Some(deltas) = deltas else {
                host.update(cx, |host, cx| {
                    // Could not ask. NOT "nothing moves" — the sheet has a
                    // different sentence for each, and conflating them would
                    // tell somebody a drain is a no-op.
                    host.sim_unavailable = true;
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
        self.phone_stop = None;
        // Nothing re-prices under a slide that has gone.
        self.requote_scheduled = None;
        let opts = approve_opts(self.speed.fee_view(), &self.clear_view, &self.guard_view);
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

    /// The fee in force was never asked of the core: its deployment read
    /// could not answer.
    pub fn fee_unanswered(&self) -> bool {
        self.speed.unanswered()
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

    /// Spec 079 FR-008: a quote that failed because the service could not be
    /// reached is asked again at a growing interval while the sheet is open
    /// and unapproved. The device pass had the row read "点击重试" with the
    /// relay down, and stay that way after it came back.
    fn schedule_requote(&mut self, cx: &mut Context<Self>) {
        let failure = requote_failure(self.speed.fee_view(), self.speed.unanswered());
        if failure.is_none() {
            // A good quote (or one still out): the count starts over.
            if !self.speed.measuring() {
                self.requote_attempt = 0;
            }
            self.requote_scheduled = None;
            return;
        }
        if self.requote_scheduled.is_some() {
            return;
        }
        let Some(wait) = requote_wait(
            failure,
            self.requote_attempt + 1,
            self.on_form(),
            self.speed.measuring(),
        ) else {
            return;
        };
        self.requote_attempt += 1;
        self.requote_seq += 1;
        let seq = self.requote_seq;
        self.requote_scheduled = Some(seq);
        cx.spawn(async move |host, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(u64::from(wait)))
                .await;
            host.update(cx, |host, cx| {
                if host.requote_scheduled != Some(seq) {
                    return;
                }
                host.requote_scheduled = None;
                let still = requote_failure(host.speed.fee_view(), host.speed.unanswered());
                if still.is_some() && host.on_form() && !host.speed.measuring() {
                    speed_control::refresh(host, cx);
                } else {
                    host.schedule_requote(cx);
                }
            })
            .ok();
        })
        .detach();
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
        let wrote_record = matches!(
            result,
            SignShellResult::RecordPersisted | SignShellResult::RecordUpdated
        );
        let pending = self.sign.resolve(id, result);
        self.pump_sign(pending, cx);
        if wrote_record {
            crate::executor::activity_feed::focus_tick(cx);
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
                SignAnswer::Blocking(work) => {
                    cx.spawn(async move |host, cx| {
                        let result = cx.background_executor().spawn(async move { work() }).await;
                        host.update(cx, |host, cx| host.resolve_sign(id, result, cx))
                            .ok();
                    })
                    .detach();
                }
                SignAnswer::Streaming(work) => {
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
                        host.update(cx, |host, cx| host.resolve_sign(id, result, cx))
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
        if let Some(handoff) = self.view.tracker_handoff.clone()
            && self.handed_off.as_deref() != Some(handoff.user_op_hash.as_str())
        {
            self.handed_off = Some(handoff.user_op_hash.clone());
            crate::executor::tracker::submitted(
                handoff.user_op_hash,
                handoff.record_ids,
                handoff.chain_id,
                cx,
            );
        }
        // `Hidden` is the core saying the request is over — the column closes
        // on the machine's word, never on a click this file interpreted.
        self.closed = self.view.surface == vela_core::app::sign_request::SignSurface::Hidden;
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
            self.handed_off.as_deref(),
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
            // Spec 079: what this answer says about how the request ended —
            // read BEFORE the sheet is cleared, while the hash this column
            // submitted is still known.
            let submitted = self.handed_off.clone().or_else(|| {
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
        intent: clear.result.as_ref().map(|result| result.intent.clone()),
        // The guard showed an unbounded amount and it was kept as the site
        // asked — the submit guard's only waiver, copied from the view that
        // drew it, never decided here.
        unlimited_approved: guard.unlimited_consented,
    }
}

/// The blind rung's two facts: who it goes to, and how many bytes of calldata
/// nobody could read. Both come from the first leg, like the decode does.
fn facts_of(request: &IncomingRequest) -> crate::signing::live::RequestFacts {
    let call = first_call(&request.method, &request.params_json);
    let data = call.as_ref().and_then(|c| c.1.clone()).unwrap_or_default();
    crate::signing::live::RequestFacts {
        to: call.and_then(|c| c.0),
        // Hex, so two characters per byte; an odd tail is a malformed payload
        // and rounds DOWN rather than claiming a byte that is not there.
        data_bytes: data.trim_start_matches("0x").len() / 2,
        native_send: native_send_of(request),
    }
}

/// A request that only moves the chain's own coin (083 W10), read from the
/// calls the executor will submit rather than from `first_call`: the figure
/// and the recipient drawn are then the ones signed.
///
/// `None` keeps the blind rung, which says nothing false: more than one call
/// (a batch whose first leg is a plain send is more than that send), any
/// calldata (the core's own `TxPlain` test is empty or `0x`), a recipient
/// that is not an address, or a chain whose coin the wallet cannot name —
/// Tempo has none, and "ETH" on a custom network would be a guess.
fn native_send_of(request: &IncomingRequest) -> Option<crate::signing::live::NativeSend> {
    // A batch is counted as the site SENT it, not as `calls_of` reads it:
    // `calls_of` drops a leg it cannot read (one with no `to` — a deployment,
    // which EIP-5792 allows), so `[send, deploy]` would otherwise count as one
    // call and draw a calm "send 1 ETH" over a batch that is more than that
    // (083 W10 review).
    if request.method == "wallet_sendCalls" {
        let params: serde_json::Value = serde_json::from_str(&request.params_json).ok()?;
        if params.get(0)?.get("calls")?.as_array()?.len() != 1 {
            return None;
        }
    }
    let calls = crate::executor::sign_request::calls_of(&request.method, &request.params_json)?;
    let [call] = calls.as_slice() else {
        return None;
    };
    let plain = matches!(call.data.as_str(), "" | "0x")
        && vela_core::app::contacts::is_address(&call.to)
        && !vela_core::app::fee_policy::is_tempo_chain(request.chain_id);
    if !plain {
        return None;
    }
    // The core's one spelling of a built-in chain's coin; a custom network
    // has none, and stays blind.
    let symbol = vela_core::app::network_admin::builtin_native_symbol(request.chain_id)?;
    Some(crate::signing::live::NativeSend {
        to: call.to.clone(),
        wei: call.value.parse().ok()?,
        symbol: symbol.to_owned(),
    })
}

/// A batch decodes from its first leg today, which is what the phone's sheet
/// shows too; the per-leg panorama (CS26) is a drawn state nobody has wired.
/// Which rung of the clear-signing ladder a request climbs.
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
        "eth_sendTransaction" | "wallet_sendCalls" => {
            let call = first_call(method, params_json);
            Some(ClearEvent::ResolveTransaction {
                to: call.as_ref().and_then(|c| c.0.clone()),
                data: call.as_ref().and_then(|c| c.1.clone()),
                value: call.and_then(|c| c.2),
                chain_id,
                locale: ClearLocale::default(),
            })
        }
        "eth_signTypedData_v4" | "eth_signTypedData" => Some(ClearEvent::ResolveTypedData {
            typed_data_json: typed_data_of(params_json),
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
    Some((field("to"), field("data"), field("value")))
}

/// `eth_signTypedData_v4`'s payload: `[address, json]` — the JSON is the
/// SECOND parameter, and reading the first would hand the decoder an address
/// where it expects a document.
fn typed_data_of(params_json: &str) -> String {
    serde_json::from_str::<serde_json::Value>(params_json)
        .ok()
        .and_then(|params| params.get(1).cloned())
        .map(|value| {
            value
                .as_str()
                .map_or_else(|| value.to_string(), str::to_owned)
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::fee_policy::FeePolicy;

    /// Spec 079 FR-008: 3 s, 6 s, 12 s, then every 15 s.
    #[test]
    fn a_failed_quote_is_asked_again_on_the_cores_schedule() {
        let waits: Vec<Option<u32>> = (1..=5)
            .map(|attempt| requote_wait(Some(FeeFailure::QuoteUnavailable), attempt, true, false))
            .collect();
        assert_eq!(
            waits,
            vec![
                Some(3_000),
                Some(6_000),
                Some(12_000),
                Some(15_000),
                Some(15_000)
            ]
        );
    }

    /// Never under a slide that has gone, never over a measurement already
    /// out, never for a failure no retry fixes, and nothing to do when the
    /// quote is good.
    #[test]
    fn the_requote_stops_where_it_must() {
        let unreachable = Some(FeeFailure::QuoteUnavailable);
        assert_eq!(requote_wait(unreachable, 1, false, false), None, "approved");
        assert_eq!(requote_wait(unreachable, 1, true, true), None, "measuring");
        assert_eq!(
            requote_wait(Some(FeeFailure::MissingPublicKey), 1, true, false),
            None
        );
        assert_eq!(
            requote_wait(Some(FeeFailure::CalculationFailed), 1, true, false),
            None
        );
        // Spec 083 fee: the relay's "this operation fails" is an answer; the
        // same operation asked again gets the same one.
        for attempt in 1..=5 {
            assert_eq!(
                requote_wait(Some(FeeFailure::WouldFail), attempt, true, false),
                None
            );
        }
        assert_eq!(requote_wait(None, 1, true, false), None);
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

    /// A deployment read that could not answer never reached the core; it is
    /// the same to a person as an unreachable relay, and asked again the same.
    #[test]
    fn an_unanswered_read_is_an_unreachable_quote() {
        let quiet = crate::core_host::CoreHost::<FeePolicy>::new().view();
        assert_eq!(requote_failure(&quiet, false), None);
        assert_eq!(
            requote_failure(&quiet, true),
            Some(FeeFailure::QuoteUnavailable)
        );
        let failed = FeeView {
            failed: Some(FeeFailure::EstimateFailed),
            ..quiet
        };
        assert_eq!(
            requote_failure(&failed, true),
            Some(FeeFailure::EstimateFailed),
            "the core's own reason first"
        );
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

    /// A transaction decodes from its call — and a BATCH decodes from its
    /// first leg, which is the same thing the phone's sheet shows.
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
        assert!(
            facts.native_send.is_none(),
            "the decoy's plain send is not what is signed"
        );
    }

    /// 083 W10: a plain native send is read as one — and only a plain native
    /// send, from the calls the executor will submit.
    #[test]
    fn only_a_single_empty_call_is_a_native_send() {
        const TO: &str = "0x76875eb2c6d2ea8d6b7fc7e0ce6d2c1e6ac0d141";
        let request = |method: &str, params: String, chain_id: u32| IncomingRequest {
            id: "1".to_owned(),
            method: method.to_owned(),
            params_json: params,
            origin: "https://example.com".to_owned(),
            transport_id: BROWSER_TRANSPORT.to_owned(),
            chain_id,
            granted_address: None,
        };
        let send = |params: String, chain_id: u32| {
            native_send_of(&request("eth_sendTransaction", params, chain_id))
        };

        let dust = send(
            format!(r#"[{{"to":"{TO}","value":"0x38d7ea4c68000"}}]"#),
            100,
        )
        .unwrap_or_else(|| unreachable!("0.001 xDAI with no calldata is a send"));
        assert_eq!(dust.wei, 1_000_000_000_000_000);
        assert_eq!(dust.symbol, "xDAI", "Gnosis' own coin");
        assert_eq!(dust.to, TO);
        let empty = send(format!(r#"[{{"to":"{TO}","value":"0x1","data":"0x"}}]"#), 1)
            .unwrap_or_else(|| unreachable!("`0x` is no calldata"));
        assert_eq!((empty.wei, empty.symbol.as_str()), (1, "ETH"));
        assert_eq!(
            send(format!(r#"[{{"to":"{TO}"}}]"#), 1).map(|s| s.wei),
            Some(0),
            "an absent value is zero, as the executor reads it"
        );

        // Everything else stays on the blind rung.
        let blind = [
            (
                format!(r#"[{{"to":"{TO}","value":"0x1","data":"0xdeadbeef"}}]"#),
                100,
            ),
            (format!(r#"[{{"to":"{TO}","value":"0xzz"}}]"#), 100),
            (r#"[{"to":"0xbbb","value":"0x1"}]"#.to_owned(), 100),
            (format!(r#"[{{"to":"{TO}","value":"0x1"}}]"#), 4217),
            (format!(r#"[{{"to":"{TO}","value":"0x1"}}]"#), 999_999),
        ];
        for (params, chain_id) in blind {
            assert!(
                send(params.clone(), chain_id).is_none(),
                "{params} on {chain_id}"
            );
        }

        let batch = |calls: &str| {
            native_send_of(&request(
                "wallet_sendCalls",
                format!(r#"[{{"calls":{calls}}}]"#),
                100,
            ))
        };
        assert!(batch(&format!(r#"[{{"to":"{TO}","value":"0x1"}}]"#)).is_some());
        assert!(
            batch(&format!(
                r#"[{{"to":"{TO}","value":"0x1"}},{{"to":"{TO}","data":"0xdead"}}]"#
            ))
            .is_none(),
            "a batch is more than its first leg"
        );
        // 083 W10 review: a leg `calls_of` cannot read (no `to` — a
        // deployment) still counts; the batch is not a lone send.
        assert!(
            batch(&format!(
                r#"[{{"to":"{TO}","value":"0xde0b6b3a7640000"}},{{"data":"0x6080"}}]"#
            ))
            .is_none(),
            "an unreadable leg is still a leg"
        );
        assert!(
            native_send_of(&request(
                "personal_sign",
                r#"["0xdead","0xabc"]"#.to_owned(),
                1
            ))
            .is_none()
        );

        // 083 W10 review: the blind rung's facts carry it — the path the
        // sheet actually reads, not only the helper.
        let facts = facts_of(&request(
            "eth_sendTransaction",
            format!(r#"[{{"to":"{TO}","value":"0x1"}}]"#),
            100,
        ));
        assert!(facts.native_send.is_some());
        assert_eq!(facts.data_bytes, 0);
    }

    /// 083 W10 review: from the request a dApp sent to the blocks the sheet
    /// draws, through the real core and the host's own `facts_of` — nothing
    /// built by hand in between. A 0.001 xDAI send reads as a send; the same
    /// request with calldata stays on the blind rung.
    #[test]
    fn a_dapp_native_send_reaches_the_sheet_as_a_send() {
        use crate::signing::fixtures::Block;
        const TO: &str = "0x76875eb2c6d2ea8d6b7fc7e0ce6d2c1e6ac0d141";
        let s = crate::signing::SigningStrings::resolve(&crate::loc::Loc::from_env());
        let drawn = |params: String| {
            let request = IncomingRequest {
                id: "1".to_owned(),
                method: "eth_sendTransaction".to_owned(),
                params_json: params,
                origin: "https://example.com".to_owned(),
                transport_id: BROWSER_TRANSPORT.to_owned(),
                chain_id: 100,
                granted_address: None,
            };
            let mut clear = CoreHost::<ClearSigning>::new();
            let kickoff = clear_kickoff(
                &request.method,
                &request.params_json,
                request.chain_id,
                None,
            )
            .unwrap_or_else(|| unreachable!("a transaction starts the decode"));
            let _ = clear.dispatch(kickoff);
            let view = crate::signing::live::localized_terms(&clear.view(), &s);
            crate::signing::live::blocks(&view, &facts_of(&request), &s)
        };

        let send = drawn(format!(r#"[{{"to":"{TO}","value":"0x38d7ea4c68000"}}]"#));
        assert!(
            matches!(send.first(), Some(Block::Intent { text, .. }) if *text == s.intent_send),
            "a send, not a contract interaction"
        );
        assert!(
            !send
                .iter()
                .any(|block| matches!(block, Block::Warning { .. })),
            "nothing was left to decode"
        );
        assert!(send.iter().any(|block| matches!(
            block,
            Block::Amount { line, .. } if line.symbol == "xDAI"
        )));
        assert!(send.iter().any(|block| matches!(
            block,
            Block::Party { label, address: Some(address), badge: None, .. }
                if *label == s.label_recipient && address == TO
        )));

        let blind = drawn(format!(
            r#"[{{"to":"{TO}","value":"0x38d7ea4c68000","data":"0xdeadbeef"}}]"#
        ));
        assert!(
            !blind.iter().any(|block| matches!(
                block,
                Block::Intent { text, .. } if *text == s.intent_send
            )),
            "calldata nobody decoded is never drawn as a send"
        );
    }

    /// `eth_signTypedData_v4` is `[address, json]`.
    ///
    /// The document is the SECOND parameter. Reading the first hands the
    /// decoder an address where it expects a typed-data payload, and the
    /// whole ladder falls to its blind rung for every typed request — a
    /// degradation nobody would see as a bug, only as "clear signing never
    /// works here".
    #[test]
    fn typed_data_comes_from_the_second_parameter() {
        let params = r#"["0xaaa","{\"primaryType\":\"Permit\"}"]"#;
        assert_eq!(typed_data_of(params), r#"{"primaryType":"Permit"}"#);

        // Some sites pass the document as an object rather than a string.
        let object = r#"["0xaaa",{"primaryType":"Permit"}]"#;
        assert_eq!(typed_data_of(object), r#"{"primaryType":"Permit"}"#);

        assert_eq!(typed_data_of("[]"), "", "nothing to decode is not a panic");
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
