//! `VelaButton` — the CTA variants of the design system, on theme tokens
//! only. Components receive already-resolved strings; they know nothing about
//! i18n (spec 007 FR-009).

use crate::theme::{self, BTN_H_PRIMARY, BTN_H_SECONDARY, OPACITY_DISABLED, Theme};
use crate::ui::spinner;
use gpui::{
    AnyElement, App, ClickEvent, Div, ElementId, FontWeight, InteractiveElement, IntoElement,
    ParentElement, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};

/// What the button is doing, which is three things and not two.
///
/// `Busy` is NOT `Disabled`: the action is running and this button is what the
/// person is waiting on. It keeps full emphasis and turns a spinner where its
/// label was, because a dimmed control reads as "unavailable" — the one thing
/// "working" must never look like (DESIGN_SYSTEM.md, "Loading state").
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Enabled,
    Disabled,
    Busy,
}

impl ButtonState {
    /// The two-state callers' spelling.
    pub fn from_enabled(enabled: bool) -> Self {
        if enabled {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Accent-filled, inverse label. Reserved for the screen's one primary action.
    Primary,
    /// Outlined on the raised surface, base label (dark label per DV-001).
    Secondary,
    /// The flow outcome stack's solid row (spec 014): filled with the `well`
    /// surface, base label — the mock's dark rows, NOT the outlined welcome
    /// secondary.
    Row,
}

/// Returns `Stateful<Div>` rather than `impl IntoElement`: under edition 2024
/// RPIT would capture the theme borrow and forbid a second call in one render.
pub fn vela_button(
    id: impl Into<ElementId>,
    variant: ButtonVariant,
    label: SharedString,
    theme: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    vela_button_opts(id, variant, label, true, theme, on_click)
}

/// `vela_button` plus the disabled treatment (spec 014 form CTA): the same
/// fill at `OPACITY_DISABLED` emphasis — mock A1's dimmed accent, never a gray
/// swap — with the pointer affordance and the click handler withheld.
///
/// The shape is v2's: a 12px rectangle. **There is no capsule anywhere in
/// design/onboarding-new** — every button in it, on every screen and in the
/// sheet, is `border-radius: 12px` — so the pill went with v1 rather than
/// surviving as a second shape nothing calls for.
pub fn vela_button_opts(
    id: impl Into<ElementId>,
    variant: ButtonVariant,
    label: SharedString,
    enabled: bool,
    theme: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    vela_button_state(
        id,
        variant,
        label,
        ButtonState::from_enabled(enabled),
        theme,
        on_click,
    )
}

/// `vela_button_opts` over the full three-state vocabulary.
pub fn vela_button_state(
    id: impl Into<ElementId>,
    variant: ButtonVariant,
    label: SharedString,
    state: ButtonState,
    theme: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let height = match variant {
        ButtonVariant::Primary => BTN_H_PRIMARY,
        ButtonVariant::Secondary | ButtonVariant::Row => BTN_H_SECONDARY,
    };
    // The height is a MINIMUM: long-locale labels wrap inside a
    // width-constrained, centered block and grow the row instead of
    // escaping the button (the radius is fixed, so growth is safe).
    let label_block = div().w_full().min_w(px(0.)).text_center().child(label);
    let base = div()
        .id(id)
        .min_h(px(height))
        .w_full()
        .flex_none()
        .rounded(px(theme::RADIUS_CTA))
        .flex()
        .items_center()
        .justify_center()
        .px(px(theme::BTN_PAD_X))
        .py(px(theme::BTN_PAD_Y))
        .text_size(theme::text_cta())
                // SEMIBOLD, as the web's `Button.svelte` (`--weight-semibold`): one
        // notch lighter than the BOLD this carried — spec 038 finding 8.
        .font_weight(FontWeight::SEMIBOLD);

    finish(base, variant, label_block, state, theme, on_click)
}

/// The welcome pair: the same rectangle as every other button, except that it
/// sits beside its sibling at ITS LABEL'S WIDTH rather than filling the
/// column. A desktop dialog sizes a button to what it says; a full-width
/// button is a phone's answer to a thumb, and two of them stacked is what made
/// the welcome read as a phone screen.
///
/// Same fills, hovers and disabled treatment as `vela_button` — only the
/// sizing differs, which is why they share `finish` below.
pub fn welcome_cta(
    id: impl Into<ElementId>,
    variant: ButtonVariant,
    label: SharedString,
    enabled: bool,
    theme: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    welcome_cta_state(
        id,
        variant,
        label,
        ButtonState::from_enabled(enabled),
        theme,
        on_click,
    )
}

/// `welcome_cta` over the full three-state vocabulary.
pub fn welcome_cta_state(
    id: impl Into<ElementId>,
    variant: ButtonVariant,
    label: SharedString,
    state: ButtonState,
    theme: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let label_block = div().min_w(px(0.)).text_center().child(label);
    let base = div()
        .id(id)
        .flex_none()
        .min_w(px(theme::CTA_MIN_W))
        .min_h(px(theme::CTA_H))
        .rounded(px(theme::RADIUS_CTA))
        .flex()
        .items_center()
        .justify_center()
        .px(px(theme::CTA_PAD_X))
        .py(px(theme::BTN_PAD_Y))
        .text_size(theme::text_cta())
                // SEMIBOLD, as the web's `Button.svelte` (`--weight-semibold`): one
        // notch lighter than the BOLD this carried — spec 038 finding 8.
        .font_weight(FontWeight::SEMIBOLD);

    finish(base, variant, label_block, state, theme, on_click)
}

/// The part every button shape shares: the variant's fill, its hover/active
/// pair, and either the click handler or the disabled dimming.
fn finish(
    base: Stateful<Div>,
    variant: ButtonVariant,
    label_block: Div,
    state: ButtonState,
    theme: &Theme,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    if state == ButtonState::Disabled {
        let styled = match variant {
            ButtonVariant::Primary => base.bg(theme.accent).text_color(theme.fg_inverse),
            ButtonVariant::Secondary => base
                .text_color(theme.fg_base)
                .border_1()
                .border_color(theme.divider),
            ButtonVariant::Row => base.bg(theme.bg_well).text_color(theme.fg_base),
        };
        return styled.opacity(OPACITY_DISABLED).child(label_block);
    }

    // Busy: the fill at full strength, the label swapped for the turning arc,
    // and no click handler — a second press cannot start the work twice.
    if state == ButtonState::Busy {
        let (styled, arc) = match variant {
            ButtonVariant::Primary => (
                base.bg(theme.accent).text_color(theme.fg_inverse),
                theme.fg_inverse,
            ),
            ButtonVariant::Secondary => (
                base.text_color(theme.fg_base)
                    .border_1()
                    .border_color(theme.divider),
                theme.fg_base,
            ),
            ButtonVariant::Row => (
                base.bg(theme.bg_well).text_color(theme.fg_base),
                theme.fg_base,
            ),
        };
        return styled.child(busy_arc(arc));
    }

    let styled = match variant {
        ButtonVariant::Primary => {
            let (hover, active) = (theme.accent_hover, theme.accent_active);
            base.bg(theme.accent)
                .text_color(theme.fg_inverse)
                .hover(move |s| s.bg(hover))
                .active(move |s| s.bg(active))
        }
        ButtonVariant::Secondary => {
            // Transparent on a hairline, not white-on-a-dark-outline: v2 draws
            // the secondary as `background: transparent; border: 1px solid
            // var(--border)`, and `divider` is the token that equals
            // `--border` in BOTH modes (#ECEBE4 / #2C2C28). `outline_strong`
            // is v1's heavy brown edge and belongs to the old capsule.
            let (hover_bg, active_bg) = (theme.bg_sunken, theme.divider);
            base.text_color(theme.fg_base)
                .border_1()
                .border_color(theme.divider)
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg))
        }
        ButtonVariant::Row => {
            let (hover_bg, active_bg) = (theme.divider, theme.bg_sunken);
            base.bg(theme.bg_well)
                .text_color(theme.fg_base)
                .hover(move |s| s.bg(hover_bg))
                .active(move |s| s.bg(active_bg))
        }
    };

    styled
        .cursor_pointer()
        .child(label_block)
        .on_click(on_click)
}

/// The busy spinner, sized to stand where the label stood.
fn busy_arc(color: gpui::Hsla) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .child(spinner::spinner(
            color,
            px(theme::BTN_SPINNER),
            px(theme::SPINNER_STROKE),
        ))
        .into_any_element()
}
