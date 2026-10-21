//! The AI agent's shared core (spec 103, spec 104 phase 7): one copy of what
//! every native shell does for an agent, the shells keeping only what is
//! theirs — their machines, their screens.
//!
//! - [`intents`] — the runner: check → examine → approve → perform →
//!   observe, the person's approvals, Stop, a call given up. Generic over the
//!   wallet ([`intents::Deps`]).
//! - [`transfers`] — drafting a transfer onto the shell's review and following
//!   it to the chain, over the shell's send ([`transfers::SendDesk`]).
//! - [`tools`] — the ClientMCP tools, the model's instructions, the answers.
//! - [`connection`] — the bridge connection, on a thread of its own;
//!   [`bridge_url`] — where it may be.
//!
//! The judgements themselves are vela-core's (`intents`, `intent_rules`).

pub mod bridge_url;
pub mod connection;
pub mod intents;
pub mod shell;
pub mod tools;
pub mod transfers;
