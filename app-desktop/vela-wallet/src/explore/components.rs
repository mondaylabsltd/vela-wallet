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

/// A site's avatar (spec 079 F16): its own icon over its letter — a picture
/// that fails to load draws nothing, so the letter beneath is what stays. The
/// icons are https only (the founder's ruling on site icons, 2026-09-19);
/// they are tried in order, the first that loads on top.
pub fn site_avatar(letter: SharedString, tint: Hsla, icon_urls: &[SharedString], size: f32) -> Div {
    let mut mark = div()
        .relative()
        .size(px(size))
        .flex_none()
        .child(letter_avatar(letter, tint, size));
    for url in icon_urls.iter().rev() {
        mark = mark.child(
            gpui::img(url.clone())
                .absolute()
                .top_0()
                .left_0()
                .size(px(size))
                .rounded_full(),
        );
    }
    mark
}

/// A tile's column: the web's 8-up grid over its 800 page — (736 − 7 × 8) / 8
/// (078 E-05).
pub const TILE_W: f32 = 85.;

/// One favourites tile: the 56 mark over an 11 label (078 E-05).
pub fn site_tile(id: ElementId, theme: &Theme, site: &SiteModel) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(TILE_W))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .cursor_pointer()
        .child(site_avatar(
            site.letter.clone(),
            site.tint,
            &site.icon_urls,
            TILE_AVATAR,
        ))
        .child(
            div()
                .max_w_full()
                .text_size(theme::text_label())
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
        .w(px(TILE_W))
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
                .text_size(theme::text_label())
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
    // Padded 12 on a button's line, as the web's `SiteRow` (078 E-05).
    let mut row = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(12.))
        .line_height(gpui::relative(crate::wallet::components::LINE_NORMAL))
        .cursor_pointer()
        .child(site_avatar(
            site.letter.clone(),
            site.tint,
            &site.icon_urls,
            ROW_AVATAR,
        ))
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
            .rounded_t(px(8.))
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
        // The start page's tab wears the sail, as the web's (078 E-04); a
        // site's tab its mark.
        face = match &tab.site {
            Some(site) => face.child(site_avatar(
                site.letter.clone(),
                site.tint,
                &site.icon_urls,
                16.,
            )),
            None => face.child(crate::ui::vela_mark(theme, px(16.))),
        };
        // The close glyph is its own control: a click on it must close the
        // tab, never merely select it, and the two live one inside the other.
        let close = actions.close.get_mut(i).and_then(Option::take);
        // A 20 box, radius 4, raised on hover — the web's `.close` (078 E-04).
        let raised = theme.bg_raised;
        let cross = div()
            .size(px(20.))
            .flex_none()
            .rounded(px(4.))
            .flex()
            .items_center()
            .justify_center()
            .child(icon_img(icons, Icon::X, false, theme.fg_muted, 12.));
        let cross = match close {
            Some(close) => cross
                .id(ElementId::from(("tab-close", i)))
                .cursor_pointer()
                .hover(move |el| el.bg(raised))
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
    toolbar_control_with(theme, icons, icon, tint, false, true)
}

/// The toolbar's icon button as the web's `.icon` (078 E-04): radius 8, a
/// sunken hover while it can act; disabled, the subtle colour at 45 % and no
/// hover. `solid` fills the glyph — the star on a favourite.
pub fn toolbar_control_with(
    theme: &Theme,
    icons: &mut IconCache,
    icon: Icon,
    tint: Hsla,
    solid: bool,
    enabled: bool,
) -> Div {
    let sunken = theme.bg_sunken;
    let control = div()
        .w(px(TOOLBAR_CONTROL))
        .h(px(TOOLBAR_CONTROL))
        .rounded(px(8.))
        .flex()
        .items_center()
        .justify_center();
    if enabled {
        control
            .hover(move |el| el.bg(sunken))
            .child(icon_img(icons, icon, solid, tint, 18.))
    } else {
        control
            .opacity(0.45)
            .child(icon_img(icons, icon, solid, theme.fg_subtle, 18.))
    }
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
/// one control, two states, never two controls. `address` ([`address_field`])
/// and `trailing` are built by the page, because the field's focus and keys
/// and the two affordances (⋯ and the account chip) are state the page owns.
/// Back, forward and reload, in that order. `None` leaves them drawn and
/// inert, which is what the mocks are — a live browser passes three listeners
/// and the same three buttons start working.
pub type NavActions = [crate::flows::panels::Click; 3];

pub fn toolbar(
    theme: &Theme,
    icons: &mut IconCache,
    address: AnyElement,
    trailing: Div,
    nav: Option<NavActions>,
) -> Div {
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

/// What the middle of the toolbar says.
pub struct AddressBar {
    /// A page is open: its host and a lock. Otherwise the search box.
    pub browsing: bool,
    pub host: SharedString,
    /// Whether the lock may be drawn. https, or an http host on this machine
    /// or its network — the core's `secure`, the same judgement that lets a
    /// site ask for a signature. A public http page gets a warning glyph: a
    /// padlock beside it would be the chrome vouching for a connection
    /// anybody on the path can read.
    pub secure: bool,
    pub placeholder: SharedString,
    /// Somebody is typing: the text so far, drawn with a caret.
    pub draft: Option<SharedString>,
    /// The whole draft is selected: drawn highlighted, with no caret.
    pub selected: bool,
    /// A word said in the bar for a moment — "Copied" after the site menu
    /// copied its link. The page is a native view gpui cannot draw over, so
    /// a toast over it would be under it; the bar is what stays visible.
    pub notice: Option<SharedString>,
}

/// The address field's contents, for the page to wrap in whatever makes it
/// editable — the focus and the keys are the page's state.
///
/// `typing` is the live editor while somebody types — the page's, since the
/// caret, the selection and the IME are its state; the mock draws the draft.
pub fn address_field(
    theme: &Theme,
    icons: &mut IconCache,
    bar: &AddressBar,
    typing: Option<AnyElement>,
) -> Div {
    let row = div()
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .min_w(px(0.))
        .overflow_hidden();
    if let Some(typing) = typing {
        return row
            .w_full()
            .child(icon_img(icons, Icon::Search, false, theme.fg_subtle, 14.))
            .child(div().flex_1().min_w(px(0.)).child(typing));
    }
    if let Some(draft) = &bar.draft {
        let text = if draft.is_empty() {
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(bar.placeholder.clone())
        } else {
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .whitespace_nowrap()
                .when(bar.selected, |text| {
                    text.bg(theme.accent.opacity(0.28)).rounded(px(2.))
                })
                .child(draft.clone())
        };
        // The caret sits AGAINST the text, in a box of its own: as a third
        // child of the row it took the row's 8px gap and floated a space
        // after the last letter, where no text would ever go.
        let mut typed = div()
            .flex()
            .items_center()
            .min_w(px(0.))
            .overflow_hidden()
            .child(text);
        if !(bar.selected && !draft.is_empty()) {
            typed = typed.child(div().w(px(1.5)).h(px(14.)).flex_none().bg(theme.accent));
        }
        return row
            .child(icon_img(icons, Icon::Search, false, theme.fg_subtle, 14.))
            .child(typed);
    }
    if let Some(notice) = &bar.notice {
        return row
            .child(icon_img(icons, Icon::Check, false, theme.success, 12.))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.success)
                    .child(notice.clone()),
            );
    }
    if bar.browsing {
        // Spec 079 (owner: "用一把锁代表 https 和非https 就行了，不文字标记"): a
        // closed lock, quiet, for https — it says the line is encrypted, not
        // that the site is honest — and an open one in the warning colour for
        // plain http. No word beside either.
        let (glyph, tint) = lock_glyph(theme, bar.secure);
        return row.child(icon_img(icons, glyph, false, tint, 12.)).child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .child(bar.host.clone()),
        );
    }
    row.child(icon_img(icons, Icon::Search, false, theme.fg_subtle, 14.))
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(bar.placeholder.clone()),
        )
}

/// The site's network picker (spec 079 FR-017; owner: "连接时 切换网络，没有
/// 网络logo呀 … 需要能看到这个网络上的余额吧"): each row leads with the chain's
/// logo, names it, shows what the account holds there, and ticks the site's
/// own. A long list scrolls inside the card rather than off the window.
pub fn network_pick_card(
    theme: &Theme,
    icons: &mut IconCache,
    rows: &[super::fixtures::NetworkPick],
    actions: Vec<Option<crate::contacts::components::MenuAction>>,
) -> Div {
    let mut list = div()
        .id("network-pick-list")
        .max_h(px(440.))
        .overflow_y_scroll()
        .flex()
        .flex_col();
    let mut actions = actions.into_iter();
    for (i, row) in rows.iter().enumerate() {
        let name = crate::executor::custom_tokens::network_name(row.chain_id);
        let line = div()
            .id(ElementId::from(("network-pick", i)))
            .flex()
            .flex_none()
            .items_center()
            .gap(px(12.))
            .h(px(44.))
            .px(px(14.))
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_base)
            .child(crate::settings::components::chain_logo_mark(
                u64::from(row.chain_id),
                crate::settings::model::lettermark(&name),
                crate::settings::model::chain_tint(u64::from(row.chain_id)).unwrap_or(0x8A_8F_98),
                20.,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .child(row.name.clone()),
            )
            .children(
                row.amount
                    .clone()
                    .map(|amount| div().flex_none().text_color(theme.fg_muted).child(amount)),
            )
            .child(div().size(px(16.)).flex_none().when(row.current, |el| {
                el.child(icon_img(icons, Icon::Check, false, theme.fg_base, 16.))
            }));
        list = list.child(match actions.next().flatten() {
            Some(action) => line
                .cursor_pointer()
                .hover(|el| el.bg(theme.bg_sunken))
                .on_click(action)
                .into_any_element(),
            None => line.into_any_element(),
        });
    }
    div()
        .w(px(300.))
        .py(px(6.))
        .rounded(px(12.))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.divider)
        .shadow_lg()
        .child(list)
}

/// The lock for a page's scheme (spec 079 FR-015): closed and neutral for
/// https, open in the warning colour for http. The one security mark every
/// browser surface draws, and never with a word.
#[must_use]
pub fn lock_glyph(theme: &Theme, secure: bool) -> (Icon, Hsla) {
    if secure {
        (Icon::Lock, theme.fg_muted)
    } else {
        (Icon::LockOpen, theme.warning_base)
    }
}

/// The load's hairline under the toolbar (spec 079 US3): an accent segment
/// running across while a load is out — asked for and not yet finished. The
/// row is always 2 px tall, so nothing moves when it starts or stops.
pub fn load_hairline(theme: &Theme, busy: bool) -> Div {
    use gpui::{Animation, AnimationExt as _};
    let track = div()
        .h(px(2.))
        .w_full()
        .flex_none()
        .relative()
        .overflow_hidden();
    if !busy {
        return track;
    }
    track.child(
        div()
            .absolute()
            .top_0()
            .h_full()
            .w(gpui::relative(0.3))
            .bg(theme.accent)
            .with_animation(
                "load-hairline",
                Animation::new(std::time::Duration::from_millis(1200)).repeat(),
                |bar, delta| bar.left(gpui::relative(delta * 1.3 - 0.3)),
            ),
    )
}

/// The three navigation buttons, live or drawn.
fn nav_controls(theme: &Theme, icons: &mut IconCache, nav: Option<NavActions>) -> Vec<AnyElement> {
    // Back and reload act; forward is the web's drawn `canForward: false`
    // — disabled, not merely grey (078 E-04).
    let icons_and_tints = [
        (Icon::ArrowLeft, true),
        (Icon::ArrowRight, false),
        (Icon::RefreshCw, true),
    ];
    let mut actions = nav.map(Vec::from).unwrap_or_default().into_iter();
    icons_and_tints
        .into_iter()
        .enumerate()
        .map(|(i, (icon, enabled))| {
            let control = toolbar_control_with(theme, icons, icon, theme.fg_base, false, enabled);
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 079 FR-015: the scheme is a lock and only a lock — closed and
    /// quiet for https, open in the warning colour for http.
    #[test]
    fn the_lock_says_the_scheme_and_nothing_else() {
        let theme = Theme::light();
        assert_eq!(lock_glyph(&theme, true), (Icon::Lock, theme.fg_muted));
        assert_eq!(
            lock_glyph(&theme, false),
            (Icon::LockOpen, theme.warning_base)
        );
    }
}
