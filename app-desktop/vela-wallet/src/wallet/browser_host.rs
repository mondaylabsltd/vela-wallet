//! Who a site is allowed to be, for as long as its document lasts.
//!
//! `dapp_permissions` is the machine every request from the browser column
//! passes through — the reads it answers itself (`eth_accounts`,
//! `eth_chainId`), the consent it asks for, the grants it keeps per origin,
//! and the handful of methods it forwards to the signing pipeline. Until spec
//! 032 phase 27 the desktop answered **4900 to all of them**, which is why a
//! real dApp could not connect: the local probe page worked only because it
//! called `eth_sendTransaction` without asking anything first.
//!
//! ## Why a host rather than a resident
//!
//! Two of the core's operations cannot be performed by a static `perform`:
//! `ForwardToSigning` opens the signing column (which is the page's), and
//! `SettleForwarded` closes out what that column still holds. Both need a
//! `Context`, so this owns the `CoreHost` and pumps it — the `SigningHost`
//! shape, for the same reason.
//!
//! ## What this file must never decide
//!
//! Whether an origin is connected, whether a frame may ask, whether a request
//! is a read or a signature, and which error code a refusal carries. All of
//! those are the core's, and every one of them is a security rule: the
//! cross-origin frame check (invariant ①), the insecure-origin gate for
//! signing (③), the grant pinned to the address it was made for (⑨). This
//! file reads storage, talks to the webview, and hands the page what the core
//! told it to hand over.

use gpui::{Context, Entity};
use serde_json::{Value, json};

use vela_core::app::dapp_permissions::{
    DappPermissions, DpermGrant, DpermOperation, DpermShellResult, DpermView, Event as DpermEvent,
};

use crate::core_host::{CoreHost, Pending};
use crate::executor::{now_ms, storage};

/// `vela.transactionHistory` — the shared local store, spelled once per reader.
const TX_KEY: &str = "vela.transactionHistory";

/// One request the core decided belongs to the signing machines.
///
/// Carried out of the host rather than acted on inside it: the column those
/// machines live in belongs to the page, and a host that opened one would be
/// two owners of the same column.
pub struct Forwarded {
    pub id: String,
    pub method: String,
    pub params_json: String,
    pub origin: String,
}

pub struct BrowserHost {
    perms: CoreHost<DappPermissions>,
    pub view: DpermView,
    /// Drained by the page on every observation.
    pub forwarded: Vec<Forwarded>,
    /// Ids handed to the signing pipeline and not yet known to be answered.
    ///
    /// Kept so `SettleForwarded` has something to settle. The exactly-one-
    /// response-per-id gate is the transport's (invariant ⑩) — `inpage.js`
    /// resolves a promise once and forgets the id — so a settle that races a
    /// signature is harmless in the direction that matters: the page never
    /// sees two answers, and a request that would otherwise hang forever gets
    /// one.
    open_ids: Vec<String>,
    /// A settle happened. The page closes the signing column on it: the
    /// request those machines were decoding is over.
    pub settled: bool,
}

impl BrowserHost {
    /// The machine, told what it needs before the first page can ask anything.
    ///
    /// The seeds are not optional. The core's own docs say the shell "must
    /// seed the initial chain with this before the first consent can be
    /// approved", and a grant is judged against ALL wallet addresses rather
    /// than the active one — told nothing, the machine would refuse a
    /// connection for a wallet that has accounts, which is the shape of bug
    /// phase 22 found on the signing side.
    pub fn new(
        addresses: Vec<String>,
        active: String,
        chain_id: u32,
        cx: &mut Context<Self>,
    ) -> Self {
        let perms = CoreHost::<DappPermissions>::new();
        let view = perms.view();
        let mut host = Self {
            perms,
            view,
            forwarded: Vec::new(),
            open_ids: Vec::new(),
            settled: false,
        };
        host.dispatch(
            DpermEvent::AccountsUpdated {
                addresses: Some(addresses),
            },
            cx,
        );
        host.dispatch(
            DpermEvent::AccountSwitched {
                address: active,
                now_ms: now_ms(),
            },
            cx,
        );
        host.dispatch(DpermEvent::ChainChanged { chain_id }, cx);
        host
    }

    pub fn dispatch(&mut self, event: DpermEvent, cx: &mut Context<Self>) {
        let pending = self.perms.dispatch(event);
        self.pump(pending, cx);
    }

    fn resolve(&mut self, id: u64, result: DpermShellResult, cx: &mut Context<Self>) {
        let pending = self.perms.resolve(id, result);
        self.pump(pending, cx);
    }

    fn pump(&mut self, pending: Vec<Pending<DpermOperation>>, cx: &mut Context<Self>) {
        for next in pending {
            let result = self.perform(&next.operation);
            self.resolve(next.id, result, cx);
        }
        self.view = self.perms.view();
        cx.notify();
    }

    /// Every operation is answered here and now.
    ///
    /// None of them is a network call: the grant store is the local file, the
    /// bridge is a script evaluation, and the two that reach the page put a
    /// row in a queue. Nothing here may block a frame, so nothing here needs
    /// a background thread.
    fn perform(&mut self, operation: &DpermOperation) -> DpermShellResult {
        match operation {
            DpermOperation::ReadGrant { origin } => DpermShellResult::GrantRead {
                origin: origin.clone(),
                grant: read_grant(origin),
            },
            DpermOperation::WriteGrant { grant } => {
                write_grant(grant);
                DpermShellResult::Ack
            }
            DpermOperation::RemoveGrant { origin } => {
                let _ = storage::remove_value(&grant_key(origin));
                DpermShellResult::Ack
            }
            DpermOperation::Respond { id, payload } => {
                crate::webview::respond_permission(id, payload);
                DpermShellResult::Ack
            }
            DpermOperation::EmitEvent { event } => {
                crate::webview::emit_page_event(event);
                DpermShellResult::Ack
            }
            DpermOperation::SettleForwarded { code, reason } => {
                // The CODE and the reason are the core's: 4001 when a person
                // declined, 4900 when the document went away or the browser
                // closed. A shell picking between them would report a refusal
                // as a failure, and a dApp treats those differently.
                let payload = vela_core::app::dapp_permissions::DpermRespondPayload::Error {
                    code: *code,
                    reason: *reason,
                };
                for id in std::mem::take(&mut self.open_ids) {
                    crate::webview::respond_permission(&id, &payload);
                }
                self.settled = true;
                DpermShellResult::Ack
            }
            DpermOperation::SaveConnectionRecord {
                address,
                chain_id,
                origin,
            } => {
                save_connection_record(address, *chain_id, origin);
                DpermShellResult::Ack
            }
            DpermOperation::ForwardToSigning {
                id,
                method,
                params_json,
                origin,
            } => {
                self.open_ids.push(id.clone());
                self.forwarded.push(Forwarded {
                    id: id.clone(),
                    method: method.clone(),
                    params_json: params_json.clone(),
                    origin: origin.clone(),
                });
                DpermShellResult::Ack
            }
        }
    }

    /// What the page takes away, exactly once.
    pub fn take_forwarded(&mut self) -> Vec<Forwarded> {
        std::mem::take(&mut self.forwarded)
    }

    /// The page asks once per observation whether the request it is drawing
    /// is over.
    pub fn take_settled(&mut self) -> bool {
        std::mem::take(&mut self.settled)
    }

    /// A signature came back, so that id is no longer this file's to settle.
    pub fn answered(&mut self, id: &str) {
        self.open_ids.retain(|open| open != id);
    }
}

/// `vela.perm.<origin>` — the cross-client key, spelled the way the other
/// three shells spell it.
fn grant_key(origin: &str) -> String {
    format!("vela.perm.{origin}")
}

/// A parse or storage failure answers `None`, which is the core's own
/// contract: an unreadable grant is not a grant, and guessing one from a
/// half-written record would connect a site nobody connected.
fn read_grant(origin: &str) -> Option<DpermGrant> {
    let value = storage::read_value(&grant_key(origin)).ok()??;
    serde_json::from_value::<DpermGrant>(value).ok()
}

fn write_grant(grant: &DpermGrant) {
    let Ok(value) = serde_json::to_value(grant) else {
        return;
    };
    // Best-effort, as the port's `setGrant` is: a grant that could not be
    // written costs one re-consent, and there is nothing truthful to say
    // about it on a page that is already connected.
    let _ = storage::write_value(&grant_key(&grant.origin), value);
}

/// The "Connected to <app>" row, in the shape the other clients read.
///
/// Field-for-field `buildConnectionRecord` (`dapp-history.ts:240-262`): the id
/// carries the clock, the empty hashes and the zero value are what a
/// connection has, and `type: "connect"` is what the feed's own reader
/// matches on.
fn save_connection_record(address: &str, chain_id: u32, origin: &str) {
    let now_ms = now_ms();
    #[allow(
        clippy::cast_possible_truncation,
        reason = "seconds, as the port stores them"
    )]
    let timestamp = (now_ms / 1000.0) as i64;
    let row = json!({
        "id": format!("dapp-{now_ms}-connect"),
        "userOpHash": "",
        "txHash": "",
        "from": address,
        "to": "",
        "value": "0",
        "symbol": "",
        "decimals": 0,
        "chainId": chain_id,
        "timestamp": timestamp,
        "status": "confirmed",
        "type": "connect",
        "dappOrigin": origin,
    });
    let mut rows = match storage::read_value(TX_KEY) {
        Ok(Some(Value::Array(rows))) => rows,
        _ => Vec::new(),
    };
    rows.push(row);
    let _ = storage::write_value(TX_KEY, Value::Array(rows));
}

/// The host, for the page's `Option<Entity<BrowserHost>>`.
pub type Host = Entity<BrowserHost>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::storage;

    /// A grant survives the round trip, under the key the other clients use.
    ///
    /// `vela.perm.<origin>` is a CROSS-CLIENT key: a wallet copied from the
    /// phone brings its connections with it, and a desktop that spelled the
    /// key its own way would silently ask for consent again on every site the
    /// person had already connected.
    #[test]
    fn a_grant_round_trips_under_the_shared_key() {
        storage::tests::with_temp_state("dperm-grant", || {
            let grant = DpermGrant {
                origin: "https://app.uniswap.org".to_owned(),
                address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
                chain_id: 100,
                granted_at_ms: 1_757_000_000_000.0,
            };
            write_grant(&grant);

            let raw = storage::read_value("vela.perm.https://app.uniswap.org")
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("the grant is not under the shared key"));
            assert_eq!(
                raw.get("address").and_then(Value::as_str),
                Some(grant.address.as_str()),
                "the grant is pinned to the address it was made for"
            );
            assert_eq!(read_grant("https://app.uniswap.org").as_ref(), Some(&grant));

            // Revoked leaves NOTHING behind — not a null, which every later
            // reader would still have to step over.
            let _ = storage::remove_value(&grant_key(&grant.origin));
            assert_eq!(read_grant("https://app.uniswap.org"), None);
            assert!(
                storage::read_value("vela.perm.https://app.uniswap.org")
                    .ok()
                    .flatten()
                    .is_none()
            );
        });
    }

    /// Half a grant is no grant.
    ///
    /// The core's contract for a failed read is `None`, and the reason is
    /// worth stating: a record that parsed into a DEFAULT would connect a site
    /// to an address nobody granted.
    #[test]
    fn an_unreadable_grant_reads_as_absent() {
        storage::tests::with_temp_state("dperm-corrupt", || {
            let _ = storage::write_value(
                "vela.perm.https://evil.example",
                json!({ "origin": "https://evil.example" }),
            );
            assert_eq!(read_grant("https://evil.example"), None);
        });
    }

    /// The audit row the feed already knows how to read.
    #[test]
    fn a_connection_is_recorded_in_the_shared_shape() {
        storage::tests::with_temp_state("dperm-record", || {
            save_connection_record("0xabc", 100, "https://app.uniswap.org");
            let rows = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("no history was written"),
            };
            let row = rows.first().unwrap_or_else(|| unreachable!("no row"));
            // `type` is what `activity_feed`'s own reader matches on; the
            // empty hashes and zero value are what a connection HAS, and a
            // send-shaped row would show up in the feed as money moving.
            assert_eq!(row.get("type").and_then(Value::as_str), Some("connect"));
            assert_eq!(row.get("value").and_then(Value::as_str), Some("0"));
            assert_eq!(row.get("txHash").and_then(Value::as_str), Some(""));
            assert_eq!(
                row.get("dappOrigin").and_then(Value::as_str),
                Some("https://app.uniswap.org")
            );
            assert_eq!(row.get("chainId").and_then(Value::as_u64), Some(100));
        });
    }
}
