//! The three dialogs that belong to the CABLE.
//!
//! Every other Vela client hands its ceremony to a system passkey sheet, and
//! that sheet says all of this on the app's behalf: your key is blinking, type
//! its PIN, which of these wallets did you mean. The desktop has no such sheet
//! — the app IS the sheet — so these three exist here and nowhere else in the
//! codebase.
//!
//! They are free functions over `(theme, loc, request)` rather than methods on
//! a screen, for one reason: the gallery renders the SAME cards the flow does.
//! Each of these states needs a particular authenticator in a particular
//! condition to reach on purpose — a key with a PIN, a key with a sensor, two
//! keys holding four wallets — which makes them the states most likely to be
//! reviewed once and then drift. A gallery that drew its own copy would drift
//! with them.

use std::cell::RefCell;

use gpui::{
    App, Div, FontWeight, ImageSource, InteractiveElement as _, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, Window, div, img, px, rgb,
};
use qrcode::{Color as QrColor, QrCode};

use vela_core::app::KeyMethod;

use crate::ctap::usb::{TouchKind, TouchRequest};
use crate::executor::passkey::{CredentialChoice, PinRequest};
use crate::loc::Loc;
use crate::outcome::{SHEET_PAD, SHEET_RADIUS, SHEET_W};
use crate::passkey_icons::{Palette, PasskeyIcon, PasskeyIconCache};
use crate::theme::{self, FLOW_GAP_LG, FLOW_GAP_MD, FLOW_GAP_SM, TOUCH_DISC, Theme};
use crate::ui::{ButtonVariant, NameFieldStrings, text_field, vela_button, vela_button_opts};

/// The dialog card every ceremony prompt sits on — shared with the Clear
/// Signer's two (`signing::trusted_signer`), so a person waiting on a page and
/// a person waiting on a key see one kind of card.
pub(crate) fn card(theme: &Theme) -> Div {
    div()
        .w(px(SHEET_W))
        .flex()
        .flex_col()
        .gap(px(FLOW_GAP_LG))
        .p(px(SHEET_PAD))
        .rounded(px(SHEET_RADIUS))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.border_card)
}

pub(crate) fn title(theme: &Theme, text: SharedString) -> Div {
    div()
        .text_size(theme::text_flow_headline())
        .font_weight(FontWeight::BOLD)
        .text_color(theme.fg_base)
        .child(text)
}

pub(crate) fn body(theme: &Theme, text: SharedString) -> Div {
    div()
        .text_size(theme::text_body())
        .line_height(theme::line_height_body())
        .text_color(theme.fg_muted)
        .child(text)
}

/// "Your key is blinking — touch it."
///
/// The first version of this was a caption on the progress screen only, and the
/// first key is minted from the NAME screen — so a person pressed 继续, watched
/// nothing happen, and had no idea a key three feet away was waiting for them.
/// The key knows; the screen has to say so wherever the screen happens to be,
/// which is why the page renders this over everything rather than inside a
/// step.
///
/// **No buttons.** There is nothing to press here — the answer is on the desk,
/// and a Cancel would only be a second way to do what walking away already does
/// (the exchange times out and reports it).
pub fn touch_card(
    theme: &Theme,
    loc: &Loc,
    waiting: &TouchRequest,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    // A phone reached over caBLE is not a security key on the desk — it runs the
    // approval behind its own fingerprint/passkey UI, so the prompt tells the
    // person to look at the phone, not to "touch" anything here. The two
    // strings are the shared corpus keys iOS and Android render for the same
    // moment (promoted 2026-08-28, closing the standing i18n follow-up).
    let (title_text, body_text) = if waiting.remote {
        (
            loc.t("onboarding.common.touchRemoteTitle"),
            loc.t("onboarding.common.touchRemoteBody"),
        )
    } else {
        let body_key = match waiting.kind {
            TouchKind::Presence => "onboarding.create.touchBody",
            TouchKind::Fingerprint => "onboarding.create.touchFingerprintBody",
            // No `{{product}}` to fill: several keys are blinking, and naming one
            // of them would be naming the wrong one.
            TouchKind::Select => "onboarding.create.touchSelectBody",
        };
        (
            loc.t("onboarding.create.touchTitle"),
            loc.t_text(body_key, "product", &waiting.product),
        )
    };

    let prompt = card(theme)
        .items_center()
        // A filled disc where the outcome badge would be: the same place, the
        // same weight, and the only thing on the card that draws the eye.
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
        .child(title(theme, title_text))
        .child(body(theme, body_text));
    // The way out — drawn ONLY where it is one. This card covers the window
    // for as long as a key waits for a finger, and it had nothing on it to
    // press (founder, 2026-09-19: the third dialog of its kind). A prompt
    // whose wait cannot be stopped yet gets no button rather than a button
    // that hides the card and leaves the ceremony running behind it.
    if waiting.cancellable {
        prompt.child(vela_button(
            "touch-cancel",
            ButtonVariant::Secondary,
            loc.t("common.cancel"),
            theme,
            on_cancel,
        ))
    } else {
        prompt
    }
}

/// The method rows' mark size — the web's `--icon-lg`.
pub const METHOD_ICON_PX: u32 = 24;

/// The three places a key lives, as the CREATE chooser offers them, in its
/// order: the key on the desk, this device, a phone by scan.
///
/// Spec 102: three, no fourth. The trusted page is where a person REVIEWS and
/// signs — an account's venue, chosen in its settings — not a place a key
/// lives; the same three places exist on the page too. A wallet on a
/// trusted signing page is the choosers' advanced entry ("Use a trusted
/// signing page", D6), under these rows.
pub const CREATE_ROUTES: [KeyMethod; 3] = [
    KeyMethod::SecurityKey,
    KeyMethod::Platform,
    KeyMethod::Hybrid,
];

/// The same three on the SIGN-IN chooser, in its own order — a wallet reached
/// from a phone is the interesting case at sign-in, and "this device" is the
/// row most likely to be greyed. `Platform` has no route on every desktop and
/// shows as unavailable-with-a-reason, exactly as it does in the create
/// picker.
pub const SIGNIN_ROUTES: [KeyMethod; 3] = [
    KeyMethod::SecurityKey,
    KeyMethod::Hybrid,
    KeyMethod::Platform,
];

/// A route's title key and its create-chooser line key — the core's words
/// (`vela_core::app::method_words`, 087 F01/F02), named by key so a test can
/// check every one resolves. What a row actually draws is [`method_line`].
#[must_use]
pub const fn method_words(method: KeyMethod) -> (&'static str, &'static str) {
    let words = method.words(Chooser::Create, DeviceUnlock::Other);
    match words.line {
        MethodLine::Key(key) => (words.title_key, key),
        // `Other` names no product, so every create line is a key.
        MethodLine::Name(_) => (words.title_key, "onboarding.create.methodPlatformBody"),
    }
}

/// Which chooser a route's line is drawn in (083 W16) — the core's.
pub use vela_core::app::method_words::KeyChooser as Chooser;
use vela_core::app::method_words::{DeviceUnlock, MethodLine};

/// A route's line under its title, as `chooser` and this machine say it — the
/// core's rule (087 F01/F02, first fixed here as 083 W16): the sign-in
/// sheet's phone row scans rather than creates, and "this device" names the
/// one authenticator this machine has.
#[must_use]
pub fn method_line(loc: &Loc, method: KeyMethod, chooser: Chooser) -> SharedString {
    match method.words(chooser, THIS_DEVICE).line {
        MethodLine::Key(key) => loc.t(key),
        MethodLine::Name(name) => SharedString::from(name),
    }
}

/// The line under a phone's QR wherever the phone creates nothing — a
/// sign-in, a send, a dApp's signature: "Scan a code", the one thing to do
/// while the code is up (083 H1 review). What to do on the phone after that
/// is the touch card's line, which replaces this card once the phone is in.
#[must_use]
pub fn scan_line(loc: &Loc) -> SharedString {
    loc.t(vela_core::app::method_words::SCAN_LINE_KEY)
}

/// What unlocks a passkey on this machine, for the core to name (083 W16,
/// 087 F01): Windows Hello on Windows, Touch ID on a Mac. Linux reaches none,
/// and its greyed row says why in its own line.
const THIS_DEVICE: DeviceUnlock = if cfg!(windows) {
    DeviceUnlock::WindowsHello
} else if cfg!(target_os = "macos") {
    DeviceUnlock::TouchId
} else {
    DeviceUnlock::Other
};

/// May this route run on this machine? Only "this device" can answer no: a
/// platform authenticator needs a system passkey service, which in this app's
/// reach only Windows has. A key on the desk and a phone by scan are reachable
/// from every desktop.
#[must_use]
pub fn method_available(method: KeyMethod) -> bool {
    method != KeyMethod::Platform || crate::executor::passkey::platform_supported()
}

/// May this place be asked for where the ceremony runs? On the person's own
/// signing page (spec 102 R3) the BROWSER runs it, and a browser reaches this
/// device's own authenticator on every desktop — so every place is open there.
#[must_use]
pub fn place_available(method: KeyMethod, on_page: bool) -> bool {
    on_page || method_available(method)
}

/// What a screen does when the person picks a way to sign in. An `Arc` rather
/// than a closure per row: the card draws four rows from one handler, and
/// gpui's own listeners are not `Clone`.
pub type PickMethod = std::sync::Arc<dyn Fn(KeyMethod, &mut Window, &mut App)>;

/// What a chooser row does when it is pressed.
pub type RowAction = std::rc::Rc<dyn Fn(&mut Window, &mut App)>;

/// The choosers' advanced entry (spec 102): "Use a trusted signing page", and —
/// once a page is chosen — that page, drawn above the three places, with a
/// way back to Vela's own keys.
pub struct OwnPage<'a> {
    /// The chosen page's row, when one is chosen.
    pub chosen: Option<&'a crate::signing::pages::PageRow>,
    /// The glyphs the chosen page's row draws with.
    pub icons: &'a RefCell<crate::icons::IconCache>,
    /// Open the page picker.
    pub on_open: RowAction,
    /// Back to Vela's own keys.
    pub on_clear: RowAction,
    /// The chosen page runs the ceremony itself (a custom domain, R3): every
    /// place is open there, this machine's own authenticator included.
    pub on_page: bool,
}

/// The advanced entry's row, as both choosers draw it under the three places:
/// set apart by a rule, the page's eye, "Use a trusted signing page" and what it
/// means for the keys.
pub fn own_page_row(
    id: &'static str,
    theme: &Theme,
    loc: &Loc,
    icons: &RefCell<PasskeyIconCache>,
    palette: Palette,
    on_open: RowAction,
) -> Div {
    let words = vela_core::app::method_words::venue_row_words(
        vela_core::app::method_words::VenueRow::SigningPage,
    );
    let mark = icons
        .borrow_mut()
        .image(PasskeyIcon::Eye, palette, METHOD_ICON_PX);
    div().w_full().pt(px(FLOW_GAP_SM)).child(
        div()
            .id(id)
            .w_full()
            .flex()
            .items_center()
            .gap(px(FLOW_GAP_MD))
            .py(px(FLOW_GAP_MD))
            .cursor_pointer()
            .hover(|s| s.bg(theme.bg_well))
            .on_click(move |_, window, cx| on_open(window, cx))
            .child(
                img(ImageSource::Render(mark))
                    .w(px(METHOD_ICON_PX as f32))
                    .h(px(METHOD_ICON_PX as f32))
                    .flex_none(),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme::text_card_title())
                            .text_color(theme.fg_base)
                            .child(loc.t(words.title_key)),
                    )
                    // One line, always (issue 475): the entry's line is
                    // short in every language now, and a row that wraps is
                    // taller than the three places over it.
                    .child(
                        body(
                            theme,
                            words
                                .line_key()
                                .map_or_else(SharedString::default, |key| loc.t(key)),
                        )
                        .whitespace_nowrap()
                        .truncate(),
                    ),
            )
            .child(
                div()
                    .flex_none()
                    .text_size(theme::text_card_title())
                    .text_color(theme.fg_subtle)
                    .child("›"),
            ),
    )
}

pub fn signin_method_card(
    theme: &Theme,
    loc: &Loc,
    icons: &RefCell<PasskeyIconCache>,
    own_page: Option<OwnPage<'_>>,
    on_pick: PickMethod,
    on_dismiss: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    // The row's mark (spec 038, #190): the founder's set, with "this device"
    // resolved to the platform this binary runs on. Paper = the card's own
    // surface, so the USB key's slots read as slots.
    let palette = Palette {
        ink: theme.fg_muted,
        muted: theme.fg_subtle,
        paper: theme.bg_raised,
    };
    // "This device" is real on exactly one desktop. Windows has Windows Hello
    // behind `webauthn.dll`; macOS and Linux reach no platform authenticator
    // from gpui at all, so there the row stays greyed and says why.
    let on_page = own_page.as_ref().is_some_and(|own| own.on_page);
    let entry = |method: KeyMethod| {
        let available = place_available(method, on_page);
        let (title_key, _) = method_words(method);
        // The one row that can be unavailable says why in its own line.
        let line = if available {
            method_line(loc, method, Chooser::SignIn)
        } else {
            loc.t("onboarding.create.securityKeyRequiredBody")
        };
        let on_pick = on_pick.clone();
        let mark =
            icons
                .borrow_mut()
                .image(PasskeyIcon::for_method(method), palette, METHOD_ICON_PX);
        let row = div()
            .id(("signin-method", method as u64))
            .w_full()
            .flex()
            .items_center()
            .gap(px(FLOW_GAP_MD))
            .py(px(FLOW_GAP_MD))
            .border_b_1()
            .border_color(theme.border_card)
            .child(
                img(ImageSource::Render(mark))
                    .w(px(METHOD_ICON_PX as f32))
                    .h(px(METHOD_ICON_PX as f32))
                    .flex_none(),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme::text_card_title())
                            .text_color(theme.fg_base)
                            .child(loc.t(title_key)),
                    )
                    .child(body(theme, line)),
            );
        if available {
            row.cursor_pointer()
                .hover(|s| s.bg(theme.bg_well))
                .on_click(move |_, window, cx| on_pick(method, window, cx))
        } else {
            row.opacity(0.4)
        }
    };

    // Spec 102: three places — and, under them, the advanced entry for a
    // wallet on the person's own signing page. Once that page is chosen it
    // heads the list: the three places are then asked on THAT page.
    let mut sheet = card(theme).child(title(theme, loc.t("onboarding.login.header")));
    if let Some(chosen) = own_page.as_ref().and_then(|own| own.chosen) {
        let clear = own_page
            .as_ref()
            .map(|own| std::rc::Rc::clone(&own.on_clear));
        sheet = sheet.child(crate::signing::pages::chosen_page_banner(
            theme,
            &mut own_page
                .as_ref()
                .map(|own| own.icons)
                .unwrap_or_else(|| unreachable!("a chosen page has its icons"))
                .borrow_mut(),
            chosen,
            loc.t("common.tryAgain"),
            Some(Box::new(move |_, window, cx| {
                if let Some(clear) = clear.as_ref() {
                    clear(window, cx);
                }
            })),
        ));
    }
    let mut rows = div().w_full().flex().flex_col();
    for method in SIGNIN_ROUTES {
        rows = rows.child(entry(method));
    }
    if let Some(own) = own_page.as_ref().filter(|own| own.chosen.is_none()) {
        rows = rows.child(own_page_row(
            "signin-own-page",
            theme,
            loc,
            icons,
            palette,
            std::rc::Rc::clone(&own.on_open),
        ));
    }
    sheet = sheet.child(rows);
    sheet.child(vela_button(
        "signin-methods-cancel",
        ButtonVariant::Secondary,
        loc.t("onboarding.common.close"),
        theme,
        on_dismiss,
    ))
}

/// The caBLE QR the person scans with their phone to sign in or add a key over
/// the hybrid transport.
///
/// **Black on white, always — not themed.** A QR is not UI chrome; it is a
/// scannable target, and a phone camera needs dark modules on a light field
/// whatever the app's theme is. So the matrix ignores the palette and draws its
/// own white quiet-zone box, the one place in this file that names a literal
/// colour on purpose.
///
/// **No buttons.** The answer is the phone; there is nothing to press. It clears
/// itself the moment the tunnel is up.
///
/// **Its line is the caller's.** The create card's "create it on a nearby
/// device" is false over a signature (083 W19) and over a sign-in (083 W16);
/// those say [`scan_line`].
pub fn qr_card_with(
    theme: &Theme,
    loc: &Loc,
    payload: &str,
    body_text: SharedString,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    // A module size that keeps a typical caBLE payload (~40 modules a side) to a
    // card-sized target without a second layout pass.
    const MODULE_PX: f32 = 5.0;

    let matrix: Div = match QrCode::new(payload.as_bytes()) {
        Ok(code) => {
            let width = code.width();
            let colors = code.to_colors();
            let mut grid = div().flex().flex_col();
            for row in 0..width {
                let mut line = div().flex().flex_row();
                for col in 0..width {
                    let dark = matches!(colors.get(row * width + col), Some(QrColor::Dark));
                    let mut cell = div().size(px(MODULE_PX));
                    if dark {
                        cell = cell.bg(rgb(0x000000));
                    }
                    line = line.child(cell);
                }
                grid = grid.child(line);
            }
            grid
        }
        // A payload too large for a QR should never reach here (the caBLE payload
        // is well within capacity); fall back to nothing rather than panic.
        Err(_) => div(),
    };

    card(theme)
        .items_center()
        .child(title(theme, loc.t("onboarding.create.methodHybridTitle")))
        .child(
            // The white quiet-zone box the matrix needs to scan.
            div()
                .p(px(FLOW_GAP_MD))
                .rounded(px(SHEET_RADIUS))
                .bg(rgb(0xffffff))
                .child(matrix),
        )
        .child(body(theme, body_text))
        // The way out. This card waits up to ninety seconds for a phone, over a
        // scrim that swallows every press; without this button the only exit
        // was quitting the app. The PIN and wallet-picker cards always had one.
        .child(vela_button(
            "qr-cancel",
            ButtonVariant::Secondary,
            loc.t("common.cancel"),
            theme,
            on_cancel,
        ))
}

/// The PIN a security key without a sensor verifies with.
///
/// The helper line under the field carries the ONE fact that changes between
/// attempts: a refused PIN says so, and says what running out costs. Otherwise
/// it is the remaining count — which is the number a person needs BEFORE they
/// guess, not after.
#[allow(clippy::too_many_arguments, clippy::allow_attributes)]
pub fn pin_card(
    theme: &Theme,
    loc: &Loc,
    request: &PinRequest,
    value: &str,
    focus: &gpui::FocusHandle,
    window: &Window,
    on_change: impl Fn(String, &mut Window, &mut App) + 'static,
    on_confirm: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    let helper = if request.retry {
        loc.t("onboarding.create.pinRejected")
    } else {
        match request.retries {
            Some(left) => loc.t_vars(
                "onboarding.create.pinAttemptsLeft",
                &[("attempts", f64::from(left))],
            ),
            // A key that will not answer `getPinRetries` still deserves a
            // dialog — just without a number it cannot supply.
            None => loc.t("onboarding.create.pinLabel"),
        }
    };
    let strings = NameFieldStrings {
        label: loc.t("onboarding.create.pinLabel"),
        placeholder: SharedString::from("••••"),
        helper,
        too_long_hint: loc.t("onboarding.create.pinRejected"),
    };

    card(theme)
        .child(title(theme, loc.t("onboarding.create.pinTitle")))
        // The key names ITSELF here — the product string it reports over USB.
        // "YubiKey 5C NFC" is what is on the desk; "your authenticator" is what
        // a form says.
        .child(body(
            theme,
            loc.t_text("onboarding.create.pinBody", "product", &request.product),
        ))
        .child(text_field(
            "pin-field",
            theme,
            &strings,
            value,
            request.retry,
            true,
            focus,
            window,
            on_change,
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(FLOW_GAP_MD))
                .child(vela_button_opts(
                    "pin-confirm",
                    ButtonVariant::Primary,
                    loc.t("onboarding.create.nextBtn"),
                    !value.is_empty(),
                    theme,
                    on_confirm,
                ))
                .child(vela_button(
                    "pin-cancel",
                    ButtonVariant::Row,
                    loc.t("common.cancel"),
                    theme,
                    on_cancel,
                )),
        )
}

/// Which of several wallets on one key.
///
/// A security key can hold more than one Vela wallet, and "who are you?" then
/// has more than one answer. Without this, the first credential the key happens
/// to return wins — leaving the others unreachable from this computer, which is
/// indistinguishable from having lost them.
///
/// The assertions behind these rows are ALREADY SIGNED: choosing is choosing
/// which one to hand the core, not asking the key to do more work.
pub fn pick_card(
    theme: &Theme,
    loc: &Loc,
    choices: &[CredentialChoice],
    on_pick: impl Fn(&usize, &mut Window, &mut App) + Clone + 'static,
    on_cancel: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> Div {
    // The key is not blinking any more by the time this is on screen — the
    // touch is what produced the list — so the product name comes from the
    // choices themselves.
    let product = choices
        .first()
        .map(|choice| choice.product.clone())
        .unwrap_or_default();

    // The rows scroll INSIDE the card: a key can hold a dozen wallets, and
    // without a height cap the card grows past the window — title pushed off
    // the top, cancel unreachable below (seen live with 12 credentials).
    // Title, body and cancel stay outside this region, always on screen.
    let mut rows = div()
        .id("wallet-pick-rows")
        .w_full()
        .max_h(px(360.))
        .min_h(px(0.))
        .overflow_y_scroll()
        .flex()
        .flex_col()
        .gap(px(2.));
    for (index, choice) in choices.iter().enumerate() {
        let unnamed = choice.name.trim().is_empty();
        let name = if unnamed {
            loc.t("onboarding.login.pickUnnamed")
        } else {
            SharedString::from(choice.name.clone())
        };
        // The leading disc carries the wallet's monogram — its name's first
        // grapheme. NOT an identicon: identicons are seeded from ADDRESSES
        // (the anti-poisoning rule), and no address exists before the pick.
        // A neutral monogram distinguishes rows without claiming an identity.
        let monogram: SharedString = if unnamed {
            SharedString::from("?")
        } else {
            SharedString::from(
                name.chars()
                    .next()
                    .map(|c| c.to_uppercase().collect::<String>())
                    .unwrap_or_default(),
            )
        };
        // The second line is the KEY, the way a browser's own picker labels
        // these rows — every wallet here lives on the same authenticator, so it
        // is the same line on every row, and that is the honest answer to
        // "where is this passkey".
        //
        // UNLESS two rows share a name. Then, and only then, the credential
        // id's head is appended: two identical rows are worse than a little
        // hex, because the person has no way to say which they meant.
        let ambiguous = choices
            .iter()
            .filter(|other| other.name == choice.name)
            .count()
            > 1;
        let subtitle = if ambiguous {
            SharedString::from(format!(
                "{product} · {}",
                choice
                    .credential_id
                    .get(..12)
                    .unwrap_or(&choice.credential_id)
            ))
        } else {
            SharedString::from(product.clone())
        };
        let on_pick = on_pick.clone();
        rows = rows.child(
            div()
                .id(("wallet-choice", index as u64))
                .w_full()
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(FLOW_GAP_MD))
                .px(px(FLOW_GAP_SM))
                .rounded(px(10.))
                .cursor_pointer()
                .hover(|style| style.bg(theme.bg_sunken))
                .on_click(move |_, window, cx| on_pick(&index, window, cx))
                .child(
                    div()
                        .flex_none()
                        .size(px(36.))
                        .rounded_full()
                        .bg(theme.bg_sunken)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(theme::text_card_title())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(monogram),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .min_w_0()
                        .child(
                            div()
                                .text_size(theme::text_card_title())
                                .text_color(theme.fg_base)
                                .child(name),
                        )
                        .child(
                            div()
                                .text_size(theme::text_flow_caption())
                                .text_color(theme.fg_subtle)
                                .child(subtitle),
                        ),
                ),
        );
    }

    card(theme)
        .child(title(theme, loc.t("onboarding.login.pickTitle")))
        .child(body(
            theme,
            loc.t_text("onboarding.login.pickBody", "product", &product),
        ))
        .child(rows)
        .child(vela_button(
            "wallet-pick-cancel",
            ButtonVariant::Row,
            loc.t("common.cancel"),
            theme,
            on_cancel,
        ))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::passkey_icons::PasskeyIcon;

    /// **Three places, no fourth** (spec 102 P2-01): both choosers offer
    /// every place a key can live — "这台设备 / 手机或平板 / USB 安全密钥" — each
    /// exactly once, and neither screen can grow or lose one without the
    /// other. The trusted page is where a person reviews and signs, not a
    /// place: it is the account's venue, never a row here.
    #[test]
    fn both_choosers_offer_every_place_exactly_once() {
        for (screen, routes) in [("create", CREATE_ROUTES), ("sign in", SIGNIN_ROUTES)] {
            assert_eq!(routes.len(), KeyMethod::ALL.len(), "{screen}");
            for method in KeyMethod::ALL {
                let offered = routes.iter().filter(|row| **row == method).count();
                assert_eq!(offered, 1, "{screen} offers {method:?} {offered} times");
            }
        }
    }

    /// The advanced entry has its own words, and they are not a place's.
    #[test]
    fn the_own_page_entry_is_worded_and_is_not_a_place() {
        let loc = crate::loc::Loc::from_env();
        let words = vela_core::app::method_words::venue_row_words(
            vela_core::app::method_words::VenueRow::SigningPage,
        );
        let title = loc.t(words.title_key);
        assert_ne!(title.as_ref(), words.title_key);
        for method in KeyMethod::ALL {
            assert_ne!(loc.t(method_words(method).0), title, "{method:?}");
        }
    }

    /// Every route says something of its own, in words the corpus carries in
    /// fifteen languages. A row echoing its key, or two rows sharing a title,
    /// is a picker that cannot be used.
    #[test]
    fn every_route_has_its_own_words_and_its_own_mark() {
        let loc = crate::loc::Loc::from_env();
        let mut titles = Vec::new();
        let mut marks = Vec::new();
        for method in CREATE_ROUTES {
            let (title_key, body_key) = method_words(method);
            for key in [title_key, body_key] {
                let said = loc.t(key);
                assert_ne!(said.as_ref(), key, "`{key}` echoed its key");
                assert!(!said.is_empty(), "`{key}` resolved empty");
            }
            titles.push(loc.t(title_key).to_string());
            marks.push(PasskeyIcon::for_method(method));
        }
        titles.sort();
        titles.dedup();
        assert_eq!(
            titles.len(),
            CREATE_ROUTES.len(),
            "two routes share a title"
        );
        marks.dedup();
        assert_eq!(marks.len(), CREATE_ROUTES.len(), "two routes share a mark");
        // The page's eye is no place's mark: it is the own-page entry's.
        assert!(!marks.contains(&PasskeyIcon::Eye));
    }

    /// Only "this device" can be unavailable, and it is the only row whose
    /// line changes when it is.
    #[test]
    fn only_this_device_can_be_out_of_reach() {
        for method in CREATE_ROUTES {
            if method == KeyMethod::Platform {
                continue;
            }
            assert!(method_available(method), "{method:?} was greyed out");
        }
        assert_eq!(
            method_available(KeyMethod::Platform),
            crate::executor::passkey::platform_supported()
        );
    }

    /// Spec 102 R3: on the person's own page the browser runs the ceremony,
    /// so every place is open there — this machine's own authenticator too,
    /// on the desktops where the app itself cannot reach one.
    #[test]
    fn every_place_is_open_on_the_persons_own_page() {
        for method in KeyMethod::ALL {
            assert!(place_available(method, true), "{method:?}");
            assert_eq!(place_available(method, false), method_available(method));
        }
    }

    /// 083 W16: the sign-in sheet's phone row creates nothing, and "this
    /// device" names the one authenticator this machine has — in both
    /// choosers. Every other line is the same in both.
    #[test]
    fn the_sign_in_sheet_signs_in_and_names_this_machines_authenticator() {
        let loc = crate::loc::Loc::from_env();
        let phone_create = method_line(&loc, KeyMethod::Hybrid, Chooser::Create);
        let phone_sign_in = method_line(&loc, KeyMethod::Hybrid, Chooser::SignIn);
        assert_eq!(phone_create, loc.t("onboarding.create.methodHybridBody"));
        assert_ne!(phone_sign_in, phone_create, "the sign-in row said create");
        assert_ne!(phone_sign_in.as_ref(), "explore.scan", "echoed its key");
        // The QR under a sign-in, a send and a dApp signature: one line.
        assert_eq!(phone_sign_in, scan_line(&loc));

        let this_device = method_line(&loc, KeyMethod::Platform, Chooser::SignIn);
        assert_eq!(
            this_device,
            method_line(&loc, KeyMethod::Platform, Chooser::Create)
        );
        #[cfg(windows)]
        assert_eq!(this_device.as_ref(), "Windows Hello");
        #[cfg(target_os = "macos")]
        assert_eq!(this_device.as_ref(), "Touch ID");

        assert_eq!(
            method_line(&loc, KeyMethod::SecurityKey, Chooser::SignIn),
            method_line(&loc, KeyMethod::SecurityKey, Chooser::Create),
        );
    }
}
