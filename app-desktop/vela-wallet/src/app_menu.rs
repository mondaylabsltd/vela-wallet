//! The macOS menu bar (spec 082 RJ17; spec 095): the application menu with
//! About, an Edit menu, View and Window — labelled from the corpus, in the
//! language the wallet is showing.
//!
//! The main menu is not decoration on macOS: with a nil `NSApp.mainMenu` the
//! menu-bar/titlebar reveal on a fullscreen Space never engages on secondary
//! displays (README, "Known gpui quirks"), and without an Edit menu ⌘C/⌘V/⌘X/
//! ⌘A/⌘Z never reach a web page's field in the in-app browser — WKWebView's
//! editing commands arrive as the Edit menu's actions through the responder
//! chain, not as keystrokes.
//!
//! How each Edit item gets where it must:
//! - Cut / Copy / Paste / Select All are gpui "OS actions": their menu items
//!   send `cut:` `copy:` `paste:` `selectAll:` to the first responder, which is
//!   the web view when a page's field has the keyboard. The wallet's own wells
//!   take the same chords as keystrokes first (`ui::editor`), so the menu is
//!   only consulted when no well wants the key.
//! - Undo / Redo: gpui routes these to its own handler, never the responder
//!   chain, so [`forward_edit`] sends `undo:` / `redo:` on to the responder
//!   that has them — and to nobody when that would be gpui's app delegate.
//! - The key equivalents are bound in a key context no element ever sets
//!   ([`MENU_ONLY`]): the menu shows and owns them, and gpui's own key
//!   dispatch never turns ⌘C into an action that would beat a well to it.

use gpui::{App, KeyBinding, Menu, MenuItem, OsAction, actions};

use crate::loc::Loc;

actions!(
    vela,
    [
        About,
        Quit,
        HideApp,
        HideOthers,
        ShowAll,
        Undo,
        Redo,
        Cut,
        Copy,
        Paste,
        SelectAll,
        ToggleFullScreen,
        Minimize,
        CloseWindow
    ]
);

/// A key context nothing in the window sets: bindings under it label menu
/// items and are never matched by gpui's own dispatch.
const MENU_ONLY: &str = "VelaMenuOnly";

/// The macOS key equivalents (spec 082 RJ17: ⌘W is Close Window, the
/// convention of a window that is not a document — it was unbound).
pub fn mac_key_bindings() -> Vec<KeyBinding> {
    vec![
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-w", CloseWindow, None),
        KeyBinding::new("cmd-m", Minimize, None),
        KeyBinding::new("cmd-h", HideApp, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, None),
        KeyBinding::new("cmd-z", Undo, Some(MENU_ONLY)),
        KeyBinding::new("shift-cmd-z", Redo, Some(MENU_ONLY)),
        KeyBinding::new("cmd-x", Cut, Some(MENU_ONLY)),
        KeyBinding::new("cmd-c", Copy, Some(MENU_ONLY)),
        KeyBinding::new("cmd-v", Paste, Some(MENU_ONLY)),
        KeyBinding::new("cmd-a", SelectAll, Some(MENU_ONLY)),
    ]
}

/// The menus, labelled in `loc`'s language.
pub fn menus(loc: &Loc) -> Vec<Menu> {
    let t = |key: &str| loc.t(&format!("componentsUi.appMenu.{key}"));
    vec![
        // The first menu is the application menu; macOS titles it with the
        // bundle name, not this string.
        Menu::new("Vela Wallet").items(vec![
            MenuItem::action(t("about"), About),
            MenuItem::separator(),
            MenuItem::action(t("hide"), HideApp),
            MenuItem::action(t("hideOthers"), HideOthers),
            MenuItem::action(t("showAll"), ShowAll),
            MenuItem::separator(),
            MenuItem::action(t("quit"), Quit),
        ]),
        Menu::new(t("edit")).items(vec![
            MenuItem::os_action(t("undo"), Undo, OsAction::Undo),
            MenuItem::os_action(t("redo"), Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action(t("cut"), Cut, OsAction::Cut),
            MenuItem::os_action(t("copy"), Copy, OsAction::Copy),
            MenuItem::os_action(t("paste"), Paste, OsAction::Paste),
            MenuItem::os_action(t("selectAll"), SelectAll, OsAction::SelectAll),
        ]),
        Menu::new(t("view")).items(vec![MenuItem::action(t("fullScreen"), ToggleFullScreen)]),
        Menu::new(t("window")).items(vec![
            MenuItem::action(t("minimize"), Minimize),
            MenuItem::separator(),
            MenuItem::action(t("closeWindow"), CloseWindow),
        ]),
    ]
}

/// Install (or re-label, after the language changed) the menu bar. macOS
/// only: the other desktops draw no menu bar.
pub fn install(cx: &mut App) {
    if cfg!(target_os = "macos") {
        keep_appkit_out_of_view_menu();
        cx.set_menus(menus(&Loc::from_env()));
    }
}

/// AppKit adds its own "Enter Full Screen" to a menu titled "View" in its
/// own (English) words — beside ours, so the English View menu showed both
/// (spec 095, read off the menu bar with Accessibility). Ours is localized and
/// bound to ⌃⌘F; AppKit's is turned off by its documented default.
#[cfg(target_os = "macos")]
fn keep_appkit_out_of_view_menu() {
    use objc2::runtime::{AnyObject, Bool};
    use objc2::{class, msg_send};
    let key = objc2_foundation::NSString::from_str("NSFullScreenMenuItemEverywhere");
    // SAFETY: `standardUserDefaults` is a shared instance, never nil in an
    // app; `setBool:forKey:` takes a BOOL and an NSString.
    unsafe {
        let defaults: *mut AnyObject = msg_send![class!(NSUserDefaults), standardUserDefaults];
        if let Some(defaults) = defaults.as_ref() {
            let _: () = msg_send![defaults, setBool: Bool::NO, forKey: &*key];
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn keep_appkit_out_of_view_menu() {}

/// The handlers, once at startup.
pub fn register(cx: &mut App) {
    // Spec 082 RD14 (W17): not while a submit POST is out — once; a second
    // Quit within 5 s goes through.
    cx.on_action(|_: &Quit, cx| {
        if crate::executor::relay::may_close(std::time::Instant::now()) {
            cx.quit();
        } else {
            cx.activate(true);
            crate::wallet::page::close_held(cx);
        }
    });
    // ⌘W (RJ17): the same close as the window's button, hold included —
    // gpui's `remove_window` does not ask `on_window_should_close`.
    cx.on_action(|_: &CloseWindow, cx| {
        if let Some(window) = cx.active_window() {
            window
                .update(cx, |_, window, cx| {
                    if crate::close_requested(window, cx) {
                        window.remove_window();
                    }
                })
                .ok();
        }
    });
    cx.on_action(|_: &Minimize, cx| {
        if let Some(window) = cx.active_window() {
            window
                .update(cx, |_, window, _| window.minimize_window())
                .ok();
        }
    });
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
    cx.on_action(|_: &About, _| show_about());
    cx.on_action(|_: &Undo, _| forward_edit("undo:"));
    cx.on_action(|_: &Redo, _| forward_edit("redo:"));
    // Cut/Copy/Paste/Select All arrive here only when no responder took the
    // native message — nothing focused takes text. Handled, so gpui does not
    // report an action without a handler; nothing to do.
    cx.on_action(|_: &Cut, _| {});
    cx.on_action(|_: &Copy, _| {});
    cx.on_action(|_: &Paste, _| {});
    cx.on_action(|_: &SelectAll, _| {});
}

/// The standard About panel: the icon, the name, the version and build
/// (`CFBundleShortVersionString (CFBundleVersion)`) and the copyright line
/// from Info.plist.
#[cfg(target_os = "macos")]
fn show_about() {
    let Some(mtm) = objc2_foundation::MainThreadMarker::new() else {
        return;
    };
    let app = objc2_app_kit::NSApplication::sharedApplication(mtm);
    // SAFETY: main thread (checked above); a nil sender is what the menu
    // item itself would pass when sent programmatically.
    unsafe { app.orderFrontStandardAboutPanel(None) };
}

#[cfg(not(target_os = "macos"))]
fn show_about() {}

/// Send `undo:` / `redo:` to the responder that implements it — a web page's
/// field, by way of its window — and to nobody when the chain would end at
/// gpui's own app delegate (its handler there would replay a menu item by a
/// tag a programmatic send does not carry).
#[cfg(target_os = "macos")]
fn forward_edit(selector: &str) {
    use objc2::runtime::Sel;
    use objc2_app_kit::NSApplication;

    let Some(mtm) = objc2_foundation::MainThreadMarker::new() else {
        return;
    };
    let app = NSApplication::sharedApplication(mtm);
    let action = Sel::register(selector);
    // SAFETY: main thread; `targetForAction:` walks the responder chain and
    // answers the object that would receive `action`, or nil.
    let Some(target) = (unsafe { app.targetForAction(action) }) else {
        return;
    };
    let target_ptr: *const objc2::runtime::AnyObject = &*target;
    let app_ptr: *const objc2::runtime::AnyObject = std::ptr::from_ref(&*app).cast();
    // SAFETY: the delegate is gpui's application delegate, alive for the
    // process; only its address is compared.
    let delegate_ptr: *const objc2::runtime::AnyObject = unsafe { app.delegate() }
        .map_or(std::ptr::null(), |delegate| {
            std::ptr::from_ref(&*delegate).cast()
        });
    if std::ptr::eq(target_ptr, app_ptr) || std::ptr::eq(target_ptr, delegate_ptr) {
        return;
    }
    // SAFETY: `target` responds to `action` (that is what found it); the
    // standard edit actions take one sender argument, nil here.
    unsafe {
        app.sendAction_to_from(action, Some(&target), None);
    }
}

#[cfg(not(target_os = "macos"))]
fn forward_edit(_selector: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys_of(binding: &KeyBinding) -> Vec<String> {
        binding
            .keystrokes()
            .iter()
            .map(gpui::KeybindingKeystroke::unparse)
            .collect()
    }

    /// Spec 082 RJ17 (G68, DX9): ⌘W is bound, to Close Window — the action
    /// whose handler goes through the same RD14 hold as the window's button.
    /// macOS key equivalents: elsewhere gpui spells ⌘ as `super`.
    #[cfg(target_os = "macos")]
    #[test]
    fn cmd_w_closes_the_window() {
        let bindings = mac_key_bindings();
        let close = bindings
            .iter()
            .find(|binding| binding.action().partial_eq(&CloseWindow))
            .unwrap_or_else(|| unreachable!("Close Window has no key"));
        assert_eq!(keys_of(close), ["cmd-w"]);
        // And ⌘Q stays Quit.
        assert!(bindings.iter().any(|binding| {
            binding.action().partial_eq(&Quit) && keys_of(binding) == ["cmd-q"]
        }));
    }

    /// Spec 095: the Edit chords label the menu and nothing else — bound in a
    /// context no element sets, so a well's own ⌘C/⌘V/⌘Z handling is never
    /// beaten to the key by a gpui action.
    #[cfg(target_os = "macos")]
    #[test]
    fn edit_chords_belong_to_the_menu_only() {
        let bindings = mac_key_bindings();
        for (action, chord) in [
            (&Undo as &dyn gpui::Action, "cmd-z"),
            (&Redo, "cmd-shift-z"),
            (&Cut, "cmd-x"),
            (&Copy, "cmd-c"),
            (&Paste, "cmd-v"),
            (&SelectAll, "cmd-a"),
        ] {
            let binding = bindings
                .iter()
                .find(|binding| binding.action().partial_eq(action))
                .unwrap_or_else(|| unreachable!("{chord} has no binding"));
            assert_eq!(keys_of(binding), [chord]);
            let context = binding
                .predicate()
                .unwrap_or_else(|| unreachable!("{chord} is bound everywhere"));
            assert_eq!(context.to_string(), MENU_ONLY);
        }
    }

    /// Spec 095: About, an Edit menu whose clipboard items are the native
    /// ones, View and Window; every label resolves in every language — none
    /// shows its key.
    #[test]
    fn the_menus_are_standard_and_localized() {
        for (tag, loc) in Loc::every_language() {
            let menus = menus(&loc);
            assert_eq!(menus.len(), 4, "{tag}");
            let titles: Vec<String> = menus.iter().map(|menu| menu.name.to_string()).collect();
            for menu in &menus {
                for item in &menu.items {
                    if let MenuItem::Action { name, .. } = item {
                        assert!(
                            !name.is_empty() && !name.contains("componentsUi."),
                            "{tag}: {name}"
                        );
                    }
                }
            }
            for title in &titles[1..] {
                assert!(!title.contains("componentsUi."), "{tag}: {title}");
            }
            let native: Vec<&str> = menus[1]
                .items
                .iter()
                .filter_map(|item| match item {
                    MenuItem::Action { os_action, .. } => *os_action,
                    _ => None,
                })
                .map(|os_action| match os_action {
                    OsAction::Undo => "undo",
                    OsAction::Redo => "redo",
                    OsAction::Cut => "cut",
                    OsAction::Copy => "copy",
                    OsAction::Paste => "paste",
                    OsAction::SelectAll => "selectAll",
                })
                .collect();
            assert_eq!(
                native,
                ["undo", "redo", "cut", "copy", "paste", "selectAll"],
                "{tag}"
            );
        }
        let en = menus(&Loc::for_tag("en"));
        let about = match &en[0].items[0] {
            MenuItem::Action { name, action, .. } => {
                assert!(action.partial_eq(&About));
                name.to_string()
            }
            _ => unreachable!("About leads the application menu"),
        };
        assert_eq!(about, "About Vela Wallet");
        assert_eq!(en[1].name.to_string(), "Edit");
    }
}
