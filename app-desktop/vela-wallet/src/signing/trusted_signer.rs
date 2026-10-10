//! The trusted signing page's cards (specs 071, 075; spec 102 D4): the
//! hand-off before the page opens, the wait while it has the request, and how
//! an attempt ended.
//!
//! **The hand-off (D4).** When an account reviews and signs on a trusted page,
//! the app does not draw the request a second time — the page is the
//! authority, and two previews of one transaction left people asking which
//! one to believe. The card says only what the page cannot: the key the
//! person will confirm with (a row, 「确认方式 | 手机或平板」, as the sheet's
//! "Signing account" row is drawn), where the page is, and one integrity line
//! that backs the word "trusted" ("Version 0ba8ee8c · matches Vela's
//! published build list · checked 14:32", or why the page will not open).
//! Open is enabled only when that line says the page may open. The fee is
//! not the card's: each screen that hands off (the dApp sheet, the send's
//! confirm) keeps its own fee and speed rows above it, so the fee is said
//! once per screen.
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
    /// "Review and sign on a trusted signing page".
    pub title: SharedString,
    /// The page's host — WHAT is trusted, named. `None` when nothing can
    /// open (the account's keys are out of reach).
    pub page: Option<SharedString>,
    /// 「确认方式 | YubiKey 5C」, "Confirm with | Phone or tablet" — the plan's
    /// key label (D-17), as a row.
    pub key: KeyRow,
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

/// A key row, worded (spec 102 integration): 「确认方式 | 手机或平板」 /
/// "Confirm with | YubiKey 5C" — or, while a ceremony on a page makes the
/// key, 「新钥匙存在 | 这台设备」 / "New key on | This device". The label is
/// the core's (`KeyLabel::label_key`, the page's own `field.confirmWith` /
/// `field.keyOn`); the value the key's own name, else its place's title. A
/// row, not a sentence, so no language has to inflect a place inside one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyRow {
    pub label: SharedString,
    pub value: SharedString,
    /// Where the key lives, for its mark.
    pub place: KeyMethod,
}

impl KeyRow {
    #[must_use]
    pub fn of(loc: &Loc, label: &KeyLabel) -> Self {
        Self {
            label: loc.t(&label.label_key),
            value: SharedString::from(label.text(|key| loc.t(key).to_string())),
            place: place_of_label(label),
        }
    }
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
/// filled per refusal — the whole sentence is the core's: which line
/// (`VenueBlock::key`) and which fact fills which placeholder
/// (`VenueBlock::vars`). Kept as templates so a screen whose strings are
/// resolved ahead (the send flow's) can say any refusal without a `Loc`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VenueBlockWords {
    /// Each line a refusal can say, by its corpus key.
    lines: Vec<(&'static str, String)>,
}

impl VenueBlockWords {
    #[must_use]
    pub fn resolve(loc: &Loc) -> Self {
        // One refusal of each kind, only to learn the keys their lines are
        // under — the values come from the refusal being said.
        let kinds = [
            VenueBlock::AppCannotReach {
                domain: String::new(),
            },
            VenueBlock::PageOnOtherDomain {
                page_domain: String::new(),
                domain: String::new(),
            },
            VenueBlock::NotOnWeb,
        ];
        Self {
            lines: kinds
                .iter()
                .map(|block| (block.key(), loc.t(block.key()).to_string()))
                .collect(),
        }
    }

    /// The sentence for `block`: its line, with the values the core names
    /// for it.
    #[must_use]
    pub fn say(&self, block: &VenueBlock) -> SharedString {
        let key = block.key();
        let template = self
            .lines
            .iter()
            .find(|(line, _)| *line == key)
            .map_or(key, |(_, template)| template.as_str());
        let vars = block.vars();
        let vars: Vec<(&str, &str)> = vars
            .iter()
            .map(|(name, value)| (*name, value.as_str()))
            .collect();
        SharedString::from(crate::signing::fill(template, &vars))
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
        key: KeyRow::of(loc, &handoff.key_label),
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

/// The key row, drawn as the sheet's own label|value rows are (its
/// 「签名账户 | 名字」 / "Signing account | name" row): the label in the quiet
/// ink on the left, the key on the right in the body ink, its place's mark
/// beside it where that row has the account's identicon.
pub fn key_row(theme: &Theme, icons: &mut IconCache, row: &KeyRow) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .child(
            div()
                .flex_none()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(row.label.clone()),
        )
        .child(
            div()
                .min_w(px(0.))
                .flex()
                .items_center()
                .gap(px(8.))
                .child(div().flex_none().child(icon_img(
                    icons,
                    place_icon(row.place),
                    false,
                    theme.fg_muted,
                    16.,
                )))
                .child(
                    div()
                        .min_w(px(0.))
                        .truncate()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(row.value.clone()),
                ),
        )
}

/// What the card says under its title: the key row, then — on one sunken
/// panel — where the page is and the integrity line, the two things that
/// make "trusted" mean something. The send's confirm draws it under its own
/// title; the hand-off card under its eye.
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
    if let Some(blocked) = &model.blocked {
        rows.push(fact(
            icon_img(icons, Icon::CircleAlert, false, theme.error_base, 16.).into_any_element(),
            div()
                .text_size(theme::text_row_sub())
                .line_height(gpui::relative(integrity::LINE_HEIGHT))
                .text_color(theme.error_base)
                .child(crate::ui::prose(blocked.clone())),
        ));
    }
    if let Some((said, tone)) = &model.integrity {
        let colour = integrity::ink(theme, *tone);
        let mut words = div().flex().flex_col().gap(px(6.)).child(integrity::words(
            theme,
            said.clone(),
            *tone,
            model.trust_label.is_none() && !model.refused,
        ));
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
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(key_row(theme, icons, &model.key))
        .child(panel)
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
/// `key` is a key ceremony's row (spec 102 integration, the core's
/// `Ceremony::key_label`): 「新钥匙存在 | 手机或平板」 while a key is made on
/// the page, 「确认方式 | 这台设备」 while one signs in or proves — so the
/// person knows which device to reach for before the browser asks. A
/// ceremony has no request to check, so its card says what is being done
/// there instead — `heading`, its own title ("Create a key on the
/// signing page") — never "check the request and sign it". `None` for a
/// signature, whose key the hand-off card already named.
///
/// `unreachable` (spec 082 RD13, W16): the person came back and the page
/// could not be reached — the card says so (`signerDown`), with Retry first
/// and Cancel; the request and its five-minute clock stay.
#[allow(
    clippy::too_many_arguments,
    reason = "the card's words, its key row, and what each of its buttons does"
)]
pub fn waiting_card(
    theme: &Theme,
    icons: &mut IconCache,
    loc: &Loc,
    heading: SharedString,
    key: Option<&KeyRow>,
    unreachable: bool,
    on_reopen: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let key = key.map(|key| {
        div()
            .w_full()
            .py(px(12.))
            .border_t_1()
            .border_b_1()
            .border_color(theme.divider)
            .child(key_row(theme, icons, key))
    });
    if unreachable {
        return card(theme)
            .items_center()
            .child(title(theme, heading))
            .child(body(theme, loc.t("componentsUi.signing.signerDown")))
            .children(key)
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
            if key.is_some() {
                heading
            } else {
                loc.t("componentsUi.signing.trustedSignerWaitingHint")
            },
        ))
        .children(key)
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
        (Refusal::NotOpened, Some(NotOpened::Integrity { line, .. })) => integrity::text(loc, line),
        (refusal, _) => loc.t(refusal.key()),
    }
}

/// The page whose check asked about its build (`AskToTrust`), when that is
/// why the page was never opened — the ended card says the question, and
/// answers it beside it (polish 9).
#[must_use]
pub fn asked_page(not_opened: Option<&NotOpened>) -> Option<String> {
    match not_opened? {
        NotOpened::Integrity { line, page } => {
            (line.state == IntegrityState::AskToTrust).then(|| page.clone())
        }
    }
}

/// Why the last attempt ended unsigned. Nothing was signed and nothing was
/// sent, whichever sentence it is. `on_trust`: the sentence is a self-hosted
/// page's "Trust it on this device?" ([`asked_page`]) — its answer, "Trust
/// this version", under it, as on every surface that asks.
pub fn ended_card(
    theme: &Theme,
    icons: &mut IconCache,
    loc: &Loc,
    heading: SharedString,
    said: SharedString,
    on_trust: Option<Click>,
    on_done: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    card(theme)
        .items_center()
        .child(title(theme, heading))
        .child(body(theme, said))
        .children(on_trust.map(|on_trust| {
            line_action(
                theme,
                icons,
                "trusted-signer-trust",
                Icon::Check,
                loc.t("settings.signing.pageTrust"),
                on_trust,
            )
        }))
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
        let page = "https://sign.example.com/".to_owned();
        let refused = NotOpened::Integrity {
            line,
            page: page.clone(),
        };
        let said = ended_words(&loc, Refusal::NotOpened, Some(&refused));
        assert!(said.contains("cdcdcdcd"), "{said}");
        // A refusal is not a question: nothing to trust.
        assert_eq!(asked_page(Some(&refused)), None);
        // A self-hosted build nobody decided about is: answered on the card.
        let asks = NotOpened::Integrity {
            line: IntegrityLine::of(
                &Verdict::AskToTrust {
                    actual: "3f".repeat(32),
                },
                "",
                None,
            ),
            page: page.clone(),
        };
        assert_eq!(asked_page(Some(&asks)), Some(page));
        assert_eq!(asked_page(None), None);
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
        // A row: "Confirm with | USB security key", never a sentence.
        assert_eq!(
            model.key.label,
            loc.t("componentsUi.signing.confirmWithLabel")
        );
        assert_eq!(
            model.key.value,
            loc.t("onboarding.create.methodSecurityKeyTitle")
        );
        assert_eq!(model.key.place, KeyMethod::SecurityKey);
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
        assert_eq!(
            handoff_model(&loc, &named, Some(&admitted))
                .key
                .value
                .as_ref(),
            "YubiKey 5C"
        );
        let wallets = Handoff {
            key_label: KeyLabel::of(Some("Savings"), "Savings", "hybrid"),
            ..handoff.clone()
        };
        let model = handoff_model(&loc, &wallets, Some(&admitted));
        assert_eq!(
            model.key.value,
            loc.t("onboarding.create.methodHybridTitle")
        );
        assert_eq!(model.key.place, KeyMethod::Hybrid);

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

    /// The key row reads as the signing page's own field does — the label
    /// and the place, no spaces stitched around a CJK place — and a key
    /// ceremony's row is the core's: "New key on" while a key is made, "Confirm
    /// with" while one signs in or proves, by its place, never a name.
    #[test]
    fn the_key_row_is_a_label_and_a_value() {
        use vela_core::trusted_signer::ceremony::Ceremony;
        let zh = Loc::for_tag("zh");
        let row = KeyRow::of(&zh, &KeyLabel::of(None, "储蓄", "hybrid"));
        assert_eq!(row.label.as_ref(), "确认方式");
        assert_eq!(row.value.as_ref(), "手机或平板");

        let create = Ceremony::RegisterPasskey {
            name: "储蓄".to_owned(),
            exclude_credential_ids: Vec::new(),
            method: KeyMethod::Hybrid,
        };
        let row = KeyRow::of(&zh, &create.key_label());
        assert_eq!(row.label.as_ref(), "新钥匙存在");
        assert_eq!(row.value.as_ref(), "手机或平板");
        assert_eq!(row.place, KeyMethod::Hybrid);

        let sign_in = Ceremony::AuthenticatePasskey {
            method: KeyMethod::SecurityKey,
        };
        let en = Loc::for_tag("en");
        let row = KeyRow::of(&en, &sign_in.key_label());
        assert_eq!(row.label, en.t("componentsUi.signing.confirmWithLabel"));
        assert_eq!(row.value, en.t("onboarding.create.methodSecurityKeyTitle"));
        let label = create.key_label();
        for (tag, loc) in Loc::every_language() {
            let row = KeyRow::of(&loc, &label);
            assert_ne!(row.label.as_ref(), label.label_key, "{tag}: echoed");
            assert_ne!(row.value.as_ref(), label.place_key, "{tag}: echoed");
            assert!(!row.label.is_empty() && !row.value.is_empty(), "{tag}");
        }
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
            // The key row's two labels (spec 102 integration).
            "componentsUi.signing.confirmWithLabel",
            "componentsUi.signing.newKeyOnLabel",
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
