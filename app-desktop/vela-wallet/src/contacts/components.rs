//! Contacts visuals (spec 018 FR-001): theme + resolved strings + fixture
//! models in, elements out. The same contract `wallet::components` follows —
//! no i18n keys, no page state, no window management.
//!
//! Everything spec 015 already built is imported, never re-implemented
//! (SC-006): `identicon_avatar`, `activity_row`, `empty_state`, `action_pill`,
//! `icon_img` and the third-column scaffold all come from
//! `crate::wallet::components`.

use crate::contacts::model::ContactRowModel;
use gpui::{
    Div, ElementId, InteractiveElement as _, IntoElement, ParentElement, SharedString, Stateful,
    StatefulInteractiveElement as _, Styled, div, px,
};

use crate::icons::{Icon, IconCache};
use crate::identicon::IdenticonCache;
use crate::theme::{
    self, CONTACTS_BUTTON_H, CONTACTS_MENU_ROW_H, CONTACTS_MENU_W, CONTACTS_RAIL_LABEL_H,
    CONTACTS_RAIL_ROW_H, CONTACTS_ROW_AVATAR, CONTACTS_SEARCH_W, Theme, WALLET_CONTROL_H,
};
use crate::wallet::components::{empty_state, icon_img, identicon_avatar};

use super::fixtures::MenuModel;

/// Leading glyph size inside menu/rail rows (M1/M2 anatomy).
const GLYPH_SM: f32 = 16.;
/// Trailing/standalone icon-button glyph (⋯, ✕, copy).
const GLYPH_MD: f32 = 18.;

// -- ContactRow ---------------------------------------------------------------

/// One contact row (DC1/DC2/DC4): identicon seeded by the FULL address, name
/// that truncates on one line, middle-truncated address in mono underneath.
/// `selected` paints the same raised wash hover does — the selected row is the
/// one the third column shows (desktop SPEC).
pub fn contact_row(
    id: impl Into<ElementId>,
    theme: &Theme,
    identicons: &mut IdenticonCache,
    // The MODEL, not the fixture: a live row's name and address come from
    // the core at runtime (spec 030). The mock path passes the same values
    // through `fixtures::rows()`.
    contact: &ContactRowModel,
    selected: bool,
) -> Stateful<Div> {
    let row = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(10.))
        .px(px(8.))
        .mx(px(-8.))
        .rounded(px(10.))
        .cursor_pointer()
        .child(identicon_avatar(
            identicons,
            &contact.address_full,
            CONTACTS_ROW_AVATAR,
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
                        .whitespace_nowrap()
                        .truncate()
                        .child(contact.name.clone()),
                )
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_subtle)
                        .whitespace_nowrap()
                        .truncate()
                        .child(contact.address_display.clone()),
                ),
        );
    if selected {
        row.bg(theme.bg_raised)
    } else {
        row.hover(|el| el.bg(theme.bg_raised))
    }
}

/// The hairline the mocks draw between contact rows inside a letter section.
pub fn row_divider(theme: &Theme) -> Div {
    div().h(px(1.)).bg(theme.divider)
}

// -- AlphaSectionList ---------------------------------------------------------

/// Letter section header (DC1): the uppercase letter plus a hairline that runs
/// to the end of the list column.
pub fn section_letter(theme: &Theme, letter: gpui::SharedString) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .pt(px(14.))
        .pb(px(6.))
        .child(
            div()
                .text_size(theme::text_label())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_subtle)
                .child(SharedString::from(letter)),
        )
        .child(div().flex_1().h(px(1.)).bg(theme.divider))
}

// -- GroupRail ----------------------------------------------------------------

/// The three rail-row treatments the desktop SPEC names. `Hover` is the
/// pointer state and lives on `Default`; `DropTarget` is the drag-over variant
/// (visual only — drag itself is out of scope per spec Assumptions).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RailState {
    Default,
    Selected,
    DropTarget,
}

/// One rail row (DC1/DC3/DC4): optional leading glyph, label, optional
/// trailing count, in one of the three `RailState` treatments.
pub fn rail_row(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    icon: Option<Icon>,
    label: SharedString,
    count: Option<u32>,
    state: RailState,
) -> Stateful<Div> {
    let selected = state == RailState::Selected;
    let fg = if selected {
        theme.fg_base
    } else {
        theme.fg_muted
    };
    let mut row = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(10.))
        .h(px(CONTACTS_RAIL_ROW_H))
        .px(px(12.))
        .rounded(px(8.))
        .cursor_pointer()
        .text_size(theme::text_row_sub())
        .text_color(fg);
    if let Some(icon) = icon {
        row = row.child(icon_img(icons, icon, false, fg, GLYPH_SM));
    }
    row = row.child(
        div()
            .flex_1()
            .min_w(px(0.))
            .whitespace_nowrap()
            .truncate()
            .child(label),
    );
    if let Some(count) = count {
        row = row.child(
            div()
                .text_color(theme.fg_subtle)
                .child(SharedString::from(count.to_string())),
        );
    }
    if selected {
        row = row
            .bg(theme.bg_raised)
            .font_weight(gpui::FontWeight::SEMIBOLD);
    } else {
        row = row.hover(|el| el.bg(theme.bg_raised));
    }
    if state == RailState::DropTarget {
        row = row
            .bg(theme.bg_raised)
            .border_1()
            .border_color(theme.accent);
    } else {
        row = row.border_1().border_color(gpui::transparent_black());
    }
    row
}

/// The `分组` caption above the group rows. Its fixed height is what lets the
/// page place DC6's context menu on the 家人 row without measuring.
pub fn rail_label(theme: &Theme, label: SharedString) -> Div {
    div()
        .h(px(CONTACTS_RAIL_LABEL_H))
        .flex()
        .items_end()
        .px(px(12.))
        .pb(px(6.))
        .text_size(theme::text_label())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.fg_subtle)
        .child(label)
}

// -- DropdownMenu / ContextMenu (one visual family — FR-007) ------------------

/// The menu card behind both DC5's header dropdown and DC6's group context
/// menu (M1/M2): raised surface, hairline border, 44 px icon+label rows, an
/// optional divider and a destructive row in `error_base`.
/// One menu row's action. Empty means the menu is a picture, which is what the
/// gallery boards want and what a row with nowhere to go should be.
pub type MenuAction = Box<dyn Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static>;

/// The anchored menu (M1/M2), with an optional action per row.
///
/// `actions` is positional: entry `i` belongs to `menu.items[i]`, and a shorter
/// list simply leaves the rest inert. 030 recorded this component as blocked
/// because its items "carry no action"; giving them an OPTIONAL one changes
/// nothing about how the gallery draws them, which is what makes it additive
/// rather than the design decision it was taken for.
pub fn menu_card(
    theme: &Theme,
    icons: &mut IconCache,
    menu: &MenuModel,
    actions: Vec<Option<MenuAction>>,
) -> Div {
    let mut card = div()
        .w(px(CONTACTS_MENU_W))
        .py(px(6.))
        .rounded(px(12.))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.divider)
        .shadow_lg()
        .flex()
        .flex_col();
    let mut actions = actions.into_iter();
    for (i, item) in menu.items.iter().enumerate() {
        let fg = if item.destructive {
            theme.error_base
        } else {
            theme.fg_base
        };
        let row = div()
            .id(gpui::ElementId::from(("menu-row", i)))
            .flex()
            .items_center()
            .gap(px(12.))
            .h(px(CONTACTS_MENU_ROW_H))
            .px(px(14.))
            .text_size(theme::text_row_sub())
            .text_color(fg)
            .child(icon_img(icons, item.icon, false, fg, GLYPH_SM))
            .child(item.label.clone());
        card = card.child(match actions.next().flatten() {
            Some(action) => row
                .cursor_pointer()
                .hover(|el| el.bg(theme.bg_sunken))
                .on_click(action)
                .into_any_element(),
            None => row.into_any_element(),
        });
        if menu.divider_after == Some(i) {
            card = card.child(div().h(px(1.)).my(px(4.)).bg(theme.divider));
        }
    }
    card
}

// -- GroupChips ---------------------------------------------------------------

/// A membership pill on the contact detail (DC2: `家人`).
pub fn group_chip(theme: &Theme, label: SharedString) -> Div {
    div()
        .h(px(24.))
        .px(px(10.))
        .rounded(px(12.))
        .flex()
        .items_center()
        .bg(theme.bg_sunken)
        .text_size(theme::text_label())
        .text_color(theme.fg_muted)
        .child(label)
}

/// The trailing `+ 分组` add chip (dashed-equivalent: outlined, not filled).
pub fn add_chip(theme: &Theme, icons: &mut IconCache, label: SharedString) -> Div {
    div()
        .h(px(24.))
        .px(px(10.))
        .rounded(px(12.))
        .flex()
        .items_center()
        .gap(px(4.))
        .border_1()
        .border_color(theme.divider)
        .text_size(theme::text_label())
        .text_color(theme.fg_subtle)
        .child(icon_img(icons, Icon::Plus, false, theme.fg_subtle, 12.))
        .child(label)
}

// -- AddressBlock -------------------------------------------------------------

/// `地址` label + the full address in mono (one line on desktop) + a copy
/// affordance kept vertically centred against it (DC2).
pub fn address_block(
    theme: &Theme,
    icons: &mut IconCache,
    label: SharedString,
    address: SharedString,
    // `None` in the gallery, where there is no address worth putting on a real
    // clipboard. A live block copies.
    on_copy: Option<MenuAction>,
) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(label),
                )
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_label())
                        .text_color(theme.fg_base)
                        .child(address),
                ),
        )
        .child(match on_copy {
            Some(on_copy) => {
                icon_button("copy-contact-address", theme, icons, Icon::Copy).on_click(on_copy)
            }
            None => icon_button("copy-contact-address", theme, icons, Icon::Copy),
        })
}

// -- GhostAddRow --------------------------------------------------------------

/// Muted `+` circle + label, never raised (DC4's 添加成员).
pub fn ghost_add_row(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    label: SharedString,
) -> Stateful<Div> {
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
        .child(
            div()
                .w(px(CONTACTS_ROW_AVATAR))
                .h(px(CONTACTS_ROW_AVATAR))
                .flex_none()
                .rounded(px(CONTACTS_ROW_AVATAR / 2.))
                .border_1()
                .border_color(theme.divider)
                .flex()
                .items_center()
                .justify_center()
                .child(icon_img(
                    icons,
                    Icon::Plus,
                    false,
                    theme.fg_subtle,
                    GLYPH_MD,
                )),
        )
        .child(
            div()
                .text_size(theme::text_row_title())
                .text_color(theme.fg_muted)
                .child(label),
        )
}

// -- Buttons ------------------------------------------------------------------

/// Accent CTA (DC3 添加联系人, DC4 群发转账).
pub fn accent_button(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    icon: Option<Icon>,
    label: SharedString,
) -> Stateful<Div> {
    let mut button = div()
        .id(id)
        .h(px(CONTACTS_BUTTON_H))
        .px(px(20.))
        .rounded(px(CONTACTS_BUTTON_H / 2.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .cursor_pointer()
        .bg(theme.accent)
        .hover(|el| el.bg(theme.accent_hover))
        .text_size(theme::text_row_title())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.fg_inverse);
    if let Some(icon) = icon {
        button = button.child(icon_img(icons, icon, false, theme.fg_inverse, GLYPH_SM));
    }
    button.child(label)
}

/// Outline CTA (DC1/DC3 添加联系人 in the header, DC3 从文件导入).
pub fn outline_button(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    icon: Option<Icon>,
    label: SharedString,
) -> Stateful<Div> {
    let mut button = div()
        .id(id)
        .h(px(CONTACTS_BUTTON_H))
        .px(px(18.))
        .rounded(px(CONTACTS_BUTTON_H / 2.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(8.))
        .cursor_pointer()
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.outline_strong)
        .hover(|el| el.bg(theme.bg_sunken))
        .text_size(theme::text_row_title())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.fg_base);
    if let Some(icon) = icon {
        button = button.child(icon_img(icons, icon, false, theme.fg_base, GLYPH_SM));
    }
    button.child(label)
}

/// Square icon button (header ⋯, address copy, group-header ⋯).
pub fn icon_button(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    icon: Icon,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(CONTACTS_BUTTON_H))
        .h(px(CONTACTS_BUTTON_H))
        .flex_none()
        .rounded(px(10.))
        .flex()
        .items_center()
        .justify_center()
        .cursor_pointer()
        .border_1()
        .border_color(theme.divider)
        .hover(|el| el.bg(theme.bg_raised))
        .child(icon_img(icons, icon, false, theme.fg_muted, GLYPH_MD))
}

/// Destructive text action (DC2 footer 删除联系人).
pub fn destructive_text_button(
    id: impl Into<ElementId>,
    theme: &Theme,
    label: SharedString,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .cursor_pointer()
        .text_size(theme::text_row_sub())
        .text_color(theme.error_base)
        .child(label)
}

/// Quiet text action with a leading glyph (DC2 footer 编辑, DC2 查看全部往来).
pub fn text_action(
    id: impl Into<ElementId>,
    theme: &Theme,
    icons: &mut IconCache,
    icon: Option<Icon>,
    label: SharedString,
) -> Stateful<Div> {
    let mut action = div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(6.))
        .cursor_pointer()
        .text_size(theme::text_row_sub())
        .text_color(theme.fg_muted);
    if let Some(icon) = icon {
        action = action.child(icon_img(icons, icon, false, theme.fg_muted, 14.));
    }
    action.child(label)
}

// -- SearchField --------------------------------------------------------------

/// Page-local search (DC1 header) with the ⌘F badge. Visual only in this
/// feature — the fixtures ship any filtered list.
pub fn search_field(theme: &Theme, icons: &mut IconCache, placeholder: SharedString) -> Div {
    div()
        .w(px(CONTACTS_SEARCH_W))
        .flex_none()
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
                .child("⌘F"),
        )
}

// -- EmptyStateCTA ------------------------------------------------------------

/// Spec-015 `empty_state` (icon tile + title + caption) extended with the CTA
/// pair the contacts mocks add: accent 添加联系人 + outline 从文件导入,
/// inline on desktop (DC3).
pub fn empty_state_cta(
    theme: &Theme,
    icons: &mut IconCache,
    title: SharedString,
    caption: SharedString,
    primary: SharedString,
    secondary: SharedString,
) -> Div {
    let artwork = empty_state(theme, icons, Icon::UsersRound, title, caption);
    div()
        .w(px(360.))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .text_center()
        .child(artwork)
        .child(
            div()
                .flex()
                .gap(px(12.))
                .child(accent_button(
                    "empty-add-contact",
                    theme,
                    icons,
                    None,
                    primary,
                ))
                .child(outline_button(
                    "empty-import-file",
                    theme,
                    icons,
                    None,
                    secondary,
                )),
        )
}
