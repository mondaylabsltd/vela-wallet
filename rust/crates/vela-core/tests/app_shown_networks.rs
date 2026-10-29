//! The Vela networks shown on the person's word (owner decision 2026-10-29):
//! Stable, MegaETH, Plume and XRPL EVM stay Vela networks in the catalog, out
//! of the default list. The network machine draws only what the record shows,
//! offers the rest back through Add network — as Vela networks, in one step —
//! and a site asking for one gets Vela's sheet for it, never a custom one.
//! Taking one out of the list keeps everything stored for it.
//!
//! The device's one-time settling of the record is `shown_networks::settle`
//! (its unit tests sit beside it); these drive the machine the way a shell
//! does, from the record it read.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::dapp_rpc::{DappAddOutcome, DappChainAsk};
use vela_core::app::network_admin::{
    Event, NetDappAddPhase, NetNetworkConfig, NetOperation as Op, NetProviderKeys,
    NetShellResult as Res, NetStoredEndpoints, NetWizardPhase, NetworkAdmin, BUILTIN_CHAINS,
};
use vela_core::app::network_space::{space_chain_ids, NetworkSpace};
use vela_core::app::shown_networks::{NetShownNetworks, RECORD_VERSION};

type Sut = DomainDriver<NetworkAdmin>;

const STABLE: u32 = 988;
const MEGAETH: u32 = 4326;
const PLUME: u32 = 98_866;
const XRPL: u32 = 1_440_000;
const NOW_ISO: &str = "2026-10-29T12:00:00.000Z";
const MY_NODE: &str = "https://my-plume-node.example";

/// The owner's order for the twenty shown by default (2026-10-29).
const DEFAULT_ORDER: [u32; 20] = [
    1, 56, 8453, 137, 42161, 43114, 10, 143, 4663, 5042, 196, 5000, 100, 4217, 480, 42220, 130,
    57073, 1868, 8217,
];

fn record(shown: &[u32]) -> NetShownNetworks {
    NetShownNetworks {
        version: RECORD_VERSION,
        shown: shown.to_vec(),
    }
}

fn started(shown: Option<NetShownNetworks>, overrides: Vec<NetNetworkConfig>) -> Sut {
    let mut sut = Sut::new();
    assert_eq!(sut.dispatch(Event::Started), vec![Op::ReadStore]);
    sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: overrides,
        endpoints: NetStoredEndpoints::default(),
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: shown,
    });
    sut
}

fn mainnets(sut: &Sut) -> Vec<u32> {
    space_chain_ids(NetworkSpace::Mainnet, &sut.view().networks)
}

fn written(ops: &[Op]) -> Option<NetShownNetworks> {
    ops.iter().find_map(|op| match op {
        Op::WriteShownNetworks { record } => Some(record.clone()),
        _ => None,
    })
}

fn my_node() -> NetNetworkConfig {
    NetNetworkConfig {
        chain_id: PLUME,
        rpc_url: MY_NODE.to_owned(),
        explorer_url: String::new(),
        bundler_url: String::new(),
        only_mine: true,
    }
}

#[test]
fn the_catalog_is_in_the_owners_order_with_the_four_after_the_twenty() {
    let mainnet: Vec<u32> = BUILTIN_CHAINS
        .iter()
        .filter(|c| !c.testnet)
        .map(|c| c.chain_id)
        .collect();
    let mut expected = DEFAULT_ORDER.to_vec();
    expected.extend([STABLE, MEGAETH, PLUME, XRPL]);
    assert_eq!(mainnet, expected);
    // Still Vela networks, every one: the catalog has 24 mainnets.
    assert_eq!(mainnet.len(), 24);
}

#[test]
fn a_settled_fresh_profile_lists_the_twenty_in_order() {
    let sut = started(Some(record(&[])), vec![]);
    assert_eq!(mainnets(&sut), DEFAULT_ORDER);
    assert_eq!(
        sut.view().hidden_vela_chain_ids,
        [STABLE, MEGAETH, PLUME, XRPL]
    );
    // The testnet space is untouched.
    let testnets = space_chain_ids(NetworkSpace::Testnet, &sut.view().networks);
    assert_eq!(testnets.len(), 9);
}

#[test]
fn a_device_with_no_record_hides_nothing() {
    let sut = started(None, vec![]);
    assert_eq!(mainnets(&sut).len(), 24);
    assert!(sut.view().hidden_vela_chain_ids.is_empty());
}

#[test]
fn a_shown_one_follows_the_twenty_and_can_be_removed() {
    let sut = started(Some(record(&[PLUME])), vec![]);
    let ids = mainnets(&sut);
    assert_eq!(ids.len(), 21);
    assert_eq!(ids[20], PLUME, "after Kaia");
    let plume = sut
        .view()
        .networks
        .into_iter()
        .find(|row| row.chain_id == PLUME)
        .expect("listed");
    assert!(plume.optional && !plume.is_custom);
    // A network shown by default is never removable.
    let eth = sut.view().networks.into_iter().next().expect("Ethereum");
    assert!(!eth.optional);
}

#[test]
fn add_network_finds_a_hidden_one_as_a_vela_network() {
    let mut sut = started(Some(record(&[])), vec![]);
    sut.dispatch(Event::SearchInput {
        query: "plu".to_owned(),
    });
    // The chain index is down: answers nothing. Plume is found all the same.
    sut.resolve(Res::DebounceElapsed);
    sut.resolve(Res::SearchIndex { chains: vec![] });
    let wizard = sut.view().wizard;
    assert_eq!(wizard.suggestions.len(), 1);
    assert_eq!(wizard.suggestions[0].chain_id, PLUME);
    assert!(wizard.suggestions[0].vela);
    assert_eq!(wizard.suggestions[0].name, "Plume");
}

#[test]
fn picking_a_hidden_one_needs_no_check_and_adding_shows_it_again() {
    let mut sut = started(Some(record(&[])), vec![]);
    let ops = sut.dispatch(Event::ChainSelected {
        chain_id: PLUME,
        keep_custom_rpc: false,
    });
    assert!(ops.is_empty(), "nothing fetched, nothing probed: {ops:?}");
    let wizard = sut.view().wizard;
    assert_eq!(wizard.phase, NetWizardPhase::Checked);
    assert!(wizard.vela_network && wizard.can_add);
    assert_eq!(wizard.error, None, "never 'already added'");
    assert_eq!(
        wizard.chain_info.as_ref().map(|i| i.name.as_str()),
        Some("Plume")
    );
    assert!(!wizard.testnet, "a mainnet, as Vela ships it");

    let ops = sut.dispatch(Event::AddConfirmed {
        now_iso: NOW_ISO.to_owned(),
    });
    assert_eq!(written(&ops), Some(record(&[PLUME])));
    assert!(
        !ops.iter()
            .any(|op| matches!(op, Op::WriteCustomNetworks { .. })),
        "never saved as a custom network"
    );
    let view = sut.view();
    assert_eq!(view.last_added_chain_id, Some(PLUME));
    assert!(view.networks.iter().any(|row| row.chain_id == PLUME));
    assert_eq!(view.hidden_vela_chain_ids, [STABLE, MEGAETH, XRPL]);
    // Shown now: picking it again is "already added".
    sut.dispatch(Event::ChainSelected {
        chain_id: PLUME,
        keep_custom_rpc: false,
    });
    assert_eq!(sut.view().wizard.phase, NetWizardPhase::Error);
}

#[test]
fn removing_one_keeps_its_settings_and_showing_it_again_restores_them() {
    let mut sut = started(Some(record(&[PLUME])), vec![my_node()]);
    let ops = sut.dispatch(Event::DeleteConfirmed {
        id: "plume".to_owned(),
    });
    assert_eq!(written(&ops), Some(record(&[])));
    assert!(
        !ops.iter()
            .any(|op| matches!(op, Op::WriteNetworkConfigs { .. })),
        "its own node stays stored: {ops:?}"
    );
    assert!(!mainnets(&sut).contains(&PLUME));

    sut.dispatch(Event::ChainSelected {
        chain_id: PLUME,
        keep_custom_rpc: false,
    });
    sut.dispatch(Event::AddConfirmed {
        now_iso: NOW_ISO.to_owned(),
    });
    let plume = sut
        .view()
        .networks
        .into_iter()
        .find(|row| row.chain_id == PLUME)
        .expect("shown again");
    assert_eq!(plume.rpc_url, MY_NODE);
    assert!(plume.own_rpc && plume.only_mine);
}

#[test]
fn a_network_shown_by_default_cannot_be_removed() {
    let mut sut = started(Some(record(&[])), vec![]);
    let ops = sut.dispatch(Event::DeleteConfirmed {
        id: "ethereum".to_owned(),
    });
    assert!(written(&ops).is_none());
    assert_eq!(mainnets(&sut).len(), 20);
}

#[test]
fn the_scan_path_shows_a_hidden_one_straight() {
    let mut sut = started(Some(record(&[])), vec![]);
    let ops = sut.dispatch(Event::AddByChainIdRequested {
        chain_id: XRPL,
        now_iso: NOW_ISO.to_owned(),
    });
    assert_eq!(written(&ops), Some(record(&[XRPL])));
}

fn ask(chain_id: u32) -> DappChainAsk {
    DappChainAsk {
        chain_id,
        chain_name: Some("Plume from the site".to_owned()),
        native_symbol: Some("SITE".to_owned()),
        rpc_urls: vec!["https://rpc.site.example".to_owned()],
        refused_rpc_urls: 0,
        explorer_url: None,
    }
}

#[test]
fn a_site_asking_for_a_hidden_one_gets_vela_s_sheet() {
    let mut sut = started(Some(record(&[])), vec![]);
    let ops = sut.dispatch(Event::DappAddRequested {
        tab: "t1".to_owned(),
        id: "a1".to_owned(),
        origin: "https://dapp.example".to_owned(),
        ask: ask(PLUME),
    });
    assert!(
        !ops.iter().any(|op| matches!(op, Op::FetchChainInfo { .. })),
        "nothing to check: {ops:?}"
    );
    let sheet = sut.view().dapp_add.expect("the sheet");
    assert_eq!(sheet.phase, NetDappAddPhase::Ready);
    assert!(sheet.vela_network && sheet.can_add && !sheet.from_site);
    // Vela's words, not the page's.
    assert_eq!(sheet.name, "Plume");
    assert_eq!(sheet.native_symbol, "PLUME");

    let ops = sut.dispatch(Event::DappAddApproved {
        now_iso: NOW_ISO.to_owned(),
    });
    assert_eq!(written(&ops), Some(record(&[PLUME])));
    assert!(!ops
        .iter()
        .any(|op| matches!(op, Op::WriteCustomNetworks { .. })));
    // The page hears "added" once the store has it.
    let ops = sut.resolve_matching(
        |op| matches!(op, Op::WriteShownNetworks { .. }),
        Res::Written,
    );
    assert!(ops.iter().any(|op| matches!(
        op,
        Op::DappAddSettled {
            outcome: DappAddOutcome::Added { chain_id: PLUME },
            ..
        }
    )));
    assert!(mainnets(&sut).contains(&PLUME));
}

#[test]
fn a_site_asking_for_a_shown_one_is_a_switch() {
    let mut sut = started(Some(record(&[PLUME])), vec![]);
    let ops = sut.dispatch(Event::DappAddRequested {
        tab: "t1".to_owned(),
        id: "a1".to_owned(),
        origin: "https://dapp.example".to_owned(),
        ask: ask(PLUME),
    });
    assert!(ops.iter().any(|op| matches!(
        op,
        Op::DappAddSettled {
            outcome: DappAddOutcome::Added { chain_id: PLUME },
            ..
        }
    )));
    assert!(sut.view().dapp_add.is_none());
}
