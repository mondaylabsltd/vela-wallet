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
//! - `EstimateFee` → `QuoteRequested` on the fee session (whose machine
//!   reads the account first, issue #483), answered when that view settles (`busy` false, a fee
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
use vela_core::app::balance_dashboard::{BalanceDashboard, BalanceView, Event as BalanceEvent};
use vela_core::app::batch_import::{
    BatchImport, BatchOperation, BatchShellResult, BatchToken, BatchView, Event as BatchEvent,
};
use vela_core::app::fee_policy::{Event as FeeEvent, FeeCall, FeeEstimateView, FeeTier, FeeView};
use vela_core::app::fee_speed::FeeSpeedView;
use vela_core::app::fee_tier_pref::FeeTierPref;
use vela_core::app::send::{
    Event as SendEvent, Send, SendAccountRef, SendAlertKind, SendDisplayContext,
    SendEstimateFailure, SendFeeOutcome, SendOpenParams, SendOperation, SendReceiptOutcome,
    SendRecipientDraft, SendShellResult, SendStage, SendView,
};
use vela_core::app::tx_tracker::{InFlightOp, TxTracker};

use crate::ceremony::CeremonyChannel;
use crate::core_host::{CoreHost, Pending};
use crate::ctap::usb::TouchRequest;
use crate::executor::passkey::{CredentialChoice, PinRequest, WindowHandle};
use crate::executor::send::{self as send_executor, SendAnswer, SendContext};
use crate::executor::trusted_signer;
use crate::executor::{balance_dashboard, batch, storage, tracker};
use crate::resident::{self, ResidentCore};
use vela_core::app::network_admin::{
    Event as NetEvent, NetView, NetWizardErrorKind, NetWizardPhase, NetworkAdmin,
};
use vela_core::app::send::SendAddNetworkOutcome;

use super::speed_control::{self, SpeedControl, SpeedHost};

/// How often the ceremony channel and the signing flag are polled while a
/// machine is busy. The same cadence onboarding uses.
const TICK_MS: u64 = 120;

/// How old the holdings may be when a send opens before the open asks the
/// dashboard for a fresh round — the web's `TOKEN_CACHE_TTL_MS`, the age at
/// which its send stops answering from the cache the home filled. The list
/// shows at once either way; this only decides whether a re-read runs
/// behind it.
const HOLDINGS_FRESH_MS: f64 = 5.0 * 60.0 * 1000.0;

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

/// What `Event::FeeTokenChanged` last told this send journey: the fee card's
/// coin and the chain it is a coin on — `None` until the first word (a
/// journey is a `SendHost`, so a fresh one has been told nothing).
///
/// The core's one bridge rule (the doc on `Event::FeeTokenChanged`; iOS
/// `SendStore.feeTokenChanged`): the send machine files the word against the
/// form's chain at the moment it is said and drops it while the form has no
/// chain. So the card's coin is told only while the fee session prices the
/// form's own chain, and whenever the pair (chain, coin) differs from what
/// was told last — a coin first seen before the form had a chain is told
/// once it has one.
#[derive(Debug, Default)]
struct FeeTokenTold(Option<(u32, Option<String>)>);

impl FeeTokenTold {
    /// The coin to tell for the card's `in_force`, priced on `pricing` while
    /// the form is on `form` — `None` when there is nothing to say.
    fn news(
        &mut self,
        in_force: &Option<String>,
        pricing: Option<u32>,
        form: Option<u32>,
    ) -> Option<Option<String>> {
        let chain_id = pricing.filter(|chain_id| Some(*chain_id) == form)?;
        if self
            .0
            .as_ref()
            .is_some_and(|(told, token)| *told == chain_id && token == in_force)
        {
            return None;
        }
        self.0 = Some((chain_id, in_force.clone()));
        Some(in_force.clone())
    }
}

/// Whether the fee card has failed (`FeeView.failure` set — a failure, or the
/// core's own re-ask after one), as this journey last told the send machine
/// (`Event::FeeFailedChanged`, the bridge beside `FeeBusyChanged`). Told when
/// it changes; a fresh journey (a new `SendHost`) has been told nothing, so
/// its first word always goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FeeFailedTold(Option<bool>);

impl FeeFailedTold {
    /// The word to tell for `failed`, or `None` when it is no news.
    fn news(&mut self, failed: bool) -> Option<bool> {
        (self.0 != Some(failed)).then(|| {
            self.0 = Some(failed);
            failed
        })
    }
}

/// The chain the send form is on: the selected token's, else the sweep's
/// (the core's `form_chain`).
fn form_chain(view: &SendView) -> Option<u32> {
    view.selected_token
        .as_ref()
        .map(|token| token.chain_id)
        .or(view.multi_chain_id)
}

pub struct SendHost {
    send: CoreHost<Send>,
    /// The account this journey sends from, fixed when it opened.
    address: String,
    /// A `FetchTokens` waiting for this account's first settled round — a
    /// send opened before the dashboard had ever finished reading it.
    pending_tokens: Option<u64>,
    /// The dashboard round the picker last received (`Settled::round`). A
    /// newer one is pushed into an open picker.
    holdings_round: Option<u64>,
    /// A waiting `FetchTokens` met a round that reached nothing, and asked the
    /// dashboard to read again: `Some(round)` is the round it met (`None`
    /// inside when there was none). The NEXT round answers, whatever it holds.
    holdings_retry: Option<Option<u64>>,
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
    /// The fee card's failure as this journey last told it ([`FeeFailedTold`]).
    fee_failed_told: FeeFailedTold,
    /// The fee card's coin in force (`FeeView.fee_token`) as this send was
    /// last told it, with its chain ([`FeeTokenTold`]).
    fee_token_told: FeeTokenTold,
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
    /// The last receipt verdict handed to the send machine — deduped on the
    /// verdict itself, not the tracker's status: a may-have-been-sent op the
    /// relay then acknowledges changes its outcome while its status stays
    /// `pending` (spec 082 RA10).
    last_receipt: Option<SendReceiptOutcome>,
    /// The operations holding their account's nonce, as last told to the send
    /// machine (`tx_tracker::in_flight_ops` of the tracker's own view) — told
    /// again only when the list changes.
    in_flight_fed: Option<Vec<InFlightOp>>,
    /// When the tracker learned the relay put this send on the network
    /// (spec 099 R6): what the landing's countdown counts from.
    pub relay_sent_at_ms: Option<f64>,
    /// The submit running on a worker (spec 082 RA4, ruling 1): the page
    /// keeps a column closed under it running, unseen, until it is in.
    submits: Submits,
}

/// A send column's submits in flight (spec 082 RA4, ruling 1): counted from
/// the `SubmitUserOp` dispatch until the core has the result — the moment the
/// payment's records are written and the tracker is handed the op, both in
/// that same turn. A column closed before then keeps its machines, unseen:
/// the POST goes on either way, and dropping them dropped the only thing that
/// would record and follow it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Submits(u32);

impl Submits {
    /// `operation` is starting on a worker; `true` when it is the submit.
    fn started(&mut self, operation: &SendOperation) -> bool {
        let submit = matches!(operation, SendOperation::SubmitUserOp { .. });
        if submit {
            self.0 += 1;
        }
        submit
    }

    /// A submit's result reached the core.
    fn settled(&mut self) {
        self.0 = self.0.saturating_sub(1);
    }

    /// Is a submit running?
    #[must_use]
    pub fn running(self) -> bool {
        self.0 > 0
    }
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
    /// Is a submit running for this column (spec 082 RA4)?
    #[must_use]
    pub fn submit_running(&self) -> bool {
        self.submits.running()
    }

    pub fn open(
        account: Account,
        params: SendOpenParams,
        display: SendDisplayContext,
        window_handle: WindowHandle,
        cx: &mut Context<Self>,
    ) -> Self {
        let channel = CeremonyChannel::new();
        let mut ctx = SendContext::new(&account, channel.ceremony(window_handle));
        // The send signs with the key the account signed in with (founder,
        // 2026-09-26), where the account reviews and signs (spec 102): its
        // trusted page, when that is its venue.
        let (trusted_signer, changed) = trusted_signer::Channel::new();
        ctx.trusted_signer = trusted_signer;
        ctx.follow_venue();
        let send = CoreHost::<Send>::new();
        let view = send.view();
        let display_code = display.code.clone();
        let mut host = Self {
            send,
            address: account.address.clone(),
            pending_tokens: None,
            holdings_round: None,
            holdings_retry: None,
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
            fee_failed_told: FeeFailedTold::default(),
            fee_token_told: FeeTokenTold::default(),
            last_fee: None,
            alert: None,
            closed: false,
            add_network: None,
            pin: None,
            pick: None,
            watching: false,
            last_counted_second: None,
            signing_reported: false,
            relay_sent_at_ms: None,
            tracked_hash: None,
            last_receipt: None,
            in_flight_fed: None,
            submits: Submits::default(),
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
        // What is in flight already holds its nonce for the first confirm,
        // not only from the tracker's next render.
        host.forward_in_flight(&tracked, cx);

        // The stored default speed, read once now and again whenever Settings
        // changes it — a send already open follows the new default until the
        // person picks one on it.
        let preference = resident::resident::<FeeTierPref>(cx);
        cx.observe(&preference, |host, _, cx| {
            speed_control::configure(host, cx)
        })
        .detach();
        speed_control::reset(&mut host, cx);

        // The picker's list is the Assets column's (`answer_tokens`), so it
        // moves when that column does: every round the dashboard settles —
        // its ten-minute tick, a pull, the refresh after a send, the window
        // coming back — reaches an open picker too.
        let dashboard = resident::resident::<BalanceDashboard>(cx);
        cx.observe(&dashboard, |host, dashboard, cx| {
            let view = dashboard.read(cx).view();
            host.on_holdings(&view, cx);
        })
        .detach();

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
        host.revalidate_holdings(cx);
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
        // The form may have just reached the chain the fee session prices.
        self.sync_fee_token(cx);
        // Back on the picker from the form: a round that settled meanwhile
        // was held back (`on_holdings`), and is due now.
        if self.view.stage == SendStage::SelectToken && self.pending_tokens.is_none() {
            let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
            self.on_holdings(&view, cx);
        }
        cx.notify();
    }

    // -- the holdings ---------------------------------------------------------

    /// Is the dashboard reading the account this send is from? A send is
    /// pinned to the account it opened for; the dashboard follows whichever
    /// is active.
    fn same_account(&self, view: &BalanceView) -> bool {
        view.address
            .as_deref()
            .is_some_and(|address| address.eq_ignore_ascii_case(&self.address))
    }

    /// `FetchTokens`, answered from the round the Assets column was drawn
    /// from instead of a second fan-out over every chain. That fan-out kept
    /// the picker blank for as long as the slowest chain took, on every open,
    /// and could disagree with the column beside it.
    ///
    /// `false` when the dashboard is reading another account: the executor's
    /// own fetch answers then.
    fn answer_tokens(&mut self, id: u64, cx: &mut Context<Self>) -> bool {
        let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        if !self.same_account(&view) {
            return false;
        }
        self.pending_tokens = Some(id);
        self.on_holdings(&view, cx);
        true
    }

    /// The dashboard changed. Answer a waiting `FetchTokens` once this
    /// account has settled a round (showing what has arrived until then), or
    /// push a newer round into an open picker.
    fn on_holdings(&mut self, view: &BalanceView, cx: &mut Context<Self>) {
        if !self.same_account(view) {
            // The person switched accounts while this send waited on its
            // first round, which will now never come: fetch it directly.
            if let Some(id) = self.pending_tokens.take() {
                self.fetch_tokens_directly(id, cx);
            }
            return;
        }
        let settled = balance_dashboard::settled(&self.address);
        if let Some(id) = self.pending_tokens {
            // One source has one failure mode: a round in which no chain
            // answered — a proxy blip at launch — used to become "could not
            // load tokens" here at once, and the picker stayed empty until
            // the next ten-minute poll. Ask the dashboard to read again first
            // (the Assets column recovers with it) and answer whatever that
            // next round holds; only a second empty round is the refusal.
            let reached_nothing = match &settled {
                Some(round) => round.tokens.is_empty() && !round.failed_chain_ids.is_empty(),
                None => view.unreachable,
            };
            if reached_nothing {
                let met = settled.as_ref().map(|round| round.round);
                match self.holdings_retry {
                    None => {
                        self.holdings_retry = Some(met);
                        balance_dashboard::dispatch(
                            BalanceEvent::RefreshRequested {
                                force: true,
                                pull: false,
                            },
                            cx,
                        );
                        return;
                    }
                    // Still the round that reached nothing: the re-read is out.
                    Some(asked) if asked == met => return,
                    Some(_) => {}
                }
            }
            self.holdings_retry = None;
            let result = match &settled {
                Some(round) => {
                    self.holdings_round = Some(round.round);
                    send_executor::tokens_loaded(&round.tokens, &round.failed_chain_ids)
                }
                // Nothing could be read and nothing is known.
                None if view.unreachable => SendShellResult::TokensLoaded {
                    tokens: None,
                    chains: send_executor::chain_infos(),
                },
                None => {
                    // The first round is still out: the chains that have
                    // answered, display-only, as the Assets column shows them.
                    if !view.tokens.is_empty() {
                        let tokens = view
                            .tokens
                            .iter()
                            .map(send_executor::to_send_token)
                            .collect();
                        self.dispatch(SendEvent::TokensPartial { tokens }, cx);
                    }
                    return;
                }
            };
            self.pending_tokens = None;
            self.resolve_send(id, result, cx);
            return;
        }
        // A newer round than the screen holds: hand it over. What follows it
        // is the core's to say (`holdings_updated`) — the picker always, the
        // form's balance and its Max too, a confirm page never, since that is
        // about the token as it was confirmed. Marked received BEFORE the
        // dispatch, so its own renders cannot hand it over again.
        let Some(round) = settled else {
            return;
        };
        if self.holdings_round.is_some_and(|held| held != round.round) {
            self.holdings_round = Some(round.round);
            let tokens = round
                .tokens
                .iter()
                .map(send_executor::to_send_token)
                .collect();
            self.dispatch(SendEvent::HoldingsUpdated { tokens }, cx);
        }
    }

    /// Holdings older than [`HOLDINGS_FRESH_MS`] are shown at once and read
    /// again behind them; the new round replaces the list when it settles.
    fn revalidate_holdings(&self, cx: &mut Context<Self>) {
        let Some(round) = balance_dashboard::settled(&self.address) else {
            // Never settled: the first round is already out.
            return;
        };
        let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        if self.same_account(&view) && crate::executor::now_ms() - round.at_ms > HOLDINGS_FRESH_MS {
            balance_dashboard::dispatch(
                BalanceEvent::RefreshRequested {
                    force: false,
                    pull: false,
                },
                cx,
            );
        }
    }

    /// The executor's own fan-out, for a send the dashboard cannot answer.
    fn fetch_tokens_directly(&mut self, id: u64, cx: &mut Context<Self>) {
        let operation = SendOperation::FetchTokens {
            address: self.address.clone(),
        };
        if let SendAnswer::Blocking(work) = send_executor::perform(&operation, &self.ctx) {
            cx.spawn(async move |host, cx| {
                let result = cx.background_executor().spawn(async move { work() }).await;
                host.update(cx, |host, cx| host.resolve_send(id, result, cx))
                    .ok();
            })
            .detach();
        }
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
                // where this app keeps its state (spec 095: Downloads).
                let directory = crate::executor::storage::save_panel_dir();
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
                auto_fee_token,
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
                    *auto_fee_token,
                    public_key_hex.is_some(),
                    cx,
                );
                return;
            }
            SendOperation::TrackSubmitted {
                user_op_hash,
                record_ids,
                chain_id,
                maybe_sent,
                submit_block,
                admitted,
                sender,
            } => {
                self.tracked_hash = Some(user_op_hash.to_lowercase());
                self.last_receipt = None;
                tracker::submitted(
                    tracker::Handoff {
                        user_op_hash: user_op_hash.clone(),
                        record_ids: record_ids.clone(),
                        chain_id: *chain_id,
                        maybe_sent: *maybe_sent,
                        submit_block: *submit_block,
                        admitted: *admitted,
                        sender: sender.clone(),
                    },
                    cx,
                );
                self.resolve_send(id, SendShellResult::TrackHandedOff, cx);
                return;
            }
            // Spec 082 RJ1: the written-ahead payment was proven never sent;
            // the tracker forgets those records (it never patches them).
            SendOperation::TrackWithdrawn {
                user_op_hash,
                record_ids,
            } => {
                tracker::withdrawn(user_op_hash, record_ids, cx);
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
            SendOperation::FetchTokens { .. } => {
                if self.answer_tokens(id, cx) {
                    return;
                }
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
                let submit = self.submits.started(&effect.operation);
                cx.spawn(async move |host, cx| {
                    let result = cx.background_executor().spawn(async move { work() }).await;
                    host.update(cx, |host, cx| {
                        // Settled before the core hears it: the records and
                        // the tracker hand-off happen in this same turn, and
                        // the redraw it causes lets a closed column go.
                        if submit {
                            host.submits.settled();
                        }
                        host.resolve_send(id, result, cx);
                    })
                    .ok();
                })
                .detach();
            }
            // The submit (spec 082 RJ1): its `OpSigned` reaches the core
            // while it waits for the core's clearance, and the result after.
            SendAnswer::Streaming(work) => {
                let submit = self.submits.started(&effect.operation);
                let (tx, mut rx) = futures::channel::mpsc::unbounded();
                cx.spawn(async move |host, cx| {
                    let work = cx
                        .background_executor()
                        .spawn(async move { work(&resident::Sink::new(tx)) });
                    while let Some(event) = rx.next().await {
                        host.update(cx, |host, cx| host.dispatch(event, cx)).ok();
                    }
                    let result = work.await;
                    host.update(cx, |host, cx| {
                        if submit {
                            host.submits.settled();
                        }
                        host.resolve_send(id, result, cx);
                    })
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

    /// `FeeQuote.requestQuote`: ask the session — its machine reads the
    /// account first, never guessed (issue #483); the answer arrives when its
    /// view settles.
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
        auto_fee_token: bool,
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
            auto_fee_token,
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
                kind: SendEstimateFailure::from(failure),
            }
        } else {
            // The session moved on under the question — the web's "abandoned".
            return;
        };
        self.pending_fee = None;
        self.resolve_send(id, SendShellResult::FeeEstimated { outcome }, cx);
    }

    /// The card's re-quotes, mirrored into the send machine: `busy` flips
    /// disarm the confirm, the coin in force names the fee row's coin while
    /// no estimate is in hand, and a settled estimate replaces the one it
    /// pre-checked with (`GasFeeCard.onBusyChange` / `onFeeUpdate`).
    fn sync_fee_to_send(&mut self, cx: &mut Context<Self>) {
        let busy = self.speed.fee_view().busy;
        if busy != self.last_fee_busy {
            self.last_fee_busy = busy;
            self.dispatch(SendEvent::FeeBusyChanged { busy }, cx);
        }
        // The card failed (or is re-asking after a failure) and holds no
        // figure: the send machine holds its confirm and, on the confirm
        // page, drops the figure it kept from Continue — never a confirm open
        // on a discarded figure between two of the core's re-asks.
        if let Some(failed) = self
            .fee_failed_told
            .news(self.speed.fee_view().failure.is_some())
        {
            if failed {
                // The figure the machine just dropped is told again when it
                // comes back, even the same one.
                self.last_fee = None;
            }
            self.dispatch(SendEvent::FeeFailedChanged { failed }, cx);
        }
        self.sync_fee_token(cx);
        if let Some(fee) = self.speed.fee_view().fee.clone()
            && self.last_fee.as_ref() != Some(&fee)
        {
            self.last_fee = Some(fee.clone());
            self.dispatch(SendEvent::FeeUpdated { estimate: fee }, cx);
        }
    }

    /// The card's coin in force into the send machine, under the bridge rule
    /// ([`FeeTokenTold`]). Run when the fee session moves AND when the send
    /// form does: a coin said before the form had a chain is told once the
    /// form is on the chain the session prices.
    fn sync_fee_token(&mut self, cx: &mut Context<Self>) {
        let pricing = self.speed.pricing_chain_id();
        let form = form_chain(&self.view);
        if let Some(fee_token) =
            self.fee_token_told
                .news(&self.speed.fee_view().fee_token, pricing, form)
        {
            self.dispatch(SendEvent::FeeTokenChanged { fee_token }, cx);
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

    /// Every operation in flight on this device, from the tracker's own view
    /// (`tx_tracker::in_flight_ops`, which reads `sender` and `stalled`), to
    /// the send machine: while the account has one on the form's chain, the
    /// confirm is held (`SendView.previous_pending`) — a second send signed
    /// now would take the same nonce. The machine dedupes too; this only
    /// keeps an unchanged list from being dispatched on every render.
    fn forward_in_flight(
        &mut self,
        tracked: &Entity<ResidentCore<TxTracker>>,
        cx: &mut Context<Self>,
    ) {
        let view = tracked.read(cx).view();
        if let Some(ops) = tracker::in_flight_news(&view, &mut self.in_flight_fed) {
            self.dispatch(SendEvent::InFlightOps { ops }, cx);
        }
    }

    fn on_tracker(&mut self, tracked: &Entity<ResidentCore<TxTracker>>, cx: &mut Context<Self>) {
        self.forward_in_flight(tracked, cx);
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
        if entry.relay_sent_at_ms != self.relay_sent_at_ms {
            self.relay_sent_at_ms = entry.relay_sent_at_ms;
            cx.notify();
        }
        // The core's one mapping (spec 082): a slow or unreachable poll sends
        // nothing (invariant ⑤); "not sent" is never the fee-rejected words.
        let Some(outcome) = vela_core::app::send::receipt_outcome_of(entry) else {
            return;
        };
        if self.last_receipt.as_ref() == Some(&outcome) {
            return;
        }
        self.last_receipt = Some(outcome.clone());
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

    /// Spec 102 D4: the hand-off card's facts, when this account reviews and
    /// signs on a trusted page (or nothing here can reach its keys).
    pub fn handoff(&self) -> Option<send_executor::Handoff> {
        self.ctx.handoff()
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
    use vela_core::app::send::{SendChainInfo, SendFeeCoin, SendToken};

    const BSC_USDT: &str = "0x55d398326f99059ff775485246999027b3197955";

    /// A send journey on the real machine, its effects answered inline: the
    /// holdings below, and every warm quote FAILED — the form has no estimate
    /// in hand, so its fee row names the coin in force (`SendView.fee_coin`).
    struct Journey(CoreHost<Send>);

    impl Journey {
        fn holding(
            chain_id: u32,
            network: &str,
            symbol: &str,
            contract: Option<&str>,
        ) -> SendToken {
            SendToken {
                network: network.to_owned(),
                chain_id,
                symbol: symbol.to_owned(),
                balance: "5".to_owned(),
                decimals: 18,
                token_address: contract.map(str::to_owned),
                price_usd: Some(1.0),
                logo_urls: Vec::new(),
                spam: false,
            }
        }

        fn bnb() -> SendToken {
            Self::holding(56, "bsc", "BNB", None)
        }

        fn eth() -> SendToken {
            Self::holding(1, "ethereum", "ETH", None)
        }

        fn open() -> Self {
            let mut journey = Self(CoreHost::<Send>::new());
            journey.drive(SendEvent::Open {
                account: Some(SendAccountRef {
                    id: "cred0".to_owned(),
                    address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
                    name: None,
                }),
                params: SendOpenParams::default(),
                display: SendDisplayContext::default(),
            });
            journey
        }

        fn drive(&mut self, event: SendEvent) {
            let mut pending = self.0.dispatch(event);
            while let Some(effect) = pending.pop() {
                let result = match &effect.operation {
                    SendOperation::FetchTokens { .. } => SendShellResult::TokensLoaded {
                        tokens: Some(vec![
                            Self::bnb(),
                            Self::holding(56, "bsc", "USDT", Some(BSC_USDT)),
                            Self::eth(),
                        ]),
                        chains: vec![
                            SendChainInfo {
                                chain_id: 56,
                                network: "bsc".to_owned(),
                                native_symbol: "BNB".to_owned(),
                            },
                            SendChainInfo {
                                chain_id: 1,
                                network: "ethereum".to_owned(),
                                native_symbol: "ETH".to_owned(),
                            },
                        ],
                    },
                    SendOperation::LoadAccountCredential { .. } => {
                        SendShellResult::AccountCredential {
                            public_key_hex: Some("04aa".to_owned()),
                        }
                    }
                    SendOperation::PrewarmFees { .. } => SendShellResult::FeesPrewarmed,
                    SendOperation::EstimateFee { .. } => SendShellResult::FeeEstimated {
                        outcome: SendFeeOutcome::Failed {
                            kind: SendEstimateFailure::QuoteUnavailable,
                        },
                    },
                    _ => continue,
                };
                pending.extend(self.0.resolve(effect.id, result));
            }
        }

        fn pick(&mut self, token: &SendToken) {
            self.drive(SendEvent::SelectToken {
                token_id: token.id(),
            });
            assert_eq!(self.view().stage, SendStage::EnterDetails);
        }

        fn view(&self) -> SendView {
            self.0.view()
        }

        fn coin(&self) -> Option<SendFeeCoin> {
            self.view().fee_coin
        }

        /// The bridge, as `SendHost::sync_fee_token` runs it: the card's coin
        /// `token`, with its session pricing `pricing`.
        fn tell(
            &mut self,
            told: &mut FeeTokenTold,
            token: &Option<String>,
            pricing: Option<u32>,
        ) -> Option<Option<String>> {
            let news = told.news(token, pricing, form_chain(&self.view()))?;
            self.drive(SendEvent::FeeTokenChanged {
                fee_token: news.clone(),
            });
            Some(news)
        }
    }

    /// The card's failure reaches the send machine once per change, and a
    /// fresh journey's first word always goes (`FeeFailedChanged`, PR 2
    /// integration): on the confirm page a failure holds the confirm and
    /// drops the figure kept from Continue — the row draws the failure in
    /// its place — and a settled quote brings both back. Through the real
    /// send core, with the real fee core's own failure and quote.
    #[test]
    fn a_failed_fee_card_holds_the_confirm_and_drops_its_figure() {
        let mut told = FeeFailedTold::default();
        assert_eq!(
            told.news(false),
            Some(false),
            "a fresh journey's first word"
        );
        assert_eq!(told.news(false), None, "no news");
        assert_eq!(told.news(true), Some(true));
        assert_eq!(told.news(true), None);
        assert_eq!(told.news(false), Some(false));

        let [quoted, ..] = crate::signing::fixtures::fee_coin_switch();
        let estimate = quoted
            .fee
            .clone()
            .unwrap_or_else(|| unreachable!("the real fee core priced it"));
        let failures = crate::signing::fixtures::fee_failures();
        let mut send = CoreHost::<Send>::new();
        let drive = |send: &mut CoreHost<Send>, event: SendEvent| {
            let mut pending = send.dispatch(event);
            while let Some(effect) = pending.pop() {
                let result = match &effect.operation {
                    SendOperation::FetchTokens { .. } => SendShellResult::TokensLoaded {
                        tokens: Some(vec![Journey::eth()]),
                        chains: vec![SendChainInfo {
                            chain_id: 1,
                            network: "ethereum".to_owned(),
                            native_symbol: "ETH".to_owned(),
                        }],
                    },
                    SendOperation::LoadAccountCredential { .. } => {
                        SendShellResult::AccountCredential {
                            public_key_hex: Some("04aa".to_owned()),
                        }
                    }
                    SendOperation::PrewarmFees { .. } => SendShellResult::FeesPrewarmed,
                    SendOperation::ProbeTreasury { .. } => SendShellResult::TreasuryProbed {
                        probe: vela_core::app::send::SendTreasuryProbe::Covered,
                    },
                    SendOperation::ResolveIdentity { .. } => {
                        SendShellResult::IdentityResolved { identity: None }
                    }
                    SendOperation::EstimateFee { .. } => SendShellResult::FeeEstimated {
                        outcome: SendFeeOutcome::Ok {
                            estimate: estimate.clone(),
                        },
                    },
                    // Timers stay out: nothing here waits on a clock.
                    _ => continue,
                };
                pending.extend(send.resolve(effect.id, result));
            }
        };
        drive(
            &mut send,
            SendEvent::Open {
                account: Some(SendAccountRef {
                    id: "cred0".to_owned(),
                    address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
                    name: None,
                }),
                params: SendOpenParams::default(),
                display: SendDisplayContext::default(),
            },
        );
        drive(
            &mut send,
            SendEvent::SelectToken {
                token_id: Journey::eth().id(),
            },
        );
        drive(
            &mut send,
            SendEvent::SetRecipient {
                recipient: "0x2222222222222222222222222222222222222222".to_owned(),
            },
        );
        drive(
            &mut send,
            SendEvent::SetAmount {
                amount: "0.001".to_owned(),
            },
        );
        drive(&mut send, SendEvent::Continue);
        let view = send.view();
        assert_eq!(view.stage, SendStage::Confirm);
        assert!(view.fee.is_some(), "Continue's figure");
        let armed = view.can_confirm;

        // The card failed: the bridge tells it, the confirm is held, and the
        // figure the fee machine discarded is gone from the confirm too.
        let mut told = FeeFailedTold::default();
        let failed = told
            .news(failures.down.failure.is_some())
            .unwrap_or_else(|| unreachable!("a first word"));
        drive(&mut send, SendEvent::FeeFailedChanged { failed });
        let held = send.view();
        assert!(!held.can_confirm, "held while the card has failed");
        assert!(held.fee.is_none(), "no figure the fee machine discarded");
        // …and so, the row draws the failure in its place.
        let s = crate::flows::FlowStrings::resolve(&crate::loc::Loc::from_env());
        let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
        let inputs = crate::flows::live::SendInputs {
            send: &held,
            fee: &failures.down,
            s: &s,
            wallet: &wallet,
            locale: "en",
            money: crate::wallet::live::Money::usd(),
            identity_name: "Golden",
            identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            speed: None,
            relay_sent_at_ms: None,
        };
        assert_eq!(
            crate::flows::live::confirm_fee_fact(&inputs).value.as_ref(),
            "—"
        );
        // Through the core's re-ask it stays held: no news for the bridge.
        assert_eq!(told.news(failures.retrying.failure.is_some()), None);

        // The quote settles again: the figure and the confirm are back.
        assert_eq!(told.news(false), Some(false));
        drive(&mut send, SendEvent::FeeFailedChanged { failed: false });
        drive(
            &mut send,
            SendEvent::FeeUpdated {
                estimate: estimate.clone(),
            },
        );
        let back = send.view();
        assert!(back.fee.is_some(), "the figure is back");
        assert_eq!(back.can_confirm, armed, "the gate as it was before");
        assert!(armed, "the confirm was armed on a settled figure");
    }

    /// The bridge beside `FeeBusyChanged` (the core's rule, iOS
    /// `SendStore.feeTokenChanged`): the card's coin reaches the send machine
    /// whenever the pair (chain, coin) differs from what this journey was
    /// last told, only while the fee session prices the form's own chain —
    /// the core files the word against the form's chain at the moment it is
    /// said, and drops it while the form has none. A fresh journey (a new
    /// `SendHost`) has been told nothing.
    #[test]
    fn the_fee_cards_coin_is_told_once_per_change_and_again_to_a_new_journey() {
        let usdt = Some(BSC_USDT.to_owned());
        let mut told = FeeTokenTold::default();
        let mut send = Journey::open();
        // No chain on the form yet: a word now is about no chain at all.
        assert_eq!(send.tell(&mut told, &usdt, Some(56)), None);
        send.pick(&Journey::bnb());
        assert_eq!(
            send.coin().and_then(|coin| coin.contract),
            None,
            "nothing was told before the form had a chain"
        );

        assert_eq!(send.tell(&mut told, &usdt, Some(56)), Some(usdt.clone()));
        assert_eq!(
            send.coin(),
            Some(SendFeeCoin {
                symbol: "USDT".to_owned(),
                contract: usdt.clone(),
                chain_id: 56,
            }),
            "the card's coin names the row"
        );

        // The person picks the chain's own coin: newer than the card's word.
        send.drive(SendEvent::ChooseFeeToken { token: None });
        assert_eq!(send.coin().and_then(|coin| coin.contract), None);
        // The card has not spoken again — the same word is not said twice, or
        // it would undo the person's pick.
        assert_eq!(
            send.tell(&mut told, &usdt, Some(56)),
            None,
            "an unchanged coin is not told again"
        );
        assert_eq!(send.coin().and_then(|coin| coin.contract), None);
        assert_eq!(send.tell(&mut told, &None, Some(56)), Some(None));
        // A session still pricing another chain says nothing about this one.
        assert_eq!(
            send.tell(&mut told, &usdt, Some(1)),
            None,
            "another chain's coin is not told"
        );
        assert_eq!(
            send.tell(&mut told, &usdt, None),
            None,
            "nor is a session that has priced nothing"
        );
        assert_eq!(send.coin().and_then(|coin| coin.contract), None);
        assert_eq!(send.tell(&mut told, &usdt, Some(56)), Some(usdt.clone()));
        assert_eq!(
            send.coin().and_then(|coin| coin.contract),
            usdt,
            "a change is told"
        );

        // A new journey has been told nothing: the same coin is told again.
        let mut told = FeeTokenTold::default();
        let mut send = Journey::open();
        send.pick(&Journey::bnb());
        assert_eq!(send.tell(&mut told, &usdt, Some(56)), Some(usdt.clone()));
        assert_eq!(
            send.coin().and_then(|coin| coin.contract),
            usdt,
            "a fresh journey hears the card's coin"
        );
    }

    /// The pair, not the coin: "the chain's own coin" said on BNB Chain is
    /// news again once the form and the session are on Ethereum.
    #[test]
    fn the_same_coin_is_told_again_when_the_form_moves_to_another_chain() {
        let mut told = FeeTokenTold::default();
        let mut send = Journey::open();
        send.pick(&Journey::bnb());
        assert_eq!(send.tell(&mut told, &None, Some(56)), Some(None));

        send.drive(SendEvent::ChangeToken);
        send.pick(&Journey::eth());
        assert_eq!(
            send.tell(&mut told, &None, Some(56)),
            None,
            "the network just left says nothing about this one"
        );
        assert_eq!(send.tell(&mut told, &None, Some(1)), Some(None));
        assert_eq!(send.tell(&mut told, &None, Some(1)), None);
        assert_eq!(
            send.coin(),
            Some(SendFeeCoin {
                symbol: "ETH".to_owned(),
                contract: None,
                chain_id: 1,
            })
        );
    }

    // The sync driver's own: the host hands its fee sessions to
    // `speed_control` (069), so nothing above imports these any more.
    #[cfg(feature = "dev-fixtures")]
    use crate::executor::chain;
    #[cfg(feature = "dev-fixtures")]
    use crate::resident::{Answer, Machine};
    #[cfg(feature = "dev-fixtures")]
    use vela_core::app::fee_policy::FeeOperation;

    /// Spec 082 RA4 / ruling 1: a send column closed (✕, Esc, another
    /// panel, another flow) while its submit runs must not take the submit's
    /// answer with it — that answer is what writes the payment's record and
    /// hands it to the tracker, and the POST goes on either way. The column
    /// counts the submit from its dispatch until the core has its result;
    /// only then may its machines go.
    #[test]
    fn a_column_closed_mid_submit_runs_until_its_result_is_in() {
        let mut submits = Submits::default();
        assert!(!submits.running());
        assert!(
            !submits.started(&SendOperation::CancelPasskeySign),
            "only the submit is money in flight"
        );
        assert!(!submits.running());
        let submit = SendOperation::SubmitUserOp {
            chain_id: 100,
            account: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
            public_key_hex: "04aa".to_owned(),
            calls: Vec::new(),
            max_fee_per_gas: None,
            gas_fee_token: None,
            quoted_fee: None,
        };
        assert!(submits.started(&submit));
        assert!(submits.running(), "closed now, it runs on unseen");
        submits.settled();
        assert!(!submits.running(), "its result is in: the machines may go");
        submits.settled();
        assert!(!submits.running(), "never below nothing");
    }

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
                        auto_fee_token,
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
                            auto_fee_token: *auto_fee_token,
                            number: crate::executor::format_prefs::current().number,
                        });
                        self.pump_fee(fee_pending);
                        let view = self.fee.view();
                        assert!(!view.busy, "the fee session settles synchronously here");
                        let outcome = match (&view.fee, view.failed) {
                            (Some(estimate), _) => SendFeeOutcome::Ok {
                                estimate: estimate.clone(),
                            },
                            (None, Some(failure)) => SendFeeOutcome::Failed {
                                kind: SendEstimateFailure::from(failure),
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
                    SendOperation::TrackWithdrawn { .. } => SendShellResult::TrackHandedOff,
                    _ => match send_executor::perform(&effect.operation, &self.ctx) {
                        SendAnswer::Now(result) => result,
                        SendAnswer::Blocking(work) => work(),
                        // The submit (RJ1): its `OpSigned` is dispatched here
                        // while the worker waits for the clearance it causes.
                        SendAnswer::Streaming(work) => {
                            let (tx, mut rx) = futures::channel::mpsc::unbounded();
                            let worker = std::thread::spawn(move || work(&resident::Sink::new(tx)));
                            loop {
                                match rx.try_recv() {
                                    Ok(event) => self.dispatch(event),
                                    Err(futures::channel::mpsc::TryRecvError::Closed) => break,
                                    Err(_) => {
                                        std::thread::sleep(std::time::Duration::from_millis(10))
                                    }
                                }
                            }
                            worker
                                .join()
                                .unwrap_or_else(|_| unreachable!("the submit panicked"))
                        }
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
                    signer_origin: None,
                })
                .collect(),
            sign_in_key: None,
            signing_domain: vela_core::signing_venue::APP_DOMAIN.to_owned(),
            signing_venue: vela_core::signing_venue::SigningVenue::InVela,
        };
        storage::save_account(&account).unwrap_or_else(|e| unreachable!("{e}"));
        storage::save_active_index(0).unwrap_or_else(|e| unreachable!("{e}"));
        account
    }

    /// The whole spine up to the confirm screen, live, for the golden Safe:
    /// its real holdings load, XDAI on Gnosis is picked, a dust transfer to
    /// fixture #2 is drafted, and `Continue` brings back the relay's real
    /// quote — the stage is Confirm and the confirm is armed. No signature, no
    /// submit: nothing moves.
    ///
    /// `VELA_LIVE_SEND=1` goes one step further and confirms — the parallel
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
            assert!(view.can_confirm, "the confirm is armed");

            if std::env::var("VELA_LIVE_SEND").as_deref() != Ok("1") {
                println!("stopping before the confirm: set VELA_LIVE_SEND=1 to spend dust");
                return;
            }
            money.dispatch(SendEvent::SlideConfirm);
            let view = money.view();
            println!(
                "after confirm: tx_status={:?} error={:?} user_op_hash={:?} tracked={:?}",
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
                relay_sent_at_ms: None,
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
                    // The picker's read-ahead is fire-and-forget.
                    SendOperation::PrewarmFees { .. } => SendShellResult::FeesPrewarmed,
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

    /// Issue #421 on a real machine, keyed exactly as the page keys it: the
    /// field's text through `amount_edited` into `SetAmount`, and the field
    /// redrawn from the core's figure (`amount_to_input`). "0" then "8" is 8
    /// in the field, 8 under it (`token_amount`, which the fiat line and the
    /// call both read) and 8 at Continue — never an "08" read as 8.
    #[test]
    fn a_zero_then_an_eight_is_eight_in_the_field_and_in_the_core() {
        use crate::flows::live::{amount_edited_with, amount_to_input_with};
        use vela_core::app::send::{SendChainInfo, SendToken};
        use vela_core::l10n::number::NumberPreset;

        let token = SendToken {
            network: "polygon".to_owned(),
            chain_id: 137,
            symbol: "POL".to_owned(),
            balance: "20".to_owned(),
            decimals: 18,
            token_address: None,
            price_usd: Some(0.107),
            logo_urls: Vec::new(),
            spam: false,
        };
        let pump = |host: &mut CoreHost<Send>, event: SendEvent| {
            let mut pending = host.dispatch(event);
            while let Some(effect) = pending.pop() {
                let result = match &effect.operation {
                    SendOperation::FetchTokens { .. } => SendShellResult::TokensLoaded {
                        tokens: Some(vec![token.clone()]),
                        chains: vec![SendChainInfo {
                            chain_id: 137,
                            network: "polygon".to_owned(),
                            native_symbol: "POL".to_owned(),
                        }],
                    },
                    // The 15 s race and the warm quote stay outstanding: this
                    // is about the figure, not the fee.
                    SendOperation::StartTimer { .. } | SendOperation::EstimateFee { .. } => {
                        continue;
                    }
                    SendOperation::ResolveIdentity { .. } => {
                        SendShellResult::IdentityResolved { identity: None }
                    }
                    SendOperation::ResolveRisk { .. } => {
                        SendShellResult::RiskResolved { risk: None }
                    }
                    SendOperation::LoadAccountCredential { .. } => {
                        SendShellResult::AccountCredential {
                            public_key_hex: Some("04aa".to_owned()),
                        }
                    }
                    SendOperation::PrewarmFees { .. } => SendShellResult::FeesPrewarmed,
                    other => unreachable!("unexpected on this path: {other:?}"),
                };
                pending.extend(host.resolve(effect.id, result));
            }
        };

        for (preset, keys, field_after, held) in [
            (NumberPreset::CommaDot, "08", "8", "8"),
            (NumberPreset::CommaDot, "0.8", "0.8", "0.8"),
            (NumberPreset::CommaDot, ".5", "0.5", "0.5"),
            (NumberPreset::DotComma, "08", "8", "8"),
            (NumberPreset::DotComma, "0,8", "0,8", "0.8"),
        ] {
            let mut host = CoreHost::<Send>::new();
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
            );
            pump(
                &mut host,
                SendEvent::SelectToken {
                    token_id: token.id(),
                },
            );
            pump(
                &mut host,
                SendEvent::SetRecipient {
                    recipient: "0x031d7D57c99CAF891e1C250554691Fd12D84772b".to_owned(),
                },
            );
            // The page: `value` is the core's figure in the person's mark,
            // `previous` is that same text, and only a cleaned edit is sent.
            for key in keys.chars() {
                let field = amount_to_input_with(&host.view().amount, preset);
                let next = format!("{field}{key}");
                if let Some(amount) = amount_edited_with(&next, &field, preset) {
                    pump(&mut host, SendEvent::SetAmount { amount });
                }
            }
            let view = host.view();
            assert_eq!(
                amount_to_input_with(&view.amount, preset),
                field_after,
                "{keys:?} under {preset:?}: what the field shows"
            );
            assert_eq!(view.amount, held, "{keys:?}: what the core holds");
            assert_eq!(view.token_amount, held, "{keys:?}: what is priced and sent");
            assert!(view.can_continue, "{keys:?}: a figure that can be sent");
        }
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
            relay_sent_at_ms: None,
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
            spent_by_operation: false,
            short: None,
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
            relay_sent_at_ms: None,
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
        assert_eq!(notice.body, s.batch_import_failed_body);

        // 087: a file in a legacy code page says how to save it.
        let legacy = BatchView {
            file_failure: Some(vela_core::app::batch_import::BatchFileFailure::UnsupportedEncoding),
            ..batch.clone()
        };
        let notice = batch_import(&legacy, "USDT", &s)
            .notice
            .unwrap_or_else(|| unreachable!("a refused file must say so"));
        assert_eq!(notice.body, s.batch_import_failed_encoding);
        assert_ne!(notice.body, s.batch_import_failed_body);

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

    /// PR 2 note 13: Continue's estimate passes the fee machine's failure
    /// through as it is (the core's `From<FeeFailure>`, the same wire shape),
    /// so its alert is worded by the cause — the chain out of reach by name,
    /// a fault inside the app as that — never mapped to a general one here.
    #[test]
    fn the_fee_failure_passes_through_as_it_is() {
        use vela_core::app::fee_policy::FeeFailure;
        for failure in crate::flows::FEE_FAILURES {
            let kind = SendEstimateFailure::from(failure);
            assert_eq!(
                serde_json::to_value(kind).ok(),
                serde_json::to_value(failure).ok(),
                "{failure:?} keeps its name"
            );
        }
        assert_eq!(
            SendEstimateFailure::from(FeeFailure::ChainRead {
                rate_limited: false
            })
            .body_key(),
            "send.alertEstimateChainDownBody"
        );
        assert_eq!(
            SendEstimateFailure::from(FeeFailure::Internal).body_key(),
            vela_core::app::fee_policy::REASON_INTERNAL_KEY
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
                sign_in_key: None,
                signing_domain: vela_core::signing_venue::APP_DOMAIN.to_owned(),
                signing_venue: vela_core::signing_venue::SigningVenue::InVela,
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
