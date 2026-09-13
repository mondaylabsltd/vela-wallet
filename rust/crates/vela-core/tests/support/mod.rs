//! Test driver for the onboarding machines.
//!
//! Drives a `Core` exactly the way the wasm bridge does — dispatch an event,
//! collect the shell operations it asked for, resolve them one at a time — so
//! the tests exercise the real correlation plumbing rather than a simulation of
//! it. No browser, no authenticator, no network, no clock.
//!
//! Fixtures are the **real** conformance vectors: a genuine attestation object
//! and a genuine pair of assertions from the same credential, so key
//! extraction, address derivation and two-signature recovery all run for real.

#![cfg(feature = "crux")]
#![allow(dead_code)] // each test file uses a different subset

use std::collections::VecDeque;

use crux_core::{App, Core, Request};
use serde_json::Value;

use vela_core::app::shell::{Effect, ShellOperation, ShellResult};
use vela_core::app::{Account, Assertion, Registration};

// ---------------------------------------------------------------------------
// Driver
// ---------------------------------------------------------------------------

pub struct Driver<A>
where
    A: App<Effect = Effect> + Default,
    A::Model: Default,
{
    app: Core<A>,
    pending: VecDeque<Request<ShellOperation>>,
}

impl<A> Driver<A>
where
    A: App<Effect = Effect> + Default,
    A::Model: Default,
{
    pub fn new() -> Self {
        Self {
            app: Core::new(),
            pending: VecDeque::new(),
        }
    }

    /// Send an event; returns the operations the core asked for, in order.
    pub fn dispatch(&mut self, event: A::Event) -> Vec<ShellOperation> {
        let effects = self.app.process_event(event);
        self.collect(effects)
    }

    /// Answer the oldest outstanding operation.
    ///
    /// Panics if nothing is outstanding — a test that resolves into thin air is
    /// asserting something that never happened.
    pub fn resolve(&mut self, result: ShellResult) -> Vec<ShellOperation> {
        let mut request = self
            .pending
            .pop_front()
            .expect("no outstanding shell operation to resolve");
        match self.app.resolve(&mut request, result) {
            Ok(effects) => self.collect(effects),
            // An aborted command's late result: expected, and a no-op.
            Err(_) => Vec::new(),
        }
    }

    /// Answer the oldest outstanding operation matching `predicate`, leaving the
    /// others outstanding. Used where two effects are legitimately in flight
    /// (the background index heal running alongside completion).
    pub fn resolve_matching(
        &mut self,
        predicate: impl Fn(&ShellOperation) -> bool,
        result: ShellResult,
    ) -> Vec<ShellOperation> {
        let index = self
            .pending
            .iter()
            .position(|request| predicate(&request.operation))
            .expect("no outstanding shell operation matches");
        let mut request = self
            .pending
            .remove(index)
            .expect("index came from position()");
        match self.app.resolve(&mut request, result) {
            Ok(effects) => self.collect(effects),
            Err(_) => Vec::new(),
        }
    }

    pub fn view(&self) -> A::ViewModel {
        self.app.view()
    }

    /// Operations still awaiting an answer.
    pub fn outstanding(&self) -> Vec<ShellOperation> {
        self.pending
            .iter()
            .map(|request| request.operation.clone())
            .collect()
    }

    fn collect(&mut self, effects: Vec<Effect>) -> Vec<ShellOperation> {
        let mut operations = Vec::new();
        for effect in effects {
            match effect {
                Effect::Render(_) => {}
                Effect::Shell(request) => {
                    operations.push(request.operation.clone());
                    self.pending.push_back(request);
                }
            }
        }
        operations
    }
}

impl<A> Default for Driver<A>
where
    A: App<Effect = Effect> + Default,
    A::Model: Default,
{
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Fixtures — real bytes from the conformance corpus
// ---------------------------------------------------------------------------

const VECTORS: &str = include_str!("../vectors/webauthn.json");

fn case(name: &str) -> Value {
    let root: Value = serde_json::from_str(VECTORS).expect("webauthn vectors parse");
    root["cases"]
        .as_array()
        .expect("cases array")
        .iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("vector case {name} not found"))
        .clone()
}

fn hex(value: &Value) -> String {
    value
        .as_str()
        .expect("hex string")
        .trim_start_matches("0x")
        .to_owned()
}

/// A real CTAP2 attestation object whose key is the same identity the assertion
/// pair below signs with — so a test can register, verify and recover
/// consistently. The vector's flags byte (0x45 = UP|UV|AT) is patched to
/// 0x5d (adds BE|BS): the DEFAULT fixture models the common case — a synced
/// platform passkey — so single-key flows are not tripped by the
/// second-key gate. [`device_bound_attestation_object_hex`] keeps the raw
/// device-bound byte for the tests OF that gate.
pub fn attestation_object_hex() -> String {
    device_bound_attestation_object_hex().replacen("d14df34945", "d14df3495d", 1)
}

/// The vector's attestation verbatim: flags 0x45, no BE/BS — a device-bound
/// credential (e.g. a security key with no sync fabric).
pub fn device_bound_attestation_object_hex() -> String {
    hex(&case("extractPublicKey/real-key")["input"]["attestation_object"])
}

/// A registration whose credential is DEVICE-BOUND (no BE/BS): the sole-key
/// case the second-key gate exists for.
pub fn device_bound_registration(credential_id: &str) -> Registration {
    Registration {
        attestation_object_hex: device_bound_attestation_object_hex(),
        ..registration(credential_id)
    }
}

/// The uncompressed public key that attestation and the assertion pair share.
pub fn expected_public_key_hex() -> String {
    let case = case("extractPublicKey/real-key");
    format!(
        "04{}{}",
        hex(&case["expect"]["x"]),
        hex(&case["expect"]["y"])
    )
}

pub fn registration(credential_id: &str) -> Registration {
    Registration {
        credential_id: credential_id.to_owned(),
        attestation_object_hex: attestation_object_hex(),
        client_data_json_hex: hex_of(r#"{"type":"webauthn.create","challenge":"Y2hhbGxlbmdl"}"#),
        authenticator_attachment: "platform".to_owned(),
        transports: "hybrid,internal".to_owned(),
    }
}

/// A second genuine P-256 curve point (x = 0x11…14 is the first x in that
/// run whose y² is a quadratic residue), distinct from the conformance key.
pub const SECOND_KEY_X: &str = "1111111111111111111111111111111111111111111111111111111111111114";
pub const SECOND_KEY_Y: &str = "d8dd738ca691a327dd14c119194a3d96dbb93d2cb9edab12387669ec973cb024";

/// The conformance attestation object with its COSE coordinates replaced by
/// [`SECOND_KEY_X`]/[`SECOND_KEY_Y`] — extraction only decodes and validates
/// the curve point, so multi-key tests get a DISTINCT founding key from a
/// byte-identical CBOR shape.
pub fn second_attestation_object_hex() -> String {
    format!(
        "a363666d74646e6f6e656761747453746d74a0686175746844617461590094\
         a69533717b230610f14ea657c0bd8231dd6fc7b7108f1215a874fbb1d14df349\
         5d00000001000000000000000000000000000000000010\
         cdcdcdcdcdcdcdcdcdcdcdcdcdcdcdcd\
         a5010203262001215820{SECOND_KEY_X}225820{SECOND_KEY_Y}"
    )
    .replace(char::is_whitespace, "")
}

/// The uncompressed second founding key, `04‖x‖y`.
pub fn second_public_key_hex() -> String {
    format!("04{SECOND_KEY_X}{SECOND_KEY_Y}")
}

/// A registration whose attestation carries the SECOND fixture key.
pub fn second_registration(credential_id: &str) -> Registration {
    Registration {
        attestation_object_hex: second_attestation_object_hex(),
        ..registration(credential_id)
    }
}

/// A creation-time membership proof. The core transports it opaquely (the
/// registry verifies it), so a shaped dummy is faithful.
pub fn member_proof(tag: &str) -> vela_core::registry_proof::RegistryProof {
    vela_core::registry_proof::RegistryProof {
        authenticator_data: format!("aa{}", hex_of(tag)),
        client_data_json: hex_of(&format!("{{\"tag\":\"{tag}\"}}")),
        challenge_index: 9,
        type_index: 1,
        r: "11".repeat(32),
        s: "22".repeat(32),
    }
}

/// Two genuine assertions from one credential over different challenges — the
/// exact input two-signature recovery needs.
pub fn assertion_pair(credential_id: &str) -> (Assertion, Assertion) {
    let pair = case("recoverPublicKey/pair-0");
    (
        assertion_from(&pair["input"]["a"], credential_id),
        assertion_from(&pair["input"]["b"], credential_id),
    )
}

/// One genuine, Safe-compatible assertion.
pub fn assertion(credential_id: &str) -> Assertion {
    assertion_pair(credential_id).0
}

/// An assertion whose clientDataJSON field order the Safe verifier rejects —
/// the "incompatible provider" case, byte-shaped exactly like the real thing.
pub fn incompatible_assertion(credential_id: &str) -> Assertion {
    let mut assertion = assertion(credential_id);
    assertion.client_data_json_hex =
        hex_of(r#"{"challenge":"Y2hhbGxlbmdl","type":"webauthn.get"}"#);
    assertion
}

fn assertion_from(value: &Value, credential_id: &str) -> Assertion {
    Assertion {
        credential_id: credential_id.to_owned(),
        signature_der_hex: hex(&value["signature_der"]),
        authenticator_data_hex: hex(&value["authenticator_data"]),
        client_data_json_hex: hex(&value["client_data_json"]),
        user_id_hex: Some(hex_of(&format!(
            "Ann\0{}",
            "0f8fad5b-d9cb-469f-a165-70867728950e"
        ))),
        authenticator_attachment: "platform".to_owned(),
    }
}

pub fn hex_of(text: &str) -> String {
    text.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn account(id: &str, name: &str, address: &str) -> Account {
    Account {
        id: id.to_owned(),
        name: name.to_owned(),
        address: address.to_owned(),
        public_key_hex: expected_public_key_hex(),
        created_at_iso: "2026-08-05T00:00:00.000Z".to_owned(),
        keys: Vec::new(),
    }
}

pub const NOW: &str = "2026-08-05T12:00:00.000Z";

// ---------------------------------------------------------------------------
// Generic driver for the per-domain machines (spec 016)
// ---------------------------------------------------------------------------

use vela_core::app::SplitEffect;

/// Same contract as [`Driver`], for any machine whose effect enum implements
/// [`SplitEffect`] — dispatch, collect operations, resolve in order.
pub struct DomainDriver<A>
where
    A: App + Default,
    A::Model: Default,
    A::Effect: SplitEffect,
{
    app: Core<A>,
    pending: VecDeque<Request<<A::Effect as SplitEffect>::Op>>,
}

impl<A> DomainDriver<A>
where
    A: App + Default,
    A::Model: Default,
    A::Effect: SplitEffect,
    <A::Effect as SplitEffect>::Op: Clone,
{
    pub fn new() -> Self {
        Self {
            app: Core::new(),
            pending: VecDeque::new(),
        }
    }

    /// Send an event; returns the operations the core asked for, in order.
    pub fn dispatch(&mut self, event: A::Event) -> Vec<<A::Effect as SplitEffect>::Op> {
        let effects = self.app.process_event(event);
        self.collect(effects)
    }

    /// Answer the oldest outstanding operation.
    pub fn resolve(
        &mut self,
        result: <<A::Effect as SplitEffect>::Op as crux_core::capability::Operation>::Output,
    ) -> Vec<<A::Effect as SplitEffect>::Op> {
        let mut request = self
            .pending
            .pop_front()
            .expect("no outstanding shell operation to resolve");
        match self.app.resolve(&mut request, result) {
            Ok(effects) => self.collect(effects),
            Err(_) => Vec::new(),
        }
    }

    /// Answer the oldest outstanding operation matching `predicate`, leaving
    /// the others outstanding — for the cases where two operations are
    /// legitimately in flight and the ORDER they answer in is the thing under
    /// test (a scan racing a store read, say).
    pub fn resolve_matching(
        &mut self,
        predicate: impl Fn(&<A::Effect as SplitEffect>::Op) -> bool,
        result: <<A::Effect as SplitEffect>::Op as crux_core::capability::Operation>::Output,
    ) -> Vec<<A::Effect as SplitEffect>::Op> {
        let index = self
            .pending
            .iter()
            .position(|request| predicate(&request.operation))
            .expect("no outstanding shell operation matches");
        let mut request = self
            .pending
            .remove(index)
            .expect("index came from position()");
        match self.app.resolve(&mut request, result) {
            Ok(effects) => self.collect(effects),
            Err(_) => Vec::new(),
        }
    }

    pub fn view(&self) -> A::ViewModel {
        self.app.view()
    }

    /// Operations still awaiting an answer.
    pub fn outstanding(&self) -> Vec<<A::Effect as SplitEffect>::Op> {
        self.pending
            .iter()
            .map(|request| request.operation.clone())
            .collect()
    }

    /// Drop the oldest outstanding operation unanswered — "the shell never
    /// got back to us". Used to assert what happens to abandoned requests.
    pub fn drop_oldest(&mut self) {
        self.pending.pop_front();
    }

    /// Drop every outstanding operation the predicate names, unanswered — a
    /// timer the run never lets fire, whatever else is queued around it.
    pub fn drop_matching(&mut self, predicate: impl Fn(&<A::Effect as SplitEffect>::Op) -> bool) {
        self.pending
            .retain(|request| !predicate(&request.operation));
    }

    fn collect(&mut self, effects: Vec<A::Effect>) -> Vec<<A::Effect as SplitEffect>::Op> {
        let mut operations = Vec::new();
        for effect in effects {
            if let Some(request) = effect.into_shell() {
                operations.push(request.operation.clone());
                self.pending.push_back(request);
            }
        }
        operations
    }
}

impl<A> Default for DomainDriver<A>
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
