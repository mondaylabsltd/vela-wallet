//! Which of Vela's networks a person's lists show (owner decision
//! 2026-10-29).
//!
//! Every network in [`BUILTIN_CHAINS`] is a Vela network — relay, fees,
//! logos and clear signing alike. Most are in every list from the start
//! ([`NetBuiltinChain::shown_by_default`]); the rest are kept out of the
//! default list for their low activity and come into a person's lists on
//! their word: Settings → Add network (or a site's add-network request)
//! shows one again in one step, as the Vela network it is, never as a custom
//! one. Showing or hiding one never touches what is stored for it — an RPC
//! override stays stored while its network is out of the list.
//!
//! The record ([`KEY`]) holds which of those the person has shown. A device
//! that has none yet — every device the day this shipped, and a fresh
//! profile — is settled ONCE ([`settle`]): a network stays shown wherever
//! the wallet finds the person using it ([`NetUseEvidence`]) — a last-known
//! balance, a record of activity, a multisig, a token they added there,
//! settings of their own for it, or the network added by hand before Vela
//! shipped it. Never is a network hidden that holds their money as far as
//! the device knows. After that the record is the person's word and nothing
//! re-decides it.
//!
//! Reading no record is reading every network as shown: a shell that has not
//! settled yet (or could not read the record) never hides anything.
//!
//! Pure: no I/O.

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::network_admin::{NetBuiltinChain, NetNetworkConfig, BUILTIN_CHAINS};

/// Where the record is stored: one for the device, every wallet's — as the
/// network list itself is.
pub const KEY: &str = "vela.shownNetworks";

/// The record's shape. A record of a later shape is read as it is, never
/// settled again.
pub const RECORD_VERSION: u32 = 1;

/// The person's word on the networks shown only on it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetShownNetworks {
    #[serde(default)]
    pub version: u32,
    /// The ones of them the person's lists show, by chain id. Order means
    /// nothing: lists follow [`BUILTIN_CHAINS`]' order.
    #[serde(default)]
    pub shown: Vec<u32>,
}

/// What a device holds that ties its person to a network — each shell
/// gathers it from its own stores; what counts is decided here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetUseEvidence {
    /// Networks where a last-known holding is more than zero — any account,
    /// either space, however old the reading. A spam token is not money.
    #[serde(default)]
    pub balance_chain_ids: Vec<u32>,
    /// Networks with a record of activity (the device's history).
    #[serde(default)]
    pub activity_chain_ids: Vec<u32>,
    /// Networks a multisig on this device is used on.
    #[serde(default)]
    pub multisig_chain_ids: Vec<u32>,
    /// Networks the person chose something on: a token they added there.
    #[serde(default)]
    pub chosen_chain_ids: Vec<u32>,
    /// The stored per-network settings (`vela.networkConfig`).
    #[serde(default)]
    pub overrides: Vec<NetNetworkConfig>,
    /// The chains of the networks the person added (`vela.customNetworks`).
    #[serde(default)]
    pub added_chain_ids: Vec<u32>,
}

/// Why a network stayed shown when the device was settled — the first
/// reason found, in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetKeptBecause {
    Balance,
    Activity,
    Multisig,
    /// An RPC or explorer of the person's own (not the one Vela ships).
    OwnSettings,
    /// Added by hand before Vela shipped it.
    AddedByHand,
    /// A token the person added there.
    Chosen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetShownKept {
    pub chain_id: u32,
    pub because: NetKeptBecause,
}

/// [`settle`]'s answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetShownSettled {
    /// The record the device now goes by.
    pub record: NetShownNetworks,
    /// The device had no record and this is its one settling: the shell
    /// stores `record`. `false`: the stored record stands, nothing to write.
    pub write: bool,
    /// On the settling: the networks that stayed shown, and why.
    pub kept: Vec<NetShownKept>,
}

/// The networks shown only on the person's word, in list order.
pub fn opt_in_chains() -> impl Iterator<Item = &'static NetBuiltinChain> {
    BUILTIN_CHAINS.iter().filter(|c| !c.shown_by_default)
}

/// A Vela network the lists show only on the person's word.
#[must_use]
pub fn is_opt_in(chain_id: u32) -> bool {
    opt_in_chains().any(|c| c.chain_id == chain_id)
}

/// Whether the lists show `chain_id`: a network shown by default, or one the
/// person added, always; one shown on their word when the record says so —
/// and every one when there is no record yet.
#[must_use]
pub fn is_shown(chain_id: u32, record: Option<&NetShownNetworks>) -> bool {
    if !is_opt_in(chain_id) {
        return true;
    }
    record.is_none_or(|r| r.shown.contains(&chain_id))
}

/// The Vela networks out of the person's lists, in list order — the ones
/// Settings → Add network offers to show again.
#[must_use]
pub fn hidden_chain_ids(record: Option<&NetShownNetworks>) -> Vec<u32> {
    opt_in_chains()
        .map(|c| c.chain_id)
        .filter(|id| !is_shown(*id, record))
        .collect()
}

/// The record after the person showed (`on`) or hid `chain_id`. Starting
/// from no record is starting from every one shown, as it reads.
#[must_use]
pub fn with_shown(record: Option<&NetShownNetworks>, chain_id: u32, on: bool) -> NetShownNetworks {
    let mut shown: Vec<u32> = opt_in_chains()
        .map(|c| c.chain_id)
        .filter(|id| is_shown(*id, record))
        .collect();
    // A later shape's ids this table does not know are kept as they are.
    if let Some(r) = record {
        for id in &r.shown {
            if !is_opt_in(*id) && !shown.contains(id) {
                shown.push(*id);
            }
        }
    }
    shown.retain(|id| *id != chain_id);
    if on && is_opt_in(chain_id) {
        shown.push(chain_id);
    }
    NetShownNetworks {
        version: record.map_or(RECORD_VERSION, |r| r.version.max(RECORD_VERSION)),
        shown,
    }
}

/// An override that is the person's own: an RPC or an explorer set and not
/// the one Vela ships. A card opened and left unchanged writes the shipped
/// values back — that is no setting of theirs.
fn own_settings(config: &NetNetworkConfig, chain: &NetBuiltinChain) -> bool {
    let own = |value: &str, shipped: &str| {
        let value = value.trim();
        !value.is_empty() && value.trim_end_matches('/') != shipped.trim_end_matches('/')
    };
    own(&config.rpc_url, chain.rpc_url) || own(&config.explorer_url, chain.explorer_url)
}

/// Why `chain` stays shown on this device, if it does.
fn kept_because(chain: &NetBuiltinChain, evidence: &NetUseEvidence) -> Option<NetKeptBecause> {
    let id = chain.chain_id;
    if evidence.balance_chain_ids.contains(&id) {
        Some(NetKeptBecause::Balance)
    } else if evidence.activity_chain_ids.contains(&id) {
        Some(NetKeptBecause::Activity)
    } else if evidence.multisig_chain_ids.contains(&id) {
        Some(NetKeptBecause::Multisig)
    } else if evidence
        .overrides
        .iter()
        .any(|c| c.chain_id == id && own_settings(c, chain))
    {
        Some(NetKeptBecause::OwnSettings)
    } else if evidence.added_chain_ids.contains(&id) {
        Some(NetKeptBecause::AddedByHand)
    } else if evidence.chosen_chain_ids.contains(&id) {
        Some(NetKeptBecause::Chosen)
    } else {
        None
    }
}

/// The device's record: the stored one as it stands, else — once — the one
/// the evidence settles: every network shown on the person's word stays
/// shown wherever they use it, and the rest leave the lists.
#[must_use]
pub fn settle(stored: Option<NetShownNetworks>, evidence: &NetUseEvidence) -> NetShownSettled {
    if let Some(record) = stored {
        return NetShownSettled {
            record,
            write: false,
            kept: Vec::new(),
        };
    }
    let kept: Vec<NetShownKept> = opt_in_chains()
        .filter_map(|chain| {
            kept_because(chain, evidence).map(|because| NetShownKept {
                chain_id: chain.chain_id,
                because,
            })
        })
        .collect();
    NetShownSettled {
        record: NetShownNetworks {
            version: RECORD_VERSION,
            shown: kept.iter().map(|k| k.chain_id).collect(),
        },
        write: true,
        kept,
    }
}

/// The stored record, read: absent, empty or unreadable bytes are no record.
#[must_use]
pub fn read_record(stored: Option<&str>) -> Option<NetShownNetworks> {
    let raw = stored.map(str::trim).filter(|raw| !raw.is_empty())?;
    serde_json::from_str(raw).ok()
}

/// [`settle`] for a shell holding JSON: the stored bytes (`""`: none) and
/// the evidence (unreadable: none found). A [`NetShownSettled`] as JSON.
#[must_use]
pub fn settle_json(stored: &str, evidence_json: &str) -> String {
    let evidence: NetUseEvidence = serde_json::from_str(evidence_json).unwrap_or_default();
    serde_json::to_string(&settle(read_record(Some(stored)), &evidence))
        .unwrap_or_else(|_| "{}".to_owned())
}

/// [`hidden_chain_ids`] for a shell holding the stored bytes (`""`: none).
#[must_use]
pub fn hidden_chain_ids_json(stored: &str) -> Vec<u32> {
    hidden_chain_ids(read_record(Some(stored)).as_ref())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const STABLE: u32 = 988;
    const MEGAETH: u32 = 4326;
    const PLUME: u32 = 98_866;
    const XRPL: u32 = 1_440_000;

    fn config(chain_id: u32, rpc_url: &str, explorer_url: &str) -> NetNetworkConfig {
        NetNetworkConfig {
            chain_id,
            rpc_url: rpc_url.to_owned(),
            explorer_url: explorer_url.to_owned(),
            bundler_url: String::new(),
            only_mine: false,
        }
    }

    #[test]
    fn the_four_are_shown_on_the_persons_word_and_twenty_by_default() {
        let opt_in: Vec<u32> = opt_in_chains().map(|c| c.chain_id).collect();
        assert_eq!(opt_in, [STABLE, MEGAETH, PLUME, XRPL]);
        let default: Vec<u32> = BUILTIN_CHAINS
            .iter()
            .filter(|c| !c.testnet && is_shown(c.chain_id, Some(&NetShownNetworks::default())))
            .map(|c| c.chain_id)
            .collect();
        assert_eq!(default.len(), 20);
        // Every testnet is in its space's list.
        assert!(BUILTIN_CHAINS
            .iter()
            .filter(|c| c.testnet)
            .all(|c| c.shown_by_default));
    }

    #[test]
    fn a_fresh_profile_settles_on_the_twenty() {
        let settled = settle(None, &NetUseEvidence::default());
        assert!(settled.write);
        assert!(settled.record.shown.is_empty());
        assert_eq!(settled.record.version, RECORD_VERSION);
        assert_eq!(
            hidden_chain_ids(Some(&settled.record)),
            [STABLE, MEGAETH, PLUME, XRPL]
        );
    }

    #[test]
    fn a_balance_keeps_its_network_shown() {
        let evidence = NetUseEvidence {
            balance_chain_ids: vec![1, PLUME],
            ..NetUseEvidence::default()
        };
        let settled = settle(None, &evidence);
        assert_eq!(settled.record.shown, [PLUME]);
        assert_eq!(
            settled.kept,
            [NetShownKept {
                chain_id: PLUME,
                because: NetKeptBecause::Balance
            }]
        );
        assert!(is_shown(PLUME, Some(&settled.record)));
        assert!(!is_shown(STABLE, Some(&settled.record)));
    }

    #[test]
    fn activity_a_multisig_or_a_token_keeps_its_network_shown() {
        let evidence = NetUseEvidence {
            activity_chain_ids: vec![STABLE],
            multisig_chain_ids: vec![MEGAETH],
            chosen_chain_ids: vec![XRPL],
            ..NetUseEvidence::default()
        };
        let settled = settle(None, &evidence);
        assert_eq!(settled.record.shown, [STABLE, MEGAETH, XRPL]);
        let because: Vec<NetKeptBecause> = settled.kept.iter().map(|k| k.because).collect();
        assert_eq!(
            because,
            [
                NetKeptBecause::Activity,
                NetKeptBecause::Multisig,
                NetKeptBecause::Chosen
            ]
        );
    }

    #[test]
    fn settings_of_their_own_keep_it_shown_but_vela_s_own_values_do_not() {
        let plume = BUILTIN_CHAINS.iter().find(|c| c.chain_id == PLUME).unwrap();
        let stable = BUILTIN_CHAINS
            .iter()
            .find(|c| c.chain_id == STABLE)
            .unwrap();
        let evidence = NetUseEvidence {
            overrides: vec![
                // Their own node.
                config(MEGAETH, "https://my-node.example", ""),
                // A card opened and left: Vela's own values written back.
                config(PLUME, plume.rpc_url, plume.explorer_url),
                // Their own explorer only.
                config(XRPL, "", "https://my-explorer.example"),
                // Vela's own RPC with a trailing slash is still Vela's.
                config(STABLE, &format!("{}/", stable.rpc_url), ""),
            ],
            ..NetUseEvidence::default()
        };
        let settled = settle(None, &evidence);
        assert_eq!(settled.record.shown, [MEGAETH, XRPL]);
        assert!(settled
            .kept
            .iter()
            .all(|k| k.because == NetKeptBecause::OwnSettings));
    }

    #[test]
    fn a_network_added_by_hand_before_vela_shipped_it_stays() {
        let evidence = NetUseEvidence {
            added_chain_ids: vec![PLUME, 4242],
            ..NetUseEvidence::default()
        };
        let settled = settle(None, &evidence);
        assert_eq!(settled.record.shown, [PLUME]);
        assert_eq!(settled.kept[0].because, NetKeptBecause::AddedByHand);
    }

    #[test]
    fn nothing_found_and_the_network_leaves_the_lists() {
        let evidence = NetUseEvidence {
            balance_chain_ids: vec![1, 8453],
            activity_chain_ids: vec![10],
            overrides: vec![config(1, "https://my-node.example", "")],
            ..NetUseEvidence::default()
        };
        let settled = settle(None, &evidence);
        assert!(settled.record.shown.is_empty());
        assert!(settled.kept.is_empty());
        // A network shown by default is never out of the lists.
        assert!(is_shown(1, Some(&settled.record)));
    }

    #[test]
    fn settling_happens_once() {
        let first = settle(None, &NetUseEvidence::default());
        // Later, a balance on Plume: the record stands — it is the person's
        // word now, and nothing is written.
        let evidence = NetUseEvidence {
            balance_chain_ids: vec![PLUME],
            ..NetUseEvidence::default()
        };
        let again = settle(Some(first.record.clone()), &evidence);
        assert!(!again.write);
        assert_eq!(again.record, first.record);
        // And through the stored bytes, as a shell does it.
        let stored = serde_json::to_string(&first.record).unwrap();
        let json: NetShownSettled =
            serde_json::from_str(&settle_json(&stored, r#"{"balance_chain_ids":[98866]}"#))
                .unwrap();
        assert!(!json.write);
        assert!(json.record.shown.is_empty());
    }

    #[test]
    fn no_record_hides_nothing_and_unreadable_bytes_are_no_record() {
        for id in [STABLE, MEGAETH, PLUME, XRPL, 1] {
            assert!(is_shown(id, None));
        }
        assert!(hidden_chain_ids(None).is_empty());
        assert_eq!(read_record(Some("not json")), None);
        assert_eq!(read_record(Some("  ")), None);
        assert_eq!(read_record(None), None);
        let settled: NetShownSettled = serde_json::from_str(&settle_json("{broken", "")).unwrap();
        assert!(settled.write);
    }

    #[test]
    fn showing_and_hiding_keep_the_rest_of_the_record() {
        let none = NetShownNetworks {
            version: RECORD_VERSION,
            shown: vec![],
        };
        let plume = with_shown(Some(&none), PLUME, true);
        assert_eq!(plume.shown, [PLUME]);
        let both = with_shown(Some(&plume), XRPL, true);
        assert_eq!(both.shown, [PLUME, XRPL]);
        // Twice is once.
        assert_eq!(with_shown(Some(&both), XRPL, true), both);
        let back = with_shown(Some(&both), PLUME, false);
        assert_eq!(back.shown, [XRPL]);
        // Hiding with no record yet: the rest stay as they read — shown.
        let from_none = with_shown(None, STABLE, false);
        assert_eq!(from_none.shown, [MEGAETH, PLUME, XRPL]);
        // A network shown by default is never written into the record.
        assert_eq!(with_shown(Some(&none), 1, true).shown, Vec::<u32>::new());
        // A later shape's unknown ids survive an edit.
        let later = NetShownNetworks {
            version: 2,
            shown: vec![777_777],
        };
        let edited = with_shown(Some(&later), PLUME, true);
        assert_eq!(edited.version, 2);
        assert_eq!(edited.shown, [777_777, PLUME]);
    }
}
