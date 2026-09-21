//! What the in-app browser's machine asks of this shell (spec 070).
//!
//! `vela_core::app::dapp_browser` decides everything a page is told: which
//! method is a read, a signature or a refusal, which chain a site is on, which
//! document an answer belongs to, and when a request is over. Until 070 this
//! file was a routing table — `classify`, the read allowlists, the chain-param
//! parser — ported from the extension and pinned to it by a test. That table
//! is the core's now (`dapp_rpc::classify`), once for every client, and what
//! is left here is the part only a shell can do:
//!
//! - the store: `vela.perm.<origin>` (a grant) and `vela.chain.<origin>` (the
//!   site's chain), one key each, spelled the way the other clients spell them;
//! - a node or bundler read through the person's own endpoints, and a
//!   user-operation receipt through the relay;
//! - the "Connected to <app>" history row;
//! - handing a message to the page, and an order to the signing column — both
//!   of which belong to the page, so they come back out of [`perform`] rather
//!   than being acted on here.
//!
//! Every operation has a neutral answer (an empty list, `None`, an ack), so a
//! failure here still settles the page's promise — the core turns a `None`
//! read into -32603, never into silence.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use vela_core::app::dapp_browser::{DbrOperation, DbrShellResult, DbrStoredSite};
use vela_core::app::dapp_permissions::DpermGrant;

use crate::executor::{now_ms, pool, relay, storage};

/// `vela.perm.<origin>` — the grant, the cross-client key.
pub const PERM_PREFIX: &str = "vela.perm.";
/// `vela.chain.<origin>` — the chain a site is on, the extension's key. A
/// number, as `protocol.js` writes it.
pub const CHAIN_PREFIX: &str = "vela.chain.";
/// `vela.transactionHistory` — the shared local store, spelled once per reader.
const TX_KEY: &str = "vela.transactionHistory";

/// One request the core handed to the signing pipeline, whole.
///
/// `granted_address` is the address the site was SHOWN: the signer is pinned
/// to it, so a site granted account A never gets a signature from B because B
/// happened to be active (spec 070 defect 2).
#[derive(Clone, Debug, PartialEq)]
pub struct Forwarded {
    pub tab: String,
    pub id: String,
    pub method: String,
    pub params_json: String,
    pub origin: String,
    pub chain_id: u32,
    pub granted_address: String,
}

/// What the signing column must do. The column is the page's, so the order
/// travels out rather than being carried out here.
#[derive(Clone, Debug, PartialEq)]
pub enum SigningOrder {
    /// Open the sheet for this request. Answered later, exactly once, with
    /// `signing_answered`.
    Forward(Forwarded),
    /// The page that asked is gone and already has its 4900: close the sheet
    /// for this request without answering it.
    Cancel { tab: String, id: String },
}

/// How one operation is performed, and therefore where.
pub enum Performed {
    /// A local read or write, answered here and now.
    Now(DbrShellResult),
    /// A network call. Blocks, so it runs off the main thread and its answer
    /// is resolved when it lands.
    Blocking(Box<dyn FnOnce() -> DbrShellResult + Send>),
    /// Hand `message_json` to `window.__velaDeliver` in `tab`. The webview is
    /// the main thread's; the core is acked at once.
    Deliver { tab: String, message_json: String },
    /// An order for the signing column; the core is acked at once.
    Signing(SigningOrder),
}

/// Perform one operation. Never fails outward (`executor::mod`'s contract).
pub fn perform(operation: &DbrOperation) -> Performed {
    match operation {
        DbrOperation::ListSites => Performed::Now(DbrShellResult::SitesListed {
            sites: list_sites(),
        }),
        DbrOperation::WriteGrant { grant } => {
            write_grant(grant);
            Performed::Now(DbrShellResult::Ack)
        }
        DbrOperation::RemoveGrant { origin } => {
            // Revoked leaves NOTHING behind — not a null, which every later
            // reader would still have to step over.
            let _ = storage::remove_value(&format!("{PERM_PREFIX}{origin}"));
            Performed::Now(DbrShellResult::Ack)
        }
        DbrOperation::WriteSiteChain { origin, chain_id } => {
            let _ = storage::write_value(&format!("{CHAIN_PREFIX}{origin}"), json!(chain_id));
            Performed::Now(DbrShellResult::Ack)
        }
        DbrOperation::Deliver {
            tab, message_json, ..
        } => Performed::Deliver {
            tab: tab.clone(),
            message_json: message_json.clone(),
        },
        DbrOperation::Read {
            chain_id,
            method,
            params_json,
            bundler,
            ..
        } => {
            let (chain_id, method, bundler) = (*chain_id, method.clone(), *bundler);
            let params = params_of(params_json);
            Performed::Blocking(Box::new(move || read(chain_id, &method, params, bundler)))
        }
        DbrOperation::ResolveUserOp {
            chain_id,
            user_op_hash,
        } => {
            let (chain_id, hash) = (*chain_id, user_op_hash.clone());
            Performed::Blocking(Box::new(move || DbrShellResult::UserOpResolved {
                tx_hash: relay::user_op_receipt(&hash, chain_id)
                    .resolution
                    .map(|landed| landed.tx_hash),
            }))
        }
        DbrOperation::ForwardToSigning {
            tab,
            id,
            method,
            params_json,
            origin,
            chain_id,
            granted_address,
        } => Performed::Signing(SigningOrder::Forward(Forwarded {
            tab: tab.clone(),
            id: id.clone(),
            method: method.clone(),
            params_json: params_json.clone(),
            origin: origin.clone(),
            chain_id: *chain_id,
            granted_address: granted_address.clone(),
        })),
        DbrOperation::CancelSigning { tab, id } => Performed::Signing(SigningOrder::Cancel {
            tab: tab.clone(),
            id: id.clone(),
        }),
        DbrOperation::SaveConnectionRecord {
            address,
            chain_id,
            origin,
        } => {
            save_connection_record(address, *chain_id, origin);
            Performed::Now(DbrShellResult::Ack)
        }
    }
}

/// The answer a blocking operation owes when its work could not finish (it
/// panicked): the same neutral answer a failure gets.
#[must_use]
pub fn neutral(operation: &DbrOperation) -> DbrShellResult {
    match operation {
        DbrOperation::Read { .. } => DbrShellResult::ReadAnswered { body_json: None },
        DbrOperation::ResolveUserOp { .. } => DbrShellResult::UserOpResolved { tx_hash: None },
        DbrOperation::ListSites => DbrShellResult::SitesListed { sites: Vec::new() },
        _ => DbrShellResult::Ack,
    }
}

/// Every stored site: the grants and the chains, merged by origin.
///
/// A grant that will not parse is NO grant rather than a failure — a record
/// that parsed into a default would connect a site to an address nobody
/// granted. Which origin a grant is really for is the core's check (a grant
/// filed under the wrong key is ignored there).
pub fn list_sites() -> Vec<DbrStoredSite> {
    let mut sites: BTreeMap<String, DbrStoredSite> = BTreeMap::new();
    for (key, value) in storage::entries_with_prefix(PERM_PREFIX).unwrap_or_default() {
        site_of(&mut sites, &key[PERM_PREFIX.len()..]).grant =
            serde_json::from_value::<DpermGrant>(value).ok();
    }
    for (key, value) in storage::entries_with_prefix(CHAIN_PREFIX).unwrap_or_default() {
        site_of(&mut sites, &key[CHAIN_PREFIX.len()..]).chain_id = chain_of(&value);
    }
    sites.into_values().collect()
}

fn site_of<'a>(
    sites: &'a mut BTreeMap<String, DbrStoredSite>,
    origin: &str,
) -> &'a mut DbrStoredSite {
    sites
        .entry(origin.to_owned())
        .or_insert_with(|| DbrStoredSite {
            origin: origin.to_owned(),
            grant: None,
            chain_id: None,
        })
}

/// A stored chain: a number (the extension's), or a decimal or `0x` string
/// from an older writer. Anything else is no chain, and the core falls back
/// to the grant's.
fn chain_of(value: &Value) -> Option<u32> {
    match value {
        Value::Number(number) => number.as_u64().and_then(|n| u32::try_from(n).ok()),
        Value::String(text) => {
            let text = text.trim();
            match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => text.parse().ok(),
            }
        }
        _ => None,
    }
}

fn write_grant(grant: &DpermGrant) {
    let Ok(value) = serde_json::to_value(grant) else {
        return;
    };
    // Best-effort, as the port's `setGrant` is: the core's copy stays
    // authoritative for this session, and a grant that could not be written
    // costs one re-consent on the next launch.
    let _ = storage::write_value(&format!("{PERM_PREFIX}{}", grant.origin), value);
}

/// The params as a node reads them. The core hands over the page's own JSON;
/// a missing `params` becomes the empty list a node expects, not `null`.
fn params_of(params_json: &str) -> Value {
    match serde_json::from_str::<Value>(params_json) {
        Ok(Value::Null) | Err(_) => json!([]),
        Ok(params) => params,
    }
}

/// One read, through the pool every other read in this app uses. The body
/// goes back whole — `{"result":…}` or `{"error":{…}}` — and the core unpacks
/// it; `None` is "no endpoint answered", which the core says as -32603.
fn read(chain_id: u32, method: &str, params: Value, bundler: bool) -> DbrShellResult {
    let body = if bundler {
        pool::bundler_call(chain_id, method, params)
    } else {
        pool::call(chain_id, method, params)
    };
    DbrShellResult::ReadAnswered {
        body_json: body.ok().map(|body| body.to_string()),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn grant(origin: &str) -> DpermGrant {
        DpermGrant {
            origin: origin.to_owned(),
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
            chain_id: 100,
            granted_at_ms: 1_757_000_000_000.0,
        }
    }

    /// A grant survives the round trip, under the key the other clients use.
    ///
    /// `vela.perm.<origin>` is a CROSS-CLIENT key: a wallet copied from the
    /// phone brings its connections with it, and a desktop that spelled the
    /// key its own way would silently ask for consent again on every site the
    /// person had already connected.
    #[test]
    fn a_grant_round_trips_under_the_shared_key() {
        storage::tests::with_temp_state("dbr-grant", || {
            let grant = grant("https://app.uniswap.org");
            let Performed::Now(DbrShellResult::Ack) = perform(&DbrOperation::WriteGrant {
                grant: grant.clone(),
            }) else {
                unreachable!("a grant write is answered now");
            };

            let raw = storage::read_value("vela.perm.https://app.uniswap.org")
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("the grant is not under the shared key"));
            assert_eq!(
                raw.get("address").and_then(Value::as_str),
                Some(grant.address.as_str()),
                "the grant is pinned to the address it was made for"
            );
            assert_eq!(
                list_sites(),
                vec![DbrStoredSite {
                    origin: grant.origin.clone(),
                    grant: Some(grant.clone()),
                    chain_id: None,
                }]
            );

            perform(&DbrOperation::RemoveGrant {
                origin: grant.origin.clone(),
            });
            assert!(
                storage::read_value("vela.perm.https://app.uniswap.org")
                    .ok()
                    .flatten()
                    .is_none(),
                "revoked leaves nothing behind"
            );
            assert!(list_sites().is_empty());
        });
    }

    /// The chain is its own key, a number, and merges with the grant of the
    /// same origin into ONE stored site.
    #[test]
    fn a_sites_chain_and_grant_are_listed_together() {
        storage::tests::with_temp_state("dbr-chain", || {
            perform(&DbrOperation::WriteGrant {
                grant: grant("https://app.uniswap.org"),
            });
            perform(&DbrOperation::WriteSiteChain {
                origin: "https://app.uniswap.org".to_owned(),
                chain_id: 8453,
            });
            perform(&DbrOperation::WriteSiteChain {
                origin: "https://curve.fi".to_owned(),
                chain_id: 1,
            });
            assert_eq!(
                storage::read_value("vela.chain.https://app.uniswap.org")
                    .ok()
                    .flatten(),
                Some(json!(8453)),
                "a number, as protocol.js writes it"
            );
            let mut sites = list_sites();
            sites.sort_by(|a, b| a.origin.cmp(&b.origin));
            assert_eq!(sites.len(), 2);
            assert_eq!(sites[0].origin, "https://app.uniswap.org");
            assert_eq!(sites[0].chain_id, Some(8453));
            assert!(sites[0].grant.is_some());
            assert_eq!(sites[1].origin, "https://curve.fi");
            assert_eq!(sites[1].chain_id, Some(1));
            assert!(sites[1].grant.is_none(), "a chain alone is not a grant");
        });
    }

    /// Half a grant is no grant, and an odd chain is no chain.
    #[test]
    fn unreadable_records_read_as_absent() {
        storage::tests::with_temp_state("dbr-corrupt", || {
            let _ = storage::write_value(
                "vela.perm.https://evil.example",
                json!({ "origin": "https://evil.example" }),
            );
            let _ = storage::write_value("vela.chain.https://evil.example", json!("gnosis"));
            let _ = storage::write_value("vela.chain.https://old.example", json!("0x64"));
            let mut sites = list_sites();
            sites.sort_by(|a, b| a.origin.cmp(&b.origin));
            assert_eq!(sites[0].origin, "https://evil.example");
            assert_eq!(sites[0].grant, None);
            assert_eq!(sites[0].chain_id, None);
            assert_eq!(sites[1].chain_id, Some(100), "an older writer's hex");
        });
    }

    /// The audit row the feed already knows how to read.
    #[test]
    fn a_connection_is_recorded_in_the_shared_shape() {
        storage::tests::with_temp_state("dbr-record", || {
            perform(&DbrOperation::SaveConnectionRecord {
                address: "0xabc".to_owned(),
                chain_id: 100,
                origin: "https://app.uniswap.org".to_owned(),
            });
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

    /// A page that sends no params is asked about with an empty list — a node
    /// refuses `null` where it wants `[]`.
    #[test]
    fn missing_params_become_an_empty_list() {
        assert_eq!(params_of("null"), json!([]));
        assert_eq!(params_of(""), json!([]));
        assert_eq!(params_of(r#"["0x1",true]"#), json!(["0x1", true]));
    }

    /// Delivery and signing are the page's; everything else settles here.
    #[test]
    fn the_page_owned_operations_come_back_out() {
        let deliver = perform(&DbrOperation::Deliver {
            tab: "browser".to_owned(),
            doc: "d1".to_owned(),
            message_json: "{}".to_owned(),
        });
        assert!(matches!(deliver, Performed::Deliver { tab, .. } if tab == "browser"));
        let cancel = perform(&DbrOperation::CancelSigning {
            tab: "browser".to_owned(),
            id: "7".to_owned(),
        });
        assert!(matches!(
            cancel,
            Performed::Signing(SigningOrder::Cancel { id, .. }) if id == "7"
        ));
        let read = perform(&DbrOperation::Read {
            tab: "browser".to_owned(),
            id: "1".to_owned(),
            chain_id: 1,
            method: "eth_blockNumber".to_owned(),
            params_json: "[]".to_owned(),
            bundler: false,
        });
        assert!(matches!(read, Performed::Blocking(_)), "never on a frame");
    }
}
