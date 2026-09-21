//! One Crux machine, alive for as long as the process, driven from gpui.
//!
//! [`crate::core_host::CoreHost`] already knows how to drive any machine; what it
//! cannot know is where the answers come from or when the screen redraws. This
//! module is that half, written **once and generic over the machine**, because
//! seventeen copies of an effect loop is seventeen chances to get the
//! correlation rules subtly wrong — which is the reason `core_host.rs` exists
//! once, in its own words.
//!
//! ## Why this is an entity and `session.rs` is a `Global`
//!
//! [`crate::session`] drives its machine synchronously inside a `Global`, and
//! its module doc argues the case: every session operation is one small local
//! file read, so moving it off-thread "would buy nothing measurable and would
//! introduce the one thing this module must not have — a window in which the
//! route is stale."
//!
//! The machines here are **both cases at once**. `read_store` is a local file
//! read; `probe_rpc` is a TLS round trip with a multi-second budget;
//! `start_search_debounce` is a timer armed on every keystroke. A synchronous
//! pump would freeze the window for the length of a network probe.
//!
//! And a `Global` *cannot* be the answer, for a mechanical reason rather than a
//! stylistic one: a background task has no handle through which to reach one.
//! `Entity::update` is what a spawned task calls when its answer arrives, so the
//! thing holding the core has to be an entity. That is precisely why
//! `onboarding.rs` is an entity and `session.rs` is not, and this module follows
//! the same fork for the same reason.
//!
//! ## The thread boundary is a type, not a convention
//!
//! [`Answer`] makes it structural. `Answer::Blocking` carries a boxed `FnOnce`
//! that is `Send`; `Core<A>` is not `Send` and so physically cannot be captured
//! by one. "The core never leaves the main thread" is therefore checked by the
//! compiler rather than promised in a comment.
//!
//! ```text
//!   gpui entity                 ResidentCore<A>              background executor
//!       │ dispatch(event) ───────────►│
//!       │        Answer::Now  ────────┤ resolved inline, loop until quiescent
//!       │        Answer::Blocking ────┼───────────────────────────►│
//!       │        Answer::After  ──────┼──► timer ──────────────────┤
//!       │◄────── entity.update(resolve(id, result)) ───────────────┘
//!       │ view = host.view(); cx.notify()  → observers redraw
//! ```

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::time::Duration;

use crux_core::App as CruxApp;
use futures::StreamExt as _;
use gpui::{App, AppContext as _, Context, Entity, Global};

use vela_core::app::SplitEffect;

use crate::core_host::CoreHost;

type Op<A> = <<A as CruxApp>::Effect as SplitEffect>::Op;
type Out<A> = <Op<A> as crux_core::capability::Operation>::Output;

/// A sink for events a long operation reports on its way.
///
/// Cloneable and callable from any thread — the balance fetch calls it from
/// twelve of them. Each call becomes an event dispatched into this machine on
/// the MAIN thread, in the order the sink received them, and all of them
/// before the operation's own result. That ordering is the whole contract:
/// `balance_dashboard` merges each arrival into its token list and settles
/// once, so a snapshot landing after the settle would resurrect a chain the
/// settle had already accounted for.
pub struct Sink<E>(futures::channel::mpsc::UnboundedSender<E>);

impl<E> Clone for Sink<E> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<E> Sink<E> {
    /// Wrap a sender. Public so a screen-owned pump (`wallet::money`) can run
    /// the same streaming path the resident does rather than a second one.
    pub fn new(tx: futures::channel::mpsc::UnboundedSender<E>) -> Self {
        Self(tx)
    }

    /// Report one event. Silently does nothing once the receiver is gone —
    /// the window closed mid-fetch, which is not a fault.
    pub fn send(&self, event: E) {
        let _ = self.0.unbounded_send(event);
    }
}

/// How an operation is performed, and therefore where.
pub enum Answer<T, E> {
    /// A local read or write. Resolved on this thread, this frame — the
    /// `session.rs` argument, kept for the operations it is actually true of.
    Now(T),
    /// Blocks: a socket, TLS, a disk the OS decides to think about. Runs on
    /// gpui's background executor.
    ///
    /// The closure is the thread boundary. It can only capture what the
    /// *operation* owns, and `Core<A>` is not `Send`, so the core cannot cross.
    Blocking(Box<dyn FnOnce() -> T + Send>),
    /// Answered after a delay — a debounce, a backoff. Uses gpui's timer rather
    /// than a parked thread: `ShellOperation::Wait` sleeps a background thread
    /// and that is right for a flow that waits a handful of times, but a search
    /// debounce fires on every keystroke and must not cost a thread each.
    After(Duration, T),
    /// Blocks, and says what it has found before it is done.
    ///
    /// The same thread boundary as [`Answer::Blocking`] — the closure is
    /// `Send` and the core is not, so it still cannot cross — plus a [`Sink`]
    /// the work calls as partial results land. Twelve chains answer at twelve
    /// different speeds, and a hero that shows nothing until the slowest one
    /// replies is a hero gated by the worst network on the list.
    Streaming(Box<dyn FnOnce(&Sink<E>) -> T + Send>),
}

/// Run a streaming answer without an async pump, reports first.
///
/// For the synchronous drivers in tests. It keeps the one ordering guarantee
/// that matters — every report is in hand before the result is used — by
/// draining after the work has returned and dropped its sink. What it does not
/// reproduce is the concurrency, which is the point of the real path and not
/// of a test.
#[cfg(test)]
pub fn run_streaming<T, E>(work: Box<dyn FnOnce(&Sink<E>) -> T + Send>) -> (Vec<E>, T) {
    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    let result = work(&Sink(tx));
    let mut reports = Vec::new();
    while let Ok(event) = rx.try_recv() {
        reports.push(event);
    }
    (reports, result)
}

/// What varies per machine — and deliberately nothing else.
pub trait Machine: CruxApp + Default + 'static
where
    Self::Model: Default,
    Self::Effect: SplitEffect,
{
    /// Named in the one log line a core fault produces.
    const LABEL: &'static str;

    /// The event that starts this machine.
    ///
    /// Takes `&App` because some machines must be told *whose* data they are
    /// about at boot: `contacts` needs the signed-in address, and the core's own
    /// comment warns that missing it "crosses the books" between accounts.
    fn boot_event(cx: &App) -> Self::Event;

    /// Perform one operation. Never fails outward — see `executor::mod`'s
    /// failure contract, which this inherits wholesale.
    fn perform(operation: &Op<Self>) -> Answer<Out<Self>, Self::Event>;
}

/// One machine and its latest view.
pub struct ResidentCore<A: Machine>
where
    A::Model: Default,
    A::Effect: SplitEffect,
{
    host: CoreHost<A>,
    view: A::ViewModel,
}

impl<A> ResidentCore<A>
where
    A: Machine,
    A::Model: Default,
    A::Effect: SplitEffect,
    Op<A>: Clone + 'static,
    Out<A>: 'static,
    A::ViewModel: Clone,
{
    fn new() -> Self {
        let host = CoreHost::<A>::new();
        let view = host.view();
        Self { host, view }
    }

    /// The current view. Cheap to clone; screens read it per frame.
    pub fn view(&self) -> A::ViewModel {
        self.view.clone()
    }

    /// Send an event and perform whatever it asks for.
    pub fn dispatch(&mut self, event: A::Event, cx: &mut Context<Self>) {
        let pending = self.host.dispatch(event);
        self.pump(pending, cx);
    }

    fn resolve(&mut self, id: u64, result: Out<A>, cx: &mut Context<Self>) {
        let pending = self.host.resolve(id, result);
        self.pump(pending, cx);
    }

    fn pump(&mut self, pending: Vec<crate::core_host::Pending<Op<A>>>, cx: &mut Context<Self>) {
        for next in pending {
            let id = next.id;
            match A::perform(&next.operation) {
                // Straight back into the core. Recursion depth is the machine's
                // own chain of local operations, which is short by construction.
                Answer::Now(result) => self.resolve(id, result, cx),
                Answer::Blocking(work) => {
                    cx.spawn(async move |resident, cx| {
                        // Spec 038: a panic in the work is survived here. The
                        // operation is then left unresolved — the machine keeps
                        // its last state — and the hook's report becomes the
                        // page's failure sheet.
                        let result = cx
                            .background_executor()
                            .spawn(async move {
                                if crate::panic_report::test_panic_requested() {
                                    panic!(
                                        "VELA_TEST_PANIC: a deliberate panic in background work"
                                    );
                                }
                                crate::panic_report::guarded(work)
                            })
                            .await;
                        if let Some(result) = result {
                            resident
                                .update(cx, |resident, cx| resident.resolve(id, result, cx))
                                .ok();
                        } else {
                            resident.update(cx, |_, cx| cx.notify()).ok();
                        }
                    })
                    .detach();
                }
                Answer::After(delay, result) => {
                    cx.spawn(async move |resident, cx| {
                        cx.background_executor().timer(delay).await;
                        resident
                            .update(cx, |resident, cx| resident.resolve(id, result, cx))
                            .ok();
                    })
                    .detach();
                }
                Answer::Streaming(work) => {
                    let (tx, mut rx) = futures::channel::mpsc::unbounded();
                    cx.spawn(async move |resident, cx| {
                        let work = cx
                            .background_executor()
                            .spawn(async move { crate::panic_report::guarded(|| work(&Sink(tx))) });
                        // Drains until every sender is gone, which happens when
                        // `work` returns and drops the sink it was handed. So
                        // this loop cannot outlive the operation, and cannot
                        // end while a snapshot is still queued — the ordering
                        // `Sink` promises.
                        while let Some(event) = rx.next().await {
                            resident
                                .update(cx, |resident, cx| resident.dispatch(event, cx))
                                .ok();
                        }
                        match work.await {
                            Some(result) => {
                                resident
                                    .update(cx, |resident, cx| resident.resolve(id, result, cx))
                                    .ok();
                            }
                            // Survived a panic: the report is in the mailbox,
                            // the next render raises it (spec 038).
                            None => {
                                resident.update(cx, |_, cx| cx.notify()).ok();
                            }
                        }
                    })
                    .detach();
                }
            }
        }
        self.view = self.host.view();
        cx.notify();
    }
}

/// Every resident machine in this process, keyed by its own type.
///
/// A `HashMap<TypeId, …>` rather than a struct with a field per machine, and
/// that is load-bearing rather than tidy: named fields would mean wiring the
/// third machine necessarily edits this file, and "wiring the third machine
/// touches no shared plumbing" is the measurement this cut exists to produce
/// (spec 030 SC-004). Adding a machine must cost zero lines here.
#[derive(Default)]
struct Residents(HashMap<TypeId, Box<dyn Any>>);

impl Global for Residents {}

/// The resident for `A`, booting it on first use.
pub fn resident<A>(cx: &mut App) -> Entity<ResidentCore<A>>
where
    A: Machine,
    A::Model: Default,
    A::Effect: SplitEffect,
    Op<A>: Clone + 'static,
    Out<A>: 'static,
    A::ViewModel: Clone,
{
    if let Some(existing) = cx
        .try_global::<Residents>()
        .and_then(|residents| residents.0.get(&TypeId::of::<A>()))
        .and_then(|any| any.downcast_ref::<Entity<ResidentCore<A>>>())
    {
        return existing.clone();
    }

    // Read what the boot event needs BEFORE taking the entity's borrow.
    let event = A::boot_event(cx);
    eprintln!("[vela-wallet] core: {} booting", A::LABEL);
    let entity = cx.new(|_| ResidentCore::<A>::new());
    entity.update(cx, |resident, cx| resident.dispatch(event, cx));

    if !cx.has_global::<Residents>() {
        cx.set_global(Residents::default());
    }
    cx.global_mut::<Residents>()
        .0
        .insert(TypeId::of::<A>(), Box::new(entity.clone()));
    entity
}

/// Forget one machine: its store was cleared under it (spec 072), and the
/// next use boots it from what is on disk now — rather than a machine that
/// still holds the cleared list writing it back.
pub fn forget<A: 'static>(cx: &mut App) {
    if cx.has_global::<Residents>() {
        cx.global_mut::<Residents>().0.remove(&TypeId::of::<A>());
    }
}

/// Forget every machine.
///
/// Called on sign-out. Contacts, networks and the chosen currency belong to the
/// ACCOUNT, and a resident that outlived a sign-out would show the previous
/// person's address book to the next one.
pub fn drop_all(cx: &mut App) {
    if cx.has_global::<Residents>() {
        cx.global_mut::<Residents>().0.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ordering `Answer::Streaming` promises, without gpui in the way.
    ///
    /// This is the same shape the pump runs: the work goes to another thread
    /// holding the sink, and the drain ends only when that sink is dropped —
    /// which is when the work returns. So the settle can never overtake a
    /// snapshot, which for `balance_dashboard` would resurrect a chain the
    /// settle had already accounted for.
    #[test]
    fn every_streamed_event_lands_before_the_result() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let work: Box<dyn FnOnce(&Sink<u32>) -> &'static str + Send> = Box::new(|sink| {
            for chain in 0..12 {
                sink.send(chain);
            }
            "settled"
        });

        let (seen, result) = futures::executor::block_on(async move {
            let work = std::thread::spawn(move || work(&Sink(tx)));
            let mut seen = Vec::new();
            while let Some(event) = rx.next().await {
                seen.push(event);
            }
            let result = work
                .join()
                .unwrap_or_else(|_| unreachable!("the work panicked"));
            (seen, result)
        });

        assert_eq!(seen, (0..12).collect::<Vec<_>>(), "in order, none dropped");
        assert_eq!(result, "settled");
    }

    /// The window closed mid-fetch. Eleven chains still have a sink to call
    /// and calling it must not take the process down with them.
    #[test]
    fn a_sink_whose_receiver_is_gone_swallows_the_report() {
        let (tx, rx) = futures::channel::mpsc::unbounded::<u32>();
        drop(rx);
        let sink = Sink(tx);
        sink.send(1);
        sink.clone().send(2);
    }
}
