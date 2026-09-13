//! Explore visuals (spec 022 FR-001): theme + resolved strings in, elements
//! out. No i18n keys, no page state, no window management — the contract
//! `ui/` established in spec 007 and the wallet kept.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    AnyElement, Div, ElementId, Hsla, InteractiveElement as _, IntoElement as _, ParentElement,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled, div, px,
};

use crate::icons::{Icon, IconCache};
use crate::identicon::IdenticonCache;
use crate::theme::{self, Theme};
use crate::wallet::components::{icon_img, identicon_avatar};

use super::fixtures::{DemoPage, SiteModel, TabModel, demo_palette};

/// Explore geometry the token set does not name (spec 022), MEASURED off the
/// mocks in design/explore at the 1280×800 desktop frame.
pub const TAB_STRIP_H: f32 = 36.;
pub const TAB_W: f32 = 200.;
pub const TAB_H: f32 = 32.;
pub const TOOLBAR_H: f32 = 56.;
pub const TOOLBAR_CONTROL: f32 = 32.;
pub const TILE_AVATAR: f32 = 56.;
pub const ROW_AVATAR: f32 = 40.;

/// A site or token's mark: its first letter on a wash of its own brand colour.
/// Deliberately NOT a fetched favicon — a wallet that downloads an icon from
/// the site it is about to warn you about has handed that site a tracking
/// pixel and a way to impersonate a brand.
pub fn letter_avatar(letter: SharedString, tint: Hsla, size: f32) -> Div {
    let mut wash = tint;
    wash.a = 0.16;
    div()
        .w(px(size))
        .h(px(size))
        .flex_none()
        .rounded_full()
        .bg(wash)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(size * 0.42))
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(tint)
        .child(letter)
}

/// One favourites tile: the 56 mark over a label a step below a row's.
pub fn site_tile(id: ElementId, theme: &Theme, site: &SiteModel) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(88.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .child(letter_avatar(site.letter.clone(), site.tint, TILE_AVATAR))
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .truncate()
                .child(site.name.clone()),
        )
}

/// The trailing "+ 添加" affordance, which is a tile like any other.
pub fn add_tile(
    id: ElementId,
    theme: &Theme,
    icons: &mut IconCache,
    label: SharedString,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(88.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .child(
            div()
                .w(px(TILE_AVATAR))
                .h(px(TILE_AVATAR))
                .rounded_full()
                .bg(theme.bg_sunken)
                .flex()
                .items_center()
                .justify_center()
                .child(icon_img(icons, Icon::Plus, false, theme.fg_subtle, 20.)),
        )
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(label),
        )
}

/// A site inside a group: mark, name, blurb, and the recent group's timestamp.
pub fn site_row(
    id: ElementId,
    theme: &Theme,
    identicons: &mut IdenticonCache,
    site: &SiteModel,
) -> Stateful<Div> {
    let _ = identicons;
    let mut row = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(10.))
        .cursor_pointer()
        .child(letter_avatar(site.letter.clone(), site.tint, ROW_AVATAR))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(site.name.clone()),
                )
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .truncate()
                        .child(site.subtitle.clone().unwrap_or_else(|| site.host.clone())),
                ),
        );
    if let Some(meta) = site.meta.clone().filter(|m| !m.is_empty()) {
        row = row.child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.accent)
                .child(meta),
        );
    }
    row
}

/// The tab strip in the window's drag area (DE1–DE4). The selected tab is the
/// same colour as the toolbar below it, so the two read as one surface.
/// What a tab strip can DO, when a machine is behind it.
///
/// One entry per tab in the order they are drawn, plus the new-tab button.
/// `None` throughout is the gallery's strip: drawn exactly as it always was,
/// answering nothing (the rule the slide, the allowance chips and the site
/// menu all follow).
#[derive(Default)]
pub struct TabActions {
    pub select: Vec<Option<crate::flows::panels::Click>>,
    pub close: Vec<Option<crate::flows::panels::Click>>,
    pub new_tab: Option<crate::flows::panels::Click>,
}

pub fn tab_strip(
    theme: &Theme,
    icons: &mut IconCache,
    tabs: &[TabModel],
    new_tab_label: SharedString,
    close_label: SharedString,
) -> Div {
    tab_strip_with(
        theme,
        icons,
        tabs,
        new_tab_label,
        close_label,
        TabActions::default(),
    )
}

pub fn tab_strip_with(
    theme: &Theme,
    icons: &mut IconCache,
    tabs: &[TabModel],
    new_tab_label: SharedString,
    close_label: SharedString,
    mut actions: TabActions,
) -> Div {
    let mut strip = div()
        .h(px(TAB_STRIP_H))
        .flex()
        .items_end()
        .gap(px(2.))
        .px(px(12.))
        .bg(theme.bg_sunken);

    for (i, tab) in tabs.iter().enumerate() {
        let mut face = div()
            .w(px(TAB_W))
            .h(px(TAB_H))
            .px(px(12.))
            .rounded_t(px(6.))
            .flex()
            .items_center()
            .gap(px(8.))
            .text_size(theme::text_row_sub())
            .when(tab.selected, |d| d.bg(theme.bg_base))
            .text_color(if tab.selected {
                theme.fg_base
            } else {
                theme.fg_muted
            });
        if let Some(site) = &tab.site {
            face = face.child(letter_avatar(site.letter.clone(), site.tint, 16.));
        }
        // The close glyph is its own control: a click on it must close the
        // tab, never merely select it, and the two live one inside the other.
        let close = actions.close.get_mut(i).and_then(Option::take);
        let cross = icon_img(icons, Icon::X, false, theme.fg_muted, 12.);
        let cross = match close {
            Some(close) => div()
                .id(ElementId::from(("tab-close", i)))
                .cursor_pointer()
                .child(cross)
                .on_click(move |event, window, cx| close(event, window, cx))
                .into_any_element(),
            None => cross.into_any_element(),
        };
        face = face
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .child(tab.title.clone()),
            )
            .child(cross);
        let select = actions.select.get_mut(i).and_then(Option::take);
        let mut tab_el = div()
            .id(ElementId::from(("tab", i)))
            .cursor_pointer()
            .child(face);
        if let Some(select) = select {
            tab_el = tab_el.on_click(move |event, window, cx| select(event, window, cx));
        }
        strip = strip.child(tab_el);
        let _ = &close_label;
    }

    let mut plus = div()
        .id("new-tab")
        .mb(px(6.))
        .w(px(20.))
        .h(px(20.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .child(icon_img(icons, Icon::Plus, false, theme.fg_muted, 14.));
    if let Some(new_tab) = actions.new_tab.take() {
        plus = plus.on_click(move |event, window, cx| new_tab(event, window, cx));
    }
    strip.child(plus).child(
        div()
            .flex_1()
            .child(div().h(px(1.)).child(new_tab_label.clone()))
            .invisible(),
    )
}

/// One toolbar control — a 32 square with a tinted glyph.
pub fn toolbar_control(theme: &Theme, icons: &mut IconCache, icon: Icon, tint: Hsla) -> Div {
    let _ = theme;
    div()
        .w(px(TOOLBAR_CONTROL))
        .h(px(TOOLBAR_CONTROL))
        .rounded(px(6.))
        .flex()
        .items_center()
        .justify_center()
        .child(icon_img(icons, icon, false, tint, 18.))
}

/// The account chip. Its green dot IS the connection state — the only thing in
/// this bar that says a site can see your address.
pub fn account_chip(
    theme: &Theme,
    identicons: &mut IdenticonCache,
    name: SharedString,
    seed: &str,
    connected: bool,
) -> Div {
    let mut chip = div()
        .h(px(TOOLBAR_CONTROL))
        .px(px(12.))
        .rounded_full()
        .bg(theme.bg_sunken)
        .flex()
        .items_center()
        .gap(px(8.))
        .child(identicon_avatar(identicons, seed, 16.))
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .child(name),
        );
    if connected {
        chip = chip.child(
            div()
                .w(px(8.))
                .h(px(8.))
                .rounded_full()
                .bg(theme.success_base),
        );
    }
    chip
}

/// The browser toolbar (DE1–DE4). On the start page the address field is the
/// search box; while browsing it collapses to the domain with its padlock —
/// one control, two states, never two controls. `trailing` is built by the
/// page, because its two affordances (⋯ and the account chip) each open a
/// different thing and the listeners belong to the entity that owns that state.
/// Back, forward and reload, in that order. `None` leaves them drawn and
/// inert, which is what the mocks are — a live browser passes three listeners
/// and the same three buttons start working.
pub type NavActions = [crate::flows::panels::Click; 3];

pub fn toolbar(
    theme: &Theme,
    icons: &mut IconCache,
    browsing: bool,
    host: SharedString,
    search_placeholder: SharedString,
    trailing: Div,
    nav: Option<NavActions>,
) -> Div {
    let address = if browsing {
        div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .child(icon_img(icons, Icon::Lock, false, theme.fg_muted, 12.))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(host),
            )
    } else {
        div()
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .child(icon_img(icons, Icon::Search, false, theme.fg_subtle, 14.))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(search_placeholder),
            )
    };

    div()
        .h(px(TOOLBAR_H))
        .px(px(20.))
        .flex()
        .items_center()
        .gap(px(8.))
        .bg(theme.bg_base)
        .border_b_1()
        .border_color(theme.divider)
        .children(nav_controls(theme, icons, nav))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .h(px(TOOLBAR_CONTROL))
                .mx(px(12.))
                .rounded(px(6.))
                .bg(theme.bg_sunken)
                .flex()
                .items_center()
                .justify_center()
                .child(address),
        )
        .child(trailing)
}

/// The three navigation buttons, live or drawn.
fn nav_controls(theme: &Theme, icons: &mut IconCache, nav: Option<NavActions>) -> Vec<AnyElement> {
    let icons_and_tints = [
        (Icon::ArrowLeft, theme.fg_base),
        (Icon::ArrowRight, theme.fg_subtle),
        (Icon::RefreshCw, theme.fg_base),
    ];
    let mut actions = nav.map(Vec::from).unwrap_or_default().into_iter();
    icons_and_tints
        .into_iter()
        .enumerate()
        .map(|(i, (icon, tint))| {
            let control = toolbar_control(theme, icons, icon, tint);
            match actions.next() {
                Some(action) => crate::flows::panels::clickable(
                    ElementId::from(("browser-nav", i)),
                    Some(action),
                    control,
                )
                .into_any_element(),
                None => control.into_any_element(),
            }
        })
        .collect()
}

/// A stand-in for whatever site is open (spec 022 §2). Deliberately NOT
/// chrome: its words and its pink button belong to the SITE, so nothing here
/// is translated and nothing here uses a Vela colour token — the palette sits
/// beside the other content colours in `fixtures::demo_palette`.
pub fn demo_page(page: &DemoPage) -> Div {
    let mut card = div()
        .w(px(320.))
        .p(px(20.))
        .rounded(px(20.))
        .bg(demo_palette::card())
        .flex()
        .flex_col()
        .gap(px(12.))
        .child(
            div()
                .text_size(px(15.))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(demo_palette::ink())
                .child(page.title.clone()),
        );

    for (value, symbol) in &page.fields {
        card = card.child(
            div()
                .p(px(16.))
                .rounded(px(12.))
                .bg(demo_palette::field())
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(px(22.))
                        .text_color(demo_palette::ink())
                        .child(value.clone()),
                )
                .child(
                    div()
                        .text_size(px(13.))
                        .text_color(demo_palette::ink_muted())
                        .child(symbol.clone()),
                ),
        );
    }

    card = card.child(
        div()
            .id("demo-cta")
            .h(px(48.))
            .rounded_full()
            .bg(page.cta_tint)
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .text_size(px(15.))
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(demo_palette::card())
            .child(page.cta.clone()),
    );

    div()
        .flex_1()
        .min_h(px(0.))
        .bg(demo_palette::surface())
        .flex()
        .flex_col()
        .items_center()
        .gap(px(12.))
        .pt(px(56.))
        .child(card)
        .child(
            div()
                .w(px(300.))
                .h(px(10.))
                .rounded_full()
                .bg(demo_palette::card()),
        )
        .child(
            div()
                .w(px(220.))
                .h(px(10.))
                .rounded_full()
                .bg(demo_palette::card()),
        )
}
