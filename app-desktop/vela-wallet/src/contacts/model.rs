//! The display models the contacts screen renders.
//!
//! The twin of `settings/model.rs`, and here for the same reason: `page.rs` read
//! `contacts_fixtures::sections()` and rendered `&'static str` fields directly,
//! so a live builder had nothing to plug into.

use gpui::SharedString;

/// One row of the A–Z roster.
#[derive(Clone, Debug, PartialEq)]
pub struct ContactRowModel {
    /// What the row shows as the name: the person's own label if they gave one,
    /// else a resolved identity, else the shortened address. The core carries
    /// all three and refuses to choose — display precedence is a render rule.
    pub name: SharedString,
    pub address_display: SharedString,
    /// The full address. The identicon is derived from it, so it must be the
    /// canonical lowercase form or two spellings of one person draw two
    /// different faces.
    pub address_full: SharedString,
    pub section: SharedString,
}

/// `0x1234…abcd`, the shortening the mocks draw.
///
/// Named here because the fixture adapter needs it too: a live row and a mock
/// row must shorten identically, or the seam shows. It is the wallet's one
/// shortening (`wallet::live::shorten_address`), which counts characters
/// rather than bytes (083 H2 review), so every surface draws the same one.
#[must_use]
pub fn shorten(address: &str) -> SharedString {
    SharedString::from(crate::wallet::live::shorten_address(address))
}

// The A–Z rule used to live here, ASCII-only: 阿豪 filed under `#`. It went to
// the core in spec 028 (`app/contacts_initials.rs`, a per-codepoint pinyin
// initial table: 阿豪 → A, 妈妈 → M, an address → `#`), because the same person
// must not file under a different letter on two clients. The shell now reads
// `ContactsView.sections`.
