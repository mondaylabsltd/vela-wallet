//! A signing page, drawn (spec 102) — the row Settings → Signing pages, the
//! choosers' "Use a trusted signing page" sheet and the account's "Where you
//! review and sign" rows all share, so a page reads the same wherever it is
//! chosen.
//!
//! A row answers the three questions that decide whether a page can be
//! trusted with an account: WHICH page it is (Vela's official signing page,
//! a self-hosted one by its domain, or the person's own label for it), WHOSE
//! keys it can use (R1: "Keys on getvela.app" — a page reaches only its own
//! domain's passkeys), and whether it is the build it claims to be (the
//! integrity line, R6).
//!
//! D6/D-19: the official page is NAMED "Vela's official signing page" — no
//! "Official" chip beside a host — and a self-hosted page the person did not
//! name reads "Self-hosted · <domain>". D7: the sheet's palette, one accent,
//! nothing tinted for its own sake.

use std::rc::Rc;

use gpui::{
    App, Div, FontWeight, InteractiveElement as _, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};

use vela_core::trusted_signer::launch::{IntegrityLine, IntegrityState};

use crate::hardware::{body, card, title};
use crate::icons::{Icon, IconCache};
use crate::loc::Loc;
use crate::signing::Tone;
use crate::signing::integrity;
use crate::signing::trusted_signer::{Click, page_host};
use crate::theme::{self, FLOW_GAP_SM, Theme};
use crate::ui::{ButtonVariant, vela_button};
use crate::wallet::components::icon_img;

/// One page, worded.
#[derive(Clone, Debug, PartialEq)]
pub struct PageRow {
    /// The page's normal address — what an event names it by.
    pub url: String,
    /// "Vela's official signing page", the person's label, else "Self-hosted
    /// · <domain>".
    pub name: SharedString,
    /// The page's address as a person recognises it (`sign.getvela.app`,
    /// `localhost:8140/clearsigning`), drawn under the name — `None` when the
    /// name already says it ("Self-hosted · sign.example.com"): a page is
    /// named once.
    pub host: Option<SharedString>,
    /// "Keys on getvela.app" — only where the keys' domain is not the page's
    /// own host (the official page, sign.getvela.app, reaches getvela.app's
    /// keys); a self-hosted page whose keys are its own host's says nothing
    /// the address has not.
    pub keys_on: Option<SharedString>,
    /// The official page — never renamed, never removed.
    pub official: bool,
    /// The integrity line and its tone.
    pub integrity: (SharedString, Tone),
    /// The last check refused the page: it will not open, and the row says
    /// why. A page still being checked, or one asking to be trusted, is not
    /// refused.
    pub refused: bool,
    /// "Trust this version" — set exactly while the check asks the person
    /// about a self-hosted page's build (`AskToTrust`).
    pub trust: Option<SharedString>,
}

/// A row's words: `name` the person's label (empty for none), `domain` the
/// domain whose keys it reaches, `line` its integrity line now.
#[must_use]
pub fn page_row(
    loc: &Loc,
    url: &str,
    name: &str,
    domain: &str,
    official: bool,
    line: &IntegrityLine,
) -> PageRow {
    let host = page_host(url);
    let named = !name.trim().is_empty() && !official;
    let title = if official {
        loc.t("settings.signing.pageOfficial")
    } else if named {
        SharedString::from(name.trim().to_owned())
    } else {
        loc.t_texts("settings.signing.pageSelfHosted", &[("domain", domain)])
    };
    // The address under the name, unless the name already says it — an
    // unnamed self-hosted page ("Self-hosted · sign.example.com") names its
    // address once.
    let host = (!title.contains(host.as_str())).then(|| SharedString::from(host));
    // Whose keys it reaches, only when that is not the page's own host.
    let keys_on = (!hostname(url).eq_ignore_ascii_case(domain.trim()))
        .then(|| loc.t_texts("settings.signing.keysOn", &[("domain", domain)]));
    let asks = line.state == IntegrityState::AskToTrust;
    PageRow {
        url: url.to_owned(),
        name: title,
        host,
        keys_on,
        official,
        integrity: (integrity::text(loc, line), integrity::tone(line)),
        refused: !line.opens && line.state != IntegrityState::Checking && !asks,
        trust: asks.then(|| loc.t("settings.signing.pageTrust")),
    }
}

/// A page address's host name alone — no scheme, port, path or user — as
/// the keys' domain is written (`sign.example.com`, `localhost`, `[::1]`).
fn hostname(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    match authority.find(']') {
        Some(end) if authority.starts_with('[') => &authority[..=end],
        _ => authority.split(':').next().unwrap_or_default(),
    }
}

/// The row's body: the page's mark, its name (and address under it), whose
/// keys it reaches, and its integrity line. Callers wrap it — a click, a
/// radio, rename and remove, and the line's own actions ([`line_actions`]).
pub fn page_row_body(theme: &Theme, icons: &mut IconCache, row: &PageRow) -> Div {
    let mut lines = div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .min_w(px(0.))
                .truncate()
                .text_size(theme::text_row_title())
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(theme.fg_base)
                .child(row.name.clone()),
        );
    if let Some(host) = &row.host {
        lines = lines.child(
            div()
                .font_family(theme::font_mono())
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .truncate()
                .child(host.clone()),
        );
    }
    if let Some(keys_on) = &row.keys_on {
        lines = lines.child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(icon_img(icons, Icon::Lock, false, theme.fg_subtle, 12.))
                .child(keys_on.clone()),
        );
    }
    // Two lines' room, unless the line's own answer follows it.
    lines = lines.child(integrity::row(
        theme,
        icons,
        row.integrity.0.clone(),
        row.integrity.1,
        row.trust.is_none() && !row.refused,
    ));
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .child(
            div()
                .size(px(32.))
                .flex_none()
                .rounded_full()
                .bg(theme.bg_sunken)
                .border_1()
                .border_color(theme.divider)
                .flex()
                .items_center()
                .justify_center()
                .child(icon_img(icons, Icon::Eye, false, theme.fg_muted, 16.)),
        )
        .child(lines)
}

/// The line's own actions, under a row's body (indented to its words): "Trust
/// this version" while a self-hosted page's check asks, "Try again" under a
/// refusal — each only when the caller binds it. `None` when the line asks
/// for nothing.
pub fn line_actions(
    theme: &Theme,
    icons: &mut IconCache,
    id: impl Into<gpui::ElementId> + Clone,
    row: &PageRow,
    try_again: SharedString,
    on_trust: Option<Click>,
    on_recheck: Option<Click>,
) -> Option<Div> {
    let action = |icons: &mut IconCache, icon: Icon, label: SharedString| {
        div()
            .flex()
            .items_center()
            .gap(px(6.))
            .cursor_pointer()
            .text_size(theme::text_row_sub())
            .font_weight(FontWeight::MEDIUM)
            .text_color(theme.accent)
            .child(icon_img(icons, icon, false, theme.accent, 12.))
            .child(label)
    };
    let id = id.into();
    let mut actions = div()
        .pl(px(44.))
        .pt(px(8.))
        .flex()
        .items_center()
        .gap(px(16.));
    let mut any = false;
    if let (Some(label), Some(on_trust)) = (row.trust.clone(), on_trust) {
        actions = actions.child(
            action(icons, Icon::Check, label)
                .id(gpui::ElementId::Name(format!("{id}-trust").into()))
                // A row that is itself a choice (the account's venue rows)
                // must not also be chosen by its line's action.
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    on_trust(event, window, cx);
                }),
        );
        any = true;
    }
    if let (true, Some(on_recheck)) = (row.refused, on_recheck) {
        actions = actions.child(
            action(icons, Icon::RefreshCw, try_again)
                .id(gpui::ElementId::Name(format!("{id}-recheck").into()))
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    on_recheck(event, window, cx);
                }),
        );
        any = true;
    }
    any.then_some(actions)
}

/// What picking a page does.
pub type PickPage = Rc<dyn Fn(String, &mut Window, &mut App)>;

/// "Use a trusted signing page" (spec 102, D6): Vela's official signing page
/// and the self-hosted ones this device keeps, each with whose keys it reaches
/// and its integrity line — a self-hosted build nobody decided about yet with
/// its "Trust this version" — a way to add one (the only way before a wallet
/// exists, when Settings is out of reach) and Close. A page whose check
/// refused it, or that still asks, cannot be picked: nothing would open.
#[allow(
    clippy::too_many_arguments,
    reason = "the rows, and what each of their controls does"
)]
pub fn own_page_sheet(
    theme: &Theme,
    icons: &mut IconCache,
    loc: &Loc,
    rows: &[PageRow],
    on_pick: PickPage,
    on_trust: PickPage,
    add: Div,
    on_close: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let mut list = div().w_full().flex().flex_col();
    for (index, row) in rows.iter().enumerate() {
        let pick = Rc::clone(&on_pick);
        let url = row.url.clone();
        let pickable = !row.refused && row.trust.is_none();
        // "Trust this version" under a self-hosted page's question, inside
        // the row and under its words — the row itself is not picked until
        // the question is answered.
        let trust = Rc::clone(&on_trust);
        let trusted_url = row.url.clone();
        let actions = line_actions(
            theme,
            icons,
            SharedString::from(format!("own-page-line-{index}")),
            row,
            loc.t("common.tryAgain"),
            Some(Box::new(move |_, window, cx| {
                trust(trusted_url.clone(), window, cx);
            })),
            None,
        );
        let entry = div()
            .id(("own-page", index))
            .w_full()
            .py(px(12.))
            .px(px(8.))
            .rounded(px(10.))
            .child(page_row_body(theme, icons, row))
            .children(actions);
        let entry = if pickable {
            entry
                .cursor_pointer()
                .hover(|style| style.bg(theme.bg_well))
                .on_click(move |_, window, cx| pick(url.clone(), window, cx))
        } else if row.refused {
            entry.opacity(0.55)
        } else {
            entry
        };
        let wrap = div().w_full().child(entry);
        list = list.child(if index > 0 {
            wrap.border_t_1().border_color(theme.divider)
        } else {
            wrap
        });
    }
    card(theme)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(FLOW_GAP_SM))
                .child(title(theme, loc.t("onboarding.create.signingPageTitle")))
                .child(body(theme, loc.t("onboarding.create.signingPageBody"))),
        )
        .child(list)
        .child(add)
        .child(vela_button(
            "own-page-close",
            ButtonVariant::Secondary,
            loc.t("onboarding.common.close"),
            theme,
            on_close,
        ))
}

/// The chosen page, as the choosers draw it above their three places: the
/// page, whose keys the new wallet's will be, and its integrity line — with
/// the line's own answers ("Trust this version" while a re-check asks about
/// a new build, "Try again" under a refusal: a page chosen while it passed
/// can be re-checked into either) and, while the page may still change, a
/// way back to Vela's own keys (`on_clear`). On the sheet's sunken panel
/// (D7), not a tinted one.
pub fn chosen_page_banner(
    theme: &Theme,
    icons: &mut IconCache,
    row: &PageRow,
    try_again: SharedString,
    on_clear: Option<Click>,
) -> Div {
    let (trust_url, again_url) = (row.url.clone(), row.url.clone());
    let actions = line_actions(
        theme,
        icons,
        "own-page-chosen-line",
        row,
        try_again,
        Some(Box::new(move |_, _, cx| {
            integrity::trust(&trust_url, cx);
        })),
        Some(Box::new(move |_, _, cx| {
            integrity::recheck(&again_url, cx);
        })),
    );
    div()
        .w_full()
        .flex()
        .items_start()
        .gap(px(12.))
        .p(px(14.))
        .rounded(px(theme::RADIUS_FIELD))
        .bg(theme.bg_sunken)
        .border_1()
        .border_color(theme.divider)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .child(page_row_body(theme, icons, row))
                .children(actions),
        )
        .children(on_clear.map(|on_clear| {
            div()
                .id("own-page-clear")
                .flex_none()
                .p(px(4.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|style| style.bg(theme.bg_raised))
                .child(icon_img(icons, Icon::X, false, theme.fg_muted, 14.))
                .on_click(on_clear)
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::trusted_signer::integrity::{CheckFailure, Verdict};

    /// A row says which page it is, whose keys it reaches and how its check
    /// went; a refused page is marked so nothing picks it.
    #[test]
    fn a_row_names_the_page_its_domain_and_its_check() {
        let loc = Loc::for_tag("en");
        let official = page_row(
            &loc,
            "https://sign.getvela.app/",
            "",
            "getvela.app",
            true,
            &IntegrityLine::checking(),
        );
        // D-19: the official page is NAMED, its address under the name.
        assert_eq!(official.name, loc.t("settings.signing.pageOfficial"));
        assert_eq!(official.host.as_deref(), Some("sign.getvela.app"));
        // Its keys are getvela.app's, not its host's: said.
        assert!(
            official
                .keys_on
                .as_ref()
                .is_some_and(|said| said.contains("getvela.app"))
        );
        assert!(official.official);
        assert!(!official.refused, "checking is not refused");
        assert_eq!(official.trust, None);

        let refused =
            IntegrityLine::of(&Verdict::CouldNotCheck(CheckFailure::Unreachable), "", None);
        let own = page_row(
            &loc,
            "http://localhost:8140/clearsigning/",
            "Desk",
            "localhost",
            false,
            &refused,
        );
        assert_eq!(own.name.as_ref(), "Desk");
        assert_eq!(own.host.as_deref(), Some("localhost:8140/clearsigning"));
        // Its keys are its own host's (`localhost`): the address says so.
        assert_eq!(own.keys_on, None);
        assert!(own.refused);
        assert_eq!(own.integrity.1, Tone::Danger);
    }

    /// D6: a self-hosted page the person did not name reads "Self-hosted ·
    /// <domain>" — in every language, never "my own" — and its domain is
    /// said once when the page sits at that domain's root.
    #[test]
    fn an_unnamed_self_hosted_page_says_so_with_its_domain() {
        for (tag, loc) in Loc::every_language() {
            let row = page_row(
                &loc,
                "https://sign.example.com/",
                "",
                "sign.example.com",
                false,
                &IntegrityLine::checking(),
            );
            assert!(row.name.contains("sign.example.com"), "{tag}: {}", row.name);
            assert!(!row.name.contains("{{"), "{tag}: {}", row.name);
            // Named once: no address under a name that says it, and no
            // "Keys on" its own host.
            assert_eq!(row.host, None, "{tag}");
            assert_eq!(row.keys_on, None, "{tag}");
        }
        let zh = Loc::for_tag("zh");
        let row = page_row(
            &zh,
            "https://sign.example.com/",
            "",
            "sign.example.com",
            false,
            &IntegrityLine::checking(),
        );
        assert_eq!(row.name.as_ref(), "自己部署的签名页 · sign.example.com");
        let official = page_row(
            &zh,
            "https://sign.getvela.app/",
            "",
            "getvela.app",
            true,
            &IntegrityLine::checking(),
        );
        assert_eq!(official.name.as_ref(), "Vela 官方签名页");
    }

    /// A page is named once (polish 3): its address goes under its name only
    /// when the name does not already say it, and "Keys on …" only when the
    /// keys' domain is not the page's own host.
    #[test]
    fn a_page_is_named_once() {
        let loc = Loc::for_tag("en");
        let line = IntegrityLine::checking();
        // Under a path, the address says more than the name.
        let pathed = page_row(
            &loc,
            "https://sign.example.com/vela/",
            "",
            "sign.example.com",
            false,
            &line,
        );
        assert_eq!(pathed.host.as_deref(), Some("sign.example.com/vela"));
        assert_eq!(pathed.keys_on, None);
        // A page whose keys live on its parent domain says whose they are.
        let parent = page_row(
            &loc,
            "https://sign.example.com/",
            "Home",
            "example.com",
            false,
            &line,
        );
        assert_eq!(parent.host.as_deref(), Some("sign.example.com"));
        assert!(
            parent
                .keys_on
                .is_some_and(|said| said.contains("example.com"))
        );
        // The person's own name that IS the address is not given it again.
        let named = page_row(
            &loc,
            "https://sign.example.com:8443/",
            "sign.example.com:8443",
            "sign.example.com",
            false,
            &line,
        );
        assert_eq!(named.host, None);
        assert_eq!(named.keys_on, None);
        assert_eq!(hostname("http://user@[::1]:8140/x"), "[::1]");
        assert_eq!(hostname("https://Sign.Example.com"), "Sign.Example.com");
    }

    /// A self-hosted build nobody decided about asks — "Trust this version"
    /// is offered, and the row is neither refused nor pickable as it stands.
    #[test]
    fn a_new_self_hosted_version_asks_to_be_trusted() {
        let loc = Loc::for_tag("en");
        let asks = IntegrityLine::of(
            &Verdict::AskToTrust {
                actual: "3f".repeat(32),
            },
            "",
            None,
        );
        let row = page_row(
            &loc,
            "https://sign.example.com/",
            "",
            "sign.example.com",
            false,
            &asks,
        );
        assert_eq!(row.trust, Some(loc.t("settings.signing.pageTrust")));
        assert!(!row.refused);
        assert!(row.integrity.0.contains("3f3f3f3f"), "{}", row.integrity.0);
    }
}
