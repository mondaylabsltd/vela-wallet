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
use vela_core::app::multisig_review_state::{
    self as review_state, ReviewShows, ReviewState, SignDoor,
};
use vela_core::app::multisig_shell::MultisigAccount;
use vela_core::app::sim_outcome::SimVerdict;
use vela_core::multisig::SafeTxHashes;
use vela_core::multisig_reading::MemberChanges;
use vela_core::multisig_review::{
    Concern, Field, Recipient, RecipientRole, Refused, SignRule, StateGuard,
};

use gpui::App;

use super::*;
use crate::multisig::host::{self as ms_host, MultisigHost};
use crate::multisig::machines::Screen;
use crate::multisig::words;

/// A site's request to a multisig (AUDIT M5).
#[path = "multisig_dapp.rs"]
mod dapp;
/// A multisig's own part of Settings → Account (AUDIT M1, M3). A child of
/// this module (its file beside this one) so it draws with the sheet's
/// helpers without widening them.
#[path = "multisig_settings.rs"]
mod settings;

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
    /// "Copied" under the outside-wallet link, until the next sheet.
    pub outside_copied: bool,
    /// r10 S4: the link the sheet's foot offers as its main press (what this
    /// person can do next, once their own members have acted) — the
    /// section above then does not offer it a second time.
    pub foot_link: Option<String>,
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
    /// What the person typed and picked on the create sheet.
    pub create_form: Option<super::multisig_create::CreateForm>,
    /// What the person picked on the add sheet (its text is `add`).
    pub add_form: Option<super::multisig_add::AddForm>,
    /// The proposals machine.s token facts (`view.tokens`, r8-2), as last
    /// heard: what a title or an amount is worded in, never read here.
    pub tokens: words::ViewTokens,
    /// A link's own sheet (`multisig_links.rs`).
    pub link_sheet: Option<super::multisig_links::LinkSheet>,
    /// The link stand last drawn (each is drawn once).
    pub link_seen: Option<String>,
    /// The multisig of a link's read-only transaction (ms-21).
    pub preview_of: Option<String>,
    /// A network the person went to add for a link.
    pub network_wait: Option<super::multisig_links::NetworkWait>,
}

/// The reject question's presses on a transaction's foot (r4D-14).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RejectPress {
    /// `ms-reject`: the question is up; nothing is asked of a machine.
    Ask,
    /// `ms-reject-cancel`: the question goes; nothing is asked or signed.
    Back,
    /// `ms-reject-confirm`: the cancellation at the transaction's number is
    /// drafted — its review first, no passkey yet.
    Confirm { chain_id: u64, nonce: u128 },
}

impl MultisigUi {
    /// What a reject press does to the sheet's state, and what (if
    /// anything) it asks of the proposals machine.
    pub(crate) fn reject_press(&mut self, press: RejectPress) -> Option<ms_proposals::Event> {
        match press {
            RejectPress::Ask => {
                self.rejecting = true;
                None
            }
            RejectPress::Back => {
                self.rejecting = false;
                None
            }
            RejectPress::Confirm { chain_id, nonce } => {
                self.rejecting = false;
                Some(ms_proposals::Event::Draft {
                    chain_id,
                    proposal: ms_proposals::ProposalKind::Cancel {
                        nonce: nonce.to_string(),
                    },
                })
            }
        }
    }
}

/// Reject is offered: a member of it on this device (r9-9: the core's
/// `members_here`, whatever the transaction is), nothing at its number
/// cancelling it already, and no execution of it on its way.
pub(crate) fn reject_offered(item: &ProposalItem, executing: bool) -> bool {
    !item.members_here.is_empty() && item.cancelled_by.is_none() && !executing
}

/// r11 S4: a transaction Vela refuses for what it is (a delegatecall, a
/// refund, a batch too deep) has nobody who can approve it at the source
/// (`can_approve` empty): no member is asked to, and no link is handed on.
fn nobody_approves(item: &ProposalItem) -> bool {
    item.can_approve.is_empty()
        && vela_core::multisig_review::refused_for_what_it_is(item.chain_id, &item.tx, &item.review)
            .is_some()
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
    /// A concern of a kind this build does not know is left out (its own
    /// block says it), never the whole review unread.
    #[serde(deserialize_with = "known_concerns")]
    pub concerns: Vec<Concern>,
    pub sign: SignRule,
    /// r11: the heading over what moves ("Balance changes"), the core's key.
    pub moves_title_key: Option<String>,
    /// r9-9: why nothing is signed, the core's one line (its corpus key).
    pub refused: Option<Refused>,
    /// What each call does, as the core read it (r8-1, r9-1).
    #[serde(skip)]
    pub reading: Option<crate::multisig::reading::ReadingDraw>,
}

fn known_concerns<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Concern>, D::Error> {
    let all: Vec<Value> = Deserialize::deserialize(d)?;
    Ok(all
        .into_iter()
        .filter_map(|c| serde_json::from_value(c).ok())
        .collect())
}

impl ReviewDraw {
    fn of(value: &Value) -> Option<Self> {
        let mut draw: Self = serde_json::from_value(value.clone()).ok()?;
        draw.reading = crate::multisig::reading::ReadingDraw::of(value);
        Some(draw)
    }
}

/// §12: the review as drawn and the one door every sign, propose, approve
/// and execute press follows — the core's `door` over the machine's state
/// and review (`sheet::sign_door`). The review is drawn only through an
/// open-to-view door (`shows: review`); one this sheet cannot read is drawn
/// as unreadable and shuts the door, never signed on unseen.
pub(crate) fn review_door(
    state: Option<&ReviewState>,
    raw: Option<&Value>,
) -> (Option<ReviewDraw>, SignDoor) {
    let draw = raw.filter(|r| !r.is_null()).and_then(ReviewDraw::of);
    let door = crate::multisig::sheet::sign_door(state, raw, draw.is_some());
    let draw = draw.filter(|_| door.shows == ReviewShows::Review);
    (draw, door)
}

/// `<chainId>:<safeTxHash>` — the views' key for one transaction.
pub(crate) fn item_key(chain_id: u64, hash: &str) -> String {
    format!("{chain_id}:{hash}")
}

pub(crate) fn network(chain_id: u64) -> String {
    crate::executor::custom_tokens::network_name(u32::try_from(chain_id).unwrap_or_default())
}

pub(crate) fn same(a: &str, b: &str) -> bool {
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
                    // r13-7: rows written for executions found elsewhere —
                    // the feed reads its store again now.
                    Screen::ActivityRows(_) => crate::executor::activity_feed::focus_tick(cx),
                    Screen::Activated(_) => {}
                }
            }
            this.multisig.tokens = host.read(cx).machines().proposals_view.tokens.clone();
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
                let hash = ask
                    .get("safe_tx_hash")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                self.open_multisig_sheet(chain_id, hash, cx);
            }
            Some("close_sheet") => self.close_multisig_sheet(cx),
            // The Send pill, as the person presses it (r9-6: documented,
            // never wired): the recipient filled where the script names one.
            Some("send") => {
                let params = vela_core::app::send::SendOpenParams {
                    prefilled_recipient: ask.get("to").and_then(Value::as_str).map(str::to_owned),
                    ..Default::default()
                };
                self.enter_flow_with(FlowEntry::Send, params, cx);
                crate::multisig::dev_script::note(&format!(
                    "page send: panel {:?}, flows {:?}, send open {}",
                    self.panel,
                    self.flows,
                    self.send_host.is_some()
                ));
            }
            Some("approve_as") => {
                self.multisig.approve_as =
                    ask.get("member").and_then(Value::as_str).map(str::to_owned);
            }
            Some("executor") => {
                self.multisig.executor =
                    ask.get("member").and_then(Value::as_str).map(str::to_owned);
            }
            Some("invite") => {
                self.multisig.invite = ask
                    .get("address")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                self.multisig.scroll = crate::ui::SmoothScroll::new();
            }
            Some("close_invite") => self.multisig.invite = None,
            // The add sheet with a link or address pasted, as the person does.
            Some("add") => {
                let text = ask.get("text").and_then(Value::as_str).map(str::to_owned);
                self.open_multisig_add(text, cx);
                if ask.get("press").and_then(Value::as_bool) == Some(true) {
                    self.ms_add_press(false, cx);
                }
            }
            Some("session") => self.multisig.session = true,
            Some("switcher") => self.account_switcher = true,
            Some("close_switcher") => self.account_switcher = false,
            Some("tech") => self.multisig.tech = true,
            // Settings → Appearance's multisig-notifications switch, pressed
            // (`{"page":"notify","on":true}`): the same call its press makes.
            Some("notify") => {
                let on = ask.get("on").and_then(Value::as_bool).unwrap_or(true);
                crate::multisig::notify::deliver::set_wanted(on);
            }
            // The network space switched, as the person's press does
            // (`{"page":"space","to":"testnet"}`).
            Some("space") => {
                let to = match ask.get("to").and_then(Value::as_str) {
                    Some("testnet") => NetworkSpace::Testnet,
                    _ => NetworkSpace::Mainnet,
                };
                crate::agent::invoke_person(Action::SetNetworkSpace { space: to }, cx);
            }
            // A section of the sidebar, pressed.
            Some("section") => {
                let to = match ask.get("to").and_then(Value::as_str) {
                    Some("explore") => Section::Explore,
                    Some("settings") => Section::Settings,
                    Some("contacts") => Section::Contacts,
                    _ => Section::Wallet,
                };
                self.go_to_section(to, cx);
            }
            // A site opened in a new tab of the browser, as the person types
            // its address into a new tab (`{"page":"browse","url":"…"}`).
            Some("browse") => {
                if let Some(url) = ask.get("url").and_then(Value::as_str) {
                    self.go_to_section(Section::Explore, cx);
                    self.open_in_new_tab(url.to_owned(), None, cx);
                }
            }
            // A site picked on the start page (`Go::Open`), or an address
            // typed into the bar (`Go::Typed`), as the person does.
            Some(kind @ ("open_site" | "type_url")) => {
                if let Some(url) = ask.get("url").and_then(Value::as_str) {
                    use crate::wallet::browser_host::Go;
                    self.go_to_section(Section::Explore, cx);
                    let go = if kind == "open_site" {
                        Go::Open(url.to_owned())
                    } else {
                        Go::Typed(url.to_owned())
                    };
                    self.browser_go(go, cx);
                }
            }
            // The open connection request's Connect, as the person presses
            // it (`{"page":"consent"}`).
            Some("consent") => {
                use vela_core::app::dapp_browser::Event as DbrEvent;
                let host = self.browser_host(cx);
                host.update(cx, |host, cx| {
                    host.dispatch(
                        DbrEvent::ConsentApproved {
                            now_ms: crate::executor::now_ms(),
                        },
                        cx,
                    );
                });
            }
            // r10 follow-up: the space notes the sheets have now, as each
            // draws it, in the script's log (`{"page":"space_notes"}`).
            Some("space_notes") => {
                let view = self.browser_host(cx).read(cx).view.clone();
                let worded = |note: &vela_core::app::dapp_browser::DbrSpaceNote| {
                    self.loc.t_text(
                        &note.key,
                        "network",
                        &crate::flows::live::chain_name(note.chain_id),
                    )
                };
                let consent = view.consent.as_ref().map(|c| {
                    (
                        c.origin.clone(),
                        c.chain_id,
                        c.space_note.as_ref().map(worded),
                    )
                });
                let signing = view
                    .signing
                    .as_ref()
                    .map(|s| (s.id.clone(), s.space_note.as_ref().map(worded)));
                let sheet = self.signing_host.as_ref().and_then(|host| {
                    let request = host.read(cx).view.request.clone()?;
                    let note = request.space_note?;
                    Some((
                        request.id,
                        note.chain_id,
                        self.loc.t_text(
                            &note.key,
                            "network",
                            &crate::flows::live::chain_name(note.chain_id),
                        ),
                    ))
                });
                crate::multisig::dev_script::note(&format!(
                    "space_notes panel {:?} consent {consent:?} signing {signing:?} sheet {sheet:?}",
                    self.panel
                ));
            }
            // What the browser holds now, in the script's log.
            Some("tabs") => {
                let explore = resident::resident::<ExploreSites>(cx).read(cx).view();
                crate::multisig::dev_script::note(&format!(
                    "tabs {:?} selected {:?}",
                    explore
                        .tabs
                        .iter()
                        .map(|t| (t.id.clone(), t.url.clone()))
                        .collect::<Vec<_>>(),
                    explore.selected_tab
                ));
            }
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
        // r9-12: one name per member, the core's (`member_labels`: kept, the
        // passkey index, its own name, the short address) — the same on
        // every platform, never picked here.
        if let Some(label) = ms_host::booted(cx).and_then(|host| {
            host.read(cx)
                .machines()
                .accounts_view
                .member_labels
                .get(&address.to_ascii_lowercase())
                .cloned()
        }) {
            return label;
        }
        let own = session::view(cx)
            .accounts
            .iter()
            .find(|row| same(&row.account.address, address))
            .map(|row| row.account.name.clone());
        let kept = account.and_then(|a| a.members.as_ref()).and_then(|m| {
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
    pub(crate) fn open_multisig_sheet(
        &mut self,
        chain_id: u64,
        hash: String,
        cx: &mut Context<Self>,
    ) {
        self.multisig.sheet = Some((chain_id, hash.clone()));
        self.multisig.approve_as = None;
        self.multisig.executor = None;
        self.multisig.tech = false;
        self.multisig.copied = false;
        self.multisig.outside_copied = false;
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
        if self.ms_loose_draft(cx) {
            host.update(cx, |host, cx| host.withdraw_draft(cx));
            return true;
        }
        if self.multisig.link_sheet.is_some() {
            self.close_link_sheet(cx);
            return true;
        }
        if self.multisig.create {
            self.close_multisig_create(cx);
            return true;
        }
        if self.multisig.add.is_some() {
            self.close_multisig_add(cx);
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
        let (view, presence, followed, followed_chip) = {
            let machines = host.read(cx).machines();
            (
                machines.accounts_view.clone(),
                machines.proposals_view.presence.clone(),
                machines.proposals_view.followed.clone(),
                machines.proposals_view.rule_chip.clone(),
            )
        };
        let prefix = if in_dialog {
            "switcher-ms"
        } else {
            "settings-ms"
        };
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
                    words::tv(
                        &loc,
                        "switcher.rowName",
                        &[("m", &m.to_string()), ("n", &n.to_string())],
                    )
                });
            // r9-14 (§9.2): the rule in force from the chain — the followed
            // multisig's from proposals, any other's as the accounts machine
            // keeps it (`AccountRow.rule`), "last known" when not read now.
            let is_followed = followed.as_deref().is_some_and(|f| same(f, &address));
            let mut flags: Vec<String> = vec![crate::multisig::member_words::row_chip(
                &loc,
                &row.rule,
                followed_chip.as_ref().filter(|_| is_followed),
            )];
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
                    flags.push(words::tc(
                        &loc,
                        "presence.needsYou",
                        waiting.needs_you as usize,
                    ));
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
                pick = pick.child(icon_img(
                    &mut self.icons,
                    Icon::Check,
                    false,
                    theme.accent,
                    18.,
                ));
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
                .id(ElementId::from((
                    if in_dialog {
                        "switcher-ms-remove"
                    } else {
                        "settings-ms-remove"
                    },
                    i,
                )))
                .a11y_button(crate::ui::a11y::close_label())
                .flex_none()
                .p(px(2.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|el| el.bg(theme.bg_sunken))
                .child(icon_img(
                    &mut self.icons,
                    Icon::X,
                    false,
                    theme.fg_subtle,
                    18.,
                ))
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
            if self
                .multisig
                .removing
                .as_deref()
                .is_some_and(|r| same(r, &address))
            {
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
                                    crate::flows::components::danger_button(
                                        theme,
                                        self.strings.account_remove.clone(),
                                    )
                                    .id(ElementId::from((
                                        if in_dialog {
                                            "switcher-ms-confirm"
                                        } else {
                                            "settings-ms-confirm"
                                        },
                                        i,
                                    )))
                                    .cursor_pointer()
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.multisig.removing = None;
                                            let host = this.ms_host(cx);
                                            let address = gone.clone();
                                            host.update(cx, |host, cx| {
                                                host.accounts(
                                                    ms_accounts::Event::Remove { address },
                                                    cx,
                                                );
                                            });
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    crate::flows::components::ghost_button(
                                        theme,
                                        self.strings.sign_out_cancel.clone(),
                                    )
                                    .rounded(px(12.))
                                    .id(ElementId::from((
                                        if in_dialog {
                                            "switcher-ms-cancel"
                                        } else {
                                            "settings-ms-cancel"
                                        },
                                        i,
                                    )))
                                    .cursor_pointer()
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.multisig.removing = None;
                                            cx.notify();
                                        },
                                    )),
                                ),
                        ),
                );
            }
        }
        // The two ways to add one.
        let create =
            crate::flows::components::ghost_button(theme, words::t(&loc, "switcher.create").into())
                .rounded(px(12.))
                .w_auto()
                .flex_initial()
                .min_w(px(0.))
                .px(px(24.))
                .id(if in_dialog {
                    "switcher-ms-create"
                } else {
                    "settings-ms-create"
                })
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    if in_dialog {
                        this.close_account_switcher(cx);
                    }
                    this.open_multisig_create(cx);
                }));
        let add =
            crate::flows::components::ghost_button(theme, words::t(&loc, "switcher.add").into())
                .rounded(px(12.))
                .w_auto()
                .flex_initial()
                .min_w(px(0.))
                .px(px(24.))
                .id(if in_dialog {
                    "switcher-ms-add"
                } else {
                    "settings-ms-add"
                })
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
                    .child(crate::ui::prose(words::t(
                        &loc,
                        "switcher.createMultisigNote",
                    ))),
            )
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .pt(px(8.))
                    .child(create)
                    .child(add),
            )
    }

    /// Open the create sheet (its kept draft, if any, comes back with it).
    pub(crate) fn open_multisig_create(&mut self, cx: &mut Context<Self>) {
        self.multisig.create = true;
        self.multisig.scroll = crate::ui::SmoothScroll::new();
        let host = self.ms_host(cx);
        host.update(cx, |host, cx| {
            host.accounts(ms_accounts::Event::NewDraft, cx)
        });
        self.ms_create_opened(cx);
        cx.notify();
    }

    /// Open the add sheet, filled with `text` (an address, or an invite).
    pub(crate) fn open_multisig_add(&mut self, text: Option<String>, cx: &mut Context<Self>) {
        self.ms_add_opened(text, None, None, cx);
    }

    // -- the home's waiting group -------------------------------------------

    /// The multisig's rule chip ("Needs 2 of 3", r8-13): the proposals
    /// machine's `rule_chip` for the multisig it follows, else the rule it
    /// was made with — with its title (each network's rule). `None`: the
    /// Vela account is shown.
    pub(crate) fn ms_rule_chip(&mut self, cx: &mut Context<Self>) -> Option<(String, String)> {
        let row = self.shown_multisig(cx)?;
        let loc = self.ms_loc();
        let host = ms_host::booted(cx)?;
        let view = &host.read(cx).machines().proposals_view;
        let followed = view
            .followed
            .as_deref()
            .is_some_and(|f| same(f, &row.account.address));
        // The followed multisig's chip from proposals; until it is followed,
        // the rule the accounts machine keeps for it (§9.2), never the
        // founding rule while a chain has said another.
        let chip = view
            .rule_chip
            .clone()
            .filter(|_| followed)
            .unwrap_or_else(|| row.rule.chip.clone());
        Some(crate::multisig::member_words::chip(&loc, &chip))
    }

    /// "Get it ready" (r4D-8, r8-13): a multisig just created here shows its
    /// first steps — invite the members (ticked once the invite was handed
    /// on), add funds (ticked once it holds something on its first
    /// network), and what its first transfer out costs — until the person
    /// dismisses them or the machine reads it set up (`set_up`).
    pub(crate) fn multisig_get_ready(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let row = self.shown_multisig(cx)?;
        let host = self.ms_host(cx);
        let (followed, set_up) = {
            let view = &host.read(cx).machines().proposals_view;
            (
                view.followed
                    .as_deref()
                    .is_some_and(|f| same(f, &row.account.address)),
                view.set_up,
            )
        };
        let address = row.account.address.clone();
        if row.account.first_run.unwrap_or(false) && followed && set_up {
            // Set up (deployed, funded and used): its first steps are behind
            // it for good (r5reg-17).
            cx.defer(move |cx| {
                ms_host::host(cx).update(cx, |host, cx| {
                    host.accounts(ms_accounts::Event::SetFirstRun { address, on: false }, cx);
                });
            });
            return None;
        }
        if !crate::multisig::sheet::get_ready_shown(row.account.first_run, followed, set_up) {
            return None;
        }
        let loc = self.ms_loc();
        let network_id = row.account.chain_ids.first().copied();
        let funded = network_id.is_some_and(|chain| {
            let view = crate::network_space::balance_view(cx);
            view.address
                .as_deref()
                .is_some_and(|a| same(a, &row.account.address))
                && view.tokens.iter().any(|t| {
                    u64::from(t.chain_id) == chain
                        && !t.spam
                        && t.balance.chars().any(|c| ('1'..='9').contains(&c))
                })
        });
        let solo = row.account.owners.len() == 1;
        let step = |this: &mut Self,
                    id: &'static str,
                    done: bool,
                    icon: Icon,
                    title: String,
                    note: Option<String>| {
            div()
                .id(id)
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(8.))
                .cursor_pointer()
                .hover(|el| el.bg(theme.bg_sunken))
                .child(icon_img(
                    &mut this.icons,
                    if done { Icon::Check } else { icon },
                    false,
                    if done {
                        theme.success_base
                    } else {
                        theme.fg_base
                    },
                    20.,
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
                                .text_color(theme.fg_base)
                                .child(title),
                        )
                        .children(note.map(|note| {
                            div()
                                .text_size(theme::text_label())
                                .text_color(theme.fg_subtle)
                                .child(crate::ui::prose(note))
                        })),
                )
                .child(icon_img(
                    &mut this.icons,
                    Icon::ChevronRight,
                    false,
                    theme.fg_subtle,
                    16.,
                ))
        };
        let mut card = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .mb(px(16.))
            .p(px(16.))
            .rounded(px(12.))
            .border_1()
            .border_color(theme.divider)
            .child(
                div()
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .child(words::t(&loc, "create.firstRunTitle")),
            );
        if !solo {
            let invite = address.clone();
            card = card.child(
                step(
                    self,
                    "ms-get-ready-invite",
                    row.invite_shared,
                    Icon::QrCode,
                    words::t(&loc, "create.firstRunInvite"),
                    Some(words::t(&loc, "create.firstRunInviteNote")),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.multisig.invite = Some(invite.clone());
                    this.multisig.scroll = crate::ui::SmoothScroll::new();
                    cx.notify();
                })),
            );
        }
        card = card.child(
            step(
                self,
                "ms-get-ready-fund",
                funded,
                Icon::ArrowDownLeft,
                words::t(&loc, "create.firstRunFund"),
                network_id.map(|c| {
                    words::tv(&loc, "create.firstRunFundNote", &[("network", &network(c))])
                }),
            )
            .on_click(cx.listener(|this, _, _, cx| {
                this.enter_flow(FlowEntry::Receive, cx);
                cx.notify();
            })),
        );
        if let Some(chain) = network_id {
            card = card.child(
                div()
                    .flex()
                    .gap(px(12.))
                    .py(px(4.))
                    .child(div().flex_none().child(icon_img(
                        &mut self.icons,
                        Icon::Info,
                        false,
                        theme.fg_subtle,
                        20.,
                    )))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(crate::ui::prose(words::tv(
                                &loc,
                                "create.firstRunCost",
                                &[("network", &network(chain))],
                            ))),
                    ),
            );
        }
        let dismiss = address.clone();
        card = card.child(div().pt(px(8.)).child(crate::ui::vela_button(
            "ms-get-ready-dismiss",
            crate::ui::ButtonVariant::Secondary,
            words::t(&loc, "create.firstRunDismiss").into(),
            theme,
            cx.listener(move |this, _, _, cx| {
                let address = dismiss.clone();
                this.ms_host(cx).update(cx, |host, cx| {
                    host.accounts(ms_accounts::Event::SetFirstRun { address, on: false }, cx);
                });
            }),
        )));
        Some(card)
    }

    /// r10-4 / r11: the networks of the other space the multisig shown
    /// lives on, when none of its networks is in the space in front (its
    /// record's `chain_ids`; which space a network is in is the core's
    /// word). Empty for a Vela account, or a multisig used here.
    pub(crate) fn multisig_elsewhere(&self, cx: &App) -> Vec<u32> {
        let Some(row) = self.shown_multisig(cx) else {
            return Vec::new();
        };
        let ids: Vec<u32> = row
            .account
            .chain_ids
            .iter()
            .filter_map(|id| u32::try_from(*id).ok())
            .collect();
        crate::network_space::lives_elsewhere(&ids)
    }

    /// r10-4: a multisig shown from the space it does not live in — used
    /// only on networks of the other space — reads as empty here ("$0.00",
    /// "Deposit your first asset"): where it lives is said, with the switch
    /// there (the create sheet's way, r7-9). Its networks are its record's
    /// (`chain_ids`); which space a network is in is the core's word.
    pub(crate) fn multisig_other_space(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let elsewhere = self.multisig_elsewhere(cx);
        if elsewhere.is_empty() {
            return None;
        }
        let loc = self.ms_loc();
        let testnet = crate::network_space::testnet();
        let (to, heading, switch) = if testnet {
            (
                NetworkSpace::Mainnet,
                "networkSpace.headingMainnet",
                "networkSpace.toMainnet",
            )
        } else {
            (
                NetworkSpace::Testnet,
                "networkSpace.headingTestnet",
                "networkSpace.toTestnet",
            )
        };
        let networks = loc.list(
            &elsewhere
                .iter()
                .map(|id| network(u64::from(*id)))
                .collect::<Vec<_>>(),
        );
        let line = format!(
            "{} · {}",
            words::tv(&loc, "add.usedOn", &[("networks", &networks)]),
            self.loc.t(heading)
        );
        Some(
            div()
                .flex()
                .flex_col()
                .items_start()
                .gap(px(8.))
                .mb(px(16.))
                .p(px(16.))
                .rounded(px(12.))
                .border_1()
                .border_color(theme.warning_border)
                .bg(theme.warning_soft)
                .child(
                    div()
                        .flex()
                        .items_start()
                        .gap(px(8.))
                        .child(div().flex_none().child(icon_img(
                            &mut self.icons,
                            Icon::Info,
                            false,
                            theme.fg_base,
                            18.,
                        )))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(theme::text_row_sub())
                                .text_color(theme.fg_base)
                                .child(crate::ui::prose(line)),
                        ),
                )
                .child(crate::ui::vela_button(
                    "ms-other-space-switch",
                    crate::ui::ButtonVariant::Secondary,
                    self.loc.t(switch),
                    theme,
                    // A switch of space moves no value: the person's press,
                    // through the same intent the space dialog answers with.
                    cx.listener(move |_, _, _, cx| {
                        crate::agent::invoke_person(Action::SetNetworkSpace { space: to }, cx);
                        cx.notify();
                    }),
                )),
        )
    }

    /// The multisig's transactions waiting, first on its home (the web's
    /// `waitingRows.group`): one row each with its one word, the group's
    /// note and its Retry; a network the mailbox holds proposals on that
    /// this device lacks, said.
    pub(crate) fn multisig_waiting_group(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
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
                        host.update(cx, |host, cx| {
                            host.proposals(ms_proposals::Event::Retry, cx)
                        });
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
            // r4b-6: proposals at one number — only one of them can run.
            let conflict = (!item.same_nonce.is_empty()).then(|| words::t(&loc, "status.conflict"));
            let sub = [
                Some(network(item.chain_id)),
                Some(words::tv(
                    &loc,
                    "action.numberValue",
                    &[("nonce", &item.nonce.to_string())],
                )),
                conflict,
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(" · ");
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
        let names = |ids: &[u64]| loc.list(&ids.iter().map(|c| network(*c)).collect::<Vec<_>>());
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
            WaitingNote::ChainUnreachable { chain_ids } => words::tv(
                loc,
                "service.chainUnreachable",
                &[("network", &names(chain_ids))],
            ),
            WaitingNote::LastKnown { at } => words::tv(
                loc,
                "panel.lastKnown",
                &[("time", &loc.relative_time(*at, crate::executor::now_ms()))],
            ),
            WaitingNote::Stale => words::t(loc, "panel.stale"),
            WaitingNote::HistoryUnreadable { chain_ids } => words::tv(
                loc,
                "panel.historyUnreadable",
                &[("network", &names(chain_ids))],
            ),
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
                let info = words::token_facts(&self.multisig.tokens, chain_id, token);
                let amount = |k: &str| {
                    let value = why.get(k);
                    // r12-1: the core's amount (`MultisigAmount`): its `said`
                    // is the words, decimals and symbol resolved by the core.
                    if let Some(said) = value.and_then(|v| words::said_amount(loc, v)) {
                        return said;
                    }
                    let raw = value
                        .and_then(|v| v.get("base_units").or(Some(v)))
                        .and_then(Value::as_str)
                        .unwrap_or("0");
                    words::amount_words(loc, raw, &info)
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

    /// What a proposal is, in one line: a contract call as the core reads
    /// it (r8-1, r9-1 — "Approve · USDC", "Batch of 2 calls", never only
    /// "Contract call" for one the core can say), anything else in the
    /// proposals view's token facts (r8-2: never 18 decimals assumed).
    fn ms_title(&self, item: &ProposalItem, account: Option<&MultisigAccount>, cx: &App) -> String {
        let chain_id = item.chain_id;
        if matches!(
            item.review.action,
            Some(vela_core::multisig::Action::Call { .. })
        ) && let Some(account) = account
            && let Some(title) = self.ms_reading_title(chain_id, &account.address, &item.tx)
        {
            return title;
        }
        words::action_title(
            &self.loc,
            item.review.action.as_ref(),
            &item.nonce.to_string(),
            // r11-6: the core's `names::who` over the one label.
            &|a: &str| words::who(a, Some(&self.ms_member_name(a, account, cx))),
            &|t: Option<&str>| words::token_facts(&self.multisig.tokens, chain_id, t),
        )
    }

    /// The words a reading is said in, on `chain_id`.
    fn ms_reading_ctx(&self, chain_id: u64) -> crate::multisig::reading::Ctx<'_> {
        crate::multisig::reading::Ctx {
            loc: &self.loc,
            native_symbol: crate::flows::live::native_symbol(
                u32::try_from(chain_id).unwrap_or_default(),
            ),
        }
    }

    /// A call's title from the core's reading of its bytes (the link door's
    /// `review.reading`, in-process), with the token facts the proposals
    /// machine read; `None` when the reading says no more than "a call".
    fn ms_reading_title(
        &self,
        chain_id: u64,
        safe: &str,
        tx: &vela_core::multisig::SafeTx,
    ) -> Option<String> {
        use crate::multisig::reading;
        let prefix = format!("{chain_id}:");
        let facts: Vec<_> = self
            .multisig
            .tokens
            .iter()
            .filter(|(k, _)| k.starts_with(&prefix))
            .map(|(_, f)| f.clone())
            .collect();
        let read =
            vela_core::multisig_rules::review::reading_of(chain_id, safe, tx, &facts).ok()?;
        let draw = reading::ReadingDraw::of(&read)?;
        if !reading::says_more_than_a_call(&draw.reading) {
            return None;
        }
        reading::title(&draw.reading, &self.ms_reading_ctx(chain_id))
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
            let lines = no_item_lines(&loc, &view, linked.as_ref(), account.is_some(), chain_id);
            let mut body = div().flex().flex_col().gap(px(8.));
            for line in lines {
                body = body.child(
                    div()
                        .py(px(4.))
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_subtle)
                        .child(line),
                );
            }
            return Some((title, body, None));
        };
        // The sheet asks for each executor's look and the review now and
        // then; the machine decides whether its last one is stale (W9).
        self.ms_sheet_asks(&item, &view, cx);
        // r8-13: executed — a past fact. Nothing on it is a check of what
        // running it WOULD do any more (the web's `past`).
        let past = item.status == ms_proposals::ProposalStatus::Done;

        // §12: the review, and the one door every press below follows.
        let (review, door) = review_door(
            crate::multisig::sheet::state_of(&view.review_states, chain_id, &hash),
            crate::multisig::sheet::review_of(&view.reviews, chain_id, &hash),
        );
        let mut body = div().flex().flex_col().gap(px(16.)).pb(px(16.));

        // What it is: the review's own reading heads it (r8-1, the web's
        // `readingTitle`), else the proposal's title.
        let heading = review
            .as_ref()
            .and_then(|r| r.reading.as_ref())
            .and_then(|d| {
                crate::multisig::reading::title(&d.reading, &self.ms_reading_ctx(chain_id))
            })
            .unwrap_or_else(|| self.ms_title(&item, account.as_ref(), cx));
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
                .child(ms_kv(
                    theme,
                    &words::t(&loc, "action.network"),
                    &network(chain_id),
                ))
                .child(ms_kv(
                    theme,
                    &words::t(&loc, "action.number"),
                    &words::tv(
                        &loc,
                        "action.numberValue",
                        &[("nonce", &item.nonce.to_string())],
                    ),
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
            body = body.child(ms_note(
                theme,
                &words::t(&loc, "proposal.previewNote"),
                Tone::Neutral,
            ));
            body = body.children(self.ms_preview_keep(theme, cx));
        }
        if item.status == ms_proposals::ProposalStatus::Queued {
            // r11 (core `waits_for`): every number it waits behind, the same
            // on every surface — one said as "#n", several as a list.
            let text = match item.waits_for.as_slice() {
                [] => None,
                [one] => Some(words::tv(&loc, "proposal.queuedNote", &[("nonce", one)])),
                many => Some(words::tv(
                    &loc,
                    "proposal.queuedNoteMany",
                    &[(
                        "nonces",
                        &loc.list(&many.iter().map(|n| format!("#{n}")).collect::<Vec<_>>()),
                    )],
                )),
            };
            if let Some(text) = text {
                body = body.child(ms_note(theme, &text, Tone::Neutral));
            }
        }
        if !item.same_nonce.is_empty() {
            body = body.child(ms_note(
                theme,
                &words::tv(
                    &loc,
                    "proposal.conflict",
                    &[("nonce", &item.nonce.to_string())],
                ),
                Tone::Caution,
            ));
        }
        if item.cancelled_by.is_some() {
            body = body.child(ms_note(
                theme,
                &words::t(&loc, "proposal.cancelPending"),
                Tone::Caution,
            ));
        }

        // The one review (§3.6), or that it is being read, or why it could
        // not be with its "Try again" (§12): no Approve until it is read
        // (the Bybit rule).
        body = body.child(self.ms_review_place(
            theme,
            review.as_ref(),
            &door,
            chain_id,
            Some(item.safe_tx_hash.as_str()),
            past,
            account.as_ref(),
            cx,
        ));

        // Approvals: who has, and how each one still to approve signs (r7-8).
        let signers = view.signers.get(&key).cloned();
        let frozen = view
            .frozen
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&key))
            .map(|(_, f)| f.clone());
        body = body.child(self.ms_approvals(
            theme,
            &item,
            signers.as_ref(),
            frozen.as_ref(),
            account.as_ref(),
            cx,
        ));

        // What happened to the last press, said.
        for action in [
            format!("approve:{}", item.safe_tx_hash.to_ascii_lowercase()),
            format!("execute:{}", item.safe_tx_hash.to_ascii_lowercase()),
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
            // r8-16: on its way now — "Sending…", never "Not sent yet".
            let text = match (entry.why.as_str(), entry.retry_at) {
                _ if entry.sending || entry.why == "sending" => {
                    words::t(&loc, "proposal.outboxSending")
                }
                ("busy", Some(at)) => words::tv(
                    &loc,
                    "proposal.outboxBusy",
                    &[(
                        "seconds",
                        &(((at - crate::executor::now_ms()) / 1000.).max(0.).ceil()).to_string(),
                    )],
                ),
                (_, Some(at)) => words::tv(
                    &loc,
                    "proposal.outboxAway",
                    &[(
                        "seconds",
                        &(((at - crate::executor::now_ms()) / 1000.).max(0.).ceil()).to_string(),
                    )],
                ),
                (_, None) => words::t(&loc, "proposal.outboxNotSent"),
            };
            body = body.child(ms_note(theme, &text, Tone::Caution));
        }

        // Its history.
        if let Some(steps) = view.timelines.get(&key).filter(|s| !s.is_empty()) {
            body = body.child(self.ms_timeline(theme, steps, account.as_ref(), cx));
        }

        // The links the members still to approve open it with (r7-8): the
        // Vela link for Vela members and those nothing says yet; the
        // outside-wallet link for an outside wallet; nothing for a smart
        // contract account (said). Only while approvals are missing.
        // The foot first: the link it offers as its main press is not
        // offered again in the section (r10 S4).
        let foot = self.ms_actions(
            theme,
            &view,
            &item,
            review.as_ref(),
            &door,
            account.as_ref(),
            cx,
        );
        if let Some(account) = &account {
            body = body.children(self.ms_invite_links(theme, &item, signers.as_ref(), account, cx));
        }
        Some((title, body, foot))
    }

    /// While the sheet is up (r8-6, the web's `look` and review asks):
    /// every executor here is asked for its look every few seconds — also
    /// while the transaction is one approval short, so the fee and
    /// "Approve and execute" are known before any press — and the review
    /// again every 30 s (5 s while the chain could not be read). The machine
    /// decides whether its last one is stale; nothing is judged here.
    fn ms_sheet_asks(
        &mut self,
        item: &ProposalItem,
        view: &ms_proposals::ProposalsView,
        cx: &mut Context<Self>,
    ) {
        const LOOK_EVERY_MS: f64 = 5_000.;
        const REVIEW_FRESH_MS: f64 = 30_000.;
        const REVIEW_RETRY_MS: f64 = 5_000.;
        let now = crate::executor::now_ms();
        let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
        let mut events = Vec::new();
        for executor in crate::multisig::sheet::looked(
            item.status,
            item.review.action.is_some(),
            &item.can_execute,
        ) {
            let key = crate::multisig::sheet::look_key(chain_id, &hash, &executor);
            if self
                .multisig
                .previewed
                .get(&key)
                .is_some_and(|at| now - at < LOOK_EVERY_MS)
            {
                continue;
            }
            self.multisig.previewed.insert(key, now);
            events.push(ms_proposals::Event::Preview {
                chain_id,
                safe_tx_hash: hash.clone(),
                executor,
            });
        }
        // r9-18: who still approves, and how, asked again while the machine
        // has no answer for it — a sheet opened the moment a site's proposal
        // was made asked before the machine held the transaction, and drew
        // only its own approval for good.
        if item.status != ms_proposals::ProposalStatus::Done
            && !view
                .signers
                .keys()
                .any(|k| k.eq_ignore_ascii_case(&item_key(chain_id, &hash)))
        {
            let key = format!("signers:{}", item_key(chain_id, &hash.to_ascii_lowercase()));
            if self
                .multisig
                .previewed
                .get(&key)
                .is_none_or(|at| now - at >= LOOK_EVERY_MS)
            {
                self.multisig.previewed.insert(key, now);
                events.push(ms_proposals::Event::ReadSigners {
                    chain_id,
                    safe_tx_hash: hash.clone(),
                });
            }
        }
        // Done (executed from this sheet, or found so): the machine reads how
        // it ran — who executed, which approvals counted (`frozen`) — once.
        if item.status == ms_proposals::ProposalStatus::Done {
            let key = format!(
                "timeline-done:{}",
                item_key(chain_id, &hash.to_ascii_lowercase())
            );
            if let std::collections::btree_map::Entry::Vacant(slot) =
                self.multisig.previewed.entry(key)
            {
                slot.insert(now);
                events.push(ms_proposals::Event::Timeline {
                    chain_id,
                    safe_tx_hash: hash.clone(),
                });
            }
        }
        // The review: read once for one done; again while it waits — once
        // read (§12: one being read is the machine's; one that failed is the
        // person's "Try again", never asked again quietly here).
        let state = crate::multisig::sheet::state_of(&view.review_states, chain_id, &hash);
        if item.status != ms_proposals::ProposalStatus::Done
            && crate::multisig::sheet::asks_review_again(state)
        {
            let review = crate::multisig::sheet::review_of(&view.reviews, chain_id, &hash);
            let unread = review.is_none_or(|r| {
                r.is_null()
                    || r.pointer("/state/verdict/kind").and_then(Value::as_str)
                        == Some("unavailable")
                    || r.pointer("/simulation/kind").and_then(Value::as_str) == Some("unreachable")
            });
            let every = if unread {
                REVIEW_RETRY_MS
            } else {
                REVIEW_FRESH_MS
            };
            let key = format!("review:{}", item_key(chain_id, &hash.to_ascii_lowercase()));
            match self.multisig.previewed.get(&key) {
                // First drawn: the open asked for it already.
                None => {
                    self.multisig.previewed.insert(key, now);
                }
                Some(at) if now - at >= every => {
                    self.multisig.previewed.insert(key, now);
                    events.push(ms_proposals::Event::Review {
                        chain_id,
                        safe_tx_hash: hash.clone(),
                        blind: false,
                    });
                }
                Some(_) => {}
            }
        }
        if events.is_empty() {
            return;
        }
        let host = self.ms_host(cx);
        cx.defer(move |cx| {
            host.update(cx, |host, cx| {
                for event in events {
                    host.proposals(event, cx);
                }
            });
        });
    }

    /// The actor's look, said under the execute press (r4-7, r8-6): the fee
    /// and who pays, or why it cannot run, or that it is being checked —
    /// with "Check again" where the look could not answer.
    #[allow(clippy::too_many_arguments, reason = "the sheet's facts")]
    fn ms_look_line(
        &mut self,
        theme: &Theme,
        item: &ProposalItem,
        actor: &str,
        look: Option<&crate::multisig::sheet::Look>,
        payer: Option<&str>,
        account: Option<&MultisigAccount>,
        // D8: the review's door is open — Approve can be pressed.
        approve_open: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        use crate::multisig::sheet::Look;
        let loc = self.ms_loc();
        let name = self.ms_member_name(actor, account, cx);
        let info = words::token_info(item.chain_id, None);
        let coin = |raw: &str| words::amount_words(&loc, raw, &info);
        let (text, tone, retry) = match look {
            None => (words::t(&loc, "proposal.feeChecking"), Tone::Neutral, false),
            Some(Look::Priced {
                fee, can_pay: true, ..
            }) => (
                words::tv(
                    &loc,
                    "proposal.feeLine",
                    &[("fee", &coin(fee)), ("name", &name)],
                ),
                Tone::Neutral,
                false,
            ),
            Some(Look::Priced {
                fee,
                can_pay: false,
                balance,
            }) => (
                match payer.filter(|p| !same(p, actor)) {
                    Some(other) => words::tv(
                        &loc,
                        "proposal.cantPayOther",
                        &[
                            ("name", &name),
                            ("other", &self.ms_member_name(other, account, cx)),
                        ],
                    ),
                    None => words::tv(
                        &loc,
                        "proposal.cannotPay",
                        &[
                            ("name", &name),
                            ("symbol", &info.symbol),
                            ("fee", &coin(fee)),
                            ("balance", &coin(balance.as_deref().unwrap_or("0"))),
                        ],
                    ),
                },
                Tone::Caution,
                false,
            ),
            Some(Look::Fails(why)) => (
                words::tv(
                    &loc,
                    "proposal.wouldFail",
                    &[("reason", &self.ms_why_words(why, item.chain_id))],
                ),
                Tone::Caution,
                false,
            ),
            // §3.5: it would deploy from a genesis the core does not find
            // verified here — Vela won't sign for it.
            Some(Look::Refused(_)) => (
                format!(
                    "{} {}",
                    words::t(&loc, "authority.refusedTitle"),
                    words::t(&loc, "authority.refusedBody")
                ),
                Tone::Danger,
                false,
            ),
            Some(Look::Spent) => (words::t(&loc, "proposal.doneNote"), Tone::Neutral, false),
            Some(Look::Unpriced) => (words::t(&loc, "proposal.feeUnpriced"), Tone::Neutral, true),
            // D8: "you can still approve" only where Approve can be pressed.
            Some(Look::Unchecked) => (
                words::t(
                    &loc,
                    if approve_open {
                        "proposal.feeUnchecked"
                    } else {
                        "proposal.feeUncheckedHeld"
                    },
                ),
                Tone::Neutral,
                true,
            ),
        };
        let mut line = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(ms_note(theme, &text, tone));
        if retry {
            let executors = item.can_execute.clone();
            let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
            line = line.child(
                text_action(
                    "ms-look-again",
                    theme,
                    &mut self.icons,
                    None,
                    words::t(&loc, "proposal.feeRetry").into(),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    let host = this.ms_host(cx);
                    let (hash, executors) = (hash.clone(), executors.clone());
                    host.update(cx, |host, cx| {
                        for executor in executors {
                            host.proposals(
                                ms_proposals::Event::Preview {
                                    chain_id,
                                    safe_tx_hash: hash.clone(),
                                    executor,
                                },
                                cx,
                            );
                        }
                    });
                })),
            );
        }
        line
    }

    /// A row of members to pick from: who acts (`As`).
    fn ms_member_picker(
        &mut self,
        theme: &Theme,
        id: &'static str,
        members: &[String],
        picked: &str,
        account: Option<&MultisigAccount>,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut row = div().flex().flex_wrap().gap(px(6.)).items_center().child(
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
                        // One hand does the work: it approves and executes.
                        this.multisig.executor = Some(member.clone());
                        this.multisig.approve_as = Some(member.clone());
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
        door: &SignDoor,
        account: Option<&MultisigAccount>,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let loc = self.ms_loc();
        if item.status == ms_proposals::ProposalStatus::Done {
            return None;
        }
        let key = item_key(item.chain_id, &item.safe_tx_hash);
        let approving = matches!(
            view.actions.get(&format!(
                "approve:{}",
                item.safe_tx_hash.to_ascii_lowercase()
            )),
            Some(ActionState::Working)
        );
        let executing = item.executing
            || matches!(
                view.actions.get(&format!(
                    "execute:{}",
                    item.safe_tx_hash.to_ascii_lowercase()
                )),
                Some(ActionState::Working)
            );
        // The review's door decides whether a signature may be given at all,
        // and with what friction ("… anyway", r8-1): the core's one rule
        // over the review's state and its sign rule (§12) — shut while it is
        // read, shut when it failed, never open by default.
        let sign = crate::multisig::sheet::press_rule(door);
        let mut foot = div().flex().flex_col().gap(px(8.));

        // Whose hand does the work (the web's actor): the person's pick,
        // else an executor that can pay, else the member used last, else
        // the first — executors first, else approvers. Every fact is the
        // machine's (`can_*`, `previews`, `approve_execute`).
        use crate::multisig::sheet::{self as rule, FootKind};
        let approved_by: Vec<String> = item
            .approvals
            .iter()
            .filter(|a| a.counts)
            .map(|a| a.owner.clone())
            .collect();
        let picked = self
            .multisig
            .executor
            .clone()
            .or_else(|| self.multisig.approve_as.clone());
        let plan = rule::foot(
            &rule::FootFacts {
                chain_id: item.chain_id,
                hash: &item.safe_tx_hash,
                status: item.status,
                readable: item.review.action.is_some(),
                approved: item.approved,
                threshold: item.threshold,
                approved_by: &approved_by,
                can_approve: &item.can_approve,
                can_execute: &item.can_execute,
                previews: &view.previews,
                approve_execute: &view.approve_execute,
                last_actor: view.last_actor.as_deref(),
            },
            picked.as_deref(),
        );
        if plan.candidates.len() > 1
            && let Some(actor) = &plan.actor
        {
            foot = foot.child(self.ms_member_picker(
                theme,
                "ms-actor",
                &plan.candidates,
                actor,
                account,
                cx,
            ));
        }
        match (&plan.kind, plan.actor.clone()) {
            (FootKind::Approve, Some(actor)) => {
                foot = foot.child(self.ms_approve_press(
                    theme,
                    item,
                    actor,
                    value_variant(sign),
                    rule::approve_label(sign),
                    door.enabled && !executing,
                    approving,
                    cx,
                ));
            }
            (
                FootKind::Execute {
                    look,
                    approve_first,
                    can_approve_only,
                    one_tap,
                },
                Some(actor),
            ) => {
                let payer = item
                    .can_execute
                    .iter()
                    .find(|e| {
                        matches!(
                            rule::look_of(&view.previews, item.chain_id, &item.safe_tx_hash, e),
                            Some(rule::Look::Priced { can_pay: true, .. })
                        )
                    })
                    .cloned();
                let blocked = look.as_ref().is_some_and(rule::Look::blocks);
                let mut presses = div().flex().gap(px(8.));
                if blocked {
                    // It cannot run by this executor: approving is the one
                    // thing left to do.
                    if *approve_first && *can_approve_only && look != &Some(rule::Look::Spent) {
                        presses = presses.child(div().flex_1().min_w(px(0.)).child(
                            self.ms_approve_press(
                                theme,
                                item,
                                actor.clone(),
                                value_variant(sign),
                                rule::approve_label(sign),
                                door.enabled && !executing,
                                approving,
                                cx,
                            ),
                        ));
                    }
                } else {
                    if *approve_first && *can_approve_only {
                        // Executing pays the fee; approving alone does not.
                        // As tall as the press beside it: one row of two answers.
                        presses = presses.child(
                            div().flex_1().min_w(px(0.)).child(
                                self.ms_approve_press(
                                    theme,
                                    item,
                                    actor.clone(),
                                    crate::ui::ButtonVariant::Secondary,
                                    rule::approve_label(sign),
                                    door.enabled && !executing,
                                    approving,
                                    cx,
                                )
                                .min_h(px(crate::theme::BTN_H_PRIMARY)),
                            ),
                        );
                    }
                    let enabled =
                        rule::execute_enabled(door, look.as_ref(), *approve_first, *one_tap)
                            && !approving;
                    let state = if executing {
                        crate::ui::ButtonState::Busy
                    } else {
                        crate::ui::ButtonState::from_enabled(enabled)
                    };
                    let (chain_id, hash, one_tap) =
                        (item.chain_id, item.safe_tx_hash.clone(), *one_tap);
                    let executor = actor.clone();
                    presses = presses.child(div().flex_1().min_w(px(0.)).child(
                        crate::ui::vela_button_state(
                            "ms-execute",
                            value_variant(sign),
                            words::t(&loc, rule::execute_label(*approve_first, sign)).into(),
                            state,
                            theme,
                            cx.listener(move |this, _, _, cx| {
                                let host = this.ms_host(cx);
                                let (hash, executor) = (hash.clone(), executor.clone());
                                // "Approve and execute" is the machine's one press:
                                // stopped before its signature, the approval is
                                // still given and kept (r4-11).
                                let event = if one_tap {
                                    ms_proposals::Event::ApproveExecute {
                                        chain_id,
                                        safe_tx_hash: hash,
                                        executor,
                                    }
                                } else {
                                    ms_proposals::Event::Execute {
                                        chain_id,
                                        safe_tx_hash: hash,
                                        executor,
                                    }
                                };
                                host.update(cx, |host, cx| host.proposals(event, cx));
                            }),
                        ),
                    ));
                }
                foot = foot.child(presses);
                // r11 one-press: a press the look stopped says why in its own
                // line (`ActionState::Stopped`), which replaces the fee line —
                // never the two side by side.
                let stopped = matches!(
                    view.actions.get(&format!(
                        "execute:{}",
                        item.safe_tx_hash.to_ascii_lowercase()
                    )),
                    Some(ActionState::Stopped { .. })
                );
                if !stopped {
                    foot = foot.child(self.ms_look_line(
                        theme,
                        item,
                        &actor,
                        look.as_ref(),
                        payer.as_deref(),
                        account,
                        door.enabled,
                        cx,
                    ));
                }
            }
            _ => {}
        }
        // r9-9: the refusal is said once, by the review's own line above; a
        // review that names no reason still says nothing is signed. (One
        // being read, or that failed, says so in the review's place.)
        if door.shows == ReviewShows::Review
            && !door.enabled
            && review.is_some_and(|r| r.refused.is_none())
            && !matches!(plan.kind, FootKind::Nothing)
        {
            foot = foot.child(ms_note(
                theme,
                &words::t(&loc, "review.blocked"),
                Tone::Danger,
            ));
        }

        // r10 S4: this device's members have done their part (none of them
        // can approve or execute it now) and approvals are still missing —
        // what this person can do next is hand the link on to the members
        // still to approve: the main press. Which link (Vela's, or the
        // outside-wallet one) is the core's split (`signers`).
        let next_is_asking = matches!(plan.kind, FootKind::Nothing)
            && !item.members_here.is_empty()
            && item.approved < item.threshold
            && !executing
            && !nobody_approves(item);
        self.multisig.foot_link = None;
        if next_is_asking
            && let Some(account) = account
            && let Some((link, outside_link)) =
                self.ms_ask_link(item, view.signers.get(&key), account, cx)
        {
            let copied = if outside_link {
                self.multisig.outside_copied
            } else {
                self.multisig.copied
            };
            self.multisig.foot_link = Some(link.clone());
            let label: SharedString = words::t(
                &loc,
                if copied {
                    "proposal.copied"
                } else {
                    "proposal.copyLink"
                },
            )
            .into();
            // Handing a link on moves no value: the neutral ink, never the
            // accent (the web's `primary`).
            foot = foot.child(
                div()
                    .id("ms-ask-copy")
                    .a11y_button(label.clone())
                    .cursor_pointer()
                    .child(crate::flows::components::ink_button(theme, label))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(link.clone()));
                        if outside_link {
                            this.multisig.outside_copied = true;
                        } else {
                            this.multisig.copied = true;
                        }
                        cx.notify();
                    })),
            );
        }

        // An outside member's wallet, from here only when no member here
        // can act (ms-21): beside each outside member's row otherwise
        // (r7-8 — never as if this device's member signed elsewhere). Once
        // this device's own members have acted it is not the main press —
        // asking the others is (r10 S4) — but still a way, quietly.
        let outside = matches!(plan.kind, FootKind::Nothing)
            && !crate::multisig::sheet::outside_members(view.signers.get(&key), &|_| false)
                .is_empty();
        if outside && next_is_asking {
            let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
            foot = foot.child(
                text_action(
                    "ms-outside",
                    theme,
                    &mut self.icons,
                    None,
                    words::t(&loc, "external.signWithWallet").into(),
                )
                .on_click(cx.listener(move |this, _, _, cx| {
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
                })),
            );
        } else if outside {
            foot = foot.child(self.ms_outside_press(
                theme,
                item,
                crate::ui::ButtonVariant::Secondary,
                cx,
            ));
        }

        // Nothing to send again while it is on its way (r8-16).
        if view
            .outbox
            .get(&key)
            .is_some_and(|e| !(e.sending || e.why == "sending"))
        {
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
        // r9-9: a member of it on this device, whatever the transaction is
        // (the core's `members_here`).
        if reject_offered(item, executing) {
            if self.multisig.rejecting {
                let (chain_id, nonce) = (item.chain_id, item.nonce);
                foot = foot
                    .child(ms_note(
                        theme,
                        &words::tc(
                            &loc,
                            "proposal.rejectHint",
                            usize::try_from(item.threshold).unwrap_or(usize::MAX),
                        ),
                        Tone::Neutral,
                    ))
                    .child(
                        div()
                            .flex()
                            .gap(px(8.))
                            .child(crate::ui::vela_button(
                                "ms-reject-cancel",
                                crate::ui::ButtonVariant::Secondary,
                                words::t(&loc, "proposal.rejectCancel").into(),
                                theme,
                                cx.listener(|this, _, _, cx| {
                                    let _ = this.multisig.reject_press(RejectPress::Back);
                                    cx.notify();
                                }),
                            ))
                            .child(crate::ui::vela_button(
                                "ms-reject-confirm",
                                crate::ui::ButtonVariant::Primary,
                                words::t(&loc, "proposal.reject").into(),
                                theme,
                                cx.listener(move |this, _, _, cx| {
                                    let press = RejectPress::Confirm { chain_id, nonce };
                                    let Some(event) = this.multisig.reject_press(press) else {
                                        return;
                                    };
                                    let host = this.ms_host(cx);
                                    host.update(cx, |host, cx| host.proposals(event, cx));
                                }),
                            )),
                    );
            } else {
                foot = foot.child(
                    destructive_text_button(
                        "ms-reject",
                        theme,
                        words::t(&loc, "proposal.reject").into(),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        let _ = this.multisig.reject_press(RejectPress::Ask);
                        cx.notify();
                    })),
                );
            }
        }
        Some(foot)
    }

    /// An approval by `owner`, in the review's words (r8-1: "Approve
    /// anyway" where the review says so).
    #[allow(clippy::too_many_arguments, reason = "one press, named parts")]
    fn ms_approve_press(
        &mut self,
        theme: &Theme,
        item: &ProposalItem,
        owner: String,
        variant: crate::ui::ButtonVariant,
        label: &'static str,
        enabled: bool,
        busy: bool,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let loc = self.ms_loc();
        let state = if busy {
            crate::ui::ButtonState::Busy
        } else {
            crate::ui::ButtonState::from_enabled(enabled)
        };
        let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
        crate::ui::vela_button_state(
            "ms-approve",
            variant,
            words::t(&loc, label).into(),
            state,
            theme,
            cx.listener(move |this, _, _, cx| {
                let host = this.ms_host(cx);
                let (hash, owner) = (hash.clone(), owner.clone());
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
        )
    }

    /// "Approve with another wallet": the machine's gate, then an outside
    /// member's wallet over a live session (`approve_outside`).
    fn ms_outside_press(
        &mut self,
        theme: &Theme,
        item: &ProposalItem,
        variant: crate::ui::ButtonVariant,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<Div> {
        let loc = self.ms_loc();
        let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
        crate::ui::vela_button(
            "ms-outside",
            variant,
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
        )
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
            // r13-3: the read the core names (the network, or both).
            ActionState::Unread { reason_key } => words::tv(
                loc,
                reason_key
                    .strip_prefix("home.multisig.")
                    .unwrap_or(reason_key),
                &[("network", &network(chain_id))],
            ),
            ActionState::NoMemberHere => words::t(loc, "proposal.notMember"),
            ActionState::SigningRefused { .. } | ActionState::SigningBlocked => {
                words::t(loc, "review.blocked")
            }
            ActionState::SigningUnchecked => words::tv(
                loc,
                "proposal.cantCheck",
                &[("network", &network(chain_id))],
            ),
            ActionState::RiskDeclined => return None,
            ActionState::Refused { reason: why } => {
                words::tv(loc, "proposal.refused", &[("reason", why)])
            }
            ActionState::MailboxRefused { .. } => reason("serviceRefused"),
            // The look stopped it before the passkey: why, as the look says.
            // r11 (core): by the look's kind — fails → wouldFail, unpriced →
            // feeUnpriced, unchecked → feeUnchecked; it stands in the fee
            // line's place.
            ActionState::Stopped { preview } => match preview.get("kind").and_then(Value::as_str) {
                Some("unpriced") => words::t(loc, "proposal.feeUnpriced"),
                Some("unchecked") => words::t(loc, "proposal.feeUnchecked"),
                _ => words::tv(
                    loc,
                    "proposal.wouldFail",
                    &[(
                        "reason",
                        &self.ms_why_words(preview.get("why").unwrap_or(preview), chain_id),
                    )],
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
    pub(crate) fn ms_failure_words(&self, detail: &str, chain_id: u64) -> String {
        use crate::multisig::live::{ShellFailure, read_failure};
        let loc = &self.loc;
        let reason = |key: &str| words::t(loc, &format!("proposal.reason.{key}"));
        match read_failure(detail) {
            Some(ShellFailure::Passkey {
                timed_out: true, ..
            }) => reason("passkeyTimedOut"),
            Some(ShellFailure::Passkey { failure, .. }) if failure == "cancelled" => {
                reason("passkeyClosed")
            }
            Some(ShellFailure::Passkey { .. }) => reason("passkeyMissing"),
            Some(ShellFailure::NotSent) => words::t(loc, "proposal.notSent"),
            Some(ShellFailure::CannotPay { need, have }) => {
                let info = words::token_info(chain_id, None);
                words::tv(
                    loc,
                    "proposal.whyShort",
                    &[
                        ("have", &words::amount_words(loc, &have, &info)),
                        ("need", &words::amount_words(loc, &need, &info)),
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

    /// §12: what stands in the review's place, as its door says — the review
    /// itself; the calm "Reading the transaction…" while it is read; or why
    /// it could not be read, "Nothing can be signed until it's read" and
    /// "Try again" (the machine's `review_again`: `proposal` names the
    /// proposal's hash, `None` the draft's). Never a sign press here: the
    /// presses follow the same door, shut in the last two.
    #[allow(clippy::too_many_arguments, reason = "the sheet's facts")]
    fn ms_review_place(
        &mut self,
        theme: &Theme,
        review: Option<&ReviewDraw>,
        door: &SignDoor,
        chain_id: u64,
        proposal: Option<&str>,
        past: bool,
        account: Option<&MultisigAccount>,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        if door.shows == ReviewShows::Review
            && let Some(review) = review
        {
            let body = self.ms_review_body(theme, review, chain_id, past, account, cx);
            // D1 (real-device pass 2): drawn, but neither its state-change
            // check nor its simulation was read — the core's danger line
            // and Try again above it; every press is "… anyway".
            if past || !crate::multisig::sheet::unchecked(door) {
                return body;
            }
            let line = door
                .reason_key
                .as_deref()
                .unwrap_or(review_state::UNCHECKED_KEY);
            let again = crate::multisig::sheet::review_again(proposal.map(|h| (chain_id, h)));
            let mut head = div().flex().flex_col().gap(px(8.)).child(ms_note(
                theme,
                &loc.t(line),
                Tone::Danger,
            ));
            if door.retry {
                head = head.child(
                    div().flex().child(
                        crate::ui::vela_button(
                            "ms-review-unchecked-again",
                            crate::ui::ButtonVariant::Secondary,
                            loc.t(review_state::TRY_AGAIN_KEY).to_string().into(),
                            theme,
                            cx.listener(move |this, _, _, cx| {
                                let host = this.ms_host(cx);
                                let event = again.clone();
                                host.update(cx, |host, cx| host.proposals(event, cx));
                            }),
                        )
                        .w_auto(),
                    ),
                );
            }
            return div().flex().flex_col().gap(px(12.)).child(head).child(body);
        }
        if door.shows == ReviewShows::Loading {
            return div().child(ms_note(
                theme,
                &loc.t(review_state::LOADING_KEY),
                Tone::Neutral,
            ));
        }
        // Failed (or a review said read that is not here to draw: never
        // signed on, said unreadable).
        let reason = door
            .reason_key
            .as_deref()
            .unwrap_or(review_state::REASON_UNREADABLE);
        let again = crate::multisig::sheet::review_again(proposal.map(|h| (chain_id, h)));
        div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(ms_note(theme, &loc.t(reason), Tone::Caution))
            .child(ms_note(
                theme,
                &loc.t(review_state::BLOCKED_KEY),
                Tone::Caution,
            ))
            .child(
                div().flex().child(
                    crate::ui::vela_button(
                        "ms-review-again",
                        crate::ui::ButtonVariant::Secondary,
                        loc.t(review_state::TRY_AGAIN_KEY).to_string().into(),
                        theme,
                        cx.listener(move |this, _, _, cx| {
                            let host = this.ms_host(cx);
                            let event = again.clone();
                            host.update(cx, |host, cx| host.proposals(event, cx));
                        }),
                    )
                    .w_auto(),
                ),
            )
    }

    /// The one review every signer signs from (§3.6): what it says, who it
    /// reaches, what moves, the state-diff guard, a member change's rule,
    /// and the hashes — drawn, never decoded again here. Each thing said
    /// once (r8-13): the review's blocks, then only a concern no block
    /// says. `past` (executed): nothing about what running it WOULD do.
    fn ms_review_body(
        &mut self,
        theme: &Theme,
        review: &ReviewDraw,
        chain_id: u64,
        past: bool,
        account: Option<&MultisigAccount>,
        cx: &mut Context<Self>,
    ) -> Div {
        let loc = self.ms_loc();
        let net = network(chain_id);
        let mut col = div().flex().flex_col().gap(px(12.));

        // r9-9: why nothing is signed — the core's one line, said once here
        // (the foot says nothing more), never an English reason of its own.
        if let Some(refused) = review.refused.as_ref().filter(|_| !past) {
            col = col.child(ms_note(theme, &loc.t(&refused.key), Tone::Danger));
        }

        // A member change, read before anything else (`MemberChangeRule`):
        // the caution, each change with its address whole, the rule it meets
        // and the one it leaves ("Approvals: 2 of 3 → 2 of 4").
        if let Some(changes) = &review.member_changes {
            let lines =
                crate::multisig::member_words::change_lines(&loc, changes, &net, &|a: &str| {
                    self.ms_member_name(a, account, cx)
                });
            if !past {
                col = col.child(ms_note(theme, &lines.caution, Tone::Caution));
            }
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
                                .child(crate::ui::address_text(a))
                        })),
                );
            }
            col = col.child(ms_note(
                theme,
                &lines.rule,
                if past { Tone::Neutral } else { Tone::Caution },
            ));
            if let Some(misfit) = lines.does_not_fit.as_ref().filter(|_| !past) {
                col = col.child(ms_note(theme, misfit, Tone::Danger));
            }
        }

        // What each call does, as the core read it (r8-1, r8-2, r8-5, r9-1):
        // the concern lines first (an unlimited grant, a nested batch, one
        // that cannot be read, decimals unknown — each once), then the lone
        // call or every leg of every batch. Nothing decoded here.
        let mut drawn_parties: Vec<String> = Vec::new();
        if let Some(draw) = &review.reading {
            let ctx = self.ms_reading_ctx(chain_id);
            // r11 (deep4 "two red boxes"): a review Vela refuses says why in
            // ONE line — the refusal's, above; the concern lines (a nested
            // batch, …) would be a second box for a transaction nobody signs.
            let lines = if past || review.refused.is_some() {
                Vec::new()
            } else {
                // Beside a refusal the core has left out the concern it
                // says (one refusal box); every concern here is drawn.
                crate::multisig::reading::concern_blocks(draw, &ctx)
            };
            let blocks = crate::multisig::reading::blocks(&draw.reading, &ctx);
            drawn_parties = crate::multisig::reading::party_addresses(&blocks);
            for (i, block) in lines.iter().chain(&blocks).enumerate() {
                col = col.child(self.ms_reading_block(theme, i, block, review, past, account, cx));
            }
        }

        // The state-diff guard first (the Bybit lesson) — said once.
        if let Some(state) = review.state.as_ref().filter(|_| !past) {
            use vela_core::multisig_review::StateVerdict;
            let said = match &state.verdict {
                StateVerdict::Unchanged => Some((
                    words::tv(&loc, "review.stateUnchanged", &[("network", &net)]),
                    Tone::Neutral,
                )),
                StateVerdict::Declared => Some((
                    words::tv(&loc, "review.stateDeclared", &[("network", &net)]),
                    Tone::Neutral,
                )),
                StateVerdict::Undeclared { writes } => {
                    let mut lines: Vec<String> = Vec::new();
                    for line in writes.iter().map(|w| ms_write_line(&loc, w)) {
                        if !lines.contains(&line) {
                            lines.push(line);
                        }
                    }
                    Some((
                        format!(
                            "{}\n{}\n{}",
                            words::t(&loc, "review.stateTitle"),
                            words::tv(&loc, "review.stateBody", &[("network", &net)]),
                            lines.join("\n")
                        ),
                        Tone::Danger,
                    ))
                }
                // The verdict's place says it fails; nothing would change.
                // r9-9: refused for another reason — said once, below.
                StateVerdict::Reverts | StateVerdict::Withheld => None,
                StateVerdict::Unavailable { .. } => Some((
                    words::tv(&loc, "review.stateUnread", &[("network", &net)]),
                    Tone::Caution,
                )),
            };
            if let Some((text, tone)) = said {
                col = col.child(ms_note(theme, &text, tone));
            }
        }

        // Who it reaches, each with what the review knows of it (its cue).
        let mut parties = div().flex().flex_col().gap(px(6.));
        for (i, r) in review.recipients.iter().enumerate() {
            let role = match r.role {
                RecipientRole::Recipient => words::t(&loc, "review.roleRecipient"),
                RecipientRole::Contract => words::t(&loc, "review.roleContract"),
                RecipientRole::Member => words::t(&loc, "review.roleMember"),
            };
            let role = match r.call {
                Some(n) => format!(
                    "{} · {}",
                    words::tv(&loc, "review.callNumber", &[("n", &n.to_string())]),
                    role
                ),
                None => role,
            };
            let name = r
                .known
                .as_ref()
                .map(|k| k.label.clone())
                .unwrap_or_else(|| self.ms_member_name(&r.address, account, cx));
            // The reading names this one already (its party row, r5reg-15):
            // its notes below are still said, its row not twice.
            if !drawn_parties.contains(&r.address.to_ascii_lowercase()) {
                let badge = self.ms_recipient_badge(r, past);
                parties = parties.child(ms_party_row(
                    theme,
                    ElementId::from(("ms-party", i)),
                    &role,
                    name,
                    badge,
                    &r.address,
                ));
            }
            if !past {
                if let Some(like) = &r.lookalike {
                    parties = parties.child(ms_note(
                        theme,
                        &words::tv(
                            &loc,
                            "review.lookalike",
                            &[
                                ("address", &r.address),
                                ("label", &like.label),
                                ("known", &like.address),
                            ],
                        ),
                        Tone::Danger,
                    ));
                }
                if r.contract_cue {
                    parties = parties.child(ms_note(
                        theme,
                        &words::tv(&loc, "review.toContract", &[("address", &r.address)]),
                        Tone::Caution,
                    ));
                }
            }
        }
        col = col.child(parties);

        if !past {
            // What moves (the simulation, or the probe's balances) — nothing
            // when the core withholds it beside a refusal.
            if let Some(sim) = ms_moves(review) {
                if let Some((key, tone)) = ms_moves_line(sim) {
                    let text = self
                        .loc
                        .t_texts(
                            key,
                            &[("reason", sim.revert_reason.as_deref().unwrap_or_default())],
                        )
                        .to_string();
                    col = col.child(ms_note(theme, &text, tone));
                }
                // r10 S4: the lines below are headed, as on the web, the
                // outside page and the phones — a bare "−0.01 USDC" did not
                // say whose balance it was or that it was a change.
                // r11: the heading's key is the review's (`moves_title_key`,
                // none when what moves is withheld).
                if !sim.deltas.is_empty()
                    && let Some(key) = review.moves_title_key.as_deref()
                {
                    col = col.child(ms_label(theme, &self.loc.t(key)));
                }
                for (i, delta) in sim.deltas.iter().enumerate() {
                    let info =
                        words::token_facts(&self.multisig.tokens, chain_id, delta.token.as_deref());
                    let (sign, raw) = match delta.delta.strip_prefix('-') {
                        Some(raw) => ("−", raw),
                        None => ("+", delta.delta.as_str()),
                    };
                    col = col.child(
                        div()
                            .id(ElementId::from(("ms-delta", i)))
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(format!("{sign}{}", words::amount_words(&loc, raw, &info))),
                    );
                }
                if review.simulation_source == "probe" && sim.kind == "deltas" {
                    col = col.child(ms_note(
                        theme,
                        &words::t(&loc, "review.movesProbe"),
                        Tone::Neutral,
                    ));
                }
            }
        }

        // The technical details: the SafeTx as signed and its hashes.
        let open = self.multisig.tech;
        col = col.child(
            text_action(
                "ms-tech",
                theme,
                &mut self.icons,
                Some(if open {
                    Icon::ChevronDown
                } else {
                    Icon::ChevronRight
                }),
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

    /// A recipient's badge (`recipientBadge`): the review's cues (first
    /// time, a contract), else whose it is; nothing on one executed.
    fn ms_recipient_badge(&self, r: &Recipient, past: bool) -> Option<(String, Tone)> {
        if past {
            return None;
        }
        let loc = &self.loc;
        let tags: Vec<String> = [
            r.first_time_cue
                .then(|| loc.t("componentsUi.signing.firstTimeTag").to_string()),
            r.contract_cue
                .then(|| loc.t("componentsUi.signing.contractTag").to_string()),
        ]
        .into_iter()
        .flatten()
        .collect();
        if !tags.is_empty() {
            return Some((
                tags.join(" · "),
                if r.lookalike.is_some() {
                    Tone::Danger
                } else {
                    Tone::Caution
                },
            ));
        }
        match r.known.as_ref().map(|k| k.kind) {
            Some(vela_core::multisig_review::KnownKind::Own) => {
                Some((words::t(loc, "review.knownOwn"), Tone::Neutral))
            }
            Some(vela_core::multisig_review::KnownKind::Member)
                if r.role != RecipientRole::Member =>
            {
                Some((words::t(loc, "review.knownMember"), Tone::Neutral))
            }
            _ => None,
        }
    }

    /// One block of the core's reading, drawn (r9-1): the lone call's
    /// verb, its facts, who it reaches (with the name and cue the review
    /// knows for that address), a leg's card, a batch's runner note, a
    /// concern's line.
    #[allow(clippy::too_many_arguments, reason = "the sheet's facts")]
    fn ms_reading_block(
        &self,
        theme: &Theme,
        i: usize,
        block: &crate::multisig::reading::Block,
        review: &ReviewDraw,
        past: bool,
        account: Option<&MultisigAccount>,
        cx: &App,
    ) -> gpui::AnyElement {
        use crate::multisig::reading::{Block, Row, Tone as T};
        let tone = |t: T| match t {
            T::Neutral => Tone::Neutral,
            T::Caution => Tone::Caution,
            T::Danger => Tone::Danger,
        };
        let ink = |t: T| match t {
            T::Neutral => theme.fg_base,
            T::Caution => theme.fg_base,
            T::Danger => theme.error_base,
        };
        let rows = |rows: &[Row]| {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(rows.iter().map(|r| {
                    let value = div().text_color(if r.tone == T::Danger {
                        theme.error_base
                    } else {
                        theme.fg_base
                    });
                    let value = if r.mono {
                        value
                            .font_family(theme::font_mono())
                            .text_size(theme::text_label())
                    } else {
                        value
                    };
                    div()
                        .flex()
                        .justify_between()
                        .items_start()
                        .gap(px(12.))
                        .text_size(theme::text_row_sub())
                        .child(
                            div()
                                .flex_none()
                                .text_color(theme.fg_subtle)
                                .child(r.label.clone()),
                        )
                        .child(value.min_w(px(0.)).text_right().child(r.value.clone()))
                }))
        };
        match block {
            Block::Intent { text, tone: t } => div()
                .id(ElementId::from(("ms-reading", i)))
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(ink(*t))
                .child(text.clone())
                .into_any_element(),
            Block::Rows(r) => rows(r).into_any_element(),
            Block::Party { label, address } => {
                let recipient = review.recipients.iter().find(|r| same(&r.address, address));
                let name = recipient
                    .and_then(|r| r.known.as_ref().map(|k| k.label.clone()))
                    .unwrap_or_else(|| self.ms_member_name(address, account, cx));
                let badge = recipient.and_then(|r| self.ms_recipient_badge(r, past));
                ms_party_row(
                    theme,
                    ElementId::from(("ms-reading", i)),
                    label,
                    name,
                    badge,
                    address,
                )
                .into_any_element()
            }
            Block::Card {
                title,
                rows: r,
                tone: t,
            } => {
                let edge = match t {
                    T::Danger => theme.error_base,
                    T::Caution => theme.warning_base,
                    T::Neutral => theme.divider,
                };
                div()
                    .id(ElementId::from(("ms-reading", i)))
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(10.))
                    .border_1()
                    .border_color(edge)
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(ink(*t))
                            .child(title.clone()),
                    )
                    .child(rows(r))
                    .into_any_element()
            }
            Block::Source { text, tone: t } | Block::Warning { text, tone: t } => {
                ms_note(theme, text, tone(*t)).into_any_element()
            }
        }
    }

    /// The approvals in, then each member still to approve and how it signs
    /// (r7-8, the machine's `signers`): a member on this device waits for
    /// its own press here — never "another wallet"; an outside wallet's row
    /// offers "Approve with another wallet"; one nothing says yet, both.
    #[allow(clippy::too_many_arguments, reason = "the sheet's facts")]
    fn ms_approvals(
        &mut self,
        theme: &Theme,
        item: &ProposalItem,
        signers: Option<&ms_proposals::ItemSigners>,
        frozen: Option<&vela_core::app::multisig_presence::FrozenFacts>,
        account: Option<&MultisigAccount>,
        cx: &mut Context<Self>,
    ) -> Div {
        use crate::multisig::sheet::{MemberWay, member_way};
        let loc = self.ms_loc();
        // Executed: what counted when it ran (the core's frozen facts — an
        // executor's own approval by sending it included), never the list
        // as it reads after (r8-13: "1/2 approved" over an executed one).
        let done = item.status == ms_proposals::ProposalStatus::Done;
        let ran = frozen.filter(|_| done);
        let (shown_approved, shown_threshold) =
            crate::multisig::sheet::approvals_shown(done, frozen, item.approved, item.threshold);
        let mut col = div().flex().flex_col().gap(px(4.)).child(ms_label(
            theme,
            &format!(
                "{} · {}",
                words::t(&loc, "proposal.approvals"),
                words::tv(
                    &loc,
                    "status.progress",
                    &[
                        ("approved", &shown_approved.to_string()),
                        ("threshold", &shown_threshold.to_string())
                    ]
                )
            ),
        ));
        let local: Vec<String> = session::view(cx)
            .accounts
            .iter()
            .map(|row| row.account.address.clone())
            .collect();
        let here = |a: &str| local.iter().any(|l| same(l, a));
        let row = |id: (&'static str, usize), name: String, state: gpui::AnyElement| {
            div()
                .id(ElementId::from(id))
                .flex()
                .justify_between()
                .items_center()
                .gap(px(8.))
                .text_size(theme::text_row_sub())
                .child(div().min_w(px(0.)).text_color(theme.fg_base).child(name))
                .child(div().flex_none().child(state))
        };
        let quiet = |text: String| {
            div()
                .text_color(theme.fg_subtle)
                .child(text)
                .into_any_element()
        };
        let mark = |name: String, address: &str| {
            if here(address) {
                format!("{name} · {}", words::t(&loc, "panel.thisDevice"))
            } else {
                name
            }
        };
        for (i, approval) in item.approvals.iter().enumerate() {
            let name = mark(
                self.ms_member_name(&approval.owner, account, cx),
                &approval.owner,
            );
            let note = if approval.counts {
                words::t(&loc, "timeline.counted")
            } else {
                words::t(&loc, "timeline.notCounted")
            };
            col = col.child(row(("ms-approval", i), name, quiet(note)));
        }
        if done {
            if let Some(f) = ran {
                // Who approved by sending it, beside the signatures.
                for (i, approver) in f
                    .approvers
                    .iter()
                    .filter(|a| !item.approvals.iter().any(|x| same(&x.owner, a)))
                    .enumerate()
                {
                    let name = mark(self.ms_member_name(approver, account, cx), approver);
                    let note = if f.executor.as_deref().is_some_and(|e| same(e, approver))
                        && f.executor_approved
                    {
                        words::t(&loc, "proposal.executed")
                    } else {
                        words::t(&loc, "timeline.counted")
                    };
                    col = col.child(row(("ms-approval-ran", i), name, quiet(note)));
                }
                let mut facts: Vec<String> = Vec::new();
                if let Some(executor) = &f.executor {
                    facts.push(words::tv(
                        &loc,
                        "proposal.executedBy",
                        &[("name", &self.ms_member_name(executor, account, cx))],
                    ));
                }
                facts.push(words::tv(
                    &loc,
                    "proposal.countedThen",
                    &[
                        ("approved", &f.approvers.len().to_string()),
                        ("threshold", &f.threshold.to_string()),
                    ],
                ));
                col = col.child(
                    div()
                        .pt(px(4.))
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(facts.join(" · ")),
                );
            } else {
                col = col.child(
                    div()
                        .pt(px(4.))
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(words::t(&loc, "proposal.approvalsSigned")),
                );
            }
            return col;
        }
        for (i, signer) in signers
            .map(|s| s.rows.clone())
            .unwrap_or_default()
            .iter()
            .enumerate()
        {
            let name = mark(
                self.ms_member_name(&signer.owner, account, cx),
                &signer.owner,
            );
            let way = member_way(signers, &signer.owner, here(&signer.owner));
            let enough = signers.is_some_and(|s| s.enough);
            let state = match way {
                _ if enough => quiet(words::t(&loc, "proposal.notNeeded")),
                MemberWay::Outside | MemberWay::Either => {
                    let (chain_id, hash) = (item.chain_id, item.safe_tx_hash.clone());
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(quiet(words::t(&loc, "proposal.waiting")))
                        .child(
                            text_action(
                                ElementId::from(("ms-outside-row", i)),
                                theme,
                                &mut self.icons,
                                None,
                                words::t(&loc, "external.signWithWallet").into(),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
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
                                },
                            )),
                        )
                        .into_any_element()
                }
                MemberWay::Contract => quiet(format!(
                    "{} · {}",
                    words::t(&loc, "proposal.waiting"),
                    words::t(&loc, "members.kindContract")
                )),
                MemberWay::InVela | MemberWay::Waiting => quiet(words::t(&loc, "proposal.waiting")),
            };
            col = col.child(row(("ms-signer", i), name, state));
        }
        col
    }

    /// The links the members still to approve open it with (r7-8, r4C-13):
    /// the Vela link for Vela members and those nothing says yet (never
    /// this device's), the outside-wallet link for an outside wallet — each
    /// only while approvals are missing; a smart contract account is said.
    /// The link the members still to approve open it with, as the sheet's
    /// section offers it ([`Self::ms_invite_links`]): Vela's for Vela
    /// members elsewhere, those nothing says yet, or while they are read;
    /// else the outside-wallet link (`true`). `None`: none to hand on (only
    /// smart contract accounts are left).
    fn ms_ask_link(
        &self,
        item: &ProposalItem,
        signers: Option<&ms_proposals::ItemSigners>,
        account: &MultisigAccount,
        cx: &App,
    ) -> Option<(String, bool)> {
        let local: Vec<String> = session::view(cx)
            .accounts
            .iter()
            .map(|row| row.account.address.clone())
            .collect();
        let here = |a: &str| local.iter().any(|l| same(l, a));
        let of = |list: Option<&Vec<String>>| -> Vec<String> {
            list.map(|l| l.iter().filter(|a| !here(a)).cloned().collect())
                .unwrap_or_default()
        };
        let in_vela = of(signers.map(|s| &s.in_vela));
        let either = of(signers.map(|s| &s.either));
        let contract = of(signers.map(|s| &s.contract));
        let outside = crate::multisig::sheet::outside_members(signers, &here);
        let tx = vela_core::multisig_link::ProposalLink {
            chain_id: item.chain_id,
            multisig: account.address.clone(),
            safe_tx_hash: item.safe_tx_hash.clone(),
        };
        if !in_vela.is_empty() || !either.is_empty() || (outside.is_empty() && contract.is_empty())
        {
            return Some((
                vela_core::multisig_link::proposal_link(
                    vela_core::multisig_link::SITE_ORIGIN,
                    self.loc.language(),
                    &tx,
                ),
                false,
            ));
        }
        if outside.is_empty() {
            return None;
        }
        let page = format!(
            "{}/{}/connect",
            vela_core::multisig_link::share_origin(vela_core::multisig_link::SITE_ORIGIN),
            self.loc.language()
        );
        Some((
            vela_core::multisig_link::connect_link(&page, None, &outside, Some(&tx)),
            true,
        ))
    }

    fn ms_invite_links(
        &mut self,
        theme: &Theme,
        item: &ProposalItem,
        signers: Option<&ms_proposals::ItemSigners>,
        account: &MultisigAccount,
        cx: &mut Context<Self>,
    ) -> Vec<Div> {
        let mut out = Vec::new();
        if item.status == ms_proposals::ProposalStatus::Done
            || item.approved >= item.threshold
            || item.executing
        {
            return out;
        }
        // r11 S4: a transaction Vela refuses for what it is (a delegatecall,
        // a refund, a batch too deep) has nobody who can approve it at the
        // source (`can_approve` empty): no member is asked to.
        if nobody_approves(item) {
            return out;
        }
        let loc = self.ms_loc();
        let local: Vec<String> = session::view(cx)
            .accounts
            .iter()
            .map(|row| row.account.address.clone())
            .collect();
        let here = |a: &str| local.iter().any(|l| same(l, a));
        let of = |list: Option<&Vec<String>>| -> Vec<String> {
            list.map(|l| l.iter().filter(|a| !here(a)).cloned().collect())
                .unwrap_or_default()
        };
        let in_vela = of(signers.map(|s| &s.in_vela));
        let either = of(signers.map(|s| &s.either));
        let contract = of(signers.map(|s| &s.contract));
        let outside = crate::multisig::sheet::outside_members(signers, &here);
        let names = |this: &Self, list: &[String], cx: &App| {
            loc.list(
                &list
                    .iter()
                    .map(|a| this.ms_member_name(a, Some(account), cx))
                    .collect::<Vec<_>>(),
            )
        };
        let tx = vela_core::multisig_link::ProposalLink {
            chain_id: item.chain_id,
            multisig: account.address.clone(),
            safe_tx_hash: item.safe_tx_hash.clone(),
        };
        // The Vela link: Vela members elsewhere, nothing-says-yet ones, and
        // those still being read.
        if !in_vela.is_empty() || !either.is_empty() || (outside.is_empty() && contract.is_empty())
        {
            let link = vela_core::multisig_link::proposal_link(
                vela_core::multisig_link::SITE_ORIGIN,
                self.loc.language(),
                &tx,
            );
            let copied = self.multisig.copied;
            let mut section = div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .child(ms_label(theme, &words::t(&loc, "proposal.invite")))
                .child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(crate::ui::prose(words::t(&loc, "proposal.inviteHint"))),
                );
            if !in_vela.is_empty() {
                section = section.child(ms_note(
                    theme,
                    &words::tv(
                        &loc,
                        "proposal.approvesInVela",
                        &[("names", &names(self, &in_vela, cx))],
                    ),
                    Tone::Neutral,
                ));
            }
            if !either.is_empty() {
                section = section.child(ms_note(
                    theme,
                    &words::tv(
                        &loc,
                        "proposal.eitherWay",
                        &[("names", &names(self, &either, cx))],
                    ),
                    Tone::Neutral,
                ));
            }
            let in_foot = self.multisig.foot_link.as_deref() == Some(link.as_str());
            section = section.when(!in_foot, |section| {
                section.child(
                    outline_button(
                        "ms-copy-link",
                        theme,
                        &mut self.icons,
                        Some(Icon::Copy),
                        words::t(
                            &loc,
                            if copied {
                                "proposal.copied"
                            } else {
                                "proposal.copyLink"
                            },
                        )
                        .into(),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(link.clone()));
                        this.multisig.copied = true;
                        cx.notify();
                    })),
                )
            });
            out.push(section);
        }
        if !contract.is_empty() {
            out.push(ms_note(
                theme,
                &words::tv(
                    &loc,
                    "proposal.contractMembers",
                    &[("names", &names(self, &contract, cx))],
                ),
                Tone::Caution,
            ));
        }
        // The outside-wallet link: it reads, reviews and signs from the
        // connect page alone — never the Vela link a wallet can't use.
        if !outside.is_empty() {
            let page = format!(
                "{}/{}/connect",
                vela_core::multisig_link::share_origin(vela_core::multisig_link::SITE_ORIGIN),
                self.loc.language()
            );
            let link = vela_core::multisig_link::connect_link(&page, None, &outside, Some(&tx));
            let copied = self.multisig.outside_copied;
            let in_foot = self.multisig.foot_link.as_deref() == Some(link.as_str());
            out.push(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(ms_label(
                        theme,
                        &words::t(&loc, "external.outsideLinkTitle"),
                    ))
                    .child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(crate::ui::prose(words::t(&loc, "external.outsideLinkHint"))),
                    )
                    .when(!in_foot, |section| {
                        section.child(
                            outline_button(
                                "ms-copy-outside-link",
                                theme,
                                &mut self.icons,
                                Some(Icon::Copy),
                                words::t(
                                    &loc,
                                    if copied {
                                        "proposal.copied"
                                    } else {
                                        "proposal.copyLink"
                                    },
                                )
                                .into(),
                            )
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                        link.clone(),
                                    ));
                                    this.multisig.outside_copied = true;
                                    cx.notify();
                                },
                            )),
                        )
                    }),
            );
        }
        out
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
                TimelineStep::Created { who, .. } => words::tv(
                    &loc,
                    "timeline.created",
                    &[("name", &self.ms_member_name(who, account, cx))],
                ),
                TimelineStep::Approved {
                    who,
                    by_executing,
                    counted,
                    ..
                } => {
                    let key = if *by_executing {
                        "timeline.approvedByExecuting"
                    } else {
                        "timeline.approved"
                    };
                    let mut line = words::tv(
                        &loc,
                        key,
                        &[("name", &self.ms_member_name(who, account, cx))],
                    );
                    if let Some(counted) = counted {
                        line = format!(
                            "{line} · {}",
                            words::t(
                                &loc,
                                if *counted {
                                    "timeline.counted"
                                } else {
                                    "timeline.notCounted"
                                }
                            )
                        );
                    }
                    line
                }
                TimelineStep::Executing => words::t(&loc, "proposal.executing"),
                TimelineStep::CancelProposed { .. } => words::t(&loc, "proposal.cancelPending"),
                TimelineStep::Executed { who, .. } => match who {
                    Some(who) => words::tv(
                        &loc,
                        "timeline.executedBy",
                        &[("name", &self.ms_member_name(who, account, cx))],
                    ),
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
        // A link's stand (`multisig_links.rs`) is drawn once, whatever is up.
        self.ms_take_link(cx);
        self.ms_network_watch(cx);
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
        // A draft no flow above draws — a rejection's cancellation, drafted
        // from the sheet's "Reject" — on the same review-first dialog.
        if self.ms_loose_draft(cx) {
            return self.ms_draft_dialog(theme, window, None, cx);
        }
        // A link's own sheet.
        if self.multisig.link_sheet.is_some() {
            return self.ms_link_dialog(theme, window, cx);
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
            let value = self
                .multisig
                .pin
                .as_ref()
                .map(|(v, _)| v.clone())
                .unwrap_or_default();
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
                theme, &self.loc, &request, &value, &focus, window, on_change, on_confirm,
                on_cancel,
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
                move |index: &usize, _: &mut Window, _: &mut gpui::App| {
                    pick.answer_choice(Some(*index))
                },
                move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| {
                    cancel.answer_choice(None)
                },
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
                move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| {
                    cancel.cancel_touch()
                },
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
            || self.multisig.link_sheet.is_some()
            || !self.multisig.dapp.is_empty()
            || self.ms_loose_draft(cx)
    }

    /// The machine holds a draft that neither the send flow (its own
    /// proposing, `send_proposing`) nor a site's request draws: the sheet's
    /// rejection — its review is drawn all the same, before any passkey.
    fn ms_loose_draft(&self, cx: &App) -> bool {
        let Some(host) = ms_host::booted(cx) else {
            return false;
        };
        let host = host.read(cx);
        host.machines().proposals_view.draft.is_some()
            && !host.send_proposing()
            && self.multisig.dapp.is_empty()
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
            .children(
                findings
                    .into_iter()
                    .map(|f| ms_note(theme, &f, Tone::Danger)),
            )
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
                            crate::ui::ButtonVariant::Danger,
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
        // §12: the draft's review, and the one door "Propose" follows.
        let (review, door) = review_door(Some(&draft.review_state), draft.review.as_ref());
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
            .child(ms_kv(
                theme,
                &words::t(&loc, "action.network"),
                &network(item.chain_id),
            ))
            .child(ms_kv(
                theme,
                &words::t(&loc, "action.number"),
                &words::tv(
                    &loc,
                    "action.numberValue",
                    &[("nonce", &item.nonce.to_string())],
                ),
            ));
        body = body.child(self.ms_review_place(
            theme,
            review.as_ref(),
            &door,
            item.chain_id,
            None,
            false,
            account.as_ref(),
            cx,
        ));
        let as_member = self
            .multisig
            .approve_as
            .clone()
            .filter(|a| draft.members_here.iter().any(|m| same(m, a)))
            .or(draft.default_member.clone());
        if draft.members_here.len() > 1 {
            let picked = as_member.clone().unwrap_or_default();
            body = body.child(self.ms_member_picker(
                theme,
                "ms-draft-as",
                &draft.members_here,
                &picked,
                account.as_ref(),
                cx,
            ));
        }
        body = body.child(ms_note(
            theme,
            &words::t(&loc, "proposal.feeNote"),
            Tone::Neutral,
        ));
        // A site's proposal (or a rejection, which no flow follows) stays on
        // its review when it did not go: why.
        let loose = site.is_none() && !host.read(cx).send_proposing();
        if (site.is_some() || loose)
            && let Some(text) = view
                .actions
                .get("propose")
                .and_then(|state| self.ms_action_words(state, item.chain_id))
        {
            body = body.child(ms_note(theme, &text, Tone::Caution));
        }
        // Its words and fill as the door says them; pressable only through it.
        let sign = crate::multisig::sheet::press_rule(&door);
        let working = matches!(view.actions.get("propose"), Some(ActionState::Working));
        let leave = site.as_ref().map(|(rid, _)| rid.clone());
        // core-ms-2: a site's transaction its proposer's own approval makes
        // ready runs in the same press ("Approve and execute",
        // `propose_execute`) — the machine's `execute_now`, its fee said.
        let execute_now = draft.execute_now && !draft.members_here.is_empty();
        let label = if draft.members_here.is_empty() {
            words::t(&loc, "send.proposeOnly")
        } else if execute_now {
            words::t(&loc, crate::multisig::sheet::execute_label(true, sign))
        } else if sign.anyway {
            words::t(&loc, "review.proposeAnyway")
        } else {
            words::t(&loc, "send.propose")
        };
        let state = if working {
            crate::ui::ButtonState::Busy
        } else {
            crate::ui::ButtonState::from_enabled(door.enabled)
        };
        // r4D-19: the answers stay in view under a long review.
        let foot = div().pt(px(12.)).child(
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
                        value_variant(sign),
                        label.into(),
                        state,
                        theme,
                        cx.listener(move |this, _, _, cx| {
                            let host = this.ms_host(cx);
                            let as_member = as_member.clone();
                            host.update(cx, |host, cx| {
                                if execute_now {
                                    host.proposals(
                                        ms_proposals::Event::ProposeExecute { as_member },
                                        cx,
                                    );
                                } else {
                                    host.confirm_draft(as_member, cx);
                                }
                            });
                        }),
                    )
                    .w_auto(),
                ),
        );
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        Some(
            crate::ui::dialog::dialog_with_foot(
                "ms-draft",
                theme,
                window,
                words::t(&loc, "proposal.title").into(),
                None,
                close_icon,
                body,
                Some(foot.into_any_element()),
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
    fn ms_session_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
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
        let tx = view
            .tx
            .as_ref()
            .map(|t| vela_core::multisig_link::ProposalLink {
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
        body = body.child(crate::ui::dialog::dialog_body(
            theme,
            crate::multisig::session_words::phase(&loc, &view),
        ));
        if let Some(failure) = &view.failure {
            body = body.child(ms_note(
                theme,
                &crate::multisig::session_words::failure(&loc, failure),
                Tone::Caution,
            ));
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
                                cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                    copy.clone(),
                                ));
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
                                host.session(
                                    vela_core::app::multisig_outside::session::Event::AskAgain,
                                    cx,
                                );
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
pub(crate) enum Tone {
    Neutral,
    Caution,
    Danger,
}

/// What the review says moves, as drawn: `None` when the core withholds it
/// (`simulation.kind` `withheld` — refused for a reason other than the state
/// guard's, a delegatecall or a batch too deep): nothing in its place, never
/// "No asset changes" under "Vela won't sign this".
pub(crate) fn ms_moves(review: &ReviewDraw) -> Option<&SimVerdict> {
    review
        .simulation
        .as_ref()
        .filter(|sim| sim.kind != vela_core::multisig_review::SIM_WITHHELD)
}

/// The one line in the verdict's place: the notice (a revert in danger, a
/// could-not-check in caution) or the core's no-change line; `None` when
/// the deltas speak for themselves.
pub(crate) fn ms_moves_line(sim: &SimVerdict) -> Option<(&str, Tone)> {
    let key = sim.notice_key.as_deref().or(sim.no_change_key.as_deref())?;
    let tone = if sim.kind == "reverts" {
        Tone::Danger
    } else if sim.notice_key.is_some() {
        Tone::Caution
    } else {
        Tone::Neutral
    };
    Some((key, tone))
}

/// The value press.s style (r9-19, the web.s `valueVariant`): the accent,
/// or the danger fill where the review asks for "… anyway" — never the
/// plain accent on a signature the review warns against.
pub(crate) fn value_variant(sign: SignRule) -> crate::ui::ButtonVariant {
    if sign.anyway {
        crate::ui::ButtonVariant::Danger
    } else {
        crate::ui::ButtonVariant::Primary
    }
}

pub(crate) fn ms_note(theme: &Theme, text: &str, tone: Tone) -> Div {
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

/// A recipient row's badge ("First time sending here", "Your account").
fn ms_badge(theme: &Theme, text: &str, tone: Tone) -> Div {
    let (bg, fg) = match tone {
        Tone::Neutral => (theme.bg_sunken, theme.fg_muted),
        Tone::Caution => (theme.warning_soft, theme.fg_base),
        Tone::Danger => (theme.error_soft, theme.error_base),
    };
    div()
        .flex_none()
        .px(px(8.))
        .py(px(2.))
        .rounded_full()
        .bg(bg)
        .text_size(theme::text_label())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(fg)
        .child(text.to_owned())
}

/// Who a transaction reaches: its role, the name known for it with its
/// badge, the address whole beneath.
fn ms_party_row(
    theme: &Theme,
    id: ElementId,
    role: &str,
    name: String,
    badge: Option<(String, Tone)>,
    address: &str,
) -> gpui::Stateful<Div> {
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(ms_label(theme, role))
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .text_color(theme.fg_base)
                        .child(name),
                )
                .children(badge.map(|(text, tone)| ms_badge(theme, &text, tone))),
        )
        .child(
            div()
                .font_family(theme::font_mono())
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(crate::ui::address_text(vela_core::multisig_link::eip55(
                    address,
                ))),
        )
}

pub(crate) fn ms_label(theme: &Theme, text: &str) -> Div {
    div()
        .text_size(theme::text_label())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.fg_subtle)
        .child(text.to_owned())
}

/// A fact row: its name, and its value on the right. A long value wraps
/// under itself (r12 zh/dark: "Can't run now — …" ran past the column's
/// edge and was cut), never pushing the name off or leaving the sheet.
fn ms_kv(theme: &Theme, key: &str, value: &str) -> Div {
    div()
        .flex()
        .justify_between()
        .gap(px(12.))
        .text_size(theme::text_row_sub())
        .child(
            div()
                .flex_shrink_0()
                .text_color(theme.fg_subtle)
                .child(key.to_owned()),
        )
        .child(
            div()
                .min_w_0()
                .text_right()
                .text_color(theme.fg_base)
                .child(value.to_owned()),
        )
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
                .child(crate::ui::address_text(value.to_owned())),
        )
}

/// One undeclared change, in words (`writeLine`).
fn ms_write_line(loc: &Loc, w: &vela_core::multisig_review::CriticalWrite) -> String {
    use vela_core::multisig_review::CriticalWrite as W;
    match w {
        W::Singleton { before, after } => words::tv(
            loc,
            "review.writeSingleton",
            &[("before", before), ("after", after)],
        ),
        W::Module { address, enabled } => words::tv(
            loc,
            if *enabled {
                "review.writeModuleOn"
            } else {
                "review.writeModuleOff"
            },
            &[("address", address)],
        ),
        W::Guard { after, .. } => words::tv(loc, "review.writeGuard", &[("after", after)]),
        W::ModuleGuard { after, .. } => {
            words::tv(loc, "review.writeModuleGuard", &[("after", after)])
        }
        W::Handler { after, .. } => words::tv(loc, "review.writeHandler", &[("after", after)]),
        W::Owners { added, removed } => added
            .iter()
            .map(|a| words::tv(loc, "review.writeAdded", &[("address", a)]))
            .chain(
                removed
                    .iter()
                    .map(|a| words::tv(loc, "review.writeRemoved", &[("address", a)])),
            )
            .collect::<Vec<_>>()
            .join("\n"),
        W::OwnerEntry { address } => {
            words::tv(loc, "review.writeOwnerEntry", &[("address", address)])
        }
        W::Threshold { before, after } => words::tv(
            loc,
            "review.writeThreshold",
            &[("before", before), ("after", after)],
        ),
        W::OwnerCount { after, .. } => {
            words::tv(loc, "review.writeOwnerCount", &[("after", after)])
        }
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

/// What a transaction sheet says while its transaction is not among the
/// multisig's (R12 S4): what is true, never "it hasn't been shared with the
/// members" for one this device was never shown. A link's proposal by the
/// core's own stand (`LinkedView.stand`, the iPhone's `linkedState`); else
/// the multisig shown's read — none shown (the account in front is not a
/// multisig): it belongs to a multisig not shown here; its mailbox read and
/// the transaction not in it: said as that; its network unreadable: said
/// as that.
pub(crate) fn no_item_lines(
    loc: &crate::loc::Loc,
    view: &ms_proposals::ProposalsView,
    linked: Option<&ms_proposals::flows::LinkedView>,
    multisig_shown: bool,
    chain_id: u64,
) -> Vec<String> {
    use ms_proposals::flows::LinkedStand;
    let network = || crate::flows::live::chain_name(u32::try_from(chain_id).unwrap_or(u32::MAX));
    let cant_check = || words::tv(loc, "proposal.cantCheck", &[("network", &network())]);
    if let Some(linked) = linked {
        let mut lines = vec![match linked.stand {
            LinkedStand::NotInMailbox => words::t(loc, "proposal.notInMailbox"),
            LinkedStand::NotFound => words::t(loc, "proposal.notFound"),
            LinkedStand::Unreadable => cant_check(),
            // r13-3: not read in time — the read still out, named.
            LinkedStand::Failed => linked.reason_key.as_deref().map_or_else(cant_check, |key| {
                words::tv(
                    loc,
                    key.strip_prefix("home.multisig.").unwrap_or(key),
                    &[("network", &network())],
                )
            }),
            LinkedStand::Reading | LinkedStand::Found => words::t(loc, "proposal.opening"),
        }];
        if linked.preview {
            lines.push(words::t(loc, "proposal.previewNote"));
        }
        return lines;
    }
    if view.loading {
        return vec![words::t(loc, "proposal.opening")];
    }
    if !multisig_shown {
        return vec![words::t(loc, "proposal.notHere")];
    }
    if view.unreadable.contains(&chain_id) {
        return vec![cant_check()];
    }
    match view.chains.iter().find(|c| c.chain_id == chain_id) {
        Some(chain) if chain.mailbox => vec![words::t(loc, "proposal.notInMailbox")],
        Some(_) => vec![words::t(loc, "proposal.notFound")],
        // Its network is not one the multisig shown is read on.
        None => vec![words::t(loc, "proposal.notHere")],
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_sheet_with_no_transaction_says_what_is_true() {
        use vela_core::app::multisig_proposals::flows::{LinkedStand, LinkedView};
        use vela_core::app::multisig_proposals::{ChainState, ProposalsView};
        use vela_core::app::multisig_shell::ChainMembers;
        let loc = crate::loc::Loc::for_tag("en");
        let t = |k: &str| super::words::t(&loc, k);
        let view = ProposalsView::default();
        let not_here = t("proposal.notHere");
        assert!(not_here.contains("isn't shown here"), "{not_here}");
        assert_eq!(
            super::no_item_lines(&loc, &view, None, false, 100),
            vec![not_here.clone()]
        );
        let loading = ProposalsView {
            loading: true,
            ..ProposalsView::default()
        };
        assert_eq!(
            super::no_item_lines(&loc, &loading, None, false, 100),
            vec![t("proposal.opening")]
        );
        let chain = |mailbox| ChainState {
            chain_id: 100,
            members: ChainMembers {
                deployed: true,
                owners: vec![],
                threshold: 1,
                nonce: 0,
            },
            items: vec![],
            mailbox,
        };
        let read = ProposalsView {
            chains: vec![chain(true)],
            ..ProposalsView::default()
        };
        assert_eq!(
            super::no_item_lines(&loc, &read, None, true, 100),
            vec![t("proposal.notInMailbox")]
        );
        // Another network than the shown multisig's: not its transaction.
        assert_eq!(
            super::no_item_lines(&loc, &read, None, true, 8453),
            vec![not_here.clone()]
        );
        let unread = ProposalsView {
            chains: vec![chain(false)],
            ..ProposalsView::default()
        };
        assert_eq!(
            super::no_item_lines(&loc, &unread, None, true, 100),
            vec![t("proposal.notFound")]
        );
        let down = ProposalsView {
            unreadable: vec![100],
            ..ProposalsView::default()
        };
        assert!(super::no_item_lines(&loc, &down, None, true, 100)[0].starts_with("Can't reach"));
        let linked = |stand, preview| LinkedView {
            safe: "0x0e96ddc491878645dB7E3a47d2a6Ccbc94f17f32".into(),
            chain_id: 100,
            safe_tx_hash: "0xab".into(),
            preview,
            stand,
            item: None,
            members: None,
            stale: false,
            reason_key: None,
            retry: false,
        };
        assert_eq!(
            super::no_item_lines(
                &loc,
                &view,
                Some(&linked(LinkedStand::Reading, true)),
                false,
                100
            ),
            vec![t("proposal.opening"), t("proposal.previewNote")]
        );
        assert_eq!(
            super::no_item_lines(
                &loc,
                &view,
                Some(&linked(LinkedStand::NotInMailbox, false)),
                true,
                100
            ),
            vec![t("proposal.notInMailbox")]
        );
        for tag in ["zh", "ja"] {
            let loc = crate::loc::Loc::for_tag(tag);
            let said = super::words::t(&loc, "proposal.notHere");
            assert!(
                !said.starts_with("home.multisig") && said != not_here,
                "{tag}: {said}"
            );
        }
    }

    use super::*;

    /// r9-9: a delegatecall to a contract Vela does not sign for is refused
    /// for ITS reason — the core's key, in words in every language — never
    /// "would change who controls the multisig", and nothing about the
    /// state is said beside it (`withheld`).
    #[test]
    fn a_delegatecall_is_refused_for_its_own_reason() {
        use vela_core::multisig_review::{ReadsInput, ReviewInput, StateVerdict, multisig_review};
        let tx = serde_json::from_value(serde_json::json!({
            "to": "0x5555555555555555555555555555555555555555",
            "value": "0",
            "data": "0xa9059cbb00000000000000000000000044444444444444444444444444444444444444440000000000000000000000000000000000000000000000000000000000000001",
            "operation": 1,
            "safeTxGas": "0", "baseGas": "0", "gasPrice": "0",
            "gasToken": "0x0000000000000000000000000000000000000000",
            "refundReceiver": "0x0000000000000000000000000000000000000000",
            "nonce": "0x7"
        }))
        .unwrap();
        let review = multisig_review(&ReviewInput {
            reads: ReadsInput {
                chain_id: 100,
                safe: "0x0e96ddc491878645dB7E3a47d2a6Ccbc94f17f32".into(),
                tx: Some(tx),
                owners: vec![
                    "0xD400866e00B055B20752a826CD5C89b811de130b".into(),
                    "0x031d7D57c99CAF891e1C250554691Fd12D84772b".into(),
                ],
                threshold: 1,
                deployed: true,
                ..ReadsInput::default()
            },
            sim_reply: Some(r#"{"error":{"code":-32601,"message":"method not supported"}}"#.into()),
            ..ReviewInput::default()
        })
        .unwrap();
        let draw = ReviewDraw::of(&serde_json::to_value(&review).unwrap()).unwrap();
        assert!(!draw.sign.allowed, "nothing is signed");
        let refused = draw.refused.as_ref().expect("its reason");
        assert_eq!(refused.key, "home.multisig.review.refusedWhy.delegatecall");
        for tag in ["en", "zh"] {
            let loc = Loc::for_tag(tag);
            let said = loc.t(&refused.key).to_string();
            assert!(!said.starts_with("home.multisig"), "{tag}: {said}");
            assert_ne!(
                said,
                words::t(&loc, "review.blocked"),
                "{tag}: not the state guard's words"
            );
        }
        assert!(
            draw.state.as_ref().is_none_or(|s| !matches!(
                s.verdict,
                StateVerdict::Unchanged | StateVerdict::Declared
            )),
            "no reassurance beside a refusal"
        );
    }

    /// A refused delegatecall (Bybit's shape, on the fork) says nothing about
    /// what moves: the core withholds the simulation, and the sheet draws no
    /// line in its place — never "No asset changes" under the refusal.
    #[test]
    fn a_refused_delegatecall_says_no_asset_changes_nowhere() {
        use vela_core::app::sim_outcome::{SimOutcome, verdict_of};
        for vector in [
            include_str!(
                "../../../../../rust/crates/vela-core/tests/vectors/multisig-review/fork-bybit-probe.json"
            ),
            include_str!(
                "../../../../../rust/crates/vela-core/tests/vectors/multisig-review/fork-bybit-trace.json"
            ),
        ] {
            let v: Value = serde_json::from_str(vector).unwrap();
            let out = vela_core::multisig::call_json(
                "multisigReview",
                &serde_json::json!({ "input": v["input"] }).to_string(),
            )
            .unwrap();
            let draw = ReviewDraw::of(&serde_json::from_str(&out).unwrap()).unwrap();
            assert_eq!(
                draw.refused.as_ref().unwrap().key,
                "home.multisig.review.refusedWhy.delegatecall"
            );
            assert_eq!(draw.simulation.as_ref().unwrap().kind, "withheld");
            assert!(
                ms_moves(&draw).is_none(),
                "nothing in the place of what moves"
            );
            assert!(
                !draw
                    .concerns
                    .iter()
                    .any(|c| matches!(c, Concern::Unsimulated))
            );
        }
        // The same sheet still says "No asset changes" where the core checked.
        let quiet = verdict_of(SimOutcome::Deltas { deltas: Vec::new() });
        let draw = ReviewDraw {
            simulation: Some(quiet),
            ..ReviewDraw::default()
        };
        let (key, tone) = ms_moves_line(ms_moves(&draw).unwrap()).unwrap();
        assert_eq!(key, "componentsUi.signing.simResultNoChange");
        assert!(tone == Tone::Neutral);
    }

    /// r9-2: Arc's USDC moved as the coin and as its ERC-20 interface is ONE
    /// balance line — the core drops the interface's (`one_line_per_balance`)
    /// — and the sheet words that one line in the coin's units: "−0.01 USDC",
    /// never twice (reading as −0.02).
    #[test]
    fn arcs_usdc_moves_one_balance_line() {
        use vela_core::app::sim_outcome::{SimVerdict, one_line_per_balance};
        use vela_core::app::token_trust::{TrustAssetDelta, TrustDeltaKind};
        const ARC: u64 = 5_042_002;
        let verdict = SimVerdict {
            kind: "deltas".to_owned(),
            deltas: vec![
                TrustAssetDelta {
                    kind: TrustDeltaKind::Native,
                    token: None,
                    delta: "-10000000000000000".to_owned(),
                },
                TrustAssetDelta {
                    kind: TrustDeltaKind::Erc20,
                    token: Some("0x3600000000000000000000000000000000000000".to_owned()),
                    delta: "-10000".to_owned(),
                },
            ],
            revert_reason: None,
            notice_risk: None,
            notice_key: None,
            no_change_key: None,
            recheck: false,
        };
        let one = one_line_per_balance(ARC, verdict);
        assert_eq!(
            one.deltas.len(),
            1,
            "one balance, one line: {:?}",
            one.deltas
        );
        let loc = Loc::for_tag("en");
        let delta = &one.deltas[0];
        let info = words::token_facts(&words::ViewTokens::new(), ARC, delta.token.as_deref());
        let raw = delta.delta.trim_start_matches('-');
        assert_eq!(
            format!("−{}", words::amount_words(&loc, raw, &info)),
            format!("−0.01 {}", crate::flows::live::native_symbol(5_042_002))
        );
    }

    /// r9-19: a signature the review asks for "anyway" wears the danger
    /// fill, never the plain accent; any other value press the accent.
    #[test]
    fn an_anyway_press_is_the_danger_fill() {
        let anyway = SignRule {
            allowed: true,
            anyway: true,
        };
        let plain = SignRule {
            allowed: true,
            anyway: false,
        };
        assert!(matches!(
            value_variant(anyway),
            crate::ui::ButtonVariant::Danger
        ));
        assert!(matches!(
            value_variant(plain),
            crate::ui::ButtonVariant::Primary
        ));
        for theme in [Theme::light(), Theme::dark()] {
            assert_ne!(theme.error_fill, theme.accent);
        }
    }

    /// r4D-14: "Reject" puts the question up and asks nothing of a machine;
    /// its "Cancel" takes it down, nothing asked (nothing can be signed);
    /// its "Reject" takes it down and drafts the cancellation at the
    /// transaction's own number on its own network — a number the core reads
    /// as that number (decimal here, hex on the web: the same quantity).
    #[test]
    fn the_reject_question_has_a_way_back_and_confirms_a_cancellation_at_its_number() {
        let mut ui = MultisigUi::default();
        assert!(!ui.rejecting);
        assert!(ui.reject_press(RejectPress::Ask).is_none());
        assert!(ui.rejecting, "the question is up");
        assert!(ui.reject_press(RejectPress::Back).is_none());
        assert!(!ui.rejecting, "the question went, nothing asked");

        assert!(ui.reject_press(RejectPress::Ask).is_none());
        let event = ui.reject_press(RejectPress::Confirm {
            chain_id: 8453,
            nonce: 26,
        });
        assert!(!ui.rejecting, "the question gives way to the review");
        let Some(ms_proposals::Event::Draft {
            chain_id,
            proposal: ms_proposals::ProposalKind::Cancel { nonce },
        }) = event
        else {
            panic!("a cancellation's draft (its review first): {event:?}");
        };
        assert_eq!(chain_id, 8453);
        let safe = "0x0e96ddc491878645dB7E3a47d2a6Ccbc94f17f32";
        let cancel = vela_core::multisig::cancel_tx(safe, &nonce).unwrap();
        assert_eq!(cancel.nonce, "0x1a", "the transaction's own number");
    }
}
