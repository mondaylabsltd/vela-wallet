//! The add sheet (AUDIT M1; the web's `AddMultisigSheet.svelte`): a
//! multisig added by its address, an ENS name, or an invite (`#msa=`) —
//! the invite's networks and members said before anything is added, adding
//! the person's press — or imported from a Safe{Wallet} export, each
//! member's kind declared first.
//!
//! The accounts machine does it all (`add`, `resolve_ens`, `open_link`,
//! `keep_invited`, `parse_export`, `read_kinds` / `pick_kind`, `import`):
//! who can act for a multisig is the chain's or the core's rule (§3.5) — a
//! refused genesis is said with every reason and nothing adds it; a high
//! risk is said in full and added only on the person's explicit press.

use std::collections::VecDeque;

use serde_json::json;
use vela_core::app::multisig_accounts::{
    self as ms_accounts, AddOutcome, ImportOutcome, InvitedMember, KindAsk, Phase,
};
use vela_core::app::multisig_members::{MemberProblem, member_problem};
use vela_core::app::multisig_shell::AccountUpdate;
use vela_core::multisig::ExportedSafe;
use vela_core::multisig_link::{self as link, AddLink};

use super::multisig::{Tone as MsTone, ms_label, ms_note, network, same};
use super::multisig_create::{ens_answer, ens_words, last_used, likely_networks};
use super::*;
use crate::multisig::{findings, words};

/// What the add field holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AddField {
    Empty,
    /// A multisig link (`#msa=` or `#ms=`, as a URL or a bare hash): the
    /// machine's one link flow reads it.
    Link(String),
    Ens(String),
    Address(String),
}

/// Read the add field: a link is the machine's, a `.eth` name is looked up,
/// anything else is meant as an address (the core's address rule says
/// whether it is one).
pub(crate) fn read_field(text: &str) -> AddField {
    let value = text.trim();
    if value.is_empty() {
        return AddField::Empty;
    }
    let hash = value.find('#').map(|at| &value[at..]);
    if let Some(hash) = hash.filter(|h| link::is_multisig_link(h)) {
        return AddField::Link(hash.to_owned());
    }
    if ms_accounts::looks_like_ens(value) {
        return AddField::Ens(value.to_owned());
    }
    AddField::Address(value.to_owned())
}

/// What an add came to, said on the sheet.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AddSaid {
    /// A line (a failure, a damaged invite).
    Line(String),
    Refused(Vec<serde_json::Value>),
    HighRisk(Vec<serde_json::Value>),
}

/// What the person typed and picked on the add sheet (the field's text is
/// `MultisigUi::add`). The outcomes are the machine's view.
pub(crate) struct AddForm {
    pub focus: FocusHandle,
    /// The invite it was opened from: its networks, its members' labels.
    pub link: Option<AddLink>,
    pub said: Option<AddSaid>,
    /// r14-5: what is said is a read that did not come back
    /// (`AddOutcome::offers_retry`, the core's rule): Add says Try again.
    pub retry: bool,
    /// The address an `add` was sent for: its outcome is awaited.
    pub asked: Option<String>,
    /// Nothing says which network: the person picks one, then Add.
    pub choosing: bool,
    pub chosen: Option<u64>,
    pub all_networks: bool,
    /// The ENS name asked about.
    pub ens: Option<String>,
    // -- importing from Safe{Wallet} --
    /// An export was read: its Safes are listed.
    pub exported: bool,
    pub picked: Vec<String>,
    /// r7: the members' kinds are asked before anything is imported.
    pub asking: bool,
    pub file_failure: Option<String>,
    /// Imports waiting their turn (one at a time), with `accept_risks`.
    pub queue: VecDeque<(String, bool)>,
    /// The import out now, and whether its event went.
    pub current: Option<(String, bool)>,
    /// An import was started (the kinds are not asked again).
    pub started: bool,
}

impl AddForm {
    pub(crate) fn new(cx: &mut Context<WalletPage>) -> Self {
        Self {
            focus: cx.focus_handle(),
            link: None,
            said: None,
            retry: false,
            asked: None,
            choosing: false,
            chosen: None,
            all_networks: false,
            ens: None,
            exported: false,
            picked: Vec::new(),
            asking: false,
            file_failure: None,
            queue: VecDeque::new(),
            current: None,
            started: false,
        }
    }

    fn importing(&self) -> bool {
        self.current.is_some() || !self.queue.is_empty()
    }
}

/// The words for an add that did not add (`m.add[kind]`).
pub(crate) fn outcome_said(loc: &Loc, outcome: &AddOutcome) -> Option<AddSaid> {
    Some(match outcome {
        AddOutcome::Added { .. } | AddOutcome::ChooseNetwork => return None,
        AddOutcome::Refused { findings } => AddSaid::Refused(findings.clone()),
        AddOutcome::HighRisk { findings } => AddSaid::HighRisk(findings.clone()),
        AddOutcome::Unverified { chain_id } => AddSaid::Line(words::tv(
            loc,
            "authority.unverified",
            &[("network", &network(*chain_id))],
        )),
        AddOutcome::NotFound => AddSaid::Line(words::t(loc, "add.notFound")),
        AddOutcome::Unavailable => AddSaid::Line(words::t(loc, "add.unavailable")),
        AddOutcome::Unsupported => AddSaid::Line(words::t(loc, "add.unsupported")),
        AddOutcome::NotMultisig => AddSaid::Line(words::t(loc, "add.notMultisig")),
        AddOutcome::Unchecked => AddSaid::Line(words::t(loc, "add.unchecked")),
        // r14-5: the network's nodes were rate-limiting — named, never
        // "couldn't reach the service".
        AddOutcome::Busy { chain_id } => AddSaid::Line(words::tv(
            loc,
            "add.busy",
            &[("network", &network(*chain_id))],
        )),
    })
}

/// r14-5: Add reads Try again while the line under the field is a read
/// that did not come back.
pub(crate) fn add_says_try_again(retry: bool, said: Option<&AddSaid>) -> bool {
    retry && matches!(said, Some(AddSaid::Line(_)))
}

/// Where the add the sheet sent stands, read from the machine (r14-5).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum AddAwait {
    /// Nothing is awaited.
    Idle,
    /// The machine is still adding (Add spins).
    Waiting,
    /// The machine finished: its outcome (`None` only from an older core).
    Answered(Option<AddOutcome>),
}

/// What the sheet waits on: the add it sent (`asked`), or — on a sheet
/// opened afresh, whose own note of it is gone (the page built again, or an
/// invite's sheet opened again by its network arriving) — the add the
/// machine is still running for the address in the field. Its outcome is
/// then said there, never dropped: the press that "spun and went back to
/// Add with no message" (r14-5) was an add whose sheet had been opened again
/// under it. A finished add from before is not said on a fresh sheet.
pub(crate) fn add_await(asked: Option<&str>, typed: &str, view: &ms_accounts::AddView) -> AddAwait {
    let running = view.phase == Phase::Working;
    let for_typed = view
        .address
        .as_deref()
        .is_some_and(|a| same(a, typed.trim()));
    let Some(asked) = asked.or((running && for_typed).then_some(typed)) else {
        return AddAwait::Idle;
    };
    if running
        || !view
            .address
            .as_deref()
            .is_some_and(|a| same(a, asked.trim()))
    {
        return AddAwait::Waiting;
    }
    if view.phase == Phase::Done {
        AddAwait::Answered(view.outcome.clone())
    } else {
        AddAwait::Waiting
    }
}

/// One exported Safe's import, in words (`None`: nothing to say, or said in
/// full below the row).
fn import_words(loc: &Loc, outcome: &ImportOutcome) -> Option<String> {
    Some(match outcome {
        ImportOutcome::Imported => words::t(loc, "import.imported"),
        ImportOutcome::NotFound => words::t(loc, "add.notFound"),
        ImportOutcome::Unavailable => words::t(loc, "add.unavailable"),
        ImportOutcome::Unsupported => words::t(loc, "add.unsupported"),
        ImportOutcome::NotDeployed => words::t(loc, "import.notDeployed"),
        ImportOutcome::Unverified { chain_id } => words::tv(
            loc,
            "authority.unverified",
            &[("network", &network(*chain_id))],
        ),
        ImportOutcome::Refused { .. } | ImportOutcome::HighRisk { .. } => return None,
    })
}

impl WalletPage {
    /// The add sheet opens, filled with `text` (an address, or a link).
    pub(crate) fn ms_add_opened(
        &mut self,
        text: Option<String>,
        link: Option<AddLink>,
        said: Option<AddSaid>,
        cx: &mut Context<Self>,
    ) {
        // Another link while one is open: a fresh sheet, never the last one's.
        self.multisig.add = Some(text.unwrap_or_default());
        let mut form = AddForm::new(cx);
        form.link = link;
        form.said = said;
        self.multisig.add_form = Some(form);
        self.multisig.scroll = crate::ui::SmoothScroll::new();
        cx.notify();
    }

    pub(crate) fn close_multisig_add(&mut self, cx: &mut Context<Self>) {
        super::multisig_links::Carry::take_add(cx);
        self.multisig.add = None;
        self.multisig.add_form = None;
        cx.notify();
    }

    /// Add, as the field reads: a link to the machine's link flow, a name
    /// looked up, an address added (on the network the invite names, or the
    /// one picked when asked).
    pub(crate) fn ms_add_press(&mut self, accept_risks: bool, cx: &mut Context<Self>) {
        let loc = self.ms_loc();
        let text = self.multisig.add.clone().unwrap_or_default();
        let networks = self.ms_device_networks(cx);
        let Some(form) = self.multisig.add_form.as_mut() else {
            return;
        };
        form.retry = false;
        let address = match read_field(&text) {
            AddField::Empty => return,
            AddField::Link(hash) => {
                let host = self.ms_host(cx);
                host.update(cx, |host, cx| {
                    host.accounts(
                        ms_accounts::Event::OpenLink {
                            hash,
                            take_kept: false,
                        },
                        cx,
                    );
                });
                return;
            }
            AddField::Ens(name) => {
                form.ens = Some(name.clone());
                form.said = None;
                let host = self.ms_host(cx);
                host.update(cx, |host, cx| {
                    host.accounts(ms_accounts::Event::ResolveEns { name }, cx)
                });
                cx.notify();
                return;
            }
            AddField::Address(address) => address,
        };
        // The core's one address rule (send-2), checksum included.
        match member_problem(&address, &[]) {
            Some(MemberProblem::Checksum) => {
                form.said = Some(AddSaid::Line(words::t(&loc, "send.invalidChecksum")));
                cx.notify();
                return;
            }
            Some(MemberProblem::InvalidAddress | MemberProblem::NotMember) => {
                form.said = Some(AddSaid::Line(words::t(&loc, "send.invalidAddress")));
                cx.notify();
                return;
            }
            _ => {}
        }
        if form.choosing && form.chosen.is_none() {
            return;
        }
        let from_link = form.link.as_ref().filter(|l| same(&l.multisig, &address));
        let link_has: Vec<u64> = from_link
            .map(|l| {
                l.chain_ids
                    .iter()
                    .copied()
                    .filter(|id| networks.contains(id))
                    .collect()
            })
            .unwrap_or_default();
        let named = if form.choosing {
            form.chosen
        } else {
            link_has.first().copied()
        };
        if !accept_risks || !matches!(form.said, Some(AddSaid::HighRisk(_))) {
            form.said = None;
        }
        form.asked = Some(address.clone());
        // Added, it is the multisig shown: the page is built again for it,
        // and what is awaited is carried over (`Carry`).
        super::multisig_links::Carry::set_add(address.clone(), form.link.clone(), cx);
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| {
            host.accounts(
                ms_accounts::Event::Add {
                    address,
                    network: named,
                    accept_risks,
                },
                cx,
            );
        });
        cx.notify();
    }

    /// The machine answered the add: said, or (added) the sheet goes — the
    /// machine shows it; an invite's labels are kept with it, and every
    /// network the invite names is one it is used on.
    fn ms_add_answered(
        &mut self,
        view: &ms_accounts::MultisigAccountsView,
        cx: &mut Context<Self>,
    ) {
        let loc = self.ms_loc();
        let networks = self.ms_device_networks(cx);
        let typed = self.multisig.add.clone().unwrap_or_default();
        let Some(form) = self.multisig.add_form.as_mut() else {
            return;
        };
        let outcome = match add_await(form.asked.as_deref(), &typed, &view.add) {
            AddAwait::Idle => return,
            AddAwait::Waiting => {
                // r14-5: a sheet opened afresh over an add still running
                // waits for it (and carries it, as a press does).
                if form.asked.is_none() {
                    let address = typed.trim().to_owned();
                    form.asked = Some(address.clone());
                    let link = form.link.clone();
                    super::multisig_links::Carry::set_add(address, link, cx);
                    cx.notify();
                }
                return;
            }
            AddAwait::Answered(outcome) => outcome,
        };
        form.asked = None;
        super::multisig_links::Carry::take_add(cx);
        let Some(outcome) = outcome else {
            // An older core's finished add with nothing to say: Add again,
            // never a spinner for ever.
            cx.notify();
            return;
        };
        match &outcome {
            AddOutcome::Added { address } => {
                let invite = form.link.clone().filter(|l| same(&l.multisig, address));
                let address = address.clone();
                let host = self.ms_host(cx);
                cx.defer(move |cx| {
                    host.update(cx, |host, cx| {
                        let Some(invite) = invite else {
                            return;
                        };
                        for chain_id in invite
                            .chain_ids
                            .iter()
                            .copied()
                            .filter(|id| networks.contains(id))
                        {
                            host.accounts(
                                ms_accounts::Event::Apply {
                                    address: address.clone(),
                                    update: AccountUpdate::UseChain { chain_id },
                                },
                                cx,
                            );
                        }
                        let members: Vec<InvitedMember> = invite
                            .members
                            .unwrap_or_default()
                            .into_iter()
                            .map(|m| InvitedMember {
                                address: m.address,
                                name: m.name,
                                kind: m
                                    .kind
                                    .and_then(|k| serde_json::to_value(k).ok())
                                    .and_then(|v| v.as_str().map(str::to_owned)),
                            })
                            .collect();
                        if !members.is_empty() {
                            host.accounts(
                                ms_accounts::Event::KeepInvited {
                                    address,
                                    chain_ids: invite.chain_ids,
                                    members,
                                },
                                cx,
                            );
                        }
                    });
                });
                self.close_multisig_add(cx);
            }
            AddOutcome::ChooseNetwork => {
                form.choosing = true;
                cx.notify();
            }
            other => {
                form.said = outcome_said(&loc, other);
                form.retry = other.offers_retry();
                cx.notify();
            }
        }
    }

    /// Every network this device has.
    fn ms_device_networks(&self, cx: &mut Context<Self>) -> Vec<u64> {
        let host = self.ms_host(cx);
        host.read(cx).machines().current_facts().networks.clone()
    }

    /// The add sheet: the machine's `add` and `import` views, the field.
    pub(crate) fn ms_add_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.multisig.add_form.is_none() {
            let text = self.multisig.add.clone();
            self.ms_add_opened(text, None, None, cx);
        }
        let loc = self.ms_loc();
        let host = self.ms_host(cx);
        let view = host.read(cx).machines().accounts_view.clone();
        self.ms_add_answered(&view, cx);
        self.ms_import_pump(&view, cx);
        // Closed by the answer (added).
        let form = self.multisig.add_form.as_ref()?;
        let body = if form.exported && form.asking {
            self.ms_import_kinds(theme, &view, cx)
        } else if form.exported {
            self.ms_import_list(theme, &view, cx)
        } else {
            self.ms_add_main(theme, window, &view, cx)
        };
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        Some(
            crate::ui::dialog::dialog(
                "ms-add",
                theme,
                window,
                words::t(&loc, "add.title").into(),
                None,
                close_icon,
                body,
                &self.multisig.scroll,
                cx.listener(|this, _, _, cx| this.close_multisig_add(cx)),
            )
            .into_any_element(),
        )
    }

    fn ms_add_main(
        &mut self,
        theme: &Theme,
        window: &Window,
        view: &ms_accounts::MultisigAccountsView,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let text = self.multisig.add.clone().unwrap_or_default();
        let networks = self.ms_device_networks(cx);
        let session = session::view(cx);
        let Some(form) = self.multisig.add_form.as_ref() else {
            return div();
        };
        let focus = form.focus.clone();
        let said = form.said.clone();
        let retry = add_says_try_again(form.retry, said.as_ref());
        let asked = form.asked.is_some();
        let choosing = form.choosing;
        let chosen = form.chosen;
        let all = form.all_networks;
        let file_failure = form.file_failure.clone();
        let ens = ens_answer(&view.add, form.ens.as_deref()).cloned();
        let ens_busy = form.ens.is_some() && ens.is_none();
        let typed = text.trim().to_owned();
        let link = form.link.clone().filter(|l| same(&l.multisig, &typed));
        let link_has: Vec<u64> = link
            .as_ref()
            .map(|l| {
                l.chain_ids
                    .iter()
                    .copied()
                    .filter(|id| networks.contains(id))
                    .collect()
            })
            .unwrap_or_default();
        let link_lacks = link
            .as_ref()
            .filter(|l| !l.chain_ids.is_empty() && link_has.is_empty())
            .map(|l| l.chain_ids[0]);

        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(crate::ui::dialog::dialog_body(
                theme,
                words::t(&loc, "add.intro"),
            ));
        let strings = crate::ui::NameFieldStrings {
            label: SharedString::default(),
            placeholder: words::t(&loc, "add.placeholder").into(),
            helper: SharedString::default(),
            too_long_hint: SharedString::default(),
        };
        let page = cx.entity();
        body = body.child(
            div()
                // Enter adds.
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key == "enter" {
                        this.ms_add_press(false, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(crate::ui::text_field(
                    "ms-add-address",
                    theme,
                    &strings,
                    &text,
                    matches!(said, Some(AddSaid::Line(_) | AddSaid::Refused(_))),
                    false,
                    &focus,
                    window,
                    move |text: String, _: &mut Window, cx: &mut gpui::App| {
                        page.update(cx, |this: &mut Self, cx| {
                            this.multisig.add = Some(text);
                            if let Some(form) = this.multisig.add_form.as_mut() {
                                form.said = None;
                                form.choosing = false;
                                form.chosen = None;
                                form.ens = None;
                            }
                            cx.notify();
                        });
                    },
                )),
        );
        match &said {
            Some(AddSaid::Line(line)) => body = body.child(ms_note(theme, line, MsTone::Danger)),
            Some(AddSaid::Refused(found)) => {
                body = body.child(self.ms_authority(theme, found, true))
            }
            Some(AddSaid::HighRisk(found)) => {
                body = body.child(self.ms_authority(theme, found, false)).child(
                    crate::flows::components::danger_button(
                        theme,
                        words::t(&loc, "authority.riskConfirm").into(),
                    )
                    .id("ms-add-anyway")
                    .a11y_button(SharedString::from(words::t(&loc, "authority.riskConfirm")))
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| this.ms_add_press(true, cx))),
                );
            }
            None => {}
        }
        if let Some(answer) = &ens {
            let (line, found) = ens_words(&loc, answer);
            let mut card = div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .p(px(12.))
                .rounded(px(10.))
                .bg(theme.bg_sunken)
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(crate::ui::prose(line)),
                );
            if let Some(address) = found {
                card = card
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_size(theme::text_row_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(crate::ui::address_text(address.clone())),
                    )
                    .child(
                        crate::ui::vela_button(
                            "ms-add-ens",
                            crate::ui::ButtonVariant::Secondary,
                            words::t(&loc, "add.add").into(),
                            theme,
                            cx.listener(move |this, _, _, cx| {
                                // The name's address is what is added.
                                this.multisig.add = Some(address.clone());
                                if let Some(form) = this.multisig.add_form.as_mut() {
                                    form.ens = None;
                                }
                                this.ms_add_press(false, cx);
                            }),
                        )
                        .w_auto(),
                    );
            }
            body = body.child(card);
        }

        // What the invite says, before anything is added (r4D-4, r4D-5).
        if let Some(link) = &link {
            let mine = |member: &link::InvitedMember| -> Option<String> {
                let address = member.address.as_deref()?;
                session
                    .accounts
                    .iter()
                    .find(|r| same(&r.account.address, address))
                    .map(|r| crate::multisig::shown::name_or(&r.account.address, &r.account.name))
            };
            let named = link
                .members
                .as_ref()
                .is_some_and(|ms| ms.iter().any(|m| m.name.is_some() || mine(m).is_some()));
            if !link.chain_ids.is_empty() || named {
                let mut card = div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .p(px(12.))
                    .rounded(px(10.))
                    .bg(theme.bg_sunken);
                if !link_has.is_empty() {
                    let names: Vec<String> = link_has.iter().map(|id| network(*id)).collect();
                    card = card.child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_base)
                            .child(words::tv(
                                &loc,
                                "add.usedOn",
                                &[("networks", &loc.list(&names))],
                            )),
                    );
                } else if let Some(lacks) = link_lacks {
                    // r13-6: one sentence — the network NAMED inside the
                    // core's `add.networkMissing` (it says the rest itself).
                    let title = network(lacks);
                    let hash = link::add_link(
                        link::SITE_ORIGIN,
                        self.loc.language(),
                        &link.multisig,
                        &link.chain_ids,
                        link.members.as_deref().unwrap_or_default(),
                    )
                    .ok()
                    .and_then(|url| url.split_once('#').map(|(_, h)| format!("#{h}")));
                    card = card
                        .child(
                            div()
                                .text_size(theme::text_row_sub())
                                .text_color(theme.fg_base)
                                .child(crate::ui::prose(words::tv(
                                    &loc,
                                    "add.networkMissing",
                                    &[("network", &title)],
                                ))),
                        )
                        .child(
                            crate::ui::vela_button(
                                "ms-add-network",
                                crate::ui::ButtonVariant::Secondary,
                                words::t(&loc, "proposal.addNetwork").into(),
                                theme,
                                cx.listener(move |this, _, _, cx| {
                                    // The invite opens again once the network is here.
                                    this.ms_wait_for_network(lacks, hash.clone(), cx);
                                    this.close_multisig_add(cx);
                                    this.ms_open_add_network(lacks, cx);
                                }),
                            )
                            .w_auto(),
                        );
                }
                // r8-12, r9-11: every member the invite names, with its kind —
                // one with no label by its address, never left out.
                if link.members.as_ref().is_some_and(|ms| !ms.is_empty()) {
                    card = card.child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_base)
                            .child(words::t(&loc, "add.invitedMembers")),
                    );
                    for (i, member) in link.members.iter().flatten().enumerate() {
                        // r5reg-24: the one of them that is this device's own account, said.
                        let you = mine(member);
                        let Some(who) = member
                            .name
                            .clone()
                            .or_else(|| you.clone())
                            .or_else(|| member.address.as_deref().map(words::short))
                        else {
                            continue;
                        };
                        let kind = member.kind.map(|k| match k {
                            link::MemberKind::Passkey => words::t(&loc, "add.kindPasskey"),
                            link::MemberKind::Contract => words::t(&loc, "members.kindContract"),
                            link::MemberKind::Wallet => words::t(&loc, "add.kindWallet"),
                        });
                        card = card.child(
                            div()
                                .id(ElementId::from(("ms-add-invited", i)))
                                .flex()
                                .flex_wrap()
                                .gap(px(8.))
                                .text_size(theme::text_row_sub())
                                .child(div().text_color(theme.fg_base).child(who))
                                .when(you.is_some(), |el| {
                                    el.child(
                                        div()
                                            .text_color(theme.fg_muted)
                                            .child(words::t(&loc, "add.thatsYou")),
                                    )
                                })
                                .children(kind.map(|k| div().text_color(theme.fg_subtle).child(k))),
                        );
                    }
                    card = card.child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(crate::ui::prose(words::t(&loc, "add.labelsNote"))),
                    );
                }
                body = body.child(card);
            }
        }

        // Nothing says which network: the person picks, then Add (r4b-8).
        if choosing {
            let hints = json!({
                "named": link_has,
                "used": last_used(&view.accounts),
            });
            let space: Vec<u64> = crate::network_space::space_chain_ids()
                .into_iter()
                .map(u64::from)
                .collect();
            let offered = likely_networks(&space, &hints);
            body = body
                .child(ms_note(
                    theme,
                    &words::t(&loc, "add.chooseNetwork"),
                    MsTone::Neutral,
                ))
                .child(self.ms_network_choice(
                    theme,
                    "ms-add-net",
                    &offered,
                    chosen,
                    all,
                    !asked,
                    cx,
                    |this, id, cx| {
                        if let Some(form) = this.multisig.add_form.as_mut() {
                            form.chosen = Some(id);
                        }
                        cx.notify();
                    },
                    |this, cx| {
                        if let Some(form) = this.multisig.add_form.as_mut() {
                            form.all_networks = true;
                        }
                        cx.notify();
                    },
                ));
        }
        if !matches!(said, Some(AddSaid::HighRisk(_))) {
            let refused = matches!(said, Some(AddSaid::Refused(_)));
            let state = if asked || ens_busy {
                crate::ui::ButtonState::Busy
            } else {
                crate::ui::ButtonState::from_enabled(
                    !typed.is_empty() && !refused && !(choosing && chosen.is_none()),
                )
            };
            body = body.child(
                crate::ui::dialog::dialog_actions().child(
                    crate::ui::vela_button_state(
                        "ms-add-add",
                        crate::ui::ButtonVariant::Secondary,
                        if retry {
                            loc.t("common.tryAgain").to_string().into()
                        } else {
                            words::t(&loc, "add.add").into()
                        },
                        state,
                        theme,
                        cx.listener(|this, _, _, cx| this.ms_add_press(false, cx)),
                    )
                    .w_auto(),
                ),
            );
        }

        // Or importing from Safe{Wallet}: the file is read on this device only.
        body = body
            .child(div().h(px(1.)).bg(theme.divider).my(px(8.)))
            .child(ms_label(theme, &words::t(&loc, "import.or")))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(crate::ui::prose(words::t(&loc, "import.body"))),
            )
            .child(
                outline_button(
                    "ms-import-choose",
                    theme,
                    &mut self.icons,
                    Some(Icon::Upload),
                    words::t(&loc, "import.choose").into(),
                )
                .on_click(cx.listener(|this, _, _, cx| this.ms_import_pick(cx))),
            );
        if let Some(failure) = file_failure {
            body = body.child(ms_note(theme, &failure, MsTone::Danger));
        }
        body
    }

    /// §3.5: what the core found, in full — refused (nothing adds it), or a
    /// high risk (added only on the person's press).
    fn ms_authority(&self, theme: &Theme, found: &[serde_json::Value], refused: bool) -> Div {
        let loc = &self.loc;
        let (title, body) = if refused {
            ("authority.refusedTitle", "authority.refusedBody")
        } else {
            ("authority.riskTitle", "authority.riskBody")
        };
        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .p(px(12.))
            .rounded(px(10.))
            .bg(theme.error_soft)
            .child(
                div()
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.error_base)
                    .child(words::t(loc, title)),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(crate::ui::prose(words::t(loc, body))),
            );
        for line in findings::finding_lines(loc, found, "") {
            col = col.child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.error_base)
                    .child(crate::ui::prose(format!("• {line}"))),
            );
        }
        col
    }

    // -- importing from Safe{Wallet} --------------------------------------------

    /// The file picker; the file is read here and handed to the core.
    fn ms_import_pick(&mut self, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |page, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            page.update(cx, |this, cx| this.ms_import_file(path, cx))
                .ok();
        })
        .detach();
    }

    /// A Safe{Wallet} export (picked, or dropped on the open add sheet):
    /// read by the core, on this device (`parse_export`).
    pub(crate) fn ms_import_file(&mut self, path: std::path::PathBuf, cx: &mut Context<Self>) {
        let loc = self.ms_loc();
        // An export is small; anything past a few megabytes is no export.
        let text = std::fs::metadata(&path)
            .ok()
            .filter(|m| m.is_file() && m.len() <= 8 * 1024 * 1024)
            .and_then(|_| std::fs::read_to_string(&path).ok());
        let Some(json) = text else {
            if let Some(form) = self.multisig.add_form.as_mut() {
                form.file_failure = Some(words::t(&loc, "import.notExport"));
            }
            cx.notify();
            return;
        };
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| {
            host.accounts(ms_accounts::Event::ParseExport { json }, cx)
        });
        let view = host.read(cx).machines().accounts_view.clone();
        let Some(form) = self.multisig.add_form.as_mut() else {
            return;
        };
        form.file_failure = None;
        form.queue.clear();
        form.current = None;
        form.started = false;
        form.asking = false;
        if view.import.error.is_some() {
            form.file_failure = Some(words::t(&loc, "import.notExport"));
        } else if view.import.safes.is_empty() {
            form.file_failure = Some(words::t(&loc, "import.empty"));
        } else {
            // Everything new is picked to start with; one already here is shown as such.
            form.picked = exported(&view)
                .into_iter()
                .filter(|s| {
                    !view
                        .accounts
                        .iter()
                        .any(|r| same(&r.account.address, &s.address))
                })
                .map(|s| s.address)
                .collect();
            form.exported = true;
            self.multisig.scroll = crate::ui::SmoothScroll::new();
        }
        cx.notify();
    }

    /// r7: the picked Safes' members not on this device, with the networks
    /// each is a member on (where code at its address is looked for).
    fn ms_import_to_ask(
        &self,
        view: &ms_accounts::MultisigAccountsView,
        cx: &gpui::App,
    ) -> Vec<KindAsk> {
        let Some(form) = self.multisig.add_form.as_ref() else {
            return Vec::new();
        };
        let locals = session::view(cx);
        let mut out: Vec<KindAsk> = Vec::new();
        for safe in exported(view)
            .into_iter()
            .filter(|s| form.picked.iter().any(|p| same(p, &s.address)))
        {
            for chain in &safe.chains {
                for owner in &chain.owners {
                    if locals
                        .accounts
                        .iter()
                        .any(|r| same(&r.account.address, owner))
                    {
                        continue;
                    }
                    match out.iter_mut().find(|o| same(&o.address, owner)) {
                        Some(seen) if !seen.chain_ids.contains(&chain.chain_id) => {
                            seen.chain_ids.push(chain.chain_id)
                        }
                        Some(_) => {}
                        None => out.push(KindAsk {
                            address: owner.clone(),
                            chain_ids: vec![chain.chain_id],
                        }),
                    }
                }
            }
        }
        out
    }

    /// The picked Safes go in, one at a time (each is the machine's `import`).
    fn ms_import_start(
        &mut self,
        view: &ms_accounts::MultisigAccountsView,
        cx: &mut Context<Self>,
    ) {
        let Some(form) = self.multisig.add_form.as_mut() else {
            return;
        };
        form.asking = false;
        form.started = true;
        for address in form.picked.clone() {
            let done = matches!(
                view.import.outcomes.get(&address.to_ascii_lowercase()),
                Some(ImportOutcome::Imported)
            );
            if !done {
                form.queue.push_back((address, false));
            }
        }
        cx.notify();
    }

    /// The next import, once the last one ended (`import.busy` cleared).
    fn ms_import_pump(&mut self, view: &ms_accounts::MultisigAccountsView, cx: &mut Context<Self>) {
        let Some(form) = self.multisig.add_form.as_mut() else {
            return;
        };
        if let Some((_, sent)) = &form.current {
            if !*sent || view.import.busy.is_some() {
                return;
            }
            form.current = None;
        }
        let Some((address, accept_risks)) = form.queue.pop_front() else {
            return;
        };
        form.current = Some((address.clone(), false));
        let host = self.ms_host(cx);
        let page = cx.entity();
        cx.defer(move |cx| {
            host.update(cx, |host, cx| {
                host.accounts(
                    ms_accounts::Event::Import {
                        address,
                        accept_risks,
                    },
                    cx,
                );
            });
            page.update(cx, |this, cx| {
                if let Some((_, sent)) = this
                    .multisig
                    .add_form
                    .as_mut()
                    .and_then(|f| f.current.as_mut())
                {
                    *sent = true;
                }
                cx.notify();
            });
        });
    }

    fn ms_import_list(
        &mut self,
        theme: &Theme,
        view: &ms_accounts::MultisigAccountsView,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let to_ask = self.ms_import_to_ask(view, cx);
        let Some(form) = self.multisig.add_form.as_ref() else {
            return div();
        };
        let picked = form.picked.clone();
        let importing = form.importing();
        let started = form.started;
        let current = form.current.as_ref().map(|(a, _)| a.clone());
        let queued: Vec<String> = form.queue.iter().map(|(a, _)| a.clone()).collect();
        let safes = exported(view);
        let outcome = |address: &str| view.import.outcomes.get(&address.to_ascii_lowercase());
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(ms_label(theme, &words::t(&loc, "import.pick")));
        for (i, safe) in safes.iter().enumerate() {
            let address = safe.address.clone();
            let result = outcome(&address);
            let waiting = current.as_deref().is_some_and(|c| same(c, &address))
                || queued.iter().any(|q| same(q, &address));
            let here = view
                .accounts
                .iter()
                .any(|r| same(&r.account.address, &address))
                && result.is_none()
                && !waiting;
            let on = picked.iter().any(|p| same(p, &address));
            let done = matches!(result, Some(ImportOutcome::Imported));
            let rule = safe
                .chains
                .first()
                .map(|c| {
                    words::tv(
                        &loc,
                        "create.thresholdValue",
                        &[
                            ("m", &c.threshold.to_string()),
                            ("n", &c.owners.len().to_string()),
                        ],
                    )
                })
                .unwrap_or_default();
            let chains = safe
                .chains
                .iter()
                .map(|c| {
                    if c.deployed {
                        network(c.chain_id)
                    } else {
                        format!(
                            "{} ({})",
                            network(c.chain_id),
                            words::t(&loc, "import.undeployed")
                        )
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            let name = safe.name.clone().unwrap_or_else(|| words::short(&address));
            let state: Option<(String, bool)> = if here {
                Some((words::t(&loc, "import.already"), false))
            } else if waiting {
                Some((words::t(&loc, "import.importing"), false))
            } else {
                result.and_then(|r| {
                    import_words(&loc, r).map(|w| (w, !matches!(r, ImportOutcome::Imported)))
                })
            };
            let locked = here || importing || done;
            let target = address.clone();
            let mut row = div()
                .id(ElementId::from(("ms-import-safe", i)))
                .a11y_toggle(SharedString::from(name.clone()), on)
                .flex()
                .items_center()
                .gap(px(10.))
                .py(px(8.))
                .border_b_1()
                .border_color(theme.divider)
                .child(
                    div()
                        .size(px(20.))
                        .flex_none()
                        .rounded(px(6.))
                        .border_1()
                        .border_color(if on {
                            theme.fg_base
                        } else {
                            theme.border_strong
                        })
                        .flex()
                        .items_center()
                        .justify_center()
                        .when(on, |el| {
                            el.child(icon_img(
                                &mut self.icons,
                                Icon::Check,
                                false,
                                theme.fg_base,
                                14.,
                            ))
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .text_color(theme.fg_base)
                                .child(name),
                        )
                        .child(
                            div()
                                .text_size(theme::text_label())
                                .text_color(theme.fg_subtle)
                                .child(format!(
                                    "{} · {rule} · {chains}",
                                    crate::wallet::live::shorten_address(&address)
                                )),
                        ),
                )
                .children(state.map(|(text, bad)| {
                    div()
                        .flex_none()
                        .max_w(px(160.))
                        .text_size(theme::text_label())
                        .text_color(if bad {
                            theme.error_base
                        } else {
                            theme.fg_muted
                        })
                        .child(crate::ui::prose(text))
                }));
            if !locked {
                row = row
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(form) = this.multisig.add_form.as_mut() {
                            if form.picked.iter().any(|p| same(p, &target)) {
                                form.picked.retain(|p| !same(p, &target));
                            } else {
                                form.picked.push(target.clone());
                            }
                        }
                        cx.notify();
                    }));
            }
            body = body.child(row);
            match result {
                Some(ImportOutcome::Refused { findings }) if !waiting => {
                    body = body.child(self.ms_authority(theme, findings, true));
                }
                Some(ImportOutcome::HighRisk { findings }) if !waiting => {
                    let again = address.clone();
                    body = body.child(self.ms_authority(theme, findings, false)).child(
                        crate::flows::components::danger_button(
                            theme,
                            words::t(&loc, "authority.riskConfirm").into(),
                        )
                        .id(ElementId::from(("ms-import-anyway", i)))
                        .a11y_button(SharedString::from(words::t(&loc, "authority.riskConfirm")))
                        .when(!importing, |el| el.cursor_pointer())
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(form) =
                                this.multisig.add_form.as_mut().filter(|f| !f.importing())
                            {
                                form.queue.push_back((again.clone(), true));
                            }
                            cx.notify();
                        })),
                    );
                }
                _ => {}
            }
        }
        let finished = started
            && !importing
            && !picked.is_empty()
            && picked.iter().all(|p| outcome(p).is_some());
        let ask_first = !to_ask.is_empty() && !started;
        let action = if finished {
            crate::ui::vela_button(
                "ms-import-done",
                crate::ui::ButtonVariant::Secondary,
                words::t(&loc, "create.done").into(),
                theme,
                cx.listener(|this, _, _, cx| this.close_multisig_add(cx)),
            )
        } else {
            let label = if ask_first {
                words::t(&loc, "create.next")
            } else {
                words::tv(
                    &loc,
                    "import.importSome",
                    &[("total", &picked.len().to_string())],
                )
            };
            let state = if importing {
                crate::ui::ButtonState::Busy
            } else {
                crate::ui::ButtonState::from_enabled(!picked.is_empty())
            };
            crate::ui::vela_button_state(
                "ms-import-next",
                crate::ui::ButtonVariant::Secondary,
                label.into(),
                state,
                theme,
                cx.listener(move |this, _, _, cx| {
                    let host = this.ms_host(cx);
                    let view = host.read(cx).machines().accounts_view.clone();
                    if ask_first {
                        // r7: the members' kinds, asked before anything is imported.
                        let members = this.ms_import_to_ask(&view, cx);
                        host.update(cx, |host, cx| {
                            host.accounts(ms_accounts::Event::ReadKinds { members }, cx)
                        });
                        if let Some(form) = this.multisig.add_form.as_mut() {
                            form.asking = true;
                        }
                        cx.notify();
                    } else {
                        this.ms_import_start(&view, cx);
                    }
                }),
            )
        };
        body.child(crate::ui::dialog::dialog_actions().child(action.w_auto()))
    }

    /// r7: how each member approves, declared before the Safes are imported.
    fn ms_import_kinds(
        &mut self,
        theme: &Theme,
        view: &ms_accounts::MultisigAccountsView,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let to_ask = self.ms_import_to_ask(view, cx);
        let total = self
            .multisig
            .add_form
            .as_ref()
            .map_or(0, |f| f.picked.len());
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(ms_label(theme, &words::t(&loc, "import.kindsTitle")))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(crate::ui::prose(words::t(&loc, "import.kindsBody"))),
            );
        for (i, member) in to_ask.iter().enumerate() {
            let kept = view
                .known_members
                .get(&member.address.to_ascii_lowercase())
                .and_then(|m| m.name.clone());
            let label = vela_core::multisig_link::call_json(
                "names.label",
                &json!({"address": member.address, "kept": kept}).to_string(),
            )
            .ok()
            .and_then(|t| serde_json::from_str::<String>(&t).ok())
            .unwrap_or_else(|| words::short(&member.address));
            let short = super::multisig_create::names_short(&member.address);
            let mut row = div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .py(px(8.))
                .border_b_1()
                .border_color(theme.divider)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(crate::wallet::components::identicon_avatar(
                            &mut self.identicons,
                            &member.address,
                            24.,
                        ))
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .text_color(theme.fg_base)
                                .child(label.clone()),
                        )
                        .when(label != short, |el| {
                            el.child(
                                div()
                                    .text_size(theme::text_label())
                                    .text_color(theme.fg_subtle)
                                    .child(short.clone()),
                            )
                        }),
                );
            if let Some(choice) = view
                .member_kinds
                .get(&member.address.to_ascii_lowercase())
                .copied()
            {
                row = row.child(self.ms_kind_choice(
                    theme,
                    "ms-import-kind",
                    i,
                    &member.address,
                    &label,
                    choice,
                    cx,
                ));
            }
            body = body.child(row);
        }
        body.child(
            crate::ui::dialog::dialog_actions()
                .child(
                    crate::ui::vela_button(
                        "ms-import-back",
                        crate::ui::ButtonVariant::Secondary,
                        words::t(&loc, "create.back").into(),
                        theme,
                        cx.listener(|this, _, _, cx| {
                            if let Some(form) = this.multisig.add_form.as_mut() {
                                form.asking = false;
                            }
                            cx.notify();
                        }),
                    )
                    .w_auto(),
                )
                .child(
                    crate::ui::vela_button(
                        "ms-import-go",
                        crate::ui::ButtonVariant::Secondary,
                        words::tv(&loc, "import.importSome", &[("total", &total.to_string())])
                            .into(),
                        theme,
                        cx.listener(|this, _, _, cx| {
                            let host = this.ms_host(cx);
                            let view = host.read(cx).machines().accounts_view.clone();
                            this.ms_import_start(&view, cx);
                        }),
                    )
                    .w_auto(),
                ),
        )
    }
}

/// The export's Safes, as the core read them.
fn exported(view: &ms_accounts::MultisigAccountsView) -> Vec<ExportedSafe> {
    view.import
        .safes
        .iter()
        .filter_map(|v| serde_json::from_value(v.clone()).ok())
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const SAFE: &str = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83";

    #[test]
    fn the_field_reads_as_a_link_a_name_or_an_address() {
        assert_eq!(read_field("   "), AddField::Empty);
        assert_eq!(
            read_field(&format!(" {SAFE} ")),
            AddField::Address(SAFE.to_owned())
        );
        assert_eq!(read_field("team.eth"), AddField::Ens("team.eth".to_owned()));
        let invite = format!("https://wallet.getvela.app/en/wallet#msa=100:{SAFE}");
        assert_eq!(
            read_field(&invite),
            AddField::Link(format!("#msa=100:{SAFE}"))
        );
        assert_eq!(
            read_field(&format!("#msa=100:{SAFE}")),
            AddField::Link(format!("#msa=100:{SAFE}"))
        );
        // A damaged invite is still meant as one: the link flow says so.
        assert_eq!(
            read_field("#msa=100:0x88cc"),
            AddField::Link("#msa=100:0x88cc".to_owned())
        );
        assert_eq!(
            read_field("https://example.com/#top"),
            AddField::Address("https://example.com/#top".to_owned())
        );
    }

    #[test]
    fn an_add_that_did_not_add_is_said_in_the_corpus_words() {
        let loc = Loc::for_tag("en");
        assert_eq!(
            outcome_said(
                &loc,
                &AddOutcome::Added {
                    address: SAFE.into()
                }
            ),
            None
        );
        assert_eq!(outcome_said(&loc, &AddOutcome::ChooseNetwork), None);
        let Some(AddSaid::Line(line)) = outcome_said(&loc, &AddOutcome::NotFound) else {
            panic!("a line");
        };
        assert!(!line.contains("home.multisig"), "{line}");
        assert!(matches!(
            outcome_said(&loc, &AddOutcome::HighRisk { findings: vec![] }),
            Some(AddSaid::HighRisk(_))
        ));
        let Some(AddSaid::Line(line)) =
            outcome_said(&loc, &AddOutcome::Unverified { chain_id: 100 })
        else {
            panic!("a line");
        };
        assert!(!line.contains("{{network}}"), "{line}");
    }

    /// r14-5: a network rate-limiting the add is named busy, in the corpus
    /// words, and Add reads Try again under it.
    #[test]
    fn a_rate_limited_add_is_said_busy_with_try_again() {
        let loc = Loc::for_tag("en");
        let busy = AddOutcome::Busy {
            chain_id: 5_042_002,
        };
        let said = outcome_said(&loc, &busy);
        let Some(AddSaid::Line(line)) = &said else {
            panic!("a line: {said:?}");
        };
        assert!(!line.contains("home.multisig"), "{line}");
        assert!(!line.contains("{{network}}"), "{line}");
        assert!(line.contains("Arc Testnet"), "the network is named: {line}");
        assert!(line.contains("busy"), "the cause is named: {line}");
        assert!(busy.offers_retry());
        assert!(add_says_try_again(busy.offers_retry(), said.as_ref()));
        // A refusal is no read to try again.
        let refused = AddOutcome::Refused { findings: vec![] };
        assert!(!add_says_try_again(
            refused.offers_retry(),
            outcome_said(&loc, &refused).as_ref()
        ));
    }

    fn view(phase: Phase, address: &str, outcome: Option<AddOutcome>) -> ms_accounts::AddView {
        ms_accounts::AddView {
            phase,
            address: Some(address.to_owned()),
            outcome,
            ..ms_accounts::AddView::default()
        }
    }

    /// r14-5: the sheet follows the machine. An add still running is awaited
    /// even by a sheet opened afresh under it (an invite's sheet opened again
    /// when its network arrived, the page built again) — so its outcome is
    /// said, never dropped with Add back and no message.
    #[test]
    fn an_add_still_running_is_awaited_by_a_sheet_opened_again() {
        let busy = AddOutcome::Busy {
            chain_id: 5_042_002,
        };
        // A fresh sheet (nothing asked) over the machine still adding it.
        assert_eq!(
            add_await(None, SAFE, &view(Phase::Working, SAFE, None)),
            AddAwait::Waiting
        );
        // Its outcome lands there, though this sheet never pressed Add.
        assert_eq!(
            add_await(
                Some(SAFE),
                SAFE,
                &view(Phase::Done, SAFE, Some(busy.clone()))
            ),
            AddAwait::Answered(Some(busy.clone()))
        );
        // A finished add from before is not said on a fresh sheet.
        assert_eq!(
            add_await(None, SAFE, &view(Phase::Done, SAFE, Some(busy.clone()))),
            AddAwait::Idle
        );
        // Another address's add is not this sheet's.
        let other = "0x00000000000000000000000000000000000000cc";
        assert_eq!(
            add_await(None, SAFE, &view(Phase::Working, other, None)),
            AddAwait::Idle
        );
        // The press's own add, still running.
        assert_eq!(
            add_await(Some(SAFE), SAFE, &view(Phase::Working, SAFE, None)),
            AddAwait::Waiting
        );
    }
}
