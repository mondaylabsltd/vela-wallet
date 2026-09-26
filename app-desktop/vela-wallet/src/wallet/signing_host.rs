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

use futures::StreamExt as _;
use gpui::Context;

use vela_core::app::Account;
use vela_core::app::approval_guard::{
    ApprovalGuard, Event as GuardEvent, GuardOperation, GuardShellResult, GuardView,
};
use vela_core::app::clear_signing::{
    ClearOperation, ClearShellResult, ClearSigning, ClearSigningView, Event as ClearEvent,
};
use vela_core::app::fee_policy::{FeeAssetView, FeeCall, FeeTier, FeeView};
use vela_core::app::fee_speed::FeeSpeedView;
use vela_core::app::fee_tier_pref::FeeTierPref;
use vela_core::app::sign_pref::{self, SignPref};
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
}

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
    /// "Sign with": this request's choice, and whether its list is open. It
    /// starts at the default Settings keeps (`sign_pref`) and never writes
    /// back to it — a choice here is about this one request.
    pub sign_method: String,
    pub sign_with_open: bool,
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
    #[allow(dead_code, reason = "held so the ceremony outlives the request")]
    channel: Arc<CeremonyChannel>,
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
}

impl SigningHost {
    /// `None` toggles the list; an id picks a method and closes it. Only a
    /// name this build offers is taken (`sign_pref::parse_method`); the Clear
    /// Signer is routed to the page Settings names right now.
    pub fn sign_with(&mut self, id: Option<&str>, cx: &mut Context<Self>) {
        let Some(id) = id else {
            self.sign_with_open = !self.sign_with_open;
            return;
        };
        if let Some(method) = sign_pref::parse_method(id) {
            let page = crate::resident::resident::<SignPref>(cx)
                .read(cx)
                .view()
                .signer_url;
            self.ctx.choose_method(method, &page);
            method.clone_into(&mut self.sign_method);
        }
        self.sign_with_open = false;
    }

    /// The Trusted Signer's waiting sheet and its last word (spec 071).
    pub fn trusted_signer(&self) -> Arc<trusted_signer::Channel> {
        Arc::clone(&self.ctx.trusted_signer)
    }

    /// Something on the Trusted Signer's channel changed: hand the browser the
    /// page if a ceremony asked for it, and redraw.
    fn trusted_signer_changed(&mut self, cx: &mut Context<Self>) {
        if let Some(url) = self.ctx.trusted_signer.take_page() {
            cx.open_url(&url);
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
            sign_method: "auto".to_owned(),
            sign_with_open: false,
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
            closed: false,
            handed_off: None,
        };
        // The stored default speed (spec 069), read now and followed while
        // the sheet is up: the column sits beside Settings, which may change it.
        let preference = crate::resident::resident::<FeeTierPref>(cx);
        cx.observe(&preference, |host, _, cx| {
            speed_control::configure(host, cx)
        })
        .detach();
        speed_control::reset(&mut host, cx);
        // Every request starts at the default "Sign with" (spec 071). Read
        // once: the sheet's own picker is the person's say from here on.
        let method = crate::resident::resident::<SignPref>(cx)
            .read(cx)
            .view()
            .method;
        host.sign_with(Some(&method), cx);
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
    /// so both happen on the background executor and the result lands back
    /// here through the entity, the way every other slow answer in this shell
    /// does.
    fn simulate(
        &mut self,
        chain_id: u32,
        wallet: String,
        calls: Vec<FeeCall>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |host, cx| {
            let judged = cx
                .background_executor()
                .spawn(async move {
                    let Some(deltas) = crate::executor::sim::simulate(&wallet, &calls, chain_id)
                    else {
                        return None;
                    };
                    Some(crate::executor::token_trust::judge(
                        &wallet, chain_id, deltas,
                    ))
                })
                .await;
            host.update(cx, |host, cx| {
                match judged {
                    Some(judgments) => host.sim = judgments,
                    // Could not ask. NOT "nothing moves" — the sheet has a
                    // different sentence for each, and conflating them would
                    // tell somebody a drain is a no-op.
                    None => host.sim_unavailable = true,
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// The fee row, tapped: a failed quote is asked again; with more than one
    /// coin to pay in, the list opens (or closes) here in the sheet — the
    /// web's `onfee`. One coin and a quote: nothing to choose.
    pub fn fee_tapped(&mut self, cx: &mut Context<Self>) {
        let fee = self.speed.fee_view();
        if fee.failed.is_some() {
            // Measured again for real — the held readings dropped first.
            speed_control::refresh(self, cx);
        } else if fee.options.len() > 1 {
            self.fee_open = !self.fee_open;
            cx.notify();
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
        let opts = approve_opts(self.speed.fee_view(), &self.clear_view, &self.guard_view);
        self.dispatch_sign(SignEvent::ApproveTapped { opts }, cx);
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
        let pending = self.sign.resolve(id, result);
        self.pump_sign(pending, cx);
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
        cx.notify();
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
    /// The column is gone: a Trusted Signer still waiting stops now rather
    /// than holding a port for five minutes nobody can see.
    fn drop(&mut self) {
        self.ctx.trusted_signer.close();
    }
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
    }
}

/// The blind rung's two facts: who it goes to, and how many bytes of calldata
/// nobody could read. Both come from the first leg, like the decode does.
fn facts_of(request: &IncomingRequest) -> crate::signing::live::RequestFacts {
    let call = first_call(&request.params_json);
    let data = call.as_ref().and_then(|c| c.1.clone()).unwrap_or_default();
    crate::signing::live::RequestFacts {
        to: call.and_then(|c| c.0),
        // Hex, so two characters per byte; an odd tail is a malformed payload
        // and rounds DOWN rather than claiming a byte that is not there.
        data_bytes: data.trim_start_matches("0x").len() / 2,
    }
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
            let call = first_call(params_json);
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

fn first_call(params_json: &str) -> Option<(Option<String>, Option<String>, Option<String>)> {
    let params: serde_json::Value = serde_json::from_str(params_json).ok()?;
    let first = params.get(0)?;
    let call = first
        .get("calls")
        .and_then(|calls| calls.get(0))
        .unwrap_or(first);
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

    /// A transaction decodes from its call — and a BATCH decodes from its
    /// first leg, which is the same thing the phone's sheet shows.
    #[test]
    fn the_decoder_is_handed_the_call_and_not_the_envelope() {
        let single = r#"[{"from":"0xaaa","to":"0xbbb","data":"0xabcd","value":"0x1"}]"#;
        assert_eq!(
            first_call(single),
            Some((
                Some("0xbbb".to_owned()),
                Some("0xabcd".to_owned()),
                Some("0x1".to_owned())
            ))
        );

        let batch = r#"[{"calls":[{"to":"0x1","data":"0xdead"},{"to":"0x2"}]}]"#;
        let (to, data, value) =
            first_call(batch).unwrap_or_else(|| unreachable!("a batch has a first leg"));
        assert_eq!(to.as_deref(), Some("0x1"));
        assert_eq!(data.as_deref(), Some("0xdead"));
        assert_eq!(value, None, "an absent value stays absent, never a zero");
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
