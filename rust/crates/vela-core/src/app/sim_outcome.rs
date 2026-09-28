//! What a transaction simulation's answer MEANS for the signing sheet (spec 082, L-D5).
//!
//! Pure. The pool's raw `eth_simulateV1` reply (an answer, a JSON-RPC error,
//! or no answer at all) becomes one outcome: the balance deltas it shows, a
//! revert with an untrusted reason made safe to print, "this node does not
//! offer simulation", or "unreachable" — and the notice each one draws.
//! The desktop, iOS and Android parsers move here so the severity is one rule.
//! Filled in by G-core (T039); registered early (T009) so no later group edits
//! `app/mod.rs`.
