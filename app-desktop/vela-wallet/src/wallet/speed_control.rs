//! The speed control of one fee surface — the send column, or the dApp
//! signing sheet (spec 069). One implementation, so the two surfaces cannot
//! drift: the design sheet says of the fee card that Send and signing "must
//! not drift", and the speed control is part of that card (the web's
//! `SpeedControl`, Android's `SpeedControl`, iOS's `FeeStore`).
//!
//! Every rule of it is the `fee_speed` core's: which tier is in force, the
//! free upgrade, the one-speed statement, which OTHER tiers must be kept
//! priced. What this module owns is the sessions — the one in force, and one
//! preview per tier the core names — and the reconcile step every shell runs:
//! when the session in force is not pricing the tier in force, PROMOTE the
//! preview that already priced this operation at that tier (the price tapped
//! is the price paid, #681), else re-price once nothing is measuring. Each
//! session carries a key, and an async answer is routed by it, so a result
//! that outlived a promotion lands on the session that asked or nowhere.
//!
//! ## Why free functions over a trait
//!
//! The sessions' answers come back through the OWNING entity's `Context` —
//! the send host's or the signing host's — so the pump has to be generic over
//! that entity. [`SpeedHost`] is the owner's side of it: where the control
//! lives, and the few things only the owner knows (whether its machine is
//! waiting on the quote in force, what a settled quote means to it).

use futures::StreamExt as _;
use gpui::Context;

use vela_core::app::fee_policy::{
    Event as FeeEvent, FeeCall, FeeOperation, FeePolicy, FeeShellResult, FeeTier, FeeView,
};
use vela_core::app::fee_speed::{
    Event as SpeedEvent, FeeSpeed, FeeSpeedView, TierPreviewQuote, TierQuote,
};
use vela_core::app::fee_tier_pref::FeeTierPref;

use crate::core_host::{CoreHost, Pending};
use crate::executor::{chain, fee_signals, format_prefs};
use crate::resident::{self, Answer, Machine};

/// The owner of a [`SpeedControl`]: an entity whose `Context` the sessions'
/// answers come back through.
pub trait SpeedHost: Sized + 'static {
    fn speed_control(&mut self) -> &mut SpeedControl;

    /// The session in force has a new view, or another session is in force.
    fn in_force_changed(&mut self, _cx: &mut Context<Self>) {}

    /// A machine is waiting on the question out on the session in force;
    /// nothing may re-price over it (it would hear its own question refused).
    fn answering(&self) -> bool {
        false
    }

    /// The deployment read for the question in force could not answer, so no
    /// quote was asked for.
    fn unreadable(&mut self, _cx: &mut Context<Self>) {}

    /// Any session moved — for an owner that polls while fees are busy.
    fn fees_moved(&mut self, _cx: &mut Context<Self>) {}
}

/// What a fee session was asked to price. Replayed at another tier for the
/// previews, and compared — minus the tier — to decide whether a preview
/// priced the same operation as the session in force.
#[derive(Clone, Debug, PartialEq, Eq)]
struct QuoteAsk {
    chain_id: u32,
    account: String,
    public_key_available: bool,
    tier: FeeTier,
    calls: Vec<FeeCall>,
    fee_token: Option<String>,
}

impl QuoteAsk {
    fn same_operation(&self, other: &Self) -> bool {
        Self {
            tier: other.tier,
            ..self.clone()
        } == *other
    }

    fn event(&self, deployed: bool) -> FeeEvent {
        FeeEvent::QuoteRequested {
            chain_id: self.chain_id,
            account: self.account.clone(),
            deployed,
            public_key_available: self.public_key_available,
            tier: self.tier,
            calls: self.calls.clone(),
            fee_token: self.fee_token.clone(),
        }
    }
}

/// One `fee_policy` session: the one in force, or a preview of another tier.
struct FeeSession {
    /// Unique for this control's life; every async answer carries it.
    key: u64,
    host: CoreHost<FeePolicy>,
    view: FeeView,
    ask: Option<QuoteAsk>,
    /// The deployment status the ask was dispatched with; `None` while it is
    /// being read, or when it could not be.
    deployed: Option<bool>,
    /// A deployment read is out before the dispatch — as much "measuring" as
    /// the core's own `busy`.
    reading: bool,
    generation: u64,
}

impl FeeSession {
    fn new(key: u64) -> Self {
        let host = CoreHost::<FeePolicy>::new();
        let view = host.view();
        Self {
            key,
            host,
            view,
            ask: None,
            deployed: None,
            reading: false,
            generation: 0,
        }
    }
}

/// The sessions and the speed core of one fee surface.
pub struct SpeedControl {
    /// The session in force: the quote the surface pre-checks against, the
    /// quote the fee row shows and the quote that is signed.
    fee: FeeSession,
    fee_view: FeeView,
    /// The other tiers' sessions, while the speed core wants them priced.
    previews: Vec<FeeSession>,
    next_session: u64,
    /// Bumped whenever the session in force is asked to price again (a new
    /// operation, a new tier, a refresh), so the previews follow it.
    generation: u64,
    /// Guards the deployment read: a slower one must not dispatch a quote for
    /// a question that has been superseded.
    fee_seq: u64,
    speed: CoreHost<FeeSpeed>,
    view: FeeSpeedView,
    last_on_form: Option<bool>,
    /// A speed pass is running; a nested one (an answer that arrived inline)
    /// leaves the final report to it.
    syncing: bool,
}

impl Default for SpeedControl {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeedControl {
    pub fn new() -> Self {
        let fee = FeeSession::new(0);
        let fee_view = fee.view.clone();
        let speed = CoreHost::<FeeSpeed>::new();
        let view = speed.view();
        Self {
            fee,
            fee_view,
            previews: Vec::new(),
            next_session: 1,
            generation: 0,
            fee_seq: 0,
            speed,
            view,
            last_on_form: None,
            syncing: false,
        }
    }

    /// The fee in force, as the fee row reads it.
    pub fn fee_view(&self) -> &FeeView {
        &self.fee_view
    }

    /// The speed control, as the core decided it.
    pub fn view(&self) -> &FeeSpeedView {
        &self.view
    }

    /// The tier this operation runs at.
    pub fn tier(&self) -> FeeTier {
        self.view.tier
    }

    /// The session pricing `tier` — for formatting that option's fee with its
    /// own fee-coin options.
    fn tier_view(&self, tier: FeeTier) -> Option<&FeeView> {
        if self.fee.ask.as_ref().is_some_and(|ask| ask.tier == tier) {
            return Some(&self.fee.view);
        }
        self.previews
            .iter()
            .find(|session| session.ask.as_ref().is_some_and(|ask| ask.tier == tier))
            .map(|session| &session.view)
    }

    /// Every (tier, view) the options can format a fee with.
    pub fn tier_views(&self) -> Vec<(FeeTier, FeeView)> {
        self.view
            .options
            .iter()
            .filter_map(|option| Some((option.tier, self.tier_view(option.tier)?.clone())))
            .collect()
    }

    /// No session has an effect out.
    pub fn idle(&self) -> bool {
        self.fee.host.is_idle() && self.previews.iter().all(|session| session.host.is_idle())
    }

    fn session_mut(&mut self, key: u64) -> Option<&mut FeeSession> {
        if self.fee.key == key {
            return Some(&mut self.fee);
        }
        self.previews.iter_mut().find(|session| session.key == key)
    }

    fn report_quotes(&mut self) {
        let previews = self
            .previews
            .iter()
            .filter_map(|session| {
                Some(TierPreviewQuote {
                    tier: session.ask.as_ref()?.tier,
                    busy: session.reading || session.view.busy,
                    fee: session.view.fee.clone(),
                })
            })
            .collect();
        let _ = self.speed.dispatch(SpeedEvent::QuotesChanged {
            chain_id: self.fee.ask.as_ref().map(|ask| ask.chain_id),
            in_force: TierQuote {
                busy: self.fee.reading || self.fee.view.busy,
                fee: self.fee.view.fee.clone(),
            },
            previews,
        });
        self.view = self.speed.view();
    }
}

// -- the sessions ------------------------------------------------------------

/// An event for the session in force (a fee-coin pick, a re-quote).
pub fn fee_dispatch<H: SpeedHost>(host: &mut H, event: FeeEvent, cx: &mut Context<H>) {
    let key = host.speed_control().fee.key;
    dispatch_to(host, key, event, cx);
}

fn dispatch_to<H: SpeedHost>(host: &mut H, key: u64, event: FeeEvent, cx: &mut Context<H>) {
    let Some(session) = host.speed_control().session_mut(key) else {
        return;
    };
    let pending = session.host.dispatch(event);
    pump(host, key, pending, cx);
}

/// An answer for the session that asked — found by its key, because it may
/// have been promoted, demoted or dropped since. Dropped: nobody is waiting,
/// and nothing else may hear it.
fn resolve<H: SpeedHost>(
    host: &mut H,
    key: u64,
    id: u64,
    result: FeeShellResult,
    cx: &mut Context<H>,
) {
    let Some(session) = host.speed_control().session_mut(key) else {
        return;
    };
    let pending = session.host.resolve(id, result);
    pump(host, key, pending, cx);
}

fn pump<H: SpeedHost>(
    host: &mut H,
    key: u64,
    pending: Vec<Pending<FeeOperation>>,
    cx: &mut Context<H>,
) {
    for effect in pending {
        let id = effect.id;
        match <FeePolicy as Machine>::perform(&effect.operation) {
            Answer::Now(result) => resolve(host, key, id, result, cx),
            Answer::Blocking(work) => {
                cx.spawn(async move |host, cx| {
                    let result = cx.background_executor().spawn(async move { work() }).await;
                    host.update(cx, |host, cx| resolve(host, key, id, result, cx))
                        .ok();
                })
                .detach();
            }
            Answer::After(delay, result) => {
                cx.spawn(async move |host, cx| {
                    cx.background_executor().timer(delay).await;
                    host.update(cx, |host, cx| resolve(host, key, id, result, cx))
                        .ok();
                })
                .detach();
            }
            // `fee_policy` streams nothing today. Implemented rather than left
            // as an `unreachable!`, because the day it does stream the
            // difference would be a panic on a confirm screen — and this is
            // the same eight lines the resident's own pump runs.
            Answer::Streaming(work) => {
                let (tx, mut rx) = futures::channel::mpsc::unbounded();
                cx.spawn(async move |host, cx| {
                    let work = cx
                        .background_executor()
                        .spawn(async move { work(&crate::resident::Sink::new(tx)) });
                    while let Some(event) = rx.next().await {
                        host.update(cx, |host, cx| dispatch_to(host, key, event, cx))
                            .ok();
                    }
                    let result = work.await;
                    host.update(cx, |host, cx| resolve(host, key, id, result, cx))
                        .ok();
                })
                .detach();
            }
        }
    }
    let control = host.speed_control();
    let Some(session) = control.session_mut(key) else {
        return;
    };
    session.view = session.host.view();
    if key == control.fee.key {
        in_force_changed(host, cx);
    }
    speed_pass(host, cx);
    host.fees_moved(cx);
    cx.notify();
}

/// The session in force moved: publish it, and — when it settled as a
/// failure — forget the readings behind it, so a retry measures again (issue
/// 212). Then the owner hears it.
fn in_force_changed<H: SpeedHost>(host: &mut H, cx: &mut Context<H>) {
    let control = host.speed_control();
    control.fee_view = control.fee.view.clone();
    if !control.fee_view.busy
        && control.fee_view.failed.is_some()
        && let Some(ask) = &control.fee.ask
    {
        fee_signals::invalidate(ask.chain_id);
    }
    host.in_force_changed(cx);
}

/// Price the operation at the tier in force: the deployment read first,
/// then the question. HOW FAST is the speed core's to say (spec 068) — the
/// stored default, a one-shot pick, or a free upgrade — so `tier` is not a
/// parameter.
pub fn ask<H: SpeedHost>(
    host: &mut H,
    chain_id: u32,
    account: String,
    public_key_available: bool,
    calls: Vec<FeeCall>,
    fee_token: Option<String>,
    cx: &mut Context<H>,
) {
    let tier = host.speed_control().tier();
    let ask = QuoteAsk {
        chain_id,
        account,
        public_key_available,
        tier,
        calls,
        fee_token,
    };
    ask_in_force(host, ask, cx);
}

/// Price `ask` on the session in force. The ask is recorded NOW, before the
/// deployment read, so a preview of the previous operation can never be
/// promoted over a question a machine is still waiting on.
fn ask_in_force<H: SpeedHost>(host: &mut H, ask: QuoteAsk, cx: &mut Context<H>) {
    let control = host.speed_control();
    control.fee_seq += 1;
    control.generation += 1;
    let seq = control.fee_seq;
    control.fee.generation = control.generation;
    control.fee.ask = Some(ask.clone());
    control.fee.deployed = None;
    control.fee.reading = true;
    speed_pass(host, cx);
    let account = ask.account.clone();
    let chain_id = ask.chain_id;
    cx.spawn(async move |host, cx| {
        let deployed = cx
            .background_executor()
            .spawn(async move { chain::is_deployed(&account, chain_id) })
            .await;
        host.update(cx, |host, cx| {
            let control = host.speed_control();
            if seq != control.fee_seq {
                return;
            }
            control.fee.reading = false;
            match deployed {
                // An indeterminate read never reaches the core: guessing
                // "deployed" ships an op without initCode, guessing
                // "undeployed" attaches one to a live account. Either way the
                // fee would be for a different operation than the one sent.
                Err(_) => {
                    host.unreadable(cx);
                    speed_pass(host, cx);
                    cx.notify();
                }
                Ok(deployed) => {
                    control.fee.deployed = Some(deployed);
                    let key = control.fee.key;
                    dispatch_to(host, key, ask.event(deployed), cx);
                }
            }
        })
        .ok();
    })
    .detach();
}

// -- the speed core ------------------------------------------------------------

fn speed_dispatch<H: SpeedHost>(host: &mut H, event: SpeedEvent, cx: &mut Context<H>) {
    let control = host.speed_control();
    // The machine asks the shell for nothing, so there is nothing to pump.
    let _ = control.speed.dispatch(event);
    control.view = control.speed.view();
    speed_pass(host, cx);
    cx.notify();
}

/// A new operation: the one-shot pick, a free upgrade and the fold die with
/// the one before it (spec 068), and it starts at the stored default.
pub fn reset<H: SpeedHost>(host: &mut H, cx: &mut Context<H>) {
    speed_dispatch(host, SpeedEvent::Reset, cx);
    configure(host, cx);
}

/// The stored default and the number preset, into the speed core. An
/// operation already open follows a new default until a speed is picked on it.
pub fn configure<H: SpeedHost>(host: &mut H, cx: &mut Context<H>) {
    let preferred = resident::resident::<FeeTierPref>(cx).read(cx).view().tier;
    let number = format_prefs::current().number;
    speed_dispatch(host, SpeedEvent::Configure { preferred, number }, cx);
}

/// Whether the person is still choosing: a free upgrade is only decided then,
/// never under somebody reading the last screen.
pub fn stage<H: SpeedHost>(host: &mut H, on_form: bool, cx: &mut Context<H>) {
    let control = host.speed_control();
    if control.last_on_form != Some(on_form) {
        control.last_on_form = Some(on_form);
        speed_dispatch(host, SpeedEvent::StageChanged { on_form }, cx);
    }
}

/// Fold or unfold the control.
pub fn toggle<H: SpeedHost>(host: &mut H, cx: &mut Context<H>) {
    speed_dispatch(host, SpeedEvent::Toggle, cx);
}

/// A tap on an option — one-shot: it prices and submits this operation and
/// never reaches the stored preference.
pub fn pick<H: SpeedHost>(host: &mut H, tier: FeeTier, cx: &mut Context<H>) {
    speed_dispatch(host, SpeedEvent::Pick { tier }, cx);
}

/// The refresh control: measure again. The held readings are dropped FIRST
/// (issue 212), so this is a new measurement and not the number already on
/// screen, and the other tiers are re-priced with it.
pub fn refresh<H: SpeedHost>(host: &mut H, cx: &mut Context<H>) {
    let control = host.speed_control();
    let Some(ask) = control.fee.ask.clone() else {
        return;
    };
    fee_signals::invalidate(ask.chain_id);
    if control.fee.reading {
        return;
    }
    if control.fee.deployed.is_none() {
        // The last attempt never reached the core, so `Requote` would be a
        // no-op and the control a dead button: ask again for real.
        ask_in_force(host, ask, cx);
        return;
    }
    control.generation += 1;
    control.fee.generation = control.generation;
    fee_dispatch(host, FeeEvent::Requote, cx);
}

// -- the reconcile step (`fee_speed.rs`) ----------------------------------------

/// Report every session to the speed core, then bring the sessions in line
/// with what it decided — until nothing moves.
fn speed_pass<H: SpeedHost>(host: &mut H, cx: &mut Context<H>) {
    if host.speed_control().syncing {
        return;
    }
    host.speed_control().syncing = true;
    for _ in 0..4 {
        host.speed_control().report_quotes();
        if !reconcile(host, cx) {
            break;
        }
    }
    sync_previews(host, cx);
    let control = host.speed_control();
    control.report_quotes();
    control.syncing = false;
}

/// Rule 1: make the session in force price the tier in force. Returns whether
/// the sessions moved.
fn reconcile<H: SpeedHost>(host: &mut H, cx: &mut Context<H>) -> bool {
    let answering = host.answering();
    let control = host.speed_control();
    let tier = control.view.tier;
    let Some(ask) = control.fee.ask.clone() else {
        return false;
    };
    if ask.tier == tier {
        return false;
    }
    if promote(host, tier, cx) {
        return true;
    }
    // Never over a measurement that is out: a machine may be waiting on it,
    // and would hear its own question refused.
    let control = host.speed_control();
    if control.fee.reading || control.fee.view.busy || answering {
        return false;
    }
    ask_in_force(host, QuoteAsk { tier, ..ask }, cx);
    false
}

/// THE PRICE YOU TAP IS THE PRICE YOU GET (issue 681): the preview that
/// already priced THIS operation at `tier` becomes the session in force, with
/// no second question to the relay — and the session it replaces becomes the
/// preview of its own tier, so nothing is re-measured.
///
/// Refused (so the caller measures for real) when that preview has no settled
/// quote of its own, or priced a different operation than the one the session
/// in force was last asked about.
fn promote<H: SpeedHost>(host: &mut H, tier: FeeTier, cx: &mut Context<H>) -> bool {
    let control = host.speed_control();
    let Some(in_force) = control.fee.ask.clone() else {
        return false;
    };
    let Some(index) = control.previews.iter().position(|session| {
        session
            .ask
            .as_ref()
            .is_some_and(|ask| ask.tier == tier && ask.same_operation(&in_force))
            && !session.reading
            && !session.view.busy
            && session
                .view
                .fee
                .as_ref()
                .is_some_and(|fee| fee.tier == tier)
    }) else {
        return false;
    };
    let promoted = control.previews.remove(index);
    let mut demoted = std::mem::replace(&mut control.fee, promoted);
    // A deployment read still out for the demoted session must not dispatch
    // onto the one now in force.
    control.fee_seq += 1;
    demoted.reading = false;
    if demoted.ask.is_some() && demoted.deployed.is_some() {
        control.previews.push(demoted);
    }
    in_force_changed(host, cx);
    true
}

/// Rule 2: a preview for every tier the core wants priced, pricing the same
/// operation as the session in force at the same generation; every other
/// preview dropped. Nothing is created while the session in force is still
/// reading its deployment status — its question is not settled enough to
/// replay.
fn sync_previews<H: SpeedHost>(host: &mut H, cx: &mut Context<H>) {
    let control = host.speed_control();
    let wanted = control.view.previews.clone();
    let base = match (&control.fee.ask, control.fee.deployed) {
        (Some(ask), Some(deployed)) if !control.fee.reading => Some((ask.clone(), deployed)),
        _ => None,
    };
    let generation = control.generation;
    control.previews.retain(|session| {
        let (Some(ask), Some((base, _))) = (&session.ask, &base) else {
            return false;
        };
        wanted.contains(&ask.tier) && ask.same_operation(base) && session.generation == generation
    });
    let Some((base, deployed)) = base else {
        return;
    };
    let missing: Vec<FeeTier> = wanted
        .iter()
        .copied()
        .filter(|tier| {
            !control
                .previews
                .iter()
                .any(|session| session.ask.as_ref().is_some_and(|ask| ask.tier == *tier))
        })
        .collect();
    let mut started = Vec::new();
    for tier in missing {
        let key = control.next_session;
        control.next_session += 1;
        let mut session = FeeSession::new(key);
        let ask = QuoteAsk {
            tier,
            ..base.clone()
        };
        session.ask = Some(ask.clone());
        session.deployed = Some(deployed);
        session.generation = generation;
        control.previews.push(session);
        started.push((key, ask));
    }
    // Installed first, asked second: an answer that arrives inline finds every
    // session already in place.
    for (key, ask) in started {
        dispatch_to(host, key, ask.event(deployed), cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ask(tier: FeeTier, amount: &str) -> QuoteAsk {
        QuoteAsk {
            chain_id: 100,
            account: "0xabc".to_owned(),
            public_key_available: true,
            tier,
            calls: vec![FeeCall {
                to: "0xdef".to_owned(),
                value: amount.to_owned(),
                data: "0x".to_owned(),
            }],
            fee_token: None,
        }
    }

    /// A preview is promotable only when it priced THE SAME operation — the
    /// tier aside. A preview of yesterday's amount must never answer a
    /// question about today's (issue 681's guard).
    #[test]
    fn the_same_operation_is_everything_but_the_tier() {
        assert!(ask(FeeTier::Fast, "1").same_operation(&ask(FeeTier::Slow, "1")));
        assert!(!ask(FeeTier::Fast, "1").same_operation(&ask(FeeTier::Fast, "2")));
        let mut other_coin = ask(FeeTier::Slow, "1");
        other_coin.fee_token = Some("0xusdc".to_owned());
        assert!(!ask(FeeTier::Fast, "1").same_operation(&other_coin));
    }

    /// The question a preview asks is the one in force, at its own tier.
    #[test]
    fn a_preview_asks_the_same_question_at_its_own_tier() {
        let FeeEvent::QuoteRequested { tier, deployed, .. } =
            ask(FeeTier::Standard, "1").event(true)
        else {
            unreachable!("a quote request");
        };
        assert_eq!(tier, FeeTier::Standard);
        assert!(deployed);
    }
}
