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
        CloseWindow,
        GoWallet,
        GoContacts,
        GoExplore,
        GoSettings,
        NewSend,
        Receive,
        Refresh,
        OpenSettings,
        CommandPalette,
        Find
    ]
);

/// One of the wallet's own commands (N-D2) — what a menu item, a shortcut
/// and a palette row run (`WalletPage::run_command`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Section(crate::wallet::page::Section),
    Send,
    Receive,
    Scan,
    Refresh,
    Find,
    Palette,
}

/// A command with its shortcut on macOS and on Windows/Linux — the one table
/// the bindings, the palette and README's shortcut table follow.
#[derive(Clone, Copy, Debug)]
pub struct CommandEntry {
    pub command: Command,
    /// gpui's spelling on macOS (`cmd-1`); `None` for a palette-only row.
    pub mac: Option<&'static str>,
    /// gpui's spelling on Windows and Linux (`ctrl-1`).
    pub other: Option<&'static str>,
    /// How the shortcut is shown: macOS's glyphs, the others' words.
    pub mac_label: &'static str,
    pub other_label: &'static str,
}

impl CommandEntry {
    /// The shortcut as this platform shows it.
    #[must_use]
    pub fn shortcut(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            self.mac_label
        } else {
            self.other_label
        }
    }
}

/// Every command, in the palette's order.
pub const COMMANDS: [CommandEntry; 10] = {
    use crate::wallet::page::Section;
    const fn entry(
        command: Command,
        mac: &'static str,
        other: &'static str,
        mac_label: &'static str,
        other_label: &'static str,
    ) -> CommandEntry {
        CommandEntry {
            command,
            mac: Some(mac),
            other: Some(other),
            mac_label,
            other_label,
        }
    }
    [
        entry(Command::Send, "cmd-n", "ctrl-n", "⌘N", "Ctrl+N"),
        entry(
            Command::Receive,
            "shift-cmd-r",
            "ctrl-shift-r",
            "⇧⌘R",
            "Ctrl+Shift+R",
        ),
        CommandEntry {
            command: Command::Scan,
            mac: None,
            other: None,
            mac_label: "",
            other_label: "",
        },
        entry(Command::Refresh, "cmd-r", "ctrl-r", "⌘R", "Ctrl+R"),
        entry(
            Command::Section(Section::Wallet),
            "cmd-1",
            "ctrl-1",
            "⌘1",
            "Ctrl+1",
        ),
        entry(
            Command::Section(Section::Contacts),
            "cmd-2",
            "ctrl-2",
            "⌘2",
            "Ctrl+2",
        ),
        entry(
            Command::Section(Section::Explore),
            "cmd-3",
            "ctrl-3",
            "⌘3",
            "Ctrl+3",
        ),
        entry(
            Command::Section(Section::Settings),
            "cmd-4",
            "ctrl-4",
            "⌘4",
            "Ctrl+4",
        ),
        entry(Command::Find, "cmd-f", "ctrl-f", "⌘F", "Ctrl+F"),
        entry(Command::Palette, "cmd-k", "ctrl-k", "⌘K", "Ctrl+K"),
    ]
};

/// The shortcut that runs `command` here: its key spelled for this
/// platform, and the action it dispatches.
fn binding(entry: &CommandEntry, mac: bool) -> Option<KeyBinding> {
    use crate::wallet::page::Section;
    let keys = if mac { entry.mac } else { entry.other }?;
    Some(match entry.command {
        Command::Section(Section::Wallet) => KeyBinding::new(keys, GoWallet, None),
        Command::Section(Section::Contacts) => KeyBinding::new(keys, GoContacts, None),
        Command::Section(Section::Explore) => KeyBinding::new(keys, GoExplore, None),
        Command::Section(Section::Settings) => KeyBinding::new(keys, GoSettings, None),
        Command::Send => KeyBinding::new(keys, NewSend, None),
        Command::Receive => KeyBinding::new(keys, Receive, None),
        Command::Refresh => KeyBinding::new(keys, Refresh, None),
        Command::Find => KeyBinding::new(keys, Find, None),
        Command::Palette => KeyBinding::new(keys, CommandPalette, None),
        Command::Scan => return None,
    })
}

/// The wallet's own shortcuts (N-D2), on every desktop: the table above, plus
/// Settings' ⌘, (Ctrl+,) — the convention every app keeps.
#[must_use]
pub fn wallet_key_bindings(mac: bool) -> Vec<KeyBinding> {
    let mut bindings: Vec<KeyBinding> = COMMANDS
        .iter()
        .filter_map(|entry| binding(entry, mac))
        .collect();
    bindings.push(KeyBinding::new(
        if mac { "cmd-," } else { "ctrl-," },
        OpenSettings,
        None,
    ));
    // F5 reads the balances again too, as it reloads a page everywhere else.
    if !mac {
        bindings.push(KeyBinding::new("f5", Refresh, None));
    }
    bindings
}

/// Run `command` on the wallet's page, in its window — when there is one: a
/// shortcut pressed on onboarding has nothing to act on.
/// A command from outside the window's own keys (the menu-bar extra).
pub(crate) fn run_on_page(command: Command, cx: &mut App) {
    on_page(command, cx);
}

fn on_page(command: Command, cx: &mut App) {
    let Some(page) = crate::agent::page(cx) else {
        return;
    };
    let Some(window) = cx.active_window().or_else(|| cx.windows().first().copied()) else {
        return;
    };
    window
        .update(cx, |_, window, cx| {
            let _ = page.update(cx, |page, cx| page.run_command(command, window, cx));
        })
        .ok();
}

/// The clipboard's text, to the wallet page — only when no field took the
/// paste (that is when this action reaches the app).
fn paste_onto_page(cx: &mut App) {
    let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) else {
        return;
    };
    let Some(page) = crate::agent::page(cx) else {
        return;
    };
    let Some(window) = cx.active_window().or_else(|| cx.windows().first().copied()) else {
        return;
    };
    window
        .update(cx, |_, window, cx| {
            let _ = page.update(cx, |page, cx| page.take_inbound(&text, window, cx));
        })
        .ok();
}

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
            // ⌘, — Settings, where every Mac app keeps it (N-D2).
            MenuItem::action(loc.t("componentsUi.mainNav.settings"), OpenSettings),
            MenuItem::separator(),
            MenuItem::action(t("hide"), HideApp),
            MenuItem::action(t("hideOthers"), HideOthers),
            MenuItem::action(t("showAll"), ShowAll),
            MenuItem::separator(),
            MenuItem::action(t("quit"), Quit),
        ]),
        // N-D2: the wallet's own commands, where a document app keeps File.
        Menu::new(loc.t("componentsUi.mainNav.wallet")).items(vec![
            MenuItem::action(loc.t("componentsUi.dock.send"), NewSend),
            MenuItem::action(loc.t("componentsUi.dock.receive"), Receive),
            MenuItem::action(loc.t("explore.refresh"), Refresh),
            MenuItem::separator(),
            MenuItem::action(t("commandPalette"), CommandPalette),
        ]),
        Menu::new(t("edit")).items(vec![
            MenuItem::os_action(t("undo"), Undo, OsAction::Undo),
            MenuItem::os_action(t("redo"), Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action(t("cut"), Cut, OsAction::Cut),
            MenuItem::os_action(t("copy"), Copy, OsAction::Copy),
            MenuItem::os_action(t("paste"), Paste, OsAction::Paste),
            MenuItem::os_action(t("selectAll"), SelectAll, OsAction::SelectAll),
            // ⌘F, where every Mac app keeps Find: the section's search (the
            // palette where it has none).
            MenuItem::separator(),
            MenuItem::action(t("find"), Find),
        ]),
        // The sections, ⌘1–⌘4, as the sidebar orders them.
        Menu::new(t("view")).items(vec![
            MenuItem::action(loc.t("componentsUi.mainNav.wallet"), GoWallet),
            MenuItem::action(loc.t("componentsUi.mainNav.contacts"), GoContacts),
            MenuItem::action(loc.t("componentsUi.mainNav.explore"), GoExplore),
            MenuItem::action(loc.t("componentsUi.mainNav.settings"), GoSettings),
            MenuItem::separator(),
            MenuItem::action(t("fullScreen"), ToggleFullScreen),
        ]),
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
    // N-D2: the wallet's commands — each the press of the control it names.
    {
        use crate::wallet::page::Section;
        cx.on_action(|_: &GoWallet, cx| on_page(Command::Section(Section::Wallet), cx));
        cx.on_action(|_: &GoContacts, cx| on_page(Command::Section(Section::Contacts), cx));
        cx.on_action(|_: &GoExplore, cx| on_page(Command::Section(Section::Explore), cx));
        cx.on_action(|_: &GoSettings, cx| on_page(Command::Section(Section::Settings), cx));
        cx.on_action(|_: &OpenSettings, cx| on_page(Command::Section(Section::Settings), cx));
    }
    cx.on_action(|_: &NewSend, cx| on_page(Command::Send, cx));
    cx.on_action(|_: &Receive, cx| on_page(Command::Receive, cx));
    cx.on_action(|_: &Refresh, cx| on_page(Command::Refresh, cx));
    cx.on_action(|_: &Find, cx| on_page(Command::Find, cx));
    cx.on_action(|_: &CommandPalette, cx| on_page(Command::Palette, cx));
    cx.on_action(|_: &About, _| show_about());
    cx.on_action(|_: &Undo, _| forward_edit("undo:"));
    cx.on_action(|_: &Redo, _| forward_edit("redo:"));
    // Cut/Copy/Paste/Select All arrive here only when no responder took the
    // native message — nothing focused takes text. Handled, so gpui does not
    // report an action without a handler; nothing to do.
    cx.on_action(|_: &Cut, _| {});
    cx.on_action(|_: &Copy, _| {});
    // N-D3: a Paste no field took is a link or an address pasted onto the
    // window — the wallet reads it (`WalletPage::take_inbound`).
    cx.on_action(|_: &Paste, cx| paste_onto_page(cx));
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
            assert_eq!(menus.len(), 5, "{tag}");
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
            let native: Vec<&str> = menus[2]
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
        assert_eq!(en[1].name.to_string(), "Wallet");
        assert_eq!(en[2].name.to_string(), "Edit");
        // Find (Edit) and the command palette (Wallet), in their own words.
        let named = |menu: &Menu, action: &dyn gpui::Action| {
            menu.items.iter().find_map(|item| match item {
                MenuItem::Action {
                    name, action: own, ..
                } if own.partial_eq(action) => Some(name.to_string()),
                _ => None,
            })
        };
        assert_eq!(named(&en[2], &Find).as_deref(), Some("Find"));
        assert_eq!(
            named(&en[1], &CommandPalette).as_deref(),
            Some("Command Palette")
        );
        let zh = menus(&Loc::for_tag("zh"));
        assert_eq!(named(&zh[2], &Find).as_deref(), Some("查找"));
        assert_eq!(named(&zh[1], &CommandPalette).as_deref(), Some("命令面板"));
    }

    /// N-D2: the wallet's shortcuts — ⌘1–4, ⌘N, ⇧⌘R, ⌘R, ⌘,, ⌘K, ⌘F on
    /// macOS, the same with Ctrl elsewhere (and F5) — each bound once, none
    /// shadowing a standard menu chord, all global (no key context).
    #[test]
    fn the_wallet_shortcuts_are_one_table_on_every_desktop() {
        for mac in [true, false] {
            let bindings = wallet_key_bindings(mac);
            let mut keys: Vec<Vec<String>> = bindings.iter().map(keys_of).collect();
            let all = keys.len();
            keys.sort();
            keys.dedup();
            assert_eq!(keys.len(), all, "a shortcut bound twice (mac: {mac})");
            let standard: Vec<Vec<String>> = mac_key_bindings().iter().map(keys_of).collect();
            if mac {
                for key in &keys {
                    assert!(!standard.contains(key), "{key:?} shadows a menu chord");
                }
            }
            for binding in &bindings {
                assert!(binding.predicate().is_none(), "a wallet shortcut is global");
            }
            let spelled: Vec<String> = keys.iter().map(|k| k.join(" ")).collect();
            // gpui spells ⌘ `cmd` on macOS and `super` elsewhere.
            let modifiers: &[&str] = if mac { &["cmd", "super"] } else { &["ctrl"] };
            for digit in 1..=4 {
                assert!(
                    spelled
                        .iter()
                        .any(|k| modifiers.iter().any(|m| k.contains(m))
                            && k.ends_with(&digit.to_string())),
                    "{modifiers:?}-{digit} (mac: {mac}): {spelled:?}"
                );
            }
        }
        assert_eq!(wallet_key_bindings(true).len(), 10);
        assert_eq!(wallet_key_bindings(false).len(), 11, "F5 too");
        // Every command but the palette-only Scan has both spellings.
        for entry in COMMANDS {
            assert_eq!(entry.mac.is_some(), entry.command != Command::Scan);
            assert_eq!(entry.other.is_some(), entry.command != Command::Scan);
        }
    }
}
