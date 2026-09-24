//! Settings visuals (spec 023): theme + resolved strings + fixture models in,
//! elements out. Same contract `wallet::components` and `contacts::components`
//! follow — no i18n keys, no page state, no window management.
//!
//! The forty mocks in `design/settings/` are a small vocabulary re-dealt, and
//! this is the vocabulary: a status pill, a callout, a nav row, a form row, a
//! labelled URL field, a checklist, a storage line and a key/value row. Every
//! panel in `wallet::page` is a composition of these.

use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Div, ElementId, InteractiveElement as _, IntoElement, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled, StyledImage as _, div, px, rgb,
};

use crate::icons::{Icon, IconCache};
use crate::theme::{self, SETTINGS_NAV_ROW_H, Theme, WALLET_CONTROL_H};
use crate::wallet::components::icon_img;

use super::fixtures::{Pill, StorageGroup, Tone};

/// Leading glyph in a nav row / form control.
const GLYPH_SM: f32 = 16.;
/// A chain's circular mark.
const MARK: f32 = 32.;

// -- StatusPill ---------------------------------------------------------------

/// The one badge every settings screen uses: latency, reachability, provider
/// state and compatibility are all this object in the mocks, differing only in
/// tone. One component, not four.
pub fn status_pill(theme: &Theme, pill: &Pill) -> Div {
    let (fg, bg) = match pill.tone {
        Tone::Ok => (theme.success_base, theme.success_soft),
        Tone::Warn => (theme.warning_base, theme.warning_soft),
        Tone::Error => (theme.error_base, theme.error_soft),
        // Unset, not failed — the mocks grey these rather than colouring them.
        Tone::Neutral => (theme.fg_subtle, theme.bg_raised),
    };
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .px(px(8.))
        .py(px(3.))
        .rounded_full()
        .bg(bg)
        .when(pill.dot, |el| {
            el.child(div().size(px(6.)).rounded_full().bg(fg))
        })
        .child(
            div()
                .text_size(theme::text_label())
                .text_color(fg)
                .child(pill.label.clone()),
        )
}

// -- Callout ------------------------------------------------------------------

/// Warning / danger / info / success. Eight mocks use it; `Success` swaps the
/// triangle for a check, because a green triangle reads as an alarm.
///
/// Only `Warning` has a desktop mock (DSR1) — the danger, info and success
/// callouts belong to phone screens the desktop folds into panels. They stay
/// because this is a component vocabulary and the tone is the ONE thing that
/// distinguishes the four; a callout that could only warn would be a warning
/// box, and the next desktop state that needs a red one would fork it.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "component vocabulary; the desktop mocks use one tone so far"
)]
pub enum CalloutTone {
    Warning,
    Danger,
    Info,
    Success,
}

pub fn callout(
    theme: &Theme,
    icons: &mut IconCache,
    tone: CalloutTone,
    text: impl Into<gpui::SharedString>,
) -> Div {
    let (fg, bg, icon) = match tone {
        CalloutTone::Warning => (theme.warning_base, theme.warning_soft, Icon::TriangleAlert),
        CalloutTone::Danger => (theme.error_base, theme.error_soft, Icon::TriangleAlert),
        CalloutTone::Info => (theme.info_base, theme.info_soft, Icon::Info),
        CalloutTone::Success => (theme.success_base, theme.success_soft, Icon::Check),
    };
    div()
        .flex()
        .items_start()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(10.))
        .bg(bg)
        .child(
            div()
                .flex_none()
                .mt(px(2.))
                .child(icon_img(icons, icon, false, fg, GLYPH_SM)),
        )
        .child(
            div()
                .flex_1()
                // Without this the flex item keeps its automatic min-width —
                // the text's intrinsic width — and a long sentence runs out of
                // the callout, out of the dialog, and over the page behind it
                // rather than wrapping. Measured on the add-network warning
                // (spec 081 FR-009), which left ~40% of its first sentence
                // sitting on top of a settings row.
                .min_w(px(0.))
                .text_size(theme::text_row_sub())
                .text_color(fg)
                .child(text.into()),
        )
}

// -- SettingsNavList ----------------------------------------------------------

/// One row of the 216px second-level nav (DST1–DST8). The selected row takes
/// `bg_raised` PLUS a hairline: on dark, raised is barely a step off sunken and
/// the fill alone does not read as a selection (desktop SPEC 暗色注意).
pub fn settings_nav_row(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    icon: Icon,
    label: gpui::SharedString,
    selected: bool,
) -> Stateful<Div> {
    let tint = if selected {
        theme.fg_base
    } else {
        theme.fg_muted
    };
    let row = div()
        .id(id)
        .h(px(SETTINGS_NAV_ROW_H))
        .px(px(12.))
        .rounded(px(10.))
        .flex()
        .items_center()
        .gap(px(12.))
        .cursor_pointer()
        .child(icon_img(icons, icon, false, tint, GLYPH_SM))
        .child(
            div()
                // Truncate inside the pill rather than draw outside it. At
                // `xlarge` the selected pill's last letter was being painted
                // past its own white background and over the column divider;
                // in German the whole label crossed into the panel beside it.
                .flex_1()
                .min_w(px(0.))
                .whitespace_nowrap()
                .truncate()
                .text_size(theme::text_row_sub())
                .text_color(tint)
                .when(selected, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                .child(label),
        );
    if selected {
        row.bg(theme.bg_raised)
            .border_1()
            .border_color(theme.divider)
    } else {
        row.hover(|el| el.bg(theme.bg_raised))
    }
}

// -- FormRow ------------------------------------------------------------------

/// A desktop panel row: label at the start, one control at the end, hairline
/// underneath. The same de-containered language the phone list uses, on its
/// side (DST2 / DST3).
pub fn form_row(theme: &Theme, label: gpui::SharedString, control: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_col()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(24.))
                .py(px(16.))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(label),
                )
                .child(
                    div()
                        .w(px(theme::SETTINGS_CONTROL_W))
                        .flex_none()
                        .flex()
                        .justify_end()
                        .child(control),
                ),
        )
        .child(div().h(px(1.)).bg(theme.divider))
}

/// The closed dropdown DST2/DST3 draw: current value plus a caret.
pub fn dropdown_trigger(theme: &Theme, icons: &mut IconCache, value: gpui::SharedString) -> Div {
    div()
        .w_full()
        .h(px(WALLET_CONTROL_H))
        .px(px(12.))
        .rounded(px(10.))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.divider)
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .whitespace_nowrap()
                .truncate()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .child(value),
        )
        .child(icon_img(
            icons,
            Icon::ChevronDown,
            false,
            theme.fg_subtle,
            GLYPH_SM,
        ))
}

/// One row of a dropdown's menu: the example, an optional note, whether it is
/// the one in force.
pub type MenuRow = (gpui::SharedString, Option<gpui::SharedString>, bool);

/// The menu an open dropdown drops (DST3). Rendered as an absolutely-positioned
/// child of the trigger's cell, because the desktop SPEC requires it to escape
/// the panel's clipping rather than push the rows below it down.
///
/// A picture of the control: the gallery's and the mock's. A menu whose rows
/// answer to a click is [`dropdown_menu_picks`].
pub fn dropdown_menu(theme: &Theme, icons: &mut IconCache, rows: &[MenuRow]) -> Div {
    menu_of(theme, icons, rows, None, true)
}

/// The same menu, live (spec 038 #E3): `on_pick` is handed the index of the
/// row the person chose.
pub fn dropdown_menu_picks(
    theme: &Theme,
    icons: &mut IconCache,
    rows: &[MenuRow],
    on_pick: impl Fn(usize, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Div {
    menu_of(theme, icons, rows, Some(Rc::new(on_pick)), true)
}

/// A live menu of NAMES rather than examples — languages, currencies (spec
/// 072) — set in the UI face, and scrolling past a fixed height: fifteen
/// languages would otherwise run off the bottom of the window.
pub fn dropdown_menu_choices(
    id: &'static str,
    theme: &Theme,
    icons: &mut IconCache,
    rows: &[MenuRow],
    on_pick: impl Fn(usize, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Stateful<Div> {
    menu_of(theme, icons, rows, Some(Rc::new(on_pick)), false)
        .id(id)
        .max_h(px(MENU_MAX_H))
        .overflow_y_scroll()
}

type PickAction = Rc<dyn Fn(usize, &mut gpui::Window, &mut gpui::App)>;

/// How far a dropped menu may reach before it scrolls instead of growing.
///
/// The menu is absolutely positioned at its trigger row, and the window's
/// minimum is 800pt tall, so an unbounded list runs off the bottom of the
/// window with no way to reach what is past the edge. The format menus are
/// three or four rows and never touch this; the currency menu is one row per
/// priceable currency — thirty of them — and would otherwise offer fourteen
/// codes nobody could pick.
const MENU_MAX_H: f32 = 400.;

fn menu_of(
    theme: &Theme,
    icons: &mut IconCache,
    rows: &[MenuRow],
    on_pick: Option<PickAction>,
    mono: bool,
) -> Div {
    let hover = theme.bg_sunken;
    // One id per menu, from the first row it draws — the same trick
    // `key_value_row` uses, and enough because only one dropdown is open at a
    // time and the four menus start with different words.
    let scroller_id = SharedString::from(format!(
        "dropdown-menu-{}",
        rows.first().map_or("", |(label, _, _)| label.as_ref())
    ));
    let mut col = div()
        .id(ElementId::from(scroller_id))
        .max_h(px(MENU_MAX_H))
        .overflow_y_scroll()
        .flex()
        .flex_col();
    let last = rows.len().saturating_sub(1);
    for (i, (label, note, selected)) in rows.iter().enumerate() {
        let mut row = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .py(px(10.))
            .child(
                div()
                    .when(mono, |el| el.font_family(theme::font_mono()))
                    .text_size(theme::text_row_sub())
                    .text_color(if *selected {
                        theme.accent
                    } else {
                        theme.fg_base
                    })
                    .when(*selected, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                    .child(label.clone()),
            )
            .child(div().flex_1());
        if let Some(note) = note {
            row = row.child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(note.clone()),
            );
        }
        if *selected {
            row = row.child(icon_img(icons, Icon::Check, false, theme.accent, 16.));
        }
        match on_pick.as_ref() {
            Some(pick) => {
                let pick = pick.clone();
                col = col.child(
                    row.id(ElementId::from(("dropdown-option", i)))
                        .cursor_pointer()
                        .hover(move |el| el.bg(hover))
                        .on_click(move |_, window, cx| pick(i, window, cx)),
                );
            }
            None => col = col.child(row),
        }
        if i != last {
            col = col.child(div().h(px(1.)).bg(theme.divider));
        }
    }
    div()
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .px(px(12.))
        .rounded(px(10.))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.divider)
        .shadow_lg()
        .flex()
        .flex_col()
        .child(col)
}

// -- SegmentedControl ---------------------------------------------------------

/// The product's ONE segmented control (design review 2026-07). Three-up for
/// the theme picker (its two-up avatar-style use was retired in spec 074); the
/// desktop reuses the same component the phone does.
pub fn segmented(
    theme: &Theme,
    icons: &mut IconCache,
    items: &[(Option<Icon>, gpui::SharedString)],
    selected: usize,
) -> Div {
    segmented_cells(theme, icons, items, selected, &mut |_, cell| {
        cell.into_any_element()
    })
}

/// The same control, with each cell handed to the caller before it is added.
///
/// The drawn version above takes no handler, which is how the Appearance panel
/// ended up showing two controls that could not be moved. A caller with a
/// `Context` wraps each cell in `.id().on_click(...)`; the gallery passes the
/// identity function and gets exactly the picture it always had.
pub fn segmented_cells(
    theme: &Theme,
    icons: &mut IconCache,
    items: &[(Option<Icon>, gpui::SharedString)],
    selected: usize,
    wrap: &mut dyn FnMut(usize, Div) -> gpui::AnyElement,
) -> Div {
    let mut row = div()
        .flex()
        .p(px(3.))
        .rounded(px(10.))
        .bg(theme.bg_sunken)
        .border_1()
        .border_color(theme.divider);
    for (i, (icon, label)) in items.iter().enumerate() {
        let is_selected = i == selected;
        let tint = if is_selected {
            theme.fg_base
        } else {
            theme.fg_muted
        };
        let mut cell = div()
            .flex_1()
            .h(px(32.))
            .px(px(8.))
            .rounded(px(8.))
            .flex()
            .items_center()
            .justify_center()
            .gap(px(6.));
        if is_selected {
            cell = cell.bg(theme.bg_raised);
        }
        if let Some(icon) = *icon {
            cell = cell.child(icon_img(icons, icon, false, tint, 14.));
        }
        let cell = cell.child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(tint)
                .when(is_selected, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                .child(label.clone()),
        );
        row = row.child(wrap(i, cell));
    }
    row
}

/// The same control, live (spec 072): `on_pick` is handed the index of the
/// cell the person chose. The cells are [`segmented`]'s own, armed, so the
/// picture and the control cannot drift apart.
pub fn segmented_picks(
    id: &'static str,
    theme: &Theme,
    icons: &mut IconCache,
    items: &[(Option<Icon>, gpui::SharedString)],
    selected: usize,
    on_pick: impl Fn(usize, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Div {
    let on_pick: PickAction = Rc::new(on_pick);
    segmented_cells(theme, icons, items, selected, &mut |i, cell| {
        let pick = on_pick.clone();
        cell.id((id, i))
            .cursor_pointer()
            .on_click(move |_, window, cx| pick(i, window, cx))
            .into_any_element()
    })
}

/// A ——●—— A, drawn only. The gallery still wants the picture; a real session
/// uses [`text_scale_stops`] so the thumb can be moved.
pub fn text_scale(theme: &Theme, steps: usize, index: usize) -> Div {
    text_scale_stops(theme, steps, index, &mut |_, stop| stop.into_any_element())
}

/// The same control, with each stop handed to the caller first — the live
/// panel wraps them in `.id().on_click(...)`.
///
/// The stop keeps a 20px-tall hit area whatever its dot is: a 4px target is
/// not a target. The dots themselves stay the mock's two sizes, because the
/// thumb has to read as the thumb at a glance.
pub fn text_scale_stops(
    theme: &Theme,
    steps: usize,
    index: usize,
    wrap: &mut dyn FnMut(usize, Div) -> gpui::AnyElement,
) -> Div {
    let mut track = div()
        .flex_1()
        .h(px(20.))
        .flex()
        .items_center()
        .justify_between();
    for i in 0..steps {
        let dot = if i == index {
            div().size(px(16.)).rounded_full().bg(theme.fg_muted)
        } else {
            div().size(px(4.)).rounded_full().bg(theme.outline_strong)
        };
        let stop = div()
            .h(px(20.))
            .w(px(20.))
            .flex()
            .items_center()
            .justify_center()
            .child(dot);
        track = track.child(wrap(i, stop));
    }
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(
            div()
                .text_size(theme::text_row_sub())
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child("A"),
        )
        .child(track)
        .child(
            div()
                .text_size(theme::text_panel_title())
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child("A"),
        )
}

/// The text-size control, live (spec 072): one stop per level the core ships
/// (`vela_core::prefs::TEXT_SCALE_LEVELS`), each its own target. The picture
/// above spaced dots along a track; a dot four pixels wide is not something a
/// person can click, so here every stop owns an equal share of the track.
pub fn text_scale_picks(
    theme: &Theme,
    steps: usize,
    index: usize,
    on_pick: impl Fn(usize, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Div {
    let on_pick: PickAction = Rc::new(on_pick);
    let mut track = div().flex_1().h(px(28.)).flex().items_center();
    for i in 0..steps {
        let pick = on_pick.clone();
        let hover = theme.fg_subtle;
        track = track.child(
            div()
                .id(("text-scale-stop", i))
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(move |_, window, cx| pick(i, window, cx))
                .child(if i == index {
                    div().size(px(16.)).rounded_full().bg(theme.fg_muted)
                } else {
                    div()
                        .size(px(6.))
                        .rounded_full()
                        .bg(theme.outline_strong)
                        .hover(move |el| el.bg(hover))
                }),
        );
    }
    div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(8.))
        .child(
            div()
                .text_size(theme::text_row_sub())
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child("A"),
        )
        .child(track)
        .child(
            div()
                .text_size(theme::text_panel_title())
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child("A"),
        )
}

// -- ChainMark / NetworkRow ---------------------------------------------------

/// A chain's circular avatar — one letter over its own brand colour.
pub fn chain_mark(letter: gpui::SharedString, color: u32, size: f32) -> Div {
    div()
        .size(px(size))
        .flex_none()
        .rounded_full()
        .bg(rgb(color))
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::text_label())
        .font_weight(gpui::FontWeight::BOLD)
        .text_color(gpui::white())
        .child(letter)
}

/// A chain's avatar where the chain is known: its logo — the one the wallet's
/// network filter and token badges draw (`marks::chain_logo_url`) — over the
/// lettermark, which is what shows while it loads, when it cannot, and for a
/// chain id the logo host could never name.
pub fn chain_logo_mark(chain_id: u64, letter: gpui::SharedString, color: u32, size: f32) -> Div {
    let url = u32::try_from(chain_id)
        .ok()
        .and_then(crate::marks::chain_logo_url);
    let Some(url) = url else {
        return chain_mark(letter, color, size);
    };
    let mark = move || chain_mark(letter.clone(), color, size).into_any_element();
    div().size(px(size)).flex_none().child(
        gpui::img(url)
            .size(px(size))
            .rounded_full()
            .with_loading(mark.clone())
            .with_fallback(mark),
    )
}

/// One network row (DST4): mark, name, chain-id line, an optional latency
/// pill, an optional 自定义 tag, and a disclosure caret. The desktop expands in
/// place rather than pushing a page, so the caret is a state and not a chevron.
#[allow(
    clippy::too_many_arguments,
    reason = "one row, one call site, all data"
)]
pub fn network_row(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    // `SharedString`, not `&'static str`: a live network's name and
    // lettermark come from the core at runtime (spec 030). The fixture
    // path passes the same constants it always did, now via `.into()`.
    chain_id: u64,
    letter: gpui::SharedString,
    color: u32,
    name: gpui::SharedString,
    meta: gpui::SharedString,
    badge: Option<&Pill>,
    tag: Option<gpui::SharedString>,
    removable: bool,
    expanded: bool,
    on_remove: Option<crate::contacts::components::MenuAction>,
) -> Stateful<Div> {
    let mut name_row = div().flex().items_center().gap(px(8.)).child(
        div()
            .text_size(theme::text_row_title())
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme.fg_base)
            .child(name),
    );
    if let Some(tag) = tag {
        name_row = name_row.child(
            div()
                .px(px(6.))
                .py(px(2.))
                .rounded(px(4.))
                .bg(theme.warning_soft)
                .text_size(theme::text_label())
                .text_color(theme.warning_base)
                .child(tag),
        );
    }

    let mut row = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(12.))
        .cursor_pointer()
        .child(chain_logo_mark(chain_id, letter, color, MARK))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(name_row)
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_subtle)
                        .child(meta),
                ),
        );
    if let Some(badge) = badge {
        row = row.child(status_pill(theme, badge));
    }
    if removable {
        // Its OWN target, and it stops there. Inside the row's own click this
        // glyph did what the row does — expand the card — so the one control
        // on this screen drawn as a destruction was the one control that did
        // something else entirely.
        row = row.child(
            div()
                .id("network-remove")
                .p(px(4.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|el| el.bg(theme.error_soft))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(icon_img(
                    icons,
                    Icon::Trash2,
                    false,
                    theme.error_base,
                    GLYPH_SM,
                ))
                .when_some(on_remove, |el, action| {
                    el.on_click(move |event, window, cx| action(event, window, cx))
                }),
        );
    }
    // The caret says what the tap DOES, which is why it flips rather than
    // pointing at the row: down opens this network's editor, up closes it. A
    // right-facing chevron would promise a page that does not exist here.
    row.child(icon_img(
        icons,
        if expanded {
            Icon::ChevronUp
        } else {
            Icon::ChevronDown
        },
        false,
        theme.fg_subtle,
        GLYPH_SM,
    ))
}

// -- UrlField -----------------------------------------------------------------

/// The same field, EDITABLE — the endpoint and provider-key surfaces.
///
/// 030 recorded these as "unfinished rather than blocked": the refusal behind
/// them was already proven, and `ui::text_field` already existed. This is that
/// field, wearing `url_field`'s clothes so a live panel and a mock one look
/// identical.
///
/// The value lives in the CORE — `NetView`'s endpoint drafts, which the machine
/// re-probes on every keystroke and persists on blur behind its own chain-id
/// gate. A shell-side copy would be a second opinion about what was typed.
#[allow(
    clippy::too_many_arguments,
    reason = "one field, one call site, all data"
)]
pub fn editable_url_field(
    id: impl Into<gpui::ElementId>,
    theme: &Theme,
    label: Option<gpui::SharedString>,
    value: &str,
    placeholder: gpui::SharedString,
    badge: Option<&Pill>,
    hint: Option<gpui::SharedString>,
    tone: Option<Tone>,
    focus: &gpui::FocusHandle,
    window: &gpui::Window,
    on_change: impl Fn(String, &mut gpui::Window, &mut gpui::App) + 'static,
    // Enter: "I am done with this field" — the same as leaving it (spec 072).
    on_enter: impl Fn(&mut gpui::Window, &mut gpui::App) + 'static,
) -> Div {
    let mut col = div().flex().flex_col().gap(px(8.)).on_key_down(
        move |event: &gpui::KeyDownEvent, window, cx| {
            if event.keystroke.key == "enter" {
                on_enter(window, cx);
            }
        },
    );
    if label.is_some() || badge.is_some() {
        let mut head = div().flex().items_center().justify_between().gap(px(8.));
        if let Some(label) = label {
            head = head.child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(label),
            );
        }
        if let Some(badge) = badge {
            head = head.child(status_pill(theme, badge));
        }
        col = col.child(head);
    }
    let strings = crate::ui::NameFieldStrings {
        label: gpui::SharedString::from(""),
        placeholder,
        helper: gpui::SharedString::from(""),
        too_long_hint: gpui::SharedString::from(""),
    };
    col = col.child(crate::ui::text_field(
        id,
        theme,
        &strings,
        value,
        // The ERROR border is the core's verdict, not a length check: an
        // endpoint that answered for another chain is the thing worth drawing
        // red, and the core is what decided that.
        matches!(tone, Some(Tone::Error)),
        false,
        focus,
        window,
        on_change,
    ));
    if let Some(hint) = hint {
        col = col.child(
            div()
                .text_size(theme::text_label())
                .line_height(px(16.))
                .text_color(theme.fg_subtle)
                .child(hint),
        );
    }
    col
}

/// A labelled mono field: a label row that may carry a latency pill, the value
/// in a sunken box, and an optional hint under it. Every endpoint on
/// DST4 / DST5 / DST6 / DSR1 is one of these.
#[allow(
    clippy::too_many_arguments,
    reason = "one field, one call site, all data"
)]
pub fn url_field(
    theme: &Theme,
    label: Option<gpui::SharedString>,
    value: gpui::SharedString,
    badge: Option<&Pill>,
    hint: Option<gpui::SharedString>,
    tone: Option<Tone>,
    // `action`: the blue action inside the box — DST5's 检查密钥 / 获取密钥.
    action: Option<gpui::SharedString>,
) -> Div {
    let border = match tone {
        Some(Tone::Error) => theme.error_base,
        Some(Tone::Ok) => theme.success_base,
        _ => theme.divider,
    };
    let mut col = div().flex().flex_col().gap(px(8.));
    if label.is_some() || badge.is_some() {
        let mut head = div().flex().items_center().justify_between().gap(px(8.));
        if let Some(label) = label {
            head = head.child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(label),
            );
        }
        if let Some(badge) = badge {
            head = head.child(status_pill(theme, badge));
        }
        col = col.child(head);
    }
    col = col.child(
        div()
            .h(px(WALLET_CONTROL_H))
            .px(px(12.))
            .rounded(px(10.))
            .bg(theme.bg_sunken)
            // A 1px border even at rest: on dark, sunken and base are one step
            // apart and the box would otherwise have no edge at all.
            .border_1()
            .border_color(border)
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .whitespace_nowrap()
                    .truncate()
                    .font_family(theme::font_mono())
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(value),
            )
            .when_some(action, |el, action| {
                el.child(
                    div()
                        .flex_none()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.info_base)
                        .child(action),
                )
            }),
    );
    if let Some(hint) = hint {
        col = col.child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(hint),
        );
    }
    col
}

// -- CheckList ----------------------------------------------------------------

/// DST4b's compatibility list: a green check or a red cross per requirement.
/// Both verdicts show all four rows — a shortened list would hide WHICH one
/// failed, and that is the only useful part of "incompatible".
pub fn check_list(
    theme: &Theme,
    icons: &mut IconCache,
    title: gpui::SharedString,
    items: &[(gpui::SharedString, bool)],
) -> Div {
    let mut col = div()
        .flex()
        .flex_col()
        .gap(px(10.))
        .p(px(16.))
        .rounded(px(10.))
        .bg(theme.bg_sunken)
        .border_1()
        .border_color(theme.divider)
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(title),
        );
    for (label, ok) in items {
        let (icon, tint) = if *ok {
            (Icon::Check, theme.success_base)
        } else {
            (Icon::X, theme.error_base)
        };
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(icon_img(icons, icon, false, tint, GLYPH_SM))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(label.clone()),
                ),
        );
    }
    col
}

// -- Storage ------------------------------------------------------------------

/// DST7's stacked bar. Shares, not pixels, so it tells the truth at any width.
pub fn storage_bar(theme: &Theme, segments: &[(f32, u32)]) -> Div {
    let mut bar = div()
        .w_full()
        .h(px(8.))
        .rounded(px(4.))
        .overflow_hidden()
        .bg(theme.bg_sunken)
        .flex();
    for (fraction, color) in segments {
        bar = bar.child(div().h_full().flex_grow(*fraction).bg(rgb(*color)));
    }
    bar
}

/// One storage group. The group label carries the consequence — "清除后无法
/// 找回" against "清除后自动重建" — which is why the same word 清除 is red in
/// the first group and plain in the second.
pub fn storage_group(theme: &Theme, group: &StorageGroup) -> Div {
    storage_group_with("storage-action", theme, group, Vec::new())
}

/// The same group with its row actions armed: `actions[i]` is what row `i`'s
/// action does, `None` (or a missing entry) leaves it drawn and inert — which
/// is what every mock row is.
pub fn storage_group_with(
    // Each group's actions under their own name: two groups on one page with
    // the same element ids would share hover and click state.
    key: &'static str,
    theme: &Theme,
    group: &StorageGroup,
    actions: Vec<Option<crate::flows::panels::Click>>,
) -> Div {
    let mut actions = actions.into_iter();
    let mut col = div().flex().flex_col().pt(px(16.)).child(
        div()
            .pb(px(4.))
            .text_size(theme::text_label())
            .text_color(theme.fg_subtle)
            .child(group.label.clone()),
    );
    for (index, item) in group.items.iter().enumerate() {
        let action_tint = if item.destructive {
            theme.error_base
        } else {
            theme.fg_muted
        };
        col = col.child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .py(px(12.))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .whitespace_nowrap()
                                .truncate()
                                .text_size(theme::text_row_title())
                                .text_color(theme.fg_base)
                                .child(item.label.clone()),
                        )
                        .child(
                            div()
                                .text_size(theme::text_label())
                                .text_color(theme.fg_subtle)
                                .child(item.meta.clone()),
                        )
                        .child(crate::flows::panels::clickable(
                            ElementId::from((key, index)),
                            actions.next().flatten(),
                            div()
                                .id(ElementId::from(SharedString::from(format!(
                                    "storage-clear-{}",
                                    item.id
                                ))))
                                .text_size(theme::text_row_sub())
                                .text_color(action_tint)
                                .when_some(actions.next().flatten(), |el, act| {
                                    // The action was built for THIS row, so it
                                    // carries its own id; a `Click` takes the
                                    // event, as every other one here does.
                                    el.cursor_pointer()
                                        .hover(move |el| el.opacity(0.7))
                                        .on_click(move |event, window, cx| act(event, window, cx))
                                })
                                .child(item.action.clone()),
                        )),
                )
                .child(div().h(px(1.)).bg(theme.divider)),
        );
    }
    col
}

// -- KeyValueRow --------------------------------------------------------------

/// DST8's technical-detail and link rows: label at the start, value at the end,
/// mono where the value is an identifier, external glyph where it is a place.
/// `link` is what an external row DOES. A row wearing the external-link glyph
/// and opening nothing is the worst of both: it advertises a place and then
/// refuses to go there. Where there is no link the row keeps the house rule and
/// drops the pointer and the hover, so it reads as text rather than a control.
pub fn key_value_row(
    theme: &Theme,
    icons: &mut IconCache,
    label: gpui::SharedString,
    value: gpui::SharedString,
    mono: bool,
    external: bool,
    link: Option<gpui::SharedString>,
) -> Div {
    // A stable id per row, from the label it draws. gpui needs one before the
    // row can take a click at all, and the labels on this page are unique.
    let label_id = SharedString::from(format!("settings-kv-{label}"));
    let mut row = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(12.))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_size(theme::text_row_sub())
                .text_color(if external {
                    theme.fg_base
                } else {
                    theme.fg_muted
                })
                .when(external, |el| el.font_weight(gpui::FontWeight::SEMIBOLD))
                .child(label),
        )
        .child(
            div()
                .whitespace_nowrap()
                .truncate()
                .text_size(theme::text_row_sub())
                .text_color(if external {
                    theme.fg_subtle
                } else {
                    theme.fg_base
                })
                .when(mono, |el| el.font_family(theme::font_mono()))
                .child(value),
        );
    if external {
        row = row.child(icon_img(
            icons,
            Icon::ExternalLink,
            false,
            theme.fg_subtle,
            14.,
        ));
    }
    let hover = theme.bg_sunken;
    let row = row
        .id(ElementId::from(label_id))
        .when_some(link, |el, url| {
            el.cursor_pointer()
                .hover(move |el| el.bg(hover))
                .on_click(move |_, _, cx| cx.open_url(&url))
        });
    div()
        .flex()
        .flex_col()
        .child(row)
        .child(div().h(px(1.)).bg(theme.divider))
}

// -- DangerCard ---------------------------------------------------------------

/// DST1's 清理数据 card — the one thing in settings drawn as a bordered box
/// rather than a hairline row, because it is the only action on the screen
/// that cannot be undone.
///
/// `on_click` arrived with spec 081 FR-017, and its absence is the defect:
/// this card was drawn from the first day with no handler at all, so the most
/// destructive-looking control in the app was the one control that did
/// nothing. `None` still means a board — the gallery draws this card too —
/// and only a live account panel passes a handler.
pub fn danger_card(
    theme: &Theme,
    title: gpui::SharedString,
    subtitle: gpui::SharedString,
    action: gpui::SharedString,
    on_click: Option<crate::flows::panels::Click>,
) -> gpui::AnyElement {
    let card = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(16.))
        .rounded(px(10.))
        .bg(theme.error_soft)
        .border_1()
        .border_color(theme.error_base)
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
                        .text_color(theme.error_base)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_muted)
                        .child(subtitle),
                ),
        )
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.error_base)
                .child(action),
        );
    match on_click {
        Some(on_click) => card
            .id("settings-erase-card")
            .cursor_pointer()
            .on_click(on_click)
            .into_any_element(),
        None => card.into_any_element(),
    }
}

// -- ConfirmSheet -------------------------------------------------------------

/// What a destructive action asks before it happens (spec 072 FR-010): a
/// title naming what goes, the consequence, an optional quieter note (what is
/// not lost) and red callout (what is), and the answers.
pub struct ConfirmCopy {
    pub title: gpui::SharedString,
    pub body: gpui::SharedString,
    pub callout: Option<gpui::SharedString>,
    pub note: Option<gpui::SharedString>,
    pub confirm: gpui::SharedString,
    /// The stacked "keep it" answer under the confirm — the settings sheets'.
    /// `None` is the contacts question, where the dialog's ✕ is the refusal
    /// and the one answer sits at the row's end.
    pub cancel: Option<gpui::SharedString>,
    /// Red for what cannot be undone; the accent for what rebuilds itself.
    pub danger: bool,
}

/// The body every question is drawn with, inside `ui::dialog` (whose title is
/// `copy.title`).
///
/// With a cancel it is the web's `settings/ui/ConfirmSheet.svelte`: the body
/// at 15 in the base colour, the note at 13 subtle, the callout, then the
/// answer and "cancel" stacked 12 apart at full width. Without one it is the
/// contacts route's dialog: the body at 13 muted and one answer at the end of
/// the row.
pub fn confirm_sheet(
    theme: &Theme,
    copy: ConfirmCopy,
    on_confirm: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> Div {
    use crate::flows::components::{accent_button, danger_button, secondary_button};

    let answer = if copy.danger {
        danger_button(theme, copy.confirm)
    } else {
        accent_button(theme, copy.confirm)
    };
    let answer = answer
        .id("confirm-accept")
        .cursor_pointer()
        .on_click(on_confirm);

    let Some(cancel) = copy.cancel else {
        return div()
            .flex()
            .flex_col()
            .child(crate::ui::dialog::dialog_body(theme, copy.body))
            .child(crate::ui::dialog::dialog_actions().child(div().child(answer.w_auto())));
    };

    let mut sheet = div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .pt(px(8.))
        .pb(px(16.))
        .child(
            div()
                .text_size(theme::text_row_title())
                .line_height(gpui::relative(1.4))
                .text_color(theme.fg_base)
                .child(copy.body),
        );
    if let Some(note) = copy.note {
        sheet = sheet.child(
            div()
                .text_size(theme::text_row_sub())
                .line_height(gpui::relative(1.4))
                .text_color(theme.fg_subtle)
                .child(note),
        );
    }
    if let Some(callout) = copy.callout {
        sheet = sheet.child(
            div()
                .p(px(12.))
                .rounded(px(10.))
                .bg(theme.error_soft)
                .text_size(theme::text_row_sub())
                .line_height(theme::line_height_body())
                .text_color(theme.error_base)
                .child(callout),
        );
    }
    sheet.child(
        div().flex().flex_col().gap(px(12.)).child(answer).child(
            secondary_button(theme, cancel)
                .id("confirm-cancel")
                .cursor_pointer()
                .on_click(on_cancel),
        ),
    )
}

// -- RpcBanner ----------------------------------------------------------------

/// DSR1's amber banner: the count of unreachable networks, then one chip per
/// network with its own 修复. Per-chain rather than one global button, because
/// the fix IS per chain — a shared button would have to ask which one first.
pub fn rpc_banner(
    theme: &Theme,
    icons: &mut IconCache,
    text: gpui::SharedString,
    // Owned strings, not `&'static str`: the chips are a chain list, and since
    // 031 that list can come from a live `BalanceView` — a network the person
    // added has a name nobody could have written into this binary.
    chips: Vec<(
        gpui::SharedString,
        u32,
        gpui::SharedString,
        gpui::SharedString,
        Option<crate::flows::panels::Click>,
    )>,
) -> Div {
    let mut row = div().flex().flex_wrap().gap(px(8.));
    for (index, (letter, color, name, action, on_click)) in chips.into_iter().enumerate() {
        // The chip IS the fix affordance — it names a chain and the thing to do
        // about it, so clicking it must open that chain's editor rather than
        // some other one's.
        let chip = div()
            .id(gpui::ElementId::from(("rpc-banner-chip", index)))
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(8.))
            .py(px(6.))
            .rounded_full()
            .bg(theme.bg_base)
            .child(chain_mark(letter, color, 20.))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(name),
            )
            // The only accent on this banner: the thing that fixes it.
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.accent)
                    .child(action),
            );
        row = row.child(match on_click {
            Some(on_click) => chip.cursor_pointer().on_click(on_click).into_any_element(),
            None => chip.into_any_element(),
        });
    }
    div()
        .flex()
        .flex_col()
        .gap(px(12.))
        .p(px(16.))
        .rounded(px(10.))
        .bg(theme.warning_soft)
        .border_1()
        .border_color(theme.warning_base)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(icon_img(
                    icons,
                    Icon::TriangleAlert,
                    false,
                    theme.warning_base,
                    GLYPH_SM,
                ))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.warning_base)
                        .child(text),
                ),
        )
        .child(row)
}
