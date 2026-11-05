//! A site's request to a multisig, on the wallet page (AUDIT M5): the
//! in-app browser's request for the account a site was granted, when that
//! account is a multisig here, goes to the proposals machine instead of the
//! Vela signing column (the web's `DappRequestHost` → `holdDappRequest`).
//!
//! - its review first, in the review-first draft dialog with the site said
//!   (`proposal.siteAsks`); a message is refused, said first (dapp-7);
//! - Decline, Escape or the close: `DappLeave` — the machine answers 4001
//!   before it was proposed, "proposed, waiting" after;
//! - proposed: the proposal's sheet opens in the third column, saying what
//!   the site gets (`siteWaits` / `siteTold` / `siteFollows`); closing it
//!   while the site waits is leaving it too;
//! - once the machine answers, the site hears it through the browser
//!   machine's answer path (`SigningAnswered`), then the machine is told
//!   (`DappSettled`); a batch's `wallet_getCallsStatus` is the machine's
//!   (`CallsStatus`).
//!
//! Nothing is decided here: which request is a multisig's is the grant's
//! address on the multisig machines' list; everything after is the
//! machine's ([`crate::multisig::dapp`]).

use vela_core::app::multisig_dapp::{HeldRequest, HeldStage};
use vela_core::app::sign_request::EXTENSION_REQUEST_TTL_MS;

use super::*;
use crate::multisig::dapp::{self as ms_dapp, CallsRead, Step};

/// The site, as the signing sheet names it.
fn site_of(origin: &str) -> String {
    signing_live::dapp_identity(origin).0.to_string()
}

impl WalletPage {
    /// The browser machine forwarded `forwarded` for the account the site
    /// was granted. When that account is a multisig here, its request is
    /// the proposals machine's — `true` — and nothing reaches the Vela
    /// signing column. One at a time: while the send flow proposes, or
    /// another site's request is held, the site hears "busy, ask again".
    #[cfg_attr(target_os = "linux", allow(dead_code))]
    pub(crate) fn ms_dapp_take(&mut self, forwarded: &Forwarded, cx: &mut Context<Self>) -> bool {
        let Some(host) = ms_host::booted(cx) else {
            return false;
        };
        let account = {
            let host = host.read(cx);
            ms_dapp::multisig_of(
                &forwarded.granted_address,
                &host.machines().accounts_view.accounts,
            )
            .cloned()
        };
        let Some(account) = account else {
            return false;
        };
        if !self.multisig.dapp.is_empty() || host.read(cx).send_proposing() {
            self.answer_dapp_signing(
                &forwarded.tab,
                forwarded.id.clone(),
                busy_answer(),
                None,
                cx,
            );
            return true;
        }
        self.multisig_observe(cx);
        // The site is told it was proposed before the limit the web's
        // requests have — so "If that takes longer than 5 minutes" is true
        // here too, and the browser's line of requests moves on.
        let ask = self
            .multisig
            .dapp
            .hold(forwarded, &account, Some(EXTENSION_REQUEST_TTL_MS));
        self.multisig.scroll = crate::ui::SmoothScroll::new();
        self.multisig.approve_as = None;
        host.update(cx, |host, cx| host.proposals(ask, cx));
        self.ms_dapp_follow(cx);
        true
    }

    /// dapp-7: a connect for a multisig says what a multisig cannot be
    /// asked (the web's `multisigConnectNote`) — `address` is the account
    /// the grant would be made for.
    pub(crate) fn ms_dapp_connect_note(
        &mut self,
        theme: &Theme,
        address: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let address = address.filter(|a| !a.is_empty())?;
        let host = ms_host::booted(cx)?;
        let multisig =
            ms_dapp::multisig_of(address, &host.read(cx).machines().accounts_view.accounts)
                .is_some();
        multisig.then(|| {
            div()
                .text_size(theme::text_row_sub())
                .line_height(gpui::relative(1.4))
                .text_color(theme.fg_muted)
                .child(crate::ui::prose(
                    self.loc.t("connect.browser.multisigConnectNote"),
                ))
        })
    }

    /// The page that asked is gone (its 4900 already sent): a request the
    /// multisig holds is left — its review withdrawn, a proposal kept for
    /// the members. `true` when it was the multisig's.
    pub(crate) fn ms_dapp_cancel(&mut self, tab: &str, id: &str, cx: &mut Context<Self>) -> bool {
        let Some(rid) = self.multisig.dapp.find(tab, id) else {
            return false;
        };
        self.ms_dapp_leave(rid, cx);
        true
    }

    /// The person left request `rid` (Decline, close, Escape, read the
    /// refusal): the machine answers it as it stands.
    pub(crate) fn ms_dapp_leave(&mut self, rid: String, cx: &mut Context<Self>) {
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| {
            host.proposals(ms_proposals::Event::DappLeave { id: rid }, cx);
        });
        self.ms_dapp_follow(cx);
    }

    /// What the proposals machine did with the requests it holds: a
    /// proposal's sheet opened once, each answer to its site once (then
    /// `DappSettled`), each batch read answered.
    pub(crate) fn ms_dapp_follow(&mut self, cx: &mut Context<Self>) {
        let Some(host) = ms_host::booted(cx) else {
            return;
        };
        let (steps, calls) = {
            let view = &host.read(cx).machines().proposals_view;
            (
                self.multisig.dapp.step(view),
                self.multisig.dapp.calls_answered(view),
            )
        };
        for step in steps {
            match step {
                Step::Open {
                    chain_id,
                    safe_tx_hash,
                } => self.open_multisig_sheet(chain_id, safe_tx_hash, cx),
                Step::Answer {
                    rid,
                    tab,
                    id,
                    answer,
                    close,
                } => {
                    // core-ms-2: delivered as the machine formed it (an
                    // error keeps its `data`); a batch's id is the browser
                    // machine's to follow from here — kept across a restart
                    // in its own store (r9-18).
                    let browser = self.browser_host(cx);
                    browser.update(cx, |browser, cx| {
                        browser.dispatch(
                            DbrEvent::MultisigAnswered {
                                tab: tab.clone(),
                                id,
                                answer: *answer,
                                now_ms: crate::executor::now_ms(),
                            },
                            cx,
                        );
                    });
                    let shown = close.as_ref().is_some_and(|(chain_id, hash)| {
                        self.multisig
                            .sheet
                            .as_ref()
                            .is_some_and(|(c, h)| c == chain_id && same(h, hash))
                    });
                    if shown {
                        self.close_multisig_sheet(cx);
                    }
                    host.update(cx, |host, cx| {
                        host.proposals(ms_proposals::Event::DappSettled { id: rid }, cx);
                    });
                }
            }
        }
        self.ms_dapp_answer_reads(calls, cx);
    }

    /// The browser's batch reads, for the proposals machine.
    pub(crate) fn ms_dapp_calls(&mut self, reads: Vec<CallsRead>, cx: &mut Context<Self>) {
        if reads.is_empty() {
            return;
        }
        let host = ms_host::booted(cx);
        let mut now = Vec::new();
        for read in reads {
            let Some(host) = &host else {
                // No machine to ask: nothing known yet, never left hanging.
                let unknown =
                    vela_core::app::multisig_dapp::DappAnswer::ok(serde_json::Value::Null);
                now.push((read.read, ms_dapp::read_body(&unknown)));
                continue;
            };
            let event = self.multisig.dapp.calls(read);
            host.update(cx, |host, cx| host.proposals(event, cx));
        }
        self.ms_dapp_answer_reads(now, cx);
        self.ms_dapp_follow(cx);
    }

    fn ms_dapp_answer_reads(&mut self, reads: Vec<(u64, String)>, cx: &mut Context<Self>) {
        if reads.is_empty() {
            return;
        }
        let browser = self.browser_host(cx);
        browser.update(cx, |browser, cx| {
            for (read, body) in reads {
                browser.multisig_calls_answered(read, body, cx);
            }
        });
    }

    /// Escape on a site's request under review (or its refusal): left.
    pub(crate) fn ms_dapp_escape(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(host) = ms_host::booted(cx) else {
            return false;
        };
        let rid = {
            let view = &host.read(cx).machines().proposals_view;
            self.multisig
                .dapp
                .at(view, HeldStage::Refused)
                .or_else(|| self.multisig.dapp.at(view, HeldStage::Reviewing))
                .map(|(rid, _)| rid)
        };
        let Some(rid) = rid else {
            return false;
        };
        self.ms_dapp_leave(rid, cx);
        true
    }

    /// The proposal's sheet went: a site still waiting on it is told it was
    /// proposed (the machine's answer for leaving it).
    pub(crate) fn ms_dapp_sheet_closed(
        &mut self,
        chain_id: u64,
        hash: &str,
        cx: &mut Context<Self>,
    ) {
        self.multisig.dapp.unsee();
        let waiting = ms_host::booted(cx).and_then(|host| {
            self.multisig
                .dapp
                .waiting_on(&host.read(cx).machines().proposals_view, chain_id, hash)
        });
        if let Some(rid) = waiting {
            self.ms_dapp_leave(rid, cx);
        }
    }

    /// What the proposal's sheet says of the site that asked for it: who
    /// asked, and what it gets — the transaction once executed, its id
    /// followed, or that it was told it was proposed.
    pub(crate) fn ms_dapp_sheet_note(
        &mut self,
        theme: &Theme,
        item: &ProposalItem,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let held = self
            .multisig
            .dapp
            .seen(item.chain_id, &item.safe_tx_hash)?
            .clone();
        let loc = self.ms_loc();
        let site = site_of(&held.origin);
        let mut note = div().flex().flex_col().gap(px(8.)).child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(words::tv(&loc, "proposal.siteAsks", &[("host", &site)])),
        );
        if item.status != ms_proposals::ProposalStatus::Done {
            let batch = held.method == "wallet_sendCalls";
            if !batch && !held.told && item.approved < item.threshold {
                note = note.child(ms_note(
                    theme,
                    &words::tv(
                        &loc,
                        "proposal.stillNeeded",
                        &[("missing", &(item.threshold - item.approved).to_string())],
                    ),
                    Tone::Neutral,
                ));
            }
            let key = if batch {
                "proposal.siteFollows"
            } else if held.told {
                "proposal.siteTold"
            } else {
                "proposal.siteWaits"
            };
            note = note.child(ms_note(
                theme,
                &words::tv(&loc, key, &[("host", &site)]),
                Tone::Neutral,
            ));
        }
        let _ = cx;
        Some(note)
    }

    /// A site's request over the page, while it is the person's to answer:
    /// the refusal of a message (dapp-7), its draft with the review
    /// (review-first, the send flow's dialog), or — before the draft —
    /// that it is being read, or why it could not be.
    pub(crate) fn ms_dapp_layer(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.multisig.dapp.is_empty() {
            return None;
        }
        let host = ms_host::booted(cx)?;
        let (refused, reviewing, drafted, propose) = {
            let view = &host.read(cx).machines().proposals_view;
            (
                self.multisig.dapp.at(view, HeldStage::Refused),
                self.multisig.dapp.at(view, HeldStage::Reviewing),
                view.draft.is_some(),
                view.actions.get("propose").cloned(),
            )
        };
        if let Some((rid, held)) = refused {
            return Some(self.ms_dapp_refusal(theme, window, rid, &held, cx));
        }
        let (rid, held) = reviewing?;
        let site = site_of(&held.origin);
        if drafted {
            return self.ms_draft_dialog(theme, window, Some((rid, site)), cx);
        }
        let failure = match &propose {
            None | Some(ActionState::Working) => None,
            // Proposed: its sheet is on the way.
            Some(ActionState::Done { .. } | ActionState::OutsideAsked { .. }) => return None,
            Some(state) => Some(
                self.ms_action_words(state, held.chain_id, cx)
                    .unwrap_or_else(|| words::t(&self.ms_loc(), "proposal.reason.unknown")),
            ),
        };
        Some(self.ms_dapp_reading(theme, window, rid, &site, failure, cx))
    }

    /// The site's transaction being read, or why it could not be (Decline,
    /// Try again).
    fn ms_dapp_reading(
        &mut self,
        theme: &Theme,
        window: &Window,
        rid: String,
        site: &str,
        failure: Option<String>,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let loc = self.ms_loc();
        let mut body = div().flex().flex_col().gap(px(14.)).child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(words::tv(&loc, "proposal.siteAsks", &[("host", site)])),
        );
        let mut actions = crate::ui::dialog::dialog_actions().child({
            let rid = rid.clone();
            crate::ui::vela_button(
                "ms-dapp-decline",
                crate::ui::ButtonVariant::Secondary,
                words::t(&loc, "external.decline").into(),
                theme,
                cx.listener(move |this, _, _, cx| this.ms_dapp_leave(rid.clone(), cx)),
            )
            .w_auto()
        });
        match &failure {
            Some(text) => {
                body = body.child(ms_note(theme, text, Tone::Danger));
                let retry = rid.clone();
                actions = actions.child(
                    crate::ui::vela_button(
                        "ms-dapp-retry",
                        crate::ui::ButtonVariant::Secondary,
                        words::t(&loc, "external.retry").into(),
                        theme,
                        cx.listener(move |this, _, _, cx| {
                            let Some(ask) = this.multisig.dapp.again(&retry) else {
                                return;
                            };
                            let host = this.ms_host(cx);
                            host.update(cx, |host, cx| host.proposals(ask, cx));
                        }),
                    )
                    .w_auto(),
                );
            }
            None => {
                body = body.child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_subtle)
                        .child(words::t(&loc, "proposal.opening")),
                );
            }
        }
        body = body.child(actions);
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        crate::ui::dialog::dialog(
            "ms-dapp-reading",
            theme,
            window,
            words::t(&loc, "proposal.title").into(),
            None,
            close_icon,
            body,
            &self.multisig.scroll,
            cx.listener(move |this, _, _, cx| this.ms_dapp_leave(rid.clone(), cx)),
        )
        .into_any_element()
    }

    /// dapp-7: a multisig was asked for a message signature, which it cannot
    /// give yet — said first; the site hears the refusal once it is read.
    fn ms_dapp_refusal(
        &mut self,
        theme: &Theme,
        window: &Window,
        rid: String,
        held: &HeldRequest,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let site = site_of(&held.origin);
        let body = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(crate::ui::dialog::dialog_body(
                theme,
                self.loc
                    .t_text("connect.browser.multisigNoMessagesBody", "host", &site)
                    .to_string(),
            ))
            .child(crate::ui::dialog::dialog_actions().child({
                let rid = rid.clone();
                crate::ui::vela_button(
                    "ms-dapp-refused",
                    crate::ui::ButtonVariant::Secondary,
                    self.loc.t("common.gotIt"),
                    theme,
                    cx.listener(move |this, _, _, cx| this.ms_dapp_leave(rid.clone(), cx)),
                )
                .w_auto()
            }));
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        crate::ui::dialog::dialog(
            "ms-dapp-refused",
            theme,
            window,
            self.loc.t("connect.browser.multisigNoMessagesTitle"),
            None,
            close_icon,
            body,
            &self.multisig.scroll,
            cx.listener(move |this, _, _, cx| this.ms_dapp_leave(rid.clone(), cx)),
        )
        .into_any_element()
    }
}
