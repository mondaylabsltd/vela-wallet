//! A multisig's own part of Settings → Account (AUDIT M1, M3) — the web's
//! `MultisigSettings.svelte` with `MemberChanges.svelte`,
//! `MemberChangeRule.svelte` and `MemberKindChoice.svelte`.
//!
//! **Where it is (the lane's call):** the web passes this block as the
//! Account page's `accountKeys` while a multisig is shown — it takes the
//! place of the Vela account's keys (and its signing venue), and everything
//! else on the page (the accounts, signing out) stays. The desktop does the
//! same: `settings_account_footer` draws [`WalletPage::multisig_settings_block`]
//! instead of the keys block while `shown_multisig` is `Some`. No new nav
//! entry: the multisig is the account the page is about, as on the web.
//!
//! Everything here is a machine's view, drawn: the accounts machine's row
//! (its name, its record, who can act for it — `check_on`), its rename plan
//! and backup; the members machine's view (each network's members, the
//! change being composed, its plan per network, the changes waiting). Every
//! press is an event. Proposing — the rename, each network's member change —
//! goes through the host's one review-first path (`MultisigHost::propose`):
//! the draft's review dialog the send flow uses, then the person's word.
//!
//! The local label (`set_label`) is not offered: the web offers none (the
//! row's name is the record's name, the label only stands in when there is
//! none, so an edited label would look like it did nothing) and the corpus
//! has no words for it. The shared name is renamed by the multisig itself.

use vela_core::app::multisig_accounts::BackupState;
use vela_core::app::multisig_members::{
    self as ms_members, Compose, EntryView, MemberRow, MultisigMembersView, NetworkState,
};
use vela_core::app::multisig_signers::{KindNote, MemberKind};

use gpui::prelude::FluentBuilder as _;
use gpui::{FocusHandle, Stateful};

use super::*;
use crate::multisig::host::Proposed;
use crate::multisig::member_words as mw;

/// One network's member change, as it goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Progress {
    Waiting,
    Proposing,
    Proposed,
    Failed,
}

/// This block's own state: what the person is typing and pressed, and what
/// was asked of the machines for the multisig shown. What a machine holds
/// is never copied here.
#[derive(Default)]
pub(crate) struct SettingsUi {
    /// The multisig the members machine was shown (its record as it was).
    shown: Option<MultisigAccount>,
    /// The record state the rename plan was asked for (r7-10: asked again
    /// when it moves).
    planned: Option<(String, bool, bool)>,
    /// The networks whose authority was asked (`check_on`) this visit.
    checked: Vec<(String, u64)>,
    /// The new shared name being typed (`None`: not renaming).
    renaming: Option<String>,
    rename_busy: bool,
    rename_failure: Option<String>,
    /// "Remove from this device" is being confirmed.
    removing: bool,
    /// Each network's member change, while it is proposed.
    progress: Vec<(u64, Progress)>,
    failure: Option<String>,
    /// The member entry had the keyboard on the last frame (its blur reads it).
    entry_focused: bool,
    focus: Option<Focus>,
}

struct Focus {
    rename: FocusHandle,
    entry: FocusHandle,
    name: FocusHandle,
}

impl SettingsUi {
    fn busy(&self) -> bool {
        self.progress.iter().any(|(_, p)| *p == Progress::Proposing)
    }
}

/// A small outlined button (Replace, Remove, a network's progress).
fn small_button(
    id: impl Into<ElementId>,
    theme: &Theme,
    label: String,
    a11y: String,
) -> Stateful<Div> {
    div()
        .id(id)
        .a11y_button(a11y)
        .flex_none()
        .px(px(10.))
        .py(px(4.))
        .rounded(px(8.))
        .border_1()
        .border_color(theme.divider)
        .text_size(theme::text_label())
        .text_color(theme.fg_base)
        .cursor_pointer()
        .hover(|el| el.bg(theme.bg_sunken))
        .child(label)
}

fn heading(theme: &Theme, text: String) -> Div {
    div()
        .text_size(theme::text_row_title())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.fg_base)
        // r14-11a: "Replace 0x88cCA0…266894" — the short form whole.
        .child(crate::ui::prose(text))
}

fn muted(theme: &Theme, text: String) -> Div {
    div()
        .text_size(theme::text_row_sub())
        .line_height(gpui::relative(1.4))
        .text_color(theme.fg_muted)
        .child(crate::ui::prose(text))
}

fn error_line(theme: &Theme, text: String) -> Div {
    div()
        .text_size(theme::text_label())
        .text_color(theme.error_base)
        .child(crate::ui::prose(text))
}

fn rule_line(theme: &Theme) -> Div {
    div().h(px(1.)).bg(theme.divider).my(px(20.))
}

impl WalletPage {
    /// The Account page was left: coming back reads the members again, as
    /// the web's page does each time it is shown.
    pub(crate) fn multisig_settings_left(&mut self) {
        let s = &mut self.multisig.settings;
        s.shown = None;
        s.planned = None;
        s.checked.clear();
    }

    /// Escape on this block: what it has open goes (the confirm, the
    /// rename, the change being composed). `true` when one did.
    pub(crate) fn multisig_settings_escape(&mut self, cx: &mut Context<Self>) -> bool {
        if !(self.section == Section::Settings && self.settings_page == SettingsPage::Account) {
            return false;
        }
        let s = &mut self.multisig.settings;
        if s.removing {
            s.removing = false;
            cx.notify();
            return true;
        }
        if s.renaming.is_some() && !s.rename_busy {
            s.renaming = None;
            cx.notify();
            return true;
        }
        let composing = self.shown_multisig(cx).is_some()
            && self
                .ms_host(cx)
                .read(cx)
                .machines()
                .members_view
                .compose
                .is_some();
        if composing && !self.multisig.settings.busy() {
            self.ms_members_compose(None, cx);
            return true;
        }
        false
    }

    fn ms_members_send(&mut self, event: ms_members::Event, cx: &mut Context<Self>) {
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| host.members(event, cx));
        cx.notify();
    }

    fn ms_members_compose(&mut self, compose: Option<Compose>, cx: &mut Context<Self>) {
        self.multisig.settings.progress.clear();
        self.multisig.settings.failure = None;
        self.ms_members_send(ms_members::Event::Compose { compose }, cx);
    }

    /// What the machines are asked as the block is drawn: the members of
    /// the multisig shown (again when its record moved), where its shared
    /// name can be renamed (again when its record state moves), its backup,
    /// and who can act for it on each of its networks — each once a visit.
    fn ms_settings_ask(&mut self, row: &ms_accounts::AccountRow, cx: &mut Context<Self>) {
        let host = self.ms_host(cx);
        let address = row.account.address.clone();
        let key = address.to_ascii_lowercase();
        // Its record as the members are read from it: its networks, its
        // members' kept names. A verdict found since (`findings`) is not a
        // reason to read every network again.
        let record = MultisigAccount {
            findings: None,
            ..row.account.clone()
        };
        if self.multisig.settings.shown.as_ref() != Some(&record) {
            let first = self
                .multisig
                .settings
                .shown
                .as_ref()
                .is_none_or(|a| !same(&a.address, &address));
            self.multisig.settings.shown = Some(record);
            let contacts = member_contacts(cx);
            host.update(cx, |host, cx| {
                host.show_members(&address, cx);
                host.members(ms_members::Event::Contacts { contacts }, cx);
                if first {
                    host.accounts(
                        ms_accounts::Event::CheckBackup {
                            address: address.clone(),
                        },
                        cx,
                    );
                }
            });
        }
        let planned = (key.clone(), row.shared, row.record_waiting);
        if self.multisig.settings.planned.as_ref() != Some(&planned) {
            self.multisig.settings.planned = Some(planned);
            host.update(cx, |host, cx| {
                host.accounts(
                    ms_accounts::Event::PlanRename {
                        address: address.clone(),
                    },
                    cx,
                );
            });
        }
        let mut chains = row.account.chain_ids.clone();
        chains.sort_unstable();
        chains.dedup();
        for chain_id in chains {
            if self
                .multisig
                .settings
                .checked
                .contains(&(key.clone(), chain_id))
            {
                continue;
            }
            self.multisig.settings.checked.push((key.clone(), chain_id));
            host.update(cx, |host, cx| {
                host.accounts(
                    ms_accounts::Event::CheckOn {
                        address: address.clone(),
                        chain_id,
                    },
                    cx,
                );
            });
        }
    }

    fn ms_settings_focus(
        &mut self,
        cx: &mut Context<Self>,
    ) -> (FocusHandle, FocusHandle, FocusHandle) {
        let focus = self.multisig.settings.focus.get_or_insert_with(|| Focus {
            rename: cx.focus_handle(),
            entry: cx.focus_handle(),
            name: cx.focus_handle(),
        });
        (
            focus.rename.clone(),
            focus.entry.clone(),
            focus.name.clone(),
        )
    }

    /// The multisig shown's own part of Settings → Account; `None` while
    /// the Vela account is shown.
    pub(crate) fn multisig_settings_block(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let row = self.shown_multisig(cx)?;
        self.ms_settings_ask(&row, cx);
        let loc = self.ms_loc();
        let host = self.ms_host(cx);
        let (members, accounts, service) = {
            let machines = host.read(cx).machines();
            (
                machines.members_view.clone(),
                machines.accounts_view.clone(),
                machines.current_facts().service_url.clone(),
            )
        };
        let members = members
            .address
            .as_deref()
            .is_some_and(|a| same(a, &row.account.address))
            .then_some(members);
        let (_, entry_focus, _) = self.ms_settings_focus(cx);
        // The entry's blur reads what is typed (the web's `onblur`).
        let entry_focused = entry_focus.is_focused(window);
        if self.multisig.settings.entry_focused && !entry_focused {
            self.ms_members_send(ms_members::Event::ReadEntry, cx);
        }
        self.multisig.settings.entry_focused = entry_focused;

        let mut col = div().flex().flex_col().gap(px(14.));
        col = col.child(self.ms_set_identity(theme, &loc, &row, members.as_ref(), cx));
        // Nobody is on a developer's service without knowing.
        col = col.children(mw::custom_service(&loc, &service).map(|note| muted(theme, note)));
        col = col.children(self.ms_set_authority(theme, &loc, &row, cx));
        if let Some(view) = &members {
            col = col.child(self.ms_set_members(theme, &loc, view, cx));
        }
        // None of its members (anywhere) is on this device: the members
        // machine's word once it read them, its founding members' until then.
        let read_only = members
            .as_ref()
            .map_or(row.local_members.is_empty(), |v| v.read_only);
        if read_only {
            col = col.child(muted(theme, words::t(&loc, "panel.readOnly")));
        }
        if let Some(view) = &members {
            col = col.child(self.ms_set_changes(theme, &loc, &row, view, window, cx));
        }
        col = col.children(self.ms_set_record(theme, &loc, &row, &accounts, cx));
        // Invite members (the dialog is the invite sheet's).
        let invite_for = row.account.address.clone();
        col = col.child(
            div().flex().child(
                crate::ui::vela_button(
                    "ms-set-invite",
                    crate::ui::ButtonVariant::Secondary,
                    words::t(&loc, "panel.invite").into(),
                    theme,
                    cx.listener(move |this, _, _, cx| {
                        this.multisig.invite = Some(invite_for.clone());
                        this.multisig.scroll = crate::ui::SmoothScroll::new();
                        cx.notify();
                    }),
                )
                .w_auto(),
            ),
        );
        col = col.child(rule_line(theme));
        col = col.child(self.ms_set_rename(theme, &loc, &row, &accounts, window, cx));
        col = col.child(rule_line(theme));
        col = col.child(self.ms_set_backup(theme, &loc, &row, &accounts, cx));
        col = col.child(rule_line(theme));
        col = col.child(self.ms_set_remove(theme, &loc, &row, cx));
        Some(div().pt(px(8.)).child(col))
    }

    /// Its name, its address and its rule; whether its record is shared.
    fn ms_set_identity(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        row: &ms_accounts::AccountRow,
        members: Option<&MultisigMembersView>,
        cx: &mut Context<Self>,
    ) -> Div {
        let account = &row.account;
        let n = account.owners.len() as u64;
        let name = row
            .name
            .clone()
            .or_else(|| account.label.clone())
            .unwrap_or_else(|| {
                words::tv(
                    loc,
                    "switcher.rowName",
                    &[("m", &account.threshold.to_string()), ("n", &n.to_string())],
                )
            });
        // The rule in force on its networks, once they are read (the core's
        // chip); the original rule until then.
        // §9.2: until the members machine has read them, the rule the
        // accounts machine keeps for it (from the chain when one said it),
        // never the founding rule over a chain's.
        let (chip, title) = members
            .and_then(|v| v.rule_chip.as_ref())
            .map_or_else(|| mw::chip(loc, &row.rule.chip), |c| mw::chip(loc, c));
        let refresh_label = words::t(loc, "proposal.feeRetry");
        let mut col = div().flex().flex_col().gap(px(6.)).child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .child(crate::wallet::components::identicon_avatar(
                    &mut self.identicons,
                    &account.address,
                    SWITCHER_IDENTICON,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(heading(theme, name))
                        .child(
                            div()
                                .font_family(theme::font_mono())
                                .text_size(theme::text_label())
                                .text_color(theme.fg_subtle)
                                .child(crate::ui::address_text(vela_core::multisig_link::eip55(
                                    &account.address,
                                ))),
                        ),
                )
                .child(
                    div()
                        .id("ms-set-rule")
                        .a11y_item(title.clone())
                        .px(px(10.))
                        .py(px(4.))
                        .rounded(px(999.))
                        .bg(theme.bg_sunken)
                        .text_size(theme::text_label())
                        .text_color(theme.fg_muted)
                        .child(chip),
                )
                .child(
                    div()
                        .id("ms-set-refresh")
                        .a11y_button(refresh_label)
                        .flex_none()
                        .p(px(6.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .hover(|el| el.bg(theme.bg_sunken))
                        .child(icon_img(
                            &mut self.icons,
                            Icon::RefreshCw,
                            false,
                            theme.fg_muted,
                            16.,
                        ))
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.ms_members_send(ms_members::Event::Refresh, cx);
                        })),
                ),
        );
        col = col.child(heading(theme, words::t(loc, "panel.members")).pt(px(8.)));
        col
    }

    /// §3.5: what the chain says of who can act for it — a genesis refused
    /// on a network (read-only there), a risk where it is deployed — and a
    /// network whose authority could not be checked yet, with its Check
    /// again (`check_on`).
    fn ms_set_authority(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        row: &ms_accounts::AccountRow,
        cx: &mut Context<Self>,
    ) -> Vec<Div> {
        let mut out = Vec::new();
        let findings = row.account.findings.clone().unwrap_or_default();
        let from = |f: &serde_json::Value| {
            f.get("from")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        };
        let refused: Vec<serde_json::Value> = findings
            .iter()
            .filter(|f| from(f).as_deref() == Some("genesis"))
            .cloned()
            .collect();
        let risk: Vec<serde_json::Value> = findings
            .iter()
            .filter(|f| from(f).as_deref() == Some("chain"))
            .cloned()
            .collect();
        if !refused.is_empty() || !row.refused_on.is_empty() {
            let mut lines: Vec<String> = row
                .refused_on
                .iter()
                .map(|id| words::tv(loc, "authority.refusedOn", &[("network", &network(*id))]))
                .collect();
            lines.extend(crate::multisig::findings::finding_lines(loc, &refused, ""));
            out.push(authority_notice(
                theme,
                words::t(loc, "authority.refusedTitle"),
                words::t(loc, "authority.refusedBody"),
                lines,
            ));
        }
        if !risk.is_empty() || !row.high_risk_on.is_empty() {
            let mut lines = crate::multisig::findings::finding_lines(loc, &risk, "");
            if lines.is_empty() {
                lines = row
                    .high_risk_on
                    .iter()
                    .map(|id| format!("{} · {}", network(*id), words::t(loc, "authority.riskFlag")))
                    .collect();
            }
            out.push(authority_notice(
                theme,
                words::t(loc, "authority.riskTitle"),
                words::t(loc, "authority.riskBody"),
                lines,
            ));
        }
        // Asked this visit, not being read, and no verdict: it could not be
        // checked (an unanswered read is no verdict) — said, and asked again
        // on the person's word.
        let key = row.account.address.to_ascii_lowercase();
        for (i, (_, chain_id)) in self
            .multisig
            .settings
            .checked
            .clone()
            .into_iter()
            .filter(|(k, _)| *k == key)
            .enumerate()
        {
            if row.checked_on.contains(&chain_id) || row.checking_on.contains(&chain_id) {
                continue;
            }
            let address = row.account.address.clone();
            out.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(ms_note(
                        theme,
                        &words::tv(
                            loc,
                            "authority.unverified",
                            &[("network", &network(chain_id))],
                        ),
                        Tone::Caution,
                    ))
                    .child(
                        div().flex().child(
                            small_button(
                                ElementId::from(("ms-set-check", i)),
                                theme,
                                words::t(loc, "proposal.feeRetry"),
                                format!(
                                    "{} · {}",
                                    words::t(loc, "proposal.feeRetry"),
                                    network(chain_id)
                                ),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    let host = this.ms_host(cx);
                                    let address = address.clone();
                                    host.update(cx, |host, cx| {
                                        host.accounts(
                                            ms_accounts::Event::CheckOn { address, chain_id },
                                            cx,
                                        );
                                    });
                                    cx.notify();
                                },
                            )),
                        ),
                    ),
            );
        }
        out
    }

    /// A list of members, each named as this device names it, its address
    /// beside it, what changed and which are here.
    fn ms_set_member_list(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        rows: &[MemberRow],
        removed: &[MemberRow],
    ) -> Div {
        let mut list = div().flex().flex_col();
        let all = rows
            .iter()
            .map(|r| (r, false))
            .chain(removed.iter().map(|r| (r, true)));
        for (r, gone) in all {
            let short = words::short(&r.address);
            let name = r.name.clone().unwrap_or_else(|| short.clone());
            let mut text = div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(if gone { theme.fg_muted } else { theme.fg_base })
                        .when(gone, |el| el.line_through())
                        .child(name.clone()),
                );
            // A member with no name reads its address once, not twice (ms-26).
            if name != short {
                text = text.child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .when(gone, |el| el.line_through())
                        .child(crate::ui::address_text(short)),
                );
            }
            let mut line = div()
                .flex()
                .items_center()
                .gap(px(10.))
                .py(px(8.))
                .border_b_1()
                .border_color(theme.divider)
                .child(crate::wallet::components::identicon_avatar(
                    &mut self.identicons,
                    &r.address,
                    24.,
                ))
                .child(text);
            let tag = |text: String| {
                div()
                    .flex_none()
                    .text_size(theme::text_label())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .child(text)
            };
            if gone {
                line = line.child(tag(words::t(loc, "settings.removed")));
            } else if r.added {
                line = line.child(tag(words::t(loc, "settings.added")));
            }
            if r.on_device && !gone {
                line = line.child(
                    div()
                        .flex_none()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_muted)
                        .child(words::t(loc, "panel.thisDevice")),
                );
            }
            list = list.child(line);
        }
        list
    }

    /// The original members, and each network's now (M3): read through this
    /// wallet's RPC, said as they stand — reading, unreadable, not deployed
    /// (it starts with the original members), or its members and rule.
    fn ms_set_members(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        view: &MultisigMembersView,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut col = div().flex().flex_col().gap(px(14.));
        let set_head = |title: String, rule: Option<String>| {
            div()
                .flex()
                .items_baseline()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(title),
                )
                .children(rule.map(|r| {
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_muted)
                        .child(r)
                }))
        };
        let original = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(set_head(
                words::t(loc, "settings.originalMembers"),
                view.original.rule.as_ref().map(|r| mw::rule_title(loc, r)),
            ))
            .child(muted(theme, words::t(loc, "settings.originalNote")))
            .child(self.ms_set_member_list(theme, loc, &view.original.members, &[]));
        col = col.child(original);
        for (i, n) in view.networks.iter().enumerate() {
            let net = network(n.chain_id);
            let title = words::tv(loc, "settings.currentOn", &[("network", &net)]);
            match &n.state {
                NetworkState::Reading => {
                    col = col.child(set_head(title, Some("…".to_owned())));
                }
                NetworkState::Unreadable { detail } => {
                    // Its chain did not answer: nothing is said of its
                    // members there, and it is read again on the person's word.
                    col = col.child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(set_head(title, None))
                            .child(
                                div()
                                    .text_size(theme::text_label())
                                    .text_color(theme.fg_subtle)
                                    .child(crate::ui::prose(detail.clone())),
                            )
                            .child(
                                div().flex().child(
                                    small_button(
                                        ElementId::from(("ms-set-reread", i)),
                                        theme,
                                        words::t(loc, "external.retry"),
                                        format!("{} · {net}", words::t(loc, "external.retry")),
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.ms_members_send(ms_members::Event::Refresh, cx);
                                        },
                                    )),
                                ),
                            ),
                    );
                }
                NetworkState::Undeployed => {
                    col = col.child(muted(
                        theme,
                        words::tv(loc, "settings.undeployedOn", &[("network", &net)]),
                    ));
                }
                NetworkState::Deployed {
                    rule,
                    members,
                    removed,
                    same,
                    threshold_was,
                    threshold_is,
                    ..
                } => {
                    let mut set = div()
                        .flex()
                        .flex_col()
                        .gap(px(4.))
                        .child(set_head(title, Some(mw::rule_title(loc, rule))));
                    if *same {
                        set = set.child(muted(theme, words::t(loc, "settings.sameAsOriginal")));
                    } else {
                        set = set.child(self.ms_set_member_list(theme, loc, members, removed));
                        if threshold_was != threshold_is {
                            set = set.child(muted(
                                theme,
                                words::tv(
                                    loc,
                                    "settings.thresholdChanged",
                                    &[
                                        ("was", &threshold_was.to_string()),
                                        ("is", &threshold_is.to_string()),
                                    ],
                                ),
                            ));
                        }
                    }
                    col = col.child(set);
                }
            }
        }
        col
    }

    /// How a member is named in the change: the row the machine gives, else
    /// the core's naming rule.
    fn ms_set_who(&self, view: &MultisigMembersView, address: &str, cx: &App) -> String {
        view.rows
            .iter()
            .chain(view.original.members.iter())
            .find(|r| same(&r.address, address))
            .and_then(|r| r.name.clone())
            .unwrap_or_else(|| {
                let account = self.shown_multisig(cx).map(|r| r.account);
                self.ms_member_name(address, account.as_ref(), cx)
            })
    }

    /// The approvals stepper ("2 of 3", − and +), labelled `label`.
    fn ms_set_stepper(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        view: &MultisigMembersView,
        label: String,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let stepper = view.stepper?;
        let busy = self.multisig.settings.busy();
        let step = |id: &'static str, glyph: &'static str, a11y: String, enabled: bool, by: i64| {
            let mut el = div()
                .id(id)
                .a11y_button(a11y)
                .w(px(32.))
                .h(px(32.))
                .rounded(px(8.))
                .border_1()
                .border_color(theme.divider)
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::text_row_title())
                .text_color(if enabled {
                    theme.fg_base
                } else {
                    theme.fg_subtle
                })
                .child(glyph);
            if enabled {
                el = el
                    .cursor_pointer()
                    .hover(|el| el.bg(theme.bg_sunken))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.ms_members_send(ms_members::Event::Step { by }, cx);
                    }));
            }
            el
        };
        let fewer = step(
            "ms-set-fewer",
            "−",
            words::t(loc, "create.fewer"),
            stepper.can_fewer && !busy,
            -1,
        );
        let more = step(
            "ms-set-more",
            "+",
            words::t(loc, "create.more"),
            stepper.can_more && !busy,
            1,
        );
        Some(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(label),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(fewer)
                        .child(
                            div()
                                .min_w(px(56.))
                                .flex()
                                .justify_center()
                                .text_size(theme::text_row_sub())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.fg_base)
                                .child(mw::rule_value(loc, stepper.value, stepper.n)),
                        )
                        .child(more),
                ),
        )
    }

    /// Change members (M3): each member with Replace and Remove, Add member,
    /// the approvals stepper, the change being composed with its plan per
    /// network and its Propose, and the changes waiting.
    fn ms_set_changes(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        row: &ms_accounts::AccountRow,
        view: &MultisigMembersView,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let busy = self.multisig.settings.busy();
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .pt(px(8.))
            .child(heading(theme, words::t(loc, "members.title")));
        if view.none_deployed {
            // Nothing deployed: a change would deploy it. Said, nothing offered.
            return col.child(muted(theme, words::t(loc, "members.noneDeployed")));
        }
        if !view.rows.is_empty() {
            let mut list = div().flex().flex_col();
            for (i, r) in view.rows.iter().enumerate() {
                let who = r.name.clone().unwrap_or_else(|| words::short(&r.address));
                let replace_owner = r.address.clone();
                let remove_owner = r.address.clone();
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .py(px(8.))
                        .border_b_1()
                        .border_color(theme.divider)
                        .child(crate::wallet::components::identicon_avatar(
                            &mut self.identicons,
                            &r.address,
                            24.,
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
                                        .text_size(theme::text_row_sub())
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.fg_base)
                                        .truncate()
                                        .child(who.clone()),
                                )
                                .when(who != words::short(&r.address), |el| {
                                    el.child(
                                        div()
                                            .font_family(theme::font_mono())
                                            .text_size(theme::text_label())
                                            .text_color(theme.fg_subtle)
                                            .child(crate::ui::address_text(words::short(
                                                &r.address,
                                            ))),
                                    )
                                }),
                        )
                        .when(!busy, |el| {
                            el.child(
                                small_button(
                                    ElementId::from(("ms-set-replace", i)),
                                    theme,
                                    words::t(loc, "members.replace"),
                                    words::tv(loc, "members.replaceLabel", &[("name", &who)]),
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.ms_members_compose(
                                            Some(Compose::Replace {
                                                owner: replace_owner.clone(),
                                            }),
                                            cx,
                                        );
                                    },
                                )),
                            )
                            .child(
                                small_button(
                                    ElementId::from(("ms-set-remove-member", i)),
                                    theme,
                                    words::t(loc, "members.remove"),
                                    words::tv(loc, "members.removeLabel", &[("name", &who)]),
                                )
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        this.ms_members_compose(
                                            Some(Compose::Remove {
                                                owner: remove_owner.clone(),
                                            }),
                                            cx,
                                        );
                                    },
                                )),
                            )
                        }),
                );
            }
            col = col
                .child(list)
                .child(muted(theme, words::t(loc, "members.lostPasskey")));
            if !matches!(view.compose, Some(Compose::Add)) && !busy {
                col = col.child(
                    div().flex().child(
                        crate::ui::vela_button(
                            "ms-set-add-member",
                            crate::ui::ButtonVariant::Secondary,
                            words::t(loc, "members.add").into(),
                            theme,
                            cx.listener(|this, _, _, cx| {
                                this.ms_members_compose(Some(Compose::Add), cx)
                            }),
                        )
                        .w_auto(),
                    ),
                );
            }
            if matches!(view.compose, None | Some(Compose::Threshold)) {
                col = col.children(self.ms_set_stepper(
                    theme,
                    loc,
                    view,
                    words::t(loc, "members.thresholdTitle"),
                    cx,
                ));
                if let Some(recovery) = &view.recovery {
                    col = col.child(muted(theme, mw::recovery(loc, recovery)));
                }
            }
        }
        if let Some(compose) = view.compose.clone() {
            col = col.child(self.ms_set_compose(theme, loc, row, view, &compose, window, cx));
        }
        if !view.groups.is_empty() {
            col = col.child(self.ms_set_waiting(theme, loc, view, cx));
        }
        col
    }

    /// The change being composed: who, the approvals after, who replaces
    /// whom, the plan on each network, and Propose.
    #[allow(clippy::too_many_arguments)]
    fn ms_set_compose(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        row: &ms_accounts::AccountRow,
        view: &MultisigMembersView,
        compose: &Compose,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .p(px(14.))
            .rounded(px(12.))
            .border_1()
            .border_color(theme.divider);
        match compose {
            Compose::Add => {
                col = col
                    .child(heading(theme, words::t(loc, "members.add")))
                    .child(self.ms_set_entry(theme, loc, &view.entry, false, window, cx))
                    .children(self.ms_set_stepper(
                        theme,
                        loc,
                        view,
                        words::t(loc, "members.thresholdAfter"),
                        cx,
                    ));
            }
            Compose::Replace { owner } => {
                let name = self.ms_set_who(view, owner, cx);
                col = col
                    .child(heading(
                        theme,
                        words::tv(loc, "members.replaceLabel", &[("name", &name)]),
                    ))
                    .child(muted(
                        theme,
                        words::tv(loc, "members.replaceIntro", &[("name", &name)]),
                    ))
                    .child(self.ms_set_entry(theme, loc, &view.entry, true, window, cx));
                if let Some(preview) = &view.replace_preview {
                    // Round 5: who replaces whom, before anything is proposed.
                    let new = preview
                        .new_name
                        .clone()
                        .unwrap_or_else(|| self.ms_set_who(view, &preview.new_address, cx));
                    col = col.child(
                        div()
                            .text_size(theme::text_row_sub())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(crate::ui::prose(words::tv(
                                loc,
                                "members.changeReplace",
                                &[("old", &name), ("new", &new)],
                            ))),
                    );
                }
            }
            Compose::Remove { owner } => {
                let name = self.ms_set_who(view, owner, cx);
                col = col
                    .child(heading(
                        theme,
                        words::tv(loc, "members.removeLabel", &[("name", &name)]),
                    ))
                    .child(muted(
                        theme,
                        words::tv(loc, "members.removeIntro", &[("name", &name)]),
                    ))
                    .children(self.ms_set_stepper(
                        theme,
                        loc,
                        view,
                        words::t(loc, "members.thresholdAfter"),
                        cx,
                    ));
            }
            Compose::Threshold => {}
        }
        if let Some(plan) = &view.plan {
            let mut list = div().flex().flex_col().gap(px(4.));
            for n in &plan.networks {
                let net = network(n.chain_id);
                let (line, refused) = mw::plan_line(loc, n, &net);
                let state = self
                    .multisig
                    .settings
                    .progress
                    .iter()
                    .find(|(c, _)| *c == n.chain_id)
                    .map(|(_, p)| *p);
                let said = match state {
                    Some(Progress::Proposing) => {
                        Some(words::tv(loc, "members.proposing", &[("network", &net)]))
                    }
                    Some(Progress::Proposed) => {
                        Some(words::tv(loc, "members.proposed", &[("network", &net)]))
                    }
                    _ => None,
                };
                list = list.child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(8.))
                        .text_size(theme::text_row_sub())
                        .text_color(if refused {
                            theme.error_base
                        } else {
                            theme.fg_base
                        })
                        .child(crate::ui::prose(line))
                        .children(said.map(|s| div().text_color(theme.fg_muted).child(s))),
                );
            }
            for chain_id in &plan.undeployed {
                list = list.child(muted(
                    theme,
                    words::tv(
                        loc,
                        "settings.undeployedOn",
                        &[("network", &network(*chain_id))],
                    ),
                ));
            }
            col = col
                .child(
                    div()
                        .text_size(theme::text_label())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_subtle)
                        .child(words::t(loc, "members.perNetwork")),
                )
                .child(list);
        }
        if let Some(failure) = self.multisig.settings.failure.clone() {
            col = col.child(error_line(theme, failure));
        }
        let busy = self.multisig.settings.busy();
        let state = if busy {
            crate::ui::ButtonState::Busy
        } else {
            crate::ui::ButtonState::from_enabled(view.can_propose)
        };
        let address = row.account.address.clone();
        col.child(
            crate::ui::dialog::dialog_actions()
                .child(
                    crate::ui::vela_button_state(
                        "ms-set-change-cancel",
                        crate::ui::ButtonVariant::Secondary,
                        self.strings.sign_out_cancel.clone(),
                        crate::ui::ButtonState::from_enabled(!busy),
                        theme,
                        cx.listener(|this, _, _, cx| {
                            if !this.multisig.settings.busy() {
                                this.ms_members_compose(None, cx);
                            }
                        }),
                    )
                    .w_auto(),
                )
                .child(
                    crate::ui::vela_button_state(
                        "ms-set-change-propose",
                        crate::ui::ButtonVariant::Value,
                        words::t(loc, "send.propose").into(),
                        state,
                        theme,
                        cx.listener(move |this, _, _, cx| this.ms_propose_change(&address, cx)),
                    )
                    .w_auto(),
                ),
        )
    }

    /// Who joins: an address, an ENS name or a suggestion (read on Enter or
    /// when the field lets go), the address it resolved to in full, the name
    /// kept for it here, and what kind of member it is.
    fn ms_set_entry(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        entry: &EntryView,
        replacing: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let (_, entry_focus, name_focus) = self.ms_settings_focus(cx);
        let strings = crate::ui::NameFieldStrings {
            label: words::t(
                loc,
                if replacing {
                    "members.newAccount"
                } else {
                    "members.entryLabel"
                },
            )
            .into(),
            placeholder: words::t(loc, "members.entryPlaceholder").into(),
            helper: SharedString::from(""),
            too_long_hint: SharedString::from(""),
        };
        let field = crate::ui::text_field(
            "ms-set-entry",
            theme,
            &strings,
            &entry.typed,
            false,
            false,
            &entry_focus,
            window,
            {
                let page = cx.entity();
                move |text: String, _: &mut Window, cx: &mut gpui::App| {
                    page.update(cx, |this: &mut Self, cx| {
                        this.ms_members_send(ms_members::Event::Entry { text }, cx);
                    });
                }
            },
        );
        let enter_focus = entry_focus.clone();
        let mut col = div().flex().flex_col().gap(px(8.)).child(
            div()
                .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                    if event.keystroke.key == "enter"
                        && !crate::ui::editor::is_composing(&enter_focus)
                    {
                        cx.stop_propagation();
                        this.ms_members_send(ms_members::Event::ReadEntry, cx);
                    }
                }))
                .child(field),
        );
        if entry.resolving {
            col = col.child(muted(
                theme,
                words::tv(loc, "members.resolving", &[("name", entry.typed.trim())]),
            ));
        } else if let Some(note) = &entry.note {
            col = col.child(error_line(theme, mw::entry_note(loc, note)));
        }
        if !entry.suggestions.is_empty() {
            let mut list = div().flex().flex_col();
            for (i, s) in entry.suggestions.iter().enumerate() {
                let short = words::short(&s.address);
                let name = s.name.clone().unwrap_or_else(|| short.clone());
                let pick = s.address.clone();
                list = list.child(
                    div()
                        .id(ElementId::from(("ms-set-suggestion", i)))
                        .a11y_button(format!("{name}, {short}"))
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .py(px(6.))
                        .px(px(6.))
                        .rounded(px(8.))
                        .cursor_pointer()
                        .hover(|el| el.bg(theme.bg_sunken))
                        .child(crate::wallet::components::identicon_avatar(
                            &mut self.identicons,
                            &s.address,
                            24.,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_size(theme::text_row_sub())
                                        .text_color(theme.fg_base)
                                        .child(name),
                                )
                                .child(
                                    div()
                                        .font_family(theme::font_mono())
                                        .text_size(theme::text_label())
                                        .text_color(theme.fg_subtle)
                                        .child(crate::ui::address_text(short)),
                                ),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.ms_members_send(
                                ms_members::Event::Pick {
                                    address: pick.clone(),
                                },
                                cx,
                            );
                        })),
                );
            }
            col = col
                .child(
                    div()
                        .text_size(theme::text_label())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_subtle)
                        .child(words::t(loc, "members.suggestions")),
                )
                .child(list);
        }
        let Some(resolved) = &entry.entry else {
            return col;
        };
        // The member is the address: shown in full.
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .p(px(10.))
                .rounded(px(10.))
                .bg(theme.bg_sunken)
                .child(crate::wallet::components::identicon_avatar(
                    &mut self.identicons,
                    &resolved.address,
                    24.,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .children(resolved.name.clone().map(|n| {
                            div()
                                .text_size(theme::text_row_sub())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.fg_base)
                                .child(n)
                        }))
                        .child(
                            div()
                                .font_family(theme::font_mono())
                                .text_size(theme::text_label())
                                .text_color(theme.fg_base)
                                .child(crate::ui::address_text(vela_core::multisig_link::eip55(
                                    &resolved.address,
                                ))),
                        ),
                ),
        );
        let name_strings = crate::ui::NameFieldStrings {
            label: words::t(loc, "members.memberName").into(),
            placeholder: words::t(loc, "members.memberNamePlaceholder").into(),
            helper: words::t(loc, "members.nameNote").into(),
            too_long_hint: SharedString::from(""),
        };
        col = col.child(crate::ui::text_field(
            "ms-set-member-name",
            theme,
            &name_strings,
            &entry.name,
            false,
            false,
            &name_focus,
            window,
            {
                let page = cx.entity();
                move |name: String, _: &mut Window, cx: &mut gpui::App| {
                    page.update(cx, |this: &mut Self, cx| {
                        this.ms_members_send(ms_members::Event::EntryName { name }, cx);
                    });
                }
            },
        ));
        if let Some(choice) = entry.choice {
            col = col.child(self.ms_set_kind(theme, loc, choice, cx));
        }
        col
    }

    /// r7: what kind of member it is (`MemberKindChoice`): found and shown,
    /// or the two choices with the hint and the note under them.
    fn ms_set_kind(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        choice: vela_core::app::multisig_signers::KindChoice,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut col = div().flex().flex_col().gap(px(6.));
        if choice.locked {
            let (what, note) = if choice.kind == Some(MemberKind::Contract) {
                ("members.kindContract", "members.kindContractNote")
            } else {
                ("members.kindVela", "members.kindKnown")
            };
            let mut known = div()
                .flex()
                .items_center()
                .gap(px(6.))
                .text_size(theme::text_row_sub())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_base);
            if choice.kind == Some(MemberKind::Vela) {
                known = known.child(icon_img(
                    &mut self.icons,
                    Icon::Check,
                    false,
                    theme.success,
                    14.,
                ));
            }
            return col
                .child(known.child(words::t(loc, what)))
                .child(muted(theme, words::t(loc, note)));
        }
        let mut kinds = div().flex().gap(px(8.));
        for (i, (kind, key)) in [
            (MemberKind::Vela, "members.kindVela"),
            (MemberKind::Wallet, "members.kindWallet"),
        ]
        .into_iter()
        .enumerate()
        {
            let on = choice.kind == Some(kind);
            let hinted = choice.suggested == Some(kind);
            let label = words::t(loc, key);
            kinds = kinds.child(
                div()
                    .id(ElementId::from(("ms-set-kind", i)))
                    .a11y_toggle(format!("{} · {label}", words::t(loc, "members.kind")), on)
                    .px(px(12.))
                    .py(px(6.))
                    .rounded(px(999.))
                    .border_1()
                    .border_color(if on {
                        theme.fg_base
                    } else if hinted {
                        theme.outline_strong
                    } else {
                        theme.divider
                    })
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .cursor_pointer()
                    .hover(|el| el.bg(theme.bg_sunken))
                    .child(label)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.ms_members_send(ms_members::Event::EntryKind { kind: Some(kind) }, cx);
                    })),
            );
        }
        col = col
            .child(
                div()
                    .text_size(theme::text_label())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_subtle)
                    .child(words::t(loc, "members.kind")),
            )
            .child(kinds);
        match choice.note {
            Some(KindNote::Likely) => col.child(muted(theme, words::t(loc, "members.kindLikely"))),
            Some(KindNote::Ask) => col.child(muted(theme, words::t(loc, "members.kindAsk"))),
            _ => col,
        }
    }

    /// The member changes waiting, one per action, each network's progress
    /// opening its transaction ("Arc 1/2 · Base 0/2").
    fn ms_set_waiting(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        view: &MultisigMembersView,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut list = div().flex().flex_col().gap(px(8.));
        let mut n = 0usize;
        for g in &view.groups {
            let title = mw::waiting_title(loc, &g.action, &|a: &str| self.ms_set_who(view, a, cx));
            let mut parts = div().flex().flex_wrap().gap(px(6.));
            for p in &g.parts {
                n += 1;
                let label = format!("{} {}/{}", network(p.chain_id), p.shown, p.threshold);
                let (chain_id, hash) = (p.chain_id, p.safe_tx_hash.clone());
                parts = parts.child(
                    small_button(
                        ElementId::from(("ms-set-waiting", n)),
                        theme,
                        label.clone(),
                        label,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_multisig_sheet(chain_id, hash.clone(), cx);
                    })),
                );
            }
            list = list.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(crate::ui::prose(title)),
                    )
                    .child(parts),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .text_size(theme::text_label())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_subtle)
                    .child(words::t(loc, "members.waitingTitle")),
            )
            .child(list)
    }

    /// Its record: not shared yet (on its way, or the older registry's —
    /// shared again with one passkey by a founding member here).
    fn ms_set_record(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        row: &ms_accounts::AccountRow,
        accounts: &ms_accounts::MultisigAccountsView,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        if row.shared {
            return None;
        }
        let key = row.account.address.to_ascii_lowercase();
        let mut col = div().flex().flex_col().gap(px(8.)).child(muted(
            theme,
            words::t(
                loc,
                if row.record_waiting {
                    "create.savedLater"
                } else {
                    "panel.notShared"
                },
            ),
        ));
        let founder = row.local_members.first().cloned();
        if let Some(owner) = founder {
            if !row.record_waiting {
                col = col.child(muted(theme, words::t(loc, "settings.shareAgainNote")));
            }
            let phase = accounts.share_again.get(&key).cloned().unwrap_or_default();
            let working = phase == ms_accounts::Phase::Working;
            let address = row.account.address.clone();
            col = col.child(
                div().flex().child(
                    crate::ui::vela_button_state(
                        "ms-set-share-again",
                        crate::ui::ButtonVariant::Secondary,
                        words::t(loc, "settings.shareAgain").into(),
                        if working {
                            crate::ui::ButtonState::Busy
                        } else {
                            crate::ui::ButtonState::from_enabled(true)
                        },
                        theme,
                        cx.listener(move |this, _, _, cx| {
                            let host = this.ms_host(cx);
                            let (address, owner) = (address.clone(), owner.clone());
                            host.update(cx, |host, cx| {
                                // The owner's passkey is this device's: the
                                // machines are told who is here first.
                                host.refresh_facts(cx);
                                host.accounts(
                                    ms_accounts::Event::ShareAgain { address, owner },
                                    cx,
                                );
                            });
                            cx.notify();
                        }),
                    )
                    .w_auto(),
                ),
            );
            if phase == ms_accounts::Phase::Failed {
                let said = match accounts.share_again_failure.get(&key) {
                    None => words::t(loc, "add.unavailable"),
                    Some(detail) => {
                        let chain = row.account.chain_ids.first().copied().unwrap_or(100);
                        self.ms_failure_words(detail, chain, cx)
                    }
                };
                col = col.child(error_line(theme, said));
            }
        }
        Some(col)
    }

    /// The shared name (the record's), and its rename — a transaction of
    /// the multisig itself on its record's chain, said where it cannot run.
    fn ms_set_rename(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        row: &ms_accounts::AccountRow,
        accounts: &ms_accounts::MultisigAccountsView,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let key = row.account.address.to_ascii_lowercase();
        let recorded = row.account.genesis.name.clone();
        let stand = mw::rename_stand(loc, accounts.rename.get(&key), &network);
        let mut col = div().flex().flex_col().gap(px(8.)).child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(heading(theme, words::t(loc, "settings.renameTitle")))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(recorded.clone().unwrap_or_else(|| "…".to_owned())),
                ),
        );
        let (chain_id, note) = match stand {
            mw::RenameStand::Asking => return col,
            mw::RenameStand::Blocked(text) | mw::RenameStand::NotDeployed(text) => {
                return col.child(muted(theme, text));
            }
            mw::RenameStand::Can { chain_id, note } => (chain_id, note),
        };
        let address = row.account.address.clone();
        match self.multisig.settings.renaming.clone() {
            None => {
                let start = recorded.clone().unwrap_or_default();
                col = col.child(
                    div().flex().child(
                        crate::ui::vela_button(
                            "ms-set-rename",
                            crate::ui::ButtonVariant::Secondary,
                            words::t(loc, "settings.renameAction").into(),
                            theme,
                            cx.listener(move |this, _, window, cx| {
                                this.multisig.settings.renaming = Some(start.clone());
                                this.multisig.settings.rename_failure = None;
                                let (rename, _, _) = this.ms_settings_focus(cx);
                                window.focus(&rename, cx);
                                cx.notify();
                            }),
                        )
                        .w_auto(),
                    ),
                );
            }
            Some(typed) => {
                let (rename_focus, _, _) = self.ms_settings_focus(cx);
                let clean = typed.trim().to_owned();
                let valid = !clean.is_empty() && vela_core::multisig::valid_name(&clean);
                let can = valid
                    && Some(&clean) != recorded.as_ref()
                    && !self.multisig.settings.rename_busy;
                let strings = crate::ui::NameFieldStrings {
                    label: words::t(loc, "create.name").into(),
                    placeholder: words::t(loc, "create.namePlaceholder").into(),
                    helper: SharedString::from(""),
                    too_long_hint: words::t(loc, "create.nameInvalid").into(),
                };
                let field = crate::ui::text_field(
                    "ms-set-rename-field",
                    theme,
                    &strings,
                    &typed,
                    !clean.is_empty() && !valid,
                    false,
                    &rename_focus,
                    window,
                    {
                        let page = cx.entity();
                        move |text: String, _: &mut Window, cx: &mut gpui::App| {
                            page.update(cx, |this: &mut Self, cx| {
                                if this.multisig.settings.renaming.is_some() {
                                    this.multisig.settings.renaming = Some(text);
                                }
                                cx.notify();
                            });
                        }
                    },
                );
                let enter = (rename_focus.clone(), address.clone());
                col = col.child(
                    div()
                        .on_key_down(cx.listener(move |this, event: &gpui::KeyDownEvent, _, cx| {
                            if event.keystroke.key == "enter"
                                && !crate::ui::editor::is_composing(&enter.0)
                            {
                                cx.stop_propagation();
                                this.ms_propose_rename(&enter.1, chain_id, cx);
                            }
                        }))
                        .child(field),
                );
                let state = if self.multisig.settings.rename_busy {
                    crate::ui::ButtonState::Busy
                } else {
                    crate::ui::ButtonState::from_enabled(can)
                };
                let go = address.clone();
                col = col.child(
                    crate::ui::dialog::dialog_actions()
                        .child(
                            crate::ui::vela_button_state(
                                "ms-set-rename-cancel",
                                crate::ui::ButtonVariant::Secondary,
                                self.strings.sign_out_cancel.clone(),
                                crate::ui::ButtonState::from_enabled(
                                    !self.multisig.settings.rename_busy,
                                ),
                                theme,
                                cx.listener(|this, _, _, cx| {
                                    this.multisig.settings.renaming = None;
                                    cx.notify();
                                }),
                            )
                            .w_auto(),
                        )
                        .child(
                            crate::ui::vela_button_state(
                                "ms-set-rename-propose",
                                crate::ui::ButtonVariant::Value,
                                words::t(loc, "send.propose").into(),
                                state,
                                theme,
                                cx.listener(move |this, _, _, cx| {
                                    this.ms_propose_rename(&go, chain_id, cx)
                                }),
                            )
                            .w_auto(),
                        ),
                );
                if let Some(failure) = self.multisig.settings.rename_failure.clone() {
                    col = col.child(error_line(theme, failure));
                }
            }
        }
        col.child(muted(theme, note))
    }

    /// Its setup on Ethereum: whether it is there, and saving it (an account
    /// here pays the fee).
    fn ms_set_backup(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        row: &ms_accounts::AccountRow,
        accounts: &ms_accounts::MultisigAccountsView,
        cx: &mut Context<Self>,
    ) -> Div {
        let key = row.account.address.to_ascii_lowercase();
        let state = accounts.backup.get(&key).cloned();
        let line = mw::backup_line(loc, state.as_ref()).unwrap_or_else(|| "…".to_owned());
        let done = matches!(state, Some(BackupState::BackedUp | BackupState::Sent));
        let actionable = matches!(
            state,
            Some(BackupState::NotBackedUp | BackupState::Failed | BackupState::CouldNotCheck)
        );
        let address = row.account.address.clone();
        let title = words::t(loc, "settings.backupTitle");
        let mut el = div()
            .id("ms-set-backup")
            .a11y_button(format!("{title}, {line}"))
            .flex()
            .items_center()
            .gap(px(12.))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(heading(theme, title))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(4.))
                            .text_size(theme::text_row_sub())
                            .text_color(if done { theme.success } else { theme.fg_muted })
                            .when(done, |el| {
                                el.child(icon_img(
                                    &mut self.icons,
                                    Icon::Check,
                                    false,
                                    theme.success,
                                    14.,
                                ))
                            })
                            .child(line),
                    ),
            );
        if actionable {
            el = el
                .cursor_pointer()
                .child(icon_img(
                    &mut self.icons,
                    Icon::ChevronRight,
                    false,
                    theme.fg_subtle,
                    16.,
                ))
                .on_click(cx.listener(move |this, _, _, cx| {
                    let host = this.ms_host(cx);
                    let couldnt = matches!(
                        host.read(cx)
                            .machines()
                            .accounts_view
                            .backup
                            .get(&address.to_ascii_lowercase()),
                        Some(BackupState::CouldNotCheck)
                    );
                    let payer = crate::session::view(cx).address;
                    let address = address.clone();
                    host.update(cx, |host, cx| {
                        if couldnt || payer.is_empty() {
                            host.accounts(ms_accounts::Event::CheckBackup { address }, cx);
                        } else {
                            host.accounts(
                                ms_accounts::Event::SaveToEthereum {
                                    address,
                                    from: payer,
                                },
                                cx,
                            );
                        }
                    });
                    cx.notify();
                }));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(el)
            .child(muted(theme, words::t(loc, "settings.backupNote")))
    }

    /// Taking it off this device, after a confirm (its funds are not affected).
    fn ms_set_remove(
        &mut self,
        theme: &Theme,
        loc: &Loc,
        row: &ms_accounts::AccountRow,
        cx: &mut Context<Self>,
    ) -> Div {
        let remove = words::t(loc, "settings.remove");
        if !self.multisig.settings.removing {
            return div().flex().child(
                crate::ui::vela_button(
                    "ms-set-remove",
                    crate::ui::ButtonVariant::Secondary,
                    remove.into(),
                    theme,
                    cx.listener(|this, _, _, cx| {
                        this.multisig.settings.removing = true;
                        cx.notify();
                    }),
                )
                .w_auto(),
            );
        }
        let address = row.account.address.clone();
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(muted(theme, words::t(loc, "switcher.removeBody")))
            .child(
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        crate::flows::components::danger_button(theme, remove.into())
                            .id("ms-set-remove-confirm")
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.multisig.settings.removing = false;
                                let host = this.ms_host(cx);
                                let address = address.clone();
                                host.update(cx, |host, cx| {
                                    host.accounts(ms_accounts::Event::Remove { address }, cx);
                                });
                                cx.notify();
                            })),
                    )
                    .child(
                        crate::flows::components::ghost_button(
                            theme,
                            self.strings.sign_out_cancel.clone(),
                        )
                        .rounded(px(12.))
                        .id("ms-set-remove-cancel")
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.multisig.settings.removing = false;
                            cx.notify();
                        })),
                    ),
            )
    }

    // -- proposing ----------------------------------------------------------

    /// Rename the shared name: a proposal of the multisig itself on its
    /// record's chain, through the review-first path; its sheet opens once
    /// it is in the mailbox.
    fn ms_propose_rename(&mut self, address: &str, chain_id: u64, cx: &mut Context<Self>) {
        let s = &self.multisig.settings;
        let Some(name) = s.renaming.as_deref().map(str::trim).map(str::to_owned) else {
            return;
        };
        if s.rename_busy || name.is_empty() || !vela_core::multisig::valid_name(&name) {
            return;
        }
        let recorded = self
            .shown_multisig(cx)
            .and_then(|r| r.account.genesis.name.clone());
        if recorded.as_deref() == Some(name.as_str()) {
            return;
        }
        self.multisig.settings.rename_busy = true;
        self.multisig.settings.rename_failure = None;
        let host = self.ms_host(cx);
        let address = address.to_owned();
        let answer = host.update(cx, |host, cx| {
            host.propose(
                chain_id,
                &address,
                ms_proposals::ProposalKind::Rename { name },
                cx,
            )
        });
        cx.notify();
        cx.spawn(async move |this, cx| {
            let end = answer.await.unwrap_or_else(|_| Proposed::withdrawn());
            this.update(cx, |this, cx| {
                this.multisig.settings.rename_busy = false;
                match end {
                    Proposed::Done { safe_tx_hash } => {
                        this.multisig.settings.renaming = None;
                        this.open_multisig_sheet(chain_id, safe_tx_hash, cx);
                    }
                    ended if ended.declined() => {}
                    Proposed::Ended { state, .. } => {
                        this.multisig.settings.rename_failure =
                            state.map(|s| this.ms_rename_failure(&s, chain_id, cx));
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Why a rename did not go, in the web's words for it.
    fn ms_rename_failure(&self, state: &ActionState, chain_id: u64, cx: &App) -> String {
        let loc = &self.loc;
        let net = network(chain_id);
        match state {
            ActionState::MailboxUnreadable => words::t(loc, "proposal.mailboxDown"),
            ActionState::SigningRefused { .. } => {
                words::tv(loc, "authority.refusedOn", &[("network", &net)])
            }
            ActionState::SigningUnchecked => {
                words::tv(loc, "authority.signUnchecked", &[("network", &net)])
            }
            ActionState::SigningBlocked => words::t(loc, "review.blocked"),
            other => self
                .ms_action_words(other, chain_id, cx)
                .unwrap_or_default(),
        }
    }

    /// Propose the change composed (the web's `proposeAll`): the members
    /// machine keeps the new member's name and kind, then each network's
    /// proposal goes, one at a time, through the review-first path — its
    /// review, the person's word, one passkey. A network that does not go
    /// is said ("Not proposed on Base: …"); a review closed or a passkey
    /// dismissed stops it, with nothing said wrong.
    fn ms_propose_change(&mut self, address: &str, cx: &mut Context<Self>) {
        if self.multisig.settings.busy() {
            return;
        }
        let host = self.ms_host(cx);
        let view = host.read(cx).machines().members_view.clone();
        let Some(plan) = view.plan.filter(|_| view.can_propose) else {
            return;
        };
        // The networks as they stood when the person pressed: once one is
        // proposed, the plan reads it as a change waiting there.
        let ready: Vec<(u64, ms_proposals::ProposalKind)> = plan
            .networks
            .iter()
            .filter_map(|n| n.proposal.clone().map(|p| (n.chain_id, p)))
            .collect();
        if ready.is_empty() {
            return;
        }
        host.update(cx, |host, cx| {
            host.members(ms_members::Event::Proposing, cx)
        });
        self.multisig.settings.progress =
            ready.iter().map(|(c, _)| (*c, Progress::Waiting)).collect();
        self.multisig.settings.failure = None;
        cx.notify();
        let address = address.to_owned();
        cx.spawn(async move |this, cx| {
            let total = ready.len();
            let mut made: Vec<(u64, String)> = Vec::new();
            for (chain_id, proposal) in ready {
                let address = address.clone();
                let Ok(answer) = this.update(cx, |this, cx| {
                    this.ms_set_progress(chain_id, Progress::Proposing);
                    cx.notify();
                    let host = this.ms_host(cx);
                    host.update(cx, |host, cx| {
                        host.propose(chain_id, &address, proposal, cx)
                    })
                }) else {
                    return;
                };
                let end = answer.await.unwrap_or_else(|_| Proposed::withdrawn());
                let stop = this
                    .update(cx, |this, cx| {
                        cx.notify();
                        match end {
                            Proposed::Done { safe_tx_hash } => {
                                this.ms_set_progress(chain_id, Progress::Proposed);
                                made.push((chain_id, safe_tx_hash));
                                false
                            }
                            ended => {
                                this.ms_set_progress(chain_id, Progress::Failed);
                                if ended.declined() {
                                    return true;
                                }
                                if let Proposed::Ended {
                                    state: Some(state), ..
                                } = &ended
                                {
                                    let reason = this.ms_change_failure(state, chain_id, cx);
                                    let net = network(chain_id);
                                    this.multisig.settings.failure = Some(words::tv(
                                        &this.loc,
                                        "members.failedOn",
                                        &[("network", &net), ("reason", &reason)],
                                    ));
                                }
                                false
                            }
                        }
                    })
                    .unwrap_or(true);
                if stop {
                    break;
                }
            }
            this.update(cx, |this, cx| {
                if !made.is_empty() && made.len() == total {
                    this.multisig.settings.progress.clear();
                    this.ms_members_send(ms_members::Event::Compose { compose: None }, cx);
                    // One network: straight to its proposal, as a transfer's
                    // confirm does.
                    if let [(chain_id, hash)] = made.as_slice() {
                        this.open_multisig_sheet(*chain_id, hash.clone(), cx);
                    }
                } else {
                    // Not every network went: the plan stays, the person
                    // can try again (a proposed one now reads as waiting).
                    this.multisig
                        .settings
                        .progress
                        .retain(|(_, p)| *p == Progress::Proposed);
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn ms_set_progress(&mut self, chain_id: u64, progress: Progress) {
        for (c, p) in &mut self.multisig.settings.progress {
            if *c == chain_id {
                *p = progress;
            }
        }
    }

    /// Why one network's member change did not go (`proposeChange`'s words).
    fn ms_change_failure(&self, state: &ActionState, chain_id: u64, cx: &App) -> String {
        match state {
            ActionState::Refused { reason } => mw::refusal_of(reason)
                .map_or_else(|| reason.clone(), |r| mw::refusal_text(&self.loc, &r)),
            ActionState::MailboxUnreadable => words::t(&self.loc, "proposal.mailboxDown"),
            other => self
                .ms_action_words(other, chain_id, cx)
                .unwrap_or_default(),
        }
    }
}

/// §3.5's notice: a title, what it means, and each line (`AuthorityNotice`).
fn authority_notice(theme: &Theme, title: String, body: String, lines: Vec<String>) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .p(px(14.))
        .rounded(px(12.))
        .border_1()
        .border_color(theme.error_base.opacity(0.3))
        .bg(theme.error_soft)
        .text_color(theme.error_base)
        .child(
            div()
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(
            div()
                .text_size(theme::text_row_sub())
                .line_height(gpui::relative(1.4))
                .child(crate::ui::prose(body)),
        )
        .children(lines.into_iter().map(|l| {
            div()
                .text_size(theme::text_label())
                .child(crate::ui::prose(format!("• {l}")))
        }))
}

/// The contacts book, as the member entry suggests from it.
fn member_contacts(cx: &mut App) -> Vec<ms_members::MemberContact> {
    use vela_core::app::contacts::{ContactKind, Contacts};
    crate::resident::resident::<Contacts>(cx)
        .read(cx)
        .view()
        .contacts
        .into_iter()
        .map(|c| ms_members::MemberContact {
            address: c.address,
            name: c.name,
            resolved_name: c.resolved_name,
            kind: Some(
                match c.kind {
                    ContactKind::Eoa => "eoa",
                    ContactKind::Account => "account",
                    ContactKind::Unknown => "unknown",
                }
                .to_owned(),
            ),
        })
        .collect()
}
