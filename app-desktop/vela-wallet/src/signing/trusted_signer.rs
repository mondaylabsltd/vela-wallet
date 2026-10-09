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
use vela_core::signing_venue::{KeyLabel, VenueBlock, place_title_key};
use vela_core::trusted_signer::launch::{IntegrityLine, IntegrityState};

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
    /// "Confirm with YubiKey 5C", "Confirm with Phone or tablet" — the
    /// plan's key label (D-17).
    pub key: SharedString,
    /// Where that key lives, for its mark.
    pub place: KeyMethod,
    /// The fee the person chose on the sheet, and its speed — one quiet row
    /// under the key (core round 5). `None` when the sheet has no settled fee
    /// for the speed in force (a message, a fee still measuring), or when the
    /// screen already shows the fee above the card (the send's own confirm).
    pub fee: Option<HandoffFeeRow>,
    /// The integrity line and its tone; `None` when no page is involved.
    pub integrity: Option<(SharedString, Tone)>,
    /// Why nothing on this device can sign for the account (R1).
    pub blocked: Option<SharedString>,
    /// May the page be opened? The core's word (`IntegrityLine::opens`), and
    /// never when the keys are out of reach.
    pub opens: bool,
    /// The last check refused the page: "Check again" is offered.
    pub refused: bool,
    /// "Trust this version" — set exactly when the check asks the person
    /// about a self-hosted page's build (`AskToTrust`).
    pub trust_label: Option<SharedString>,
    /// The button: "Continue to signing page".
    pub open_label: SharedString,
    /// "Try again", under a refused line.
    pub recheck_label: SharedString,
}

/// The hand-off's fee row, worded: "Network fee", the figure as the sheet's
/// folded fee row draws it ("0.0012 USDC · ≈$0.01"), and — where the network
/// offers more than one — the speed it is priced at ("Standard"), on its own
/// quiet line so a narrow column never breaks the figure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HandoffFeeRow {
    pub label: SharedString,
    pub figure: SharedString,
    pub speed: Option<SharedString>,
}

/// The place a key label names, for its mark: the place whose title key it
/// carries (the core always sets one).
#[must_use]
pub fn place_of_label(label: &KeyLabel) -> KeyMethod {
    KeyMethod::ALL
        .into_iter()
        .find(|place| place_title_key(place.name()) == label.place_key)
        .unwrap_or(KeyMethod::Platform)
}

/// The words of a venue refusal (R1, and the web's), resolved once and
/// filled per refusal — the core names the key (`VenueBlock::key`), this
/// fills its domains. Kept as templates so a screen whose strings are
/// resolved ahead (the send flow's) can say any refusal without a `Loc`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VenueBlockWords {
    app: String,
    page: String,
    web: String,
}

impl VenueBlockWords {
    #[must_use]
    pub fn resolve(loc: &Loc) -> Self {
        let template = |block: VenueBlock| loc.t(block.key()).to_string();
        Self {
            app: template(VenueBlock::AppCannotReach {
                domain: String::new(),
            }),
            page: template(VenueBlock::PageOnOtherDomain {
                page_domain: String::new(),
                domain: String::new(),
            }),
            web: template(VenueBlock::NotOnWeb),
        }
    }

    /// The sentence for `block`, its domains filled.
    #[must_use]
    pub fn say(&self, block: &VenueBlock) -> SharedString {
        SharedString::from(match block {
            VenueBlock::AppCannotReach { domain } => {
                crate::signing::fill(&self.app, &[("domain", domain)])
            }
            VenueBlock::PageOnOtherDomain {
                page_domain,
                domain,
            } => crate::signing::fill(
                &self.page,
                &[("pageDomain", page_domain), ("domain", domain)],
            ),
            VenueBlock::NotOnWeb => self.web.clone(),
        })
    }
}

/// R1's reason, in the corpus's sentence.
#[must_use]
pub fn block_words(loc: &Loc, block: &VenueBlock) -> SharedString {
    VenueBlockWords::resolve(loc).say(block)
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
    let key = handoff.key_label.text(|key| loc.t(key).to_string());
    let blocked = handoff.block.as_ref().map(|block| block_words(loc, block));
    let integrity = line
        .filter(|_| blocked.is_none())
        .map(|line| (integrity::text(loc, line), integrity::tone(line)));
    let opens = blocked.is_none() && line.is_some_and(|line| line.opens);
    let asks =
        blocked.is_none() && line.is_some_and(|line| line.state == IntegrityState::AskToTrust);
    HandoffModel {
        title: loc.t("componentsUi.signing.handoffTitle"),
        page: handoff
            .page
            .as_deref()
            .filter(|_| blocked.is_none())
            .map(|page| SharedString::from(page_host(page))),
        key: loc.t_texts("componentsUi.signing.handoffKey", &[("key", key.as_str())]),
        place: place_of_label(&handoff.key_label),
        fee: None,
        refused: line
            .is_some_and(|line| !line.opens && line.state != IntegrityState::Checking && !asks),
        trust_label: asks.then(|| loc.t("settings.signing.pageTrust")),
        integrity,
        blocked,
        opens,
        open_label: loc.t("componentsUi.signing.openSigner"),
        recheck_label: loc.t("common.tryAgain"),
    }
}

/// The hand-off's fee row: `label`, the sheet's own folded fee line for the
/// quote in force (`figure`, drawn by the sheet's formatter) and the speed the
/// core's `HandoffFee` names, if it names one.
#[must_use]
pub fn handoff_fee_row(
    loc: &Loc,
    label: SharedString,
    fee: &vela_core::app::sign_confirm::HandoffFee,
    figure: &str,
) -> HandoffFeeRow {
    HandoffFeeRow {
        label,
        figure: SharedString::from(figure.to_owned()),
        speed: fee.tier_key.as_deref().map(|tier| loc.t(tier)),
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

/// A quiet text action under a line — "Try again", "Trust this version" —
/// in the one accent, as the sheet's own inline actions are.
fn line_action(
    theme: &Theme,
    icons: &mut IconCache,
    id: &'static str,
    icon: Icon,
    label: SharedString,
    on_click: Click,
) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .cursor_pointer()
        .text_size(theme::text_row_sub())
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.accent)
        .child(icon_img(icons, icon, false, theme.accent, 12.))
        .child(label)
        .on_click(on_click)
}

/// The card's facts, on one sunken panel: where the page is, the key, the
/// fee the person chose, and the integrity line — the things that make
/// "trusted" mean something.
pub fn handoff_facts(
    theme: &Theme,
    icons: &mut IconCache,
    model: &HandoffModel,
    on_recheck: Option<Click>,
    on_trust: Option<Click>,
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
    if let Some(fee) = &model.fee {
        let value = div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .items_end()
            .gap(px(2.))
            .child(
                div()
                    .text_right()
                    .text_color(theme.fg_base)
                    .child(fee.figure.clone()),
            )
            .children(fee.speed.clone().map(|speed| {
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_muted)
                    .child(speed)
            }));
        rows.push(fact(
            icon_img(icons, Icon::Coins, false, theme.fg_muted, 16.).into_any_element(),
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap(px(12.))
                .text_size(theme::text_row_sub())
                .child(
                    div()
                        .flex_none()
                        .text_color(theme.fg_muted)
                        .child(fee.label.clone()),
                )
                .child(value),
        ));
    }
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
                .text_color(integrity::words_ink(theme, *tone))
                .child(said.clone()),
        );
        if let (Some(on_trust), Some(label)) = (on_trust, model.trust_label.clone()) {
            words = words.child(line_action(
                theme,
                icons,
                "handoff-trust",
                Icon::Check,
                label,
                on_trust,
            ));
        }
        if let Some(on_recheck) = on_recheck.filter(|_| model.refused) {
            words = words.child(line_action(
                theme,
                icons,
                "handoff-recheck",
                Icon::RefreshCw,
                model.recheck_label.clone(),
                on_recheck,
            ));
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
///
/// D7: the sheet's own palette — the eye sits in a neutral well, not a
/// tinted chip; the one accent is the pill Open.
pub fn handoff_card(
    theme: &Theme,
    icons: &mut IconCache,
    model: &HandoffModel,
    on_open: Option<Click>,
    on_recheck: Option<Click>,
    on_trust: Option<Click>,
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
                        .bg(theme.bg_well)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_img(icons, Icon::Eye, false, theme.fg_muted, 20.)),
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
        .child(handoff_facts(theme, icons, model, on_recheck, on_trust))
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
    heading: SharedString,
    unreachable: bool,
    on_reopen: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    if unreachable {
        return card(theme)
            .items_center()
            .child(title(theme, heading))
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
/// admit it).
#[must_use]
pub fn ended_words(loc: &Loc, refusal: Refusal, not_opened: Option<&NotOpened>) -> SharedString {
    match (refusal, not_opened) {
        (Refusal::NotOpened, Some(NotOpened::Integrity(line))) => integrity::text(loc, line),
        (refusal, _) => loc.t(refusal.key()),
    }
}

/// Why the last attempt ended unsigned. Nothing was signed and nothing was
/// sent, whichever sentence it is.
pub fn ended_card(
    theme: &Theme,
    loc: &Loc,
    heading: SharedString,
    said: SharedString,
    on_done: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    card(theme)
        .items_center()
        .child(title(theme, heading))
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

    /// A page never opened says WHY — the integrity line's own sentence —
    /// never a generic ending.
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
            key_label: KeyLabel::of(None, "Savings", "security_key"),
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
                .contains(&*loc.t("onboarding.create.methodSecurityKeyTitle"))
        );
        assert_eq!(model.place, KeyMethod::SecurityKey);
        assert_eq!(model.fee, None, "a fee row is the sheet's to add");
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
        assert_eq!(model.trust_label, None);
        // A self-hosted build nobody decided about: not armed, not "refused",
        // and "Trust this version" is offered.
        let asks = IntegrityLine::of(
            &Verdict::AskToTrust {
                actual: "3f".repeat(32),
            },
            "",
            None,
        );
        let model = handoff_model(&loc, &handoff, Some(&asks));
        assert!(!model.opens && !model.refused);
        assert_eq!(model.trust_label, Some(loc.t("settings.signing.pageTrust")));

        // D-17: the key's own name, when the person gave it one, is what the
        // card names; a key named after the wallet is named by its place.
        let named = Handoff {
            key_label: KeyLabel::of(Some("YubiKey 5C"), "Savings", "security_key"),
            ..handoff.clone()
        };
        assert!(
            handoff_model(&loc, &named, Some(&admitted))
                .key
                .contains("YubiKey 5C")
        );
        let wallets = Handoff {
            key_label: KeyLabel::of(Some("Savings"), "Savings", "hybrid"),
            ..handoff.clone()
        };
        let model = handoff_model(&loc, &wallets, Some(&admitted));
        assert!(!model.key.contains("Savings"), "{}", model.key);
        assert!(
            model
                .key
                .contains(&*loc.t("onboarding.create.methodHybridTitle"))
        );
        assert_eq!(model.place, KeyMethod::Hybrid);

        // Out of reach: the reason, no page, no line, never armed.
        let stranded = Handoff {
            page: None,
            block: Some(VenueBlock::AppCannotReach {
                domain: "sign.example.com".to_owned(),
            }),
            key_label: KeyLabel::default(),
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

    /// Every venue refusal — R1's two and the web's — says its reason with
    /// its domains, in every language, never a key or a placeholder.
    #[test]
    fn every_venue_refusal_is_said_with_its_domains() {
        let blocks = [
            VenueBlock::AppCannotReach {
                domain: "sign.example.com".to_owned(),
            },
            VenueBlock::PageOnOtherDomain {
                page_domain: "pages.example.org".to_owned(),
                domain: "getvela.app".to_owned(),
            },
            VenueBlock::NotOnWeb,
        ];
        for (tag, loc) in Loc::every_language() {
            let words = VenueBlockWords::resolve(&loc);
            for block in &blocks {
                let said = words.say(block);
                assert_ne!(said.as_ref(), block.key(), "{tag}: echoed");
                assert!(!said.contains("{{"), "{tag}: {said}");
                assert_eq!(said, block_words(&loc, block), "{tag}");
            }
            assert!(words.say(&blocks[0]).contains("sign.example.com"), "{tag}");
            let page = words.say(&blocks[1]);
            assert!(
                page.contains("pages.example.org") && page.contains("getvela.app"),
                "{tag}"
            );
        }
    }

    /// Core round 5: the fee row restates the speed by its name, and only
    /// where the network offers more than one.
    #[test]
    fn the_fee_row_names_the_speed_it_was_priced_at() {
        use vela_core::app::fee_policy::FeeEstimateView;
        use vela_core::app::fee_policy::FeeTier;
        use vela_core::app::sign_confirm::HandoffFee;
        let loc = Loc::for_tag("en");
        let estimate = FeeEstimateView {
            chain_id: 100,
            total_wei: "1".to_owned(),
            max_fee_per_gas: "1".to_owned(),
            network_fee_per_gas: "1".to_owned(),
            relayer_fee_per_gas: "0".to_owned(),
            bundler_gas_price: "1".to_owned(),
            in_band_gas_basis: "0".to_owned(),
            effective_gas_price: None,
            max_gas_price: None,
            total_gas: "1".to_owned(),
            deployed: true,
            tier: FeeTier::Standard,
            quoted: true,
            fee_asset: vela_core::app::fee_policy::FeeAssetView::Native,
            fee_recipient: None,
        };
        let fee = |tier: Option<FeeTier>| HandoffFee {
            fee: estimate.clone(),
            tier,
            tier_key: tier.map(|_| "send.gasTier.standard".to_owned()),
        };
        let figure = "0.0012 USDC · ≈$0.01";
        let label = loc.t("componentsUi.gas.networkFee");
        let row = handoff_fee_row(&loc, label.clone(), &fee(Some(FeeTier::Standard)), figure);
        assert_eq!(row.label, label);
        assert_eq!(row.figure.as_ref(), figure);
        assert_eq!(row.speed, Some(loc.t("send.gasTier.standard")));
        assert_eq!(handoff_fee_row(&loc, label, &fee(None), figure).speed, None);
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
