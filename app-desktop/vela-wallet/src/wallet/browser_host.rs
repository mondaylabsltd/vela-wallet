//! The in-app browser's machine, driven from gpui (spec 070).
//!
//! `vela_core::app::dapp_browser` is the whole decision half of the browser:
//! it parses what the page posted, routes every method through one table,
//! keeps each tab's document and its open requests, the per-origin chain and
//! the grants, serialises signing, and says which document every answer is
//! for. Before 070 this file hosted `dapp_permissions` — a machine that
//! modelled ONE document — and the page kept the rest itself: a routing table
//! (`executor::dapp_rpc`), a column-wide `browser_chain`, a list of open ids to
//! settle, and a signing column that silently replaced the request it was
//! showing when a second one arrived.
//!
//! ## What this file must never decide
//!
//! Whether an origin is connected, whether a request is a read or a
//! signature, which chain a site is on, which document an answer belongs to,
//! and which error code a refusal carries. The core owns every one of those,
//! and each is a security rule. This file reads the store, runs a read on a
//! worker, hands the page's strings to the webview, and hands the signing
//! column its orders.
//!
//! ## Two halves
//!
//! [`BrowserDriver`] runs the machine with no gpui in it: a local operation is
//! answered inline, a network call and a delivery and a signing order come
//! back out as [`Outbound`]. That is what the tests drive, against the real
//! machine and a temporary store. [`BrowserHost`] is the entity around it —
//! it puts a delivery into the webview, a read on the background executor,
//! and an order where the page will pick it up.

use std::collections::VecDeque;

use gpui::Context;

use vela_core::app::dapp_browser::{DappBrowser, DbrOperation, DbrShellResult, DbrView, Event};

use crate::core_host::{CoreHost, Pending};
use crate::executor::dapp_browser::{self as executor, Performed, SigningOrder};
use crate::executor::now_ms;

/// The one tab this shell has.
///
/// wry gives the desktop ONE webview (see `webview.rs`), and the explore tab
/// strip is a list of remembered URLs that it re-navigates between. So the
/// core's "tab" — the thing that holds documents — is that webview, and a
/// switch between strip tabs is what it really is: a navigation, whose new
/// document retires the old one.
pub const BROWSER_TAB: &str = "browser";

/// What the machine needs done that only the entity around it can do.
pub enum Outbound {
    /// Into the page in `tab`, now.
    Deliver { tab: String, message_json: String },
    /// A network call: run it off the main thread and resolve `id` with its
    /// answer — or with `neutral` if it could not finish.
    Work {
        id: u64,
        work: Box<dyn FnOnce() -> DbrShellResult + Send>,
        neutral: DbrShellResult,
    },
    /// For the signing column.
    Signing(SigningOrder),
}

/// The machine and its operations, with no gpui in the way.
pub struct BrowserDriver {
    core: CoreHost<DappBrowser>,
}

impl Default for BrowserDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: CoreHost::new(),
        }
    }

    #[must_use]
    pub fn view(&self) -> DbrView {
        self.core.view()
    }

    /// Send an event; perform what can be performed now.
    pub fn dispatch(&mut self, event: Event) -> Vec<Outbound> {
        let pending = self.core.dispatch(event);
        self.pump(pending)
    }

    /// A network call came back.
    pub fn resolve(&mut self, id: u64, result: DbrShellResult) -> Vec<Outbound> {
        let pending = self.core.resolve(id, result);
        self.pump(pending)
    }

    /// Drain the machine: local answers go straight back in (in the order the
    /// core asked), everything else goes out. A delivery and a signing order
    /// are acked the moment they are handed over — the core owes the page
    /// nothing more for them, and a later answer comes back as an event.
    fn pump(&mut self, pending: Vec<Pending<DbrOperation>>) -> Vec<Outbound> {
        let mut out = Vec::new();
        let mut queue: VecDeque<Pending<DbrOperation>> = pending.into();
        while let Some(next) = queue.pop_front() {
            let answer = match executor::perform(&next.operation) {
                Performed::Now(result) => result,
                Performed::Deliver { tab, message_json } => {
                    out.push(Outbound::Deliver { tab, message_json });
                    DbrShellResult::Ack
                }
                Performed::Signing(order) => {
                    out.push(Outbound::Signing(order));
                    DbrShellResult::Ack
                }
                Performed::Blocking(work) => {
                    out.push(Outbound::Work {
                        id: next.id,
                        work,
                        neutral: executor::neutral(&next.operation),
                    });
                    continue;
                }
            };
            queue.extend(self.core.resolve(next.id, answer));
        }
        out
    }
}

/// The browser machine, as the page's entity.
pub struct BrowserHost {
    driver: BrowserDriver,
    pub view: DbrView,
    /// Orders for the signing column, drained by the page on every
    /// observation. The column is the page's: a host that opened one would be
    /// two owners of the same column.
    orders: Vec<SigningOrder>,
    /// What the machine was last told, so a refresh that changed nothing
    /// says nothing.
    chains: Vec<u32>,
    wallet: Option<(Vec<String>, String)>,
}

impl BrowserHost {
    /// The machine, told about the world before it reads the store.
    ///
    /// The order is the core's own boot: accounts and networks first, then
    /// `Start` lists the sites. Listed last, the grants are judged against a
    /// KNOWN account list and follow the active account without being
    /// re-stamped — told the account after the list, every grant would look
    /// freshly made on every launch.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let driver = BrowserDriver::new();
        let view = driver.view();
        let mut host = Self {
            driver,
            view,
            orders: Vec::new(),
            chains: Vec::new(),
            wallet: None,
        };
        host.follow_wallet(cx);
        host.follow_networks(cx);
        host.dispatch(Event::Start, cx);
        // An account switch is a session change; so is everything else the
        // session does, which is why `follow_wallet` compares before it speaks.
        cx.observe_global::<crate::session::SessionState>(|host, cx| host.follow_wallet(cx))
            .detach();
        host
    }

    pub fn dispatch(&mut self, event: Event, cx: &mut Context<Self>) {
        let out = self.driver.dispatch(event);
        self.act(out, cx);
    }

    fn resolve(&mut self, id: u64, result: DbrShellResult, cx: &mut Context<Self>) {
        let out = self.driver.resolve(id, result);
        self.act(out, cx);
    }

    fn act(&mut self, out: Vec<Outbound>, cx: &mut Context<Self>) {
        for next in out {
            match next {
                Outbound::Deliver { tab, message_json } => {
                    crate::webview::deliver(&tab, &message_json);
                }
                Outbound::Work { id, work, neutral } => {
                    cx.spawn(async move |host, cx| {
                        // A panic in the work is survived (spec 038) and the
                        // page still gets its neutral answer — a read that
                        // never settles is the one outcome this must not have.
                        let result = cx
                            .background_executor()
                            .spawn(async move { crate::panic_report::guarded(work) })
                            .await
                            .unwrap_or(neutral);
                        host.update(cx, |host, cx| host.resolve(id, result, cx))
                            .ok();
                    })
                    .detach();
                }
                Outbound::Signing(order) => self.orders.push(order),
            }
        }
        self.view = self.driver.view();
        cx.notify();
    }

    /// One string from the page, with the URL of the document that sent it
    /// and whether that document is the top one — both the platform's word,
    /// never the page's (see `webview::on_ipc`).
    ///
    /// A switch or an add is judged against the wallet's networks, which the
    /// person edits in Settings; those are re-read here, before the one kind of
    /// request that needs them, rather than on every read a page makes.
    pub fn page_message(
        &mut self,
        frame_url: String,
        is_main_frame: bool,
        message_json: String,
        cx: &mut Context<Self>,
    ) {
        if message_json.contains("wallet_switchEthereumChain")
            || message_json.contains("wallet_addEthereumChain")
        {
            self.follow_networks(cx);
        }
        self.dispatch(
            Event::PageMessage {
                tab: BROWSER_TAB.to_owned(),
                // The SENDER's URL — wry reads it from the message's own
                // frame — so a subframe that reaches the IPC channel directly
                // is named by its own origin, never by the page around it.
                frame_origin: frame_url,
                is_main_frame,
                message_json,
            },
            cx,
        );
    }

    /// The chains a site may switch or add to — the same list the network
    /// settings show.
    pub fn follow_networks(&mut self, cx: &mut Context<Self>) {
        let chains = crate::wallet::signing_host::known_chain_ids();
        if chains != self.chains {
            self.chains.clone_from(&chains);
            self.dispatch(Event::NetworksChanged { chain_ids: chains }, cx);
        }
    }

    /// Every account and the active one, from the session.
    ///
    /// While the session is still reading storage it says nothing: `None` is
    /// the core's "not known yet", and a cold read must never log a site out.
    fn follow_wallet(&mut self, cx: &mut Context<Self>) {
        let session = crate::session::view(cx);
        if session.loading {
            return;
        }
        let addresses: Vec<String> = session
            .accounts
            .iter()
            .map(|row| row.account.address.clone())
            .collect();
        let active = session.address;
        let told = (addresses.clone(), active.clone());
        if self.wallet.as_ref() == Some(&told) {
            return;
        }
        let switched = self.wallet.as_ref().map(|(_, was)| was) != Some(&active);
        self.wallet = Some(told);
        self.dispatch(
            Event::AccountsUpdated {
                addresses: (!addresses.is_empty()).then_some(addresses),
            },
            cx,
        );
        if switched && !active.is_empty() {
            // Every grant follows it (`followActiveAccount`), and each open
            // page of those sites hears `accountsChanged` — the core's rule.
            self.dispatch(
                Event::AccountSwitched {
                    address: active,
                    now_ms: now_ms(),
                },
                cx,
            );
        }
    }

    /// What the page takes away, exactly once.
    pub fn take_orders(&mut self) -> Vec<SigningOrder> {
        std::mem::take(&mut self.orders)
    }

    /// The view of the one tab, if the machine has heard of it.
    #[must_use]
    pub fn tab(&self) -> Option<&vela_core::app::dapp_browser::DbrTabView> {
        self.view.tabs.iter().find(|tab| tab.tab == BROWSER_TAB)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use vela_core::app::dapp_permissions::DpermGrant;
    use vela_core::app::sign_request::{SignErrorKind, SignResponsePayload};

    use super::*;
    use crate::executor::dapp_browser::Forwarded;
    use crate::executor::storage;

    const DAPP: &str = "https://dapp.example";
    const OTHER: &str = "https://other.example";
    const A1: &str = "0x1111111111111111111111111111111111111111";
    const A2: &str = "0x2222222222222222222222222222222222222222";

    fn seed_grant(origin: &str, address: &str, chain_id: u32) {
        let grant = DpermGrant {
            origin: origin.to_owned(),
            address: address.to_owned(),
            chain_id,
            granted_at_ms: 1_757_000_000_000.0,
        };
        let Ok(value) = serde_json::to_value(grant) else {
            unreachable!("a grant serialises");
        };
        if storage::write_value(&format!("vela.perm.{origin}"), value).is_err() {
            unreachable!("could not seed the grant");
        }
    }

    /// Booted the way `BrowserHost::new` boots it: the wallet, the networks,
    /// then the store.
    fn booted() -> BrowserDriver {
        let mut driver = BrowserDriver::new();
        driver.dispatch(Event::AccountsUpdated {
            addresses: Some(vec![A1.to_owned(), A2.to_owned()]),
        });
        driver.dispatch(Event::AccountSwitched {
            address: A1.to_owned(),
            now_ms: 1_757_000_000_000.0,
        });
        driver.dispatch(Event::NetworksChanged {
            chain_ids: vec![1, 100, 8453],
        });
        let out = driver.dispatch(Event::Start);
        assert!(out.is_empty(), "listing the sites is answered inline");
        assert!(driver.view().ready);
        driver
    }

    fn page(driver: &mut BrowserDriver, origin: &str, message: Value) -> Vec<Outbound> {
        driver.dispatch(Event::PageMessage {
            tab: BROWSER_TAB.to_owned(),
            frame_origin: format!("{origin}/app"),
            is_main_frame: true,
            message_json: message.to_string(),
        })
    }

    fn ask(
        driver: &mut BrowserDriver,
        origin: &str,
        doc: &str,
        id: &str,
        method: &str,
        params: Value,
    ) -> Vec<Outbound> {
        page(
            driver,
            origin,
            json!({"t":"req","doc":doc,"id":id,"method":method,"params":params}),
        )
    }

    /// Every delivery, parsed, with the tab it was addressed to.
    fn delivered(out: &[Outbound]) -> Vec<(String, Value)> {
        out.iter()
            .filter_map(|next| match next {
                Outbound::Deliver { tab, message_json } => Some((
                    tab.clone(),
                    serde_json::from_str(message_json).unwrap_or(Value::Null),
                )),
                _ => None,
            })
            .collect()
    }

    fn answers(out: &[Outbound]) -> Vec<Value> {
        delivered(out)
            .into_iter()
            .filter(|(_, message)| message["dir"] == "res")
            .map(|(_, message)| message)
            .collect()
    }

    fn forwarded(out: &[Outbound]) -> Vec<Forwarded> {
        out.iter()
            .filter_map(|next| match next {
                Outbound::Signing(SigningOrder::Forward(forward)) => Some(forward.clone()),
                _ => None,
            })
            .collect()
    }

    fn cancelled(out: &[Outbound]) -> Vec<String> {
        out.iter()
            .filter_map(|next| match next {
                Outbound::Signing(SigningOrder::Cancel { id, .. }) => Some(id.clone()),
                _ => None,
            })
            .collect()
    }

    fn signed(driver: &mut BrowserDriver, id: &str, result: &str) -> Vec<Outbound> {
        driver.dispatch(Event::SigningAnswered {
            tab: BROWSER_TAB.to_owned(),
            id: id.to_owned(),
            payload: SignResponsePayload::Ok {
                result: Some(result.to_owned()),
            },
            user_op_hash: None,
        })
    }

    /// hello → a request → one answer, into the webview that asked and
    /// addressed to the document that asked.
    #[test]
    fn a_request_is_answered_into_the_document_that_asked() {
        storage::tests::with_temp_state("dbr-host-hello", || {
            let mut driver = booted();
            let hello = page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            assert!(delivered(&hello).is_empty(), "a hello is not answered");
            let out = ask(&mut driver, DAPP, "d1", "1", "eth_chainId", json!([]));
            let delivered = delivered(&out);
            assert_eq!(delivered.len(), 1);
            let (tab, message) = &delivered[0];
            assert_eq!(tab, BROWSER_TAB);
            assert_eq!(message["doc"], json!("d1"), "the bridge drops any other");
            assert_eq!(message["id"], json!("1"));
            assert_eq!(
                message["result"],
                json!("0x1"),
                "a new site starts on Ethereum"
            );
            // And an unknown method is 4200, never the 4900 this shell used
            // to answer with.
            let out = ask(&mut driver, DAPP, "d1", "2", "eth_sign", json!([]));
            assert_eq!(answers(&out)[0]["error"]["code"], json!(4200));
        });
    }

    /// A signature goes to the column with the site's OWN chain and the
    /// address the site was shown — never the column-wide chain, never
    /// whichever account happens to be active.
    #[test]
    fn a_signature_is_forwarded_with_the_granted_address_and_the_sites_chain() {
        storage::tests::with_temp_state("dbr-host-forward", || {
            seed_grant(DAPP, A1, 100);
            let _ = storage::write_value("vela.chain.https://dapp.example", json!(8453));
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let out = ask(
                &mut driver,
                DAPP,
                "d1",
                "7",
                "personal_sign",
                json!(["0x00", A1]),
            );
            assert!(answers(&out).is_empty(), "the column answers, later");
            assert_eq!(
                forwarded(&out),
                vec![Forwarded {
                    tab: BROWSER_TAB.to_owned(),
                    id: "7".to_owned(),
                    method: "personal_sign".to_owned(),
                    params_json: json!(["0x00", A1]).to_string(),
                    origin: DAPP.to_owned(),
                    chain_id: 8453,
                    granted_address: A1.to_owned(),
                }]
            );
            let out = signed(&mut driver, "7", "0xsig");
            assert_eq!(answers(&out)[0]["result"], json!("0xsig"));
        });
    }

    /// The desktop used to DROP the first request when a second arrived. Now
    /// the second waits in line and opens when the first is answered.
    #[test]
    fn a_second_signature_waits_instead_of_being_dropped() {
        storage::tests::with_temp_state("dbr-host-queue", || {
            seed_grant(DAPP, A1, 100);
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let first = ask(&mut driver, DAPP, "d1", "1", "personal_sign", json!([]));
            let second = ask(&mut driver, DAPP, "d1", "2", "personal_sign", json!([]));
            assert_eq!(forwarded(&first).len(), 1);
            assert!(forwarded(&second).is_empty(), "one sheet at a time");
            assert!(answers(&second).is_empty(), "neither dropped nor refused");
            assert_eq!(driver.view().queued_signing, 1);

            let out = signed(&mut driver, "1", "0x11");
            assert_eq!(answers(&out)[0]["id"], json!("1"));
            let next = forwarded(&out);
            assert_eq!(next.len(), 1);
            assert_eq!(next[0].id, "2", "the one that waited opens next");

            // A column that cannot open (the wallet's own signing is up) says
            // so with -32002, and the line moves on rather than sticking.
            let out = driver.dispatch(Event::SigningAnswered {
                tab: BROWSER_TAB.to_owned(),
                id: "2".to_owned(),
                payload: SignResponsePayload::Err {
                    code: -32002,
                    kind: SignErrorKind::SubmitFailed,
                    message: Some("Another signing request is open".to_owned()),
                },
                user_op_hash: None,
            });
            assert_eq!(answers(&out)[0]["error"]["code"], json!(-32002));
            assert!(driver.view().signing.is_none());
        });
    }

    /// The page navigated away under an open sheet: its promise gets 4900
    /// once, and the column is told to close — nothing is delivered into the
    /// next document.
    #[test]
    fn a_navigation_under_an_open_sheet_cancels_it() {
        storage::tests::with_temp_state("dbr-host-cancel", || {
            seed_grant(DAPP, A1, 100);
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            ask(
                &mut driver,
                DAPP,
                "d1",
                "1",
                "eth_sendTransaction",
                json!([{}]),
            );

            // Leaving Explore is not a navigation: nothing happens at all
            // until the webview reports one.
            let started = driver.dispatch(Event::NavigationStarted {
                tab: BROWSER_TAB.to_owned(),
                url: format!("{OTHER}/"),
            });
            assert!(started.is_empty(), "a load beginning settles nothing");
            let out = page(&mut driver, OTHER, json!({"t":"hello","doc":"d2"}));
            assert_eq!(cancelled(&out), vec!["1".to_owned()]);
            let settled = answers(&out);
            assert_eq!(settled.len(), 1);
            assert_eq!(settled[0]["error"]["code"], json!(4900));
            assert_eq!(settled[0]["doc"], json!("d1"), "addressed to the dead page");

            // The late answer from the column reaches nobody.
            let late = signed(&mut driver, "1", "0xlate");
            assert!(answers(&late).is_empty());
        });
    }

    /// A load that finishes with no document saying hello (an error page)
    /// retires the old document too.
    #[test]
    fn a_load_without_a_provider_settles_the_old_page() {
        storage::tests::with_temp_state("dbr-host-load", || {
            seed_grant(DAPP, A1, 100);
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            ask(&mut driver, DAPP, "d1", "1", "personal_sign", json!([]));
            driver.dispatch(Event::NavigationStarted {
                tab: BROWSER_TAB.to_owned(),
                url: "https://broken.example/".to_owned(),
            });
            let out = driver.dispatch(Event::LoadFinished {
                tab: BROWSER_TAB.to_owned(),
                url: "https://broken.example/".to_owned(),
            });
            assert_eq!(cancelled(&out), vec!["1".to_owned()]);
            assert_eq!(answers(&out)[0]["error"]["code"], json!(4900));
        });
    }

    /// A read goes to the network off the main thread, and the node's body
    /// comes back as the page's answer.
    #[test]
    fn a_read_waits_for_the_network_and_comes_back() {
        storage::tests::with_temp_state("dbr-host-read", || {
            let _ = storage::write_value("vela.chain.https://dapp.example", json!(100));
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let out = ask(&mut driver, DAPP, "d1", "9", "eth_blockNumber", json!([]));
            assert!(answers(&out).is_empty());
            let ids: Vec<u64> = out
                .iter()
                .filter_map(|next| match next {
                    Outbound::Work { id, neutral, .. } => {
                        assert_eq!(*neutral, DbrShellResult::ReadAnswered { body_json: None });
                        Some(*id)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(ids.len(), 1, "one call, not run here");
            let out = driver.resolve(
                ids[0],
                DbrShellResult::ReadAnswered {
                    body_json: Some(r#"{"jsonrpc":"2.0","id":1,"result":"0x10"}"#.to_owned()),
                },
            );
            assert_eq!(answers(&out)[0]["result"], json!("0x10"));
        });
    }

    /// Consent: one approval writes the grant under the shared key, the
    /// history row, and answers the page with the active account.
    #[test]
    fn approving_a_connection_writes_the_grant_and_answers() {
        storage::tests::with_temp_state("dbr-host-consent", || {
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let out = ask(
                &mut driver,
                DAPP,
                "d1",
                "1",
                "eth_requestAccounts",
                json!([]),
            );
            assert!(answers(&out).is_empty(), "the person is asked first");
            assert_eq!(
                driver.view().consent.map(|consent| consent.origin),
                Some(DAPP.to_owned())
            );
            let out = driver.dispatch(Event::ConsentApproved {
                now_ms: 1_757_000_000_000.0,
            });
            assert_eq!(answers(&out)[0]["result"], json!([A1]));
            let stored = storage::read_value("vela.perm.https://dapp.example")
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("no grant was written"));
            assert_eq!(stored["address"], json!(A1));
            let history = storage::read_value("vela.transactionHistory")
                .ok()
                .flatten()
                .unwrap_or_default();
            assert_eq!(history[0]["type"], json!("connect"));
        });
    }

    /// Settings' list: every stored grant, and a revoke from there reaches
    /// the store and any open page of that site.
    #[test]
    fn connected_sites_are_listed_and_revoked() {
        storage::tests::with_temp_state("dbr-host-sites", || {
            seed_grant(DAPP, A1, 100);
            seed_grant(OTHER, A1, 1);
            let mut driver = booted();
            let origins: Vec<String> = driver
                .view()
                .sites
                .iter()
                .map(|site| site.origin.clone())
                .collect();
            assert_eq!(origins.len(), 2);
            assert!(origins.contains(&DAPP.to_owned()) && origins.contains(&OTHER.to_owned()));

            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let out = driver.dispatch(Event::RevokeRequested {
                origin: DAPP.to_owned(),
            });
            let events: Vec<Value> = delivered(&out)
                .into_iter()
                .map(|(_, message)| message["event"].clone())
                .collect();
            assert_eq!(events, vec![json!("accountsChanged"), json!("disconnect")]);
            assert!(
                storage::read_value("vela.perm.https://dapp.example")
                    .ok()
                    .flatten()
                    .is_none()
            );
            assert_eq!(driver.view().sites.len(), 1);

            driver.dispatch(Event::RevokeAll);
            assert!(driver.view().sites.is_empty());
            assert!(
                storage::read_value("vela.perm.https://other.example")
                    .ok()
                    .flatten()
                    .is_none(),
                "Storage's clear reaches the live session and the file"
            );
        });
    }

    /// The connection panel's network row: the site moves, the choice is
    /// kept per origin, and a chain the wallet does not have is not a choice.
    #[test]
    fn a_person_can_pick_a_sites_network() {
        storage::tests::with_temp_state("dbr-host-pick", || {
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let out = driver.dispatch(Event::SiteChainPicked {
                origin: DAPP.to_owned(),
                chain_id: 100,
            });
            let events: Vec<(Value, Value)> = delivered(&out)
                .into_iter()
                .map(|(_, message)| (message["event"].clone(), message["data"].clone()))
                .collect();
            assert_eq!(events, vec![(json!("chainChanged"), json!("0x64"))]);
            assert_eq!(
                storage::read_value("vela.chain.https://dapp.example")
                    .ok()
                    .flatten(),
                Some(json!(100))
            );
            let tab = driver.view().tabs.into_iter().next();
            assert_eq!(tab.map(|tab| tab.chain_id), Some(100));

            driver.dispatch(Event::SiteChainPicked {
                origin: DAPP.to_owned(),
                chain_id: 424_242,
            });
            assert_eq!(driver.view().tabs[0].chain_id, 100);
        });
    }
}
