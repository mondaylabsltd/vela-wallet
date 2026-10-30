//! N-D2: the wallet's commands from the keyboard — ⌘1–⌘4 (Ctrl+1–4) for the
//! sections, ⌘N Send, ⇧⌘R Receive, ⌘R Refresh, ⌘, Settings, ⌘F the section's
//! search, and ⌘K the command palette that lists every one of them with its
//! shortcut.
//!
//! The palette is also the menu Windows and Linux have: gpui draws no menu
//! bar there, so the same commands, the same words and the same shortcuts
//! the macOS menu bar shows are one keystroke (Ctrl+K) away, typed to
//! narrow and Enter to run. Every command does exactly what its button does
//! — a command is a shortcut to a screen, never to a signature: Send opens
//! the form, and nothing moves until the person confirms there.

use gpui::{ClickEvent, KeyDownEvent};

use super::*;
use crate::app_menu::Command;

/// The palette's state while it is open.
#[derive(Clone, Debug, Default)]
pub(super) struct Palette {
    /// What is typed — narrows the list by the commands' words.
    pub query: String,
    /// The highlighted row among the ones shown.
    pub selected: usize,
}

impl WalletPage {
    /// Run one of the wallet's commands (the menu bar, a shortcut, the
    /// palette's row). Each is the press of the control it names.
    pub(crate) fn run_command(
        &mut self,
        command: Command,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.palette = None;
        match command {
            Command::Section(section) => self.go_to_section(section, cx),
            Command::Send => {
                self.go_to_section(Section::Wallet, cx);
                self.enter_flow(FlowEntry::Send, cx);
            }
            Command::Receive => {
                self.go_to_section(Section::Wallet, cx);
                self.enter_flow(FlowEntry::Receive, cx);
            }
            Command::Scan => {
                self.go_to_section(Section::Wallet, cx);
                self.enter_flow(FlowEntry::Scan, cx);
            }
            Command::Refresh => self.refresh_by_hand(cx),
            Command::Find => self.find(window, cx),
            Command::Palette => self.open_palette(window, cx),
        }
        cx.notify();
    }

    /// ⌘F: the search of the section on screen — the address book's, the
    /// browser's address bar — and the palette where the section has none.
    fn find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.section {
            Section::Contacts => window.focus(&self.contacts_query_focus, cx),
            Section::Explore => window.focus(&self.address_focus, cx),
            Section::Wallet | Section::Settings => self.open_palette(window, cx),
        }
    }

    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.identity.is_none() {
            return;
        }
        self.palette = Some(Palette::default());
        window.focus(&self.palette_focus, cx);
        cx.notify();
    }

    /// The commands the palette offers now, with their words and their
    /// shortcut as this platform spells it, narrowed by what is typed.
    fn palette_rows(&self, query: &str) -> Vec<(Command, SharedString, &'static str)> {
        let query = query.trim().to_lowercase();
        crate::app_menu::COMMANDS
            .iter()
            .filter(|entry| entry.command != Command::Palette)
            // Find is a row only where the section has a search of its own;
            // elsewhere ⌘F is this palette, already open.
            .filter(|entry| {
                entry.command != Command::Find
                    || matches!(self.section, Section::Contacts | Section::Explore)
            })
            .filter(|entry| match entry.command {
                Command::Section(section) => section.available(),
                _ => true,
            })
            .map(|entry| {
                (
                    entry.command,
                    self.command_words(entry.command),
                    entry.shortcut(),
                )
            })
            .filter(|(_, words, _)| query.is_empty() || words.to_lowercase().contains(&query))
            .collect()
    }

    /// A command's words — the corpus's, the ones its own control shows.
    fn command_words(&self, command: Command) -> SharedString {
        match command {
            Command::Section(Section::Wallet) => self.strings.nav_wallet.clone(),
            Command::Section(Section::Contacts) => self.strings.nav_contacts.clone(),
            Command::Section(Section::Explore) => self.strings.nav_explore.clone(),
            Command::Section(Section::Settings) => self.strings.nav_settings.clone(),
            Command::Send => self.strings.action_send.clone(),
            Command::Receive => self.strings.action_receive.clone(),
            Command::Scan => self.strings.action_scan.clone(),
            Command::Refresh => self.loc.t("explore.refresh"),
            // The menu bar's own words for the two (`componentsUi.appMenu`),
            // so the palette and the menu name them alike.
            Command::Find => self.loc.t("componentsUi.appMenu.find"),
            Command::Palette => self.loc.t("componentsUi.appMenu.commandPalette"),
        }
    }

    /// The palette, over everything (⌘K / Ctrl+K): a search well, the
    /// commands under it, ↑/↓ to move, Enter to run, Esc to close.
    pub(super) fn palette_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let palette = self.palette.clone()?;
        let rows = self.palette_rows(&palette.query);
        let selected = palette.selected.min(rows.len().saturating_sub(1));
        let placeholder = self.loc.t("componentsUi.commandBar.placeholder");
        let input = crate::ui::search_input(
            "palette-query",
            theme,
            &palette.query,
            placeholder.clone(),
            &self.palette_focus,
            window,
            {
                let page = cx.entity();
                move |text: String, _: &mut Window, cx: &mut gpui::App| {
                    page.update(cx, |this: &mut Self, cx| {
                        if let Some(palette) = this.palette.as_mut() {
                            palette.query = text.clone();
                            palette.selected = 0;
                        }
                        cx.notify();
                    });
                }
            },
        );
        let mut list = div().flex().flex_col().gap(px(2.)).pt(px(8.));
        for (index, (command, words, shortcut)) in rows.iter().enumerate() {
            let command = *command;
            let on = index == selected;
            list = list.child(
                div()
                    .id(("palette-row", index as u64))
                    .a11y_button(crate::ui::a11y::sentence([words.as_ref(), shortcut]))
                    .aria_keyshortcuts(*shortcut)
                    .aria_selected(on)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(10.))
                    .cursor_pointer()
                    .when(on, |row| row.bg(theme.bg_sunken))
                    .hover(|row| row.bg(theme.bg_sunken))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(words.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(SharedString::from(*shortcut)),
                    )
                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.run_command(command, window, cx);
                    })),
            );
        }
        let count = rows.len();
        let picked = rows.get(selected).map(|(command, ..)| *command);
        let body = div()
            .flex()
            .flex_col()
            .capture_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                let ks = &event.keystroke;
                match ks.key.as_str() {
                    "down" | "up" if count > 0 => {
                        if let Some(palette) = this.palette.as_mut() {
                            let at = palette.selected.min(count - 1);
                            palette.selected = if ks.key == "down" {
                                (at + 1) % count
                            } else {
                                (at + count - 1) % count
                            };
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }
                    "enter" => {
                        if let Some(command) = picked {
                            this.run_command(command, window, cx);
                        }
                        cx.stop_propagation();
                    }
                    "escape" => {
                        this.palette = None;
                        cx.stop_propagation();
                        cx.notify();
                    }
                    _ => {}
                }
            }))
            .child(input)
            .child(list);
        Some(
            crate::ui::dialog::dialog(
                "command-palette",
                theme,
                window,
                placeholder,
                None,
                self.dialog_close_icon(theme),
                body,
                &self.dialog_scroll("command-palette"),
                Self::closer(cx, |this, _| this.palette = None),
            )
            .into_any_element(),
        )
    }
}
