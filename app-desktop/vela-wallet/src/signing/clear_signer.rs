//! The Clear Signer's dialogs: where it is, the pairing, the wait, and how it
//! ended (specs 071 and 075).
//!
//! The page does the work in a browser, so what the wallet can show is where
//! that browser is, how to reach it, that it is waiting — with the one thing
//! the browser cannot do for itself, opening the page again — and the way out.
//! When an attempt ends unanswered the sheet says why in the corpus's own
//! sentence (071 contract §5); the request itself is still open underneath, to
//! be answered another way.
//!
//! Free functions over `(theme, loc, …)`, like the cable's dialogs in
//! `hardware`, and on the same card — so the gallery renders the real ones and
//! a person waiting on a page and a person waiting on a key see one kind of
//! card.

use gpui::{
    App, Div, InteractiveElement as _, ParentElement, StatefulInteractiveElement as _, Styled,
    Window, div, px, rgb,
};
use qrcode::{Color as QrColor, QrCode};

use crate::executor::clear_signer::{Pairing, Place, Refusal};
use crate::hardware::{body, card, title};
use crate::loc::Loc;
use crate::outcome::SHEET_RADIUS;
use crate::theme::{self, FLOW_GAP_MD, FLOW_GAP_SM, TOUCH_DISC, Theme};
use crate::ui::{ButtonVariant, vela_button, vela_button_opts};

/// How wide the pairing QR is drawn. The relay link is long — a page URL, a
/// percent-encoded relay address, a room and a fingerprint — so its matrix has
/// many more modules than a caBLE payload's, and a fixed module size would
/// paint a card nobody can fit a phone in front of. The WIDTH is fixed instead
/// and the module divides into it.
const PAIR_QR: f32 = 232.;

/// What a screen does when the person says where their Clear Signer is. An
/// `Arc` rather than a closure per row: the card draws two rows from one
/// handler, and gpui's own listeners are not `Clone`.
pub type PickPlace = std::sync::Arc<dyn Fn(Place, &mut Window, &mut App)>;

/// "Where is your Clear Signer?" — asked whenever the route is chosen, because
/// both answers are real on every desktop (contract §2): this computer's own
/// browser over a loopback socket, or the phone in the person's hand over the
/// relay.
///
/// Neither row is greyed out and neither is a default. A wallet cannot know
/// which browser holds somebody's passkey, and guessing wrong here means a
/// ceremony that opens the wrong screen and then fails.
pub fn where_card(
    theme: &Theme,
    loc: &Loc,
    on_pick: PickPlace,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let row = |place: Place, id: &'static str, key: &str| {
        let on_pick = std::sync::Arc::clone(&on_pick);
        div()
            .id(id)
            .w_full()
            .flex()
            .items_center()
            .py(px(FLOW_GAP_MD))
            .border_b_1()
            .border_color(theme.border_card)
            .cursor_pointer()
            .hover(|style| style.bg(theme.bg_well))
            .on_click(move |_, window, cx| on_pick(place, window, cx))
            .child(
                div()
                    .text_size(theme::text_card_title())
                    .text_color(theme.fg_base)
                    .child(loc.t(key)),
            )
    };
    card(theme)
        .child(title(theme, loc.t("componentsUi.signing.clearSignerWhere")))
        .child(row(
            Place::ThisDevice,
            "clear-signer-here",
            "componentsUi.signing.clearSignerThisDevice",
        ))
        .child(row(
            Place::OtherDevice,
            "clear-signer-there",
            "componentsUi.signing.clearSignerOtherDevice",
        ))
        .child(vela_button(
            "clear-signer-where-cancel",
            ButtonVariant::Secondary,
            loc.t("common.cancel"),
            theme,
            on_cancel,
        ))
}

/// The cross-device pairing (relay.md §§2–3): the link as a QR and as
/// something to copy, then the six digits.
///
/// Two states on one card, because they are one moment for the person: while
/// nothing has joined the room it is "open this on your other device", and the
/// instant both ends have derived a code it is "do these two screens agree?".
///
/// **The confirm button is the security boundary**, not a courtesy. Until it is
/// pressed the wallet has sent the relay nothing but its own hello — which is
/// what stops a stand-in page (somebody who got the link) from being handed a
/// create and answering it with a key of their own.
pub fn pair_card(
    theme: &Theme,
    loc: &Loc,
    pairing: &Pairing,
    on_copy: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    on_confirm: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let mut sheet = card(theme)
        .items_center()
        .child(title(theme, loc.t("componentsUi.signing.clearSignerPair")));
    match &pairing.code {
        // Both ends are there: the only thing left is the person's eyes.
        Some(code) => {
            sheet = sheet
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_flow_headline())
                        .text_color(theme.fg_base)
                        .child(gpui::SharedString::from(code.clone())),
                )
                .child(body(
                    theme,
                    loc.t_text("componentsUi.signing.clearSignerCode", "code", code),
                ))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap(px(FLOW_GAP_SM))
                        .child(vela_button_opts(
                            "clear-signer-code-confirm",
                            ButtonVariant::Primary,
                            loc.t("componentsUi.signing.clearSignerCodeConfirm"),
                            !pairing.confirmed,
                            theme,
                            on_confirm,
                        ))
                        .child(vela_button(
                            "clear-signer-pair-cancel",
                            ButtonVariant::Secondary,
                            loc.t("common.cancel"),
                            theme,
                            on_cancel,
                        )),
                );
        }
        None => {
            sheet = sheet
                .child(qr_block(&pairing.link))
                .child(body(
                    theme,
                    loc.t("componentsUi.signing.clearSignerPairHint"),
                ))
                .child(body(
                    theme,
                    loc.t("componentsUi.signing.clearSignerPairWaiting"),
                ))
                .child(
                    div()
                        .w_full()
                        .flex()
                        .flex_col()
                        .gap(px(FLOW_GAP_SM))
                        .child(vela_button(
                            "clear-signer-copy-link",
                            ButtonVariant::Secondary,
                            loc.t("componentsUi.signing.clearSignerCopyLink"),
                            theme,
                            on_copy,
                        ))
                        .child(vela_button(
                            "clear-signer-pair-cancel",
                            ButtonVariant::Secondary,
                            loc.t("common.cancel"),
                            theme,
                            on_cancel,
                        )),
                );
        }
    }
    sheet
}

/// The pairing link as a scannable target.
///
/// **Black on white, always — not themed**, for the same reason the caBLE QR is
/// (`hardware::qr_card`): a phone camera needs dark modules on a light field
/// whatever this app's palette is. A link too long for any QR version draws
/// nothing rather than panicking — the copyable address beside it still works.
fn qr_block(link: &str) -> Div {
    let matrix: Div = match QrCode::new(link.as_bytes()) {
        Ok(code) => {
            let width = code.width();
            let colors = code.to_colors();
            #[allow(
                clippy::cast_precision_loss,
                reason = "a QR is at most 177 modules a side"
            )]
            let module = PAIR_QR / width as f32;
            let mut grid = div().flex().flex_col();
            for row in 0..width {
                let mut line = div().flex().flex_row();
                for col in 0..width {
                    let dark = matches!(colors.get(row * width + col), Some(QrColor::Dark));
                    let mut cell = div().size(px(module));
                    if dark {
                        cell = cell.bg(rgb(0x000000));
                    }
                    line = line.child(cell);
                }
                grid = grid.child(line);
            }
            grid
        }
        Err(_) => div(),
    };
    div()
        .p(px(FLOW_GAP_MD))
        .rounded(px(SHEET_RADIUS))
        .bg(rgb(0xffffff))
        .child(matrix)
}

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
