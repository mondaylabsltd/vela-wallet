//! Multisig accounts on the desktop (AUDIT M1–M6): the SAME core machines
//! the web drives (`vela_core::app::multisig_*`, docs/multisig-machines.md),
//! in-process. The owner's rule: one core for every business rule — this
//! module draws and does I/O only.
//!
//! - [`shell`] — the one executor: every `MultisigOperation`, sorted by
//!   where it is performed;
//! - [`machines`] — the one host: boot from the app's facts, the notice
//!   router, what one machine's view tells another (no gpui).

pub mod activity;
#[cfg(feature = "dev-fixtures")]
pub mod dev_script;
pub mod findings;
pub mod host;
pub mod live;
pub mod machines;
pub mod notify;
pub mod peer;
#[cfg(all(test, feature = "dev-fixtures"))]
mod scenarios;
pub mod session_words;
pub mod shell;
pub mod shown;
pub mod words;
