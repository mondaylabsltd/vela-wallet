//! The Clear Signer's dialogs: the wait, and how it
//! ended (specs 071 and 075).
//!
//! The page does the work in a browser, so what the wallet can show is that it
//! is waiting — with the one thing the browser cannot do for itself, opening
//! the page again — and the way out.
//! When an attempt ends unanswered the sheet says why in the corpus's own
//! sentence (071 contract §5); the request itself is still open underneath, to
//! be answered another way.
//!
//! Free functions over `(theme, loc, …)`, like the cable's dialogs in
//! `hardware`, and on the same card — so the gallery renders the real ones and
//! a person waiting on a page and a person waiting on a key see one kind of
//! card.

use gpui::{App, Div, ParentElement, Styled, Window, div, px};

use crate::executor::clear_signer::Refusal;
use crate::hardware::{body, card, title};
use crate::loc::Loc;
use crate::theme::{FLOW_GAP_SM, TOUCH_DISC, Theme};
use crate::ui::{ButtonVariant, vela_button};

/// "Waiting for the Clear Signer…" — over the flow while the page has the
/// request. The hint says what the page's own browser may ask first (Chrome
/// asks before a page reaches this computer's loopback, research R3).
pub fn waiting_card(
    theme: &Theme,
    loc: &Loc,
    on_reopen: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    card(theme)
        .items_center()
        // The touch prompt's disc, in the same place: a ceremony is under way
        // somewhere other than this window.
        .child(
            div()
                .size(px(TOUCH_DISC))
                .flex_none()
                .rounded_full()
                .bg(theme.bg_well)
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .size(px(TOUCH_DISC / 2.4))
                        .rounded_full()
                        .bg(theme.accent),
                ),
        )
        .child(title(
            theme,
            loc.t("componentsUi.signing.clearSignerWaiting"),
        ))
        .child(body(
            theme,
            loc.t("componentsUi.signing.clearSignerWaitingHint"),
        ))
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(FLOW_GAP_SM))
                .child(vela_button(
                    "clear-signer-reopen",
                    ButtonVariant::Secondary,
                    loc.t("componentsUi.signing.clearSignerReopen"),
                    theme,
                    on_reopen,
                ))
                .child(vela_button(
                    "clear-signer-cancel",
                    ButtonVariant::Secondary,
                    loc.t("common.cancel"),
                    theme,
                    on_cancel,
                )),
        )
}

/// Why the last ceremony ended unsigned. Nothing was signed and nothing was
/// sent, whichever sentence it is.
pub fn ended_card(
    theme: &Theme,
    loc: &Loc,
    refusal: Refusal,
    on_done: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    card(theme)
        .items_center()
        .child(title(theme, loc.t("componentsUi.signing.clearSignerTitle")))
        .child(body(theme, loc.t(refusal.key())))
        .child(vela_button(
            "clear-signer-done",
            ButtonVariant::Primary,
            loc.t("common.done"),
            theme,
            on_done,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every ending has its own sentence in every build — a key echoed on
    /// this card is the person told nothing at the one moment something went
    /// wrong, and two endings sharing a sentence is the person told the wrong
    /// thing.
    #[test]
    fn every_ending_has_its_own_sentence() {
        let loc = Loc::from_env();
        let mut said: Vec<String> = [
            Refusal::Closed,
            Refusal::Refused,
            Refusal::Mismatch,
            Refusal::TimedOut,
        ]
        .into_iter()
        .map(|refusal| {
            let words = loc.t(refusal.key()).to_string();
            assert_ne!(words, refusal.key(), "{refusal:?} echoed its key");
            assert!(!words.is_empty(), "{refusal:?} resolved empty");
            words
        })
        .collect();
        let endings = said.len();
        said.sort();
        said.dedup();
        assert_eq!(said.len(), endings, "two endings share one sentence");
    }

    /// Every word these two cards can show resolves, in whatever language this
    /// build starts in. A card whose title is a dotted key is a person stuck
    /// at the one screen they cannot get past.
    #[test]
    fn every_card_has_its_words() {
        let loc = Loc::from_env();
        for key in [
            "componentsUi.signing.clearSignerWaiting",
            "componentsUi.signing.clearSignerWaitingHint",
            "componentsUi.signing.clearSignerReopen",
            "componentsUi.signing.clearSignerTitle",
            "common.cancel",
            "common.done",
        ] {
            let said = loc.t(key);
            assert_ne!(said.as_ref(), key, "`{key}` echoed");
            assert!(!said.is_empty(), "`{key}` resolved empty");
        }
    }
}
