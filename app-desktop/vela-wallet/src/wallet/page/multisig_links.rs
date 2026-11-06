//! Where a multisig link opened here stands (the accounts machine's `link`,
//! drawn as the web's `MultisigHost.drawLink` draws it): a damaged link said
//! at once; an invite opens the add sheet filled in; a proposal on a network
//! this device lacks says so, with Add network; "Opening…" while the
//! multisig is read; a link whose multisig has no member here opens its
//! transaction read-only, with "Add to this device"; one that could not be
//! added opens the add sheet with why. Each stand is drawn once, then let
//! go (`close_link`) — "Opening…" stays until the machine moves on.

use vela_core::app::multisig_accounts::link::LinkStand;
use vela_core::app::multisig_accounts::{self as ms_accounts, AddOutcome};
use vela_core::multisig_link::{AddLinkFault, BrokenLink};

use super::multisig::{Tone as MsTone, ms_note, same};
use super::multisig_add::{AddSaid, outcome_said};
use super::*;
use crate::multisig::words;

/// A link's own sheet over the page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum LinkSheet {
    /// A line and nothing else to do (`proposal.badLink`, `proposal.opening`).
    Notice { key: &'static str, opening: bool },
    /// The link's network is not on this device.
    Missing { chain_id: u64 },
}

/// A network the person went to add for a link: once it is here, the
/// machines are told (`networks`) — the proposal link waiting for it opens
/// on its own; an invite is opened again (`hash`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NetworkWait {
    pub chain_id: u64,
    pub invite: Option<String>,
}

/// What a stand asks of the page (pure: the tests read it).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum LinkTake {
    /// A link's sheet.
    Sheet(LinkSheet),
    /// The add sheet: its text, the invite, what is said at once — and
    /// (r15-2) whether its Try again is the core's `retry_link`.
    Add {
        text: String,
        link: Option<vela_core::multisig_link::AddLink>,
        outcome: Option<AddOutcome>,
        fault: Option<&'static str>,
        retry_link: bool,
    },
    /// The multisig of a read-only transaction (`preview`).
    Preview(String),
    /// Nothing to draw (its transaction's sheet opens through the screen).
    Nothing,
}

/// What `stand` is drawn as, and whether it is let go now (`close_link`).
pub(crate) fn take_stand(stand: &LinkStand) -> (LinkTake, bool) {
    match stand {
        LinkStand::Broken {
            link: BrokenLink::Add,
            text,
            fault,
        } => (
            LinkTake::Add {
                text: text.clone(),
                link: None,
                outcome: None,
                fault: Some(match fault {
                    Some(AddLinkFault::Checksum) => "add.badChecksum",
                    _ => "add.badInvite",
                }),
                retry_link: false,
            },
            true,
        ),
        LinkStand::Broken { .. } => (
            LinkTake::Sheet(LinkSheet::Notice {
                key: "proposal.badLink",
                opening: false,
            }),
            true,
        ),
        LinkStand::Invite { link } => (
            LinkTake::Add {
                text: link.multisig.clone(),
                link: Some(link.clone()),
                outcome: None,
                fault: None,
                retry_link: false,
            },
            true,
        ),
        LinkStand::MissingNetwork { link } => (
            LinkTake::Sheet(LinkSheet::Missing {
                chain_id: link.chain_id,
            }),
            true,
        ),
        LinkStand::Opening { .. } => (
            LinkTake::Sheet(LinkSheet::Notice {
                key: "proposal.opening",
                opening: true,
            }),
            false,
        ),
        LinkStand::Preview { link, .. } => (LinkTake::Preview(link.multisig.clone()), true),
        // §18 (c): a lookup that did not come back is retryable by the
        // core's rule (`LinkStand::Failed.retry`, `AddOutcome::offers_retry`):
        // the add sheet's Add says Try again for it. r15-2: that Try again is
        // the core's `retry_link` — the same link, opened again — so the
        // stand is kept until the sheet is done with it (`retry` finds it
        // there); one that answered is let go at once.
        LinkStand::Failed {
            link,
            reason,
            retry,
        } => (
            LinkTake::Add {
                text: link.multisig.clone(),
                link: None,
                outcome: Some(reason.clone()),
                fault: None,
                retry_link: *retry,
            },
            !*retry,
        ),
        LinkStand::Member { .. } | LinkStand::Kept => (LinkTake::Nothing, true),
    }
}

/// What a sheet carries across the page being rebuilt. Creating or adding a
/// multisig makes it the one shown (the machine's `activate`), and the
/// window builds the page again for it (`main.rs`: a page is its shown
/// account's) — the sheet's own state goes with the old page. So the
/// created screen, and an add still awaited (with its invite, whose labels
/// and networks are kept once it lands), are carried here and opened again
/// on the new page.
#[derive(Default)]
pub(crate) struct Carry {
    /// Create was pressed (the name typed).
    create: Option<String>,
    /// An add is awaited: its address and the invite it came from.
    add: Option<(String, Option<vela_core::multisig_link::AddLink>)>,
}

impl gpui::Global for Carry {}

impl Carry {
    pub(crate) fn set_create(name: String, cx: &mut gpui::App) {
        cx.default_global::<Self>().create = Some(name);
    }

    pub(crate) fn take_create(cx: &mut gpui::App) -> Option<String> {
        cx.default_global::<Self>().create.take()
    }

    pub(crate) fn set_add(
        address: String,
        link: Option<vela_core::multisig_link::AddLink>,
        cx: &mut gpui::App,
    ) {
        cx.default_global::<Self>().add = Some((address, link));
    }

    pub(crate) fn take_add(
        cx: &mut gpui::App,
    ) -> Option<(String, Option<vela_core::multisig_link::AddLink>)> {
        cx.default_global::<Self>().add.take()
    }
}

impl WalletPage {
    /// A new page: the sheet carried from the last one opens again — the
    /// created multisig's screen (no new draft: the machine's `create` view
    /// is the one just made), or the add still awaited.
    pub(crate) fn ms_restore_carried(&mut self, cx: &mut Context<Self>) {
        if let Some(name) = Carry::take_create(cx) {
            let mut form = super::multisig_create::CreateForm::new(Vec::new(), cx);
            form.name = name.clone();
            form.pressed = true;
            form.draft_read = true;
            form.step = super::multisig_create::CreateStep::Review;
            self.multisig.create = true;
            self.multisig.create_form = Some(form);
            // Still carried until the person closes the created screen.
            Carry::set_create(name, cx);
        }
        if let Some((address, link)) = Carry::take_add(cx) {
            self.ms_add_opened(Some(address.clone()), link.clone(), None, cx);
            if let Some(form) = self.multisig.add_form.as_mut() {
                form.asked = Some(address.clone());
            }
            Carry::set_add(address, link, cx);
        }
    }

    /// The machine's `link`, drawn once each: what it opens is opened, and
    /// it is let go — "Opening…" only once the machine moves on.
    pub(crate) fn ms_take_link(&mut self, cx: &mut Context<Self>) {
        let host = self.ms_host(cx);
        let stand = host.read(cx).machines().accounts_view.link.clone();
        let key = stand.as_ref().and_then(|s| serde_json::to_string(s).ok());
        if key == self.multisig.link_seen {
            return;
        }
        self.multisig.link_seen = key;
        let Some(stand) = stand else {
            return;
        };
        let (take, close) = take_stand(&stand);
        // Any stand past "Opening…" ends it.
        if matches!(
            self.multisig.link_sheet,
            Some(LinkSheet::Notice { opening: true, .. })
        ) {
            self.multisig.link_sheet = None;
        }
        // r15-2: a link pasted into the add sheet is read there — the
        // sheet's Add spins while the core reads it ("Opening…" is that
        // spinner, not a sheet over it), and the sheet goes once the link
        // opens its transaction or asks for its network.
        let reading = self
            .multisig
            .add_form
            .as_ref()
            .is_some_and(|form| form.reading);
        match take {
            LinkTake::Sheet(LinkSheet::Notice { opening: true, .. }) if reading => {}
            LinkTake::Sheet(sheet) => {
                if reading {
                    self.close_multisig_add(cx);
                }
                self.multisig.link_sheet = Some(sheet);
                self.multisig.scroll = crate::ui::SmoothScroll::new();
            }
            LinkTake::Add {
                text,
                link,
                outcome,
                fault,
                retry_link,
            } => {
                let loc = self.ms_loc();
                let said = fault
                    .map(|key| AddSaid::Line(words::t(&loc, key)))
                    .or_else(|| outcome.as_ref().and_then(|o| outcome_said(&loc, o)));
                // r15-2: the invite the sheet already drew from the pasted
                // link is this one: the sheet stays as the person has it.
                let drawn = self
                    .multisig
                    .add_form
                    .as_ref()
                    .is_some_and(|form| link.is_some() && form.link == link && !form.reading)
                    && self
                        .multisig
                        .add
                        .as_deref()
                        .is_some_and(|typed| same(typed, &text));
                if !drawn {
                    // The add sheet is the one in front; a draft being created is kept.
                    self.multisig.create = false;
                    self.multisig.create_form = None;
                    self.ms_add_opened(Some(text.clone()), link, said, cx);
                    if let Some(form) = self.multisig.add_form.as_mut() {
                        // r16-17: Add on what the damaged link held says it
                        // again, never "Enter a valid address".
                        form.broken = fault.map(|key| (text, key));
                        form.choosing = matches!(outcome, Some(AddOutcome::ChooseNetwork));
                        // r14-5: a lookup that did not come back offers Try again.
                        form.retry = outcome.as_ref().is_some_and(AddOutcome::offers_retry);
                        // r15-2: and that Try again opens the link again.
                        form.retry_link = retry_link;
                    }
                }
            }
            LinkTake::Preview(address) => {
                if reading {
                    self.close_multisig_add(cx);
                }
                self.multisig.preview_of = Some(address);
            }
            LinkTake::Nothing => {
                if reading {
                    self.close_multisig_add(cx);
                }
            }
        }
        if close {
            cx.defer(move |cx| {
                host.update(cx, |host, cx| {
                    host.accounts(ms_accounts::Event::CloseLink, cx)
                });
            });
        }
        cx.notify();
    }

    /// A network asked for by a link is here now: the machines are told
    /// (the proposal waiting for it opens), an invite is opened again.
    pub(crate) fn ms_network_watch(&mut self, cx: &mut Context<Self>) {
        let Some(wait) = self.multisig.network_wait.clone() else {
            return;
        };
        let here = crate::network_space::all_networks()
            .iter()
            .any(|(id, _)| u64::from(*id) == wait.chain_id);
        if !here {
            return;
        }
        self.multisig.network_wait = None;
        let host = self.ms_host(cx);
        cx.defer(move |cx| {
            host.update(cx, |host, cx| {
                host.refresh_facts(cx);
                if wait.invite.is_some() {
                    // The invite kept (`keep_link`) opens now.
                    host.accounts(
                        ms_accounts::Event::OpenLink {
                            hash: String::new(),
                            take_kept: true,
                        },
                        cx,
                    );
                }
            });
        });
    }

    /// Wait for `chain_id`; an invite's `hash` is kept by the machine
    /// meanwhile (`keep_link`).
    pub(crate) fn ms_wait_for_network(
        &mut self,
        chain_id: u64,
        invite: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(hash) = invite.clone() {
            let host = self.ms_host(cx);
            host.update(cx, |host, cx| {
                host.accounts(ms_accounts::Event::KeepLink { hash }, cx)
            });
        }
        self.multisig.network_wait = Some(NetworkWait { chain_id, invite });
    }

    /// Settings → Add network, the chain filled in (the wizard looks it up
    /// in the chain index, and asks for an RPC when the index does not know it).
    pub(crate) fn ms_open_add_network(&mut self, chain_id: u64, cx: &mut Context<Self>) {
        let Ok(chain_id) = u32::try_from(chain_id) else {
            return;
        };
        self.settings_dialog = Some(SettingsDialog::AddNetwork);
        resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
            resident.dispatch(NetEvent::WizardReset, cx);
            resident.dispatch(
                NetEvent::ChainSelected {
                    chain_id,
                    keep_custom_rpc: false,
                },
                cx,
            );
        });
        cx.notify();
    }

    /// The link's sheet went: "Opening…" lets the link go; a missing
    /// network's lets the link waiting for it go (`drop_link`).
    pub(crate) fn close_link_sheet(&mut self, cx: &mut Context<Self>) {
        let Some(sheet) = self.multisig.link_sheet.take() else {
            return;
        };
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| match sheet {
            LinkSheet::Notice { opening: true, .. } => {
                host.accounts(ms_accounts::Event::CloseLink, cx)
            }
            LinkSheet::Missing { .. } => host.accounts(ms_accounts::Event::DropLink, cx),
            LinkSheet::Notice { .. } => {}
        });
        cx.notify();
    }

    /// The link's own sheet (a notice, or the missing network).
    pub(crate) fn ms_link_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let sheet = self.multisig.link_sheet.clone()?;
        let loc = self.ms_loc();
        let body = match sheet {
            LinkSheet::Notice { key, .. } => {
                div().child(ms_note(theme, &words::t(&loc, key), MsTone::Neutral))
            }
            LinkSheet::Missing { chain_id } => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(words::tv(
                            &loc,
                            "proposal.missingNetwork",
                            &[("chainId", &chain_id.to_string())],
                        )),
                )
                .child(crate::ui::dialog::dialog_body(
                    theme,
                    words::t(&loc, "proposal.missingNetworkBody"),
                ))
                .child(
                    crate::ui::dialog::dialog_actions().child(
                        crate::ui::vela_button(
                            "ms-link-add-network",
                            crate::ui::ButtonVariant::Secondary,
                            words::t(&loc, "proposal.addNetwork").into(),
                            theme,
                            cx.listener(move |this, _, _, cx| {
                                // The machine keeps the link waiting for its
                                // network: the sheet steps aside for the wizard.
                                this.multisig.link_sheet = None;
                                this.ms_wait_for_network(chain_id, None, cx);
                                this.ms_open_add_network(chain_id, cx);
                            }),
                        )
                        .w_auto(),
                    ),
                ),
        };
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        Some(
            crate::ui::dialog::dialog(
                "ms-link",
                theme,
                window,
                words::t(&loc, "proposal.title").into(),
                None,
                close_icon,
                body,
                &self.multisig.scroll,
                cx.listener(|this, _, _, cx| this.close_link_sheet(cx)),
            )
            .into_any_element(),
        )
    }

    /// ms-21: a link's transaction on a device with none of its members is
    /// read only; following its multisig here is the person's press
    /// (`keep_found`) — it is kept, not shown.
    pub(crate) fn ms_preview_keep(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let address = self.multisig.preview_of.clone()?;
        let loc = self.ms_loc();
        let host = self.ms_host(cx);
        let kept = host
            .read(cx)
            .machines()
            .accounts_view
            .accounts
            .iter()
            .any(|r| same(&r.account.address, &address));
        if kept {
            return Some(ms_note(
                theme,
                &words::t(&loc, "proposal.watching"),
                MsTone::Neutral,
            ));
        }
        Some(
            div().child(
                crate::ui::vela_button(
                    "ms-preview-keep",
                    crate::ui::ButtonVariant::Secondary,
                    words::t(&loc, "proposal.watch").into(),
                    theme,
                    cx.listener(|this, _, _, cx| {
                        let host = this.ms_host(cx);
                        host.update(cx, |host, cx| {
                            host.accounts(
                                ms_accounts::Event::KeepFound {
                                    accept_risks: false,
                                },
                                cx,
                            );
                        });
                    }),
                )
                .w_auto(),
            ),
        )
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use vela_core::multisig_link::{AddLink, ProposalLink};

    use super::*;

    const SAFE: &str = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83";

    fn proposal() -> ProposalLink {
        ProposalLink {
            chain_id: 100,
            multisig: SAFE.into(),
            safe_tx_hash: "0xab".into(),
        }
    }

    #[test]
    fn each_stand_is_drawn_as_the_web_draws_it() {
        let (take, close) = take_stand(&LinkStand::Broken {
            link: BrokenLink::Add,
            text: "100:0x88".into(),
            fault: Some(AddLinkFault::Checksum),
        });
        assert!(close);
        assert!(matches!(
            take,
            LinkTake::Add {
                fault: Some("add.badChecksum"),
                ..
            }
        ));

        let (take, _) = take_stand(&LinkStand::Broken {
            link: BrokenLink::Proposal,
            text: String::new(),
            fault: None,
        });
        assert_eq!(
            take,
            LinkTake::Sheet(LinkSheet::Notice {
                key: "proposal.badLink",
                opening: false
            })
        );

        let invite = AddLink {
            multisig: SAFE.into(),
            chain_ids: vec![100],
            members: None,
        };
        let (take, _) = take_stand(&LinkStand::Invite {
            link: invite.clone(),
        });
        assert_eq!(
            take,
            LinkTake::Add {
                text: SAFE.into(),
                link: Some(invite),
                outcome: None,
                fault: None,
                retry_link: false,
            }
        );

        let (take, _) = take_stand(&LinkStand::MissingNetwork { link: proposal() });
        assert_eq!(take, LinkTake::Sheet(LinkSheet::Missing { chain_id: 100 }));

        let (take, close) = take_stand(&LinkStand::Opening { link: proposal() });
        assert!(!close, "Opening stays until the machine moves on");
        assert!(matches!(
            take,
            LinkTake::Sheet(LinkSheet::Notice { opening: true, .. })
        ));

        let (take, _) = take_stand(&LinkStand::Preview {
            link: proposal(),
            kept: false,
        });
        assert_eq!(take, LinkTake::Preview(SAFE.into()));

        let (take, _) = take_stand(&LinkStand::Failed {
            link: proposal(),
            reason: AddOutcome::NotFound,
            retry: false,
        });
        assert!(matches!(
            take,
            LinkTake::Add {
                outcome: Some(AddOutcome::NotFound),
                retry_link: false,
                ..
            }
        ));

        let (take, close) = take_stand(&LinkStand::Kept);
        assert_eq!(take, LinkTake::Nothing);
        assert!(close);
    }
}
