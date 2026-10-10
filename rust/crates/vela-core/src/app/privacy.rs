//! Balance privacy: ONE rule for every money surface (spec 016 invariant ⑧).
//!
//! The threat model is handing the phone to somebody: once the person hides
//! their balance, nothing money-shaped they did not ask to see may show, and a
//! restart never brings it back. A leak on one surface defeats the mask on
//! every other, so the rule lives here, once, and the four shells only draw it.
//!
//! **What the core withholds by construction while hidden** — never sent, so
//! no shell can leak it:
//! - the hero's figure (`BalanceView::display_total_usd`),
//! - the cached total (`BalanceView::cached_total_usd`),
//! - every account switcher figure (`BalanceSwitcherView::balances` is empty,
//!   `BalanceSwitcherView::hidden` says why),
//! - an unreachable network's last-seen worth
//!   (`UnreachableNetwork::last_seen_usd`),
//! - the receipt toast (`FeedView::toast`).
//!
//! **What a shell masks** with [`MASK`] (the hero with [`BALANCE_MASK`]),
//! reading `BalanceView::hidden` / `FeedView::hidden`: every figure — token
//! amount AND its fiat worth — on every surface in [`MoneySurface::MASKED`].
//! A feed row's own figure masks when `FeedItem::figure_maskable` says it is
//! money; an unlimited allowance is a risk to see, and a signature with no
//! figure has nothing to hide (a mask there would claim a figure).
//!
//! **A masked amount keeps its unit** ([`masked_amount`]): "•••• xDAI", never
//! a bare "••••", wherever the shown figure carries one ("0.5 xDAI"). The
//! unit says what kind of money moved without saying how much — the secret
//! is the number — and one shell dropping it while another keeps it is the
//! same transfer reading two ways (the 102 device run: a hidden transfer's
//! detail read "•••• xDAI" on iOS and "••••" on Android). A figure drawn
//! with no unit beside it (a holding's amount under its own ticker, a fiat
//! total) masks to [`MASK`] alone, as before.
//!
//! **What stays visible on purpose** ([`MoneySurface::VISIBLE`]): the flows a
//! person starts, where the number IS the decision — Send (the picker, Max,
//! the confirm), the signing sheet (balance changes, the fee) and Receive's
//! "arrived" list (the payer's own transfer, not what the wallet holds). Never
//! "fix" these: you cannot choose an amount, or consent to one, you cannot see.
//!
//! The shared fixture `tests/fixtures/privacy-hidden.json` carries a hidden
//! balance and feed with distinctive figures; every shell replays it through
//! every surface builder it has and asserts no output contains those digits.

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::activity_feed::FeedItem;

/// What a masked figure draws: the same four dots on every surface, every
/// shell.
pub const MASK: &str = "••••";

/// The hero's mask — one glyph wider, so the hidden total keeps its weight.
pub const BALANCE_MASK: &str = "••••••";

/// A masked amount as every shell draws it: the mask, then the unit the
/// shown figure carries — "•••• xDAI". The one rule for whether a hidden
/// amount keeps its unit: it does. `unit` is the token's symbol exactly as
/// the shown figure prints it; an empty one (a figure with no unit of its
/// own) gives [`MASK`] alone, never a trailing space.
#[must_use]
pub fn masked_amount(unit: &str) -> String {
    let unit = unit.trim();
    if unit.is_empty() {
        MASK.to_owned()
    } else {
        format!("{MASK} {unit}")
    }
}

/// Every surface that draws a money figure, by whether privacy masks it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum MoneySurface {
    /// The home total (the core withholds it; the shell draws [`BALANCE_MASK`]).
    HomeTotal,
    /// The home holdings list: each token's amount and its fiat worth.
    Holdings,
    /// The Assets page.
    Assets,
    /// A token's detail: its balance, its worth, and its activity rows.
    TokenDetail,
    /// The home Activity rows: the figure and a dApp row's "received".
    HomeActivity,
    /// History.
    History,
    /// A transfer's detail.
    TransferDetail,
    /// A dApp row's detail: its balance changes and what came back.
    DappDetail,
    /// A contact's page: what passed between you.
    ContactActivity,
    /// The account switcher: every row and its total.
    AccountSwitcher,
    /// The balance detail sheet, its unpriced list and its unreachable list.
    BalanceDetail,
    /// Explore's network picker holdings.
    NetworkPicker,
    /// The "received" toast (the core withholds it).
    ReceiptToast,
    /// Send: the picker, Max and the confirm. Visible.
    Send,
    /// The signing sheet: balance changes and the fee. Visible.
    SigningSheet,
    /// Receive's "arrived" list. Visible.
    Receive,
}

impl MoneySurface {
    /// Masked while the balance is hidden.
    pub const MASKED: [Self; 13] = [
        Self::HomeTotal,
        Self::Holdings,
        Self::Assets,
        Self::TokenDetail,
        Self::HomeActivity,
        Self::History,
        Self::TransferDetail,
        Self::DappDetail,
        Self::ContactActivity,
        Self::AccountSwitcher,
        Self::BalanceDetail,
        Self::NetworkPicker,
        Self::ReceiptToast,
    ];

    /// Never masked: the number is the decision.
    pub const VISIBLE: [Self; 3] = [Self::Send, Self::SigningSheet, Self::Receive];

    /// Whether this surface's figures draw as [`MASK`] while `hidden`.
    #[must_use]
    pub fn masks(self, hidden: bool) -> bool {
        hidden && !Self::VISIBLE.contains(&self)
    }
}

/// Whether a feed row's own figure is money, and so masks while hidden.
///
/// - an amount (sent, received, a dApp's outflow) — money;
/// - a batch's asset count, the figure of a mixed-token batch — masked with
///   the rest of the row, as every shell already drew it;
/// - an allowance with a cap — money;
/// - an UNLIMITED allowance — not an amount but a risk to see: never masked;
/// - a dApp row (a signature, a call) that moved and granted nothing — no
///   figure, nothing to mask: four dots there would claim one.
#[must_use]
pub fn figure_maskable(item: &FeedItem) -> bool {
    if item.value.is_some() {
        return true;
    }
    match &item.dapp {
        Some(dapp) => dapp
            .allowance
            .as_ref()
            .is_some_and(|allowance| !allowance.unlimited && allowance.value.is_some()),
        None => item.batch.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One rule on every shell: a hidden amount keeps its unit and hides
    /// only the number; a figure with no unit is the mask alone.
    #[test]
    fn a_masked_amount_keeps_its_unit() {
        assert_eq!(masked_amount("xDAI"), "•••• xDAI");
        assert_eq!(masked_amount("USDC"), format!("{MASK} USDC"));
        assert_eq!(masked_amount(""), MASK);
        assert_eq!(masked_amount("  "), MASK, "never a trailing space");
        assert!(!masked_amount("ETH").chars().any(|c| c.is_ascii_digit()));
    }
}
