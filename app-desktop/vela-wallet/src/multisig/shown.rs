//! Whose wallet the pages show: the multisig shown, else the session's own
//! Vela account (the web's `core/shown.ts`). Every surface that wears an
//! identity — the header, the hero, Activity, Receive — reads it here, so
//! moving between screens never changes whose balance or activity is on
//! screen. The accounts machine decides which multisig is shown
//! (`active`); this only carries it to the window.

use gpui::{App, Global};

/// The multisig shown, as the window wears it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShownAccount {
    pub address: String,
    /// Its record's name (or the person's label), if any.
    pub name: Option<String>,
    /// The original rule: m of n.
    pub threshold: u64,
    pub owners: u64,
}

/// The global the root observes: a change rebuilds the wallet page for the
/// address shown, as an account switch does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Shown(pub Option<ShownAccount>);

impl Global for Shown {}

/// The multisig shown, if any.
#[must_use]
pub fn current(cx: &App) -> Option<ShownAccount> {
    cx.try_global::<Shown>().and_then(|s| s.0.clone())
}

/// The address the pages show: the multisig's, else the session's.
#[must_use]
pub fn address(cx: &App) -> String {
    current(cx).map_or_else(|| crate::session::view(cx).address, |s| s.address)
}

/// Set what is shown; observers wake only on a change.
pub fn set(shown: Option<ShownAccount>, cx: &mut App) {
    if current(cx) != shown || !cx.has_global::<Shown>() {
        cx.set_global(Shown(shown));
    }
}
