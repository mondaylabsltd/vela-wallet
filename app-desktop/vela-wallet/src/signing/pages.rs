//! A signing page, drawn (spec 102) — the row Settings → Signing pages, the
//! choosers' "Use my own signing page" sheet and the account's "Where you
//! review and sign" rows all share, so a page reads the same wherever it is
//! chosen.
//!
//! A row answers the three questions that decide whether a page can be
//! trusted with an account: WHERE it is (its host, or the person's label over
//! it), WHOSE keys it can use (R1: "Keys on getvela.app" — a page reaches only
//! its own domain's passkeys), and whether it is the build it claims to be
//! (the integrity line, R6).

use std::rc::Rc;

use gpui::{
    App, Div, FontWeight, InteractiveElement as _, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, px,
};

use vela_core::trusted_signer::launch::IntegrityLine;

use crate::hardware::{body, card, title};
use crate::icons::{Icon, IconCache};
use crate::loc::Loc;
use crate::signing::Tone;
use crate::signing::integrity;
use crate::signing::trusted_signer::page_host;
use crate::theme::{self, FLOW_GAP_SM, Theme};
use crate::ui::{ButtonVariant, vela_button};
use crate::wallet::components::icon_img;

/// One page, worded.
#[derive(Clone, Debug, PartialEq)]
pub struct PageRow {
    /// The page's normal address — what an event names it by.
    pub url: String,
    /// The person's label, else the host.
    pub name: SharedString,
    /// The host, drawn under a label (`None` when the name IS the host).
    pub host: Option<SharedString>,
    /// "Keys on getvela.app".
    pub keys_on: SharedString,
    /// "Official", on the official page.
    pub official: Option<SharedString>,
    /// The integrity line and its tone.
    pub integrity: (SharedString, Tone),
    /// The last check refused the page: it will not open, and the row says
    /// why. A page still being checked is not refused.
    pub refused: bool,
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
    let named = !name.trim().is_empty();
    PageRow {
        url: url.to_owned(),
        name: SharedString::from(if named {
            name.trim().to_owned()
        } else {
            host.clone()
        }),
        host: named.then(|| SharedString::from(host)),
        keys_on: loc.t_texts("settings.signing.keysOn", &[("domain", domain)]),
        official: official.then(|| loc.t("settings.signing.pageOfficial")),
        integrity: (integrity::text(loc, line), integrity::tone(line)),
        refused: !line.opens
            && line.state != vela_core::trusted_signer::launch::IntegrityState::Checking,
    }
}

/// The row's body: the page's mark, its name (and host under a label), whose
/// keys it reaches, and its integrity line. Callers wrap it — a click, a
/// radio, rename and remove.
pub fn page_row_body(theme: &Theme, icons: &mut IconCache, row: &PageRow) -> Div {
    let mut head = div().flex().items_center().gap(px(8.)).min_w(px(0.)).child(
        div()
            .min_w(px(0.))
            .truncate()
            .text_size(theme::text_row_title())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.fg_base)
            .child(row.name.clone()),
    );
    if let Some(official) = &row.official {
        head = head.child(
            div()
                .flex_none()
                .px(px(8.))
                .py(px(2.))
                .rounded_full()
                .bg(theme.info_soft)
                .text_size(theme::text_label())
                .font_weight(FontWeight::MEDIUM)
                .text_color(theme.info_base)
                .child(official.clone()),
        );
    }
    let mut lines = div()
        .flex_1()
        .min_w(px(0.))
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(head);
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
    lines = lines
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(icon_img(icons, Icon::Lock, false, theme.fg_subtle, 12.))
                .child(row.keys_on.clone()),
        )
        .child(integrity::row(
            theme,
            icons,
            row.integrity.0.clone(),
            row.integrity.1,
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

/// What picking a page does.
pub type PickPage = Rc<dyn Fn(String, &mut Window, &mut App)>;

/// "Use my own signing page" (spec 102): the pages this device keeps, each
/// with whose keys it reaches and its integrity line, a way to add one — the
/// only way before a wallet exists, when Settings is out of reach — and
/// Close. A page whose check refused it cannot be picked: nothing would open.
pub fn own_page_sheet(
    theme: &Theme,
    icons: &mut IconCache,
    loc: &Loc,
    rows: &[PageRow],
    on_pick: PickPage,
    add: Div,
    on_close: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let mut list = div().w_full().flex().flex_col();
    for (index, row) in rows.iter().enumerate() {
        let pick = Rc::clone(&on_pick);
        let url = row.url.clone();
        let entry = div()
            .id(("own-page", index))
            .w_full()
            .py(px(12.))
            .px(px(8.))
            .rounded(px(10.))
            .child(page_row_body(theme, icons, row));
        let entry = if row.refused {
            entry.opacity(0.55)
        } else {
            entry
                .cursor_pointer()
                .hover(|style| style.bg(theme.bg_well))
                .on_click(move |_, window, cx| pick(url.clone(), window, cx))
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
                .child(title(theme, loc.t("onboarding.create.ownPageTitle")))
                .child(body(theme, loc.t("onboarding.create.ownPageBody"))),
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
/// page, whose keys the new wallet's will be, and its integrity line — with a
/// way back to Vela's own keys.
pub fn chosen_page_banner(
    theme: &Theme,
    icons: &mut IconCache,
    row: &PageRow,
    on_clear: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    div()
        .w_full()
        .flex()
        .items_start()
        .gap(px(12.))
        .p(px(14.))
        .rounded(px(theme::RADIUS_FIELD))
        .bg(theme.info_soft)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .child(page_row_body(theme, icons, row)),
        )
        .child(
            div()
                .id("own-page-clear")
                .flex_none()
                .p(px(4.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|style| style.bg(theme.bg_raised))
                .child(icon_img(icons, Icon::X, false, theme.fg_muted, 14.))
                .on_click(on_clear),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::trusted_signer::integrity::{CheckFailure, Verdict};

    /// A row says where the page is, whose keys it reaches and how its check
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
        assert_eq!(official.name.as_ref(), "sign.getvela.app");
        assert_eq!(official.host, None);
        assert!(official.keys_on.contains("getvela.app"));
        assert!(official.official.is_some());
        assert!(!official.refused, "checking is not refused");

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
        assert!(own.refused);
        assert_eq!(own.integrity.1, Tone::Danger);
    }
}
