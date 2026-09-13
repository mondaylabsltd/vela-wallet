//! Wallet visuals (spec 015 FR-001): theme + resolved strings in, elements
//! out. No i18n keys, no page state, no window management — the same contract
//! `ui/` established in spec 007.

use gpui::{
    Div, ElementId, ImageSource, InteractiveElement as _, IntoElement, ParentElement, Pixels,
    SharedString, Stateful, StatefulInteractiveElement as _, Styled, canvas, div,
    fill as quad_fill, img, px,
};

use crate::icons::{Icon, IconCache};
use crate::identicon::IdenticonCache;
use crate::theme::{
    self, Theme, WALLET_AVATAR, WALLET_BADGE, WALLET_CONTROL_H, WALLET_NAV_ROW_H, WALLET_ROW_ICON,
    WALLET_TOAST_DISC,
};

use super::fixtures::{
    ActivityKind, ActivityRowModel, AssetRowModel, BalanceModel, BalanceState, ChainRowModel, Fiat,
    StatusKind,
};

/// Tinted glyph at a logical size. `pub(crate)` so the contacts module
/// (spec 018) composes its rows from the same rasterization path instead of
/// growing a second one (SC-006).
pub(crate) fn icon_img(
    icons: &mut IconCache,
    icon: Icon,
    solid: bool,
    color: gpui::Hsla,
    size: f32,
) -> impl IntoElement {
    img(ImageSource::Render(icons.image(
        icon,
        solid,
        color,
        size as u32,
    )))
    .w(px(size))
    .h(px(size))
    .flex_none()
}

/// Small tinted glyphs the page composes into its own rows.
pub fn close_icon(theme: &Theme, icons: &mut IconCache) -> impl IntoElement {
    icon_img(icons, Icon::X, false, theme.fg_muted, 18.)
}
pub fn copy_icon(theme: &Theme, icons: &mut IconCache) -> impl IntoElement {
    icon_img(icons, Icon::Copy, false, theme.fg_base, 16.)
}
pub fn warning_icon(theme: &Theme, icons: &mut IconCache) -> impl IntoElement {
    icon_img(icons, Icon::TriangleAlert, false, theme.warning, 16.)
}
pub fn chevron_icon(theme: &Theme, icons: &mut IconCache) -> impl IntoElement {
    icon_img(icons, Icon::ChevronRight, false, theme.fg_muted, 12.)
}

/// Circular identicon avatar (US3) — the PNG is square; the corner radius on
/// the image quad is what crops it round.
pub fn identicon_avatar(
    identicons: &mut IdenticonCache,
    seed: &str,
    size: f32,
) -> impl IntoElement {
    img(ImageSource::Render(identicons.avatar(seed, size as u32)))
        .w(px(size))
        .h(px(size))
        .rounded(px(size / 2.))
        .flex_none()
}

/// A passkey provider's mark, or nothing when the catalog does not know the
/// authenticator model (a hardware key, or one that reported no AAGUID). Square
/// with a soft corner, not a circle: it is a logo, not a person.
pub fn passkey_mark(
    identicons: &mut IdenticonCache,
    aaguid: &str,
    dark: bool,
    size: f32,
) -> Option<impl IntoElement> {
    let image = identicons.passkey_mark(aaguid, dark, size as u32)?;
    Some(
        img(ImageSource::Render(image))
            .w(px(size))
            .h(px(size))
            .rounded(px(theme::RADIUS_ACK))
            .flex_none(),
    )
}

/// The security-key mark for a key the provider catalog cannot name, or nothing
/// when the row deserves none (a platform authenticator).
pub fn passkey_fallback_mark(
    identicons: &mut IdenticonCache,
    attachment: &str,
    transports: &str,
    chose_security_key: bool,
    theme: &Theme,
    size: f32,
) -> Option<impl IntoElement> {
    let image = identicons.passkey_fallback(
        attachment,
        transports,
        chose_security_key,
        theme,
        size as u32,
    )?;
    Some(
        img(ImageSource::Render(image))
            .w(px(size))
            .h(px(size))
            .flex_none(),
    )
}

/// Sidebar header: avatar + name + chevron + mono address.
pub fn wallet_header(
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    seed: &str,
    name: SharedString,
    address: SharedString,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .child(identicon_avatar(identicons, seed, WALLET_AVATAR))
        .child(
            div()
                .flex()
                .flex_col()
                .min_w(px(0.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(4.))
                        .child(
                            div()
                                .text_size(theme::text_section())
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.fg_base)
                                .whitespace_nowrap()
                                .truncate()
                                .child(name),
                        )
                        .child(icon_img(
                            icons,
                            Icon::ChevronDown,
                            false,
                            theme.fg_subtle,
                            14.,
                        )),
                )
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(address),
                ),
        )
}

/// One sidebar nav row: solid icon + raised wash when selected, outline
/// otherwise (spec FR-007).
pub fn nav_row(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    icon: Icon,
    label: SharedString,
    selected: bool,
) -> Stateful<Div> {
    let fg = if selected {
        theme.fg_base
    } else {
        theme.fg_muted
    };
    let row = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .h(px(WALLET_NAV_ROW_H))
        .px(px(12.))
        .rounded(px(10.))
        .cursor_pointer()
        .text_size(theme::text_row_title())
        .text_color(fg)
        .child(icon_img(icons, icon, selected, fg, 20.))
        .child(label);
    if selected {
        row.bg(theme.bg_raised)
            .font_weight(gpui::FontWeight::SEMIBOLD)
    } else {
        row.hover(|el| el.bg(theme.bg_raised))
    }
}

/// One network-filter row: dot, name, count, accent check when selected.
pub fn chain_row(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    row: &ChainRowModel,
) -> Stateful<Div> {
    let dot = div()
        .w(px(10.))
        .h(px(10.))
        .flex_none()
        .rounded(px(5.))
        .bg(row.dot.unwrap_or(theme.fg_subtle));
    let mut el = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .h(px(32.))
        .px(px(12.))
        .rounded(px(8.))
        .cursor_pointer()
        .hover(|el| el.bg(theme.bg_raised))
        .text_size(theme::text_row_sub())
        .text_color(theme.fg_base)
        .child(dot)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .whitespace_nowrap()
                .truncate()
                .child(row.name.clone()),
        );
    if row.selected {
        el = el.child(icon_img(icons, Icon::Check, false, theme.accent, 14.));
    }
    el.child(
        div()
            .text_color(theme.fg_subtle)
            .child(SharedString::from(row.count.to_string())),
    )
}

/// The pinned ⌘K search affordance (visual only in this feature).
pub fn sidebar_search(theme: &Theme, icons: &mut IconCache, placeholder: SharedString) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .h(px(WALLET_CONTROL_H))
        .px(px(12.))
        .rounded(px(10.))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.divider)
        .text_size(theme::text_row_sub())
        .text_color(theme.fg_subtle)
        .child(icon_img(icons, Icon::Search, false, theme.fg_subtle, 14.))
        .child(div().flex_1().min_w(px(0.)).truncate().child(placeholder))
        .child(
            div()
                .font_family(theme::font_mono())
                .text_size(theme::text_label())
                .child("⌘K"),
        )
}

/// Hero balance with its four states and optional status line (spec FR-008:
/// masking is a render variant, not a separate screen).
/// The figure, with or without a press behind it.
///
/// One helper rather than a `when` at each state, because the hidden hero and
/// the live one must be the SAME target: hiding is a toggle, and a gesture that
/// only works in one direction is a trap.
fn pressable(figure: Div, on_toggle: Option<BalanceToggle>) -> Div {
    let Some(action) = on_toggle else {
        return figure;
    };
    div().child(
        figure
            .id("balance-hero-toggle")
            .cursor_pointer()
            // No hover tint and no ripple: this is a 40-pixel numeral, and a
            // box drawn around money to say "clickable" is the containerised
            // look this design language spent its budget removing.
            .on_click(move |event, window, cx| action(event, window, cx)),
    )
}

/// What a tap on the figure does. `None` draws the same hero with nothing to
/// press — the gallery, and any window with no session behind it.
pub type BalanceToggle = Box<dyn Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App)>;

pub fn balance_display(
    theme: &Theme,
    icons: &mut IconCache,
    model: &BalanceModel,
    on_toggle: Option<BalanceToggle>,
) -> Div {
    let mut root = div().flex().flex_col().gap(px(8.)).child(
        div()
            .text_size(theme::text_label())
            .font_weight(gpui::FontWeight::MEDIUM)
            .text_color(theme.fg_subtle)
            .child(SharedString::from(format!("{} · USD", model.label))),
    );

    root = match model.state {
        BalanceState::Loading => root.child(
            div()
                .w(px(220.))
                .h(px(44.))
                .rounded(px(8.))
                .bg(theme.bg_sunken),
        ),
        BalanceState::Hidden => root.child(pressable(
            div()
                .flex()
                .items_center()
                .gap(px(16.))
                .child(
                    div()
                        .text_size(theme::text_balance_hero())
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.fg_base)
                        .child(model.integer.clone()),
                )
                // The way back. The dots alone would be a screen with no
                // affordance on it, which is how a person concludes the app
                // has lost their money rather than that they hid it.
                .child(icon_img(icons, Icon::EyeOff, false, theme.fg_subtle, 20.)),
            on_toggle,
        )),
        BalanceState::Normal | BalanceState::ZeroLive => {
            let mut amount = div().flex().items_end().child(
                div()
                    .text_size(theme::text_balance_hero())
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme.fg_base)
                    .child(model.integer.clone()),
            );
            if let Some(decimals) = model.decimals.clone() {
                amount = amount.child(
                    div()
                        .pb(px(4.))
                        .text_size(theme::text_balance_decimals())
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.fg_subtle)
                        .child(SharedString::from(format!(".{decimals}"))),
                );
            }
            root.child(pressable(amount, on_toggle))
        }
    };

    if let Some(live) = model.live.clone() {
        root = root.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(div().w(px(8.)).h(px(8.)).rounded(px(4.)).bg(theme.success))
                .child(live),
        );
    }

    if let Some((kind, text)) = model.status.clone() {
        let (icon, color) = match kind {
            StatusKind::Warning => (Icon::TriangleAlert, theme.warning),
            StatusKind::Refreshing => (Icon::RefreshCw, theme.fg_muted),
        };
        root = root.child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(theme::text_row_sub())
                .text_color(color)
                .child(icon_img(icons, icon, false, color, 14.))
                .child(text)
                .child(icon_img(icons, Icon::ChevronRight, false, color, 12.)),
        );
    }

    root
}

/// Desktop action pill (D1: icon + label inline). Caller chains `.on_click`.
pub fn action_pill(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    icon: Icon,
    label: SharedString,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex_1()
        .h(px(52.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .rounded(px(12.))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.border_card)
        .cursor_pointer()
        .hover(|el| el.bg(theme.bg_sunken))
        .text_size(theme::text_row_title())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.fg_base)
        .child(icon_img(icons, icon, false, theme.fg_base, 16.))
        .child(label)
}

/// Section header: bold title + muted trailing action with chevron.
pub fn section_header(
    theme: &Theme,
    icons: &mut IconCache,
    title: SharedString,
    action: SharedString,
) -> Div {
    let (title, action) = section_header_parts(theme, icons, title, action);
    section_header_row().child(title).child(action)
}

/// The header's frame, for callers that bind the halves separately.
pub fn section_header_row() -> Div {
    div().flex().items_center().justify_between().py(px(10.))
}

/// `section_header`, taken apart.
///
/// The assets header names one panel and its action opens another — a header
/// whose action reads "add" must not open the list, and a title that names the
/// section must not open the add form. Both halves come from here so the two
/// destinations cannot drift into two different-looking headers.
pub fn section_header_parts(
    theme: &Theme,
    icons: &mut IconCache,
    title: SharedString,
    action: SharedString,
) -> (Div, Div) {
    (
        div()
            .text_size(theme::text_section())
            .font_weight(gpui::FontWeight::BOLD)
            .text_color(theme.fg_base)
            .child(title),
        div()
            .flex()
            .items_center()
            .gap(px(2.))
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_muted)
            .child(action)
            .child(icon_img(
                icons,
                Icon::ChevronRight,
                false,
                theme.fg_muted,
                12.,
            )),
    )
}

fn lead_circle(theme: &Theme, inner: impl IntoElement, badge: gpui::Hsla) -> Div {
    div()
        .relative()
        .w(px(WALLET_ROW_ICON))
        .h(px(WALLET_ROW_ICON))
        .flex_none()
        .child(
            div()
                .w_full()
                .h_full()
                .rounded(px(WALLET_ROW_ICON / 2.))
                .bg(theme.bg_sunken)
                .flex()
                .items_center()
                .justify_center()
                .child(inner),
        )
        .child(
            div()
                .absolute()
                .bottom_0()
                .right_0()
                .w(px(WALLET_BADGE))
                .h(px(WALLET_BADGE))
                .rounded(px(WALLET_BADGE / 2.))
                .bg(badge)
                .border_2()
                .border_color(theme.bg_base),
        )
}

/// Activity row (mock H1/D1): direction glyph + chain badge, title/subtitle,
/// signed amount + unit. Masked keeps the unit (H5's rule).
pub fn activity_row(theme: &Theme, icons: &mut IconCache, row: &ActivityRowModel) -> Div {
    let glyph = match row.kind {
        ActivityKind::Sent => Icon::ArrowUpRight,
        ActivityKind::Received => Icon::ArrowDownLeft,
        ActivityKind::Dapp => Icon::Link2,
    };
    let amount_color = if row.positive {
        theme.success
    } else {
        theme.fg_base
    };
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(10.))
        .child(lead_circle(
            theme,
            icon_img(icons, glyph, false, theme.fg_muted, 18.),
            row.badge,
        ))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(row.title.clone()),
                )
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .whitespace_nowrap()
                        .truncate()
                        .child(row.subtitle.clone()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(4.))
                .flex_none()
                .child(
                    div()
                        .text_size(theme::text_amount())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(amount_color)
                        .child(row.amount.clone()),
                )
                .child(
                    div()
                        .text_size(theme::text_unit())
                        .text_color(theme.fg_subtle)
                        .child(row.unit.clone()),
                ),
        )
}

fn token_glyph(theme: &Theme, ticker: &str) -> Div {
    let glyph: String = ticker.chars().take(3).collect::<String>().to_uppercase();
    div()
        .text_size(theme::text_label())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.fg_muted)
        .child(SharedString::from(glyph))
}

/// Token icon: sunken circle with a 3-letter glyph + chain badge.
pub fn token_icon(theme: &Theme, ticker: &str, badge: gpui::Hsla) -> Div {
    lead_circle(theme, token_glyph(theme, ticker), badge)
}

/// Asset row. Caller chains `.on_click` (opens the detail panel — US2).
pub fn asset_row(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    row: &AssetRowModel,
) -> Stateful<Div> {
    let _ = icons;
    let fiat = match &row.fiat {
        Fiat::Value(text) => div()
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_subtle)
            .child(text.clone()),
        Fiat::NoPrice(text) => div()
            .text_size(theme::text_row_sub())
            .text_color(theme.warning)
            .child(text.clone()),
        Fiat::Masked => div()
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_subtle)
            .child(super::fixtures::MASK),
    };
    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(10.))
        .px(px(8.))
        .mx(px(-8.))
        .rounded(px(10.))
        .cursor_pointer()
        .hover(|el| el.bg(theme.bg_raised))
        .child(token_icon(theme, row.ticker.as_ref(), row.badge))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(row.ticker.clone()),
                )
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .whitespace_nowrap()
                        .truncate()
                        .child(row.chain.clone()),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .gap(px(2.))
                .flex_none()
                .child(
                    div()
                        .text_size(theme::text_amount())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(row.balance.clone()),
                )
                .child(fiat),
        )
}

/// Empty state: sunken circle + outline icon, title, caption.
pub fn empty_state(
    theme: &Theme,
    icons: &mut IconCache,
    icon: Icon,
    title: SharedString,
    caption: SharedString,
) -> Div {
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .py(px(32.))
        .child(
            div()
                .w(px(56.))
                .h(px(56.))
                .rounded(px(28.))
                .bg(theme.bg_sunken)
                .flex()
                .items_center()
                .justify_center()
                .child(icon_img(icons, icon, false, theme.fg_subtle, 24.)),
        )
        .child(
            div()
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_base)
                .child(title),
        )
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(caption),
        )
}

/// Loading placeholder mimicking row geometry (static — motion adds nothing
/// to a fixture gallery).
pub fn skeleton_row(theme: &Theme) -> Div {
    let bar = |w: f32| {
        div()
            .w(px(w))
            .h(px(10.))
            .rounded(px(4.))
            .bg(theme.bg_sunken)
    };
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(12.))
        .child(
            div()
                .w(px(WALLET_ROW_ICON))
                .h(px(WALLET_ROW_ICON))
                .flex_none()
                .rounded(px(WALLET_ROW_ICON / 2.))
                .bg(theme.bg_sunken),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(bar(120.))
                .child(bar(180.)),
        )
        .child(bar(56.))
}

/// The deterministic 21×21 demo pattern (data-model.md): three finder squares
/// + xorshift32(0x5EED) noise, on an always-white card. Never encodes data.
pub fn qr_placeholder(theme: &Theme, caption: SharedString, side: Pixels) -> Div {
    const N: usize = 21;
    let ink = gpui::Hsla::from(gpui::rgb(0x1a1a18));
    let white = gpui::Hsla::from(gpui::rgb(0xffffff));

    let cells: Vec<bool> = {
        let mut s: u32 = 0x5eed;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s
        };
        let in_finder =
            |r: usize, c: usize| (r < 7 && c < 7) || (r < 7 && c >= N - 7) || (r >= N - 7 && c < 7);
        let finder_on = |r: usize, c: usize| {
            let lr = if r < 7 { r } else { r - (N - 7) };
            let lc = if c < 7 { c } else { c - (N - 7) };
            lr.min(lc).min(6 - lr).min(6 - lc) != 1
        };
        (0..N * N)
            .map(|i| {
                let (r, c) = (i / N, i % N);
                if in_finder(r, c) {
                    finder_on(r, c)
                } else if next() & 3 == 0 {
                    false
                } else {
                    next() % 2 == 0
                }
            })
            .collect()
    };

    let _ = theme;
    div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(12.))
        .p(px(20.))
        .rounded(px(16.))
        .bg(white)
        .child(
            div().w(side).h(side).child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        let cell = f32::from(bounds.size.width) / N as f32;
                        for (i, on) in cells.iter().enumerate() {
                            if !*on {
                                continue;
                            }
                            let (r, c) = (i / N, i % N);
                            let origin = gpui::Point::new(
                                bounds.origin.x + px(c as f32 * cell),
                                bounds.origin.y + px(r as f32 * cell),
                            );
                            let size = gpui::Size {
                                width: px(cell + 0.5),
                                height: px(cell + 0.5),
                            };
                            window.paint_quad(quad_fill(gpui::Bounds { origin, size }, ink));
                        }
                    },
                )
                .size_full(),
            ),
        )
        .child(
            div()
                .text_size(theme::text_label())
                .text_color(ink.opacity(0.5))
                .child(caption),
        )
}

/// The money-in celebration (D1b): a floating pill saying what landed.
///
/// The phone's version is a solid green banner with white words on it. This is
/// not that, for a reason that is about the palette rather than about taste:
/// the dark palette's success green (`#3da872`) under white text is a contrast
/// ratio of about three to one, which passes for a 28-pixel glyph and fails for
/// a sentence. So the colour lives in the disc — a small area, a shape rather
/// than a word — and the sentence sits on the raised surface every other
/// floating thing in this shell uses, at full contrast in both palettes.
///
/// Nothing here is interactive, and deliberately: it is over the middle of the
/// window for 2.8 seconds and a person who reaches for what is underneath must
/// get what is underneath. `menu_card`'s `occlude` is what a menu needs and
/// what this must never have.
pub fn receipt_toast(theme: &Theme, icons: &mut IconCache, text: SharedString) -> Div {
    let height = WALLET_TOAST_DISC + 20.;
    div()
        .flex()
        .items_center()
        .gap(px(10.))
        .pl(px(10.))
        .pr(px(18.))
        .py(px(10.))
        .rounded(px(height / 2.))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.divider)
        .shadow_lg()
        .child(
            div()
                .w(px(WALLET_TOAST_DISC))
                .h(px(WALLET_TOAST_DISC))
                .flex_none()
                .rounded(px(WALLET_TOAST_DISC / 2.))
                .bg(theme.success)
                .flex()
                .items_center()
                .justify_center()
                .child(icon_img(
                    icons,
                    Icon::ArrowDownLeft,
                    false,
                    theme.fg_inverse,
                    16.,
                )),
        )
        .child(
            div()
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_base)
                .child(text),
        )
}
