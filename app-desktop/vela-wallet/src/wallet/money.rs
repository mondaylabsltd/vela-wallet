//! The send host: one `send` machine and one `fee_policy` machine, alive for
//! one journey through the send flow, driven from gpui.
//!
//! ## Why this is not two residents
//!
//! [`crate::resident`] hosts a machine for the life of the process and knows
//! nothing about its neighbours. The send journey needs the opposite on both
//! counts: its machines are born when the flow opens and discarded when it
//! closes (a second send starts from a fresh machine, not a resumed one), and
//! the two must talk — `send` asks `EstimateFee` and the answer is whatever the
//! `fee_policy` session settles on. That session is ONE object per surface:
//! the quote the core pre-checks against, the quote the confirm card shows
//! and the quote that is signed are the same estimate with the same owner.
//! The web tier records four integrations that failed by splitting it.
//!
//! So this host does what [`crate::onboarding::OnboardingPage`] does for the
//! create and login machines: holds the cores, performs their effects, runs
//! the ceremony channel's poll, and owns the few operations that belong to a
//! screen rather than to an executor.
//!
//! ## What the screen owns
//!
//! - `EstimateFee` → a deployment read, then `QuoteRequested` on the fee
//!   session, answered when that session's view settles (`busy` false, a fee
//!   or a failure). The web's `FeeQuote.requestQuote`, without the promise.
//! - `TrackSubmitted` → the app-resident tracker, whose view this host
//!   observes and forwards as `ReceiptUpdate` — only the three verdicts the
//!   core accepts.
//! - `ShowAlert` → a kind the panel words; `Close` → a flag the column reads.
//! - `SigningStarted` → raised by the sign closure the instant the prompt
//!   opens, seen by the poll, dispatched once.
//!
//! ## The speed control (spec 069)
//!
//! The fee sessions — the one in force and a preview per other tier — and the
//! reconcile step between them and the `fee_speed` core are
//! [`super::speed_control`], shared with the dApp signing sheet. What this
//! host adds is the send machine's side: its `EstimateFee` is the question
//! out on the session in force, and nothing re-prices over it.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use futures::StreamExt as _;
use gpui::{Context, Entity, FocusHandle};

use vela_core::app::Account;
use vela_core::app::batch_import::{
    BatchImport, BatchOperation, BatchShellResult, BatchToken, BatchView, Event as BatchEvent,
};
use vela_core::app::fee_policy::{
    Event as FeeEvent, FeeCall, FeeEstimateView, FeeFailure, FeeTier, FeeView,
};
use vela_core::app::fee_speed::FeeSpeedView;
use vela_core::app::fee_tier_pref::FeeTierPref;
use vela_core::app::send::{
    Event as SendEvent, Send, SendAccountRef, SendAlertKind, SendDisplayContext,
    SendEstimateFailure, SendFeeOutcome, SendOpenParams, SendOperation, SendReceiptOutcome,
    SendRecipientDraft, SendShellResult, SendView,
};
use vela_core::app::sign_pref::SignPref;
use vela_core::app::tx_tracker::{TrackStatus, TxTracker};

use crate::ceremony::CeremonyChannel;
use crate::core_host::{CoreHost, Pending};
use crate::ctap::usb::TouchRequest;
use crate::executor::passkey::{CredentialChoice, PinRequest, WindowHandle};
use crate::executor::send::{self as send_executor, SendAnswer, SendContext};
use crate::executor::trusted_signer;
use crate::executor::{batch, storage, tracker};
use crate::resident::{self, ResidentCore};
use vela_core::app::network_admin::{
    Event as NetEvent, NetView, NetWizardErrorKind, NetWizardPhase, NetworkAdmin,
};
use vela_core::app::send::SendAddNetworkOutcome;

use super::speed_control::{self, SpeedControl, SpeedHost};

/// How often the ceremony channel and the signing flag are polled while a
/// machine is busy. The same cadence onboarding uses.
const TICK_MS: u64 = 120;

/// The PIN dialog a security key raised mid-signature.
pub struct PinDialog {
    pub request: PinRequest,
    pub value: String,
    pub focus: FocusHandle,
}

/// What an applied import becomes: appended to the form's rows by default,
/// seeding them only when the person chose to replace what is there.
fn batch_apply_event(replaces: bool, recipients: Vec<SendRecipientDraft>) -> SendEvent {
    if replaces {
        SendEvent::SeedSplitRecipients { recipients }
    } else {
        SendEvent::AppendSplitRecipients { recipients }
    }
}

pub struct SendHost {
    send: CoreHost<Send>,
    /// A locked request's "add this network" in flight (078 W-04): the
    /// network admin's view is watched until its wizard settles.
    add_network: Option<gpui::Subscription>,
    pub view: SendView,
    /// The fee sessions and the speed control (spec 069): the session in
    /// force is the quote the core pre-checks against, the quote the confirm
    /// card shows and the quote that is signed.
    speed: SpeedControl,
    /// The batch importer, born when the send machine shows its sheet and
    /// gone when it hides it (spec 032 phase 5). Its own machine: the parse,
    /// the conversion and the apply gate are its, and a stale paste or rate
    /// from a previous open is never reused because the core is new.
    batch: Option<CoreHost<BatchImport>>,
    pub batch_view: Option<BatchView>,
    /// The person chose "Replace them instead": this import SEEDS the split
    /// rather than adding to the rows already on the form (issue #265). A
    /// choice about one import — it resets whenever the sheet opens.
    pub batch_replaces: bool,
    /// The display currency the sheet reads fiat figures in by default.
    display_code: String,
    ctx: SendContext,
    channel: Arc<CeremonyChannel>,
    window_handle: WindowHandle,
    /// The `EstimateFee` effect the fee session is answering.
    pending_fee: Option<u64>,
    last_fee_busy: bool,
    /// The WHOLE estimate the send machine last heard, not its charge (#686):
    /// on a floor-clamped chain two tiers charge the same wei, and a stamp of
    /// the charge alone kept the old tier's estimate in the send machine.
    last_fee: Option<FeeEstimateView>,
    /// `ShowAlert`'s kind, until the panel acknowledges it.
    pub alert: Option<SendAlertKind>,
    /// The core asked to leave the flow.
    pub closed: bool,
    pub pin: Option<PinDialog>,
    pub pick: Option<Vec<CredentialChoice>>,
    watching: bool,
    /// The last whole second the receipt re-rendered for (#D3).
    last_counted_second: Option<u64>,
    signing_reported: bool,
    tracked_hash: Option<String>,
    last_track_status: Option<TrackStatus>,
}

/// The active account, whole — the send flow signs as it.
pub fn active_account() -> Option<Account> {
    let accounts = storage::load_accounts().ok()?;
    accounts.into_iter().nth(storage::load_active_index())
}

/// The account at `address`, whole — the one a site was granted, which is
/// the one that signs for it (spec 070: never whichever happens to be active).
#[cfg_attr(
    target_os = "linux",
    allow(dead_code, reason = "Linux has no signing column to open")
)]
pub fn account_by_address(address: &str) -> Option<Account> {
    storage::load_accounts()
        .ok()?
        .into_iter()
        .find(|account| account.address.eq_ignore_ascii_case(address))
}

impl SendHost {
    pub fn open(
        account: Account,
        params: SendOpenParams,
        display: SendDisplayContext,
        window_handle: WindowHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let channel = CeremonyChannel::new();
        let mut ctx = SendContext::new(&account, channel.ceremony(window_handle));
        // The send has no "Sign with" of its own: it signs the way Settings
        // says every signature starts (spec 071), read once as it opens.
        let (trusted_signer, changed) = trusted_signer::Channel::new();
        ctx.trusted_signer = trusted_signer;
        let preference = resident::resident::<SignPref>(cx).read(cx).view();
        ctx.sign_with(&preference.method, &preference.signer_url);
        let send = CoreHost::<Send>::new();
        let view = send.view();
        let display_code = display.code.clone();
        let mut host = Self {
            send,
            view,
            speed: SpeedControl::new(),
            batch: None,
            batch_view: None,
            batch_replaces: false,
            display_code,
            ctx,
            channel,
            window_handle,
            pending_fee: None,
            last_fee_busy: false,
            last_fee: None,
            alert: None,
            closed: false,
            add_network: None,
            pin: None,
            pick: None,
            watching: false,
            last_counted_second: None,
            signing_reported: false,
            tracked_hash: None,
            last_track_status: None,
        };

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

        // Receipts arrive through the app-resident tracker, which outlives
        // this host; observing it is what turns a confirmation into the
        // receipt screen's state.
        let tracked = resident::resident::<TxTracker>(cx);
        cx.observe(&tracked, |host, tracked, cx| host.on_tracker(&tracked, cx))
            .detach();

        // The stored default speed, read once now and again whenever Settings
        // changes it — a send already open follows the new default until the
        // person picks one on it.
        let preference = resident::resident::<FeeTierPref>(cx);
        cx.observe(&preference, |host, _, cx| {
            speed_control::configure(host, cx)
        })
        .detach();
        speed_control::reset(&mut host, cx);

        host.dispatch(
            SendEvent::Open {
                account: Some(SendAccountRef {
                    id: account.id.clone(),
                    address: account.address.clone(),
                    name: (!account.name.is_empty()).then(|| account.name.clone()),
                }),
                params,
                display,
            },
            cx,
        );
        host
    }

    // -- the two machines ----------------------------------------------------

    pub fn dispatch(&mut self, event: SendEvent, cx: &mut Context<Self>) {
        let pending = self.send.dispatch(event);
        self.pump_send(pending, cx);
    }

    /// An event for the fee session in force (a fee-coin pick, say).
    pub fn fee_dispatch(&mut self, event: FeeEvent, cx: &mut Context<Self>) {
        speed_control::fee_dispatch(self, event, cx);
    }

    fn resolve_send(&mut self, id: u64, result: SendShellResult, cx: &mut Context<Self>) {
        let pending = self.send.resolve(id, result);
        self.pump_send(pending, cx);
    }

    fn pump_send(&mut self, pending: Vec<Pending<SendOperation>>, cx: &mut Context<Self>) {
        for effect in pending {
            self.perform_send(effect, cx);
        }
        self.view = self.send.view();
        self.sync_stage(cx);
        self.sync_batch(cx);
        self.ensure_watcher(cx);
        cx.notify();
    }

    // -- the batch importer ----------------------------------------------------

    /// The sheet follows the send machine's flag: open it with a fresh
    /// machine when the flag rises, drop the machine when it falls.
    fn sync_batch(&mut self, cx: &mut Context<Self>) {
        if !self.view.show_batch_import {
            if self.batch.is_some() {
                self.batch = None;
                self.batch_view = None;
                cx.notify();
            }
            return;
        }
        if self.batch.is_some() {
            return;
        }
        let Some(token) = self.view.selected_token.clone() else {
            return;
        };
        self.batch = Some(CoreHost::<BatchImport>::new());
        self.batch_replaces = false;
        self.batch_dispatch(
            BatchEvent::Open {
                token: BatchToken {
                    symbol: token.symbol,
                    decimals: token.decimals,
                    balance: token.balance,
                    price_usd: token.price_usd,
                },
                currency_code: self.display_code.clone(),
                // What an import can actually add: the core's cap less the
                // rows already started. Opened at a flat sixty, its "only the
                // first N will be sent" was a promise the append then broke by
                // truncating past it.
                max_recipients: self.view.split_import_room,
            },
            cx,
        );
    }

    pub fn batch_dispatch(&mut self, event: BatchEvent, cx: &mut Context<Self>) {
        let Some(batch) = self.batch.as_mut() else {
            return;
        };
        let pending = batch.dispatch(event);
        self.pump_batch(pending, cx);
    }

    fn resolve_batch(&mut self, id: u64, result: BatchShellResult, cx: &mut Context<Self>) {
        let Some(batch) = self.batch.as_mut() else {
            return;
        };
        let pending = batch.resolve(id, result);
        self.pump_batch(pending, cx);
    }

    /// "Replace them instead" / "Add to them instead".
    pub fn toggle_batch_merge(&mut self, cx: &mut Context<Self>) {
        self.batch_replaces = !self.batch_replaces;
        cx.notify();
    }

    /// The desktop's paste: the drawn box is not a text editor, so clicking
    /// it reads the clipboard and hands the text to the core.
    pub fn paste_into_batch(&mut self, cx: &mut Context<Self>) {
        let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
            return;
        };
        self.batch_dispatch(BatchEvent::SetRawText { text }, cx);
    }

    fn pump_batch(&mut self, pending: Vec<Pending<BatchOperation>>, cx: &mut Context<Self>) {
        for effect in pending {
            self.perform_batch(effect, cx);
        }
        let Some(batch) = self.batch.as_ref() else {
            return;
        };
        let view = batch.view();
        self.batch_view = Some(view.clone());
        cx.notify();
        if view.applied {
            // The core parsed and priced them; the send machine seeds its
            // split editor from exactly those rows, and nothing is recomputed
            // here. Ids are the core's to assign (`rcpt_{n}`).
            let recipients: Vec<SendRecipientDraft> = view
                .recipients
                .iter()
                .map(|recipient| SendRecipientDraft {
                    id: String::new(),
                    address: recipient.address.clone(),
                    amount: recipient.amount.clone(),
                    name: recipient.name.clone(),
                })
                .collect();
            self.batch = None;
            self.batch_view = None;
            // ADDED to whoever is already on the form, unless the person chose
            // to replace them: an import that silently threw typed rows away
            // was issue #265.
            if !recipients.is_empty() {
                self.dispatch(batch_apply_event(self.batch_replaces, recipients), cx);
            }
            self.dispatch(SendEvent::CloseBatchImport, cx);
        }
    }

    /// The three operations. The rate is a blocking read; the two dialogs
    /// belong to gpui and are awaited here, with the file work off-thread.
    fn perform_batch(&mut self, effect: Pending<BatchOperation>, cx: &mut Context<Self>) {
        let id = effect.id;
        match effect.operation {
            BatchOperation::FetchUsdFiatRate { code } => {
                cx.spawn(async move |host, cx| {
                    let rate = {
                        let code = code.clone();
                        cx.background_executor()
                            .spawn(async move { batch::usd_fiat_rate(&code) })
                            .await
                    };
                    host.update(cx, |host, cx| {
                        host.resolve_batch(id, BatchShellResult::RateResolved { code, rate }, cx);
                    })
                    .ok();
                })
                .detach();
            }
            BatchOperation::PickFile => {
                let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
                    files: true,
                    directories: false,
                    multiple: false,
                    prompt: None,
                });
                cx.spawn(async move |host, cx| {
                    let picked = match paths.await {
                        Ok(Ok(Some(paths))) => paths.into_iter().next(),
                        // Cancelled, or the platform declined: nothing happened.
                        _ => None,
                    };
                    let result = match picked {
                        None => BatchShellResult::FilePickCancelled,
                        Some(path) => {
                            let name = batch::file_name(&path);
                            let content = cx
                                .background_executor()
                                .spawn(async move { batch::read_table(&path) })
                                .await;
                            match content {
                                Some(content) => BatchShellResult::FilePicked { name, content },
                                None => BatchShellResult::FilePickFailed,
                            }
                        }
                    };
                    host.update(cx, |host, cx| host.resolve_batch(id, result, cx))
                        .ok();
                })
                .detach();
            }
            BatchOperation::SaveTemplateFile { name, contents, .. } => {
                // The save dialog opens where a person keeps their files, not
                // where this app keeps its state.
                let directory = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                let target = cx.prompt_for_new_path(&directory, Some(&name));
                cx.spawn(async move |host, cx| {
                    let result = match target.await {
                        Ok(Ok(Some(path))) => match std::fs::write(&path, contents) {
                            Ok(()) => BatchShellResult::TemplateSaved,
                            Err(error) => {
                                eprintln!(
                                    "[vela-wallet] batch template: {}: {error}",
                                    path.display()
                                );
                                BatchShellResult::TemplateSaveFailed
                            }
                        },
                        // A dismissed dialog keeps the plain label, silently.
                        _ => BatchShellResult::TemplateSaveFailed,
                    };
                    host.update(cx, |host, cx| host.resolve_batch(id, result, cx))
                        .ok();
                })
                .detach();
            }
        }
    }

    /// Start one send operation. The four screen-owned arms are performed
    /// here; everything else goes to the executor and, when it blocks, to
    /// the background executor with the answer routed back by effect id.
    fn perform_send(&mut self, effect: Pending<SendOperation>, cx: &mut Context<Self>) {
        let id = effect.id;
        match &effect.operation {
            SendOperation::EstimateFee {
                chain_id,
                account,
                tx,
                batch,
                gas_fee_token,
                public_key_hex,
            } => {
                // A batch takes precedence only when it HAS legs; an empty
                // one would otherwise silence the single call beside it.
                let calls = match (batch, tx) {
                    (Some(batch), _) if !batch.is_empty() => batch.clone(),
                    (_, Some(tx)) => vec![tx.clone()],
                    _ => Vec::new(),
                };
                self.request_quote(
                    id,
                    *chain_id,
                    account.clone(),
                    calls,
                    gas_fee_token.clone(),
                    public_key_hex.is_some(),
                    cx,
                );
                return;
            }
            SendOperation::TrackSubmitted {
                user_op_hash,
                record_ids,
                chain_id,
            } => {
                self.tracked_hash = Some(user_op_hash.to_lowercase());
                self.last_track_status = None;
                tracker::submitted(user_op_hash.clone(), record_ids.clone(), *chain_id, cx);
                self.resolve_send(id, SendShellResult::TrackHandedOff, cx);
                return;
            }
            // A payment link on a chain this wallet does not have (078 W-04):
            // the settings wizard's own journey — registry, chain document,
            // compatibility probe, save — asked by chain id, as the web's
            // `addCustomNetworkByChainId` asks it, and answered once the
            // admin's wizard settles. It answered "error" unconditionally.
            SendOperation::AddNetwork { chain_id } => {
                let chain_id = *chain_id;
                let admin = resident::resident::<NetworkAdmin>(cx);
                self.add_network = Some(cx.observe(&admin, move |host, admin, cx| {
                    if host.add_network.is_none() {
                        return;
                    }
                    if let Some(outcome) = add_network_settled(&admin.read(cx).view(), chain_id) {
                        host.add_network = None;
                        host.resolve_send(id, SendShellResult::NetworkAdded { outcome }, cx);
                    }
                }));
                let now_iso = crate::executor::now_iso();
                admin.update(cx, |admin, cx| {
                    admin.dispatch(NetEvent::AddByChainIdRequested { chain_id, now_iso }, cx);
                });
                return;
            }
            SendOperation::ShowAlert { kind } => {
                self.alert = Some(kind.clone());
                self.resolve_send(id, SendShellResult::AlertAcknowledged, cx);
                return;
            }
            SendOperation::Close => {
                self.closed = true;
                self.resolve_send(id, SendShellResult::Closed, cx);
                return;
            }
            // Closing the channel is what cancels a waiting ceremony; a fresh
            // one is opened for the next attempt. The executor owes the ack.
            SendOperation::CancelPasskeySign => self.cancel_ceremony(),
            SendOperation::SubmitUserOp { .. } => {
                self.ctx.signing_started.store(false, Ordering::SeqCst);
                self.signing_reported = false;
            }
            _ => {}
        }

        match send_executor::perform(&effect.operation, &self.ctx) {
            SendAnswer::Now(result) => self.resolve_send(id, result, cx),
            SendAnswer::Blocking(work) => {
                cx.spawn(async move |host, cx| {
                    let result = cx.background_executor().spawn(async move { work() }).await;
                    host.update(cx, |host, cx| host.resolve_send(id, result, cx))
                        .ok();
                })
                .detach();
            }
            SendAnswer::After(delay, result) => {
                cx.spawn(async move |host, cx| {
                    cx.background_executor().timer(delay).await;
                    host.update(cx, |host, cx| host.resolve_send(id, result, cx))
                        .ok();
                })
                .detach();
            }
            // Every screen-owned operation returned above; nothing reaches
            // here by construction.
            SendAnswer::Screen => {}
        }
    }

    // -- the fee session ------------------------------------------------------

    /// `FeeQuote.requestQuote`: read the deployment status (never guessed),
    /// then ask the session; the answer arrives when its view settles.
    ///
    /// HOW FAST is the speed control's to say (spec 068): the tier the speed
    /// core has in force — the stored default, a one-shot pick, or a free
    /// upgrade.
    #[allow(clippy::too_many_arguments, reason = "the operation's own fields")]
    fn request_quote(
        &mut self,
        effect_id: u64,
        chain_id: u32,
        account: String,
        calls: Vec<FeeCall>,
        fee_token: Option<String>,
        public_key_available: bool,
        cx: &mut Context<Self>,
    ) {
        // A newer question supersedes the last inside the core; its asker is
        // answered with a refusal rather than left waiting forever.
        if let Some(previous) = self.pending_fee.take() {
            self.resolve_send(previous, estimate_failed(), cx);
        }
        self.pending_fee = Some(effect_id);
        speed_control::ask(
            self,
            chain_id,
            account,
            public_key_available,
            calls,
            fee_token,
            cx,
        );
    }

    /// The fee session's view, as the send machine's answer — judged against
    /// the SAME view the confirm card renders, so the two cannot disagree.
    fn settle_fee(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.pending_fee else {
            return;
        };
        let fee_view = self.speed.fee_view();
        if fee_view.busy {
            return;
        }
        let outcome = if let Some(estimate) = &fee_view.fee {
            SendFeeOutcome::Ok {
                estimate: estimate.clone(),
            }
        } else if let Some(failure) = fee_view.failed {
            SendFeeOutcome::Failed {
                kind: map_failure(failure),
            }
        } else {
            // The session moved on under the question — the web's "abandoned".
            return;
        };
        self.pending_fee = None;
        self.resolve_send(id, SendShellResult::FeeEstimated { outcome }, cx);
    }

    /// The card's re-quotes, mirrored into the send machine: `busy` flips
    /// disarm the confirm slide, and a settled estimate replaces the one it
    /// pre-checked with (`GasFeeCard.onBusyChange` / `onFeeUpdate`).
    fn sync_fee_to_send(&mut self, cx: &mut Context<Self>) {
        let busy = self.speed.fee_view().busy;
        if busy != self.last_fee_busy {
            self.last_fee_busy = busy;
            self.dispatch(SendEvent::FeeBusyChanged { busy }, cx);
        }
        if let Some(fee) = self.speed.fee_view().fee.clone()
            && self.last_fee.as_ref() != Some(&fee)
        {
            self.last_fee = Some(fee.clone());
            self.dispatch(SendEvent::FeeUpdated { estimate: fee }, cx);
        }
    }

    // -- the speed control (spec 069) ----------------------------------------

    /// The fee in force, as the fee row reads it.
    pub fn fee_view(&self) -> &FeeView {
        self.speed.fee_view()
    }

    /// The speed control, as the core decided it.
    pub fn speed_view(&self) -> &FeeSpeedView {
        self.speed.view()
    }

    /// A free upgrade is only decided while the person is still choosing:
    /// the confirm must not change tier under somebody reading it.
    fn sync_stage(&mut self, cx: &mut Context<Self>) {
        let on_form = self.view.stage == vela_core::app::send::SendStage::EnterDetails;
        speed_control::stage(self, on_form, cx);
    }

    /// Fold or unfold the control.
    pub fn toggle_speed(&mut self, cx: &mut Context<Self>) {
        speed_control::toggle(self, cx);
    }

    /// A tap on an option — one-shot: it prices and submits this send and
    /// never reaches the stored preference.
    pub fn pick_speed(&mut self, tier: FeeTier, cx: &mut Context<Self>) {
        speed_control::pick(self, tier, cx);
    }

    /// The refresh control: measure again, the held readings dropped first.
    pub fn refresh_fee(&mut self, cx: &mut Context<Self>) {
        speed_control::refresh(self, cx);
    }

    /// Every (tier, view) the options can format a fee with.
    pub fn speed_tier_views(&self) -> Vec<(FeeTier, FeeView)> {
        self.speed.tier_views()
    }

    fn fees_idle(&self) -> bool {
        self.speed.idle()
    }

    // -- the tracker ----------------------------------------------------------

    fn on_tracker(&mut self, tracked: &Entity<ResidentCore<TxTracker>>, cx: &mut Context<Self>) {
        let Some(hash) = self.tracked_hash.clone() else {
            return;
        };
        let view = tracked.read(cx).view();
        let Some(entry) = view
            .entries
            .iter()
            .find(|entry| entry.user_op_hash.eq_ignore_ascii_case(&hash))
        else {
            return;
        };
        if self.last_track_status == Some(entry.status) {
            return;
        }
        self.last_track_status = Some(entry.status);
        // Only the three verdicts `ReceiptUpdate` accepts; a slow or
        // unreachable poll sends nothing (invariant ⑤).
        let outcome = match entry.status {
            TrackStatus::Confirmed => SendReceiptOutcome::Confirmed {
                tx_hash: entry.tx_hash.clone().unwrap_or_default(),
            },
            TrackStatus::Dropped => SendReceiptOutcome::Failed { rejected: false },
            TrackStatus::Rejected => SendReceiptOutcome::Failed { rejected: true },
            TrackStatus::FeeHeld => SendReceiptOutcome::FeeHeld,
            TrackStatus::Pending | TrackStatus::Unreachable | TrackStatus::AcceptedNotLanded => {
                return;
            }
        };
        self.dispatch(
            SendEvent::ReceiptUpdate {
                user_op_hash: entry.user_op_hash.clone(),
                outcome,
            },
            cx,
        );
    }

    // -- the ceremony ---------------------------------------------------------

    fn cancel_ceremony(&mut self) {
        // A Trusted Signer waiting on its page is a ceremony too.
        self.ctx.trusted_signer.cancel();
        self.channel.close();
        self.channel = CeremonyChannel::new();
        self.ctx.ceremony = self.channel.ceremony(self.window_handle);
        self.pin = None;
        self.pick = None;
    }

    /// Poll the ceremony channel and the signing flag while anything is in
    /// flight. Detached; ends by returning.
    /// A submitted receipt with a clock to count against (#D3).
    fn receipt_counting(&self) -> bool {
        self.send.view().receipt.as_ref().is_some_and(|receipt| {
            receipt.status == vela_core::app::send::SendReceiptStatus::Submitted
                && receipt.submitted_at_ms.is_some()
        })
    }

    fn ensure_watcher(&mut self, cx: &mut Context<Self>) {
        if self.watching || (self.send.is_idle() && self.fees_idle() && !self.receipt_counting()) {
            return;
        }
        self.watching = true;
        cx.spawn(async move |host, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(TICK_MS))
                    .await;
                let keep_going = host.update(cx, |host, cx| host.tick(cx)).unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    /// One poll. Returns whether to keep polling.
    fn tick(&mut self, cx: &mut Context<Self>) -> bool {
        let signing =
            self.ctx.signing_started.load(Ordering::SeqCst) || self.ctx.trusted_signer.waiting();
        if !self.signing_reported && signing {
            self.signing_reported = true;
            self.dispatch(SendEvent::SigningStarted, cx);
        }
        if let Some(request) = self.channel.pending_pin() {
            if self
                .pin
                .as_ref()
                .is_none_or(|open| open.request.retry != request.retry)
            {
                self.pin = Some(PinDialog {
                    request,
                    value: String::new(),
                    focus: cx.focus_handle(),
                });
            }
        } else if self.pin.is_some() {
            self.pin = None;
        }
        let asking = self.channel.pending_choice();
        if self.pick.is_some() != asking.is_some() {
            self.pick = asking;
        }
        // Spec 038 #D3: while the relay has the op the receipt counts seconds,
        // so the watcher keeps ticking and re-renders once per second.
        let counting = self.receipt_counting();
        if counting {
            let second = (crate::executor::now_ms() / 1000.0) as u64;
            if self.last_counted_second != Some(second) {
                self.last_counted_second = Some(second);
                cx.notify();
            }
        }
        let busy = !self.send.is_idle() || !self.fees_idle() || counting;
        cx.notify();
        if !busy {
            self.watching = false;
        }
        busy
    }

    /// What the key is waiting for right now, if anything.
    pub fn touch_waiting(&self) -> Option<TouchRequest> {
        self.channel.touch_waiting()
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

    /// The caBLE QR to show, if a hybrid ceremony waits for a scan.
    pub fn qr_showing(&self) -> Option<String> {
        self.channel.qr_showing()
    }

    pub fn answer_pin(&mut self, value: Option<String>, cx: &mut Context<Self>) {
        self.channel.answer_pin(value);
        self.pin = None;
        cx.notify();
    }

    /// The person dismissed the "touch your key" prompt: stop the exchange.
    pub fn cancel_touch(&mut self, cx: &mut Context<Self>) {
        self.channel.cancel_touch();
        cx.notify();
    }

    /// The person dismissed the hybrid QR: stop the scan behind it.
    pub fn cancel_qr(&mut self, cx: &mut Context<Self>) {
        self.channel.cancel_qr();
        cx.notify();
    }

    pub fn answer_choice(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        self.channel.answer_choice(index);
        self.pick = None;
        cx.notify();
    }

    /// The panel showed the alert.
    pub fn acknowledge_alert(&mut self, cx: &mut Context<Self>) {
        self.alert = None;
        cx.notify();
    }
}

impl Drop for SendHost {
    /// The flow is gone: a Trusted Signer still waiting stops now rather than
    /// holding a port for five minutes nobody can see.
    fn drop(&mut self) {
        self.ctx.trusted_signer.close();
    }
}

/// The send machine's side of the speed control.
impl SpeedHost for SendHost {
    fn speed_control(&mut self) -> &mut SpeedControl {
        &mut self.speed
    }

    /// Mirror the session in force into the send machine, and answer its
    /// question if it settled.
    fn in_force_changed(&mut self, cx: &mut Context<Self>) {
        self.sync_fee_to_send(cx);
        self.settle_fee(cx);
    }

    /// `EstimateFee` is out: its answer must be the question it asked.
    fn answering(&self) -> bool {
        self.pending_fee.is_some()
    }

    fn unreadable(&mut self, cx: &mut Context<Self>) {
        if let Some(id) = self.pending_fee.take() {
            self.resolve_send(id, estimate_failed(), cx);
        }
    }

    fn fees_moved(&mut self, cx: &mut Context<Self>) {
        self.ensure_watcher(cx);
    }
}

fn estimate_failed() -> SendShellResult {
    SendShellResult::FeeEstimated {
        outcome: SendFeeOutcome::Failed {
            kind: SendEstimateFailure::EstimateFailed,
        },
    }
}

/// The fee vocabulary IS the send vocabulary, one name at a time.
fn map_failure(failure: FeeFailure) -> SendEstimateFailure {
    match failure {
        FeeFailure::MissingPublicKey => SendEstimateFailure::MissingPublicKey,
        FeeFailure::FeeTokenUnavailable => SendEstimateFailure::FeeTokenUnavailable,
        FeeFailure::QuoteUnavailable => SendEstimateFailure::QuoteUnavailable,
        FeeFailure::CalculationFailed => SendEstimateFailure::CalculationFailed,
        FeeFailure::EstimateFailed => SendEstimateFailure::EstimateFailed,
        FeeFailure::GasQuoteTooHigh => SendEstimateFailure::GasQuoteTooHigh,
    }
}

/// Where the network admin's wizard has got to with `chain_id`, as the send
/// machine words it — `None` while it is still resolving and probing. The
/// web's `addCustomNetworkByChainId` mapping: saved is added; an unknown
/// chain is not found; one already present counts as added only when it is a
/// saved custom row; anything else it refused is not compatible.
fn add_network_settled(view: &NetView, chain_id: u32) -> Option<SendAddNetworkOutcome> {
    let saved = view
        .networks
        .iter()
        .any(|row| row.chain_id == chain_id && row.is_custom);
    let wizard = &view.wizard;
    if wizard.phase == NetWizardPhase::Error
        && let Some(error) = &wizard.error
    {
        return Some(match error {
            NetWizardErrorKind::NotFound { .. } => SendAddNetworkOutcome::NotFound,
            NetWizardErrorKind::AlreadyAdded { .. } if saved => SendAddNetworkOutcome::Added,
            _ => SendAddNetworkOutcome::NotCompatible { detail: None },
        });
    }
    (wizard.phase == NetWizardPhase::Idle && saved).then_some(SendAddNetworkOutcome::Added)
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::fee_policy::FeePolicy;

    /// Issue #265: an import ADDS to the recipients already on the form, and
    /// replaces them only when the person chose "Replace them instead".
    #[test]
    fn an_import_adds_to_the_form_unless_told_to_replace() {
        let rows = || {
            vec![SendRecipientDraft {
                id: String::new(),
                address: "0x00000000000000000000000000000000000000aa".to_owned(),
                amount: "1".to_owned(),
                name: Some("Ana".to_owned()),
            }]
        };
        match batch_apply_event(false, rows()) {
            SendEvent::AppendSplitRecipients { recipients } => {
                assert_eq!(recipients, rows(), "the rows go through untouched");
            }
            other => unreachable!("appended by default, not {other:?}"),
        }
        assert!(matches!(
            batch_apply_event(true, rows()),
            SendEvent::SeedSplitRecipients { .. }
        ));
    }

    /// The host without gpui: both machines pumped to quiescence on this
    /// thread, every blocking answer performed inline, the two timers left
    /// deliberately unanswered (the 15 s estimate race must be won by the
    /// estimate, and the quote's TTL is advisory). What `SendHost` does on the
    /// main thread, minus the thread.
    #[cfg(feature = "dev-fixtures")]
    struct SyncMoney {
        send: CoreHost<Send>,
        fee: CoreHost<FeePolicy>,
        ctx: SendContext,
        alerts: Vec<SendAlertKind>,
        submitted: Vec<String>,
    }

    #[cfg(feature = "dev-fixtures")]
    impl SyncMoney {
        fn dispatch(&mut self, event: SendEvent) {
            let pending = self.send.dispatch(event);
            self.pump(pending);
        }

        fn view(&self) -> SendView {
            self.send.view()
        }

        fn pump(&mut self, mut pending: Vec<Pending<SendOperation>>) {
            while let Some(effect) = pending.pop() {
                let id = effect.id;
                let result = match &effect.operation {
                    SendOperation::EstimateFee {
                        chain_id,
                        account,
                        tx,
                        batch,
                        gas_fee_token,
                        public_key_hex,
                    } => {
                        let calls = match (batch, tx) {
                            (Some(batch), _) if !batch.is_empty() => batch.clone(),
                            (_, Some(tx)) => vec![tx.clone()],
                            _ => Vec::new(),
                        };
                        let deployed = chain::is_deployed(account, *chain_id)
                            .unwrap_or_else(|e| unreachable!("{e}"));
                        let fee_pending = self.fee.dispatch(FeeEvent::QuoteRequested {
                            chain_id: *chain_id,
                            account: account.clone(),
                            deployed,
                            public_key_available: public_key_hex.is_some(),
                            tier: FeeTier::Fast,
                            calls,
                            fee_token: gas_fee_token.clone(),
                        });
                        self.pump_fee(fee_pending);
                        let view = self.fee.view();
                        assert!(!view.busy, "the fee session settles synchronously here");
                        let outcome = match (&view.fee, view.failed) {
                            (Some(estimate), _) => SendFeeOutcome::Ok {
                                estimate: estimate.clone(),
                            },
                            (None, Some(failure)) => SendFeeOutcome::Failed {
                                kind: map_failure(failure),
                            },
                            (None, None) => {
                                unreachable!("a settled session has a fee or a failure")
                            }
                        };
                        SendShellResult::FeeEstimated { outcome }
                    }
                    SendOperation::TrackSubmitted { user_op_hash, .. } => {
                        self.submitted.push(user_op_hash.clone());
                        SendShellResult::TrackHandedOff
                    }
                    SendOperation::ShowAlert { kind } => {
                        self.alerts.push(kind.clone());
                        SendShellResult::AlertAcknowledged
                    }
                    SendOperation::Close => SendShellResult::Closed,
                    // The screen's network admin is not part of this harness.
                    SendOperation::AddNetwork { .. } => SendShellResult::NetworkAdded {
                        outcome: vela_core::app::send::SendAddNetworkOutcome::Error,
                    },
                    // The estimate race: answering the timer first would make
                    // every estimate a timeout. Left pending on purpose.
                    SendOperation::StartTimer { .. } => continue,
                    _ => match send_executor::perform(&effect.operation, &self.ctx) {
                        SendAnswer::Now(result) => result,
                        SendAnswer::Blocking(work) => work(),
                        SendAnswer::After(..) => continue,
                        SendAnswer::Screen => unreachable!("every screen arm is matched above"),
                    },
                };
                pending.extend(self.send.resolve(id, result));
            }
        }

        fn pump_fee(&mut self, mut pending: Vec<Pending<FeeOperation>>) {
            while let Some(effect) = pending.pop() {
                let id = effect.id;
                let result = match <FeePolicy as Machine>::perform(&effect.operation) {
                    Answer::Now(result) => result,
                    Answer::Blocking(work) => work(),
                    // The TTL is advisory; a test does not wait thirty seconds.
                    Answer::After(..) => continue,
                    // `fee_policy` streams nothing today; if it starts, its
                    // reports belong in the machine before the result, which
                    // is the one thing this driver must not get wrong.
                    Answer::Streaming(work) => {
                        let (reports, result) = crate::resident::run_streaming(work);
                        for report in reports {
                            pending.extend(self.fee.dispatch(report));
                        }
                        result
                    }
                };
                pending.extend(self.fee.resolve(id, result));
            }
        }
    }

    /// The golden multi-key Safe, seeded as the active account from the
    /// fixture keyset — the wallet every 031 live sweep read from.
    #[cfg(feature = "dev-fixtures")]
    fn seed_golden_account() -> Account {
        use vela_core::app::AccountKey;
        use vela_core::dev_fixtures as fixtures;
        let keys = fixtures::accounts().unwrap_or_else(|e| unreachable!("{e}"));
        let account = Account {
            id: keys[0].credential_id_hex.clone(),
            name: "Golden".to_owned(),
            address: fixtures::multi_address().unwrap_or_else(|e| unreachable!("{e}")),
            public_key_hex: keys[0].public_key_hex.clone(),
            created_at_iso: "2026-09-05T00:00:00.000Z".to_owned(),
            keys: keys
                .iter()
                .map(|key| AccountKey {
                    credential_id: key.credential_id_hex.clone(),
                    public_key_hex: key.public_key_hex.clone(),
                    name: key.name.to_owned(),
                    transports: String::new(),
                })
                .collect(),
        };
        storage::save_account(&account).unwrap_or_else(|e| unreachable!("{e}"));
        storage::save_active_index(0).unwrap_or_else(|e| unreachable!("{e}"));
        account
    }

    /// The whole spine up to the confirm screen, live, for the golden Safe:
    /// its real holdings load, XDAI on Gnosis is picked, a dust transfer to
    /// fixture #2 is drafted, and `Continue` brings back the relay's real
    /// quote — the stage is Confirm and the slide is armed. No signature, no
    /// submit: nothing moves.
    ///
    /// `VELA_LIVE_SEND=1` goes one step further and slides — the parallel
    /// space's fixture #1 signs, the relay accepts, and the hash is printed.
    /// That step spends dust and is never run by default.
    #[cfg(feature = "dev-fixtures")]
    #[test]
    #[ignore = "real network; VELA_LIVE_SEND=1 spends dust"]
    fn live_the_golden_safe_reaches_confirm_with_a_real_quote() {
        crate::executor::storage::tests::with_temp_state("money-live", || {
            let account = seed_golden_account();
            let ceremony = CeremonyChannel::new().ceremony(0);
            let ctx = SendContext::new(&account, ceremony);
            let mut money = SyncMoney {
                send: CoreHost::<Send>::new(),
                fee: CoreHost::<FeePolicy>::new(),
                ctx,
                alerts: Vec::new(),
                submitted: Vec::new(),
            };
            money.dispatch(SendEvent::Open {
                account: Some(SendAccountRef {
                    id: account.id.clone(),
                    address: account.address.clone(),
                    name: Some(account.name.clone()),
                }),
                params: SendOpenParams::default(),
                display: SendDisplayContext::default(),
            });
            let view = money.view();
            println!("tokens: {}", view.tokens.len());
            for token in &view.tokens {
                println!(
                    "  {} on {} = {}",
                    token.symbol, token.chain_id, token.balance
                );
            }
            let xdai = view
                .tokens
                .iter()
                .find(|token| token.chain_id == 100 && token.token_address.is_none())
                .unwrap_or_else(|| unreachable!("the golden Safe holds xDAI on Gnosis"));
            assert!(
                money.alerts.is_empty(),
                "no alert on open: {:?}",
                money.alerts
            );

            money.dispatch(SendEvent::SelectToken {
                token_id: xdai.id(),
            });
            assert_eq!(
                money.view().stage,
                vela_core::app::send::SendStage::EnterDetails
            );

            let to = vela_core::dev_fixtures::account(1)
                .unwrap_or_else(|e| unreachable!("{e}"))
                .address;
            money.dispatch(SendEvent::SetRecipient {
                recipient: to.clone(),
            });
            money.dispatch(SendEvent::SetAmount {
                amount: "0.001".to_owned(),
            });
            let view = money.view();
            println!(
                "draft: to={to} amount={} token_amount={} warning={:?} can_continue={}",
                view.amount, view.token_amount, view.amount_warning, view.can_continue
            );
            assert!(view.can_continue, "the draft is sendable");

            money.dispatch(SendEvent::Continue);
            let view = money.view();
            println!(
                "after continue: stage={:?} fee={:?} fee_busy={} treasury={:?} alerts={:?} can_confirm={}",
                view.stage,
                view.fee.as_ref().map(|fee| (
                    &fee.total_wei,
                    &fee.fee_asset,
                    fee.fee_recipient.clone()
                )),
                view.fee_busy,
                view.treasury_bootstrap,
                money.alerts,
                view.can_confirm
            );
            assert_eq!(view.stage, vela_core::app::send::SendStage::Confirm);
            let fee = view
                .fee
                .clone()
                .unwrap_or_else(|| unreachable!("a real quote"));
            assert_eq!(fee.chain_id, 100);
            assert!(fee.quoted, "the relay's own quote, not a local fallback");
            assert!(view.can_confirm, "the slide is armed");

            if std::env::var("VELA_LIVE_SEND").as_deref() != Ok("1") {
                println!("stopping before the slide: set VELA_LIVE_SEND=1 to spend dust");
                return;
            }
            money.dispatch(SendEvent::SlideConfirm);
            let view = money.view();
            println!(
                "after slide: tx_status={:?} error={:?} user_op_hash={:?} tracked={:?}",
                view.tx_status, view.tx_error, view.user_op_hash, money.submitted
            );
            assert!(
                view.user_op_hash.is_some(),
                "the relay accepted the operation"
            );
        });
    }

    /// The importer's machine through its executor, no dialogs: a pasted
    /// two-row table in USD (which prices itself) previews, converts at the
    /// mirrored rate, and applies to exactly those two drafts — the rows the
    /// host seeds the split editor with.
    #[test]
    fn a_pasted_table_applies_to_its_rows() {
        use vela_core::app::batch_import::{
            BatchImport, BatchToken, BatchUnit, Event as BatchEvent,
        };
        let mut batch = CoreHost::<BatchImport>::new();
        let pump = |batch: &mut CoreHost<BatchImport>, event: BatchEvent| {
            let mut pending = batch.dispatch(event);
            while let Some(effect) = pending.pop() {
                let result = match effect.operation {
                    BatchOperation::FetchUsdFiatRate { code } => BatchShellResult::RateResolved {
                        rate: batch::usd_fiat_rate(&code),
                        code,
                    },
                    // The dialogs are the host's; a test has no picker to open.
                    BatchOperation::PickFile => BatchShellResult::FilePickCancelled,
                    BatchOperation::SaveTemplateFile { .. } => BatchShellResult::TemplateSaved,
                };
                pending.extend(batch.resolve(effect.id, result));
            }
        };
        pump(
            &mut batch,
            BatchEvent::Open {
                token: BatchToken {
                    symbol: "USDT".to_owned(),
                    decimals: 6,
                    balance: "20000".to_owned(),
                    price_usd: Some(1.0),
                },
                currency_code: "USD".to_owned(),
                max_recipients: 60,
            },
        );
        let view = batch.view();
        assert!(view.opened);
        assert_eq!(view.unit, BatchUnit::Fiat);
        assert_eq!(
            view.rate_status,
            vela_core::app::batch_import::BatchRateStatus::Ok
        );
        assert!(!view.rate_input.is_empty(), "USD prices itself: {view:?}");

        pump(
            &mut batch,
            BatchEvent::SetRawText {
                text: "0x031d7D57c99CAF891e1C250554691Fd12D84772b, 5000\n\
                       0x58cd0ce6A27099220543b31710d7860d75Ba1d3d, 8000\n\
                       not-an-address, 1\n"
                    .to_owned(),
            },
        );
        let view = batch.view();
        // The line with no address is the PARSER's error, not a preview row;
        // it is counted among the rejected so the person sees it was skipped.
        assert_eq!(view.preview.len(), 2);
        assert_eq!(view.recipient_count, 2);
        assert_eq!(view.rejected, 1);
        assert!(view.can_apply, "{view:?}");
        assert_eq!(view.total_token, "13000");

        pump(&mut batch, BatchEvent::Apply);
        let view = batch.view();
        assert!(view.applied);
        assert_eq!(view.recipients.len(), 2);
        assert_eq!(view.recipients[0].amount, "5000");
        assert_eq!(
            view.recipients[1].address.to_lowercase(),
            "0x58cd0ce6a27099220543b31710d7860d75ba1d3d"
        );
    }

    /// The committed workbook, all the way from the disk to payment rows.
    ///
    /// The unit test beside `read_table` proves calamine reads the file; this
    /// proves the matrix it produces is one the core turns into recipients and
    /// amounts. Between them sits the whole reason the fixture exists:
    /// "bring your payroll from Excel" is a money path, and until now every
    /// test of it used text somebody typed into the test.
    #[test]
    fn the_committed_workbook_becomes_payment_rows() {
        use vela_core::app::batch_import::{
            BatchImport, BatchToken, BatchUnit, Event as BatchEvent,
        };
        let mut batch = CoreHost::<BatchImport>::new();
        let pump = |batch: &mut CoreHost<BatchImport>, event: BatchEvent| {
            let mut pending = batch.dispatch(event);
            while let Some(effect) = pending.pop() {
                let result = match effect.operation {
                    BatchOperation::FetchUsdFiatRate { code } => BatchShellResult::RateResolved {
                        rate: batch::usd_fiat_rate(&code),
                        code,
                    },
                    // The real picker's answer, read off the real file.
                    BatchOperation::PickFile => {
                        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                            .join("tests/fixtures/payroll-sample.xlsx");
                        let content = batch::read_table(&path)
                            .unwrap_or_else(|| unreachable!("the fixture reads"));
                        BatchShellResult::FilePicked {
                            name: batch::file_name(&path),
                            content,
                        }
                    }
                    BatchOperation::SaveTemplateFile { .. } => BatchShellResult::TemplateSaved,
                };
                pending.extend(batch.resolve(effect.id, result));
            }
        };
        pump(
            &mut batch,
            BatchEvent::Open {
                token: BatchToken {
                    symbol: "USDT".to_owned(),
                    decimals: 6,
                    balance: "20000".to_owned(),
                    price_usd: Some(1.0),
                },
                currency_code: "USD".to_owned(),
                max_recipients: 60,
            },
        );
        pump(&mut batch, BatchEvent::PickFileRequested);

        let view = batch.view();
        assert_eq!(view.file_name.as_deref(), Some("payroll-sample.xlsx"));
        assert_eq!(view.unit, BatchUnit::Fiat);
        // Two priced rows; the header is not one of them, and neither is the
        // short row that carries an address and no amount.
        assert_eq!(view.preview.len(), 2, "{view:?}");
        assert_eq!(
            view.rejected, 1,
            "the amount-less row is counted, not hidden"
        );
        assert_eq!(
            view.total_token, "5173.88",
            "5000 + 173.88, at USD's own rate"
        );

        pump(&mut batch, BatchEvent::Apply);
        let view = batch.view();
        assert!(view.applied);
        assert_eq!(view.recipients.len(), 2);
        assert_eq!(view.recipients[0].amount, "5000");
        // The decimal survives the sheet, the reader and the conversion.
        assert_eq!(view.recipients[1].amount, "173.88");
    }

    /// The core refuses and the screen SAYS SO — the defect this phase
    /// exists for. Every branch is driven through the real machine (no
    /// hand-written view), and the assertion is on the sentence a person
    /// would read, not on the field behind it.
    #[test]
    fn every_refusal_the_core_makes_reaches_the_screen() {
        use crate::flows::live::{SendInputs, notice_way_out, send_confirm, send_form};
        use crate::flows::{FlowStrings, fixtures::CtaState};
        use crate::loc::Loc;
        use crate::wallet::WalletStrings;
        use vela_core::app::send::{SendChainInfo, SendToken};

        let loc = Loc::from_env();
        let s = FlowStrings::resolve(&loc);
        let wallet = WalletStrings::resolve(&loc);
        let fee = CoreHost::<FeePolicy>::new().view();
        let screen = |send: &SendView, confirming: bool| {
            let inputs = SendInputs {
                send,
                fee: &fee,
                s: &s,
                wallet: &wallet,
                locale: "en",
                money: crate::wallet::live::Money::usd(),
                identity_name: "Golden",
                identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                speed: None,
            };
            let notice = if confirming {
                send_confirm(&inputs).notice
            } else {
                send_form(&inputs).notice
            };
            let way_out = notice_way_out(&inputs, confirming);
            (notice, way_out)
        };

        // A real machine, driven to a form with one xDAI holding.
        let mut host = CoreHost::<Send>::new();
        // The alerts the core raised. `RefCell` because the closure below
        // borrows it while it is also read between steps.
        let alerts: std::cell::RefCell<Vec<SendAlertKind>> = std::cell::RefCell::new(Vec::new());
        let pump = |host: &mut CoreHost<Send>, event: SendEvent, tokens: Option<Vec<SendToken>>| {
            let mut pending = host.dispatch(event);
            while let Some(effect) = pending.pop() {
                let result = match &effect.operation {
                    SendOperation::FetchTokens { .. } => SendShellResult::TokensLoaded {
                        tokens: tokens.clone(),
                        chains: vec![SendChainInfo {
                            chain_id: 100,
                            network: "gnosis".to_owned(),
                            native_symbol: "xDAI".to_owned(),
                        }],
                    },
                    // Never answer the 15s race first, or every estimate is a
                    // timeout; every other operation answers its empty twin.
                    SendOperation::StartTimer { .. } => continue,
                    // 028 warms the relay quote from the FORM, with no call to
                    // price yet (`tx: None, batch: None`). This test is about
                    // what the core refuses while somebody types, so the warm
                    // quote is left outstanding exactly as the 15s race is.
                    SendOperation::EstimateFee { .. } => continue,
                    SendOperation::ResolveIdentity { .. } => {
                        SendShellResult::IdentityResolved { identity: None }
                    }
                    SendOperation::ResolveRisk { .. } => {
                        SendShellResult::RiskResolved { risk: None }
                    }
                    SendOperation::SimulateCalls { .. } => {
                        SendShellResult::SimResolved { sim_json: None }
                    }
                    SendOperation::LoadAccountCredential { .. } => {
                        SendShellResult::AccountCredential {
                            public_key_hex: Some("04aa".to_owned()),
                        }
                    }
                    SendOperation::ShowAlert { kind } => {
                        alerts.borrow_mut().push(kind.clone());
                        SendShellResult::AlertAcknowledged
                    }
                    other => unreachable!("unexpected on this path: {other:?}"),
                };
                pending.extend(host.resolve(effect.id, result));
            }
        };
        let token = SendToken {
            network: "gnosis".to_owned(),
            chain_id: 100,
            symbol: "xDAI".to_owned(),
            balance: "0.75".to_owned(),
            decimals: 18,
            token_address: None,
            price_usd: Some(1.0),
            logo_urls: Vec::new(),
            spam: false,
        };
        pump(
            &mut host,
            SendEvent::Open {
                account: Some(SendAccountRef {
                    id: "cred0".to_owned(),
                    address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
                    name: None,
                }),
                params: SendOpenParams::default(),
                display: SendDisplayContext::default(),
            },
            Some(vec![token.clone()]),
        );
        pump(
            &mut host,
            SendEvent::SelectToken {
                token_id: token.id(),
            },
            None,
        );
        pump(
            &mut host,
            SendEvent::SetRecipient {
                recipient: "0x031d7D57c99CAF891e1C250554691Fd12D84772b".to_owned(),
            },
            None,
        );

        // Nothing typed yet: no refusal, but the button is honestly shut.
        let view = host.view();
        assert!(screen(&view, false).0.is_none());
        assert_eq!(
            send_form_state(&view, &s, &wallet, &fee),
            CtaState::Disabled
        );

        // More than the balance — the core warns, and the sentence names the
        // coin. This is the one that used to be invisible.
        pump(
            &mut host,
            SendEvent::SetAmount {
                amount: "999".to_owned(),
            },
            None,
        );
        let view = host.view();
        let (notice, way_out) = screen(&view, false);
        let notice =
            notice.unwrap_or_else(|| unreachable!("the core warned: {:?}", view.amount_warning));
        assert!(
            notice.body.contains("xDAI"),
            "the warning names the coin: {}",
            notice.body
        );
        assert!(!notice.error, "still typing — amber, not red");
        assert_eq!(way_out, None);
        // The PORTED gate: an unlocked send leaves Continue armed on an
        // over-balance figure and refuses when it is pressed. So the sentence
        // above is the only thing on screen between the two — which is
        // exactly why dropping it left a person with a button that did
        // nothing and no explanation.
        assert!(view.can_continue, "the ported gate arms it: {view:?}");
        assert_eq!(send_form_state(&view, &s, &wallet, &fee), CtaState::Enabled);
        pump(&mut host, SendEvent::Continue, None);
        assert!(
            matches!(
                alerts.borrow().first(),
                Some(SendAlertKind::InsufficientBalance { .. })
            ),
            "pressing it refuses: {:?}",
            alerts.borrow()
        );
        assert_eq!(
            host.view().stage,
            vela_core::app::send::SendStage::EnterDetails,
            "and the flow stays on the form"
        );

        // A figure it can send: the warning clears and the button arms.
        pump(
            &mut host,
            SendEvent::SetAmount {
                amount: "0.001".to_owned(),
            },
            None,
        );
        let view = host.view();
        assert!(
            screen(&view, false).0.is_none(),
            "{:?}",
            view.amount_warning
        );
        assert!(view.can_continue);
        assert_eq!(send_form_state(&view, &s, &wallet, &fee), CtaState::Enabled);
    }

    /// The form's CTA state, for the assertions above.
    #[cfg(test)]
    fn send_form_state(
        send: &SendView,
        s: &crate::flows::FlowStrings,
        wallet: &crate::wallet::WalletStrings,
        fee: &FeeView,
    ) -> crate::flows::fixtures::CtaState {
        crate::flows::live::send_form(&crate::flows::live::SendInputs {
            send,
            fee,
            s,
            wallet,
            locale: "en",
            money: crate::wallet::live::Money::usd(),
            identity_name: "Golden",
            identity_address: "0x0",
            speed: None,
        })
        .cta_state
    }

    /// The two gates the core owns and the screen must not re-decide: a fee
    /// coin that cannot cover the fee is drawn unselectable, and a file that
    /// could not be read says so.
    #[test]
    fn the_fee_gate_and_the_unreadable_file_reach_the_screen() {
        use crate::flows::FlowStrings;
        use crate::flows::live::{SendInputs, batch_import, fee_token};
        use crate::loc::Loc;
        use crate::wallet::WalletStrings;
        use vela_core::app::batch_import::{BatchRateStatus, BatchUnit, BatchView};
        use vela_core::app::fee_policy::FeeOptionView;

        let loc = Loc::from_env();
        let s = FlowStrings::resolve(&loc);
        let wallet = WalletStrings::resolve(&loc);
        let option = |symbol: &str, insufficient: bool| FeeOptionView {
            symbol: symbol.to_owned(),
            contract: (symbol != "xDAI").then(|| format!("0x{symbol}")),
            decimals: 18,
            balance: "1000000000000000000".to_owned(),
            recipient: "0x1111111111111111111111111111111111111111".to_owned(),
            usd_balance: "1".to_owned(),
            usd_price: Some("1".to_owned()),
            amount: Some("10000000000000000".to_owned()),
            insufficient,
            selected: symbol == "xDAI",
        };
        let fee = FeeView {
            options: vec![option("xDAI", false), option("USDC", true)],
            ..CoreHost::<FeePolicy>::new().view()
        };
        let send = CoreHost::<Send>::new().view();
        let rows = fee_token(&SendInputs {
            send: &send,
            fee: &fee,
            s: &s,
            wallet: &wallet,
            locale: "en",
            money: crate::wallet::live::Money::usd(),
            identity_name: "Golden",
            identity_address: "0x0",
            speed: None,
        })
        .rows;
        assert_eq!(rows.len(), 2, "both are shown — for context");
        assert!(!rows[0].insufficient && rows[0].selected);
        assert!(
            rows[1].insufficient,
            "a coin that cannot pay the fee is not selectable"
        );

        // A picked file the shell could not read.
        let batch = BatchView {
            file_error: true,
            unit: BatchUnit::Fiat,
            fiat_code: "USD".to_owned(),
            rate_status: BatchRateStatus::Ok,
            rate_input: "1".to_owned(),
            ..CoreHost::<BatchImport>::new().view()
        };
        let notice = batch_import(&batch, "USDT", &s)
            .notice
            .unwrap_or_else(|| unreachable!("an unreadable file must say so"));
        assert!(notice.error);
        assert_eq!(notice.title, Some(s.batch_import_failed_title.clone()));

        // Over balance: said on the total line, beside the figure it is
        // about (the web's `overText`) — not a second time as a notice.
        let batch = BatchView {
            file_error: false,
            over_balance: true,
            recipient_count: 2,
            total_token: "13000".to_owned(),
            ..batch
        };
        assert!(batch_import(&batch, "USDT", &s).notice.is_none());
        let total = crate::flows::live::batch_total(&batch, "USDT", "12000", None, &s)
            .unwrap_or_else(|| unreachable!("over balance must say so"));
        assert_eq!(total.value, "13000 USDT");
        assert!(total.over.is_some());
    }

    #[test]
    fn the_fee_vocabulary_maps_one_to_one() {
        assert_eq!(
            map_failure(FeeFailure::GasQuoteTooHigh),
            SendEstimateFailure::GasQuoteTooHigh
        );
        assert_eq!(
            map_failure(FeeFailure::MissingPublicKey),
            SendEstimateFailure::MissingPublicKey
        );
        assert!(matches!(
            estimate_failed(),
            SendShellResult::FeeEstimated {
                outcome: SendFeeOutcome::Failed {
                    kind: SendEstimateFailure::EstimateFailed
                }
            }
        ));
    }

    /// The active account is the one the session index names, whole — keys
    /// included, so a multi-key wallet signs as itself.
    #[test]
    fn the_active_account_is_read_whole() {
        crate::executor::storage::tests::with_temp_state("money-active", || {
            assert!(active_account().is_none());
            let mut account = Account {
                id: "cred0".to_owned(),
                name: "Wallet".to_owned(),
                address: "0x0000000000000000000000000000000000000001".to_owned(),
                public_key_hex: "04aa".to_owned(),
                created_at_iso: String::new(),
                keys: Vec::new(),
            };
            let _ = storage::save_account(&account);
            account.id = "cred1".to_owned();
            account.address = "0x0000000000000000000000000000000000000002".to_owned();
            let _ = storage::save_account(&account);
            let _ = storage::save_active_index(1);
            assert_eq!(
                active_account().map(|a| a.address).as_deref(),
                Some("0x0000000000000000000000000000000000000002")
            );
        });
    }
}
