//! The half of the architecture that **performs**.
//!
//! `vela-core` decides and asks; the shell does. On desktop there is no bridge
//! and no JSON in between: `Core<A>` runs in this process, so an operation
//! arrives as a Rust enum and an answer goes back as one. What is preserved
//! from the wasm bridge is not the encoding but the CORRELATION RULES, because
//! those are where a driver gets subtly wrong:
//!
//! 1. Effect ids are monotonic, per core instance.
//! 2. An id this host does not know means **the answer outlived the question** —
//!    an operation that was already abandoned. Expected, not a fault: report
//!    nothing and change nothing.
//! 3. A `resolve` error means the command that owned the request was aborted
//!    before the answer arrived. Same story, one layer down.
//!
//! None of the three is a bug, none should reach a user, and all three are why
//! this file exists once and is generic over the machine rather than being
//! written per screen.
//!
//! ```text
//!   gpui entity                CoreHost<A>              executor
//!       │ dispatch(event) ────────►│
//!       │◄──────── Vec<Pending> ───│
//!       │ perform(operation) ──────────────────────────────►│  (background)
//!       │◄──────────────────────────────── ShellResult ─────│
//!       │ resolve(id, result) ────►│
//!       │◄──────── Vec<Pending> ───│   …until the queue drains
//! ```

use std::collections::HashMap;

use crux_core::capability::Operation;
use crux_core::{App, Core, Request};

use vela_core::app::SplitEffect;

/// One operation the core is waiting on, and the id that will answer it.
pub struct Pending<Op> {
    pub id: u64,
    pub operation: Op,
}

/// Drives one Crux machine in-process.
///
/// Generic over the machine rather than over a concrete `Effect`, because the
/// onboarding machines and the session machine speak two different operation
/// vocabularies ([`vela_core::app::shell::ShellOperation`] and
/// [`vela_core::app::session::SessionOperation`]) and there is no reason for
/// the driver to know which one it is holding.
pub struct CoreHost<A>
where
    A: App + Default,
    A::Model: Default,
    A::Effect: SplitEffect,
{
    core: Core<A>,
    pending: HashMap<u64, Request<<A::Effect as SplitEffect>::Op>>,
    next_id: u64,
}

impl<A> CoreHost<A>
where
    A: App + Default,
    A::Model: Default,
    A::Effect: SplitEffect,
    <A::Effect as SplitEffect>::Op: Clone,
{
    pub fn new() -> Self {
        Self {
            core: Core::new(),
            pending: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn view(&self) -> A::ViewModel {
        let _span = crate::perf::span("view", crate::perf::short_type_name::<A>());
        self.core.view()
    }

    /// Send an event; returns what the core now needs performed.
    pub fn dispatch(&mut self, event: A::Event) -> Vec<Pending<<A::Effect as SplitEffect>::Op>> {
        // Phase 4: the core's own time per event, per machine (`VELA_PERF`).
        let _span = crate::perf::span("dispatch", crate::perf::short_type_name::<A>());
        let effects = self.core.process_event(event);
        self.collect(effects)
    }

    /// Answer an operation by id.
    pub fn resolve(
        &mut self,
        id: u64,
        result: <<A::Effect as SplitEffect>::Op as Operation>::Output,
    ) -> Vec<Pending<<A::Effect as SplitEffect>::Op>> {
        let Some(mut request) = self.pending.remove(&id) else {
            // Rule 2. A late answer to a question nobody is still asking.
            return Vec::new();
        };
        let _span = crate::perf::span("resolve", crate::perf::short_type_name::<A>());
        match self.core.resolve(&mut request, result) {
            Ok(effects) => self.collect(effects),
            // Rule 3.
            Err(_) => Vec::new(),
        }
    }

    /// Whether anything is still outstanding. Used by the flow screens to
    /// decide whether a pump has drained, not to decide anything the core owns.
    pub fn is_idle(&self) -> bool {
        self.pending.is_empty()
    }

    fn collect(&mut self, effects: Vec<A::Effect>) -> Vec<Pending<<A::Effect as SplitEffect>::Op>> {
        let mut out = Vec::new();
        for effect in effects {
            // `Effect::Render` falls out here as `None`. The caller re-renders
            // unconditionally after a pump, so a render needs no separate
            // handling — but it must still be SPLIT OFF, or it would be queued
            // as an operation no executor can perform.
            if let Some(request) = effect.into_shell() {
                self.next_id += 1;
                let id = self.next_id;
                out.push(Pending {
                    id,
                    operation: request.operation.clone(),
                });
                self.pending.insert(id, request);
            }
        }
        out
    }
}

impl<A> Default for CoreHost<A>
where
    A: App + Default,
    A::Model: Default,
    A::Effect: SplitEffect,
    <A::Effect as SplitEffect>::Op: Clone,
{
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Why there is no `cancelled_effect_ids` here
// ---------------------------------------------------------------------------
//
// The wasm bridge carries that field and it is always empty, for a reason that
// is a property of the MACHINES rather than of the transport: each pipeline
// keeps at most one operation in flight and stamps every request with the
// attempt that asked for it, so a superseded answer is dropped by the core on
// arrival instead of being aborted by the shell. A desktop executor that
// implemented cancellation would therefore be implementing a path the core
// never takes — and would need a second, different reason to be correct.
//
// The one operation that genuinely takes unbounded time is a passkey ceremony
// waiting for a finger on a security key. That is bounded by the CTAPHID
// timeout in `ctap::usb`, which answers with a failure result rather than
// leaving the core waiting — the same shape every other failure takes.
