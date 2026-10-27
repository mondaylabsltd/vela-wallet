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
            // Spec 107: Explore, at its tab in front.
            Screen::Explore => self.section = Section::Explore,
        }
        cx.notify();
        None
    }

    // -- Explore (spec 107) --------------------------------------------------

    /// Explore as the agent reads it: the browser machines' own views
    /// through the core's `explore_facts`.
    pub(crate) fn agent_explore_facts(
        &self,
        explore: &vela_core::app::explore_sites::ExploreView,
        history: &vela_core::app::browser_history::BhistView,
        cx: &App,
    ) -> vela_core::app::intent_rules::ExploreFacts {
        let dapp = self
            .browser_host
            .as_ref()
            .map(|host| host.read(cx).view.clone())
            .unwrap_or_default();
        // The request whose sheet the person sees: a connect's is Explore's,
        // over its tab; a signature's is the signing sheet; a network's is
        // the add-network sheet, up while the request is.
        let sheet = if let Some(consent) = &dapp.consent {
            (self.section == Section::Explore
                && explore.selected_tab.as_deref() == Some(consent.tab.as_str()))
            .then(|| format!("connect:{}", consent.tab))
        } else if let Some(signing) = &dapp.signing {
            self.signing_host
                .is_some()
                .then(|| format!("sign:{}:{}", signing.tab, signing.id))
        } else {
            dapp.adding_network
                .as_ref()
                .map(|adding| format!("add_network:{}:{}", adding.tab, adding.id))
        };
        vela_core::app::intent_rules::explore_facts(
            explore,
            &dapp,
            history,
            self.nav_enabled[0],
            self.nav_enabled[1],
            sheet.as_deref(),
        )
    }

    /// Do one of Explore's actions as its own controls do; a refusal, in
    /// words, when it cannot be done here.
    pub(crate) fn agent_explore(
        &mut self,
        action: &Action,
        cx: &mut Context<Self>,
    ) -> Option<String> {
        use crate::wallet::browser_host::Go;
        use vela_core::app::intents::BrowseStep;
        if self.identity.is_none() {
            return Some("the wallet is not open".to_owned());
        }
        let explore = |cx: &mut Context<Self>, event: vela_core::app::explore_sites::Event| {
            resident::resident::<ExploreSites>(cx)
                .update(cx, |resident, cx| resident.dispatch(event, cx));
        };
        let history = |cx: &mut Context<Self>, event: vela_core::app::browser_history::Event| {
            resident::resident::<BrowserHistory>(cx)
                .update(cx, |resident, cx| resident.dispatch(event, cx));
        };
        let origin = |text: &str| text.trim().trim_end_matches('/').to_owned();
        match action {
            Action::OpenSite { url, new_tab } => {
                self.section = Section::Explore;
                if *new_tab {
                    self.open_in_new_tab(url.trim().to_owned(), None, cx);
                } else {
                    self.browser_go(Go::Typed(url.trim().to_owned()), cx);
                }
            }
            Action::Browse { to } => self.browser_go(
                match to {
                    BrowseStep::Back => Go::Back,
                    BrowseStep::Forward => Go::Forward,
                    BrowseStep::Reload => Go::Reload,
                },
                cx,
            ),
            Action::CloseTab { id } => self.close_browser_tab(id, cx),
            Action::CloseAllTabs => {
                self.close_browser_tabs(&vela_core::app::explore_sites::TabCloseScope::All, cx);
            }
            Action::SetSiteFavorite { url, favorite } => {
                if *favorite {
                    explore(
                        cx,
                        vela_core::app::explore_sites::Event::FavoriteAdded {
                            url: url.trim().to_owned(),
                            title: None,
                            now_ms: crate::executor::now_ms(),
                        },
                    );
                } else if let Some(site) = resident::resident::<ExploreSites>(cx)
                    .read(cx)
                    .view()
                    .favorites
                    .into_iter()
                    .find(|site| {
                        vela_core::app::dapp_permissions::origin_of(&site.url)
                            == vela_core::app::dapp_permissions::origin_of(url.trim())
                    })
                {
                    explore(
                        cx,
                        vela_core::app::explore_sites::Event::FavoriteRemoved {
                            origin: site.origin,
                        },
                    );
                }
            }
            Action::ClearRecentSites => {
                history(cx, vela_core::app::browser_history::Event::ClearAll)
            }
            Action::ForgetSite { origin: site } => history(
                cx,
                vela_core::app::browser_history::Event::DeleteOrigin {
                    origin: origin(site),
                },
            ),
            Action::DisconnectSite { origin: site } => {
                let host = self.browser_host(cx);
                let site = origin(site);
                host.update(cx, |host, cx| {
                    host.dispatch(DbrEvent::RevokeRequested { origin: site }, cx)
                });
            }
            Action::SetSiteNetwork {
                origin: site,
                chain_id,
            } => {
                let host = self.browser_host(cx);
                let site = origin(site);
                let chain_id = *chain_id;
                host.update(cx, |host, cx| {
                    host.dispatch(
                        DbrEvent::SiteChainPicked {
                            origin: site,
                            chain_id,
                        },
                        cx,
                    );
                });
            }
            Action::RejectDappRequest { id } => {
                let target = vela_core::app::intent_rules::dapp_request_target(id)?;
                match target.kind.as_str() {
                    "connect" => {
                        let host = self.browser_host(cx);
                        host.update(cx, |host, cx| {
                            host.dispatch(
                                DbrEvent::ConsentRejected {
                                    now_ms: crate::executor::now_ms(),
                                },
                                cx,
                            );
                        });
                    }
                    "sign" => {
                        let Some(signing) = self.signing_host.clone() else {
                            return Some("the signing request is not on its sheet yet".to_owned());
                        };
                        signing.update(cx, |host, cx| {
                            host.dispatch_sign(
                                vela_core::app::sign_request::Event::RejectTapped,
                                cx,
                            );
                        });
                    }
                    "add_network" => {
                        let admin = self.network_admin(cx);
                        admin.update(cx, |admin, cx| {
                            admin.dispatch(
                                vela_core::app::network_admin::Event::DappAddDeclined,
                                cx,
                            );
                        });
                    }
                    _ => return Some("no dApp request with that id is waiting".to_owned()),
                }
            }
            Action::AskPersonDappRequest { id } => {
                let target = vela_core::app::intent_rules::dapp_request_target(id)?;
                // A signature's and a network's sheets are the app's own, in
                // front wherever the person is; a connect's is over its tab.
                if target.kind == "connect" {
                    self.section = Section::Explore;
                    explore(
                        cx,
                        vela_core::app::explore_sites::Event::TabSelected { id: target.tab },
                    );
                }
            }
            _ => return Some("not one of Explore's actions".to_owned()),
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
                wallets: None,
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

        // Spec 106: the usual way in is clientmcp.dev; the fields fold under
        // "Use your own bridge".
        let linked = config.linked.clone();
        col = col.child(self.agent_link_section(linked.as_ref(), theme, cx));
        // Spec 107 E8: the wallets it may use — none unless switched on here.
        col = col.child(self.agent_wallets_section(&config.wallets, theme, cx));
        let own_open = linked.is_none()
            && self
                .agent_own_open
                .unwrap_or(!config.token.trim().is_empty());

        let on = config.enabled;
        let mut toggle = Some(toggle_row(
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
                        wallets: None,
                    },
                    cx,
                );
            },
        ));
        if linked.is_some() {
            col = col.children(toggle.take());
        } else {
            col = col.child(
                div()
                    .id("agent-own-bridge")
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .cursor_pointer()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(if own_open { "▾" } else { "▸" })
                    .child(self.agent_text("settings.agent.ownBridge"))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.agent_own_open = Some(!own_open);
                        cx.notify();
                    })),
            );
        }
        if own_open {
            col = col.children(toggle.take());

            let url = self.agent_url_draft.clone().unwrap_or(config.url);
            let invalid =
                !url.trim().is_empty() && agent::settings::bridge_url_problem(&url).is_some();
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

    // -- spec 106: connecting with clientmcp.dev -------------------------------

    /// Connected through clientmcp.dev: whose it is, where the person connects
    /// their agent, Reconnect and Disconnect. Otherwise the one button.
    fn agent_link_section(
        &mut self,
        linked: Option<&agent::settings::AgentLinked>,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        use crate::ui::{ButtonState, ButtonVariant, vela_button, vela_button_state};
        let mut col = div().flex().flex_col().gap(px(12.));
        if let Some(linked) = linked {
            let who = self
                .agent_text("settings.agent.connectedAs")
                .replace("{{login}}", &linked.login);
            let account = vela_core::clientmcp_link::account_url(&linked.site);
            col = col
                .child(
                    div()
                        .id("agent-linked")
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(who),
                )
                .child(
                    div()
                        .id("agent-client-id")
                        .font_family(crate::theme::font_mono())
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(
                            self.agent_text("settings.agent.clientId")
                                .replace("{{id}}", &linked.client_id),
                        ),
                )
                .children(self.agent_address(theme, cx))
                .child(
                    div()
                        .id("agent-connect-agent")
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .p(px(12.))
                        .rounded(px(theme::RADIUS_CTA))
                        .bg(theme.bg_raised)
                        .cursor_pointer()
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.fg_base)
                                .child(format!(
                                    "{} →",
                                    self.agent_text("settings.agent.connectAgent")
                                )),
                        )
                        .child(
                            div()
                                .text_size(theme::text_row_sub())
                                .text_color(theme.fg_muted)
                                .child(self.agent_text("settings.agent.connectAgentHint")),
                        )
                        .on_click(move |_, _, cx| cx.open_url(&account)),
                )
                .child(
                    div()
                        .flex()
                        .gap(px(12.))
                        .child(div().w(px(180.)).child(vela_button(
                            "agent-reconnect",
                            ButtonVariant::Secondary,
                            self.agent_text("settings.agent.reconnect"),
                            theme,
                            cx.listener(|this, _, _, cx| this.agent_connect(cx)),
                        )))
                        .child(div().w(px(180.)).child(vela_button(
                            "agent-disconnect",
                            ButtonVariant::Secondary,
                            self.agent_text("settings.agent.disconnect"),
                            theme,
                            |_, _, cx| {
                                // Forgotten here; the client stays on
                                // clientmcp.dev until deleted there.
                                agent::invoke_person(
                                    Action::ConfigureAgent {
                                        enabled: Some(false),
                                        bridge_url: Some(String::new()),
                                        client_token: Some(String::new()),
                                        wallets: None,
                                    },
                                    cx,
                                );
                            },
                        ))),
                );
            if self.agent_link_waiting.is_some() {
                col = col.child(self.agent_link_waiting_row(theme, cx));
            }
        } else {
            let waiting = self.agent_link_waiting.is_some();
            col = col.child(div().w(px(280.)).child(vela_button_state(
                "agent-connect",
                ButtonVariant::Secondary,
                self.agent_text("settings.agent.connect"),
                if waiting {
                    ButtonState::Busy
                } else {
                    ButtonState::Enabled
                },
                theme,
                cx.listener(|this, _, _, cx| this.agent_connect(cx)),
            )));
            col = col.child(if waiting {
                self.agent_link_waiting_row(theme, cx)
            } else {
                div()
                    .text_size(theme::text_row_sub())
                    .line_height(px(20.))
                    .text_color(theme.fg_muted)
                    .child(crate::ui::prose(
                        self.agent_text("settings.agent.connectHint"),
                    ))
            });
        }
        if let Some(problem) = &self.agent_link_problem {
            col = col.child(
                div()
                    .id("agent-link-problem")
                    .text_size(theme::text_row_sub())
                    .line_height(px(20.))
                    .text_color(theme.error_base)
                    .child(self.agent_link_problem_text(problem)),
            );
        }
        col
    }

    /// The MCP server address for the person's AI agent, to copy from here:
    /// masked until Show — it is a credential.
    fn agent_address(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<gpui::Stateful<Div>> {
        let address = cx
            .try_global::<Agent>()
            .map(|agent| agent.config.agent_url.clone())
            .filter(|url| !url.is_empty())?;
        let shown = if self.agent_address_shown {
            address.clone()
        } else {
            match address.rfind('/') {
                Some(at) => format!("{}••••••••••••", &address[..=at]),
                None => "••••••••••••".to_owned(),
            }
        };
        let action = |id: &'static str, label: SharedString| {
            div()
                .id(id)
                .cursor_pointer()
                .px(px(12.))
                .py(px(4.))
                .rounded_full()
                .border_1()
                .border_color(theme.divider)
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .child(label)
        };
        Some(
            div()
                .id("agent-address")
                .flex()
                .flex_col()
                .gap(px(8.))
                .p(px(12.))
                .rounded(px(theme::RADIUS_CTA))
                .bg(theme.bg_raised)
                .child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(self.agent_text("settings.agent.agentAddress")),
                )
                .child(
                    div()
                        .font_family(crate::theme::font_mono())
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(shown),
                )
                .child(
                    div()
                        .flex()
                        .justify_end()
                        .gap(px(8.))
                        .child(
                            action(
                                "agent-address-show",
                                self.agent_text(if self.agent_address_shown {
                                    "settings.agent.hide"
                                } else {
                                    "settings.agent.show"
                                }),
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.agent_address_shown = !this.agent_address_shown;
                                cx.notify();
                            })),
                        )
                        .child(
                            action(
                                "agent-address-copy",
                                self.agent_text(if self.agent_address_copied {
                                    "settings.agent.copied"
                                } else {
                                    "settings.agent.copy"
                                }),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        address.clone(),
                                    ));
                                    this.agent_address_copied = true;
                                    cx.notify();
                                    cx.spawn(async move |page, cx| {
                                        cx.background_executor()
                                            .timer(std::time::Duration::from_millis(1800))
                                            .await;
                                        page.update(cx, |this, cx| {
                                            this.agent_address_copied = false;
                                            cx.notify();
                                        })
                                        .ok();
                                    })
                                    .detach();
                                },
                            )),
                        ),
                )
                .child(
                    div()
                        .text_size(theme::text_label())
                        .line_height(px(16.))
                        .text_color(theme.fg_subtle)
                        .child(crate::ui::prose(
                            self.agent_text("settings.agent.agentAddressHint"),
                        )),
                ),
        )
    }

    /// Spec 107 E8: a switch per wallet on this device; the agent sees and
    /// acts only in the ones on. Each change is the person's own
    /// `configure_agent`, the list whole.
    fn agent_wallets_section(
        &self,
        allowed: &[String],
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let accounts = session::view(cx).accounts;
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .child(self.agent_text("settings.agent.wallets")),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(crate::ui::prose(
                        self.agent_text("settings.agent.walletsHint"),
                    )),
            );
        if allowed.is_empty() {
            col = col.child(
                div()
                    .id("agent-wallets-none")
                    .text_size(theme::text_row_sub())
                    .text_color(theme.warning_base)
                    .child(self.agent_text("settings.agent.walletsNone")),
            );
        }
        for row in accounts {
            let address = row.account.address.clone();
            let on = allowed.iter().any(|w| w.eq_ignore_ascii_case(&address));
            let list = allowed.to_vec();
            let short = if address.len() > 12 {
                format!("{}…{}", &address[..6], &address[address.len() - 4..])
            } else {
                address.clone()
            };
            col = col.child(toggle_row(
                SharedString::from(format!("agent-wallet-{address}")),
                theme,
                SharedString::from(row.account.name.clone()),
                SharedString::from(short),
                on,
                move |_, _, cx| {
                    let mut next: Vec<String> = list
                        .iter()
                        .filter(|w| !w.eq_ignore_ascii_case(&address))
                        .cloned()
                        .collect();
                    if !on {
                        next.push(address.clone());
                    }
                    agent::invoke_person(
                        Action::ConfigureAgent {
                            enabled: None,
                            bridge_url: None,
                            client_token: None,
                            wallets: Some(next),
                        },
                        cx,
                    );
                },
            ));
        }
        col
    }

    /// "Waiting for clientmcp.dev…", and Cancel.
    fn agent_link_waiting_row(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap(px(12.))
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_muted)
            .child(self.agent_text("settings.agent.connecting"))
            .child(
                div()
                    .id("agent-link-cancel")
                    .cursor_pointer()
                    .underline()
                    .child(self.agent_text("common.cancel"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(cancel) = this.agent_link_waiting.take() {
                            cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        cx.notify();
                    })),
            )
    }

    fn agent_link_problem_text(&self, problem: &agent::link::Problem) -> String {
        use vela_core::clientmcp_link::LinkError;
        let key = match problem {
            agent::link::Problem::Link(LinkError::Refused(reason)) => {
                return self
                    .agent_text("settings.agent.linkRefused")
                    .replace("{{reason}}", reason);
            }
            agent::link::Problem::Link(LinkError::Unavailable) => "settings.agent.linkUnavailable",
            agent::link::Problem::Link(LinkError::Expired) => "settings.agent.linkExpired",
            agent::link::Problem::Link(LinkError::NotOurs) => "settings.agent.linkNotOurs",
            agent::link::Problem::Network => "settings.agent.linkNetwork",
            agent::link::Problem::Link(LinkError::Malformed(_))
            | agent::link::Problem::Cancelled => "settings.agent.linkFailed",
        };
        self.agent_text(key).to_string()
    }

    /// Connect (or reconnect) with clientmcp.dev: listen on a loopback port,
    /// open the browser at the site, and wait for it on a background thread.
    pub(super) fn agent_connect(&mut self, cx: &mut Context<Self>) {
        if self.agent_link_waiting.is_some() {
            return;
        }
        self.agent_link_problem = None;
        let started = match agent::link::start() {
            Ok(started) => started,
            Err(problem) => {
                self.agent_link_problem = Some(problem);
                cx.notify();
                return;
            }
        };
        cx.open_url(&started.pending.authorize_url);
        let cancel = started.cancel.clone();
        self.agent_link_waiting = Some(cancel.clone());
        let site = started.pending.site.clone();
        let done = self.agent_text("settings.agent.linkReturn").to_string();
        cx.spawn(async move |page, cx| {
            let result = cx
                .background_executor()
                .spawn(async move { agent::link::finish(started, &done) })
                .await;
            page.update(cx, |this, cx| {
                // A sign-in cancelled (or replaced) meanwhile is not this one.
                if this
                    .agent_link_waiting
                    .as_ref()
                    .is_some_and(|waiting| std::sync::Arc::ptr_eq(waiting, &cancel))
                {
                    this.agent_link_waiting = None;
                    this.agent_linked(result, site, cx);
                }
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// The sign-in's end: what clientmcp.dev issued, applied as the person's
    /// own `configure_agent`, then whose it is.
    fn agent_linked(
        &mut self,
        result: Result<vela_core::clientmcp_link::Linked, agent::link::Problem>,
        site: String,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(linked) => {
                self.agent_link_problem = None;
                let agent_url = linked.agent_url;
                let whose = agent::settings::AgentLinked {
                    site,
                    login: linked.login,
                    client_id: linked.client_id,
                };
                // E8: the first connection offers the wallet in front; a list
                // the person already chose is kept.
                let none_yet = cx
                    .try_global::<Agent>()
                    .is_none_or(|agent| agent.config.wallets.is_empty());
                let active = session::view(cx).address;
                let wallets = (none_yet && !active.is_empty()).then(|| vec![active]);
                agent::invoke_person_then(
                    Action::ConfigureAgent {
                        enabled: Some(true),
                        bridge_url: Some(linked.bridge_url),
                        client_token: Some(linked.client_token),
                        wallets,
                    },
                    cx,
                    move |_, cx| agent::set_linked(Some(whose), agent_url, cx),
                );
            }
            Err(agent::link::Problem::Cancelled) => {}
            Err(problem) => self.agent_link_problem = Some(problem),
        }
        cx.notify();
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

    /// What an approval's detail reads around the action: the wallets, the
    /// networks, Explore — from the page's own residents (the page is the
    /// one drawing, so it is not read back through the agent's port).
    fn agent_detail_world(&self, cx: &mut Context<Self>) -> vela_core::app::intent_rules::World {
        let explore = resident::resident::<ExploreSites>(cx).read(cx).view();
        let history = resident::resident::<BrowserHistory>(cx).read(cx).view();
        let facts = self.agent_explore_facts(&explore, &history, cx);
        let networks = resident::resident::<NetworkAdmin>(cx).read(cx).view();
        serde_json::from_value(serde_json::json!({
            "session": session::view(cx),
            "networks": networks,
            "explore": facts,
        }))
        .unwrap_or_default()
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
            // The wallet it leaves and the one it goes to (the core's).
            Action::SwitchAccount { .. }
            | Action::SetSiteNetwork { .. }
            | Action::DisconnectSite { .. } => vela_core::app::intent_rules::approval_detail_in(
                action,
                &self.agent_detail_world(cx),
            ),
            Action::RemoveWallet { address } => {
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
            // The space it switches to, in the header label's words.
            Action::SetNetworkSpace { space } => Some(
                self.agent_text(match space {
                    vela_core::app::network_space::NetworkSpace::Mainnet => "networkSpace.mainnet",
                    vela_core::app::network_space::NetworkSpace::Testnet => "networkSpace.testnet",
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
