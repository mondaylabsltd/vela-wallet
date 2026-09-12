//! The display models the settings screen renders.
//!
//! Introduced by spec 030, and the reason it did not exist before is the reason
//! it has to now: `page.rs` read fixture constants directly, so there was no
//! seam a live builder could plug into. Both `fixtures.rs` and `live.rs` produce
//! these, the screen renders whichever it is handed, and "the galleries are
//! unchanged" becomes something a diff can prove.
//!
//! Only the surfaces this cut takes live are modelled. A model for a screen
//! nothing feeds yet would be a guess about a shape 031 has not chosen.

use gpui::SharedString;

/// One row of 设置 → 网络.
#[derive(Clone, Debug, PartialEq)]
pub struct NetworkRowModel {
    /// Identity for the expand/collapse state. A fixture id (`"ethereum"`) or
    /// the core's own row id — the screen only compares it to itself.
    pub id: SharedString,
    pub name: SharedString,
    pub letter: SharedString,
    pub color: u32,
    pub chain_id: u64,
    /// `Some` ⇒ draw the latency badge. `None` covers three different things
    /// the mock also draws without one: a custom network, a probe still in
    /// flight, and a probe that failed. The core distinguishes them; this row
    /// only needs to know whether a number exists.
    pub latency_ms: Option<u32>,
    pub custom: bool,
}

/// The brand tint for a chain, or `None` for one nothing has drawn.
///
/// Read out of `fixtures::NETWORKS` rather than duplicated here: those colours
/// are design data and the fixture file is where the design lives, so a live row
/// for Ethereum is tinted by the same constant the mock is. A chain the mocks
/// never drew has no brand colour to borrow, and the caller supplies a neutral —
/// inventing one would be the shell making a visual decision nobody designed.
#[must_use]
pub fn chain_tint(chain_id: u64) -> Option<u32> {
    super::fixtures::NETWORKS
        .iter()
        .find(|n| n.chain_id == chain_id)
        .map(|n| n.color)
}

/// The lettermark for a name, matching what the mocks draw: the first character,
/// uppercased.
#[must_use]
pub fn lettermark(name: &str) -> SharedString {
    name.chars()
        .next()
        .map(|c| SharedString::from(c.to_uppercase().to_string()))
        .unwrap_or_else(|| SharedString::from("?"))
}
