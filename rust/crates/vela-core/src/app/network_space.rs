//! The mainnet space and the testnet space (spec 105).
//!
//! Every network belongs to one: a built-in by the table
//! ([`super::network_admin::BUILTIN_CHAINS`]'s `testnet`), a network the
//! person added by their word ([`super::network_admin::custom_is_testnet`]:
//! a testnet unless they said otherwise). One device-wide switch says which
//! space is in front; every list of networks a shell builds — the balances it
//! reads, send and receive, the dApps' chains — goes through
//! [`space_chain_ids`], so the four shells cannot disagree on what a space
//! holds.
//!
//! Pure: no I/O.

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::network_admin::{NetCustomNetwork, NetNetworkRow, BUILTIN_CHAINS};

/// Where the space is stored: one value for the device, every wallet's.
pub const KEY: &str = "vela.networkSpace";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetworkSpace {
    /// Networks whose assets have real value. Where a device starts.
    #[default]
    Mainnet,
    /// Testnets and developer networks: nothing here has real value.
    Testnet,
}

impl NetworkSpace {
    /// As stored and as said on the wire.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            NetworkSpace::Mainnet => "mainnet",
            NetworkSpace::Testnet => "testnet",
        }
    }

    /// The stored value, read: anything missing or unreadable is the mainnet
    /// space.
    #[must_use]
    pub fn parse(stored: Option<&str>) -> NetworkSpace {
        match stored.map(str::trim) {
            Some("testnet") => NetworkSpace::Testnet,
            _ => NetworkSpace::Mainnet,
        }
    }

    /// The space a network of that kind belongs to.
    #[must_use]
    pub fn of(testnet: bool) -> NetworkSpace {
        if testnet {
            NetworkSpace::Testnet
        } else {
            NetworkSpace::Mainnet
        }
    }
}

/// Whether `chain_id` is a testnet: the built-in table's word, else the
/// person's for a network they added, else — a chain nobody vouched for — a
/// testnet.
#[must_use]
pub fn chain_is_testnet(chain_id: u32, custom: &[NetCustomNetwork]) -> bool {
    if let Some(chain) = BUILTIN_CHAINS.iter().find(|c| c.chain_id == chain_id) {
        return chain.testnet;
    }
    custom
        .iter()
        .find(|n| n.chain_id == chain_id)
        .is_none_or(super::network_admin::custom_is_testnet)
}

/// The networks of `space`, in the rows' order (the network list the
/// network machine draws: built-ins first, then the person's).
#[must_use]
pub fn space_rows(space: NetworkSpace, rows: &[NetNetworkRow]) -> Vec<NetNetworkRow> {
    rows.iter()
        .filter(|row| NetworkSpace::of(row.is_testnet) == space)
        .cloned()
        .collect()
}

/// The chain ids of `space`'s networks — what a shell reads balances on, and
/// offers to send, receive and a dApp.
#[must_use]
pub fn space_chain_ids(space: NetworkSpace, rows: &[NetNetworkRow]) -> Vec<u32> {
    space_rows(space, rows)
        .iter()
        .map(|row| row.chain_id)
        .collect()
}

/// [`space_chain_ids`] for a shell holding the network list as JSON (the
/// network machine's `NetView.networks`): `space` as stored, the ids as a
/// JSON array. Unreadable rows are no rows.
#[must_use]
pub fn space_chain_ids_json(space: &str, rows_json: &str) -> String {
    let rows: Vec<NetNetworkRow> = serde_json::from_str(rows_json).unwrap_or_default();
    serde_json::to_string(&space_chain_ids(NetworkSpace::parse(Some(space)), &rows))
        .unwrap_or_else(|_| "[]".to_owned())
}

/// The chain a site with no chain of its own starts on (dapp-17) — no switch
/// of its own and no grant: the network the person last put a site on in
/// this space (`last_used`), while it is still one of the space's networks;
/// else the space's first network (`space_chain_ids`, in the list's order);
/// else Ethereum. A testnet-space wallet no longer starts a site on mainnet.
#[must_use]
pub fn site_start_chain(space_chain_ids: &[u32], last_used: Option<u32>) -> u32 {
    site_start_chain_for(space_chain_ids, last_used, &[])
}

/// [`site_start_chain`] for an account (r9, extension): after the network
/// the person last put a site on, the first of `account_chains` — the
/// networks the active account holds something on or used, most telling
/// first — that the space offers; only then the space's first network. A
/// plain account holding only Arc USDC no longer starts a site on Sepolia.
#[must_use]
pub fn site_start_chain_for(
    space_chain_ids: &[u32],
    last_used: Option<u32>,
    account_chains: &[u32],
) -> u32 {
    if let Some(chain_id) = last_used.filter(|c| space_chain_ids.contains(c)) {
        return chain_id;
    }
    account_chains
        .iter()
        .find(|c| space_chain_ids.contains(c))
        .or_else(|| space_chain_ids.first())
        .copied()
        .unwrap_or(1)
}

/// [`chain_is_testnet`] for a shell holding the stored `vela.customNetworks`
/// as JSON (the core's own snake-case records; unreadable is none).
#[must_use]
pub fn chain_is_testnet_json(chain_id: u32, custom_json: &str) -> bool {
    let custom: Vec<NetCustomNetwork> = serde_json::from_str(custom_json).unwrap_or_default();
    chain_is_testnet(chain_id, &custom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_site_starts_on_the_network_last_used_in_this_space() {
        // Last used, and still in the space.
        assert_eq!(site_start_chain(&[1, 8453, 100], Some(100)), 100);
        // Never used, or used in the other space / no longer there: the space's first.
        assert_eq!(site_start_chain(&[1, 8453, 100], None), 1);
        assert_eq!(
            site_start_chain(&[11_155_111, 84_532], Some(8453)),
            11_155_111
        );
        assert_eq!(
            site_start_chain(&[11_155_111, 84_532], Some(84_532)),
            84_532
        );
        // A space with no network at all: Ethereum, as before.
        assert_eq!(site_start_chain(&[], Some(100)), 1);
    }

    #[test]
    fn a_new_site_starts_where_the_account_holds_or_used_within_the_space() {
        // r9: a plain account holding only Arc USDC, testnet space.
        let space = [11_155_111, 84_532, 5_042_002];
        assert_eq!(site_start_chain_for(&space, None, &[5_042_002]), 5_042_002);
        // The network last used comes first; one outside the space never.
        assert_eq!(
            site_start_chain_for(&space, Some(84_532), &[5_042_002]),
            84_532
        );
        assert_eq!(
            site_start_chain_for(&space, None, &[8453, 5_042_002]),
            5_042_002
        );
        assert_eq!(site_start_chain_for(&space, None, &[8453]), 11_155_111);
        assert_eq!(site_start_chain_for(&[], None, &[8453]), 1);
    }

    fn custom(chain_id: u32, is_testnet: Option<bool>) -> NetCustomNetwork {
        NetCustomNetwork {
            id: format!("custom-{chain_id}"),
            display_name: "Mine".to_owned(),
            chain_id,
            icon_label: String::new(),
            icon_color: String::new(),
            icon_bg: String::new(),
            logo_url: String::new(),
            is_l2: false,
            rpc_url: String::new(),
            explorer_url: String::new(),
            bundler_url: String::new(),
            native_symbol: "X".to_owned(),
            added_at_iso: String::new(),
            is_testnet,
        }
    }

    #[test]
    fn a_device_starts_in_the_mainnet_space() {
        assert_eq!(NetworkSpace::parse(None), NetworkSpace::Mainnet);
        assert_eq!(NetworkSpace::parse(Some("nonsense")), NetworkSpace::Mainnet);
        assert_eq!(NetworkSpace::parse(Some("testnet")), NetworkSpace::Testnet);
        assert_eq!(
            NetworkSpace::parse(Some(NetworkSpace::Mainnet.as_str())),
            NetworkSpace::Mainnet
        );
    }

    #[test]
    fn every_network_belongs_to_one_space() {
        let mine = [
            custom(4242, Some(false)),
            custom(31337, None),
            custom(7777, Some(true)),
        ];
        // The table's word.
        assert!(!chain_is_testnet(1, &mine), "Ethereum");
        assert!(chain_is_testnet(11_155_111, &mine), "Sepolia");
        assert!(chain_is_testnet(5_042_002, &mine), "Arc Testnet");
        // The person's word; an older record, and a chain nobody knows: testnet.
        assert!(!chain_is_testnet(4242, &mine));
        assert!(chain_is_testnet(31337, &mine));
        assert!(chain_is_testnet(7777, &mine));
        assert!(chain_is_testnet(999_999, &mine));
    }

    #[test]
    fn the_built_in_testnets_are_the_testnet_spaces() {
        let testnets: Vec<u32> = BUILTIN_CHAINS
            .iter()
            .filter(|c| c.testnet)
            .map(|c| c.chain_id)
            .collect();
        assert_eq!(
            testnets,
            [11_155_111, 84_532, 421_614, 11_155_420, 80_002, 97, 43_113, 5_042_002, 42_431]
        );
        assert_eq!(BUILTIN_CHAINS.iter().filter(|c| !c.testnet).count(), 24);
    }
}
