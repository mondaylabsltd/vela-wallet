//! The multisig sheets beyond the transaction's own (AUDIT M1): create, add
//! (by address, an invite, or a Safe{Wallet} export) and invite. Each draws
//! the accounts machine's view (`create`, `add`, `import`, `link`) and sends
//! its events; nothing is decided here.

use std::collections::BTreeMap;

use vela_core::app::multisig_accounts as ms_accounts;
use vela_core::multisig_link::{self as link, InviteAccount, MemberInfo};

use super::multisig::item_key;
use super::*;
use crate::multisig::words;

impl WalletPage {
    /// The create sheet (filled in by the M1 sheets pass).
    pub(crate) fn ms_create_dialog(
        &mut self,
        _theme: &Theme,
        _window: &Window,
        _cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        None
    }

    /// The create sheet closed: what it holds is kept (r7-9) by the sheet's
    /// own `keep_draft`, so closing loses nothing.
    pub(crate) fn close_multisig_create(&mut self, cx: &mut Context<Self>) {
        self.multisig.create = false;
        cx.notify();
    }

    /// The add sheet (filled in by the M1 sheets pass).
    pub(crate) fn ms_add_dialog(
        &mut self,
        _theme: &Theme,
        _window: &Window,
        _cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        None
    }

    /// The invite a multisig's members are sent (`#msa=`, the core's
    /// `invite_link`): its link and its code, copied or shown. Copying it
    /// ticks the home's first step (`invite_shared`).
    pub(crate) fn ms_invite_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let address = self.multisig.invite.clone()?;
        let loc = self.ms_loc();
        let host = self.ms_host(cx);
        let (row, now) = {
            let machines = host.read(cx).machines();
            let row = machines
                .accounts_view
                .accounts
                .iter()
                .find(|r| r.account.address.eq_ignore_ascii_case(&address))
                .cloned()?;
            let followed = machines
                .proposals_view
                .followed
                .as_deref()
                .is_some_and(|f| f.eq_ignore_ascii_case(&address));
            let now = if followed {
                machines.proposals_view.members_anywhere.clone()
            } else {
                Vec::new()
            };
            (row, now)
        };
        let known: BTreeMap<String, MemberInfo> = row
            .account
            .members
            .clone()
            .unwrap_or_default()
            .into_iter()
            .map(|(k, v)| {
                (
                    k,
                    MemberInfo {
                        name: v.name,
                        kind: v
                            .kind
                            .and_then(|k| serde_json::from_value(serde_json::json!(k)).ok()),
                    },
                )
            })
            .collect();
        let invite = link::invite_link(
            link::SITE_ORIGIN,
            loc.language(),
            &InviteAccount {
                address: row.account.address.clone(),
                owners: row.account.owners.clone(),
                chain_ids: row.account.chain_ids.clone(),
            },
            &now,
            &known,
        )
        .ok()?;
        let copy = invite.clone();
        let shared = row.account.address.clone();
        let body = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(crate::ui::dialog::dialog_body(theme, words::t(&loc, "invite.body")))
            .child(
                div()
                    .flex()
                    .justify_center()
                    .child(crate::flows::components::qr_card(theme, None, Some(&invite))),
            )
            .child(
                div()
                    .font_family(theme::font_mono())
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(invite.clone()),
            )
            .child(
                crate::ui::vela_button(
                    "ms-invite-copy",
                    crate::ui::ButtonVariant::Secondary,
                    words::t(&loc, "proposal.copyLink").into(),
                    theme,
                    cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy.clone()));
                        let host = this.ms_host(cx);
                        let address = shared.clone();
                        host.update(cx, |host, cx| {
                            host.accounts(ms_accounts::Event::InviteShared { address }, cx);
                        });
                    }),
                ),
            )
            .child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(crate::ui::prose(words::t(&loc, "invite.outsideHint"))),
            );
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        let _ = item_key;
        Some(
            crate::ui::dialog::dialog(
                "ms-invite",
                theme,
                window,
                words::t(&loc, "panel.invite").into(),
                None,
                close_icon,
                body,
                &self.multisig.scroll,
                cx.listener(|this, _, _, cx| {
                    this.multisig.invite = None;
                    cx.notify();
                }),
            )
            .into_any_element(),
        )
    }
}
