//! The AI agent on the wallet page (spec 103, spec 104 phase 5): what only
//! the page can do for an intent — open a screen as its own row would, set a
//! preference it redraws by, erase, clear a store, run the send column — and
//! what the person sees of the agent: Settings → AI agent, the bar over the
//! page, and the approval sheet for its manage-level requests.

use gpui::{AnyElement, App};

use vela_core::app::intents::{Action, Preference, Screen};

use super::*;
use crate::agent::{self, Agent, Link};
use crate::settings::components::toggle_row;
use crate::settings::fixtures::Pill;

/// The core's settings page names, as this shell's panels. `community` has no
/// panel of its own here: its links are rows of the settings column.
fn settings_page(name: &str) -> Option<SettingsPage> {
    Some(match name {
        "account" | "signing-venue" => SettingsPage::Account,
        "appearance" => SettingsPage::Appearance,
        "localization" => SettingsPage::Localization,
        "networks" => SettingsPage::Networks,
        "rpc-providers" => SettingsPage::RpcProviders,
        "endpoints" => SettingsPage::Endpoints,
        "storage" => SettingsPage::Storage,
        "fee-speed" => SettingsPage::FeeSpeed,
        "feedback" => SettingsPage::Feedback,
        "about" => SettingsPage::About,
        "agent" => SettingsPage::Agent,
        "signing-pages" => SettingsPage::Signing,
        _ => return None,
    })
}

impl WalletPage {
    // -- what the intents ask of the page -----------------------------------

    /// The page the person is on, as `vela_state` names it.
    pub(crate) fn agent_page_name(&self) -> &'static str {
        match self.section {
            Section::Wallet => "wallet",
            Section::Contacts => "contacts",
            Section::Explore => "explore",
            Section::Settings => "settings",
        }
    }

    /// The send open in the column, if any.
    pub(crate) fn agent_send(&self) -> Option<gpui::Entity<SendHost>> {
        self.send_host.clone()
    }

    /// The dApp signing request on its sheet, as the core's view has it.
    pub(crate) fn agent_signing(&self, cx: &App) -> Option<serde_json::Value> {
        #[cfg(not(target_os = "linux"))]
        {
            let host = self.signing_host.as_ref()?;
            let request = host.read(cx).view.request.clone()?;
            serde_json::to_value(request).ok()
        }
        // No in-app browser on Linux: no dApp asks to sign here.
        #[cfg(target_os = "linux")]
        {
            let _ = cx;
            None
        }
    }

    /// Open the send column with `params`, as a token's own 转账 does.
    pub(crate) fn agent_open_send(
        &mut self,
        params: SendOpenParams,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<SendHost>> {
        self.identity.as_ref()?;
        self.section = Section::Wallet;
        self.enter_flow_with(FlowEntry::Send, params, cx);
        cx.notify();
        self.send_host.clone()
    }

    /// Leave the send column, as its own Back off the form does.
    pub(crate) fn agent_close_send(&mut self, cx: &mut Context<Self>) {
        self.let_send_go(cx);
        self.send_fee_picker = false;
        self.flows.clear();
        self.panel = PanelId::None;
        cx.notify();
    }

    /// Open a screen as its own row would. `Some(why not)` when it cannot.
    pub(crate) fn agent_show(&mut self, screen: &Screen, cx: &mut Context<Self>) -> Option<String> {
        if self.identity.is_none() {
            return Some("the wallet is not open".to_owned());
        }
        match screen {
            Screen::Home => self.section = Section::Wallet,
            Screen::Contacts => {
                self.section = Section::Contacts;
                self.group = None;
            }
            Screen::Contact { address } => {
                self.section = Section::Contacts;
                if !self.open_contact_by_address(address, cx) {
                    return Some("no contact has that address".to_owned());
                }
            }
            Screen::Group { id } => {
                self.section = Section::Contacts;
                let groups = self.group_models(cx);
                let Some(index) = groups.iter().position(|(gid, _, _)| gid.as_ref() == id) else {
                    return Some("no group has that id".to_owned());
                };
                self.group = Some(index);
                self.menu = None;
            }
            Screen::Settings { page } => {
                self.section = Section::Settings;
                if let Some(name) = page {
                    let Some(page) = settings_page(name) else {
                        return Some(format!("the desktop has no settings page called {name}"));
                    };
                    self.settings_page = page;
                }
            }
            Screen::Asset { chain_id, token } => {
                let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
                let index = view.tokens.iter().position(|held| {
                    held.chain_id == *chain_id
                        && match (token, &held.token_address) {
                            (None, None) => true,
                            (Some(wanted), Some(address)) => wanted.eq_ignore_ascii_case(address),
                            _ => false,
                        }
                });
                let Some(index) = index else {
                    return Some("the wallet holds no such token on that chain".to_owned());
                };
                self.section = Section::Wallet;
                self.asset_detail = Some(index);
                self.panel = PanelId::AssetDetail;
            }
            Screen::Activity { id } => {
                self.section = Section::Wallet;
                self.asset_detail = None;
                self.tx_detail = Some(id.clone());
                self.tx_technical = None;
                self.enter_flow(FlowEntry::TxDetail, cx);
            }
            Screen::Receive { chain_id } => {
                self.section = Section::Wallet;
                self.enter_flow(FlowEntry::Receive, cx);
                if let Some(chain_id) = chain_id {
                    self.receive_chain = *chain_id;
                    self.push_step(FlowStep::ReceiveQr);
                }
            }
        }
        cx.notify();
        None
    }

    /// A preference the page redraws by, set as its own control sets it.
    pub(crate) fn agent_preference(
        &mut self,
        preference: &Preference,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match preference {
            Preference::Theme(value) => {
                crate::executor::preferences::set_theme(value);
                // "Follow System" reads the window's appearance again.
                self.mode = ThemeMode::detect(window);
            }
            Preference::Language(value) => {
                crate::executor::preferences::set_language(value);
                self.relocalize();
                // The menu bar speaks the new language too (spec 095).
                crate::app_menu::install(cx);
            }
            Preference::TextScale(value) => {
                crate::executor::preferences::set_text_scale(value);
                // Every text on screen changes size, not only this page's.
                window.refresh();
            }
            Preference::NumberFormat(value) => format_prefs::set_number_word(value),
            Preference::DateFormat(value) => format_prefs::set_date_word(value),
            Preference::TimeFormat(value) => format_prefs::set_time_word(value),
            // Set by their machines, not here (`agent::perform`).
            Preference::Currency(_) | Preference::FeeSpeed(_) | Preference::Analytics(_) => {}
        }
        cx.notify();
    }

    /// Erase This Device, as its confirmed button does. `Some(why)` when keys
    /// survived it.
    pub(crate) fn agent_erase(&mut self, cx: &mut Context<Self>) -> Option<String> {
        self.erase_device(cx);
        self.erase_failed.as_ref().map(|left| {
            format!(
                "the erase did not finish: {} key(s) are still here",
                left.len()
            )
        })
    }

    /// One storage row's Clear. The connected sites are its own row's
    /// "disconnect all".
    pub(crate) fn agent_clear_storage(&mut self, item: &str, cx: &mut Context<Self>) {
        if item == "dapps" {
            let host = self.browser_host(cx);
            host.update(cx, |host, cx| host.dispatch(DbrEvent::RevokeAll, cx));
            cx.notify();
            return;
        }
        if let Some(id) = vela_core::storage_catalog::ITEMS
            .iter()
            .find(|row| row.id == item)
            .map(|row| row.id)
        {
            self.clear_storage_row(id, cx);
        }
    }

    pub(crate) fn agent_clear_caches(&mut self, cx: &mut Context<Self>) {
        self.clear_caches(cx);
    }

    /// The hero's ↻, as the person's own press.
    pub(crate) fn agent_refresh(&mut self, cx: &mut Context<Self>) {
        self.refresh_by_hand(cx);
    }
}

// ---------------------------------------------------------------------------
// What the person sees of the agent
// ---------------------------------------------------------------------------

/// What an action an agent asked for is called: `settings.agent.action.<it>`.
fn action_key(kind: &str) -> String {
    let mut key = String::from("settings.agent.action.");
    let mut upper = false;
    for ch in kind.chars() {
        if ch == '_' {
            upper = true;
        } else if upper {
            key.extend(ch.to_uppercase());
            upper = false;
        } else {
            key.push(ch);
        }
    }
    key
}

/// The bridge field's placeholder: the shape of a client endpoint.
const BRIDGE_PLACEHOLDER: &str = "wss://bridge.example.com/connect";

/// How long an agent's request is up before its Allow counts (ms).
const APPROVAL_ARM_MS: f64 = 700.;

impl WalletPage {
    fn agent_text(&self, key: &str) -> SharedString {
        self.loc.t(key)
    }

    fn agent_action_name(&self, kind: &str) -> SharedString {
        let key = action_key(kind);
        let name = self.loc.t(&key);
        // A catalog action this build has no words for reads by its own name.
        if name.as_ref() == key {
            SharedString::from(kind.to_owned())
        } else {
            name
        }
    }

    /// The connection, as a pill: off, connecting, online, offline, refused.
    pub(super) fn agent_status_pill(&self, cx: &App) -> Pill {
        let agent = cx.try_global::<Agent>();
        let ready = agent.is_some_and(|agent| agent.config.ready());
        let (tone, key) = match agent.map(|agent| &agent.link) {
            _ if !ready => (Tone::Neutral, "settings.agent.statusOff"),
            Some(Link::Online) => (Tone::Ok, "settings.agent.statusOnline"),
            Some(Link::Offline) => (Tone::Warn, "settings.agent.statusOffline"),
            Some(Link::Closed(_)) => (Tone::Error, "settings.agent.statusRefused"),
            _ => (Tone::Neutral, "settings.agent.statusConnecting"),
        };
        settings_fixtures::pill(tone, self.agent_text(key))
    }

    /// The person's edits of the bridge and token fields, saved when the
    /// field is left (or Enter) — not on every key, which would dial a
    /// half-typed token once per character.
    pub(super) fn agent_commit(&mut self, cx: &mut Context<Self>) {
        let url = self.agent_url_draft.take();
        let token = self.agent_token_draft.take();
        if url.is_none() && token.is_none() {
            return;
        }
        agent::invoke_person(
            Action::ConfigureAgent {
                enabled: None,
                bridge_url: url,
                client_token: token,
            },
            cx,
        );
        cx.notify();
    }

    /// Settings → AI agent (spec 103 D1): the switch, the bridge, the token,
    /// and what an agent connects to. The person's alone — every change goes
    /// through the `configure_agent` intent, which an agent may never ask for.
    pub(super) fn settings_agent(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let config = cx
            .try_global::<Agent>()
            .map(|agent| agent.config.clone())
            .unwrap_or_default();
        let page = cx.entity();
        let mut col = div().flex().flex_col().gap(px(24.)).max_w(px(560.)).child(
            div()
                .text_size(theme::text_row_sub())
                .line_height(px(20.))
                .text_color(theme.fg_muted)
                .child(crate::ui::prose(
                    self.agent_text("settings.agent.description"),
                )),
        );

        let on = config.enabled;
        col = col.child(toggle_row(
            "settings-agent",
            theme,
            self.agent_text("settings.agent.enable"),
            self.agent_text("settings.agent.subtitle"),
            on,
            move |_, _, cx| {
                agent::invoke_person(
                    Action::ConfigureAgent {
                        enabled: Some(!on),
                        bridge_url: None,
                        client_token: None,
                    },
                    cx,
                );
            },
        ));

        let url = self.agent_url_draft.clone().unwrap_or(config.url);
        let invalid = !url.trim().is_empty() && agent::settings::bridge_url_problem(&url).is_some();
        let hint = if invalid {
            self.agent_text("settings.agent.bridgeInvalid")
        } else {
            self.agent_text("settings.agent.bridgeHint")
        };
        col = col.child(editable_url_field(
            "agent-bridge",
            theme,
            Some(self.agent_text("settings.agent.bridge")),
            &url,
            SharedString::from(BRIDGE_PLACEHOLDER),
            None,
            Some(hint),
            invalid.then_some(Tone::Error),
            &self.agent_url_focus,
            window,
            {
                let page = page.clone();
                move |text: String, _: &mut Window, cx: &mut App| {
                    page.update(cx, |this, cx| {
                        this.agent_url_draft = Some(text);
                        cx.notify();
                    });
                }
            },
            {
                let page = page.clone();
                move |_: &mut Window, cx: &mut App| {
                    page.update(cx, |this, cx| this.agent_commit(cx))
                }
            },
        ));

        // The token is a credential: drawn as bullets, like a security key's PIN.
        let token = self.agent_token_draft.clone().unwrap_or(config.token);
        let strings = crate::ui::NameFieldStrings {
            label: SharedString::from(""),
            placeholder: SharedString::from(""),
            helper: SharedString::from(""),
            too_long_hint: SharedString::from(""),
        };
        let token_field = crate::ui::text_field(
            "agent-token",
            theme,
            &strings,
            &token,
            false,
            true,
            &self.agent_token_focus,
            window,
            {
                let page = page.clone();
                move |text: String, _: &mut Window, cx: &mut App| {
                    page.update(cx, |this, cx| {
                        this.agent_token_draft = Some(text);
                        cx.notify();
                    });
                }
            },
        );
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .on_key_down({
                    let page = page.clone();
                    move |event: &gpui::KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" {
                            page.update(cx, |this, cx| this.agent_commit(cx));
                        }
                    }
                })
                .child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(self.agent_text("settings.agent.token")),
                )
                .child(token_field)
                .child(
                    div()
                        .text_size(theme::text_label())
                        .line_height(px(16.))
                        .text_color(theme.fg_subtle)
                        .child(crate::ui::prose(
                            self.agent_text("settings.agent.tokenHint"),
                        )),
                ),
        );

        if let Some(endpoint) = agent::settings::mcp_endpoint(&url) {
            col = col.child(url_field(
                theme,
                Some(self.agent_text("settings.agent.endpoint")),
                SharedString::from(endpoint),
                None,
                Some(self.agent_text("settings.agent.endpointHint")),
                None,
                None,
            ));
        }

        // The agent's latest call, by its action's name and how it ended.
        let latest = agent::book(cx).and_then(|book| book.activity.first().cloned());
        if let Some(latest) = latest {
            let how = match &latest.outcome {
                Some(agent::intents::Outcome::Rejected { reason }) => {
                    serde_json::to_value(reason.code)
                        .ok()
                        .and_then(|code| code.as_str().map(str::to_owned))
                        .unwrap_or_default()
                }
                Some(agent::intents::Outcome::Accepted { .. }) => "accepted".to_owned(),
                Some(agent::intents::Outcome::Completed) => "completed".to_owned(),
                None => latest
                    .stage
                    .and_then(|stage| serde_json::to_value(stage).ok())
                    .and_then(|stage| stage.as_str().map(str::to_owned))
                    .unwrap_or_default(),
            };
            let value = format!("{} · {how}", self.agent_action_name(&latest.action));
            col = col.child(form_row(
                theme,
                self.agent_text("settings.agent.lastAction"),
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(value),
            ));
        }
        col
    }

    /// The agent's request waiting for the person, if one is.
    pub(super) fn agent_pending(&self, cx: &App) -> Option<agent::intents::PendingApproval> {
        agent::book(cx).and_then(|book| book.approvals.first().cloned())
    }

    /// The agent's capsule, floating at the top of the window as the web's
    /// does (spec 103): whether it is connected — a dot in the link's tone —
    /// what it did last, and Stop, which refuses everything of its still
    /// waiting and switches it off. Shown while the agent is set up, or
    /// waits on the person.
    pub(super) fn agent_bar(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let (ready, link) = cx
            .try_global::<Agent>()
            .map(|agent| (agent.config.ready(), agent.link.clone()))
            .unwrap_or((false, Link::Off));
        let pending = self.agent_pending(cx).is_some();
        if !ready && !pending {
            return None;
        }
        let latest = agent::book(cx).and_then(|book| book.activity.first().cloned());
        let busy = latest.as_ref().is_some_and(|entry| entry.stage.is_some());
        let (dot, words) = match link {
            Link::Online if busy => (theme.info_base, None),
            Link::Online => (theme.success_base, None),
            Link::Closed(_) => (theme.error_base, Some("settings.agent.statusRefused")),
            Link::Offline => (theme.warning_base, Some("settings.agent.statusOffline")),
            Link::Connecting | Link::Off => {
                (theme.warning_base, Some("settings.agent.statusConnecting"))
            }
        };
        // Online, what it did last; otherwise how the link stands.
        let what = match (words, &latest) {
            (Some(key), _) => self.agent_text(key),
            (None, Some(entry)) => self.agent_action_name(&entry.action),
            (None, None) => self.agent_text("settings.agent.statusOnline"),
        };
        let capsule = div()
            .id("agent-bar")
            .flex()
            .items_center()
            .gap(px(12.))
            .max_w(px(520.))
            .py(px(6.))
            .pl(px(16.))
            .pr(px(6.))
            .rounded_full()
            .border_1()
            .border_color(theme.divider)
            .bg(theme.bg_raised)
            .shadow(crate::ui::dialog::shadow_lg())
            .text_size(theme::text_row_sub())
            .child(div().size(px(8.)).rounded_full().flex_none().bg(dot))
            .child(
                div()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .flex_none()
                    .whitespace_nowrap()
                    .child(self.agent_text("settings.agent.title")),
            )
            .child(
                div()
                    .min_w(px(0.))
                    .overflow_hidden()
                    .text_ellipsis()
                    .whitespace_nowrap()
                    .text_color(theme.fg_muted)
                    .child(what),
            )
            .child(
                div()
                    .id("agent-stop")
                    .flex_none()
                    .h(px(28.))
                    .px(px(14.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .cursor_pointer()
                    .bg(theme.error_soft)
                    .text_color(theme.error_base)
                    .child(self.agent_text("settings.agent.stop"))
                    .on_click(cx.listener(|_, _, _, cx| agent::stop(cx))),
            );
        // Centred over the window, clear of the title bar's buttons; only the
        // capsule takes clicks.
        Some(
            div()
                .absolute()
                .top(px(10.))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(capsule),
        )
    }

    /// What the request names, as the person knows it: a contact's or a
    /// wallet's name, the store, the preference.
    fn agent_request_detail(&self, action: &Action, cx: &mut Context<Self>) -> Option<String> {
        let same = |a: &str, b: &str| a.eq_ignore_ascii_case(b);
        match action {
            Action::DeleteContact { address } => {
                let view = resident::resident::<Contacts>(cx).read(cx).view();
                let known = view.contacts.iter().find(|c| same(&c.address, address));
                Some(
                    known
                        .and_then(|c| c.name.clone())
                        .unwrap_or_else(|| address.clone()),
                )
            }
            Action::DeleteGroup { id } => {
                let view = resident::resident::<Contacts>(cx).read(cx).view();
                Some(
                    view.groups
                        .iter()
                        .find(|g| &g.id == id)
                        .map_or_else(|| id.clone(), |g| g.name.clone()),
                )
            }
            Action::RemoveWallet { address } | Action::SwitchAccount { address } => {
                let view = session::view(cx);
                let row = view
                    .accounts
                    .iter()
                    .find(|r| same(&r.account.address, address));
                Some(row.map_or_else(
                    || address.clone(),
                    |r| format!("{} · {address}", r.account.name),
                ))
            }
            Action::ClearStorageItem { item } => Some(item.clone()),
            Action::SetPreference { preference } => {
                serde_json::to_value(preference).ok().map(|value| {
                    format!(
                        "{}: {}",
                        value["kind"].as_str().unwrap_or_default(),
                        value["value"]
                    )
                })
            }
            Action::SetBalancePrivacy { hidden } => Some(
                self.agent_text(if *hidden {
                    "home.a11yHideBalance"
                } else {
                    "home.a11yShowBalance"
                })
                .to_string(),
            ),
            // The core's: the network, URL or record it changes.
            _ => vela_core::app::intent_rules::approval_detail(action),
        }
    }

    /// The agent's manage-level request, asked here in the wallet — not in
    /// the agent's chat. Dismissing it is a no.
    pub(super) fn agent_approval_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let pending = self.agent_pending(cx)?;
        let name = self.agent_action_name(pending.action.kind());
        let what = self.agent_request_detail(&pending.action, cx);
        let body = match what {
            Some(what) => format!(
                "{name} — {what}. {}",
                self.agent_text("settings.agent.approveHint")
            ),
            None => format!("{name}. {}", self.agent_text("settings.agent.approveHint")),
        };
        let copy = ConfirmCopy {
            title: SharedString::default(),
            body: SharedString::from(body),
            callout: None,
            note: None,
            // The safe answer is the easy one: Deny is the sheet's answer,
            // on top and in the accent; Allow is the quiet one under it.
            confirm: self.agent_text("settings.agent.deny"),
            cancel: Some(self.agent_text("settings.agent.allow")),
            danger: false,
        };
        let id = pending.id;
        // Allow counts only once the request has been up a moment: the next
        // request takes this dialog's place, and a click already on its way
        // to Allow does not land on it.
        let since = pending.since;
        let sheet = confirm_sheet(
            theme,
            copy,
            cx.listener(move |_, _: &gpui::ClickEvent, _, cx| agent::answer(id, false, cx)),
            cx.listener(move |_, _: &gpui::ClickEvent, _, cx| {
                if vela_agent::transfers::wall_ms() - since >= APPROVAL_ARM_MS {
                    agent::answer(id, true, cx);
                }
            }),
        );
        Some(
            crate::ui::dialog::dialog(
                "agent-approval",
                theme,
                window,
                self.agent_text("settings.agent.approveTitle"),
                None,
                self.dialog_close_icon(theme),
                sheet,
                &self.dialog_scroll("agent-approval"),
                Self::closer(cx, move |_, cx| agent::answer(id, false, cx)),
            )
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_action_an_agent_may_ask_for_has_a_name() {
        let loc = crate::loc::Loc::for_tag("en");
        for entry in vela_core::app::intents::catalog() {
            if entry.person_only {
                continue;
            }
            let key = action_key(&entry.action);
            assert_ne!(loc.t(&key).as_ref(), key, "{} has no words", entry.action);
        }
        assert_eq!(
            action_key("prepare_batch_transfer"),
            "settings.agent.action.prepareBatchTransfer"
        );
    }

    #[test]
    fn every_core_settings_page_but_community_has_a_panel() {
        for name in vela_core::app::intents::SETTINGS_PAGES {
            assert_eq!(settings_page(name).is_some(), name != "community", "{name}");
        }
    }
}
