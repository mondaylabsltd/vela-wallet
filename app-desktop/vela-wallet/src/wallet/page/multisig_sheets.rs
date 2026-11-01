//! The invite sheet (AUDIT M1): the link and code a multisig's members are
//! sent. The create and add sheets are `multisig_create.rs` and
//! `multisig_add.rs`; a link's stand is `multisig_links.rs`.

use vela_core::app::multisig_accounts as ms_accounts;
use vela_core::multisig_link as link;

use super::*;
use crate::multisig::words;

impl WalletPage {
    /// The invite a multisig's members are sent (`#msa=`, the core's
    /// `add_link` over its `invite_members`): its link and its code, copied or shown. Copying it
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
        let row = host
            .read(cx)
            .machines()
            .accounts_view
            .accounts
            .iter()
            .find(|r| r.account.address.eq_ignore_ascii_case(&address))
            .cloned()?;
        // r9-12: the members the core puts in the invite (`invite_members`:
        // founding members, then members now; a name only where one is known,
        // and the kind) — never a list built here.
        let invite = link::add_link(
            link::SITE_ORIGIN,
            loc.language(),
            &row.account.address,
            &row.account.chain_ids,
            &row.invite_members,
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
