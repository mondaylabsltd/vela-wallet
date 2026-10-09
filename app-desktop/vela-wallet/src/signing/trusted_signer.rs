//! The trusted signing page's cards (specs 071, 075; spec 102 D4): the
//! hand-off before the page opens, the wait while it has the request, and how
//! an attempt ended.
//!
//! **The hand-off (D4).** When an account reviews and signs on a trusted page,
//! the app does not draw the request a second time — the page is the
//! authority, and two previews of one transaction left people asking which
//! one to believe. The card says only what the page cannot: where it is, the
//! key the person will confirm with, and one integrity line that backs the
//! word "trusted" ("Version 0ba8ee8c · matches Vela's published build list ·
//! checked 14:32", or why the page will not open). Open is enabled only when
//! that line says the page may open.
//!
//! The page does the work in a browser, so while it has the request the
//! wallet shows that it is waiting — with the one thing the browser cannot do
//! for itself, opening the page again — and the way out. When an attempt ends
//! unanswered the card says why in the corpus's own sentence (071 contract
//! §5); the request itself is still open underneath, to be answered another
//! way.
//!
//! Free functions over `(theme, loc, …)`, like the cable's dialogs in
//! `hardware`, and on the same card — so the gallery renders the real ones and
//! a person waiting on a page and a person waiting on a key see one kind of
//! card.

use gpui::{
    App, Div, FontWeight, InteractiveElement as _, IntoElement as _, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};

use vela_core::app::KeyMethod;
use vela_core::app::method_words::{DeviceUnlock, KeyChooser};
use vela_core::signing_venue::VenueBlock;
use vela_core::trusted_signer::launch::IntegrityLine;

use crate::executor::send::Handoff;
use crate::executor::trusted_signer::{NotOpened, Refusal};
use crate::hardware::{body, card, title};
use crate::icons::{Icon, IconCache};
use crate::loc::Loc;
use crate::signing::Tone;
use crate::signing::integrity;
use crate::theme::{self, FLOW_GAP_SM, TOUCH_DISC, Theme};
use crate::ui::{ButtonState, ButtonVariant, vela_button, vela_button_state};
use crate::wallet::components::icon_img;

/// What a click on a card does.
pub type Click = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static>;

// ---------------------------------------------------------------------------
// The hand-off (D4)
// ---------------------------------------------------------------------------

/// The hand-off card, worded.
#[derive(Clone, Debug, PartialEq)]
pub struct HandoffModel {
    /// "Review and sign on your trusted page".
    pub title: SharedString,
    /// The page's host — WHAT is trusted, named. `None` when nothing can
    /// open (the account's keys are out of reach).
    pub page: Option<SharedString>,
    /// "Confirm with This device" — the key's name, or its place.
    pub key: SharedString,
    pub place: KeyMethod,
    /// The integrity line and its tone; `None` when no page is involved.
    pub integrity: Option<(SharedString, Tone)>,
    /// Why nothing on this device can sign for the account (R1).
    pub blocked: Option<SharedString>,
    /// May the page be opened? The core's word (`IntegrityLine::opens`), and
    /// never when the keys are out of reach.
    pub opens: bool,
    /// The last check refused the page: "Check again" is offered.
    pub refused: bool,
    /// The button: "Continue to signing page".
    pub open_label: SharedString,
    /// "Try again", under a refused line.
    pub recheck_label: SharedString,
}

/// A key's place, named as the choosers name it ("This device", "Phone or
/// tablet", "USB security key").
#[must_use]
pub fn place_title(loc: &Loc, place: KeyMethod) -> SharedString {
    loc.t(place
        .words(KeyChooser::Create, DeviceUnlock::Other)
        .title_key)
}

/// R1's reason, in the corpus's sentence.
#[must_use]
pub fn block_words(loc: &Loc, block: &VenueBlock) -> SharedString {
    match block {
        VenueBlock::AppCannotReach { domain } => {
            loc.t_texts(block.key(), &[("domain", domain.as_str())])
        }
        VenueBlock::PageOnOtherDomain {
            page_domain,
            domain,
        } => loc.t_texts(
            block.key(),
            &[
                ("pageDomain", page_domain.as_str()),
                ("domain", domain.as_str()),
            ],
        ),
    }
}

/// The page's host, as a person recognises it (`sign.getvela.app`,
/// `localhost:8140/clearsigning`).
#[must_use]
pub fn page_host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['?', '#'])
        .next()
        .unwrap_or_default()
        .trim_end_matches('/')
        .to_owned()
}

/// The card's words for `handoff`, with the page's integrity `line` (the
/// board's, for the venue page).
#[must_use]
pub fn handoff_model(loc: &Loc, handoff: &Handoff, line: Option<&IntegrityLine>) -> HandoffModel {
    let key = handoff
        .key_name
        .clone()
        .map_or_else(|| place_title(loc, handoff.key_place), SharedString::from);
    let blocked = handoff.block.as_ref().map(|block| block_words(loc, block));
    let integrity = line
        .filter(|_| blocked.is_none())
        .map(|line| (integrity::text(loc, line), integrity::tone(line)));
    let opens = blocked.is_none() && line.is_some_and(|line| line.opens);
    HandoffModel {
        title: loc.t("componentsUi.signing.handoffTitle"),
        page: handoff
            .page
            .as_deref()
            .filter(|_| blocked.is_none())
            .map(|page| SharedString::from(page_host(page))),
        key: loc.t_texts("componentsUi.signing.handoffKey", &[("key", key.as_ref())]),
        place: handoff.key_place,
        refused: line.is_some_and(|line| {
            !line.opens && line.state != vela_core::trusted_signer::launch::IntegrityState::Checking
        }),
        integrity,
        blocked,
        opens,
        open_label: loc.t("componentsUi.signing.openSigner"),
        recheck_label: loc.t("common.tryAgain"),
    }
}

/// The mark of a key's place, from the app's own glyphs.
fn place_icon(place: KeyMethod) -> Icon {
    match place {
        KeyMethod::Platform => Icon::Monitor,
        KeyMethod::Hybrid => Icon::ScanLine,
        KeyMethod::SecurityKey => Icon::Lock,
    }
}

/// The card's facts, on one sunken panel: where the page is, the key, and the
/// integrity line — the three things that make "trusted" mean something.
pub fn handoff_facts(
    theme: &Theme,
    icons: &mut IconCache,
    model: &HandoffModel,
    on_recheck: Option<Click>,
) -> Div {
    let fact = |icon: gpui::AnyElement, content: Div| {
        div()
            .flex()
            .items_start()
            .gap(px(12.))
            .py(px(12.))
            .child(div().flex_none().pt(px(1.)).child(icon))
            .child(content.flex_1().min_w(px(0.)))
    };
    let mut panel = div()
        .flex()
        .flex_col()
        .px(px(16.))
        .rounded(px(theme::RADIUS_FIELD))
        .bg(theme.bg_sunken)
        .border_1()
        .border_color(theme.divider);
    let mut rows: Vec<Div> = Vec::new();
    if let Some(page) = &model.page {
        rows.push(fact(
            icon_img(icons, Icon::Link2, false, theme.fg_muted, 16.).into_any_element(),
            div()
                .font_family(theme::font_mono())
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .truncate()
                .child(page.clone()),
        ));
    }
    rows.push(fact(
        icon_img(icons, place_icon(model.place), false, theme.fg_muted, 16.).into_any_element(),
        div()
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_base)
            .child(model.key.clone()),
    ));
    if let Some(blocked) = &model.blocked {
        rows.push(fact(
            icon_img(icons, Icon::CircleAlert, false, theme.error_base, 16.).into_any_element(),
            div()
                .text_size(theme::text_row_sub())
                .line_height(gpui::relative(1.4))
                .text_color(theme.error_base)
                .child(blocked.clone()),
        ));
    }
    if let Some((said, tone)) = &model.integrity {
        let colour = integrity::ink(theme, *tone);
        let mut words = div().flex().flex_col().gap(px(6.)).child(
            div()
                .text_size(theme::text_row_sub())
                .line_height(gpui::relative(1.4))
                .text_color(if *tone == Tone::Neutral {
                    theme.fg_muted
                } else {
                    colour
                })
                .child(said.clone()),
        );
        if let Some(on_recheck) = on_recheck.filter(|_| model.refused) {
            words = words.child(
                div()
                    .id("handoff-recheck")
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.accent)
                    .child(icon_img(icons, Icon::RefreshCw, false, theme.accent, 12.))
                    .child(model.recheck_label.clone())
                    .on_click(on_recheck),
            );
        }
        rows.push(fact(
            icon_img(icons, integrity::icon(*tone), false, colour, 16.).into_any_element(),
            words,
        ));
    }
    for (index, row) in rows.into_iter().enumerate() {
        panel = panel.child(if index > 0 {
            row.border_t_1().border_color(theme.divider)
        } else {
            row
        });
    }
    panel
}

/// The hand-off card (D4): the eye — this is where you LOOK — the title, the
/// facts and Open. `on_open` is `None` while the request cannot be approved
/// yet (a fee still measuring); the button is drawn either way, disabled
/// until both the request and the page may go.
pub fn handoff_card(
    theme: &Theme,
    icons: &mut IconCache,
    model: &HandoffModel,
    on_open: Option<Click>,
    on_recheck: Option<Click>,
) -> Div {
    let ready = model.opens && on_open.is_some();
    let open = vela_button_state(
        "handoff-open",
        ButtonVariant::Primary,
        model.open_label.clone(),
        ButtonState::from_enabled(ready),
        theme,
        move |event, window, cx| {
            if let Some(on_open) = on_open.as_ref() {
                on_open(event, window, cx);
            }
        },
    );
    div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(
                    div()
                        .size(px(40.))
                        .flex_none()
                        .rounded_full()
                        .bg(theme.info_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_img(icons, Icon::Eye, false, theme.info_base, 20.)),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(theme::text_card_title())
                        .font_weight(FontWeight::SEMIBOLD)
                        .line_height(gpui::relative(1.3))
                        .text_color(theme.fg_base)
                        .child(model.title.clone()),
                ),
        )
        .child(handoff_facts(theme, icons, model, on_recheck))
        .child(open)
}

// ---------------------------------------------------------------------------
// The wait, and how it ended
// ---------------------------------------------------------------------------

/// "Waiting for the signing page…" — over the flow while the page has the
/// request: check it there, and sign it there.
///
/// `unreachable` (spec 082 RD13, W16): the person came back and the page
/// could not be reached — the card says so (`signerDown`), with Retry first
/// and Cancel; the request and its five-minute clock stay.
pub fn waiting_card(
    theme: &Theme,
    loc: &Loc,
    unreachable: bool,
    on_reopen: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    if unreachable {
        return card(theme)
            .items_center()
            .child(title(theme, loc.t("componentsUi.signing.handoffTitle")))
            .child(body(theme, loc.t("componentsUi.signing.signerDown")))
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_col()
                    .gap(px(FLOW_GAP_SM))
                    .child(vela_button(
                        "trusted-signer-retry",
                        ButtonVariant::Primary,
                        loc.t("connect.browser.retry"),
                        theme,
                        on_reopen,
                    ))
                    .child(vela_button(
                        "trusted-signer-cancel",
                        ButtonVariant::Secondary,
                        loc.t("common.cancel"),
                        theme,
                        on_cancel,
                    )),
            );
    }
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
            loc.t("componentsUi.signing.trustedSignerWaiting"),
        ))
        .child(body(
            theme,
            loc.t("componentsUi.signing.trustedSignerWaitingHint"),
        ))
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(FLOW_GAP_SM))
                .child(vela_button(
                    "trusted-signer-reopen",
                    ButtonVariant::Secondary,
                    loc.t("componentsUi.signing.trustedSignerReopen"),
                    theme,
                    on_reopen,
                ))
                .child(vela_button(
                    "trusted-signer-cancel",
                    ButtonVariant::Secondary,
                    loc.t("common.cancel"),
                    theme,
                    on_cancel,
                )),
        )
}

/// How the last attempt ended, in the sentence the card says: one of the four
/// endings, or — for a page never opened — why (spec 102: its check did not
/// admit it, or nothing here can reach the account's keys).
#[must_use]
pub fn ended_words(loc: &Loc, refusal: Refusal, not_opened: Option<&NotOpened>) -> SharedString {
    match (refusal, not_opened) {
        (Refusal::NotOpened, Some(NotOpened::Integrity(line))) => integrity::text(loc, line),
        (Refusal::NotOpened, Some(NotOpened::Venue(block))) => block_words(loc, block),
        (refusal, _) => loc.t(refusal.key()),
    }
}

/// Why the last attempt ended unsigned. Nothing was signed and nothing was
/// sent, whichever sentence it is.
pub fn ended_card(
    theme: &Theme,
    loc: &Loc,
    said: SharedString,
    on_done: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    card(theme)
        .items_center()
        .child(title(theme, loc.t("componentsUi.signing.handoffTitle")))
        .child(body(theme, said))
        .child(vela_button(
            "trusted-signer-done",
            ButtonVariant::Primary,
            loc.t("common.done"),
            theme,
            on_done,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::trusted_signer::integrity::Verdict;

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
            Refusal::NotOpened,
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

    /// A page never opened says WHY — the integrity line's own sentence, or
    /// the reason the keys are out of reach — never a generic ending.
    #[test]
    fn a_page_never_opened_says_why() {
        let loc = Loc::for_tag("en");
        let line = IntegrityLine::of(
            &Verdict::Refused {
                actual: "cd".repeat(32),
                expected: Vec::new(),
            },
            "",
            None,
        );
        let said = ended_words(&loc, Refusal::NotOpened, Some(&NotOpened::Integrity(line)));
        assert!(said.contains("cdcdcdcd"), "{said}");
        let block = VenueBlock::AppCannotReach {
            domain: "sign.example.com".to_owned(),
        };
        let said = ended_words(&loc, Refusal::NotOpened, Some(&NotOpened::Venue(block)));
        assert!(said.contains("sign.example.com"), "{said}");
        // Any other ending is its own sentence, whatever was last refused.
        assert_eq!(
            ended_words(&loc, Refusal::Closed, None),
            loc.t(Refusal::Closed.key())
        );
    }

    /// D4: the card names the page, the key, and the integrity line; Open is
    /// armed only when the line says the page may open — and never for keys
    /// nothing here can reach.
    #[test]
    fn the_hand_off_names_the_page_the_key_and_the_check() {
        let loc = Loc::for_tag("en");
        let handoff = Handoff {
            page: Some("https://sign.getvela.app/".to_owned()),
            block: None,
            key_name: None,
            key_place: KeyMethod::SecurityKey,
        };
        let admitted = IntegrityLine::of(
            &Verdict::Open,
            vela_core::trusted_signer::integrity::LAUNCH,
            Some(1_760_000_000_000),
        );
        let model = handoff_model(&loc, &handoff, Some(&admitted));
        assert_eq!(model.page.as_deref(), Some("sign.getvela.app"));
        assert!(
            model
                .key
                .contains(&*place_title(&loc, KeyMethod::SecurityKey))
        );
        assert!(!model.key.contains("{{"), "{}", model.key);
        assert!(model.opens);
        let (said, tone) = model
            .integrity
            .clone()
            .unwrap_or_else(|| unreachable!("a page has its line"));
        assert_eq!(tone, Tone::Success);
        assert!(said.contains(&admitted.version), "{said}");

        // Still checking: drawn, not armed.
        let checking = handoff_model(&loc, &handoff, Some(&IntegrityLine::checking()));
        assert!(!checking.opens && !checking.refused);
        // Refused: not armed, and "check again" is offered.
        let refused = IntegrityLine::of(
            &Verdict::CouldNotCheck(
                vela_core::trusted_signer::integrity::CheckFailure::Unreachable,
            ),
            "",
            None,
        );
        let model = handoff_model(&loc, &handoff, Some(&refused));
        assert!(!model.opens && model.refused);

        // The key's own name, when it has one, is what the card names.
        let named = Handoff {
            key_name: Some("YubiKey 5C".to_owned()),
            ..handoff.clone()
        };
        assert!(
            handoff_model(&loc, &named, Some(&admitted))
                .key
                .contains("YubiKey 5C")
        );

        // Out of reach: the reason, no page, no line, never armed.
        let stranded = Handoff {
            page: None,
            block: Some(VenueBlock::AppCannotReach {
                domain: "sign.example.com".to_owned(),
            }),
            key_name: None,
            key_place: KeyMethod::Platform,
        };
        let model = handoff_model(&loc, &stranded, Some(&admitted));
        assert!(!model.opens);
        assert_eq!(model.page, None);
        assert_eq!(model.integrity, None);
        assert!(
            model
                .blocked
                .is_some_and(|said| said.contains("sign.example.com"))
        );
    }

    #[test]
    fn a_pages_host_is_what_a_person_recognises() {
        assert_eq!(page_host("https://sign.getvela.app/"), "sign.getvela.app");
        assert_eq!(
            page_host("http://localhost:8140/clearsigning/"),
            "localhost:8140/clearsigning"
        );
        assert_eq!(page_host("https://x.test/?a#b"), "x.test");
    }

    /// Every word these cards can show resolves, in whatever language this
    /// build starts in. A card whose title is a dotted key is a person stuck
    /// at the one screen they cannot get past.
    #[test]
    fn every_card_has_its_words() {
        let loc = Loc::from_env();
        for key in [
            "componentsUi.signing.handoffTitle",
            "componentsUi.signing.handoffKey",
            "componentsUi.signing.openSigner",
            "componentsUi.signing.trustedSignerWaiting",
            "componentsUi.signing.trustedSignerWaitingHint",
            "componentsUi.signing.trustedSignerReopen",
            // Spec 082 RD13: the page could not be reached, and its Retry.
            "componentsUi.signing.signerDown",
            "connect.browser.retry",
            "common.cancel",
            "common.done",
        ] {
            let said = loc.t(key);
            assert_ne!(said.as_ref(), key, "`{key}` echoed");
            assert!(!said.is_empty(), "`{key}` resolved empty");
        }
    }
}
