//! Multisig accounts on the wallet page (AUDIT M1–M6): the switcher's
//! "Multisig accounts", the home's waiting group, a transaction's sheet in
//! the third column (its one review, its approvals, who signs how, its
//! history and its actions), the send flow's review-first draft, the
//! high-risk question and an outside owner's session.
//!
//! Everything drawn here is the core's: the machines' views
//! (`crate::multisig::host`) said in the corpus' words
//! (`home.multisig.*`). Nothing is decided on screen — every press is an
//! event to a machine. The Bybit rule: every signer sees the core's decoded
//! review, the state-diff guard and a fresh chain read before a signature,
//! and an Approve is not offered until that review is here.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;
use vela_core::app::multisig_accounts as ms_accounts;
use vela_core::app::multisig_presence::{ReadyBlock, Standing, TimelineStep, WaitingNote};
use vela_core::app::multisig_proposals::{self as ms_proposals, ActionState, ProposalItem};
use vela_core::app::multisig_shell::MultisigAccount;
use vela_core::app::sim_outcome::SimVerdict;
use vela_core::multisig::SafeTxHashes;
use vela_core::multisig_reading::MemberChanges;
use vela_core::multisig_review::{Concern, Field, Recipient, RecipientRole, SignRule, StateGuard};

use gpui::App;

use super::*;
use crate::multisig::host::{self as ms_host, MultisigHost};
use crate::multisig::machines::Screen;
use crate::multisig::words;

/// A multisig's own part of Settings → Account (AUDIT M1, M3). A child of
/// this module (its file beside this one) so it draws with the sheet's
/// helpers without widening them.
#[path = "multisig_settings.rs"]
mod settings;
/// A site's request to a multisig (AUDIT M5).
#[path = "multisig_dapp.rs"]
mod dapp;

/// The multisig surfaces' state on this page — which sheet is open, and
/// what the person picked on it. What a machine holds is never copied here.
#[derive(Default)]
pub(crate) struct MultisigUi {
    /// The transaction whose sheet the third column shows.
    pub sheet: Option<(u64, String)>,
    /// The member an approval is signed as, when several are here.
    pub approve_as: Option<String>,
    /// The member that executes, when several are here.
    pub executor: Option<String>,
    /// The technical details are open.
    pub tech: bool,
    /// "Copied" under the link, until the next sheet.
    pub copied: bool,
    /// The outside owner's session sheet is up.
    pub session: bool,
    /// The person asked to reject: the question is up.
    pub rejecting: bool,
    /// The host's notifications reach this page.
    pub observing: Option<gpui::Subscription>,
    pub scroll: crate::ui::SmoothScroll,
    /// The last time each open sheet asked for its preview (ms).
    pub previewed: BTreeMap<String, f64>,
    /// The words, for the language in force ([`WalletPage::ms_loc`]).
    pub loc: Option<std::rc::Rc<Loc>>,
    /// A security key's PIN being typed for a multisig signature: what is
    /// typed, and the field's focus.
    pub pin: Option<(String, gpui::FocusHandle)>,
    /// The multisig whose removal is being asked about (the switcher row).
    pub removing: Option<String>,
    /// The create sheet is up (`multisig_create.rs`).
    pub create: bool,
    /// The add sheet is up (`multisig_add.rs`), filled with this text.
    pub add: Option<String>,
    /// The invite sheet for this multisig is up.
    pub invite: Option<String>,
    /// Settings → Account's multisig block (`multisig_settings.rs`).
    pub settings: settings::SettingsUi,
    /// The sites' requests the proposals machine holds (AUDIT M5).
    pub dapp: crate::multisig::dapp::Holds,
}

/// The one review, as drawn (`multisig_review::MultisigReview`'s JSON):
/// read, never decided again.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct ReviewDraw {
    pub review: Option<Value>,
    pub member_changes: Option<MemberChanges>,
    pub hashes: Option<SafeTxHashes>,
    pub fields: Vec<Field>,
    pub recipients: Vec<Recipient>,
    pub state: Option<StateGuard>,
    pub simulation: Option<SimVerdict>,
    pub simulation_source: String,
    pub concerns: Vec<Concern>,
    pub sign: SignRule,
}

impl ReviewDraw {
    fn of(value: &Value) -> Option<Self> {
        serde_json::from_value(value.clone()).ok()
    }

    /// The transaction's own refusal (Vela does not sign it), if any.
    fn refusal(&self) -> Option<String> {
        self.review
            .as_ref()
            .and_then(|r| r.get("refusal"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    }
}

/// `<chainId>:<safeTxHash>` — the views' key for one transaction.
pub(crate) fn item_key(chain_id: u64, hash: &str) -> String {
    format!("{chain_id}:{hash}")
}

fn network(chain_id: u64) -> String {
    crate::executor::custom_tokens::network_name(u32::try_from(chain_id).unwrap_or_default())
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

impl WalletPage {
    /// The words, shared across the sheet's own `&mut self` calls (one
    /// engine, rebuilt when the language moves).
    pub(crate) fn ms_loc(&mut self) -> std::rc::Rc<Loc> {
        let language = self.loc.language().to_owned();
        match &self.multisig.loc {
            Some(loc) if loc.language() == language => loc.clone(),
            _ => {
                let loc = std::rc::Rc::new(Loc::for_language(&language));
                self.multisig.loc = Some(loc.clone());
                loc
            }
        }
    }

    /// The host, booted on first use.
    pub(crate) fn ms_host(&self, cx: &mut Context<Self>) -> gpui::Entity<MultisigHost> {
        ms_host::host(cx)
    }

    /// Follow the host: what its machines ask the screen to do (a link's
    /// transaction, an outside owner's session), and a redraw when it moves.
    pub(crate) fn multisig_observe(&mut self, cx: &mut Context<Self>) {
        if self.multisig.observing.is_some() {
            return;
        }
        let host = self.ms_host(cx);
        self.multisig.observing = Some(cx.observe(&host, |this, host, cx| {
            let screen = host.update(cx, |host, _| host.take_screen());
            for item in screen {
                match item {
                    Screen::OpenProposal {
                        chain_id,
                        safe_tx_hash,
                        ..
                    } => this.open_multisig_sheet(chain_id, safe_tx_hash, cx),
                    Screen::OutsideSession => {
                        this.multisig.session = true;
                    }
                    Screen::Activated(_) | Screen::ActivityRows(_) => {}
                }
            }
            this.ms_dapp_follow(cx);
            cx.notify();
        }));
    }

    /// `VELA_MULTISIG_SCRIPT`'s page steps (developer builds): the presses
    /// a script makes on this page — a transaction's sheet, opened or closed.
    #[cfg(feature = "dev-fixtures")]
    pub(crate) fn ms_script_asks(&mut self, cx: &mut Context<Self>) {
        let Some(ask) = cx
            .try_global::<crate::multisig::dev_script::PageAsk>()
            .and_then(|a| a.0.clone())
        else {
            return;
        };
        cx.set_global(crate::multisig::dev_script::PageAsk(None));
        match ask.get("page").and_then(Value::as_str) {
            Some("open_sheet") => {
                let chain_id = ask.get("chain_id").and_then(Value::as_u64).unwrap_or(100);
                let hash = ask.get("safe_tx_hash").and_then(Value::as_str).unwrap_or_default().to_owned();
                self.open_multisig_sheet(chain_id, hash, cx);
            }
            Some("close_sheet") => self.close_multisig_sheet(cx),
            Some("approve_as") => {
                self.multisig.approve_as = ask.get("member").and_then(Value::as_str).map(str::to_owned);
            }
            Some("executor") => {
                self.multisig.executor = ask.get("member").and_then(Value::as_str).map(str::to_owned);
            }
            Some("invite") => {
                self.multisig.invite = ask.get("address").and_then(Value::as_str).map(str::to_owned);
                self.multisig.scroll = crate::ui::SmoothScroll::new();
            }
            Some("close_invite") => self.multisig.invite = None,
            Some("session") => self.multisig.session = true,
            Some("switcher") => self.account_switcher = true,
            Some("close_switcher") => self.account_switcher = false,
            Some("tech") => self.multisig.tech = true,
            _ => {}
        }
        cx.notify();
    }

    /// The multisig shown, as its row (`None`: the Vela account is shown).
    pub(crate) fn shown_multisig(&self, cx: &App) -> Option<ms_accounts::AccountRow> {
        let host = ms_host::booted(cx)?;
        let view = &host.read(cx).machines().accounts_view;
        let active = view.active.as_deref()?;
        view.accounts
            .iter()
            .find(|row| same(&row.account.address, active))
            .cloned()
    }

    /// How a member is named here: this device's account, the name the
    /// multisig keeps, else its short address (the core's `names.label`).
    fn ms_member_name(&self, address: &str, account: Option<&MultisigAccount>, cx: &App) -> String {
        let own = session::view(cx)
            .accounts
            .iter()
            .find(|row| same(&row.account.address, address))
            .map(|row| row.account.name.clone());
        let kept = account
            .and_then(|a| a.members.as_ref())
            .and_then(|m| {
                m.iter()
                    .find(|(k, _)| same(k, address))
                    .and_then(|(_, info)| info.name.clone())
            });
        // The core's one naming rule (`names.label`), through the link door.
        let args = serde_json::json!({"address": address, "own": own, "kept": kept});
        vela_core::multisig_link::call_json("names.label", &args.to_string())
            .ok()
            .and_then(|text| serde_json::from_str::<String>(&text).ok())
            .unwrap_or_else(|| words::short(address))
    }

    // -- opening and closing the sheet -------------------------------------

    /// Open a transaction's sheet in the third column: its review gathered,
    /// how its remaining approvals can come read, the faster pace on.
    pub(crate) fn open_multisig_sheet(&mut self, chain_id: u64, hash: String, cx: &mut Context<Self>) {
        self.multisig.sheet = Some((chain_id, hash.clone()));
        self.multisig.approve_as = None;
        self.multisig.executor = None;
        self.multisig.tech = false;
        self.multisig.copied = false;
        self.multisig.rejecting = false;
        self.multisig.scroll = crate::ui::SmoothScroll::new();
        self.account_switcher = false;
        self.panel = PanelId::Multisig;
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| {
            host.proposals(ms_proposals::Event::Watch, cx);
            host.proposals(
                ms_proposals::Event::Review {
                    chain_id,
                    safe_tx_hash: hash.clone(),
                    blind: false,
                },
                cx,
            );
            host.proposals(
                ms_proposals::Event::ReadSigners {
                    chain_id,
                    safe_tx_hash: hash.clone(),
                },
                cx,
            );
            host.proposals(
                ms_proposals::Event::Timeline {
                    chain_id,
                    safe_tx_hash: hash,
                },
                cx,
            );
        });
        cx.notify();
    }

    /// The sheet went: the slower pace again, a link's transaction let go.
    pub(crate) fn close_multisig_sheet(&mut self, cx: &mut Context<Self>) {
        let Some((chain_id, hash)) = self.multisig.sheet.take() else {
            return;
        };
        self.ms_dapp_sheet_closed(chain_id, &hash, cx);
        if self.panel == PanelId::Multisig {
            self.panel = PanelId::None;
        }
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| {
            host.proposals(ms_proposals::Event::Unwatch, cx);
            if host.machines().proposals_view.linked.is_some() {
                host.proposals(ms_proposals::Event::CloseLinked, cx);
            }
        });
        cx.notify();
    }

    /// Escape on a multisig layer: the topmost one goes. `true` when one did.
    pub(crate) fn multisig_escape(&mut self, cx: &mut Context<Self>) -> bool {
        let host = self.ms_host(cx);
        if host.read(cx).risk.is_some() {
            host.update(cx, |host, cx| host.answer_risk(false, cx));
            return true;
        }
        if self.multisig.session {
            self.multisig.session = false;
            host.update(cx, |host, cx| host.end_session(cx));
            return true;
        }
        if host.read(cx).send_reviewing() {
            host.update(cx, |host, cx| host.withdraw_draft(cx));
            return true;
        }
        if self.ms_dapp_escape(cx) {
            return true;
        }
        if self.multisig.create {
            self.close_multisig_create(cx);
            return true;
        }
        if self.multisig.add.is_some() {
            self.multisig.add = None;
            cx.notify();
            return true;
        }
        if self.multisig.invite.is_some() {
            self.multisig.invite = None;
            cx.notify();
            return true;
        }
        if self.multisig.rejecting {
            self.multisig.rejecting = false;
            cx.notify();
            return true;
        }
        if self.multisig.sheet.is_some() {
            self.close_multisig_sheet(cx);
            return true;
        }
        false
    }

    // -- the switcher -------------------------------------------------------------

    /// The Vela account comes back in front: no multisig shown.
    pub(crate) fn ms_show_account(&mut self, cx: &mut Context<Self>) {
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| {
            host.accounts(ms_accounts::Event::Activate { address: None }, cx);
        });
    }

    /// The switcher's "Multisig accounts" (the web's `MultisigRows`): each
    /// multisig on this device — its name, its rule, whether it is shared,
    /// read-only here, what waits on it — the one shown checked; then the
    /// two ways to add one. A press activates it (the accounts machine).
    pub(crate) fn multisig_switcher_section(
        &mut self,
        theme: &Theme,
        in_dialog: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let host = self.ms_host(cx);
        let (view, presence) = {
            let machines = host.read(cx).machines();
            (
                machines.accounts_view.clone(),
                machines.proposals_view.presence.clone(),
            )
        };
        let prefix = if in_dialog { "switcher-ms" } else { "settings-ms" };
        let mut section = div().flex().flex_col().pt(px(20.)).child(
            div()
                .pb(px(4.))
                .text_size(theme::text_label())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_subtle)
                .child(words::t(&loc, "switcher.section")),
        );
        for (i, row) in view.accounts.iter().enumerate() {
            let address = row.account.address.clone();
            let active = view.active.as_deref().is_some_and(|a| same(a, &address));
            let m = row.account.threshold;
            let n = row.account.owners.len() as u64;
            let name = row
                .name
                .clone()
                .or_else(|| row.account.label.clone())
                .unwrap_or_else(|| {
                    words::tv(&loc, "switcher.rowName", &[("m", &m.to_string()), ("n", &n.to_string())])
                });
            let mut flags: Vec<String> = vec![words::rule_chip(&loc, m, n)];
            if !row.shared {
                flags.push(words::t(&loc, "switcher.notShared"));
            }
            if row.local_members.is_empty() {
                flags.push(words::t(&loc, "switcher.readOnly"));
            }
            if !row.high_risk_on.is_empty() {
                flags.push(words::t(&loc, "authority.riskFlag"));
            }
            if let Some(Some(waiting)) = presence.get(&address.to_ascii_lowercase()) {
                if waiting.needs_you > 0 {
                    flags.push(words::tc(&loc, "presence.needsYou", waiting.needs_you as usize));
                }
                if waiting.ready > 0 {
                    flags.push(words::tc(&loc, "presence.ready", waiting.ready as usize));
                }
            }
            let sub = format!(
                "{} · {}",
                crate::wallet::live::shorten_address(&address),
                flags.join(" · ")
            );
            let name_colour = if active { theme.accent } else { theme.fg_base };
            let mut pick = div()
                .id(ElementId::from((prefix, i)))
                .a11y_button(SharedString::from(format!("{name}, {sub}")))
                .flex_1()
                .min_w(px(0.))
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(12.))
                .cursor_pointer()
                .child(crate::wallet::components::identicon_avatar(
                    &mut self.identicons,
                    &address,
                    super::SWITCHER_IDENTICON,
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
                                .text_color(name_colour)
                                .truncate()
                                .child(name),
                        )
                        .child(
                            div()
                                .text_size(theme::text_label())
                                .text_color(theme.fg_subtle)
                                .truncate()
                                .child(sub),
                        ),
                );
            if active {
                pick = pick.child(icon_img(&mut self.icons, Icon::Check, false, theme.accent, 18.));
            }
            let target = address.clone();
            pick = pick.on_click(cx.listener(move |this, _, _, cx| {
                let host = this.ms_host(cx);
                let address = target.clone();
                host.update(cx, |host, cx| {
                    host.accounts(
                        ms_accounts::Event::Activate {
                            address: Some(address),
                        },
                        cx,
                    );
                });
                if in_dialog {
                    this.close_account_switcher(cx);
                }
                cx.notify();
            }));
            let doomed = address.clone();
            let remove = div()
                .id(ElementId::from((if in_dialog { "switcher-ms-remove" } else { "settings-ms-remove" }, i)))
                .a11y_button(crate::ui::a11y::close_label())
                .flex_none()
                .p(px(2.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|el| el.bg(theme.bg_sunken))
                .child(icon_img(&mut self.icons, Icon::X, false, theme.fg_subtle, 18.))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.multisig.removing = Some(doomed.clone());
                    cx.notify();
                }));
            section = section.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(theme.divider)
                    .child(pick)
                    .child(remove),
            );
            if self.multisig.removing.as_deref().is_some_and(|r| same(r, &address)) {
                let gone = address.clone();
                section = section.child(
                    div()
                        .pt(px(8.))
                        .pb(px(12.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(theme::text_label())
                                .text_color(theme.fg_muted)
                                .child(crate::ui::prose(words::t(&loc, "switcher.removeBody"))),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(4.))
                                .child(
                                    crate::flows::components::danger_button(theme, self.strings.account_remove.clone())
                                        .id(ElementId::from((if in_dialog { "switcher-ms-confirm" } else { "settings-ms-confirm" }, i)))
                                        .cursor_pointer()
                                        .on_click(cx.listener(move |this, _, _, cx| {
                                            this.multisig.removing = None;
                                            let host = this.ms_host(cx);
                                            let address = gone.clone();
                                            host.update(cx, |host, cx| {
                                                host.accounts(ms_accounts::Event::Remove { address }, cx);
                                            });
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    crate::flows::components::ghost_button(theme, self.strings.sign_out_cancel.clone())
                                        .rounded(px(12.))
                                        .id(ElementId::from((if in_dialog { "switcher-ms-cancel" } else { "settings-ms-cancel" }, i)))
                                        .cursor_pointer()
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.multisig.removing = None;
                                            cx.notify();
                                        })),
                                ),
                        ),
                );
            }
        }
        // The two ways to add one.
        let create = crate::flows::components::ghost_button(theme, words::t(&loc, "switcher.create").into())
            .rounded(px(12.))
            .w_auto()
            .flex_initial()
            .min_w(px(0.))
            .px(px(24.))
            .id(if in_dialog { "switcher-ms-create" } else { "settings-ms-create" })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if in_dialog {
                    this.close_account_switcher(cx);
                }
                this.open_multisig_create(cx);
            }));
        let add = crate::flows::components::ghost_button(theme, words::t(&loc, "switcher.add").into())
            .rounded(px(12.))
            .w_auto()
            .flex_initial()
            .min_w(px(0.))
            .px(px(24.))
            .id(if in_dialog { "switcher-ms-add" } else { "settings-ms-add" })
            .cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if in_dialog {
                    this.close_account_switcher(cx);
                }
                this.open_multisig_add(None, cx);
            }));
        section
            .child(
                div()
                    .pt(px(12.))
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(crate::ui::prose(words::t(&loc, "switcher.createMultisigNote"))),
            )
            .child(div().flex().gap(px(12.)).pt(px(8.)).child(create).child(add))
    }

    /// Open the create sheet (its kept draft, if any, comes back with it).
    pub(crate) fn open_multisig_create(&mut self, cx: &mut Context<Self>) {
        self.multisig.create = true;
        self.multisig.scroll = crate::ui::SmoothScroll::new();
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| host.accounts(ms_accounts::Event::NewDraft, cx));
        cx.notify();
    }

    /// Open the add sheet, filled with `text` (an address, or an invite).
    pub(crate) fn open_multisig_add(&mut self, text: Option<String>, cx: &mut Context<Self>) {
        self.multisig.add = Some(text.unwrap_or_default());
        self.multisig.scroll = crate::ui::SmoothScroll::new();
        cx.notify();
    }

    // -- the home's waiting group -------------------------------------------

    /// The multisig's transactions waiting, first on its home (the web's
    /// `waitingRows.group`): one row each with its one word, the group's
    /// note and its Retry; a network the mailbox holds proposals on that
    /// this device lacks, said.
    pub(crate) fn multisig_waiting_group(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let row = self.shown_multisig(cx)?;
        let host = self.ms_host(cx);
        let view = host.read(cx).machines().proposals_view.clone();
        if !view
            .followed
            .as_deref()
            .is_some_and(|f| same(f, &row.account.address))
        {
            return None;
        }
        let loc = self.ms_loc();
        let open: Vec<ProposalItem> = view
            .items
            .iter()
            .filter(|i| i.status != ms_proposals::ProposalStatus::Done)
            .cloned()
            .collect();
        if open.is_empty() && view.missing.is_empty() && view.waiting_note.is_none() {
            return None;
        }
        let mut group = div().flex().flex_col().pb(px(16.)).child(
            div()
                .py(px(8.))
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_base)
                .child(words::t(&loc, "panel.pending")),
        );
        if let Some(note) = &view.waiting_note {
            let (text, retry) = self.ms_note_words(note);
            let mut line = div()
                .flex()
                .items_center()
                .gap(px(8.))
                .py(px(4.))
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(div().flex_1().child(crate::ui::prose(text)));
            if retry {
                line = line.child(
                    text_action(
                        "ms-retry",
                        theme,
                        &mut self.icons,
                        None,
                        words::t(&loc, "service.retry").into(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        let host = this.ms_host(cx);
                        host.update(cx, |host, cx| host.proposals(ms_proposals::Event::Retry, cx));
                    })),
                );
            }
            group = group.child(line);
        }
        for (i, item) in open.iter().enumerate() {
            let key = item_key(item.chain_id, &item.safe_tx_hash);
            let title = self.ms_title(item, Some(&row.account), cx);
            let standing = view
                .standings
                .get(&key)
                .map(|s| self.ms_standing_words(s, item.chain_id))
                .unwrap_or_default();
            let caution = matches!(
                view.standings.get(&key),
                Some(Standing::YourTurn | Standing::Ready)
            );
            let sub = format!(
                "{} · {}",
                network(item.chain_id),
                words::tv(&loc, "action.numberValue", &[("nonce", &item.nonce.to_string())])
            );
            let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
            group = group.child(
                div()
                    .id(ElementId::from(("ms-waiting", i)))
                    .a11y_item(SharedString::from(format!("{title}, {standing}")))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(10.))
                    .border_b_1()
                    .border_color(theme.divider)
                    .cursor_pointer()
                    .hover(|el| el.bg(theme.bg_sunken))
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
                                    .text_color(theme.fg_base)
                                    .truncate()
                                    .child(title),
                            )
                            .child(
                                div()
                                    .text_size(theme::text_label())
                                    .text_color(theme.fg_subtle)
                                    .child(sub),
                            ),
                    )
                    .child(
                        div()
                            .flex_none()
                            .text_size(theme::text_label())
                            .font_weight(if caution {
                                gpui::FontWeight::SEMIBOLD
                            } else {
                                gpui::FontWeight::NORMAL
                            })
                            .text_color(if caution {
                                theme.fg_base
                            } else {
                                theme.fg_subtle
                            })
                            .child(standing),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_multisig_sheet(chain_id, hash.clone(), cx);
                    })),
            );
        }
        for missing in &view.missing {
            group = group.child(
                div()
                    .py(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(words::tv(
                                &loc,
                                "proposal.missingNetwork",
                                &[("chainId", &missing.chain_id.to_string())],
                            )),
                    )
                    .child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(words::tv(
                                &loc,
                                "proposal.missingCount",
                                &[("n", &missing.count.to_string())],
                            )),
                    ),
            );
        }
        Some(group)
    }

    fn ms_note_words(&self, note: &WaitingNote) -> (String, bool) {
        let loc = &self.loc;
        let names = |ids: &[u64]| ids.iter().map(|c| network(*c)).collect::<Vec<_>>().join(", ");
        let text = match note {
            WaitingNote::Offline { since } => match since {
                Some(at) => words::tv(
                    loc,
                    "service.offlineSince",
                    &[("time", &loc.relative_time(*at, crate::executor::now_ms()))],
                ),
                None => words::t(loc, "service.offline"),
            },
            WaitingNote::Unreachable => words::t(loc, "service.unreachableNote"),
            WaitingNote::ChainUnreachable { chain_ids } => {
                words::tv(loc, "service.chainUnreachable", &[("network", &names(chain_ids))])
            }
            WaitingNote::LastKnown { at } => words::tv(
                loc,
                "panel.lastKnown",
                &[("time", &loc.relative_time(*at, crate::executor::now_ms()))],
            ),
            WaitingNote::Stale => words::t(loc, "panel.stale"),
            WaitingNote::HistoryUnreadable { chain_ids } => {
                words::tv(loc, "panel.historyUnreadable", &[("network", &names(chain_ids))])
            }
            WaitingNote::Checking => words::t(loc, "panel.checking"),
        };
        (text, note.retry())
    }

    /// A transaction's one word (`standings`).
    fn ms_standing_words(&self, standing: &Standing, chain_id: u64) -> String {
        let loc = &self.loc;
        match standing {
            Standing::Unsent => words::t(loc, "status.unsent"),
            Standing::Executing => words::t(loc, "status.executing"),
            Standing::Queued => words::t(loc, "status.queued"),
            Standing::Ready => words::t(loc, "status.ready"),
            Standing::YourTurn => words::t(loc, "status.yourTurn"),
            Standing::Progress {
                approved,
                threshold,
            } => words::tv(
                loc,
                "status.progress",
                &[
                    ("approved", &approved.to_string()),
                    ("threshold", &threshold.to_string()),
                ],
            ),
            Standing::Blocked { block } => {
                let reason = match block {
                    ReadyBlock::NobodyPays => words::t(loc, "status.nobodyPays"),
                    ReadyBlock::Fails { why } => self.ms_why_words(why, chain_id),
                };
                words::tv(loc, "status.cannotRun", &[("reason", &reason)])
            }
        }
    }

    /// Why it would fail (`short` with token, have, need; `reverts`).
    fn ms_why_words(&self, why: &Value, chain_id: u64) -> String {
        let loc = &self.loc;
        match why.get("kind").and_then(Value::as_str) {
            Some("short") => {
                let token = why.get("token").and_then(Value::as_str);
                let info = words::token_info(chain_id, token);
                let amount = |k: &str| {
                    let raw = why.get(k).and_then(Value::as_str).unwrap_or("0");
                    format!("{} {}", words::units(raw, info.decimals), info.symbol)
                };
                words::tv(
                    loc,
                    "proposal.whyShort",
                    &[("have", &amount("have")), ("need", &amount("need"))],
                )
            }
            Some("reverts") => match why.get("reason").and_then(Value::as_str) {
                Some(reason) if !reason.is_empty() => {
                    words::tv(loc, "proposal.whyReverts", &[("reason", reason)])
                }
                _ => words::t(loc, "proposal.whyRevertsPlain"),
            },
            _ => words::t(loc, "proposal.uncheckedReason"),
        }
    }

    /// What a proposal is, in one line.
    fn ms_title(&self, item: &ProposalItem, account: Option<&MultisigAccount>, cx: &App) -> String {
        let chain_id = item.chain_id;
        words::action_title(
            &self.loc,
            item.review.action.as_ref(),
            &item.nonce.to_string(),
            &|a: &str| self.ms_member_name(a, account, cx),
            &|t: Option<&str>| {
                // The coin needs no read; a token's symbol is the chain's.
                match t {
                    None => words::token_info(chain_id, None),
                    Some(_) => words::token_info(chain_id, t),
                }
            },
        )
    }

    // -- the transaction's sheet ----------------------------------------------

    /// The third column's title and body for the transaction open, with its
    /// actions pinned under it.
    pub(crate) fn multisig_panel(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<(SharedString, Div, Option<Div>)> {
        let (chain_id, hash) = self.multisig.sheet.clone()?;
        let host = self.ms_host(cx);
        let view = host.read(cx).machines().proposals_view.clone();
        let key = item_key(chain_id, &hash);
        let loc = self.ms_loc();
        let account = self.shown_multisig(cx).map(|r| r.account);
        let linked = view.linked.clone().filter(|l| same(&l.safe_tx_hash, &hash));
        let item = view
            .items
            .iter()
            .find(|i| i.chain_id == chain_id && same(&i.safe_tx_hash, &hash))
            .cloned()
            .or_else(|| linked.as_ref().and_then(|l| l.item.clone()));
        let title: SharedString = words::t(&loc, "proposal.title").into();
        let Some(item) = item else {
            let body = div().child(
                div()
                    .py(px(12.))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(words::t(&loc, if view.loading || linked.is_some() {
                        "proposal.opening"
                    } else {
                        "proposal.notFound"
                    })),
            );
            return Some((title, body, None));
        };
        // The sheet asks for the look now and then; the machine decides
        // whether its last look is stale (W9).
        self.ms_ask_preview(&item, cx);

        let review = view.reviews.get(&key).and_then(ReviewDraw::of);
        let mut body = div().flex().flex_col().gap(px(16.)).pb(px(16.));

        // What it is.
        let heading = self.ms_title(&item, account.as_ref(), cx);
        body = body.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(theme::text_panel_title())
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.fg_base)
                        .child(heading),
                )
                .child(ms_kv(theme, &words::t(&loc, "action.network"), &network(chain_id)))
                .child(ms_kv(
                    theme,
                    &words::t(&loc, "action.number"),
                    &words::tv(&loc, "action.numberValue", &[("nonce", &item.nonce.to_string())]),
                ))
                .children(view.standings.get(&key).map(|s| {
                    ms_kv(
                        theme,
                        &words::t(&loc, "proposal.approvals"),
                        &self.ms_standing_words(s, chain_id),
                    )
                })),
        );

        if let Some(note) = self.ms_dapp_sheet_note(theme, &item, cx) {
            body = body.child(note);
        }
        if linked.as_ref().is_some_and(|l| l.preview) {
            body = body.child(ms_note(theme, &words::t(&loc, "proposal.previewNote"), Tone::Neutral));
        }
        if item.status == ms_proposals::ProposalStatus::Queued {
            body = body.child(ms_note(
                theme,
                &words::tv(&loc, "proposal.queuedNote", &[("nonce", &item.nonce.saturating_sub(1).to_string())]),
                Tone::Neutral,
            ));
        }
        if !item.same_nonce.is_empty() {
            body = body.child(ms_note(
                theme,
                &words::tv(&loc, "proposal.conflict", &[("nonce", &item.nonce.to_string())]),
                Tone::Caution,
            ));
        }
        if item.cancelled_by.is_some() {
            body = body.child(ms_note(theme, &words::t(&loc, "proposal.cancelPending"), Tone::Caution));
        }

        // The one review (§3.6), or that it is being read: no Approve until
        // it is here (the Bybit rule).
        match &review {
            Some(review) => body = body.child(self.ms_review_body(theme, review, chain_id, account.as_ref(), cx)),
            None => {
                body = body.child(ms_note(theme, &words::t(&loc, "proposal.feeChecking"), Tone::Neutral));
            }
        }

        // Approvals: who has, whether it counts on this network now.
        body = body.child(self.ms_approvals(theme, &item, account.as_ref(), cx));

        // How the remaining approvals can come (r7-8).
        if let Some(signers) = view.signers.get(&key) {
            let name_all = |list: &[String]| {
                list.iter()
                    .map(|a| self.ms_member_name(a, account.as_ref(), cx))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            if !signers.enough {
                if !signers.in_vela.is_empty() {
                    body = body.child(ms_note(
                        theme,
                        &words::tv(&loc, "proposal.approvesInVela", &[("names", &name_all(&signers.in_vela))]),
                        Tone::Neutral,
                    ));
                }
                if !signers.either.is_empty() {
                    body = body.child(ms_note(
                        theme,
                        &words::tv(&loc, "proposal.eitherWay", &[("names", &name_all(&signers.either))]),
                        Tone::Neutral,
                    ));
                }
                if !signers.contract.is_empty() {
                    body = body.child(ms_note(
                        theme,
                        &words::tv(&loc, "proposal.contractMembers", &[("names", &name_all(&signers.contract))]),
                        Tone::Caution,
                    ));
                }
            }
        }

        // Its executor's look: the fee and who pays, or why it cannot run.
        if item.status == ms_proposals::ProposalStatus::Ready && !item.can_execute.is_empty() {
            body = body.child(self.ms_preview_line(theme, &view, &item, account.as_ref(), cx));
        }

        // What happened to the last press, said.
        for action in [
            format!("approve:{}", item.safe_tx_hash),
            format!("execute:{}", item.safe_tx_hash),
        ] {
            if let Some(text) = view
                .actions
                .get(&action)
                .and_then(|state| self.ms_action_words(state, chain_id))
            {
                body = body.child(ms_note(theme, &text, Tone::Caution));
            }
        }
        if let Some(text) = view
            .executions
            .get(&key)
            .and_then(|execution| ms_execution_words(&loc, execution))
        {
            body = body.child(ms_note(theme, &text, Tone::Neutral));
        }
        if let Some(entry) = view.outbox.get(&key) {
            let text = match (entry.why.as_str(), entry.retry_at) {
                ("busy", Some(at)) => words::tv(
                    &loc,
                    "proposal.outboxBusy",
                    &[("seconds", &(((at - crate::executor::now_ms()) / 1000.).max(0.).ceil()).to_string())],
                ),
                (_, Some(at)) => words::tv(
                    &loc,
                    "proposal.outboxAway",
                    &[("seconds", &(((at - crate::executor::now_ms()) / 1000.).max(0.).ceil()).to_string())],
                ),
                (_, None) => words::t(&loc, "proposal.outboxNotSent"),
            };
            body = body.child(ms_note(theme, &text, Tone::Caution));
        }

        // Its history.
        if let Some(steps) = view.timelines.get(&key).filter(|s| !s.is_empty()) {
            body = body.child(self.ms_timeline(theme, steps, account.as_ref(), cx));
        }

        // The link the members open it with.
        if let Some(account) = &account {
            let link = vela_core::multisig_link::proposal_link(
                vela_core::multisig_link::SITE_ORIGIN,
                self.loc.language(),
                &vela_core::multisig_link::ProposalLink {
                    chain_id,
                    multisig: account.address.clone(),
                    safe_tx_hash: item.safe_tx_hash.clone(),
                },
            );
            let copied = self.multisig.copied;
            body = body.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(ms_label(theme, &words::t(&loc, "proposal.invite")))
                    .child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(crate::ui::prose(words::t(&loc, "proposal.inviteHint"))),
                    )
                    .child(
                        outline_button(
                            "ms-copy-link",
                            theme,
                            &mut self.icons,
                            Some(Icon::Copy),
                            words::t(&loc, if copied { "proposal.copied" } else { "proposal.copyLink" }).into(),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(link.clone()));
                            this.multisig.copied = true;
                            cx.notify();
                        })),
                    ),
            );
        }

        let foot = self.ms_actions(theme, &view, &item, review.as_ref(), account.as_ref(), cx);
        Some((title, body, foot))
    }

    /// Ask for the executor's look every few seconds while the sheet is up.
    fn ms_ask_preview(&mut self, item: &ProposalItem, cx: &mut Context<Self>) {
        if item.status != ms_proposals::ProposalStatus::Ready || item.can_execute.is_empty() {
            return;
        }
        let executor = self
            .multisig
            .executor
            .clone()
            .filter(|e| item.can_execute.iter().any(|c| same(c, e)))
            .unwrap_or_else(|| item.can_execute[0].clone());
        let key = format!("{}:{}:{}", item.chain_id, item.safe_tx_hash, executor);
        let now = crate::executor::now_ms();
        if self.multisig.previewed.get(&key).is_some_and(|at| now - at < 4_000.) {
            return;
        }
        self.multisig.previewed.insert(key, now);
        let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
        let host = self.ms_host(cx);
        cx.defer(move |cx| {
            host.update(cx, |host, cx| {
                host.proposals(
                    ms_proposals::Event::Preview {
                        chain_id,
                        safe_tx_hash: hash,
                        executor,
                    },
                    cx,
                );
            });
        });
    }

    fn ms_preview_line(
        &mut self,
        theme: &Theme,
        view: &ms_proposals::ProposalsView,
        item: &ProposalItem,
        account: Option<&MultisigAccount>,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let executor = self
            .multisig
            .executor
            .clone()
            .filter(|e| item.can_execute.iter().any(|c| same(c, e)))
            .unwrap_or_else(|| item.can_execute[0].clone());
        let key = format!(
            "{}:{}:{}",
            item.chain_id,
            item.safe_tx_hash.to_ascii_lowercase(),
            executor.to_ascii_lowercase()
        );
        let name = self.ms_member_name(&executor, account, cx);
        let text = match view.previews.get(&key) {
            None => words::t(&loc, "proposal.feeChecking"),
            Some(p) => match p.get("kind").and_then(Value::as_str) {
                Some("priced") => {
                    let info = words::token_info(item.chain_id, None);
                    let amount = p
                        .get("fee")
                        .and_then(|f| f.get("amount"))
                        .map(|a| match a {
                            Value::String(s) => vela_core::app::multisig_shell::quantity(s)
                                .map(|v| v.to_string())
                                .unwrap_or_default(),
                            other => other.to_string(),
                        })
                        .unwrap_or_default();
                    let fee = format!("{} {}", words::units(&amount, info.decimals), info.symbol);
                    let can_pay = p.get("canPay").and_then(Value::as_bool).unwrap_or(true);
                    if can_pay {
                        words::tv(&loc, "proposal.feeLine", &[("fee", &fee), ("name", &name)])
                    } else {
                        let balance = p.get("balance").and_then(Value::as_str).unwrap_or("0");
                        words::tv(
                            &loc,
                            "proposal.cannotPay",
                            &[
                                ("name", &name),
                                ("symbol", &info.symbol),
                                ("fee", &fee),
                                ("balance", &format!("{} {}", words::units(balance, info.decimals), info.symbol)),
                            ],
                        )
                    }
                }
                Some("fails") => words::tv(
                    &loc,
                    "proposal.wouldFail",
                    &[("reason", &self.ms_why_words(p.get("why").unwrap_or(&Value::Null), item.chain_id))],
                ),
                Some("unpriced") => words::t(&loc, "proposal.feeUnpriced"),
                Some("unchecked") => words::t(&loc, "proposal.feeUnchecked"),
                _ => words::t(&loc, "proposal.feeChecking"),
            },
        };
        let mut line = div().flex().flex_col().gap(px(6.)).child(ms_note(theme, &text, Tone::Neutral));
        // Several members here can execute: the person picks who pays.
        if item.can_execute.len() > 1 {
            line = line.child(self.ms_member_picker(theme, "ms-executor", &item.can_execute, &executor, account, true, cx));
        }
        line
    }

    /// A row of members to pick from (`as` / the executor).
    #[allow(clippy::too_many_arguments, reason = "one picker, named parts")]
    fn ms_member_picker(
        &mut self,
        theme: &Theme,
        id: &'static str,
        members: &[String],
        picked: &str,
        account: Option<&MultisigAccount>,
        executor: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row = div()
            .flex()
            .flex_wrap()
            .gap(px(6.))
            .items_center()
            .child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(words::t(&self.loc, "proposal.as")),
            );
        for (i, member) in members.iter().enumerate() {
            let on = same(member, picked);
            let name = self.ms_member_name(member, account, cx);
            let member = member.clone();
            row = row.child(
                div()
                    .id(ElementId::from((id, i)))
                    .a11y_button(SharedString::from(name.clone()))
                    .px(px(10.))
                    .py(px(4.))
                    .rounded(px(12.))
                    .border_1()
                    .border_color(if on { theme.fg_base } else { theme.divider })
                    .bg(if on { theme.bg_sunken } else { theme.bg_base })
                    .text_size(theme::text_label())
                    .text_color(theme.fg_base)
                    .cursor_pointer()
                    .hover(|el| el.bg(theme.bg_sunken))
                    .child(name)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if executor {
                            this.multisig.executor = Some(member.clone());
                        } else {
                            this.multisig.approve_as = Some(member.clone());
                        }
                        cx.notify();
                    })),
            );
        }
        row
    }

    /// The pinned actions: Approve (as a member here), Execute (by one),
    /// with another wallet, Reject, Send again — each the machine's event.
    #[allow(clippy::too_many_arguments, reason = "the sheet's facts")]
    fn ms_actions(
        &mut self,
        theme: &Theme,
        view: &ms_proposals::ProposalsView,
        item: &ProposalItem,
        review: Option<&ReviewDraw>,
        account: Option<&MultisigAccount>,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let loc = self.ms_loc();
        if item.status == ms_proposals::ProposalStatus::Done {
            return None;
        }
        let key = item_key(item.chain_id, &item.safe_tx_hash);
        let approving = matches!(
            view.actions.get(&format!("approve:{}", item.safe_tx_hash)),
            Some(ActionState::Working)
        );
        let executing = item.executing
            || matches!(
                view.actions.get(&format!("execute:{}", item.safe_tx_hash)),
                Some(ActionState::Working)
            );
        // The review decides whether a signature may be given at all.
        let sign = review.map(|r| r.sign).unwrap_or(SignRule {
            allowed: false,
            anyway: false,
        });
        let mut foot = div().flex().flex_col().gap(px(8.));

        if !item.can_approve.is_empty() {
            let as_member = self
                .multisig
                .approve_as
                .clone()
                .filter(|a| item.can_approve.iter().any(|c| same(c, a)))
                .unwrap_or_else(|| item.can_approve[0].clone());
            if item.can_approve.len() > 1 {
                foot = foot.child(self.ms_member_picker(theme, "ms-approve-as", &item.can_approve, &as_member, account, false, cx));
            }
            let label = if sign.anyway {
                words::t(&loc, "review.approveAnyway")
            } else {
                words::t(&loc, "proposal.approve")
            };
            let state = if approving {
                crate::ui::ButtonState::Busy
            } else {
                crate::ui::ButtonState::from_enabled(review.is_some() && sign.allowed)
            };
            let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
            foot = foot.child(crate::ui::vela_button_state(
                "ms-approve",
                crate::ui::ButtonVariant::Primary,
                label.into(),
                state,
                theme,
                cx.listener(move |this, _, _, cx| {
                    let host = this.ms_host(cx);
                    let (hash, owner) = (hash.clone(), as_member.clone());
                    host.update(cx, |host, cx| {
                        host.proposals(
                            ms_proposals::Event::Approve {
                                chain_id,
                                safe_tx_hash: hash,
                                owner,
                            },
                            cx,
                        );
                    });
                }),
            ));
            if review.is_some() && !sign.allowed {
                foot = foot.child(ms_note(theme, &words::t(&loc, "review.blocked"), Tone::Danger));
            }
        }

        if item.status == ms_proposals::ProposalStatus::Ready && !item.can_execute.is_empty() {
            let executor = self
                .multisig
                .executor
                .clone()
                .filter(|e| item.can_execute.iter().any(|c| same(c, e)))
                .unwrap_or_else(|| item.can_execute[0].clone());
            let label = if sign.anyway {
                words::t(&loc, "review.executeAnyway")
            } else {
                words::t(&loc, "proposal.execute")
            };
            let state = if executing {
                crate::ui::ButtonState::Busy
            } else {
                crate::ui::ButtonState::from_enabled(review.is_some() && sign.allowed)
            };
            let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
            foot = foot.child(crate::ui::vela_button_state(
                "ms-execute",
                crate::ui::ButtonVariant::Primary,
                label.into(),
                state,
                theme,
                cx.listener(move |this, _, _, cx| {
                    let host = this.ms_host(cx);
                    let (hash, executor) = (hash.clone(), executor.clone());
                    host.update(cx, |host, cx| {
                        host.proposals(
                            ms_proposals::Event::Execute {
                                chain_id,
                                safe_tx_hash: hash,
                                executor,
                            },
                            cx,
                        );
                    });
                }),
            ));
        }

        // An outside member's wallet (no member here can approve).
        let outside = view
            .signers
            .get(&key)
            .is_some_and(|s| !s.enough && (!s.outside.is_empty() || !s.either.is_empty()));
        if outside {
            let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
            foot = foot.child(
                crate::ui::vela_button(
                    "ms-outside",
                    crate::ui::ButtonVariant::Secondary,
                    words::t(&loc, "external.signWithWallet").into(),
                    theme,
                    cx.listener(move |this, _, _, cx| {
                        let host = this.ms_host(cx);
                        let hash = hash.clone();
                        host.update(cx, |host, cx| {
                            host.proposals(
                                ms_proposals::Event::ApproveOutside {
                                    chain_id,
                                    safe_tx_hash: hash,
                                },
                                cx,
                            );
                        });
                    }),
                ),
            );
        }

        if view.outbox.contains_key(&key) {
            let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
            foot = foot.child(crate::ui::vela_button(
                "ms-send-again",
                crate::ui::ButtonVariant::Secondary,
                words::t(&loc, "proposal.outboxSendAgain").into(),
                theme,
                cx.listener(move |this, _, _, cx| {
                    let host = this.ms_host(cx);
                    let hash = hash.clone();
                    host.update(cx, |host, cx| {
                        host.proposals(
                            ms_proposals::Event::SendAgain {
                                chain_id: Some(chain_id),
                                safe_tx_hash: Some(hash),
                            },
                            cx,
                        );
                    });
                }),
            ));
        }

        // Reject: a cancellation at its number, approved as any other.
        let members_here = !item.can_approve.is_empty() || !item.can_execute.is_empty()
            || item.approvals.iter().any(|a| {
                session::view(cx)
                    .accounts
                    .iter()
                    .any(|row| same(&row.account.address, &a.owner))
            });
        if members_here && item.cancelled_by.is_none() && !executing {
            if self.multisig.rejecting {
                let threshold = item.threshold.to_string();
                let (chain_id, nonce) = (item.chain_id, item.nonce.to_string());
                foot = foot
                    .child(ms_note(
                        theme,
                        &words::tv(&loc, "proposal.rejectHint", &[("threshold", &threshold)]),
                        Tone::Neutral,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(
                                crate::ui::vela_button(
                                    "ms-reject-cancel",
                                    crate::ui::ButtonVariant::Secondary,
                                    words::t(&loc, "proposal.rejectCancel").into(),
                                    theme,
                                    cx.listener(|this, _, _, cx| {
                                        this.multisig.rejecting = false;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(crate::ui::vela_button(
                                "ms-reject-confirm",
                                crate::ui::ButtonVariant::Primary,
                                words::t(&loc, "proposal.reject").into(),
                                theme,
                                cx.listener(move |this, _, _, cx| {
                                    this.multisig.rejecting = false;
                                    let host = this.ms_host(cx);
                                    let nonce = nonce.clone();
                                    host.update(cx, |host, cx| {
                                        host.proposals(
                                            ms_proposals::Event::Draft {
                                                chain_id,
                                                proposal: ms_proposals::ProposalKind::Cancel { nonce },
                                            },
                                            cx,
                                        );
                                    });
                                }),
                            )),
                    );
            } else {
                foot = foot.child(
                    destructive_text_button("ms-reject", theme, words::t(&loc, "proposal.reject").into())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.multisig.rejecting = true;
                            cx.notify();
                        })),
                );
            }
        }
        Some(foot)
    }

    /// What the last press came to, when it did not go (`ActionState`).
    fn ms_action_words(&self, state: &ActionState, chain_id: u64) -> Option<String> {
        let loc = &self.loc;
        let reason = |key: &str| words::t(loc, &format!("proposal.reason.{key}"));
        Some(match state {
            ActionState::Working | ActionState::Done { .. } | ActionState::OutsideAsked { .. } => {
                return None;
            }
            ActionState::Cancelled => reason("passkeyClosed"),
            ActionState::MailboxUnreadable => words::t(loc, "proposal.mailboxDown"),
            ActionState::NoMemberHere => words::t(loc, "proposal.notMember"),
            ActionState::SigningRefused { .. } | ActionState::SigningBlocked => {
                words::t(loc, "review.blocked")
            }
            ActionState::SigningUnchecked => {
                words::tv(loc, "proposal.cantCheck", &[("network", &network(chain_id))])
            }
            ActionState::RiskDeclined => return None,
            ActionState::Refused { reason: why } => {
                words::tv(loc, "proposal.refused", &[("reason", why)])
            }
            ActionState::MailboxRefused { .. } => reason("serviceRefused"),
            // The look stopped it before the passkey: why, as the look says.
            ActionState::Stopped { preview } => match preview.get("kind").and_then(Value::as_str) {
                Some("unpriced") => words::tv(
                    loc,
                    "proposal.failed",
                    &[("reason", &words::t(loc, "proposal.unpricedReason"))],
                ),
                Some("unchecked") => words::tv(
                    loc,
                    "proposal.failed",
                    &[("reason", &words::t(loc, "proposal.uncheckedReason"))],
                ),
                _ => words::tv(
                    loc,
                    "proposal.wouldFail",
                    &[("reason", &self.ms_why_words(preview.get("why").unwrap_or(preview), chain_id))],
                ),
            },
            ActionState::Failed { detail } => self.ms_failure_words(detail, chain_id),
            ActionState::NoOutsideSigner => words::tv(
                loc,
                "proposal.failed",
                &[("reason", &words::t(loc, "proposal.noOutsideSigner"))],
            ),
        })
    }

    /// A shell failure (`live::ShellFailure`) or the core's own words.
    fn ms_failure_words(&self, detail: &str, chain_id: u64) -> String {
        use crate::multisig::live::{ShellFailure, read_failure};
        let loc = &self.loc;
        let reason = |key: &str| words::t(loc, &format!("proposal.reason.{key}"));
        match read_failure(detail) {
            Some(ShellFailure::Passkey { timed_out: true, .. }) => reason("passkeyTimedOut"),
            Some(ShellFailure::Passkey { failure, .. }) if failure == "cancelled" => reason("passkeyClosed"),
            Some(ShellFailure::Passkey { .. }) => reason("passkeyMissing"),
            Some(ShellFailure::NotSent) => words::t(loc, "proposal.notSent"),
            Some(ShellFailure::CannotPay { need, have }) => {
                let info = words::token_info(chain_id, None);
                words::tv(
                    loc,
                    "proposal.whyShort",
                    &[
                        ("have", &format!("{} {}", words::units(&have, info.decimals), info.symbol)),
                        ("need", &format!("{} {}", words::units(&need, info.decimals), info.symbol)),
                    ],
                )
            }
            Some(ShellFailure::Unknown { message }) => {
                words::tv(loc, "proposal.failed", &[("reason", &message)])
            }
            Some(_) => reason("unknown"),
            None => words::tv(loc, "proposal.failed", &[("reason", detail)]),
        }
    }

    // -- the review ---------------------------------------------------------

    /// The one review every signer signs from (§3.6): what it says, who it
    /// reaches, what moves, the state-diff guard, a member change's rule,
    /// and the hashes — drawn, never decoded again here.
    fn ms_review_body(
        &mut self,
        theme: &Theme,
        review: &ReviewDraw,
        chain_id: u64,
        account: Option<&MultisigAccount>,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let net = network(chain_id);
        let mut col = div().flex().flex_col().gap(px(12.));

        if let Some(refusal) = review.refusal() {
            col = col.child(ms_note(
                theme,
                &words::tv(&loc, "proposal.refused", &[("reason", &refusal)]),
                Tone::Danger,
            ));
        }

        // A member change, read before anything else (`MemberChangeRule`):
        // the caution, each change with its address whole, the rule it meets
        // and the one it leaves ("Approvals: 2 of 3 → 2 of 4").
        if let Some(changes) = &review.member_changes {
            let lines = crate::multisig::member_words::change_lines(&loc, changes, &net, &|a: &str| {
                self.ms_member_name(a, account, cx)
            });
            col = col.child(ms_note(theme, &lines.caution, Tone::Caution));
            for (c, line) in changes.changes.iter().zip(&lines.changes) {
                let whole: Vec<String> = [c.removed.as_deref(), c.added.as_deref()]
                    .into_iter()
                    .flatten()
                    .map(vela_core::multisig_link::eip55)
                    .collect();
                col = col.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(theme::text_row_sub())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.fg_base)
                                .child(line.clone()),
                        )
                        .children(whole.into_iter().map(|a| {
                            div()
                                .font_family(theme::font_mono())
                                .text_size(theme::text_label())
                                .text_color(theme.fg_muted)
                                .child(a)
                        })),
                );
            }
            col = col.child(ms_note(theme, &lines.rule, Tone::Caution));
            if let Some(misfit) = &lines.does_not_fit {
                col = col.child(ms_note(theme, misfit, Tone::Danger));
            }
        }

        // Who it reaches.
        let mut parties = div().flex().flex_col().gap(px(6.));
        for (i, r) in review.recipients.iter().enumerate() {
            let role = match r.role {
                RecipientRole::Recipient => words::t(&loc, "review.roleRecipient"),
                RecipientRole::Contract => words::t(&loc, "review.roleContract"),
                RecipientRole::Member => words::t(&loc, "review.roleMember"),
            };
            let role = match r.call {
                Some(n) => format!("{} · {}", words::tv(&loc, "review.callNumber", &[("n", &n.to_string())]), role),
                None => role,
            };
            let name = r
                .known
                .as_ref()
                .map(|k| k.label.clone())
                .unwrap_or_else(|| self.ms_member_name(&r.address, account, cx));
            parties = parties.child(
                div()
                    .id(ElementId::from(("ms-party", i)))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(ms_label(theme, &role))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(name),
                    )
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(vela_core::multisig_link::eip55(&r.address)),
                    ),
            );
        }
        col = col.child(parties);

        // What it says of itself, in its tone.
        for concern in &review.concerns {
            let (text, tone) = match concern {
                Concern::NotSigned { why } => (words::tv(&loc, "proposal.refused", &[("reason", why)]), Tone::Danger),
                Concern::StateChange => (words::tv(&loc, "review.stateBody", &[("network", &net)]), Tone::Danger),
                Concern::StateUnread { .. } => (words::tv(&loc, "review.stateUnread", &[("network", &net)]), Tone::Caution),
                Concern::Reverts => (words::t(&loc, "proposal.whyRevertsPlain"), Tone::Danger),
                Concern::Unsimulated => (words::t(&loc, "proposal.uncheckedReason"), Tone::Caution),
                Concern::Lookalike { address, of, label } => (
                    words::tv(&loc, "review.lookalike", &[("address", address), ("label", label), ("known", of)]),
                    Tone::Danger,
                ),
                Concern::FirstTime { address } => (
                    format!("{} · {}", self.loc.t("componentsUi.signing.firstTimeTag"), address),
                    Tone::Caution,
                ),
                Concern::ToContract { address } => {
                    (words::tv(&loc, "review.toContract", &[("address", address)]), Tone::Caution)
                }
                Concern::Blind => (self.loc.t("clearSigning.modalNoDescriptor").to_string(), Tone::Caution),
            };
            col = col.child(ms_note(theme, &text, tone));
        }

        // What moves (the simulation, or the probe's balances).
        if let Some(sim) = &review.simulation {
            if let Some(key) = sim.notice_key.as_ref().or(sim.no_change_key.as_ref()) {
                let text = self
                    .loc
                    .t_texts(key, &[("reason", sim.revert_reason.as_deref().unwrap_or_default())])
                    .to_string();
                col = col.child(ms_note(theme, &text, Tone::Neutral));
            }
            for (i, delta) in sim.deltas.iter().enumerate() {
                let info = words::token_info(chain_id, delta.token.as_deref());
                let (sign, raw) = match delta.delta.strip_prefix('-') {
                    Some(raw) => ("−", raw),
                    None => ("+", delta.delta.as_str()),
                };
                col = col.child(
                    div()
                        .id(ElementId::from(("ms-delta", i)))
                        .text_size(theme::text_row_title())
                        .text_color(theme.fg_base)
                        .child(format!("{sign}{} {}", words::units(raw, info.decimals), info.symbol)),
                );
            }
            if review.simulation_source == "probe" {
                col = col.child(ms_note(theme, &words::t(&loc, "review.movesProbe"), Tone::Neutral));
            }
        }

        // The state-diff guard (the Bybit lesson).
        if let Some(state) = &review.state {
            use vela_core::multisig_review::StateVerdict;
            let (text, tone) = match &state.verdict {
                StateVerdict::Unchanged => (words::tv(&loc, "review.stateUnchanged", &[("network", &net)]), Tone::Neutral),
                StateVerdict::Declared => (words::tv(&loc, "review.stateDeclared", &[("network", &net)]), Tone::Neutral),
                StateVerdict::Undeclared { writes } => {
                    let lines: Vec<String> = writes.iter().map(|w| ms_write_line(&loc, w)).collect();
                    (
                        format!("{}\n{}", words::t(&loc, "review.stateTitle"), lines.join("\n")),
                        Tone::Danger,
                    )
                }
                StateVerdict::Reverts => (words::t(&loc, "proposal.whyRevertsPlain"), Tone::Caution),
                StateVerdict::Unavailable { .. } => {
                    (words::tv(&loc, "review.stateUnread", &[("network", &net)]), Tone::Caution)
                }
            };
            col = col.child(ms_note(theme, &text, tone));
        }

        // The technical details: the SafeTx as signed and its hashes.
        let open = self.multisig.tech;
        col = col.child(
            text_action(
                "ms-tech",
                theme,
                &mut self.icons,
                Some(if open { Icon::ChevronDown } else { Icon::ChevronRight }),
                words::t(&loc, "review.techTitle").into(),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.multisig.tech = !this.multisig.tech;
                cx.notify();
            })),
        );
        if open {
            let mut tech = div().flex().flex_col().gap(px(4.));
            for field in &review.fields {
                tech = tech.child(ms_mono_kv(theme, &field.name, &field.value));
            }
            if let Some(h) = &review.hashes {
                tech = tech
                    .child(ms_label(theme, &words::t(&loc, "review.hashesTitle")))
                    .child(ms_mono_kv(theme, "domainHash", &h.domain_hash))
                    .child(ms_mono_kv(theme, "messageHash", &h.message_hash))
                    .child(ms_mono_kv(theme, "safeTxHash", &h.safe_tx_hash));
            }
            col = col.child(tech);
        }
        col
    }

    fn ms_approvals(
        &mut self,
        theme: &Theme,
        item: &ProposalItem,
        account: Option<&MultisigAccount>,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let mut col = div().flex().flex_col().gap(px(4.)).child(ms_label(
            theme,
            &format!(
                "{} · {}",
                words::t(&loc, "proposal.approvals"),
                words::tv(
                    &loc,
                    "status.progress",
                    &[
                        ("approved", &item.approved.min(item.threshold).to_string()),
                        ("threshold", &item.threshold.to_string())
                    ]
                )
            ),
        ));
        for (i, approval) in item.approvals.iter().enumerate() {
            let name = self.ms_member_name(&approval.owner, account, cx);
            let note = if approval.counts {
                words::t(&loc, "timeline.counted")
            } else {
                words::t(&loc, "timeline.notCounted")
            };
            col = col.child(
                div()
                    .id(ElementId::from(("ms-approval", i)))
                    .flex()
                    .justify_between()
                    .text_size(theme::text_row_sub())
                    .child(div().text_color(theme.fg_base).child(name))
                    .child(div().text_color(theme.fg_subtle).child(note)),
            );
        }
        col
    }

    fn ms_timeline(
        &mut self,
        theme: &Theme,
        steps: &[TimelineStep],
        account: Option<&MultisigAccount>,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(ms_label(theme, &words::t(&loc, "timeline.heading")));
        for (i, step) in steps.iter().enumerate() {
            let text = match step {
                TimelineStep::Created { who, .. } => {
                    words::tv(&loc, "timeline.created", &[("name", &self.ms_member_name(who, account, cx))])
                }
                TimelineStep::Approved { who, by_executing, counted, .. } => {
                    let key = if *by_executing { "timeline.approvedByExecuting" } else { "timeline.approved" };
                    let mut line = words::tv(&loc, key, &[("name", &self.ms_member_name(who, account, cx))]);
                    if let Some(counted) = counted {
                        line = format!(
                            "{line} · {}",
                            words::t(&loc, if *counted { "timeline.counted" } else { "timeline.notCounted" })
                        );
                    }
                    line
                }
                TimelineStep::Executing => words::t(&loc, "proposal.executing"),
                TimelineStep::CancelProposed { .. } => words::t(&loc, "proposal.cancelPending"),
                TimelineStep::Executed { who, .. } => match who {
                    Some(who) => words::tv(&loc, "timeline.executedBy", &[("name", &self.ms_member_name(who, account, cx))]),
                    None => words::t(&loc, "timeline.executed"),
                },
                #[allow(unreachable_patterns)]
                _ => continue,
            };
            col = col.child(
                div()
                    .id(ElementId::from(("ms-step", i)))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(text),
            );
        }
        col
    }

    // -- the layer over the page: risk, draft, session ------------------------

    /// The multisig dialogs over the page, topmost first: the high-risk
    /// question, the send flow's review-first draft, an outside owner's
    /// session.
    pub(crate) fn multisig_layer(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.multisig_layer_up(cx) {
            return None;
        }
        let host = ms_host::booted(cx)?;
        if let Some(prompt) = self.ms_ceremony_layer(theme, window, &host, cx) {
            return Some(prompt);
        }
        if let Some(risk) = host.read(cx).risk.clone() {
            return Some(self.ms_risk_dialog(theme, window, &risk, cx));
        }
        if host.read(cx).send_reviewing() {
            return self.ms_draft_dialog(theme, window, None, cx);
        }
        if self.multisig.session && host.read(cx).machines().session_view().is_some() {
            return Some(self.ms_session_dialog(theme, window, cx));
        }
        if let Some(layer) = self.ms_dapp_layer(theme, window, cx) {
            return Some(layer);
        }
        if self.multisig.create {
            return self.ms_create_dialog(theme, window, cx);
        }
        if self.multisig.add.is_some() {
            return self.ms_add_dialog(theme, window, cx);
        }
        if self.multisig.invite.is_some() {
            return self.ms_invite_dialog(theme, window, cx);
        }
        None
    }

    /// A passkey ceremony a multisig signature runs (an approval, an
    /// execution, a record's proof): a security key's PIN, the account to
    /// pick, the phone's QR, the touch — the same cards the send flow draws
    /// (`hardware::*`), over everything, as a ceremony's question is.
    fn ms_ceremony_layer(
        &mut self,
        theme: &Theme,
        window: &Window,
        host: &gpui::Entity<MultisigHost>,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let channel = host.read(cx).ceremony.clone();
        let scrim = |id: &'static str| {
            div()
                .id(id)
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.backdrop)
        };
        if let Some(request) = channel.pending_pin() {
            let focus = match &self.multisig.pin {
                Some((_, focus)) => focus.clone(),
                None => {
                    let focus = cx.focus_handle();
                    self.multisig.pin = Some((String::new(), focus.clone()));
                    focus
                }
            };
            let value = self.multisig.pin.as_ref().map(|(v, _)| v.clone()).unwrap_or_default();
            let page = cx.entity().downgrade();
            let on_change = {
                let page = page.clone();
                move |text: String, _: &mut Window, cx: &mut gpui::App| {
                    let _ = page.update(cx, |page, cx| {
                        if let Some((value, _)) = page.multisig.pin.as_mut() {
                            *value = text;
                        }
                        cx.notify();
                    });
                }
            };
            let on_confirm = {
                let (page, channel) = (page.clone(), channel.clone());
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                    let value = page
                        .update(cx, |page, _| page.multisig.pin.take().map(|(v, _)| v))
                        .ok()
                        .flatten();
                    channel.answer_pin(value);
                }
            };
            let on_cancel = {
                let channel = channel.clone();
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                    let _ = page.update(cx, |page, _| page.multisig.pin = None);
                    channel.answer_pin(None);
                }
            };
            let card = crate::hardware::pin_card(
                theme, &self.loc, &request, &value, &focus, window, on_change, on_confirm, on_cancel,
            );
            return Some(scrim("ms-pin-scrim").child(card).into_any_element());
        }
        self.multisig.pin = None;
        if let Some(choices) = channel.pending_choice() {
            let (pick, cancel) = (channel.clone(), channel.clone());
            let card = crate::hardware::pick_card(
                theme,
                &self.loc,
                &choices,
                move |index: &usize, _: &mut Window, _: &mut gpui::App| pick.answer_choice(Some(*index)),
                move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| cancel.answer_choice(None),
            );
            return Some(scrim("ms-pick-scrim").child(card).into_any_element());
        }
        if let Some(payload) = channel.qr_showing() {
            let cancel = channel.clone();
            let card = crate::hardware::qr_card_with(
                theme,
                &self.loc,
                &payload,
                crate::hardware::scan_line(&self.loc),
                move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| cancel.cancel_qr(),
            );
            return Some(scrim("ms-qr-scrim").child(card).into_any_element());
        }
        if let Some(waiting) = channel.touch_waiting() {
            let cancel = channel.clone();
            let card = crate::hardware::touch_card(
                theme,
                &self.loc,
                &waiting,
                move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| cancel.cancel_touch(),
            );
            return Some(scrim("ms-touch-scrim").child(card).into_any_element());
        }
        None
    }

    /// A multisig dialog is over the page (`dialog_up`: the in-app
    /// browser's page goes off the screen under it).
    pub(crate) fn multisig_layer_up(&self, cx: &App) -> bool {
        let Some(host) = ms_host::booted(cx) else {
            return false;
        };
        let host = host.read(cx);
        let channel = &host.ceremony;
        channel.pending_pin().is_some()
            || channel.pending_choice().is_some()
            || channel.qr_showing().is_some()
            || channel.touch_waiting().is_some()
            || host.risk.is_some()
            || host.send_reviewing()
            || (self.multisig.session && host.machines().session_view().is_some())
            || host.machines().accounts_view.link.is_some()
            || self.multisig.create
            || self.multisig.add.is_some()
            || self.multisig.invite.is_some()
    }

    fn ms_risk_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        risk: &ms_host::RiskAsk,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let loc = self.ms_loc();
        let net = network(risk.chain_id);
        let findings: Vec<serde_json::Value> = risk.findings.clone();
        let findings = crate::multisig::findings::finding_lines(&loc, &findings, &net);
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(crate::ui::dialog::dialog_body(
                theme,
                words::tv(&loc, "authority.signRiskBody", &[("network", &net)]),
            ))
            .children(findings.into_iter().map(|f| ms_note(theme, &f, Tone::Danger)))
            .child(
                crate::ui::dialog::dialog_actions()
                    .child(
                        crate::ui::vela_button(
                            "ms-risk-no",
                            crate::ui::ButtonVariant::Secondary,
                            words::t(&loc, "authority.signRiskCancel").into(),
                            theme,
                            cx.listener(|this, _, _, cx| {
                                let host = this.ms_host(cx);
                                host.update(cx, |host, cx| host.answer_risk(false, cx));
                            }),
                        )
                        .w_auto(),
                    )
                    .child(
                        crate::ui::vela_button(
                            "ms-risk-yes",
                            crate::ui::ButtonVariant::Primary,
                            words::t(&loc, "authority.signRiskConfirm").into(),
                            theme,
                            cx.listener(|this, _, _, cx| {
                                let host = this.ms_host(cx);
                                host.update(cx, |host, cx| host.answer_risk(true, cx));
                            }),
                        )
                        .w_auto(),
                    ),
            );
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        crate::ui::dialog::dialog(
            "ms-risk",
            theme,
            window,
            words::t(&loc, "authority.signRiskTitle").into(),
            None,
            close_icon,
            body,
            &self.multisig.scroll,
            cx.listener(|this, _, _, cx| {
                let host = this.ms_host(cx);
                host.update(cx, |host, cx| host.answer_risk(false, cx));
            }),
        )
        .into_any_element()
    }

    /// The send flow's proposal, on its review before any passkey (§3.6):
    /// "Propose and approve" signs it once; closing withdraws it. A site's
    /// request (`site`: the machine's id for it, and the site) is the same
    /// review with the site said, and Decline or a close leaves it — the
    /// machine answers the site (AUDIT M5).
    fn ms_draft_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        site: Option<(String, String)>,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let host = self.ms_host(cx);
        let view = host.read(cx).machines().proposals_view.clone();
        let draft = view.draft.clone()?;
        let loc = self.ms_loc();
        let account = self.shown_multisig(cx).map(|r| r.account);
        let item = draft.item.clone();
        let review = draft.review.as_ref().and_then(ReviewDraw::of);
        let mut body = div().flex().flex_col().gap(px(14.));
        if let Some((_, host)) = &site {
            body = body.child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(words::tv(&loc, "proposal.siteAsks", &[("host", host)])),
            );
        }
        body = body
            .child(
                div()
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .child(self.ms_title(&item, account.as_ref(), cx)),
            )
            .child(ms_kv(theme, &words::t(&loc, "action.network"), &network(item.chain_id)))
            .child(ms_kv(
                theme,
                &words::t(&loc, "action.number"),
                &words::tv(&loc, "action.numberValue", &[("nonce", &item.nonce.to_string())]),
            ));
        match &review {
            Some(review) => body = body.child(self.ms_review_body(theme, review, item.chain_id, account.as_ref(), cx)),
            None => body = body.child(ms_note(theme, &words::t(&loc, "proposal.feeChecking"), Tone::Neutral)),
        }
        let as_member = self
            .multisig
            .approve_as
            .clone()
            .filter(|a| draft.members_here.iter().any(|m| same(m, a)))
            .or(draft.default_member.clone());
        if draft.members_here.len() > 1 {
            let picked = as_member.clone().unwrap_or_default();
            body = body.child(self.ms_member_picker(theme, "ms-draft-as", &draft.members_here, &picked, account.as_ref(), false, cx));
        }
        body = body.child(ms_note(theme, &words::t(&loc, "proposal.feeNote"), Tone::Neutral));
        // A site's proposal stays on its review when it did not go: why.
        if site.is_some()
            && let Some(text) = view
                .actions
                .get("propose")
                .and_then(|state| self.ms_action_words(state, item.chain_id))
        {
            body = body.child(ms_note(theme, &text, Tone::Caution));
        }
        let sign = review.as_ref().map(|r| r.sign).unwrap_or_default();
        let working = matches!(view.actions.get("propose"), Some(ActionState::Working));
        let leave = site.as_ref().map(|(rid, _)| rid.clone());
        let label = if draft.members_here.is_empty() {
            words::t(&loc, "send.proposeOnly")
        } else if sign.anyway {
            words::t(&loc, "review.proposeAnyway")
        } else {
            words::t(&loc, "send.propose")
        };
        let state = if working {
            crate::ui::ButtonState::Busy
        } else {
            crate::ui::ButtonState::from_enabled(review.is_some() && sign.allowed)
        };
        body = body.child(
            crate::ui::dialog::dialog_actions()
                .child({
                    let leave = leave.clone();
                    crate::ui::vela_button(
                        "ms-draft-withdraw",
                        crate::ui::ButtonVariant::Secondary,
                        words::t(
                            &loc,
                            if leave.is_some() {
                                "external.decline"
                            } else {
                                "proposal.dontPropose"
                            },
                        )
                        .into(),
                        theme,
                        cx.listener(move |this, _, _, cx| this.ms_draft_leave(leave.clone(), cx)),
                    )
                    .w_auto()
                })
                .child(
                    crate::ui::vela_button_state(
                        "ms-draft-propose",
                        crate::ui::ButtonVariant::Primary,
                        label.into(),
                        state,
                        theme,
                        cx.listener(move |this, _, _, cx| {
                            let host = this.ms_host(cx);
                            let as_member = as_member.clone();
                            host.update(cx, |host, cx| host.confirm_draft(as_member, cx));
                        }),
                    )
                    .w_auto(),
                ),
        );
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        Some(
            crate::ui::dialog::dialog(
                "ms-draft",
                theme,
                window,
                words::t(&loc, "proposal.title").into(),
                None,
                close_icon,
                body,
                &self.multisig.scroll,
                cx.listener(move |this, _, _, cx| this.ms_draft_leave(leave.clone(), cx)),
            )
            .into_any_element(),
        )
    }

    /// The draft's way out: the send flow's proposal withdrawn, or a site's
    /// request (`rid`) left — the machine answers the site.
    fn ms_draft_leave(&mut self, rid: Option<String>, cx: &mut Context<Self>) {
        match rid {
            Some(rid) => self.ms_dapp_leave(rid, cx),
            None => {
                let host = self.ms_host(cx);
                host.update(cx, |host, cx| host.withdraw_draft(cx));
            }
        }
    }

    /// Vela's side of an outside owner's session (M4): the connect link and
    /// its code, where the session stands, and the person's ways out. The
    /// owner opens the link in a browser with their own wallet; the page
    /// reviews the same transaction from the core's one review.
    fn ms_session_dialog(&mut self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> gpui::AnyElement {
        let host = self.ms_host(cx);
        let loc = self.ms_loc();
        let (view, request) = match host.read(cx).machines().session_view() {
            Some((view, request)) => (view.clone(), request.clone()),
            None => return div().into_any_element(),
        };
        // The page to open: with the invite for a new room, none for one
        // paired before — the members who may sign and the transaction, all
        // in the fragment (the core's `connect_link`).
        let page = format!(
            "{}/{}/connect",
            vela_core::multisig_link::share_origin(vela_core::multisig_link::SITE_ORIGIN),
            self.loc.language()
        );
        let tx = view.tx.as_ref().map(|t| vela_core::multisig_link::ProposalLink {
            chain_id: t.chain_id,
            multisig: t.safe.clone(),
            safe_tx_hash: t.safe_tx_hash.clone(),
        });
        let link = (view.open && (view.invite.is_some() || view.resumed)).then(|| {
            vela_core::multisig_link::connect_link(
                &page,
                view.invite.as_deref(),
                &view.members,
                tx.as_ref(),
            )
        });
        let mut body = div().flex().flex_col().gap(px(12.));
        body = body.child(crate::ui::dialog::dialog_body(theme, crate::multisig::session_words::phase(&loc, &view)));
        if let Some(failure) = &view.failure {
            body = body.child(ms_note(theme, &crate::multisig::session_words::failure(&loc, failure), Tone::Caution));
        }
        // How this side is getting onto the relay, with its Retry.
        if let Some(line) =
            crate::multisig::session_words::relay(&loc, &view.relay, crate::executor::now_ms())
        {
            body = body.child(ms_note(theme, &line, Tone::Caution)).child(
                text_action(
                    "ms-session-relay-retry",
                    theme,
                    &mut self.icons,
                    None,
                    words::t(&loc, "external.retry").into(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    let host = this.ms_host(cx);
                    host.update(cx, |host, cx| host.retry_relay(cx));
                })),
            );
        }
        if let Some(link) = &link {
            body = body.child(
                div()
                    .flex()
                    .justify_center()
                    .child(crate::flows::components::qr_card(theme, None, Some(link))),
            );
            let open = link.clone();
            let copy = link.clone();
            body = body.child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        crate::ui::vela_button(
                            "ms-session-copy",
                            crate::ui::ButtonVariant::Secondary,
                            words::t(&loc, "proposal.copyLink").into(),
                            theme,
                            move |_, _, cx| {
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone()));
                            },
                        )
                        .w_auto(),
                    )
                    .child(
                        crate::ui::vela_button(
                            "ms-session-open",
                            crate::ui::ButtonVariant::Secondary,
                            loc.t("explore.openInSystemBrowser").to_string().into(),
                            theme,
                            move |_, _, cx| {
                                crate::executor::opener::open(&open, cx);
                            },
                        )
                        .w_auto(),
                    ),
            );
        }
        let _ = request;
        body = body.child(
            crate::ui::dialog::dialog_actions()
                .child(
                    crate::ui::vela_button(
                        "ms-session-again",
                        crate::ui::ButtonVariant::Secondary,
                        words::t(&loc, "external.askAgain").into(),
                        theme,
                        cx.listener(|this, _, _, cx| {
                            let host = this.ms_host(cx);
                            host.update(cx, |host, cx| {
                                host.session(vela_core::app::multisig_outside::session::Event::AskAgain, cx);
                            });
                        }),
                    )
                    .w_auto(),
                )
                .child(
                    crate::ui::vela_button(
                        "ms-session-close",
                        crate::ui::ButtonVariant::Secondary,
                        crate::ui::a11y::close_label(),
                        theme,
                        cx.listener(|this, _, _, cx| {
                            this.multisig.session = false;
                            let host = this.ms_host(cx);
                            host.update(cx, |host, cx| host.end_session(cx));
                        }),
                    )
                    .w_auto(),
                ),
        );
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        crate::ui::dialog::dialog(
            "ms-session",
            theme,
            window,
            words::t(&loc, "external.sheetTitle").into(),
            None,
            close_icon,
            body,
            &self.multisig.scroll,
            cx.listener(|this, _, _, cx| {
                this.multisig.session = false;
                let host = this.ms_host(cx);
                host.update(cx, |host, cx| host.end_session(cx));
            }),
        )
        .into_any_element()
    }
}

/// How a line is said.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tone {
    Neutral,
    Caution,
    Danger,
}

fn ms_note(theme: &Theme, text: &str, tone: Tone) -> Div {
    let (bg, fg) = match tone {
        Tone::Neutral => (theme.bg_sunken, theme.fg_muted),
        Tone::Caution => (theme.warning_soft, theme.fg_base),
        Tone::Danger => (theme.error_soft, theme.error_base),
    };
    div()
        .px(px(12.))
        .py(px(8.))
        .rounded(px(10.))
        .bg(bg)
        .text_size(theme::text_row_sub())
        .line_height(gpui::relative(1.4))
        .text_color(fg)
        .child(crate::ui::prose(text.to_owned()))
}

fn ms_label(theme: &Theme, text: &str) -> Div {
    div()
        .text_size(theme::text_label())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.fg_subtle)
        .child(text.to_owned())
}

fn ms_kv(theme: &Theme, key: &str, value: &str) -> Div {
    div()
        .flex()
        .justify_between()
        .gap(px(12.))
        .text_size(theme::text_row_sub())
        .child(div().text_color(theme.fg_subtle).child(key.to_owned()))
        .child(div().text_color(theme.fg_base).child(value.to_owned()))
}

fn ms_mono_kv(theme: &Theme, key: &str, value: &str) -> Div {
    div()
        .flex()
        .flex_col()
        .text_size(theme::text_label())
        .child(div().text_color(theme.fg_subtle).child(key.to_owned()))
        .child(
            div()
                .font_family(theme::font_mono())
                .text_color(theme.fg_base)
                .child(value.to_owned()),
        )
}

/// One undeclared change, in words (`writeLine`).
fn ms_write_line(loc: &Loc, w: &vela_core::multisig_review::CriticalWrite) -> String {
    use vela_core::multisig_review::CriticalWrite as W;
    match w {
        W::Singleton { before, after } => words::tv(loc, "review.writeSingleton", &[("before", before), ("after", after)]),
        W::Module { address, enabled } => words::tv(
            loc,
            if *enabled { "review.writeModuleOn" } else { "review.writeModuleOff" },
            &[("address", address)],
        ),
        W::Guard { after, .. } => words::tv(loc, "review.writeGuard", &[("after", after)]),
        W::ModuleGuard { after, .. } => words::tv(loc, "review.writeModuleGuard", &[("after", after)]),
        W::Handler { after, .. } => words::tv(loc, "review.writeHandler", &[("after", after)]),
        W::Owners { added, removed } => added
            .iter()
            .map(|a| words::tv(loc, "review.writeAdded", &[("address", a)]))
            .chain(removed.iter().map(|a| words::tv(loc, "review.writeRemoved", &[("address", a)])))
            .collect::<Vec<_>>()
            .join("\n"),
        W::OwnerEntry { address } => words::tv(loc, "review.writeOwnerEntry", &[("address", address)]),
        W::Threshold { before, after } => words::tv(loc, "review.writeThreshold", &[("before", before), ("after", after)]),
        W::OwnerCount { after, .. } => words::tv(loc, "review.writeOwnerCount", &[("after", after)]),
        W::Nonce => words::t(loc, "review.writeNonce"),
        W::Storage { slot } => words::tv(loc, "review.writeStorage", &[("slot", slot)]),
    }
}

/// How an execution followed here ended (`executions`), when it is news.
fn ms_execution_words(loc: &Loc, execution: &Value) -> Option<String> {
    let key = match execution.get("kind").and_then(Value::as_str)? {
        "executed" => "proposal.executed",
        "executing" | "pending" => "proposal.executing",
        "lostRace" => "proposal.lostRace",
        "lostRacePaid" => "proposal.lostRacePaid",
        "failed" | "execFailed" => "proposal.execFailed",
        "notLanded" => "proposal.notLanded",
        _ => return None,
    };
    Some(words::t(loc, key))
}
