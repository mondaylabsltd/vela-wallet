//! Vela Wallet desktop entry — window management only (spec 007 FR-009).

#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod ceremony;
mod contacts;
mod core_host;
mod ctap;
mod executor;
mod explore;
mod flows;
mod gallery;
mod hardware;
mod icons;
mod identicon;
mod intro;
mod intro_art;
mod loc;
mod onboarding;
mod onboarding_flow;
mod outcome;
mod panic_report;
mod parallel_space;
mod passkey_directory;
mod passkey_icons;
mod raster;
mod resident;
mod session;
mod settings;
mod signing;
mod theme;
mod ui;
mod wallet;
#[cfg(not(target_os = "linux"))]
mod webview;
// Same name, no browser behind it: see the file's own header for why the Linux
// build gets a module rather than a `cfg` in every caller.
#[cfg(target_os = "linux")]
#[path = "webview_absent.rs"]
mod webview;
mod window_frame;

use gallery::GalleryView;
use gpui::{
    App, AppContext as _, Bounds, Context, Div, IntoElement, KeyBinding, Menu, MenuItem,
    ParentElement as _, QuitMode, Render, Styled as _, TitlebarOptions, Window, WindowBounds,
    WindowOptions, actions, div, point, px, size,
};
use onboarding::OnboardingPage;
use theme::{WINDOW_H, WINDOW_W};
use vela_core::app::session::SessionRoute;
use wallet::page::{Identity, WalletPage};

/// The window's one child, and the only thing in this app that decides which
/// screen a person is on.
///
/// **The core decides WHAT is allowed; this decides WHEN to navigate.** It
/// renders `SessionView::allowed_route` and concludes nothing of its own — in
/// particular it does not read the account list and infer a route, because
/// during the storage read there is no answer yet and `Loading` is how the core
/// says so. Rendering onboarding in that gap would flash the welcome screen at
/// somebody who already has a wallet.
struct Root {
    onboarding: Option<gpui::Entity<OnboardingPage>>,
    wallet: Option<gpui::Entity<WalletPage>>,
}

impl Root {
    fn new(cx: &mut Context<Self>) -> Self {
        // Re-render whenever the session changes: the hand-off out of
        // onboarding is a global write, and this is what turns it into a
        // navigation.
        cx.observe_global::<session::SessionState>(|_, cx| cx.notify())
            .detach();
        Self {
            onboarding: None,
            wallet: None,
        }
    }
}

impl Render for Root {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = session::view(cx);
        let root: Div = div().size_full();
        let root = match view.allowed_route {
            // Storage unread. Paint the surface and nothing else — a splash
            // that lasts one frame is invisible, and a wrong screen is not.
            SessionRoute::Loading => {
                root.bg(theme::Theme::of(theme::ThemeMode::detect(window)).bg_base)
            }
            SessionRoute::Onboarding => {
                // Dropped on the way out, so a second sign-in starts from a
                // fresh machine rather than resuming a finished one.
                self.wallet = None;
                // And so do the resident machines. Contacts, networks and the
                // chosen currency belong to the ACCOUNT; a resident that
                // outlived a sign-out would show the previous person's address
                // book to the next one.
                resident::drop_all(cx);
                let page = self
                    .onboarding
                    .get_or_insert_with(|| cx.new(|cx| OnboardingPage::new(window, cx)))
                    .clone();
                root.child(page)
            }
            SessionRoute::Wallet => {
                self.onboarding = None;
                let identity = Identity {
                    name: view
                        .accounts
                        .get(view.active_index)
                        .map_or_else(|| "".into(), |row| row.account.name.clone().into()),
                    address: view.address.clone(),
                };
                let page = self
                    .wallet
                    .get_or_insert_with(|| cx.new(|cx| WalletPage::signed_in(identity, window, cx)))
                    .clone();
                root.child(page)
            }
        };
        // The parallel space's marker, over whichever screen is up. It renders
        // whenever the space is active and never behind a build flag alone —
        // a test wallet must never wear the real one's face.
        parallel_space::overlay(root, window)
    }
}

/// Which root the window hosts.
/// `VELA_PAGE=wallet|contacts|explore|settings|gallery`
/// (spec 015 research.md D4, extended by spec 018 research.md D1 and spec 023)
/// — same env-pin family as `VELA_THEME`/`VELA_LANG`; the default remains the
/// onboarding flow.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum RootPage {
    Onboarding,
    Wallet,
    Contacts,
    /// Spec 022 — the browser, for review without clicking through the wallet.
    Explore,
    Settings,
    Gallery,
}

impl RootPage {
    fn from_env() -> Self {
        match std::env::var("VELA_PAGE").as_deref() {
            Ok("wallet") => Self::Wallet,
            Ok("contacts") => Self::Contacts,
            Ok("explore") => Self::Explore,
            Ok("settings") => Self::Settings,
            Ok("gallery") => Self::Gallery,
            _ => Self::Onboarding,
        }
    }
}

// TODO(i18n): menu labels are English-only until the corpus grows menu keys.
actions!(vela, [Quit, HideApp, HideOthers, ShowAll, ToggleFullScreen]);

/// Open the (only) application window. Called at startup, and again from the
/// reopen handler when the Dock icon is clicked after the window was closed.
fn open_main_window(cx: &mut App) {
    // Two dev galleries coexist: `VELA_GALLERY=1` (spec 014) browses the
    // onboarding-flow state fixtures; `VELA_PAGE=gallery` (spec 015) browses
    // the wallet-home states. Unifying them is a recorded follow-up.
    if gallery::gallery_enabled() {
        open_window_with(cx, |window, cx| cx.new(|cx| GalleryView::new(window, cx)));
        return;
    }
    match RootPage::from_env() {
        // The default: the session's route guard picks the screen.
        RootPage::Onboarding => open_window_with(cx, |_window, cx| cx.new(Root::new)),
        RootPage::Wallet => open_window_with(cx, |window, cx| {
            cx.new(|cx| WalletPage::new(false, window, cx))
        }),
        RootPage::Contacts => open_window_with(cx, |window, cx| {
            cx.new(|cx| WalletPage::contacts(window, cx))
        }),
        RootPage::Explore => open_window_with(cx, |window, cx| {
            cx.new(|cx| WalletPage::explore(window, cx))
        }),
        RootPage::Settings => open_window_with(cx, |window, cx| {
            cx.new(|cx| WalletPage::settings(window, cx))
        }),
        RootPage::Gallery => open_window_with(cx, |window, cx| {
            cx.new(|cx| WalletPage::new(true, window, cx))
        }),
    }
}

fn open_window_with<V: gpui::Render + 'static>(
    cx: &mut App,
    build: impl FnOnce(&mut gpui::Window, &mut App) -> gpui::Entity<V>,
) {
    let bounds = Bounds::centered(None, size(px(WINDOW_W), px(WINDOW_H)), cx);

    cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            // The card grid does not reflow below the design size (spec 007
            // edge cases): the design size is the minimum.
            window_min_size: Some(size(px(WINDOW_W), px(WINDOW_H))),
            // Sets the Wayland `xdg_toplevel.app_id` and the X11 `WM_CLASS`.
            // Both are how a desktop shell matches a running window to its
            // installed `.desktop` file, so this string, the file name
            // `packaging/app.getvela.VelaWallet.desktop` and the
            // `StartupWMClass` inside it must stay identical — otherwise
            // GNOME shows the app with a generic icon and the binary name.
            app_id: Some("app.getvela.VelaWallet".into()),
            titlebar: Some(TitlebarOptions {
                title: Some("Vela Wallet".into()),
                // Content owns the full canvas, as in the mocks; only the
                // traffic lights remain, inset to the mock's position.
                appears_transparent: true,
                traffic_light_position: Some(point(px(20.), px(20.))),
            }),
            ..Default::default()
        },
        build,
    )
    .expect("failed to open the main window");
}

fn main() {
    // Spec 038: a panic on a worker thread becomes a sheet, not a vanished
    // window. Installed before anything can spawn.
    panic_report::install();
    // LastWindowClosed instead of gpui's macOS default (keep running): a
    // keep-running single-window app must reopen its window from the Dock
    // icon, and that path is dead on macOS 26 — AppKit's TextInputUI panel
    // stays `isVisible` after the last real window closes, the system then
    // reports hasVisibleWindows=YES on a Dock click, and gpui's
    // `should_handle_reopen` swallows the event before any on_reopen callback
    // (README, "Known gpui quirks"). Closing the window therefore quits, the
    // same behaviour gpui already defaults to on Windows and Linux.
    let app = gpui_platform::application().with_quit_mode(QuitMode::LastWindowClosed);

    // Insurance, not the fix: unreachable while LastWindowClosed holds, but
    // if a future gpui or quit-mode change ever leaves the app running with
    // no window, a Dock-icon click should build one rather than nothing.
    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            open_main_window(cx);
        }
    });

    app.run(|cx: &mut App| {
        // Storage is read before the first window opens, so the route guard has
        // a real answer to give on frame one.
        // The UI face, before any window: every page root names it, and a
        // family named before it is loaded renders in the fallback face until
        // the next frame — a visible flash on a wallet's first screen.
        let fonts = theme::UI_FONT_FILES
            .iter()
            .map(|bytes| std::borrow::Cow::Borrowed(*bytes))
            .collect();
        if let Err(error) = cx.text_system().add_fonts(fonts) {
            eprintln!("[vela-wallet] bundled fonts could not be loaded: {error}");
        }
        session::boot(cx);

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.on_action(|_: &HideApp, cx| cx.hide());
        cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
        cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
        cx.on_action(|_: &ToggleFullScreen, cx| {
            if let Some(window) = cx.active_window() {
                window
                    .update(cx, |_, window, _| window.toggle_fullscreen())
                    .ok();
            }
        });

        // The main menu is not just convention on macOS: with a nil
        // NSApp.mainMenu, the menu-bar/titlebar reveal on a fullscreen Space
        // never engages on secondary displays, so a fullscreened window there
        // shows no titlebar on hover and offers no way back out. gpui installs
        // no menu of its own — Zed always sets one, which is why upstream
        // never trips over this.
        if cfg!(target_os = "macos") {
            cx.bind_keys([
                KeyBinding::new("cmd-q", Quit, None),
                KeyBinding::new("cmd-h", HideApp, None),
                KeyBinding::new("alt-cmd-h", HideOthers, None),
                KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
            ]);
            cx.set_menus(vec![
                // The first menu is the application menu; macOS titles it with
                // the bundle name, not this string.
                Menu::new("Vela Wallet").items(vec![
                    MenuItem::action("Hide Vela Wallet", HideApp),
                    MenuItem::action("Hide Others", HideOthers),
                    MenuItem::action("Show All", ShowAll),
                    MenuItem::separator(),
                    MenuItem::action("Quit Vela Wallet", Quit),
                ]),
                Menu::new("View").items(vec![MenuItem::action(
                    "Toggle Full Screen",
                    ToggleFullScreen,
                )]),
            ]);
        }
        open_main_window(cx);

        cx.activate(true);
    });
}
