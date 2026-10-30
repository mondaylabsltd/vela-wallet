//! The generic JSON bridge every Crux core in this crate is exported through.
//!
//! Extracted verbatim from the 011 onboarding bridge so its semantics — the
//! monotonic effect id, "an unknown id means the answer outlived the
//! question", "an aborted command's late result is expected, not a fault" —
//! exist exactly once. A new machine (spec 016 and later) is one
//! [`bridge_class!`] line plus a 3-line [`SplitEffect`] impl in `vela-core`;
//! nothing here is copied per app, so nothing here can fork per app.
//!
//! ```text
//! dispatch(event_json) ─► { view, effects: [{ id, operation }], cancelled_effect_ids }
//! resolve_effect(id, result_json) ─► same shape
//! skip_unchanged_views(include_volatile) ─► later answers: { view?, view_version, volatile?, … }
//! ```
//!
//! `skip_unchanged_views` is opt-in per shell: until a shell calls it, every
//! answer carries the whole view, as it always has
//! (`vela_core::app::view_host`).

use std::collections::HashMap;

use crux_core::capability::Operation;
use crux_core::{App, Core, Request};
use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::JsValue;

use vela_core::app::view_host::ViewHost;
use vela_core::app::SplitEffect;

/// One answer. A shell that never called `skip_unchanged_views` gets exactly
/// `{ view, effects, cancelled_effect_ids }`, the whole view every time; one
/// that did gets `view_version` always and `view` only when it changed
/// (`vela_core::app::view_host`), plus `volatile` if it asked for it.
#[derive(Serialize)]
struct DispatchResult<'a, Op> {
    #[serde(skip_serializing_if = "Option::is_none")]
    view: Option<&'a serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    view_version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    volatile: Option<&'a serde_json::Value>,
    effects: Vec<ShellEffect<Op>>,
    /// Always empty today: every machine keeps at most one in-flight
    /// operation per pipeline and drops stale answers by attempt, so the
    /// shell is never asked to abort. Kept in the payload because the shared
    /// effect loop reads it.
    cancelled_effect_ids: Vec<u64>,
}

#[derive(Serialize)]
struct ShellEffect<Op> {
    id: u64,
    operation: Op,
}

/// The generic half. `#[wasm_bindgen]` cannot export generics, so each
/// exported class is a thin [`bridge_class!`]-generated wrapper over this.
pub(crate) struct Bridge<A>
where
    A: App + Default,
    A::Model: Default,
    A::Effect: SplitEffect,
{
    core: Core<A>,
    pending: HashMap<u64, Request<<A::Effect as SplitEffect>::Op>>,
    next_effect_id: u64,
    host: ViewHost,
}

impl<A> Bridge<A>
where
    A: App + Default,
    A::Model: Default,
    A::Event: DeserializeOwned,
    A::ViewModel: Serialize,
    A::Effect: SplitEffect,
    <A::Effect as SplitEffect>::Op: Clone + Serialize,
    <<A::Effect as SplitEffect>::Op as Operation>::Output: DeserializeOwned,
{
    pub(crate) fn new() -> Self {
        Self {
            core: Core::new(),
            pending: HashMap::new(),
            next_effect_id: 0,
            host: ViewHost::new(
                <A::Effect as SplitEffect>::RENDERS_EVERY_CHANGE,
                <A::Effect as SplitEffect>::VOLATILE_VIEW_KEYS,
            ),
        }
    }

    /// The shell opts in to answers without an unchanged view
    /// (`vela_core::app::view_host`).
    pub(crate) fn skip_unchanged_views(&mut self, include_volatile: bool) {
        self.host.skip_unchanged_views(include_volatile);
    }

    pub(crate) fn dispatch(&mut self, event_json: &str) -> Result<String, JsValue> {
        let event: A::Event = serde_json::from_str(event_json)
            .map_err(|error| JsValue::from_str(&format!("invalid event from shell: {error}")))?;
        let effects = self.core.process_event(event);
        self.serialize(effects)
    }

    pub(crate) fn resolve_effect(
        &mut self,
        effect_id: u64,
        result_json: &str,
    ) -> Result<String, JsValue> {
        let result: <<A::Effect as SplitEffect>::Op as Operation>::Output =
            serde_json::from_str(result_json).map_err(|error| {
                JsValue::from_str(&format!("invalid result from shell: {error}"))
            })?;

        // An unknown id means the answer outlived the question — the shell
        // resolved an operation that was already abandoned. Expected, not a
        // fault: report the current view and change nothing.
        let Some(mut request) = self.pending.remove(&effect_id) else {
            return self.serialize(Vec::new());
        };

        match self.core.resolve(&mut request, result) {
            Ok(effects) => self.serialize(effects),
            // Same story one layer down: the command that owned this request
            // was aborted before the answer arrived.
            Err(_) => self.serialize(Vec::new()),
        }
    }

    /// Through `serde_json::Value`, exactly as [`Self::serialize`] writes the
    /// view inside every dispatch result — so both report the same JSON (keys
    /// in `Value`'s sorted order) and each `ViewModel` is serialized by ONE
    /// serializer, not two. Writing the view straight to a string here gave
    /// every one of the ~33 machines a second, string-serializer copy of its
    /// view code: 108,563 wasm bytes (25,838 brotli'd) for the same JSON
    /// value. The cost is an intermediate `Value` per call — ~13 µs on the
    /// Send machine's view — on the call the effect loop makes once when it
    /// starts a machine; every later view arrives inside a dispatch result.
    pub(crate) fn view(&self) -> Result<String, JsValue> {
        serde_json::to_value(self.core.view())
            .and_then(|view| serde_json::to_string(&view))
            .map_err(|error| JsValue::from_str(&format!("could not serialize view: {error}")))
    }

    fn serialize(&mut self, effects: Vec<A::Effect>) -> Result<String, JsValue> {
        let mut shell_effects = Vec::new();
        let mut rendered = false;
        for effect in effects {
            // A render falls out as `None`: what tells the host the view may
            // have moved.
            if let Some(request) = effect.into_shell() {
                self.next_effect_id += 1;
                let id = self.next_effect_id;
                let operation = request.operation.clone();
                self.pending.insert(id, request);
                shell_effects.push(ShellEffect { id, operation });
            } else {
                rendered = true;
            }
        }

        if self.host.needs_view(rendered) {
            let view = serde_json::to_value(self.core.view()).map_err(|error| {
                JsValue::from_str(&format!("could not serialize view: {error}"))
            })?;
            self.host.store(view);
        }
        let answer = self.host.answer();
        serde_json::to_string(&DispatchResult {
            view: answer.view,
            view_version: answer.view_version,
            volatile: answer.volatile,
            effects: shell_effects,
            cancelled_effect_ids: Vec::new(),
        })
        .map_err(|error| JsValue::from_str(&format!("could not serialize result: {error}")))
    }
}

/// Export one Crux machine as a wasm class with the canonical
/// `dispatch` / `resolve_effect` / `view` surface.
macro_rules! bridge_class {
    ($(#[$doc:meta])* $class:ident, $app:ty) => {
        $(#[$doc])*
        #[wasm_bindgen::prelude::wasm_bindgen]
        pub struct $class(crate::bridge::Bridge<$app>);

        #[wasm_bindgen::prelude::wasm_bindgen]
        impl $class {
            #[wasm_bindgen(constructor)]
            pub fn new() -> Self {
                Self(crate::bridge::Bridge::new())
            }

            pub fn dispatch(
                &mut self,
                event_json: &str,
            ) -> Result<String, wasm_bindgen::JsValue> {
                self.0.dispatch(event_json)
            }

            pub fn resolve_effect(
                &mut self,
                effect_id: u64,
                result_json: &str,
            ) -> Result<String, wasm_bindgen::JsValue> {
                self.0.resolve_effect(effect_id, result_json)
            }

            pub fn view(&self) -> Result<String, wasm_bindgen::JsValue> {
                self.0.view()
            }

            /// Opt in to lighter answers: from the next one on, each carries
            /// `view_version`, and `view` only when it differs from the last
            /// view this object sent (no `view`: keep the one you hold).
            /// `include_volatile`: also send the machine's volatile fields
            /// apart, in `volatile`, when they move.
            pub fn skip_unchanged_views(&mut self, include_volatile: bool) {
                self.0.skip_unchanged_views(include_volatile);
            }
        }

        impl Default for $class {
            fn default() -> Self {
                Self::new()
            }
        }
    };
}

pub(crate) use bridge_class;
