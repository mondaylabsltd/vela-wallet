//! The menu-bar extra (macOS) and the notification-area icon (Windows)
//! (N-D3): the wallet's total at a glance, "Updated …" under it, and Send,
//! Receive and the window one click away.
//!
//! **Off by default** — a wallet's figure in the menu bar is in view of
//! anybody looking at the screen, so it is the person's choice
//! (`vela.desktop.menuBar`, Settings → Appearance). And it follows the
//! balance mask: while balances are hidden the extra shows the mark and the
//! mask, never a figure (the core withholds the figure itself while hidden —
//! `BalanceView::hidden`, invariant ⑧ — so there is nothing here to leak).
//!
//! The figure is the HOME's: the core's total, last-known rows included,
//! with the hero's own "Updated <ago>" (`home.lastUpdated`). Nothing here
//! reads a network.
//!
//! Every item is the press of the control it names: Send opens the form,
//! nothing moves until the person confirms in the window.
//!
//! Linux has none yet: a StatusNotifierItem needs a D-Bus menu
//! (`com.canonical.dbusmenu`) this app does not implement, and GNOME shows
//! none without an extension — README "Menu bar and tray".

use std::sync::Mutex;

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(windows)]
mod windows;

/// The store key: `true` when the person turned the extra on.
pub const PREF_KEY: &str = "vela.desktop.menuBar";

/// The Settings switch's words (title, hint). NOT in the corpus yet — the
/// I18N lane adds them (README "Menu bar and tray"); until then the switch
/// is not drawn and the extra stays off.
pub const SWITCH_KEYS: (&str, &str) = if cfg!(windows) {
    (
        "settingsModals.appearance.trayIcon",
        "settingsModals.appearance.statusItemHint",
    )
} else {
    (
        "settingsModals.appearance.menuBar",
        "settingsModals.appearance.statusItemHint",
    )
};

/// What the extra shows. Strings in the person's language, already masked.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TrayModel {
    /// The total as the hero draws it ("$1,383.28"), the mask while hidden,
    /// `None` while there is no figure (the first read, nothing reachable).
    pub figure: Option<String>,
    /// "Updated 2 minutes ago", when a read has settled.
    pub updated: Option<String>,
    pub send: String,
    pub receive: String,
    /// "Vela Wallet" — the product's name, never translated.
    pub open: String,
}

/// What a press in the extra asks of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    Open,
    Send,
    Receive,
}

type Commands = Mutex<(
    UnboundedSender<TrayCommand>,
    Option<UnboundedReceiver<TrayCommand>>,
)>;

fn commands() -> &'static Commands {
    static COMMANDS: std::sync::OnceLock<Commands> = std::sync::OnceLock::new();
    COMMANDS.get_or_init(|| {
        let (tx, rx) = unbounded();
        Mutex::new((tx, Some(rx)))
    })
}

/// A press, from the platform's callback.
#[cfg_attr(
    not(any(target_os = "macos", windows)),
    allow(dead_code, reason = "no extra on this platform yet")
)]
pub(crate) fn press(command: TrayCommand) {
    if let Ok(commands) = commands().lock() {
        let _ = commands.0.unbounded_send(command);
    }
}

/// The window's end of the presses. `None` once taken.
pub fn take_commands() -> Option<UnboundedReceiver<TrayCommand>> {
    commands().lock().ok().and_then(|mut slot| slot.1.take())
}

/// Whether this platform can show an extra at all.
#[must_use]
pub const fn supported() -> bool {
    cfg!(any(target_os = "macos", windows))
}

/// Whether the person turned the extra on (`VELA_MENU_BAR=1|0` pins it in a
/// developer build).
/// Read once per launch (the page asks every frame), then kept in step by
/// [`set_wanted`].
static WANTED: Mutex<Option<bool>> = Mutex::new(None);

#[must_use]
pub fn wanted() -> bool {
    if let Some(pinned) = crate::dev_env::var!("VELA_MENU_BAR") {
        return pinned == "1";
    }
    let Ok(mut cached) = WANTED.lock() else {
        return false;
    };
    *cached.get_or_insert_with(|| {
        crate::executor::storage::read_value(PREF_KEY)
            .ok()
            .flatten()
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    })
}

/// Turn the extra on or off for good (the Settings switch).
pub fn set_wanted(on: bool) {
    if let Err(error) = crate::executor::storage::write_value(PREF_KEY, serde_json::json!(on)) {
        crate::diag::vlog!("tray", "menu-bar choice not saved: {error}");
    }
    if let Ok(mut cached) = WANTED.lock() {
        *cached = Some(on);
    }
    if !on {
        hide();
    }
}

/// What is on screen now, so an unchanged frame touches nothing.
static SHOWN: Mutex<Option<TrayModel>> = Mutex::new(None);

/// Show (or update) the extra. Main thread; cheap when nothing changed.
pub fn show(model: TrayModel) {
    let Ok(mut shown) = SHOWN.lock() else {
        return;
    };
    if shown.as_ref() == Some(&model) {
        return;
    }
    #[cfg(target_os = "macos")]
    macos::show(&model, &mark_png());
    #[cfg(windows)]
    windows::show(&model);
    *shown = Some(model);
}

/// Take the extra away (turned off, signed out, quitting).
pub fn hide() {
    let Ok(mut shown) = SHOWN.lock() else {
        return;
    };
    if shown.take().is_none() {
        return;
    }
    #[cfg(target_os = "macos")]
    macos::hide();
    #[cfg(windows)]
    windows::hide();
}

/// The Vela mark as a menu-bar TEMPLATE image: the logo's three shapes
/// (`docs/design/onboarding/logo-light.svg`) in black on transparency, 18 pt
/// at 2×, inside the box the design draws it in. The system tints a template
/// for the bar's appearance — light, dark, and the inverted highlight.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[must_use]
pub fn mark_png() -> Vec<u8> {
    const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="-31 -30 320 320"><path d="M122,0C70,53,38,118,18,187L122,187L122,0Z" fill="#000"/><path d="M142,42C193,75,225,128,240,187L142,187L142,42Z" fill="#000"/><path d="M0,207L258,207C243,240,211,260,165,260L92,260C49,260,16,240,0,207L0,207" fill="#000"/></svg>"##;
    const SIDE: u32 = 36;
    let Ok(tree) = resvg::usvg::Tree::from_str(SVG, &resvg::usvg::Options::default()) else {
        return Vec::new();
    };
    let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(SIDE, SIDE) else {
        return Vec::new();
    };
    let scale = SIDE as f32 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_extra_is_off_until_the_person_turns_it_on() {
        crate::executor::storage::tests::with_temp_state("tray-pref", || {
            if let Ok(mut cached) = WANTED.lock() {
                *cached = None;
            }
            assert!(!wanted(), "off by default");
            set_wanted(true);
            assert!(wanted());
            set_wanted(false);
            assert!(!wanted());
        });
    }

    #[test]
    fn the_mark_is_a_template_drawn_in_black_on_clear() {
        let png = mark_png();
        let image = image::load_from_memory(&png)
            .unwrap_or_else(|e| unreachable!("the mark decodes: {e}"))
            .to_rgba8();
        assert_eq!(image.dimensions(), (36, 36));
        let mut inked = 0;
        for pixel in image.pixels() {
            let [r, g, b, a] = pixel.0;
            if a > 0 {
                inked += 1;
                assert!(r < 8 && g < 8 && b < 8, "a template is black: {:?}", pixel.0);
            }
        }
        assert!(inked > 200, "the mark is drawn ({inked} px)");
        // Air around it, as the design's box: the corners are clear.
        assert_eq!(image.get_pixel(0, 0).0[3], 0);
        assert_eq!(image.get_pixel(35, 35).0[3], 0);
    }

    #[test]
    fn a_press_reaches_the_window_once_taken() {
        let mut presses = take_commands().unwrap_or_else(|| unreachable!("taken once"));
        assert!(take_commands().is_none(), "one window end");
        press(TrayCommand::Send);
        assert_eq!(presses.try_recv().ok(), Some(TrayCommand::Send));
    }
}
