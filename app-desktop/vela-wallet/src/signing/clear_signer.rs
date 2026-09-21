//! The Clear Signer's two dialogs (spec 071): the wait, and how it ended.
//!
//! The page does the signing in the person's browser, so the only thing the
//! wallet can show meanwhile is that it is waiting — with the one thing the
//! browser cannot do for it, opening the page again, and the way out. When a
//! ceremony ends unsigned the sheet says why in the corpus's own sentence
//! (contract §5); the request itself is still open underneath, to be signed
//! another way.
//!
//! Free functions over `(theme, loc, …)`, like the cable's dialogs in
//! `hardware`, and on the same card.

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

    /// Every ending has its own sentence in every build, and the waiting
    /// sheet's words resolve — a key echoed on this card is the person told
    /// nothing at the one moment something went wrong.
    #[test]
    fn every_ending_and_the_wait_have_words() {
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
            words
        })
        .collect();
        said.sort();
        said.dedup();
        assert_eq!(said.len(), 4, "two endings share one sentence");
        for key in [
            "componentsUi.signing.clearSignerWaiting",
            "componentsUi.signing.clearSignerWaitingHint",
            "componentsUi.signing.clearSignerReopen",
            "componentsUi.signing.clearSignerTitle",
        ] {
            assert_ne!(loc.t(key).as_ref(), key, "`{key}` echoed");
        }
    }
}
