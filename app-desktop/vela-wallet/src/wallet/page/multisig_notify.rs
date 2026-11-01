//! Multisig notifications on the wallet page (AUDIT M6): the Settings →
//! Appearance switch beside the menu-bar one, the in-app line shown when
//! the platform's notification center cannot show one, and a click's way
//! to the transaction's sheet (`crate::multisig::notify::deliver`).

use vela_core::app::multisig_accounts as ms_accounts;

use super::*;
use crate::multisig::notify::deliver::{self, Target};

/// How far a click got toward the transaction's sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoticedOpen {
    /// The sheet is open.
    Opened,
    /// Its multisig was asked to be shown; the page for it comes next.
    Showing,
    /// Not yet (the page, or its list, is on its way).
    Waiting,
    /// Nothing to open: the multisig is not on this device any more.
    Gone,
}

impl WalletPage {
    /// The switch: off until the person turns it on. Drawn only once the
    /// corpus has its words — a switch labelled with its own key path is
    /// not shipped (as the menu-bar switch).
    pub(super) fn ms_notify_row(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        self.identity.as_ref()?;
        let (title_key, hint_key) = deliver::SWITCH_KEYS;
        let title = self.loc.t(title_key);
        let hint = self.loc.t(hint_key);
        if title.as_ref() == title_key || hint.as_ref() == hint_key {
            return None;
        }
        let on = deliver::wanted();
        let page = cx.entity();
        Some(crate::settings::components::toggle_row(
            "settings-multisig-notify",
            theme,
            title,
            hint,
            on,
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                deliver::set_wanted(!on);
                page.update(cx, |_, cx| cx.notify());
            },
        ))
    }

    /// The in-app line: the notification the platform could not show
    /// (macOS without its bundle or its permission) at the bottom of the
    /// window — the contacts toast's pill. A press opens its transaction.
    /// r10 S4: it stays until it was seen (the window in front for a while,
    /// `attention::SEEN_FOR`) or dismissed with its ×.
    pub(super) fn ms_notify_toast(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let line = deliver::in_app(cx)?;
        let tag = line.worded.tag.clone();
        let label = format!("{} — {}", line.worded.title, line.worded.body);
        let mut words = div()
            .id("multisig-notify-line")
            .a11y_button(label.clone())
            .flex_1()
            .min_w(px(0.))
            .truncate()
            .child(label);
        if line.target.is_some() {
            words = words
                .cursor_pointer()
                .on_click(move |_, _, cx| deliver::clicked(&tag, cx));
        }
        let dismiss = self.loc.t("componentsUi.signing.dismiss");
        let close = div()
            .id("multisig-notify-dismiss")
            .a11y_button(dismiss)
            .flex_none()
            .cursor_pointer()
            .child(icon_img(
                &mut self.icons,
                Icon::X,
                false,
                theme.bg_base,
                16.,
            ))
            .on_click(|_, _, cx| deliver::dismiss_in_app(cx));
        let pill = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .pl(px(16.))
            .pr(px(10.))
            .py(px(8.))
            .max_w(px(560.))
            .rounded_full()
            .bg(theme.fg_base)
            .text_color(theme.bg_base)
            .text_size(theme::text_label())
            .shadow(crate::ui::dialog::shadow_lg())
            .child(words)
            .child(close);
        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(32.))
                .flex()
                .justify_center()
                .child(pill),
        )
    }

    /// A notification's transaction, opened: its multisig shown first
    /// (`activate`: ask the accounts machine, once), then — on the page
    /// built for it, its list read — the sheet.
    pub(crate) fn ms_open_noticed(
        &mut self,
        target: &Target,
        activate: bool,
        cx: &mut Context<Self>,
    ) -> NoticedOpen {
        let host = self.ms_host(cx);
        let is = |a: Option<&str>| a.is_some_and(|a| a.eq_ignore_ascii_case(&target.multisig));
        let (here, active, followed, hash) = {
            let machines = host.read(cx).machines();
            let accounts = &machines.accounts_view;
            let proposals = &machines.proposals_view;
            let here = accounts
                .accounts
                .iter()
                .any(|row| row.account.address.eq_ignore_ascii_case(&target.multisig));
            let read = proposals.read_at.is_some();
            let hash = proposals
                .items
                .iter()
                .find(|i| {
                    i.chain_id == target.chain_id
                        && i.safe_tx_hash.eq_ignore_ascii_case(&target.safe_tx_hash)
                })
                .map(|i| i.safe_tx_hash.clone());
            (
                here,
                is(accounts.active.as_deref()),
                is(proposals.followed.as_deref()) && read,
                hash,
            )
        };
        if !here {
            return NoticedOpen::Gone;
        }
        if !active {
            if activate {
                let address = target.multisig.clone();
                host.update(cx, |host, cx| {
                    host.accounts(
                        ms_accounts::Event::Activate {
                            address: Some(address),
                        },
                        cx,
                    );
                });
                return NoticedOpen::Showing;
            }
            return NoticedOpen::Waiting;
        }
        let mine = self
            .identity
            .as_ref()
            .is_some_and(|i| i.address.eq_ignore_ascii_case(&target.multisig));
        if !mine || !followed {
            return NoticedOpen::Waiting;
        }
        if self.section != Section::Wallet {
            self.go_to_section(Section::Wallet, cx);
        }
        let hash = hash.unwrap_or_else(|| target.safe_tx_hash.clone());
        self.open_multisig_sheet(target.chain_id, hash, cx);
        NoticedOpen::Opened
    }
}
