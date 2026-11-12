//! Rules of the network-admin machine, one test per rule.
//!
//! Inventory invariants ①–⑨ each have at least one test named after the
//! rule; the recorded quirks (the reset-without-pool-flush, the scan path
//! flattening `rpcFailed`) are pinned as tests too, so an accidental "fix"
//! fails loudly instead of silently changing product behavior.
//!
//! Invariant ④ is no longer one of those quirks: spec 017 closed the
//! save-without-a-probe gap, so the tests below pin the GATE — refused on a
//! proven mismatch, saved on anything unverifiable, held while the verdict is
//! still in flight.
//!
//! The machine tests drive the lifecycle exactly the way the shell will:
//! dispatch an event, answer the operations one at a time, in order.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::network_admin::{
    chain_setup_url, clean_endpoint_value, default_endpoint, explorer_base_url,
    is_allowed_endpoint_url, is_builtin_chain, is_code_deployed, is_localhost_http, net_blocker,
    p256_call_indicates_support, parse_chain_data, public_rpc_urls, rank_search,
    registry_says_testnet, Event, NetBlocker, NetChainIndexEntry, NetCustomNetwork,
    NetEndpointField, NetHealthBody, NetNetworkConfig, NetOperation as Op, NetOverrideField,
    NetProbeHealth, NetProviderKeys, NetRawChainData, NetRpcFailureKind, NetServiceEndpoints,
    NetServiceHealth, NetShellResult as Res, NetStoredEndpoints, NetWizardErrorKind,
    NetWizardPhase, NetworkAdmin, BUILTIN_CHAINS, DEFAULT_BUNDLER_SERVICE_URL,
    DEFAULT_ETHEREUM_DATA_URL, DEFAULT_FIAT_RATES_URL, DEFAULT_GENESIS_SERVICE_URL,
    DEFAULT_PASSKEY_INDEX_URL, MISSING_CONTRACTS_HINT, NO_P256_HINT, P256_PRECOMPILE, PUBLIC_RPCS,
    REQUIRED_CONTRACTS, SEARCH_DEBOUNCE_MS,
};
use vela_core::app::remote_mark::chain_logo_url;

type Sut = DomainDriver<NetworkAdmin>;

const NEW_CHAIN: u32 = 7777;
const RPC_SLOW: &str = "https://slow.example";
const RPC_FAST: &str = "https://fast.example";
const NOW_ISO: &str = "2026-08-09T12:00:00.000Z";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn store_loaded(customs: Vec<NetCustomNetwork>, overrides: Vec<NetNetworkConfig>) -> Res {
    Res::StoreLoaded {
        custom_networks: customs,
        network_configs: overrides,
        endpoints: NetStoredEndpoints::default(),
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: None,
    }
}

fn started_with(customs: Vec<NetCustomNetwork>, overrides: Vec<NetNetworkConfig>) -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Started);
    assert_eq!(ops, vec![Op::ReadStore]);
    let ops = sut.resolve(store_loaded(customs, overrides));
    assert!(ops.is_empty());
    sut
}

fn started() -> Sut {
    started_with(vec![], vec![])
}

fn custom_network(chain_id: u32) -> NetCustomNetwork {
    NetCustomNetwork {
        id: format!("custom-{chain_id}"),
        display_name: format!("Chain {chain_id}"),
        chain_id,
        icon_label: "TST".to_owned(),
        icon_color: "#888888".to_owned(),
        icon_bg: "#F0F0F0".to_owned(),
        logo_url: String::new(),
        is_l2: false,
        rpc_url: "https://custom-rpc.example".to_owned(),
        explorer_url: "https://custom-scan.example/".to_owned(),
        bundler_url: format!("https://custom-bundler.example/{chain_id}"),
        native_symbol: "TST".to_owned(),
        added_at_iso: "2026-08-01T00:00:00.000Z".to_owned(),
        is_testnet: None,
    }
}

/// Registry data with two clean HTTPS RPCs — slow listed first.
fn raw_chain() -> NetRawChainData {
    NetRawChainData {
        chain_id: None,
        name: Some("Testland".to_owned()),
        short_name: Some("test".to_owned()),
        native_currency_name: Some("Test Ether".to_owned()),
        native_currency_symbol: Some("TST".to_owned()),
        native_currency_decimals: Some(18),
        rpc: vec![RPC_SLOW.to_owned(), RPC_FAST.to_owned()],
        explorers: vec!["https://scan.example".to_owned()],
        testnet: false,
        slip44: None,
    }
}

fn probe(url: &str, reported: Option<u32>, latency_ms: f64) -> Res {
    Res::Probed {
        url: url.to_owned(),
        reported_chain_id: reported,
        latency_ms,
    }
}

fn code_result(url: &str, address: &str, code: Option<&str>) -> Res {
    Res::Code {
        url: url.to_owned(),
        address: address.to_owned(),
        code: code.map(str::to_owned),
    }
}

fn code_ok(url: &str, address: &str) -> Res {
    code_result(url, address, Some("0x6080604052"))
}

/// A 32-byte `0x…01` — the P256 probe's success value (len 66, value 1).
fn p256_one() -> String {
    format!("0x{}1", "0".repeat(63))
}

/// Select NEW_CHAIN, resolve its registry data — returns the probe ops.
fn select_and_resolve(sut: &mut Sut, data: NetRawChainData) -> Vec<Op> {
    let ops = sut.dispatch(Event::ChainSelected {
        chain_id: NEW_CHAIN,
        keep_custom_rpc: false,
    });
    assert_eq!(
        ops,
        vec![Op::FetchChainInfo {
            chain_id: NEW_CHAIN
        }]
    );
    sut.resolve(Res::ChainInfo {
        chain_id: NEW_CHAIN,
        data: Some(data),
    })
}

/// Drive the fastest-RPC race: slow answers 80ms, fast answers 20ms.
/// Returns the contract-check ops (11 × getCode + 1 × P256 call).
fn resolve_race(sut: &mut Sut) -> Vec<Op> {
    assert!(sut
        .resolve(probe(RPC_SLOW, Some(NEW_CHAIN), 80.0))
        .is_empty());
    sut.resolve(probe(RPC_FAST, Some(NEW_CHAIN), 20.0))
}

/// Answer all 12 contract checks as deployed, then the P256 call as valid.
/// Returns whatever the final resolution produced.
fn resolve_contracts_ok(sut: &mut Sut, url: &str) -> Vec<Op> {
    for (_, address, _) in REQUIRED_CONTRACTS {
        assert!(sut.resolve(code_ok(url, address)).is_empty());
    }
    sut.resolve(Res::P256Call {
        url: url.to_owned(),
        result: Some(p256_one()),
    })
}

/// A machine driven to `Checked` + compatible on NEW_CHAIN.
fn checked_compatible() -> Sut {
    let mut sut = started();
    let ops = select_and_resolve(&mut sut, raw_chain());
    assert_eq!(
        ops,
        vec![
            Op::ProbeRpc {
                url: RPC_SLOW.to_owned()
            },
            Op::ProbeRpc {
                url: RPC_FAST.to_owned()
            },
        ]
    );
    let ops = resolve_race(&mut sut);
    assert_eq!(ops.len(), 13, "12 contract checks + the P256 call");
    assert!(resolve_contracts_ok(&mut sut, RPC_FAST).is_empty());
    assert_eq!(sut.view().wizard.phase, NetWizardPhase::Checked);
    sut
}

fn endpoint_refused(sut: &Sut, field: NetEndpointField) -> bool {
    sut.view()
        .endpoints
        .into_iter()
        .find(|e| e.field == field)
        .is_some_and(|e| e.refused)
}

fn endpoint_view(sut: &Sut, field: NetEndpointField) -> NetServiceHealth {
    sut.view()
        .endpoints
        .into_iter()
        .find(|e| e.field == field)
        .map(|e| e.health)
        .expect("endpoint field present")
}

// ===========================================================================
// Startup & load gating
// ===========================================================================

#[test]
fn started_loads_the_store_and_lists_builtins_plus_customs() {
    let sut = started_with(vec![custom_network(999)], vec![]);
    let view = sut.view();
    assert!(view.loaded);
    assert_eq!(
        view.networks.len(),
        BUILTIN_CHAINS.len() + 1,
        "the built-ins + 1 custom"
    );
    let custom = &view.networks[BUILTIN_CHAINS.len()];
    assert!(custom.is_custom);
    assert_eq!(custom.chain_id, 999);
    assert!(!view.networks[0].is_custom);
}

#[test]
fn mutations_before_the_store_loads_are_dropped() {
    let mut sut = Sut::new();
    // No Started at all — the ledger is not the ledger yet.
    assert!(sut
        .dispatch(Event::ChainSelected {
            chain_id: NEW_CHAIN,
            keep_custom_rpc: false
        })
        .is_empty());
    assert!(sut
        .dispatch(Event::AddByChainIdRequested {
            chain_id: NEW_CHAIN,
            now_iso: NOW_ISO.to_owned()
        })
        .is_empty());
    assert!(sut
        .dispatch(Event::EndpointBlurred {
            field: NetEndpointField::PasskeyIndex
        })
        .is_empty());
    assert_eq!(sut.view().wizard.phase, NetWizardPhase::Idle);
}

#[test]
fn a_partially_stored_endpoints_blob_merges_with_the_defaults() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Started);
    sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![],
        endpoints: NetStoredEndpoints {
            ethereum_data_url: Some("https://my-data.example".to_owned()),
            ..Default::default()
        },
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: None,
    });
    let view = sut.view();
    assert_eq!(view.endpoints[0].value, "https://my-data.example");
    assert_eq!(view.endpoints[1].value, DEFAULT_PASSKEY_INDEX_URL);
    assert_eq!(view.endpoints[2].value, DEFAULT_BUNDLER_SERVICE_URL);
    assert_eq!(view.endpoints[3].value, DEFAULT_FIAT_RATES_URL);
}

// ===========================================================================
// Invariant ① — one chainId, one entry; ONE dedup implementation
// ===========================================================================

#[test]
fn a_builtin_chain_id_is_rejected_by_the_wizard() {
    let mut sut = started();
    let ops = sut.dispatch(Event::ChainSelected {
        chain_id: 1,
        keep_custom_rpc: false,
    });
    assert!(ops.is_empty(), "no resolve for a duplicate");
    let view = sut.view();
    assert_eq!(view.wizard.phase, NetWizardPhase::Error);
    assert_eq!(
        view.wizard.error,
        Some(NetWizardErrorKind::AlreadyAdded { chain_id: 1 })
    );
}

#[test]
fn an_existing_custom_chain_id_is_rejected_by_the_wizard() {
    let mut sut = started_with(vec![custom_network(999)], vec![]);
    assert!(sut
        .dispatch(Event::ChainSelected {
            chain_id: 999,
            keep_custom_rpc: false
        })
        .is_empty());
    assert_eq!(
        sut.view().wizard.error,
        Some(NetWizardErrorKind::AlreadyAdded { chain_id: 999 })
    );
}

/// The unification: today's scan path (`add-network.ts`) has NO dedup check —
/// the inventory collapses both callers onto this single gate.
#[test]
fn the_scan_path_passes_the_same_dedup_gate() {
    let mut sut = started_with(vec![custom_network(999)], vec![]);
    let ops = sut.dispatch(Event::AddByChainIdRequested {
        chain_id: 999,
        now_iso: NOW_ISO.to_owned(),
    });
    assert!(ops.is_empty());
    assert_eq!(
        sut.view().wizard.error,
        Some(NetWizardErrorKind::AlreadyAdded { chain_id: 999 })
    );
}

// ===========================================================================
// Search — debounce, ranking (invariant ⑧)
// ===========================================================================

#[test]
fn typing_starts_the_debounce_and_only_the_last_wave_searches() {
    let mut sut = started();
    let ops = sut.dispatch(Event::SearchInput {
        query: "e".to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::StartSearchDebounce {
            ms: SEARCH_DEBOUNCE_MS
        }]
    );
    let ops = sut.dispatch(Event::SearchInput {
        query: "et".to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::StartSearchDebounce {
            ms: SEARCH_DEBOUNCE_MS
        }]
    );
    // The first keystroke's timer fires — a superseded wave, no search.
    assert!(sut.resolve(Res::DebounceElapsed).is_empty());
    // The last one searches.
    assert_eq!(
        sut.resolve(Res::DebounceElapsed),
        vec![Op::FetchSearchIndex]
    );
}

#[test]
fn an_empty_query_clears_suggestions_without_searching() {
    let mut sut = started();
    sut.dispatch(Event::SearchInput {
        query: "e".to_owned(),
    });
    let ops = sut.dispatch(Event::SearchInput {
        query: "   ".to_owned(),
    });
    assert!(ops.is_empty());
    let view = sut.view();
    assert_eq!(view.wizard.phase, NetWizardPhase::Idle);
    assert!(view.wizard.suggestions.is_empty());
}

fn index_entry(chain_id: u32, name: &str, short: &str, symbol: &str) -> NetChainIndexEntry {
    NetChainIndexEntry {
        chain_id,
        name: name.to_owned(),
        short_name: short.to_owned(),
        native_currency_symbol: symbol.to_owned(),
        has_logo: true,
        vela: false,
    }
}

/// Invariant ⑧ first half: an exact chainId hit ranks FIRST even when a
/// fuzzy match appears earlier in the index.
#[test]
fn an_exact_chain_id_match_ranks_first() {
    let mut sut = started();
    sut.dispatch(Event::SearchInput {
        query: "137".to_owned(),
    });
    sut.resolve(Res::DebounceElapsed);
    let ops = sut.resolve(Res::SearchIndex {
        chains: vec![
            // Matches "137" by id substring, listed BEFORE the exact hit.
            index_entry(555_137, "Weirdnet", "wrd", "WRD"),
            index_entry(137, "Polygon", "matic", "POL"),
            index_entry(1, "Ethereum", "eth", "ETH"),
        ],
    });
    assert!(ops.is_empty());
    let suggestions = sut.view().wizard.suggestions;
    assert_eq!(
        suggestions.iter().map(|s| s.chain_id).collect::<Vec<_>>(),
        vec![137, 555_137],
        "exact chainId first, fuzzy second, non-match dropped"
    );
}

#[test]
fn search_results_dedupe_and_cap_at_ten() {
    let chains: Vec<NetChainIndexEntry> = (0..15)
        .map(|i| index_entry(1000 + i, &format!("Ether Fork {i}"), "ef", "ETH"))
        .collect();
    let ranked = rank_search(&chains, "ether");
    assert_eq!(ranked.len(), 10);
    // "1005" exact hit also appears among fuzzy matches — deduped.
    let ranked = rank_search(&chains, "1005");
    assert_eq!(ranked.iter().filter(|c| c.chain_id == 1005).count(), 1);
    assert_eq!(ranked[0].chain_id, 1005);
}

// ===========================================================================
// Wizard — resolve, candidates, race
// ===========================================================================

#[test]
fn an_unknown_chain_reads_not_found() {
    let mut sut = started();
    sut.dispatch(Event::ChainSelected {
        chain_id: NEW_CHAIN,
        keep_custom_rpc: false,
    });
    assert!(sut
        .resolve(Res::ChainInfo {
            chain_id: NEW_CHAIN,
            data: None
        })
        .is_empty());
    assert_eq!(
        sut.view().wizard.error,
        Some(NetWizardErrorKind::NotFound {
            chain_id: NEW_CHAIN
        })
    );
}

/// Invariant ⑧ second half: a key-placeholder RPC URL never enters the
/// candidate list; ws:// and http:// are filtered too.
#[test]
fn placeholder_rpc_urls_never_enter_the_candidates() {
    let mut sut = started();
    let data = NetRawChainData {
        rpc: vec![
            "https://keyed.example/${API_KEY}".to_owned(),
            "https://clean.example".to_owned(),
            "http://plain.example".to_owned(),
            "wss://ws.example".to_owned(),
        ],
        ..raw_chain()
    };
    let ops = select_and_resolve(&mut sut, data);
    assert_eq!(
        ops,
        vec![Op::ProbeRpc {
            url: "https://clean.example".to_owned()
        }],
        "only the clean HTTPS URL is probed"
    );
    let info = sut.view().wizard.chain_info.expect("resolved");
    assert_eq!(info.rpc_urls, vec!["https://clean.example".to_owned()]);
}

#[test]
fn the_typed_custom_rpc_is_probed_first_and_survives_recheck() {
    let mut sut = started();
    sut.dispatch(Event::CustomRpcEdited {
        value: "  https://mine.example  ".to_owned(),
    });
    let ops = sut.dispatch(Event::ChainSelected {
        chain_id: NEW_CHAIN,
        keep_custom_rpc: true,
    });
    assert_eq!(
        ops,
        vec![Op::FetchChainInfo {
            chain_id: NEW_CHAIN
        }]
    );
    let ops = sut.resolve(Res::ChainInfo {
        chain_id: NEW_CHAIN,
        data: Some(raw_chain()),
    });
    assert_eq!(
        ops,
        vec![
            Op::ProbeRpc {
                url: "https://mine.example".to_owned()
            },
            Op::ProbeRpc {
                url: RPC_SLOW.to_owned()
            },
            Op::ProbeRpc {
                url: RPC_FAST.to_owned()
            },
        ],
        "custom RPC first, trimmed"
    );
    assert_eq!(sut.view().wizard.custom_rpc, "  https://mine.example  ");
}

#[test]
fn selecting_without_keep_clears_the_custom_rpc() {
    let mut sut = started();
    sut.dispatch(Event::CustomRpcEdited {
        value: "https://mine.example".to_owned(),
    });
    sut.dispatch(Event::ChainSelected {
        chain_id: NEW_CHAIN,
        keep_custom_rpc: false,
    });
    assert_eq!(sut.view().wizard.custom_rpc, "");
}

#[test]
fn a_registry_without_any_rpc_reads_no_rpc_endpoint() {
    let mut sut = started();
    let data = NetRawChainData {
        rpc: vec!["http://plain.example".to_owned()],
        ..raw_chain()
    };
    assert!(select_and_resolve(&mut sut, data).is_empty());
    assert_eq!(
        sut.view().wizard.error,
        Some(NetWizardErrorKind::NoRpcEndpoint)
    );
}

#[test]
fn the_fastest_responsive_rpc_wins_the_race() {
    let sut = checked_compatible();
    let compat = sut.view().wizard.compat.expect("checked");
    assert_eq!(compat.best_rpc_url.as_deref(), Some(RPC_FAST));
    assert_eq!(compat.best_rpc_latency_ms, Some(20.0));
}

#[test]
fn the_contract_checks_run_against_the_race_winner() {
    let mut sut = started();
    select_and_resolve(&mut sut, raw_chain());
    let ops = resolve_race(&mut sut);
    let expected: Vec<Op> = REQUIRED_CONTRACTS
        .iter()
        .map(|(_, address, _)| Op::RpcGetCode {
            url: RPC_FAST.to_owned(),
            address: (*address).to_owned(),
        })
        .chain(std::iter::once(Op::RpcCallP256 {
            url: RPC_FAST.to_owned(),
        }))
        .collect();
    assert_eq!(ops, expected);
}

// ===========================================================================
// Invariant ② — 11 contracts + P256, or no entry (funds would be trapped)
// ===========================================================================

#[test]
fn a_chain_missing_any_required_contract_never_saves() {
    let mut sut = started();
    select_and_resolve(&mut sut, raw_chain());
    resolve_race(&mut sut);
    for (i, (_, address, _)) in REQUIRED_CONTRACTS.iter().enumerate() {
        // "Safe L2" is missing.
        let code = if i == 4 { Some("0x") } else { Some("0x6080") };
        assert!(sut.resolve(code_result(RPC_FAST, address, code)).is_empty());
    }
    sut.resolve(Res::P256Call {
        url: RPC_FAST.to_owned(),
        result: Some(p256_one()),
    });

    let view = sut.view();
    let compat = view.wizard.compat.expect("checked");
    assert!(!compat.compatible);
    assert!(!compat.contracts[4].deployed);
    assert_eq!(compat.contracts[4].name, "Safe L2");
    assert_eq!(compat.p256_available, Some(true));
    assert!(!view.wizard.can_add);
    // Contracts can be deployed: the line says so, and the button opens
    // Chain Setup on THIS network.
    assert_eq!(compat.blocker, Some(NetBlocker::MissingContracts));
    assert_eq!(compat.hint_key.as_deref(), Some(MISSING_CONTRACTS_HINT));
    assert_eq!(
        compat.setup_url.as_deref(),
        Some("https://getvela.app/chain-setup?chain=7777")
    );

    // The confirm is inert — nothing is written, nothing enters the ledger.
    assert!(sut
        .dispatch(Event::AddConfirmed {
            now_iso: NOW_ISO.to_owned()
        })
        .is_empty());
    assert_eq!(sut.view().networks.len(), BUILTIN_CHAINS.len());
}

#[test]
fn a_chain_without_the_p256_precompile_never_saves() {
    let mut sut = started();
    select_and_resolve(&mut sut, raw_chain());
    resolve_race(&mut sut);
    for (_, address, _) in REQUIRED_CONTRACTS {
        assert!(sut.resolve(code_ok(RPC_FAST, address)).is_empty());
    }
    // Strategy 1 fails → the machine falls back to getCode at the precompile.
    let ops = sut.resolve(Res::P256Call {
        url: RPC_FAST.to_owned(),
        result: Some("0x".to_owned()),
    });
    assert_eq!(
        ops,
        vec![Op::RpcGetCode {
            url: RPC_FAST.to_owned(),
            address: P256_PRECOMPILE.to_owned(),
        }]
    );
    // Strategy 2 fails too — no RIP-7212, passkeys could never sign.
    assert!(sut
        .resolve(code_result(RPC_FAST, P256_PRECOMPILE, Some("0x")))
        .is_empty());

    let compat = sut.view().wizard.compat.expect("checked");
    assert!(!compat.compatible);
    assert_eq!(compat.p256_available, Some(false));
    // A precompile is the chain's own: nothing to deploy, so no Chain Setup
    // button — the line says Vela cannot work here and money would be stuck.
    assert_eq!(compat.blocker, Some(NetBlocker::NoP256));
    assert_eq!(compat.hint_key.as_deref(), Some(NO_P256_HINT));
    assert_eq!(compat.setup_url, None);
    assert!(sut
        .dispatch(Event::AddConfirmed {
            now_iso: NOW_ISO.to_owned()
        })
        .is_empty());
}

/// No verifier AND missing contracts: the verifier is the answer. Deploying
/// every contract would still leave a chain no passkey can sign on, so the
/// tool is not offered (the Chain Setup page's own `blocked` verdict).
#[test]
fn no_p256_wins_over_missing_contracts() {
    assert_eq!(net_blocker(false, false), Some(NetBlocker::NoP256));
    assert_eq!(net_blocker(false, true), Some(NetBlocker::NoP256));
    assert_eq!(net_blocker(true, false), Some(NetBlocker::MissingContracts));
    assert_eq!(net_blocker(true, true), None);

    let mut sut = started();
    select_and_resolve(&mut sut, raw_chain());
    resolve_race(&mut sut);
    for (_, address, _) in REQUIRED_CONTRACTS {
        sut.resolve(code_result(RPC_FAST, address, Some("0x")));
    }
    sut.resolve(Res::P256Call {
        url: RPC_FAST.to_owned(),
        result: Some("0x".to_owned()),
    });
    sut.resolve(code_result(RPC_FAST, P256_PRECOMPILE, Some("0x")));
    let compat = sut.view().wizard.compat.expect("checked");
    assert_eq!(compat.blocker, Some(NetBlocker::NoP256));
    assert_eq!(compat.setup_url, None);
}

#[test]
fn the_setup_link_carries_the_chain_and_the_hints_are_in_the_corpus() {
    assert_eq!(
        chain_setup_url(42_220),
        "https://getvela.app/chain-setup?chain=42220"
    );
    let i18n = vela_core::i18n::I18n::embedded().expect("embedded corpus");
    let opts = vela_core::i18n::Options::default();
    for key in [NO_P256_HINT, MISSING_CONTRACTS_HINT] {
        assert!(i18n.exists(key, &opts), "{key}");
    }
}

#[test]
fn p256_code_at_the_precompile_rescues_strategy_two() {
    let mut sut = started();
    select_and_resolve(&mut sut, raw_chain());
    resolve_race(&mut sut);
    for (_, address, _) in REQUIRED_CONTRACTS {
        sut.resolve(code_ok(RPC_FAST, address));
    }
    // A too-short call answer (zkSync-style) → fall back to getCode; code
    // present at 0x100 == the precompile exists.
    let ops = sut.resolve(Res::P256Call {
        url: RPC_FAST.to_owned(),
        result: Some("0x01".to_owned()),
    });
    assert_eq!(ops.len(), 1);
    sut.resolve(code_result(RPC_FAST, P256_PRECOMPILE, Some("0x1234")));

    let compat = sut.view().wizard.compat.expect("checked");
    assert!(compat.compatible);
    assert_eq!(compat.p256_available, Some(true));
}

/// Spec 081 FR-002. "Unset means the default" is one rule, in one place —
/// because three shells had each written their own, and one of them cached the
/// answer for the life of the process, so changing the public-key index in
/// Settings changed nothing.
#[test]
fn a_blank_endpoint_resolves_to_its_default_whatever_the_blank_looks_like() {
    for field in [
        NetEndpointField::EthereumData,
        NetEndpointField::PasskeyIndex,
        NetEndpointField::BundlerService,
        NetEndpointField::FiatRates,
        NetEndpointField::GenesisService,
    ] {
        let with = |value: &str| {
            let mut e = NetServiceEndpoints::default();
            let slot = match field {
                NetEndpointField::EthereumData => &mut e.ethereum_data_url,
                NetEndpointField::PasskeyIndex => &mut e.passkey_index_url,
                NetEndpointField::BundlerService => &mut e.bundler_service_url,
                NetEndpointField::FiatRates => &mut e.fiat_rates_url,
                NetEndpointField::GenesisService => &mut e.genesis_service_url,
            };
            *slot = value.to_owned();
            e
        };

        assert_eq!(
            NetServiceEndpoints::default().effective(field),
            default_endpoint(field)
        );
        for blank in ["", "   ", "\t"] {
            assert_eq!(
                with(blank).effective(field),
                default_endpoint(field),
                "a field holding {blank:?} is a field nobody set"
            );
        }
        assert_eq!(
            with("https://mine.example").effective(field),
            "https://mine.example"
        );
    }
}

/// Spec 081 FR-009. A chain can have everything a one-key wallet needs and
/// still be unable to hold a multi-key one: keys two to seven are signer
/// contracts Safe's factory creates inside the setup, so without the factory
/// and its singleton that wallet cannot be deployed at all. The old check
/// called such a chain "compatible" with no qualification.
#[test]
fn a_chain_without_safes_signer_factory_is_single_key_only() {
    let mut sut = started();
    select_and_resolve(&mut sut, raw_chain());
    resolve_race(&mut sut);
    for (_, address, multi_key_only) in REQUIRED_CONTRACTS {
        if multi_key_only {
            assert!(sut.resolve(code_result(RPC_FAST, address, None)).is_empty());
        } else {
            assert!(sut.resolve(code_ok(RPC_FAST, address)).is_empty());
        }
    }
    sut.resolve(Res::P256Call {
        url: RPC_FAST.to_owned(),
        result: Some(p256_one()),
    });

    let compat = sut.view().wizard.compat.expect("checked");
    assert!(compat.compatible, "a one-key wallet works here");
    assert!(
        !compat.multi_key_ready,
        "a wallet with two to seven keys does not"
    );
    assert_eq!(compat.blocker, None, "not refused");
    assert_eq!(compat.setup_url, None);
    let missing: Vec<_> = compat
        .contracts
        .iter()
        .filter(|c| !c.deployed)
        .map(|c| c.name.clone())
        .collect();
    assert_eq!(
        missing,
        vec![
            "Safe Passkey Signer Factory".to_owned(),
            "Safe Passkey Signer Singleton".to_owned()
        ],
        "and the screen can name exactly what is missing"
    );
}

/// The handler Vela never uses is no longer required: a Vela account's
/// fallback handler IS the 4337 module (`safe.rs` setup calldata), so
/// demanding Safe's CompatibilityFallbackHandler turned away chains the
/// wallet works on perfectly well.
#[test]
fn the_unused_fallback_handler_is_not_required() {
    const COMPATIBILITY_FALLBACK_HANDLER: &str = "0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99";
    assert!(
        !REQUIRED_CONTRACTS
            .iter()
            .any(|(_, address, _)| address.eq_ignore_ascii_case(COMPATIBILITY_FALLBACK_HANDLER)),
        "the CompatibilityFallbackHandler is not part of a Vela wallet"
    );
    assert!(
        REQUIRED_CONTRACTS
            .iter()
            .any(|(_, a, _)| a.eq_ignore_ascii_case(vela_core::safe::WEBAUTHN_SIGNER_FACTORY)),
        "the signer factory is"
    );
}

#[test]
fn a_fully_provisioned_chain_saves_with_the_fastest_rpc() {
    let mut sut = checked_compatible();
    assert!(sut.view().wizard.can_add);
    let ops = sut.dispatch(Event::AddConfirmed {
        now_iso: NOW_ISO.to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::WriteCustomNetworks {
            networks: vec![NetCustomNetwork {
                id: "custom-7777".to_owned(),
                display_name: "Testland".to_owned(),
                chain_id: NEW_CHAIN,
                icon_label: "TST".to_owned(),
                icon_color: "#888888".to_owned(),
                icon_bg: "#F0F0F0".to_owned(),
                logo_url: format!("{DEFAULT_ETHEREUM_DATA_URL}/chainlogos/eip155-{NEW_CHAIN}.png"),
                is_l2: false,
                rpc_url: RPC_FAST.to_owned(),
                explorer_url: "https://scan.example".to_owned(),
                bundler_url: format!("{DEFAULT_BUNDLER_SERVICE_URL}/{NEW_CHAIN}"),
                native_symbol: "TST".to_owned(),
                added_at_iso: NOW_ISO.to_owned(),
                // Nobody chose a space: the registry's word (set-5), and
                // "Testland" carries no test network's mark — a mainnet.
                is_testnet: Some(false),
            }]
        }]
    );
    let view = sut.view();
    assert_eq!(view.last_added_chain_id, Some(NEW_CHAIN));
    assert_eq!(view.wizard.phase, NetWizardPhase::Idle, "reset + close");
    assert_eq!(view.networks.len(), BUILTIN_CHAINS.len() + 1);
    assert!(view.networks[BUILTIN_CHAINS.len()].is_custom);
}

/// Spec 105: the wizard says which space a network goes to — what the
/// registry says the chain is (set-5) unless the person says otherwise — and
/// keeps it with the record; the person can move it later, and each
/// network's row says its space.
#[test]
fn an_added_network_keeps_the_space_the_person_chose() {
    let mut sut = checked_compatible();
    assert!(
        !sut.view().wizard.testnet,
        "the registry describes a mainnet: the mainnet space is preselected"
    );
    sut.dispatch(Event::WizardTestnetChosen { testnet: true });
    assert!(sut.view().wizard.testnet, "the person's word wins");
    sut.dispatch(Event::WizardTestnetChosen { testnet: false });
    assert!(!sut.view().wizard.testnet);
    let ops = sut.dispatch(Event::AddConfirmed {
        now_iso: NOW_ISO.to_owned(),
    });
    let Some(Op::WriteCustomNetworks { networks }) = ops.first() else {
        panic!("saved: {ops:?}");
    };
    assert_eq!(networks[0].is_testnet, Some(false));
    let row = |sut: &Sut| {
        sut.view()
            .networks
            .into_iter()
            .find(|row| row.chain_id == NEW_CHAIN)
            .expect("listed")
    };
    assert!(!row(&sut).is_testnet);

    let ops = sut.dispatch(Event::CustomSpaceChanged {
        id: "custom-7777".to_owned(),
        testnet: true,
    });
    let Some(Op::WriteCustomNetworks { networks }) = ops.first() else {
        panic!("moved: {ops:?}");
    };
    assert_eq!(networks[0].is_testnet, Some(true));
    assert!(row(&sut).is_testnet);
    // The built-ins carry the table's word.
    let view = sut.view();
    let space = |chain: u32| {
        view.networks
            .iter()
            .find(|r| r.chain_id == chain)
            .map(|r| r.is_testnet)
    };
    assert_eq!(space(1), Some(false));
    assert_eq!(space(11_155_111), Some(true));
}

/// A network the person added before it shipped built in (Arc Testnet) is
/// listed once: the built-in.
#[test]
fn a_custom_network_now_built_in_is_listed_once() {
    let sut = started_with(vec![custom_network(5_042_002)], vec![]);
    let view = sut.view();
    let arc: Vec<_> = view
        .networks
        .iter()
        .filter(|r| r.chain_id == 5_042_002)
        .collect();
    assert_eq!(arc.len(), 1);
    assert!(!arc[0].is_custom);
    assert!(arc[0].is_testnet);
}

/// The wizard's bundler/logo URLs follow the CONFIGURED service endpoints
/// (`getBundlerServiceURL()` / `getEthereumDataURL()`), not the constants.
#[test]
fn custom_service_endpoints_feed_the_new_network_record() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Started);
    sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![],
        endpoints: NetStoredEndpoints {
            ethereum_data_url: Some("https://my-data.example".to_owned()),
            bundler_service_url: Some("https://my-relay.example".to_owned()),
            ..Default::default()
        },
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: None,
    });
    select_and_resolve(&mut sut, raw_chain());
    resolve_race(&mut sut);
    resolve_contracts_ok(&mut sut, RPC_FAST);
    let ops = sut.dispatch(Event::AddConfirmed {
        now_iso: NOW_ISO.to_owned(),
    });
    let Some(Op::WriteCustomNetworks { networks }) = ops.first() else {
        panic!("expected the custom-network write, got {ops:?}");
    };
    assert_eq!(
        networks[0].bundler_url,
        format!("https://my-relay.example/{NEW_CHAIN}")
    );
    assert_eq!(
        networks[0].logo_url,
        format!("https://my-data.example/chainlogos/eip155-{NEW_CHAIN}.png")
    );
}

/// A network's logo is the marks' one rule (`remote_mark::chain_logo_url`):
/// the base trimmed of spaces and trailing slashes, a blank one the built-in
/// endpoint — never `…//chainlogos/…` for a base saved with its slash.
#[test]
fn a_new_networks_logo_is_the_marks_one_rule() {
    for base in [
        "https://my-data.example/",
        "  https://my-data.example//  ",
        "https://my-data.example",
    ] {
        assert_eq!(
            parse_chain_data(&raw_chain(), NEW_CHAIN, base).logo_url,
            format!("https://my-data.example/chainlogos/eip155-{NEW_CHAIN}.png"),
            "{base:?}"
        );
        assert_eq!(
            Some(parse_chain_data(&raw_chain(), NEW_CHAIN, base).logo_url),
            chain_logo_url(base, NEW_CHAIN),
            "{base:?}"
        );
    }
    assert_eq!(
        parse_chain_data(&raw_chain(), NEW_CHAIN, "").logo_url,
        format!("{DEFAULT_ETHEREUM_DATA_URL}/chainlogos/eip155-{NEW_CHAIN}.png"),
        "blank is the built-in endpoint"
    );

    // Through the wizard, with the endpoint saved with its trailing slash.
    let mut sut = Sut::new();
    sut.dispatch(Event::Started);
    sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![],
        endpoints: NetStoredEndpoints {
            ethereum_data_url: Some("https://my-data.example/".to_owned()),
            ..Default::default()
        },
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: None,
    });
    select_and_resolve(&mut sut, raw_chain());
    resolve_race(&mut sut);
    resolve_contracts_ok(&mut sut, RPC_FAST);
    let ops = sut.dispatch(Event::AddConfirmed {
        now_iso: NOW_ISO.to_owned(),
    });
    let Some(Op::WriteCustomNetworks { networks }) = ops.first() else {
        panic!("expected the custom-network write, got {ops:?}");
    };
    assert_eq!(
        networks[0].logo_url,
        format!("https://my-data.example/chainlogos/eip155-{NEW_CHAIN}.png")
    );
}

// ===========================================================================
// Invariant ③ — RPC failure is "unable to verify", NEVER "not compatible"
// ===========================================================================

#[test]
fn all_rpcs_failing_reads_unable_to_verify_not_incompatible() {
    let mut sut = started();
    select_and_resolve(&mut sut, raw_chain());
    assert!(sut.resolve(probe(RPC_SLOW, None, 0.0)).is_empty());
    assert!(sut.resolve(probe(RPC_FAST, None, 0.0)).is_empty());

    let view = sut.view();
    // Checked — the Retry affordance — not the Error terminal.
    assert_eq!(view.wizard.phase, NetWizardPhase::Checked);
    let compat = view.wizard.compat.expect("inconclusive result present");
    assert_eq!(compat.rpc_failure, Some(NetRpcFailureKind::AllProbesFailed));
    assert!(!compat.compatible);
    assert_eq!(compat.p256_available, None, "never probed — no verdict");
    assert_eq!(compat.blocker, None, "inconclusive is not a refusal");
    assert_eq!((compat.hint_key, compat.setup_url), (None, None));
    assert!(compat.contracts.iter().all(|c| !c.deployed));
    assert!(!view.wizard.can_add);
    assert!(sut
        .dispatch(Event::AddConfirmed {
            now_iso: NOW_ISO.to_owned()
        })
        .is_empty());
}

#[test]
fn a_non_https_only_candidate_set_is_also_inconclusive() {
    let mut sut = started();
    sut.dispatch(Event::CustomRpcEdited {
        value: "http://my.example".to_owned(),
    });
    sut.dispatch(Event::ChainSelected {
        chain_id: NEW_CHAIN,
        keep_custom_rpc: true,
    });
    let data = NetRawChainData {
        rpc: vec![],
        ..raw_chain()
    };
    assert!(sut
        .resolve(Res::ChainInfo {
            chain_id: NEW_CHAIN,
            data: Some(data)
        })
        .is_empty());
    let compat = sut.view().wizard.compat.expect("checked");
    assert_eq!(
        compat.rpc_failure,
        Some(NetRpcFailureKind::NoHttpsCandidates)
    );
    assert_eq!(sut.view().wizard.phase, NetWizardPhase::Checked);
}

// ===========================================================================
// The scan path (add-network.ts) — auto-save, verbatim flattening
// ===========================================================================

#[test]
fn the_scan_path_saves_a_compatible_chain_without_confirmation() {
    let mut sut = started();
    let ops = sut.dispatch(Event::AddByChainIdRequested {
        chain_id: NEW_CHAIN,
        now_iso: NOW_ISO.to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::FetchChainInfo {
            chain_id: NEW_CHAIN
        }]
    );
    sut.resolve(Res::ChainInfo {
        chain_id: NEW_CHAIN,
        data: Some(raw_chain()),
    });
    resolve_race(&mut sut);
    let ops = resolve_contracts_ok(&mut sut, RPC_FAST);
    let Some(Op::WriteCustomNetworks { networks }) = ops.first() else {
        panic!("auto mode saves on compatible, got {ops:?}");
    };
    assert_eq!(networks[0].id, "custom-7777");
    assert_eq!(networks[0].added_at_iso, NOW_ISO);
    let view = sut.view();
    assert_eq!(view.last_added_chain_id, Some(NEW_CHAIN));
    assert_eq!(view.wizard.phase, NetWizardPhase::Idle);
}

/// Spec 038 #E1: the scan path keeps invariant ③ too — an inconclusive RPC
/// failure is "could not check", never "not compatible". (`add-network.ts:47`
/// flattened them; a desktop whose proxy was refusing then called Celo
/// incompatible, and Celo has every contract and RIP-7212.)
#[test]
fn the_scan_path_keeps_rpc_failure_apart_from_not_compatible() {
    let mut sut = started();
    sut.dispatch(Event::AddByChainIdRequested {
        chain_id: NEW_CHAIN,
        now_iso: NOW_ISO.to_owned(),
    });
    sut.resolve(Res::ChainInfo {
        chain_id: NEW_CHAIN,
        data: Some(raw_chain()),
    });
    sut.resolve(probe(RPC_SLOW, None, 0.0));
    assert!(sut.resolve(probe(RPC_FAST, None, 0.0)).is_empty());
    assert_eq!(
        sut.view().wizard.error,
        Some(NetWizardErrorKind::CheckFailed {
            chain_id: NEW_CHAIN
        })
    );
    assert_eq!(
        sut.view().networks.len(),
        BUILTIN_CHAINS.len(),
        "nothing saved"
    );
}

// ===========================================================================
// Staleness — superseded waves never repaint fresh state
// ===========================================================================

#[test]
fn a_late_result_from_a_superseded_wizard_run_is_dropped() {
    let mut sut = started();
    sut.dispatch(Event::ChainSelected {
        chain_id: NEW_CHAIN,
        keep_custom_rpc: false,
    });
    sut.dispatch(Event::ChainSelected {
        chain_id: 8888,
        keep_custom_rpc: false,
    });
    // The first selection's chain info arrives late — dropped.
    assert!(sut
        .resolve(Res::ChainInfo {
            chain_id: NEW_CHAIN,
            data: Some(raw_chain())
        })
        .is_empty());
    assert_eq!(sut.view().wizard.phase, NetWizardPhase::Resolving);
    // The current selection proceeds normally.
    assert!(sut
        .resolve(Res::ChainInfo {
            chain_id: 8888,
            data: None
        })
        .is_empty());
    assert_eq!(
        sut.view().wizard.error,
        Some(NetWizardErrorKind::NotFound { chain_id: 8888 })
    );
}

// ===========================================================================
// Invariant ④ (the save gate) + ⑤ — override saves
// ===========================================================================

/// Answer the two health probes one expanded card issues, in the order
/// `override_probe_wave` pushes them (RPC first, explorer second).
fn answer_card_probes(sut: &mut Sut, rpc_url: &str, reported: Option<u32>, explorer_url: &str) {
    sut.resolve(Res::Probed {
        url: rpc_url.to_owned(),
        reported_chain_id: reported,
        latency_ms: 12.0,
    });
    sut.resolve(Res::Reachable {
        url: explorer_url.to_owned(),
        ok: true,
        latency_ms: 20.0,
    });
}

const WRONG_RPC: &str = "https://wrong-chain.example";
const ETH_SCAN: &str = "https://etherscan.io";

/// Invariant ④: an RPC that PROVES it serves another chain is refused. Nothing
/// is written, no pool is flushed, and the card says which chain it actually
/// got — the wrong-chain endpoint never reaches the pool's top tier.
#[test]
fn an_override_save_is_refused_when_the_rpc_reports_another_chain() {
    let mut sut = started();
    sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    answer_card_probes(
        &mut sut,
        "https://ethereum-rpc.publicnode.com",
        Some(1),
        ETH_SCAN,
    );
    sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        value: WRONG_RPC.to_owned(),
    });
    // The edit's own probe answers "this is chain 137".
    answer_card_probes(&mut sut, WRONG_RPC, Some(137), ETH_SCAN);

    let ops = sut.dispatch(Event::OverrideBlurred { chain_id: 1 });
    assert!(ops.is_empty(), "refused: nothing written, nothing flushed");
    let row = &sut.view().networks[0];
    assert_eq!(
        row.rpc_chain_mismatch
            .map(|m| (m.expected_chain_id, m.reported_chain_id)),
        Some((1, 137)),
        "the card carries what to say: expected 1, got 137"
    );
    assert!(!row.rpc_save_deferred);
}

/// The gate refuses on PROOF only. An endpoint that timed out, refused, or
/// answered garbage is "unable to verify" — and the save proceeds, exactly as
/// the compatibility checker never condemns a chain it could not reach
/// (invariant ③'s discipline).
#[test]
fn an_unverifiable_rpc_still_saves() {
    let mut sut = started();
    sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    answer_card_probes(
        &mut sut,
        "https://ethereum-rpc.publicnode.com",
        Some(1),
        ETH_SCAN,
    );
    sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        value: "https://offline.example".to_owned(),
    });
    answer_card_probes(&mut sut, "https://offline.example", None, ETH_SCAN);

    let ops = sut.dispatch(Event::OverrideBlurred { chain_id: 1 });
    assert_eq!(
        ops,
        vec![
            Op::WriteNetworkConfigs {
                configs: vec![NetNetworkConfig {
                    chain_id: 1,
                    rpc_url: "https://offline.example".to_owned(),
                    explorer_url: ETH_SCAN.to_owned(),
                    bundler_url: String::new(),
                    only_mine: false,
                }]
            },
            Op::InvalidatePools { chain_id: Some(1) },
            Op::ClearBundlerCache { chain_id: 1 },
        ],
        "write + pool flush + bundler-cache flush"
    );
    assert!(sut.view().networks[0].rpc_chain_mismatch.is_none());
}

/// A blur that lands before the probe does must WAIT, not write. Writing first
/// and undoing later has already served balances from the wrong chain.
#[test]
fn a_blur_before_the_verdict_is_held_until_the_probe_answers() {
    let mut sut = started();
    sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    answer_card_probes(
        &mut sut,
        "https://ethereum-rpc.publicnode.com",
        Some(1),
        ETH_SCAN,
    );
    sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        value: WRONG_RPC.to_owned(),
    });

    let ops = sut.dispatch(Event::OverrideBlurred { chain_id: 1 });
    assert!(
        ops.is_empty(),
        "the verdict is not in yet — nothing is written"
    );
    assert!(sut.view().networks[0].rpc_save_deferred);

    // The probe answers "chain 137" — the owed save resolves into a refusal.
    let ops = sut.resolve(Res::Probed {
        url: WRONG_RPC.to_owned(),
        reported_chain_id: Some(137),
        latency_ms: 12.0,
    });
    assert!(ops.is_empty(), "still refused, just later");
    let row = &sut.view().networks[0];
    assert!(!row.rpc_save_deferred);
    assert_eq!(
        row.rpc_chain_mismatch.map(|m| m.reported_chain_id),
        Some(137)
    );
}

/// The same hold, resolving the other way: the probe agrees, so the owed save
/// runs with the full write + flush sequence.
#[test]
fn a_held_save_commits_once_the_probe_agrees() {
    let mut sut = started();
    sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    answer_card_probes(
        &mut sut,
        "https://ethereum-rpc.publicnode.com",
        Some(1),
        ETH_SCAN,
    );
    sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        value: "https://my-node.example".to_owned(),
    });
    assert!(sut
        .dispatch(Event::OverrideBlurred { chain_id: 1 })
        .is_empty());

    let ops = sut.resolve(Res::Probed {
        url: "https://my-node.example".to_owned(),
        reported_chain_id: Some(1),
        latency_ms: 9.0,
    });
    assert_eq!(
        ops,
        vec![
            Op::WriteNetworkConfigs {
                configs: vec![NetNetworkConfig {
                    chain_id: 1,
                    rpc_url: "https://my-node.example".to_owned(),
                    explorer_url: ETH_SCAN.to_owned(),
                    bundler_url: String::new(),
                    only_mine: false,
                }]
            },
            Op::InvalidatePools { chain_id: Some(1) },
            Op::ClearBundlerCache { chain_id: 1 },
        ]
    );
    assert!(!sut.view().networks[0].rpc_save_deferred);
}

/// An emptied RPC field issues no probe, so the gate has nothing to disprove:
/// a held save must not wait forever for an answer that will never come.
#[test]
fn a_held_save_over_an_emptied_rpc_field_still_lands() {
    let mut sut = started();
    sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    assert!(sut
        .dispatch(Event::OverrideBlurred { chain_id: 1 })
        .is_empty());
    assert!(sut.view().networks[0].rpc_save_deferred);

    let ops = sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        value: String::new(),
    });
    assert!(
        ops.contains(&Op::WriteNetworkConfigs {
            configs: vec![NetNetworkConfig {
                chain_id: 1,
                rpc_url: String::new(),
                explorer_url: ETH_SCAN.to_owned(),
                bundler_url: String::new(),
                only_mine: false,
            }]
        }),
        "an unverifiable (empty) URL saves; got {ops:?}"
    );
    assert!(!sut.view().networks[0].rpc_save_deferred);
}

/// A refusal is about a URL, not about a card: the next keystroke clears it,
/// so the message can never outlive the value it describes.
#[test]
fn editing_the_field_clears_a_previous_refusal() {
    let mut sut = started();
    sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    answer_card_probes(
        &mut sut,
        "https://ethereum-rpc.publicnode.com",
        Some(1),
        ETH_SCAN,
    );
    sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        value: WRONG_RPC.to_owned(),
    });
    answer_card_probes(&mut sut, WRONG_RPC, Some(137), ETH_SCAN);
    sut.dispatch(Event::OverrideBlurred { chain_id: 1 });
    assert!(sut.view().networks[0].rpc_chain_mismatch.is_some());

    sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        value: "https://ethereum-rpc.publicnode.com".to_owned(),
    });
    assert!(sut.view().networks[0].rpc_chain_mismatch.is_none());
}

/// Invariant ⑤ first half: the bundler field is not editable per network —
/// a save must never clobber an existing (custom) bundler URL.
#[test]
fn an_override_save_never_clears_the_saved_bundler_url() {
    let mut sut = started_with(
        vec![],
        vec![NetNetworkConfig {
            chain_id: 1,
            rpc_url: "https://saved-rpc.example".to_owned(),
            explorer_url: "https://saved-scan.example".to_owned(),
            bundler_url: "https://my-bundler.example/1".to_owned(),
            only_mine: false,
        }],
    );
    sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    // The card seeds from the SAVED config, not the builtin default.
    let row = &sut.view().networks[0];
    assert_eq!(row.rpc_url, "https://saved-rpc.example");
    answer_card_probes(
        &mut sut,
        "https://saved-rpc.example",
        Some(1),
        "https://saved-scan.example",
    );
    sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        value: "https://new-rpc.example".to_owned(),
    });
    // The gate needs a verdict before it can let the write through.
    answer_card_probes(
        &mut sut,
        "https://new-rpc.example",
        Some(1),
        "https://saved-scan.example",
    );
    let ops = sut.dispatch(Event::OverrideBlurred { chain_id: 1 });
    let Some(Op::WriteNetworkConfigs { configs }) = ops.first() else {
        panic!("expected the config write, got {ops:?}");
    };
    assert_eq!(configs.len(), 1, "upsert by chainId, not append");
    assert_eq!(configs[0].rpc_url, "https://new-rpc.example");
    assert_eq!(
        configs[0].bundler_url, "https://my-bundler.example/1",
        "bundler preserved"
    );
}

/// Invariant ⑤, the half nobody wrote down: a BUILT-IN network's bundler is
/// the configured relay, not a constant.
///
/// The shells seed their bundler pool from these rows (Android's
/// `NetworkEndpointSource` has no other source at all), so a row pinned to
/// `DEFAULT_BUNDLER_SERVICE_URL` made Settings › Service nodes › Vela Relay a
/// field that saved, probed, showed a latency badge — and changed where not
/// one user operation went, on any of the 24 chains Vela ships.
#[test]
fn a_builtin_rows_bundler_follows_the_configured_relay() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Started);
    sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![],
        endpoints: NetStoredEndpoints {
            bundler_service_url: Some("https://my-relay.example".to_owned()),
            ..Default::default()
        },
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: None,
    });
    let row = &sut.view().networks[0];
    assert_eq!(row.chain_id, 1);
    assert_eq!(row.bundler_url, "https://my-relay.example/1");

    // And it MOVES: editing the field re-points every built-in row, with no
    // reload in between.
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::BundlerService,
        value: "https://other-relay.example".to_owned(),
    });
    sut.dispatch(Event::EndpointBlurred {
        field: NetEndpointField::BundlerService,
    });
    assert_eq!(
        sut.view().networks[0].bundler_url,
        "https://other-relay.example/1"
    );
}

/// The same rule against a record written BEFORE it existed: an RPC override
/// saved a copy of the shipped relay into `bundler_url`, and reading that copy
/// back as a per-network override would pin the chain to it forever.
#[test]
fn a_legacy_overrides_snapshot_of_the_shipped_relay_is_not_an_override() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Started);
    sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![NetNetworkConfig {
            chain_id: 1,
            rpc_url: "https://saved-rpc.example".to_owned(),
            explorer_url: ETH_SCAN.to_owned(),
            bundler_url: format!("{DEFAULT_BUNDLER_SERVICE_URL}/1"),
            only_mine: false,
        }],
        endpoints: NetStoredEndpoints {
            bundler_service_url: Some("https://my-relay.example".to_owned()),
            ..Default::default()
        },
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: None,
    });
    assert_eq!(
        sut.view().networks[0].bundler_url,
        "https://my-relay.example/1"
    );
}

/// A per-network bundler somebody really set still outranks the relay.
#[test]
fn a_real_per_network_bundler_still_wins_over_the_configured_relay() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Started);
    sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![NetNetworkConfig {
            chain_id: 1,
            rpc_url: "https://saved-rpc.example".to_owned(),
            explorer_url: ETH_SCAN.to_owned(),
            bundler_url: "https://my-bundler.example/1".to_owned(),
            only_mine: false,
        }],
        endpoints: NetStoredEndpoints {
            bundler_service_url: Some("https://my-relay.example".to_owned()),
            ..Default::default()
        },
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: None,
    });
    assert_eq!(
        sut.view().networks[0].bundler_url,
        "https://my-bundler.example/1"
    );
}

#[test]
fn a_custom_networks_card_preserves_its_own_bundler() {
    let mut sut = started_with(vec![custom_network(999)], vec![]);
    sut.dispatch(Event::OverrideExpanded { chain_id: 999 });
    answer_card_probes(
        &mut sut,
        "https://custom-rpc.example",
        Some(999),
        "https://custom-scan.example/",
    );
    let ops = sut.dispatch(Event::OverrideBlurred { chain_id: 999 });
    let Some(Op::WriteNetworkConfigs { configs }) = ops.first() else {
        panic!("expected the config write, got {ops:?}");
    };
    assert_eq!(configs[0].bundler_url, "https://custom-bundler.example/999");
}

/// Invariant ⑤ second half: deleting a custom network flushes EVERY pool so
/// nothing keeps querying the removed chain.
#[test]
fn deleting_a_custom_network_drops_it_and_flushes_every_pool() {
    let mut sut = started_with(vec![custom_network(999)], vec![]);
    let ops = sut.dispatch(Event::DeleteConfirmed {
        id: "custom-999".to_owned(),
    });
    assert_eq!(
        ops,
        vec![
            Op::WriteCustomNetworks { networks: vec![] },
            Op::InvalidatePools { chain_id: None },
        ]
    );
    assert_eq!(sut.view().networks.len(), BUILTIN_CHAINS.len());
}

// ===========================================================================
// Override card health — the unified probe, wave staleness
// ===========================================================================

#[test]
fn expanding_a_card_probes_rpc_and_explorer() {
    let mut sut = started();
    let ops = sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    assert_eq!(
        ops,
        vec![
            Op::ProbeRpc {
                url: "https://ethereum-rpc.publicnode.com".to_owned()
            },
            Op::ProbeReachable {
                url: "https://etherscan.io".to_owned()
            },
        ]
    );
    let row = &sut.view().networks[0];
    assert_eq!(row.rpc_health, Some(NetProbeHealth::Checking));

    sut.resolve(probe("https://ethereum-rpc.publicnode.com", Some(1), 42.0));
    sut.resolve(Res::Reachable {
        url: "https://etherscan.io".to_owned(),
        ok: true,
        latency_ms: 90.0,
    });
    let row = &sut.view().networks[0];
    assert_eq!(
        row.rpc_health,
        Some(NetProbeHealth::Ok { latency_ms: 42.0 })
    );
    assert_eq!(
        row.explorer_health,
        Some(NetProbeHealth::Ok { latency_ms: 90.0 })
    );
}

#[test]
fn editing_a_field_reprobes_both_and_orphans_the_old_wave() {
    let mut sut = started();
    sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    let ops = sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        value: "https://new.example".to_owned(),
    });
    assert_eq!(ops.len(), 2, "both fields re-probed on either edit");
    // The FIRST wave's answers arrive late — dropped, badges stay checking.
    sut.resolve(probe("https://ethereum-rpc.publicnode.com", Some(1), 42.0));
    sut.resolve(Res::Reachable {
        url: "https://etherscan.io".to_owned(),
        ok: true,
        latency_ms: 90.0,
    });
    let row = &sut.view().networks[0];
    assert_eq!(row.rpc_health, Some(NetProbeHealth::Checking));
    // The current wave lands.
    sut.resolve(probe("https://new.example", Some(1), 7.0));
    let row = &sut.view().networks[0];
    assert_eq!(row.rpc_health, Some(NetProbeHealth::Ok { latency_ms: 7.0 }));
}

// ===========================================================================
// Invariant ⑥ — HTTPS + identity + trim (service endpoints)
// ===========================================================================

/// Spec 081 follow-up, found on an iPhone: the page showed `https://index.invalid`
/// with a green "645 ms" beside it. iOS opens Service Endpoints in the same
/// breath as it opens Settings, so `EndpointsOpened` arrived before the store
/// answered; the wave probed the DEFAULTS, the stored URLs then landed and
/// replaced the text, and the default's verdict stayed sitting next to them.
/// The probes are owed, not dropped — the page must end up saying something
/// true about the URLs it is actually showing.
#[test]
fn a_page_opened_before_the_store_answers_probes_the_stored_urls() {
    let mut sut = Sut::new();
    assert_eq!(sut.dispatch(Event::Started), vec![Op::ReadStore]);

    let ops = sut.dispatch(Event::EndpointsOpened);
    assert!(
        ops.is_empty(),
        "nothing is known about this person's endpoints yet, so nothing is asked: {ops:?}"
    );

    let ops = sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![],
        endpoints: NetStoredEndpoints {
            passkey_index_url: Some("https://index.example".to_owned()),
            ..NetStoredEndpoints::default()
        },
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: None,
    });
    assert!(
        ops.contains(&Op::FetchServiceHealth {
            field: NetEndpointField::PasskeyIndex,
            base_url: "https://index.example".to_owned(),
        }),
        "the owed probe asks about the configured index, not the default: {ops:?}"
    );
    assert!(
        !ops.contains(&Op::FetchServiceHealth {
            field: NetEndpointField::PasskeyIndex,
            base_url: DEFAULT_PASSKEY_INDEX_URL.to_owned(),
        }),
        "and never about the default"
    );
    assert_eq!(ops.len(), 4, "all four fields, once each: {ops:?}");

    // Owed once. A second load (a re-read after a write) does not re-probe.
    let ops = sut.resolve(store_loaded(vec![], vec![]));
    assert!(ops.is_empty(), "the debt was paid: {ops:?}");
}

#[test]
fn opening_the_endpoint_editor_probes_all_four_fields() {
    let mut sut = started();
    let ops = sut.dispatch(Event::EndpointsOpened);
    assert_eq!(
        ops,
        vec![
            Op::FetchServiceHealth {
                field: NetEndpointField::EthereumData,
                base_url: DEFAULT_ETHEREUM_DATA_URL.to_owned(),
            },
            Op::FetchServiceHealth {
                field: NetEndpointField::PasskeyIndex,
                base_url: DEFAULT_PASSKEY_INDEX_URL.to_owned(),
            },
            Op::FetchServiceHealth {
                field: NetEndpointField::BundlerService,
                base_url: DEFAULT_BUNDLER_SERVICE_URL.to_owned(),
            },
            Op::FetchFiatRates {
                url: DEFAULT_FIAT_RATES_URL.to_owned(),
            },
        ]
    );
}

#[test]
fn plain_http_reads_not_https_and_is_never_fetched() {
    let mut sut = started();
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::PasskeyIndex,
        value: "http://index.example".to_owned(),
    });
    let ops = sut.dispatch(Event::EndpointsRefreshRequested);
    assert!(
        !ops.iter().any(|op| matches!(
            op,
            Op::FetchServiceHealth {
                field: NetEndpointField::PasskeyIndex,
                ..
            }
        )),
        "no fetch for a non-HTTPS URL"
    );
    assert_eq!(
        endpoint_view(&sut, NetEndpointField::PasskeyIndex),
        NetServiceHealth::NotHttps
    );
}

#[test]
fn localhost_http_is_allowed_but_lookalike_hosts_are_not() {
    assert!(is_localhost_http("http://localhost:3000/api"));
    assert!(is_localhost_http("http://127.0.0.1:8080"));
    assert!(is_localhost_http("http://localhost"));
    // The regex anchors the host label — 10.0.0.1-style tricks don't pass.
    assert!(!is_localhost_http("http://127.0.0.1.evil.com"));
    assert!(!is_localhost_http("http://localhost.evil.com"));
    assert!(!is_localhost_http("https://localhost"));
    assert!(!is_localhost_http("http://localhost:abc"));

    let mut sut = started();
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::PasskeyIndex,
        value: "http://localhost:3000".to_owned(),
    });
    let ops = sut.dispatch(Event::EndpointsRefreshRequested);
    assert!(ops.iter().any(|op| matches!(
        op,
        Op::FetchServiceHealth {
            field: NetEndpointField::PasskeyIndex,
            ..
        }
    )));
}

/// A passkey index answering with another service's identity is a login
/// hazard — the badge must read invalid, not ok.
#[test]
fn the_endpoint_identity_must_match_the_service() {
    let mut sut = started();
    sut.dispatch(Event::EndpointsOpened);
    // FIFO: data first — a correct identity reads ok.
    sut.resolve(Res::ServiceHealth {
        field: NetEndpointField::EthereumData,
        body: NetHealthBody::Identity {
            service: Some("ethereum-data".to_owned()),
            status: Some("ok".to_owned()),
        },
        latency_ms: 30.0,
    });
    // The passkey field answers with the WRONG service identity.
    sut.resolve(Res::ServiceHealth {
        field: NetEndpointField::PasskeyIndex,
        body: NetHealthBody::Identity {
            service: Some("ethereum-data".to_owned()),
            status: Some("ok".to_owned()),
        },
        latency_ms: 25.0,
    });
    assert_eq!(
        endpoint_view(&sut, NetEndpointField::EthereumData),
        NetServiceHealth::Ok {
            latency_ms: 30.0,
            rate_count: None
        }
    );
    assert_eq!(
        endpoint_view(&sut, NetEndpointField::PasskeyIndex),
        NetServiceHealth::InvalidResponse { latency_ms: 25.0 }
    );
}

#[test]
fn a_degraded_status_is_invalid_even_with_the_right_identity() {
    let mut sut = started();
    sut.dispatch(Event::EndpointsOpened);
    sut.resolve(Res::ServiceHealth {
        field: NetEndpointField::EthereumData,
        body: NetHealthBody::Identity {
            service: Some("ethereum-data".to_owned()),
            status: Some("degraded".to_owned()),
        },
        latency_ms: 30.0,
    });
    assert_eq!(
        endpoint_view(&sut, NetEndpointField::EthereumData),
        NetServiceHealth::InvalidResponse { latency_ms: 30.0 }
    );
}

#[test]
fn saving_an_endpoint_trims_and_strips_crlf_and_flushes_pools() {
    let mut sut = started();
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::PasskeyIndex,
        value: "  https://pk.example/a\r\nX-Injected: 1  ".to_owned(),
    });
    let ops = sut.dispatch(Event::EndpointBlurred {
        field: NetEndpointField::PasskeyIndex,
    });
    let Some(Op::WriteServiceEndpoints { endpoints }) = ops.first() else {
        panic!("expected the endpoints write, got {ops:?}");
    };
    assert_eq!(
        endpoints.passkey_index_url, "https://pk.example/aX-Injected: 1",
        "trimmed, CR/LF stripped — no header injection survives the save"
    );
    assert!(!endpoints.passkey_index_url.contains('\r'));
    assert!(!endpoints.passkey_index_url.contains('\n'));
    assert_eq!(ops.get(1), Some(&Op::InvalidatePools { chain_id: None }));
    // …and all four fields re-probe (2 write ops + 4 probes).
    assert_eq!(ops.len(), 6);
}

#[test]
fn the_probe_base_url_drops_one_trailing_slash_but_the_saved_value_keeps_it() {
    let mut sut = started();
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::PasskeyIndex,
        value: "https://pk.example/".to_owned(),
    });
    let ops = sut.dispatch(Event::EndpointBlurred {
        field: NetEndpointField::PasskeyIndex,
    });
    let Some(Op::WriteServiceEndpoints { endpoints }) = ops.first() else {
        panic!("expected the endpoints write, got {ops:?}");
    };
    assert_eq!(endpoints.passkey_index_url, "https://pk.example/");
    assert!(ops.contains(&Op::FetchServiceHealth {
        field: NetEndpointField::PasskeyIndex,
        base_url: "https://pk.example".to_owned(),
    }));
}

/// set-8: the URL rule DOES gate a save. A value that is not an address the
/// wallet may use is not written (it used to be, and was then used for every
/// request); the service keeps the address it had and the field says so.
/// Health badges still never gate: an https address saves whatever its
/// badge will read.
#[test]
fn an_endpoint_the_url_rule_refuses_is_not_saved() {
    for bad in [
        "http://insecure.example",
        "javascript:alert(1)",
        "ftp://index.example",
        "notaurl",
        "https://",
    ] {
        let mut sut = started();
        sut.dispatch(Event::EndpointEdited {
            field: NetEndpointField::PasskeyIndex,
            value: bad.to_owned(),
        });
        let ops = sut.dispatch(Event::EndpointBlurred {
            field: NetEndpointField::PasskeyIndex,
        });
        assert!(
            !ops.iter()
                .any(|op| matches!(op, Op::WriteServiceEndpoints { .. })),
            "{bad:?} must not be written: {ops:?}"
        );
        assert!(!ops.contains(&Op::InvalidatePools { chain_id: None }));
        let view = sut.view();
        let field = view
            .endpoints
            .iter()
            .find(|e| e.field == NetEndpointField::PasskeyIndex)
            .expect("field");
        assert!(field.refused, "{bad:?}: the field says it was not saved");
        assert_eq!(field.value, bad, "the typed value stays, to be fixed");
        assert_eq!(field.health, NetServiceHealth::NotHttps);

        // The next keystroke clears the refusal; a good address then saves.
        sut.dispatch(Event::EndpointEdited {
            field: NetEndpointField::PasskeyIndex,
            value: "https://index.example".to_owned(),
        });
        assert!(!endpoint_refused(&sut, NetEndpointField::PasskeyIndex));
        let ops = sut.dispatch(Event::EndpointBlurred {
            field: NetEndpointField::PasskeyIndex,
        });
        let Some(Op::WriteServiceEndpoints { endpoints }) = ops.first() else {
            panic!("expected the endpoints write, got {ops:?}");
        };
        assert_eq!(endpoints.passkey_index_url, "https://index.example");
    }
}

#[test]
fn a_blank_endpoint_still_saves_and_reset_clears_a_refusal() {
    let mut sut = started();
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::FiatRates,
        value: "data:text/plain,1".to_owned(),
    });
    sut.dispatch(Event::EndpointBlurred {
        field: NetEndpointField::FiatRates,
    });
    assert!(endpoint_refused(&sut, NetEndpointField::FiatRates));
    sut.dispatch(Event::ResetEndpointsToDefaults);
    assert!(!endpoint_refused(&sut, NetEndpointField::FiatRates));

    // Blank is "the default", not a refused address.
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::FiatRates,
        value: "   ".to_owned(),
    });
    let ops = sut.dispatch(Event::EndpointBlurred {
        field: NetEndpointField::FiatRates,
    });
    assert!(matches!(
        ops.first(),
        Some(Op::WriteServiceEndpoints { .. })
    ));
    assert!(!endpoint_refused(&sut, NetEndpointField::FiatRates));
}

/// A refused address already in storage is not used: `effective` answers
/// the default, as for a blank field.
#[test]
fn a_stored_endpoint_the_url_rule_refuses_resolves_to_the_default() {
    let e = NetServiceEndpoints {
        passkey_index_url: "javascript:alert(1)".to_owned(),
        ethereum_data_url: "http://data.example".to_owned(),
        fiat_rates_url: "http://localhost:8080/rates".to_owned(),
        ..NetServiceEndpoints::default()
    };
    assert_eq!(
        e.effective(NetEndpointField::PasskeyIndex),
        DEFAULT_PASSKEY_INDEX_URL
    );
    assert_eq!(
        e.effective(NetEndpointField::EthereumData),
        DEFAULT_ETHEREUM_DATA_URL
    );
    assert_eq!(
        e.effective(NetEndpointField::FiatRates),
        "http://localhost:8080/rates"
    );
}

/// The one URL rule. The same lists are pinned on the web's draw-time guard
/// (`app-web/vela-wallet/src/lib/services/url-policy.test.ts`); a case added
/// here belongs there too.
#[test]
fn the_url_rule_allows_https_and_loopback_http_only() {
    for ok in [
        "https://rpc.example",
        "https://rpc.example/v2/key?x=1#y",
        "https://rpc.example:8545",
        "https://user:pass@node.example/rpc",
        "HTTPS://Rpc.Example",
        "https://[2001:db8::1]:443/",
        "https://例え.jp",
        "http://localhost",
        "http://localhost:8545/rpc",
        "http://127.0.0.1:3000",
        "http://[::1]:8545",
    ] {
        assert!(is_allowed_endpoint_url(ok), "{ok:?} is allowed");
    }
    for bad in [
        "",
        "javascript:alert(1)",
        "JavaScript:alert(1)",
        "data:text/html,<script>1</script>",
        "ftp://rpc.example",
        "ws://rpc.example",
        "wss://rpc.example",
        "notaurl",
        "rpc.example",
        "https://",
        "https:///path",
        "https://.example",
        "https://rpc.example:",
        "https://rpc.example:abc",
        "https://rpc example",
        "https://[::1]x",
        "https://rpc.example/\r\nX-Injected: 1",
        "http://rpc.example",
        "http://127.0.0.1.evil.com",
        "http://localhost.evil.com",
        "http://localhost:abc",
        "http://localhost?x",
    ] {
        assert!(!is_allowed_endpoint_url(bad), "{bad:?} is refused");
    }
}

#[test]
fn the_fiat_endpoint_validates_the_rate_map_shape() {
    let mut sut = started();
    sut.dispatch(Event::EndpointsOpened);
    // FIFO: skip the three identity fields.
    for field in [
        NetEndpointField::EthereumData,
        NetEndpointField::PasskeyIndex,
        NetEndpointField::BundlerService,
    ] {
        sut.resolve(Res::ServiceHealth {
            field,
            body: NetHealthBody::Failed,
            latency_ms: 0.0,
        });
    }
    sut.resolve(Res::FiatRates {
        body: NetHealthBody::Rates { rate_count: 0 },
        latency_ms: 40.0,
    });
    assert_eq!(
        endpoint_view(&sut, NetEndpointField::FiatRates),
        NetServiceHealth::InvalidResponse { latency_ms: 40.0 },
        "'No rates returned' — reachable but not a rates service"
    );

    sut.dispatch(Event::EndpointsRefreshRequested);
    for field in [
        NetEndpointField::EthereumData,
        NetEndpointField::PasskeyIndex,
        NetEndpointField::BundlerService,
    ] {
        sut.resolve(Res::ServiceHealth {
            field,
            body: NetHealthBody::Failed,
            latency_ms: 0.0,
        });
    }
    sut.resolve(Res::FiatRates {
        body: NetHealthBody::Rates { rate_count: 160 },
        latency_ms: 35.0,
    });
    assert_eq!(
        endpoint_view(&sut, NetEndpointField::FiatRates),
        NetServiceHealth::Ok {
            latency_ms: 35.0,
            rate_count: Some(160)
        }
    );
}

/// Quirk kept from SettingsScreen.tsx:509: reset persists the defaults and
/// re-probes, but does NOT flush the pools (a field blur does).
#[test]
fn resetting_endpoints_restores_defaults_without_a_pool_flush() {
    let mut sut = started();
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::BundlerService,
        value: "https://my-relay.example".to_owned(),
    });
    sut.dispatch(Event::EndpointBlurred {
        field: NetEndpointField::BundlerService,
    });
    let ops = sut.dispatch(Event::ResetEndpointsToDefaults);
    let Some(Op::WriteServiceEndpoints { endpoints }) = ops.first() else {
        panic!("expected the endpoints write, got {ops:?}");
    };
    assert_eq!(endpoints.bundler_service_url, DEFAULT_BUNDLER_SERVICE_URL);
    assert!(
        !ops.iter()
            .any(|op| matches!(op, Op::InvalidatePools { .. })),
        "verbatim quirk: no pool flush on reset"
    );
    assert_eq!(ops.len(), 5, "1 write + 4 probes");
}

// ===========================================================================
// RPC providers, retired (owner decision 2026-10-29): saved keys become the
// per-network RPCs they were serving, then are deleted — once, recoverably
// ===========================================================================

/// Obviously fake keys: these tests must never hold a real one.
const FAKE_ALCHEMY: &str = "fake-alchemy-key";
const FAKE_DRPC: &str = "fake-drpc-key";
const FAKE_ANKR: &str = "fake-ankr-key";

fn provider_keys(alchemy: Option<&str>, drpc: Option<&str>, ankr: Option<&str>) -> NetProviderKeys {
    NetProviderKeys {
        alchemy: alchemy.map(str::to_owned),
        drpc: drpc.map(str::to_owned),
        ankr: ankr.map(str::to_owned),
    }
}

/// Load a store that still holds provider keys: the machine and what it asks.
fn load_with_keys(
    customs: Vec<NetCustomNetwork>,
    overrides: Vec<NetNetworkConfig>,
    keys: NetProviderKeys,
) -> (Sut, Vec<Op>) {
    let mut sut = Sut::new();
    sut.dispatch(Event::Started);
    let ops = sut.resolve(Res::StoreLoaded {
        custom_networks: customs,
        network_configs: overrides,
        endpoints: NetStoredEndpoints::default(),
        provider_keys: keys,
        multisig_service: false,
        shown_networks: None,
    });
    (sut, ops)
}

/// The configs a retirement writes (`None`: nothing to write, keys only).
fn retired_configs(ops: &[Op]) -> Option<Vec<NetNetworkConfig>> {
    match ops {
        [Op::RetireProviderKeys { configs }] => configs.clone(),
        other => panic!("expected exactly the retirement, got {other:?}"),
    }
}

/// The RPC a network gets from `keys` over an otherwise empty store.
fn retired_rpc(keys: NetProviderKeys, chain_id: u32) -> Option<String> {
    let (_, ops) = load_with_keys(vec![], vec![], keys);
    retired_configs(&ops)
        .unwrap_or_default()
        .into_iter()
        .find(|c| c.chain_id == chain_id)
        .map(|c| c.rpc_url)
}

fn written(configs_written: bool, keys_removed: bool) -> Res {
    Res::ProviderKeysRetired {
        configs_written,
        keys_removed,
    }
}

fn shown_rpc(sut: &Sut, chain_id: u32) -> String {
    sut.view()
        .networks
        .iter()
        .find(|row| row.chain_id == chain_id)
        .expect("listed")
        .rpc_url
        .clone()
}

/// No keys, nothing to retire: the load asks for nothing.
#[test]
fn a_store_without_provider_keys_retires_nothing() {
    let (_, ops) = load_with_keys(vec![], vec![], NetProviderKeys::default());
    assert!(ops.is_empty());
}

/// Every network the provider tier was serving keeps that endpoint, now as
/// its own RPC — and only those: a chain the provider has no slug for keeps
/// Vela's.
#[test]
fn a_saved_key_becomes_the_rpc_of_every_network_it_served() {
    let (sut, ops) = load_with_keys(vec![], vec![], provider_keys(None, None, Some(FAKE_ANKR)));
    let configs = retired_configs(&ops).expect("configs to write");
    let chains: Vec<u32> = configs.iter().map(|c| c.chain_id).collect();
    // Ankr's eight, in the built-in order (the owner's, 2026-10-29).
    assert_eq!(chains, vec![1, 56, 8453, 137, 42161, 43114, 10, 100]);
    let gnosis = configs.iter().find(|c| c.chain_id == 100).expect("gnosis");
    assert_eq!(
        gnosis.rpc_url,
        format!("https://rpc.ankr.com/gnosis/{FAKE_ANKR}")
    );
    // The rest of the record is the network's defaults — a built-in's
    // explorer, and no bundler of its own.
    let builtin = BUILTIN_CHAINS
        .iter()
        .find(|c| c.chain_id == 100)
        .expect("built in");
    assert_eq!(gnosis.explorer_url, builtin.explorer_url);
    assert_eq!(gnosis.bundler_url, "");
    assert!(!gnosis.only_mine);
    // Shown at once: the network's RPC is the one it was asking — and, §41
    // (P2-8), a third party's keyed endpoint is not the person's own node.
    assert_eq!(shown_rpc(&sut, 100), gnosis.rpc_url);
    assert!(
        !sut.view()
            .networks
            .iter()
            .find(|r| r.chain_id == 100)
            .unwrap()
            .own_rpc
    );
    // Tempo had no Ankr slug: it keeps Vela's endpoint.
    assert_eq!(
        retired_rpc(provider_keys(None, None, Some(FAKE_ANKR)), 4217),
        None
    );
}

/// The URLs are the pool's provider tier's, byte for byte.
#[test]
fn the_retired_urls_are_the_ones_the_provider_tier_asked() {
    assert_eq!(
        retired_rpc(provider_keys(Some("k"), None, None), 1).as_deref(),
        Some("https://eth-mainnet.g.alchemy.com/v2/k")
    );
    assert_eq!(
        retired_rpc(provider_keys(None, Some("k"), None), 8453).as_deref(),
        Some("https://lb.drpc.org/ogrpc?network=base&dkey=k")
    );
    assert_eq!(
        retired_rpc(provider_keys(None, None, Some("k")), 100).as_deref(),
        Some("https://rpc.ankr.com/gnosis/k")
    );
    // Ankr has no Tempo; an empty key built no URL.
    assert_eq!(
        retired_rpc(provider_keys(None, None, Some("k")), 4217),
        None
    );
    assert_eq!(retired_rpc(provider_keys(Some(""), None, None), 1), None);
    // X Layer and Arc were served; XRPL EVM by nobody.
    assert_eq!(
        retired_rpc(provider_keys(Some("k"), None, None), 196).as_deref(),
        Some("https://xlayer-mainnet.g.alchemy.com/v2/k")
    );
    assert_eq!(
        retired_rpc(provider_keys(None, Some("key"), None), 5_042).as_deref(),
        Some("https://lb.drpc.org/ogrpc?network=arc&dkey=key")
    );
    assert_eq!(
        retired_rpc(provider_keys(None, None, Some("key")), 5_042),
        None
    );
    assert_eq!(
        retired_rpc(provider_keys(Some("k"), Some("k"), Some("k")), 1_440_000),
        None
    );
}

/// Where more than one key served a chain, the tier's own cold-start order —
/// Alchemy, dRPC, Ankr — says which one the network keeps.
#[test]
fn with_several_keys_a_network_keeps_the_one_the_old_order_picked() {
    let keys = provider_keys(Some(FAKE_ALCHEMY), Some(FAKE_DRPC), Some(FAKE_ANKR));
    assert_eq!(
        retired_rpc(keys.clone(), 1),
        Some(format!(
            "https://eth-mainnet.g.alchemy.com/v2/{FAKE_ALCHEMY}"
        ))
    );
    let keys = provider_keys(None, Some(FAKE_DRPC), Some(FAKE_ANKR));
    assert_eq!(
        retired_rpc(keys, 56),
        Some(format!(
            "https://lb.drpc.org/ogrpc?network=bsc&dkey={FAKE_DRPC}"
        ))
    );
}

/// A network with an RPC of its own asked it first, ahead of any provider:
/// it keeps it. One whose saved RPC is just the shipped one did ask the
/// provider, and moves to it — keeping the rest of what it had saved.
#[test]
fn a_network_with_its_own_rpc_keeps_it() {
    let own = NetNetworkConfig {
        chain_id: 1,
        rpc_url: "https://my-node.example".to_owned(),
        explorer_url: "https://etherscan.io".to_owned(),
        bundler_url: String::new(),
        only_mine: true,
    };
    let base = BUILTIN_CHAINS
        .iter()
        .find(|c| c.chain_id == 8453)
        .expect("base");
    let shipped = NetNetworkConfig {
        chain_id: 8453,
        rpc_url: base.rpc_url.to_owned(),
        explorer_url: "https://my-scan.example".to_owned(),
        bundler_url: String::new(),
        only_mine: false,
    };
    let (_, ops) = load_with_keys(
        vec![],
        vec![own.clone(), shipped],
        provider_keys(Some(FAKE_ALCHEMY), None, None),
    );
    let configs = retired_configs(&ops).expect("configs");
    // Storage order kept; the own node untouched.
    assert_eq!(configs[0], own);
    assert_eq!(configs[1].chain_id, 8453);
    assert_eq!(
        configs[1].rpc_url,
        format!("https://base-mainnet.g.alchemy.com/v2/{FAKE_ALCHEMY}")
    );
    assert_eq!(configs[1].explorer_url, "https://my-scan.example");
    assert_eq!(configs.iter().filter(|c| c.chain_id == 1).count(), 1);
}

/// A network the person added is weighed by the same rule; one no provider
/// has a slug for keeps the RPC it was added with.
#[test]
fn a_network_the_person_added_that_no_provider_served_keeps_its_rpc() {
    let (_, ops) = load_with_keys(
        vec![custom_network(NEW_CHAIN)],
        vec![],
        provider_keys(Some(FAKE_ALCHEMY), Some(FAKE_DRPC), Some(FAKE_ANKR)),
    );
    let configs = retired_configs(&ops).expect("configs");
    assert!(configs.iter().all(|c| c.chain_id != NEW_CHAIN));
}

/// Keys saved, but every network they served has an RPC of its own: nothing
/// to write, the keys still go.
#[test]
fn keys_that_served_nothing_are_deleted_without_a_write() {
    let overrides: Vec<NetNetworkConfig> = [1, 56, 137, 42161, 10, 8453, 43114, 100]
        .into_iter()
        .map(|chain_id| NetNetworkConfig {
            chain_id,
            rpc_url: format!("https://node-{chain_id}.example"),
            explorer_url: String::new(),
            bundler_url: String::new(),
            only_mine: false,
        })
        .collect();
    let (_, ops) = load_with_keys(
        vec![],
        overrides,
        provider_keys(None, None, Some(FAKE_ANKR)),
    );
    assert_eq!(ops, vec![Op::RetireProviderKeys { configs: None }]);
}

/// Written and deleted: every pool re-reads, and a second load (the keys are
/// gone) asks for nothing — once.
#[test]
fn a_finished_retirement_flushes_the_pools_and_is_not_repeated() {
    let (mut sut, ops) = load_with_keys(
        vec![],
        vec![],
        provider_keys(Some(FAKE_ALCHEMY), None, None),
    );
    let configs = retired_configs(&ops).expect("configs");
    let ops = sut.resolve(written(true, true));
    assert_eq!(ops, vec![Op::InvalidatePools { chain_id: None }]);
    sut.resolve(Res::Invalidated);
    let url = format!("https://eth-mainnet.g.alchemy.com/v2/{FAKE_ALCHEMY}");
    assert_eq!(shown_rpc(&sut, 1), url);

    // The next launch reads what was written, and no keys.
    let ops = sut.dispatch(Event::Started);
    assert_eq!(ops, vec![Op::ReadStore]);
    let ops = sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: configs,
        endpoints: NetStoredEndpoints::default(),
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: None,
    });
    assert!(ops.is_empty());
    assert_eq!(shown_rpc(&sut, 1), url);
}

/// The write failed: the keys stay, the ledger goes back to what storage
/// holds, and the next load tries again — the same answer.
#[test]
fn a_failed_retirement_keeps_the_keys_and_retries() {
    let keys = provider_keys(Some(FAKE_ALCHEMY), None, None);
    let (mut sut, ops) = load_with_keys(vec![], vec![], keys.clone());
    let first = retired_configs(&ops).expect("configs");
    let ops = sut.resolve(written(false, false));
    assert!(ops.is_empty(), "no pool flush for nothing written: {ops:?}");
    let builtin = BUILTIN_CHAINS
        .iter()
        .find(|c| c.chain_id == 1)
        .expect("ethereum");
    assert_eq!(shown_rpc(&sut, 1), builtin.rpc_url);

    sut.dispatch(Event::Started);
    let ops = sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![],
        endpoints: NetStoredEndpoints::default(),
        provider_keys: keys,
        multisig_service: false,
        shown_networks: None,
    });
    assert_eq!(retired_configs(&ops), Some(first));
}

/// The configs were written but the keys could not be deleted: the next load
/// finds every served network on its own RPC already, writes nothing, and
/// deletes the keys — idempotent.
#[test]
fn a_half_finished_retirement_finishes_without_writing_twice() {
    let keys = provider_keys(Some(FAKE_ALCHEMY), None, None);
    let (mut sut, ops) = load_with_keys(vec![], vec![], keys.clone());
    let configs = retired_configs(&ops).expect("configs");
    let ops = sut.resolve(written(true, false));
    assert_eq!(ops, vec![Op::InvalidatePools { chain_id: None }]);
    sut.resolve(Res::Invalidated);

    sut.dispatch(Event::Started);
    let ops = sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: configs,
        endpoints: NetStoredEndpoints::default(),
        provider_keys: keys,
        multisig_service: false,
        shown_networks: None,
    });
    assert_eq!(ops, vec![Op::RetireProviderKeys { configs: None }]);
}

/// An RPC the person saves while the write is in flight is theirs: a failed
/// retirement does not put the old one back over it.
#[test]
fn a_failed_retirement_never_undoes_what_the_person_saved_since() {
    let (mut sut, _) = load_with_keys(
        vec![],
        vec![],
        provider_keys(Some(FAKE_ALCHEMY), None, None),
    );
    sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        value: "https://my-node.example".to_owned(),
    });
    // The card's probes answer: the node is Ethereum.
    sut.resolve_matching(
        |op| matches!(op, Op::ProbeRpc { url } if url == "https://my-node.example"),
        probe("https://my-node.example", Some(1), 40.0),
    );
    sut.dispatch(Event::OverrideBlurred { chain_id: 1 });
    sut.resolve_matching(
        |op| matches!(op, Op::RetireProviderKeys { .. }),
        written(false, false),
    );
    // The next write of the ledger: the person's node stays; a network only
    // the failed retirement had touched is back on its shipped RPC.
    let ops = sut.dispatch(Event::OnlyMineChanged {
        chain_id: 1,
        on: true,
    });
    let Some(Op::WriteNetworkConfigs { configs }) = ops.first() else {
        panic!("expected the config write, got {ops:?}");
    };
    let mainnet = configs.iter().find(|c| c.chain_id == 1).expect("ethereum");
    assert_eq!(mainnet.rpc_url, "https://my-node.example");
    assert!(configs.iter().all(|c| c.chain_id != 56), "{configs:?}");
}

/// The keys travel to storage and nowhere else: no other operation carries
/// them, and nothing ever writes them back.
#[test]
fn a_retirement_sends_the_keys_nowhere_but_storage() {
    let (mut sut, ops) = load_with_keys(
        vec![],
        vec![],
        provider_keys(Some(FAKE_ALCHEMY), None, None),
    );
    assert!(matches!(ops.as_slice(), [Op::RetireProviderKeys { .. }]));
    let ops = sut.resolve(written(true, true));
    for op in &ops {
        assert!(!format!("{op:?}").contains(FAKE_ALCHEMY), "{op:?}");
    }
}

// ===========================================================================
// Invariant ⑨ — unknown chain ⇒ NO explorer link
// ===========================================================================

#[test]
fn an_unknown_chain_yields_no_explorer_link() {
    assert_eq!(explorer_base_url(31_337, &[]), None);
    assert_eq!(
        explorer_base_url(1, &[]),
        Some("https://etherscan.io".to_owned())
    );
    // A custom network's trailing slash is stripped.
    assert_eq!(
        explorer_base_url(999, &[custom_network(999)]),
        Some("https://custom-scan.example".to_owned())
    );
    // An empty configured explorer is no link at all — never a fallback.
    let mut bare = custom_network(998);
    bare.explorer_url = String::new();
    assert_eq!(explorer_base_url(998, &[bare]), None);
    // set-7: an explorer the URL rule refuses is no link either — a
    // `javascript:` one used to become the `href` of "View on explorer".
    for bad in [
        "javascript:alert(1)",
        "data:text/html,x",
        "http://scan.example",
    ] {
        let mut evil = custom_network(997);
        evil.explorer_url = bad.to_owned();
        assert_eq!(explorer_base_url(997, &[evil]), None, "{bad:?}");
    }
}

/// The catalog's explorer list is data from the network: the first entry the
/// URL rule allows is the one saved.
#[test]
fn a_catalog_explorer_the_url_rule_refuses_is_skipped() {
    let mut data = raw_chain();
    data.explorers = vec![
        "javascript:alert(1)".to_owned(),
        String::new(),
        "https://scan.example".to_owned(),
    ];
    let info = parse_chain_data(&data, NEW_CHAIN, DEFAULT_ETHEREUM_DATA_URL);
    assert_eq!(info.explorer_url, "https://scan.example");
    data.explorers = vec!["javascript:alert(1)".to_owned()];
    let info = parse_chain_data(&data, NEW_CHAIN, DEFAULT_ETHEREUM_DATA_URL);
    assert_eq!(info.explorer_url, "");
}

/// set-6: a per-network RPC or explorer the URL rule refuses is never probed
/// and never written — the previous endpoint keeps serving — and the card
/// says which field. A `data:` explorer "answers" a no-cors fetch, which is
/// how a green pill once stood beside one.
#[test]
fn an_override_the_url_rule_refuses_is_neither_probed_nor_saved() {
    for (field, bad) in [
        (NetOverrideField::Rpc, "javascript:alert(1)"),
        (NetOverrideField::Rpc, "ftp://rpc.example"),
        (NetOverrideField::Rpc, "http://rpc.example"),
        (NetOverrideField::Explorer, "javascript:alert(1)"),
        (NetOverrideField::Explorer, "data:text/html,x"),
    ] {
        let mut sut = started();
        sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
        answer_card_probes(
            &mut sut,
            "https://ethereum-rpc.publicnode.com",
            Some(1),
            ETH_SCAN,
        );
        let ops = sut.dispatch(Event::OverrideFieldEdited {
            chain_id: 1,
            field,
            value: bad.to_owned(),
        });
        assert!(
            !ops.iter().any(|op| matches!(
                op,
                Op::ProbeRpc { url } | Op::ProbeReachable { url } if url == bad
            )),
            "{bad:?} is never fetched: {ops:?}"
        );
        let row = &sut.view().networks[0];
        assert_eq!(
            row.rpc_url_refused,
            field == NetOverrideField::Rpc,
            "{bad:?}"
        );
        assert_eq!(
            row.explorer_url_refused,
            field == NetOverrideField::Explorer,
            "{bad:?}"
        );
        let refused_health = match field {
            NetOverrideField::Rpc => row.rpc_health.clone(),
            NetOverrideField::Explorer => row.explorer_health.clone(),
        };
        assert_eq!(refused_health, Some(NetProbeHealth::Error), "{bad:?}");

        // Answer whatever the other field asked for, then leave the field.
        for op in ops {
            match op {
                Op::ProbeRpc { url } => {
                    sut.resolve(probe(&url, Some(1), 12.0));
                }
                Op::ProbeReachable { url } => {
                    sut.resolve(Res::Reachable {
                        url,
                        ok: true,
                        latency_ms: 20.0,
                    });
                }
                _ => {}
            }
        }
        let ops = sut.dispatch(Event::OverrideBlurred { chain_id: 1 });
        assert!(ops.is_empty(), "{bad:?}: nothing written: {ops:?}");
        assert!(!sut.view().networks[0].rpc_save_deferred);
    }
}

/// A blur while the RPC probe is still out does not hold a refused save:
/// the URL rule answers it at once.
#[test]
fn a_refused_explorer_is_refused_even_before_the_rpc_probe_answers() {
    let mut sut = started();
    sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 1,
        field: NetOverrideField::Explorer,
        value: "javascript:alert(1)".to_owned(),
    });
    let ops = sut.dispatch(Event::OverrideBlurred { chain_id: 1 });
    assert!(ops.is_empty());
    let row = &sut.view().networks[0];
    assert!(!row.rpc_save_deferred);
    assert!(row.explorer_url_refused);
}

// ===========================================================================
// The unified probe vocabulary
// ===========================================================================

/// The inventory's "four eth_chainId probes become ONE operation word":
/// wizard candidates and override RPC health both speak `ProbeRpc`; nothing
/// else probes a chain id.
#[test]
fn every_chain_id_probe_speaks_the_one_probe_operation() {
    let mut sut = started();
    let wizard_ops = select_and_resolve(&mut sut, raw_chain());
    assert!(wizard_ops
        .iter()
        .all(|op| matches!(op, Op::ProbeRpc { .. })));

    let mut sut = started();
    let card_ops = sut.dispatch(Event::OverrideExpanded { chain_id: 1 });
    assert!(matches!(card_ops[0], Op::ProbeRpc { .. }));
}

// ===========================================================================
// Pure helpers — ported verbatim
// ===========================================================================

#[test]
fn code_deployment_follows_check_code_verbatim() {
    assert!(!is_code_deployed(None));
    assert!(!is_code_deployed(Some("")));
    assert!(!is_code_deployed(Some("0x")));
    assert!(!is_code_deployed(Some("0x0")));
    // Any longer answer — even a zkSync bytecode hash — counts as deployed.
    assert!(is_code_deployed(Some("0x1234")));
    assert!(is_code_deployed(Some("0x1")));
}

#[test]
fn p256_call_acceptance_follows_the_ts_conditions() {
    // Value 1 at 32 bytes (len 66) — accepted.
    assert!(p256_call_indicates_support(Some(&p256_one())));
    // Too short, empty, bare 0x, wrong value, non-hex — all rejected.
    assert!(!p256_call_indicates_support(Some("0x1")));
    assert!(!p256_call_indicates_support(Some("")));
    assert!(!p256_call_indicates_support(Some("0x")));
    assert!(!p256_call_indicates_support(Some(&format!(
        "0x{}2",
        "0".repeat(63)
    ))));
    assert!(!p256_call_indicates_support(Some(&format!(
        "0x{}g",
        "0".repeat(65)
    ))));
    assert!(!p256_call_indicates_support(None));
}

#[test]
fn endpoint_cleaning_strips_exactly_whitespace_and_crlf() {
    assert_eq!(
        clean_endpoint_value("  https://a.example  "),
        "https://a.example"
    );
    assert_eq!(
        clean_endpoint_value("https://a.example/x\r\nHeader: y"),
        "https://a.example/xHeader: y"
    );
    // Interior spaces survive — only CR/LF is stripped inside.
    assert_eq!(clean_endpoint_value("a b"), "a b");
}

/// Arc (spec 060) — the thirteenth built-in network. Every field here is load
/// bearing: the native symbol drives the $1 peg (and therefore the $0.01 fee
/// floor), and the RPC and explorer are what a person actually reaches.
#[test]
fn arc_is_a_builtin_network_with_the_fields_the_rest_of_the_wallet_reads() {
    // 24 mainnets and the testnet space's 9 (spec 105).
    assert_eq!(BUILTIN_CHAINS.len(), 33);
    let arc = BUILTIN_CHAINS
        .iter()
        .find(|c| c.chain_id == 5_042)
        .expect("Arc must be built in");
    assert_eq!(arc.id, "arc");
    assert_eq!(arc.display_name, "Arc");
    // NOT "USD": Arc's gas coin is USDC and every surface names it that.
    assert_eq!(arc.native_symbol, "USDC");
    assert_eq!(arc.rpc_url, "https://rpc.mainnet.arc.io");
    assert_eq!(arc.explorer_url, "https://explorer.arc.io");
    // ~0.48 s blocks with deterministic finality.
    assert_eq!(arc.typical_inclusion_s, 2);
}

/// Arc testnet was reachable only by adding it (spec 060); since spec 105 it
/// ships with the testnet space, beside the other common public testnets —
/// and like every built-in, its relayer is the operator's to keep funded.
#[test]
fn arc_testnet_is_a_builtin_testnet() {
    let arc = BUILTIN_CHAINS
        .iter()
        .find(|c| c.chain_id == 5_042_002)
        .expect("Arc Testnet ships");
    assert!(arc.testnet);
    assert_eq!(arc.native_symbol, "USDC");
    assert!(BUILTIN_CHAINS
        .iter()
        .filter(|c| !c.testnet)
        .all(|c| c.chain_id != 5_042_002));
}

/// The services moved to `getvela.app` (spec 060). These constants are what a
/// clean install reaches on first run.
#[test]
fn builtin_service_endpoints_point_at_the_cloudflare_deployments() {
    assert_eq!(
        DEFAULT_ETHEREUM_DATA_URL,
        "https://ethereum-data.getvela.app"
    );
    assert_eq!(
        DEFAULT_BUNDLER_SERVICE_URL,
        "https://vela-relay-cf.getvela.app"
    );
}

/// Who can fix an out-of-gas relayer (spec 060). On a network Vela ships the
/// operator owns it; on one a person added — a devnet, an internal chain —
/// there may be nobody else who could hold gas on it at all, and the sheet must
/// not ask them to report it to someone who cannot help.
#[test]
fn a_builtin_network_is_one_whose_relayer_the_operator_owns() {
    for chain_id in [
        1, 10, 56, 100, 130, 137, 143, 480, 4_217, 5_042, 8_453, 42_161, 43_114, 196, 988, 1_868,
        4_326, 4_663, 5_000, 8_217, 42_220, 57_073, 98_866, 1_440_000,
    ] {
        assert!(
            is_builtin_chain(chain_id),
            "chain {chain_id} ships with Vela"
        );
    }
    // The testnet space's built-ins too (spec 105): Arc Testnet, Sepolia, …
    assert!(is_builtin_chain(5_042_002));
    assert!(is_builtin_chain(11_155_111));
    assert!(
        !is_builtin_chain(31_337),
        "a local devnet is nobody else's to fund"
    );
    assert!(!is_builtin_chain(1_337));
}

/// The eleven admitted with spec 061. Each field is what a person reaches or
/// what a money rule reads.
#[test]
fn the_eleven_are_built_in() {
    let by_id = |id: u32| {
        BUILTIN_CHAINS
            .iter()
            .find(|c| c.chain_id == id)
            .expect("built in")
    };
    assert_eq!(by_id(988).native_symbol, "USDT0");
    assert_eq!(by_id(196).native_symbol, "OKB");
    assert_eq!(by_id(1_440_000).native_symbol, "XRP");
}

/// The curated public endpoints: built-in networks only, https only, no
/// endpoint twice — and none of the ones measured dead (2026-10-10: every
/// `1rpc.io`, the rate-limited `bsc.drpc.org`).
#[test]
fn the_public_rpcs_are_built_in_https_and_alive_when_measured() {
    let mut seen = std::collections::BTreeSet::new();
    for (chain_id, urls) in PUBLIC_RPCS {
        assert!(
            BUILTIN_CHAINS.iter().any(|c| c.chain_id == *chain_id),
            "{chain_id} is not a built-in network"
        );
        assert!(!urls.is_empty());
        for url in *urls {
            assert!(url.starts_with("https://"), "{url}");
            assert!(seen.insert(*url), "{url} twice");
            assert!(!url.contains("1rpc.io"), "{url}: 1rpc.io is gone");
            assert_ne!(*url, "https://bsc.drpc.org", "rate-limited");
        }
    }
    assert_eq!(
        public_rpc_urls(137),
        [
            "https://polygon-bor-rpc.publicnode.com",
            "https://polygon.gateway.tenderly.co"
        ]
    );
    assert!(public_rpc_urls(NEW_CHAIN).is_empty());
}

// ===========================================================================
// PR 3 notes 5, 10, 18: every path a network is added by says why it stopped
// ===========================================================================

/// The path with no confirm step (a scanned request, "Add this network")
/// used to drop its check and could only say "Incompatible". It keeps it: a
/// network with no P-256 verifier says so there too, with no Chain Setup
/// link — and one that only lacks contracts gets the link, on that chain.
#[test]
fn the_scan_path_keeps_its_check_so_a_refusal_says_why() {
    use vela_core::app::network_admin::{WIZARD_CHECK_FAILED, WIZARD_NOT_COMPATIBLE};
    let scan = || {
        let mut sut = started();
        sut.dispatch(Event::AddByChainIdRequested {
            chain_id: NEW_CHAIN,
            now_iso: NOW_ISO.to_owned(),
        });
        sut.resolve(Res::ChainInfo {
            chain_id: NEW_CHAIN,
            data: Some(raw_chain()),
        });
        resolve_race(&mut sut);
        sut
    };

    // No P-256 verifier: contracts all there, the precompile is not.
    let mut sut = scan();
    for (_, address, _) in REQUIRED_CONTRACTS {
        sut.resolve(code_ok(RPC_FAST, address));
    }
    sut.resolve(Res::P256Call {
        url: RPC_FAST.to_owned(),
        result: Some("0x".to_owned()),
    });
    sut.resolve(code_result(RPC_FAST, P256_PRECOMPILE, Some("0x")));
    let wizard = sut.view().wizard;
    assert_eq!(
        wizard.error,
        Some(NetWizardErrorKind::NotCompatible {
            chain_id: NEW_CHAIN
        })
    );
    let compat = wizard.compat.expect("the check is kept beside the error");
    assert_eq!(compat.blocker, Some(NetBlocker::NoP256));
    assert_eq!(compat.setup_url, None, "nothing to deploy");
    assert_eq!(wizard.error_key.as_deref(), Some(NO_P256_HINT));
    assert!(!wizard.can_add);
    assert_eq!(
        sut.view().networks.len(),
        BUILTIN_CHAINS.len(),
        "nothing saved"
    );

    // Missing contracts: the verifier answers, the contracts are not there.
    let mut sut = scan();
    for (_, address, _) in REQUIRED_CONTRACTS {
        sut.resolve(code_result(RPC_FAST, address, Some("0x")));
    }
    sut.resolve(Res::P256Call {
        url: RPC_FAST.to_owned(),
        result: Some(p256_one()),
    });
    let wizard = sut.view().wizard;
    let compat = wizard.compat.expect("the check is kept beside the error");
    assert_eq!(compat.blocker, Some(NetBlocker::MissingContracts));
    assert_eq!(
        compat.setup_url.as_deref(),
        Some(chain_setup_url(NEW_CHAIN).as_str())
    );
    assert_eq!(wizard.error_key.as_deref(), Some(MISSING_CONTRACTS_HINT));

    // Inconclusive: the probes failed. Kept too, and never worded as a
    // refusal — no reason, no link.
    let mut sut = scan_unreachable();
    let wizard = sut.view().wizard;
    assert_eq!(
        wizard.error,
        Some(NetWizardErrorKind::CheckFailed {
            chain_id: NEW_CHAIN
        })
    );
    assert_eq!(wizard.error_key.as_deref(), Some(WIZARD_CHECK_FAILED));
    let compat = wizard.compat.expect("kept");
    assert!(compat.rpc_failure.is_some());
    assert_eq!((compat.blocker, compat.setup_url), (None, None));

    // The next selection starts clean: no stale reason under a new chain.
    sut.dispatch(Event::ChainSelected {
        chain_id: 8888,
        keep_custom_rpc: false,
    });
    let wizard = sut.view().wizard;
    assert_eq!((wizard.compat, wizard.error_key), (None, None));

    // A refusal whose check named no reason still has a sentence.
    assert_eq!(
        vela_core::app::network_admin::wizard_error_key(
            &NetWizardErrorKind::NotCompatible { chain_id: 1 },
            None
        ),
        WIZARD_NOT_COMPATIBLE
    );
}

/// The scan path with both probes failing.
fn scan_unreachable() -> Sut {
    let mut sut = started();
    sut.dispatch(Event::AddByChainIdRequested {
        chain_id: NEW_CHAIN,
        now_iso: NOW_ISO.to_owned(),
    });
    sut.resolve(Res::ChainInfo {
        chain_id: NEW_CHAIN,
        data: Some(raw_chain()),
    });
    sut.resolve(probe(RPC_SLOW, None, 0.0));
    sut.resolve(probe(RPC_FAST, None, 0.0));
    sut
}

/// Every way the wizard stops has a sentence of its own, in the corpus: on
/// the web three of them had none and read "Incompatible" — over a network
/// that was merely already added, or not found.
#[test]
fn every_wizard_stop_has_its_own_sentence_in_the_corpus() {
    use vela_core::app::network_admin::{
        WIZARD_ALREADY_ADDED, WIZARD_CHECK_FAILED, WIZARD_NOT_COMPATIBLE, WIZARD_NOT_FOUND,
        WIZARD_NO_RPC_ENDPOINT,
    };
    // Already added.
    let mut sut = started_with(vec![custom_network(999)], vec![]);
    sut.dispatch(Event::ChainSelected {
        chain_id: 999,
        keep_custom_rpc: false,
    });
    assert_eq!(
        sut.view().wizard.error_key.as_deref(),
        Some(WIZARD_ALREADY_ADDED)
    );

    // Not found.
    let mut sut = started();
    sut.dispatch(Event::ChainSelected {
        chain_id: NEW_CHAIN,
        keep_custom_rpc: false,
    });
    sut.resolve(Res::ChainInfo {
        chain_id: NEW_CHAIN,
        data: None,
    });
    assert_eq!(
        sut.view().wizard.error_key.as_deref(),
        Some(WIZARD_NOT_FOUND)
    );

    // No RPC endpoint.
    let mut sut = started();
    let mut bare = raw_chain();
    bare.rpc.clear();
    select_and_resolve(&mut sut, bare);
    let wizard = sut.view().wizard;
    assert_eq!(wizard.error, Some(NetWizardErrorKind::NoRpcEndpoint));
    assert_eq!(wizard.error_key.as_deref(), Some(WIZARD_NO_RPC_ENDPOINT));

    // No stop, no sentence.
    assert_eq!(started().view().wizard.error_key, None);

    let i18n = vela_core::i18n::I18n::embedded().expect("embedded corpus");
    let opts = vela_core::i18n::Options::default();
    for key in [
        WIZARD_ALREADY_ADDED,
        WIZARD_NOT_FOUND,
        WIZARD_NO_RPC_ENDPOINT,
        WIZARD_NOT_COMPATIBLE,
        WIZARD_CHECK_FAILED,
    ] {
        assert!(i18n.exists(key, &opts), "{key}");
    }
}

// ===========================================================================
// PR 3 notes 20, 25: the public tier, its dead hosts, and what the store is told
// ===========================================================================

/// The host is compared, never a substring: a path that mentions the name is
/// not that host, and a subdomain of it is.
#[test]
fn a_dead_rpc_host_is_matched_by_host_only() {
    use vela_core::app::network_admin::{is_dead_rpc_host, DEAD_RPC_HOSTS};
    assert_eq!(DEAD_RPC_HOSTS, ["1rpc.io"]);
    for dead in [
        "https://1rpc.io/matic",
        "https://1RPC.io/eth",
        "https://1rpc.io",
        "https://1rpc.io:443/arb",
        "https://key@1rpc.io/op",
        "https://eu.1rpc.io/base",
        "1rpc.io/gnosis",
    ] {
        assert!(is_dead_rpc_host(dead), "{dead}");
    }
    for alive in [
        "https://polygon.gateway.tenderly.co",
        "https://not1rpc.io/matic",
        "https://1rpc.io.example.com/eth",
        "https://example.com/1rpc.io/eth",
        "https://example.com/?via=1rpc.io",
        "",
    ] {
        assert!(!is_dead_rpc_host(alive), "{alive}");
    }
    // Nothing Vela ships names one: not the public tier, not a default.
    for (_, urls) in PUBLIC_RPCS {
        for url in *urls {
            assert!(!is_dead_rpc_host(url), "{url}");
        }
    }
    for chain in BUILTIN_CHAINS {
        assert!(!is_dead_rpc_host(chain.rpc_url), "{}", chain.rpc_url);
    }
}

const PRIVACY_EVIDENCE: &str = "../../../docs/store-submission/privacy-evidence.md";
const DOC_BEGIN: &str = "<!-- public-rpcs:begin -->";
const DOC_END: &str = "<!-- public-rpcs:end -->";

/// The table the privacy evidence carries between its two markers: every
/// host of [`PUBLIC_RPCS`], chain by chain, in the order they are asked.
fn public_rpc_table() -> String {
    let mut out = String::from("| Network | Hosts asked, in order |\n|---|---|\n");
    for (chain_id, urls) in PUBLIC_RPCS {
        let name = BUILTIN_CHAINS
            .iter()
            .find(|chain| chain.chain_id == *chain_id)
            .map_or("", |chain| chain.display_name);
        let hosts: Vec<String> = urls
            .iter()
            .map(|url| {
                let host = url.trim_start_matches("https://");
                format!("`{}`", host.split('/').next().unwrap_or(host))
            })
            .collect();
        out.push_str(&format!("| {name} ({chain_id}) | {} |\n", hosts.join(", ")));
    }
    out
}

/// What the store is told about third parties is what the wallet contacts:
/// the privacy evidence's list of public RPC hosts IS the core's list, host
/// for host. It was written by hand, and named `1rpc.io` for weeks after
/// the wallet had stopped asking it. Regenerate after changing the list:
/// `VELA_WRITE_FIXTURES=1 cargo test -p vela-core --features crux --test app_network_admin`.
#[test]
fn the_privacy_evidence_names_exactly_the_public_rpc_hosts() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PRIVACY_EVIDENCE);
    let doc = std::fs::read_to_string(&path).expect("docs/store-submission/privacy-evidence.md");
    let (head, rest) = doc.split_once(DOC_BEGIN).expect("the begin marker");
    let (have, tail) = rest.split_once(DOC_END).expect("the end marker");
    let want = format!("\n{}", public_rpc_table());
    if have != want && std::env::var_os("VELA_WRITE_FIXTURES").is_some() {
        let rewritten = format!("{head}{DOC_BEGIN}{want}{DOC_END}{tail}");
        std::fs::write(&path, rewritten).expect("write the privacy evidence");
        return;
    }
    assert!(
        have == want,
        "docs/store-submission/privacy-evidence.md lists other public RPC hosts than \
         PUBLIC_RPCS — regenerate with VELA_WRITE_FIXTURES=1 cargo test -p vela-core \
         --features crux --test app_network_admin\n--- doc\n{have}\n--- core\n{want}"
    );
    // And no host the pool refuses to ask is named anywhere as contacted.
    for dead in vela_core::app::network_admin::DEAD_RPC_HOSTS {
        assert!(!have.contains(dead), "{dead} is listed as contacted");
    }
}

// ===========================================================================
// PR 3 final notes F4, F14, F22: one rule for the RPC field and its label
// ===========================================================================

/// The field where a person names an endpoint of their own — and "Re-check
/// with this RPC", which is drawn exactly where the field is — follows the
/// state, not the shell: required (and not called "optional") when the
/// network lists no endpoint; optional when the check passed or could not
/// reach a verdict; absent under a refusal another endpoint would not
/// change, and where there is no chain to check.
#[test]
fn the_rpc_field_is_offered_where_another_endpoint_is_a_way_on() {
    use vela_core::app::network_admin::{NetRpcField, RPC_FIELD_OPTIONAL, RPC_FIELD_REQUIRED};
    let field = |sut: &Sut| {
        let wizard = sut.view().wizard;
        (wizard.rpc_field, wizard.rpc_field_label_key)
    };

    // Nothing selected, searching, checking: no field.
    let mut sut = started();
    assert_eq!(field(&sut), (NetRpcField::None, None));
    select_and_resolve(&mut sut, raw_chain());
    assert_eq!(field(&sut), (NetRpcField::None, None), "still checking");

    // Compatible: an own endpoint may be preferred.
    let sut = checked_compatible();
    assert_eq!(
        field(&sut),
        (NetRpcField::Optional, Some(RPC_FIELD_OPTIONAL.to_owned()))
    );

    // No endpoint listed: the field is the one thing asked for.
    let mut sut = started();
    let mut bare = raw_chain();
    bare.rpc.clear();
    select_and_resolve(&mut sut, bare);
    assert_eq!(
        field(&sut),
        (NetRpcField::Required, Some(RPC_FIELD_REQUIRED.to_owned()))
    );

    // Could not reach a verdict — in the wizard, and on the scan path.
    let mut sut = started();
    select_and_resolve(&mut sut, raw_chain());
    sut.resolve(probe(RPC_SLOW, None, 0.0));
    sut.resolve(probe(RPC_FAST, None, 0.0));
    assert_eq!(field(&sut).0, NetRpcField::Optional, "unable to verify");
    assert_eq!(field(&scan_unreachable()).0, NetRpcField::Optional);

    // A refusal: no P-256 verifier. Another endpoint would say the same.
    let mut sut = started();
    select_and_resolve(&mut sut, raw_chain());
    resolve_race(&mut sut);
    for (_, address, _) in REQUIRED_CONTRACTS {
        sut.resolve(code_ok(RPC_FAST, address));
    }
    sut.resolve(Res::P256Call {
        url: RPC_FAST.to_owned(),
        result: Some("0x".to_owned()),
    });
    sut.resolve(code_result(RPC_FAST, P256_PRECOMPILE, Some("0x")));
    assert_eq!(field(&sut), (NetRpcField::None, None), "refused");

    // Already added, and not found: no chain to check.
    let mut sut = started_with(vec![custom_network(999)], vec![]);
    sut.dispatch(Event::ChainSelected {
        chain_id: 999,
        keep_custom_rpc: false,
    });
    assert_eq!(field(&sut), (NetRpcField::None, None));

    let i18n = vela_core::i18n::I18n::embedded().expect("embedded corpus");
    let opts = vela_core::i18n::Options::default();
    for key in [RPC_FIELD_OPTIONAL, RPC_FIELD_REQUIRED] {
        assert!(i18n.exists(key, &opts), "{key}");
    }
}

// ---------------------------------------------------------------------------
// "Only use my node" (owner decision 2026-10-28)
// ---------------------------------------------------------------------------

fn own_override(chain_id: u32, rpc: &str, only_mine: bool) -> NetNetworkConfig {
    NetNetworkConfig {
        chain_id,
        rpc_url: rpc.to_owned(),
        explorer_url: String::new(),
        bundler_url: String::new(),
        only_mine,
    }
}

fn row_of(sut: &Sut, chain_id: u32) -> vela_core::app::network_admin::NetNetworkRow {
    sut.view()
        .networks
        .into_iter()
        .find(|row| row.chain_id == chain_id)
        .expect("the network is listed")
}

/// Only a network whose saved RPC is the person's own is "theirs": a shipped
/// default saved back, or no override at all, is not.
#[test]
fn a_network_has_its_own_rpc_only_when_the_saved_rpc_is_not_the_shipped_one() {
    let gnosis = BUILTIN_CHAINS.iter().find(|c| c.chain_id == 100).unwrap();
    let sut = started_with(
        vec![custom_network(999)],
        vec![
            own_override(100, "https://my-node.example", false),
            own_override(1, BUILTIN_CHAINS[0].rpc_url, false),
            own_override(999, "https://mine.example", true),
        ],
    );
    assert!(row_of(&sut, 100).own_rpc);
    assert!(!row_of(&sut, 100).only_mine);
    assert_eq!(BUILTIN_CHAINS[0].chain_id, 1);
    assert!(
        !row_of(&sut, 1).own_rpc,
        "the shipped endpoint is not theirs"
    );
    assert!(!row_of(&sut, 10).own_rpc, "nothing saved");
    assert!(row_of(&sut, 999).own_rpc && row_of(&sut, 999).only_mine);
    assert_ne!(gnosis.rpc_url, "https://my-node.example");
}

/// The switch is saved with the network's config, and the chain's pool is
/// reloaded at once so its next read obeys it.
#[test]
fn only_mine_is_saved_with_the_network_config_and_reloads_its_pool() {
    let mut sut = started_with(
        vec![],
        vec![own_override(100, "https://my-node.example", false)],
    );
    let ops = sut.dispatch(Event::OnlyMineChanged {
        chain_id: 100,
        on: true,
    });
    assert_eq!(
        ops,
        vec![
            Op::WriteNetworkConfigs {
                configs: vec![own_override(100, "https://my-node.example", true)],
            },
            Op::InvalidatePools {
                chain_id: Some(100)
            },
        ]
    );
    assert!(row_of(&sut, 100).only_mine);
    // The same answer twice is no change.
    assert!(sut
        .dispatch(Event::OnlyMineChanged {
            chain_id: 100,
            on: true
        })
        .is_empty());
}

/// Without an RPC of the person's own there is no node to keep to: the switch
/// is not offered, and a stray event writes nothing.
#[test]
fn only_mine_without_an_own_rpc_changes_nothing() {
    let mut sut = started();
    assert!(sut
        .dispatch(Event::OnlyMineChanged {
            chain_id: 100,
            on: true
        })
        .is_empty());
    assert!(!row_of(&sut, 100).only_mine);
}

/// Editing the RPC keeps the switch: a new address for their node is still
/// their node.
#[test]
fn an_rpc_edit_keeps_only_mine() {
    let mut saved = own_override(100, "https://my-node.example", true);
    saved.explorer_url = ETH_SCAN.to_owned();
    let mut sut = started_with(vec![], vec![saved]);
    sut.dispatch(Event::OverrideExpanded { chain_id: 100 });
    answer_card_probes(&mut sut, "https://my-node.example", Some(100), ETH_SCAN);
    sut.dispatch(Event::OverrideFieldEdited {
        chain_id: 100,
        field: NetOverrideField::Rpc,
        value: "https://my-other-node.example".to_owned(),
    });
    answer_card_probes(
        &mut sut,
        "https://my-other-node.example",
        Some(100),
        ETH_SCAN,
    );
    let ops = sut.dispatch(Event::OverrideBlurred { chain_id: 100 });
    let Some(Op::WriteNetworkConfigs { configs }) = ops.first() else {
        panic!("expected the config write, got {ops:?}");
    };
    assert_eq!(configs[0].rpc_url, "https://my-other-node.example");
    assert!(configs[0].only_mine);
}

/// set-5: the wizard preselects the space from what the chain registry says
/// the chain is — SLIP-44's testnet coin type, or a test network's word in
/// its name — and the person's choice still wins.
#[test]
fn the_wizard_preselects_the_space_the_registry_names() {
    let space = |data: NetRawChainData| {
        let mut sut = started();
        select_and_resolve(&mut sut, data);
        sut.view().wizard.testnet
    };
    assert!(!space(raw_chain()), "a mainnet");
    assert!(space(NetRawChainData {
        slip44: Some(1),
        ..raw_chain()
    }));
    assert!(space(NetRawChainData {
        name: Some("Unichain Sepolia Testnet".to_owned()),
        ..raw_chain()
    }));
    assert!(space(NetRawChainData {
        name: Some("Tempo Testnet Moderato".to_owned()),
        short_name: Some("tempo-moderato".to_owned()),
        ..raw_chain()
    }));
    assert!(space(NetRawChainData {
        name: Some("Amoy".to_owned()),
        ..raw_chain()
    }));
    assert!(space(NetRawChainData {
        testnet: true,
        ..raw_chain()
    }));
    // A word inside another is not the word: "Contestnetwork" is no testnet.
    assert!(!space(NetRawChainData {
        name: Some("Contestnetwork".to_owned()),
        slip44: Some(60),
        ..raw_chain()
    }));

    // The person's choice wins over the registry's word…
    let mut sut = started();
    select_and_resolve(
        &mut sut,
        NetRawChainData {
            slip44: Some(1),
            ..raw_chain()
        },
    );
    assert!(sut.view().wizard.testnet);
    sut.dispatch(Event::WizardTestnetChosen { testnet: false });
    assert!(!sut.view().wizard.testnet);
    // …for that pick: picking a chain again asks the registry again.
    select_and_resolve(
        &mut sut,
        NetRawChainData {
            slip44: Some(1),
            ..raw_chain()
        },
    );
    assert!(sut.view().wizard.testnet);
}

#[test]
fn registry_says_testnet_reads_the_document() {
    assert!(!registry_says_testnet(&raw_chain()));
    assert!(registry_says_testnet(&NetRawChainData {
        slip44: Some(1),
        ..raw_chain()
    }));
    assert!(registry_says_testnet(&NetRawChainData {
        short_name: Some("arb-sepolia".to_owned()),
        ..raw_chain()
    }));
    assert!(!registry_says_testnet(&NetRawChainData::default()));
}

// ---------------------------------------------------------------------------
// The multisig service: one more endpoint, on a shell with multisig accounts
// ---------------------------------------------------------------------------

fn started_with_multisig(endpoints: NetStoredEndpoints) -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::Started);
    sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![],
        endpoints,
        provider_keys: NetProviderKeys::default(),
        multisig_service: true,
        shown_networks: None,
    });
    sut
}

/// A shell without multisigs never shows, probes or uses it.
#[test]
fn the_multisig_service_is_an_endpoint_only_where_multisigs_are() {
    let sut = started();
    assert!(sut
        .view()
        .endpoints
        .iter()
        .all(|e| e.field != NetEndpointField::GenesisService));
    let mut sut = started();
    let ops = sut.dispatch(Event::EndpointsOpened);
    assert!(!ops.iter().any(|op| matches!(
        op,
        Op::FetchServiceHealth {
            field: NetEndpointField::GenesisService,
            ..
        }
    )));

    // Nor saves it, whoever asks (an agent's `set_service_endpoint`).
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::GenesisService,
        value: "https://elsewhere.example".to_owned(),
    });
    let ops = sut.dispatch(Event::EndpointBlurred {
        field: NetEndpointField::GenesisService,
    });
    assert!(ops.is_empty());

    let sut = started_with_multisig(NetStoredEndpoints::default());
    let last = sut.view().endpoints.last().cloned().expect("five");
    assert_eq!(last.field, NetEndpointField::GenesisService);
    assert_eq!(last.value, DEFAULT_GENESIS_SERVICE_URL);
    assert_eq!(last.default_value, DEFAULT_GENESIS_SERVICE_URL);
}

/// Probed at `/api/health` like the others; its answer names the service and
/// carries no `status` — that is the service, not an invalid answer.
#[test]
fn the_multisig_service_is_probed_and_known_by_its_name() {
    let mut sut = started_with_multisig(NetStoredEndpoints {
        genesis_service_url: Some("http://127.0.0.1:8788/".to_owned()),
        ..Default::default()
    });
    assert_eq!(sut.view().endpoints[4].value, "http://127.0.0.1:8788/");
    let ops = sut.dispatch(Event::EndpointsOpened);
    assert!(ops.contains(&Op::FetchServiceHealth {
        field: NetEndpointField::GenesisService,
        base_url: "http://127.0.0.1:8788".to_owned(),
    }));
    sut.resolve(Res::ServiceHealth {
        field: NetEndpointField::GenesisService,
        body: NetHealthBody::Identity {
            service: Some("safe-genesis-registry".to_owned()),
            status: None,
        },
        latency_ms: 12.0,
    });
    assert!(matches!(
        sut.view().endpoints[4].health,
        NetServiceHealth::Ok { .. }
    ));
    // Something else answering there is not it.
    let mut sut = started_with_multisig(NetStoredEndpoints::default());
    sut.dispatch(Event::EndpointsOpened);
    sut.resolve(Res::ServiceHealth {
        field: NetEndpointField::GenesisService,
        body: NetHealthBody::Identity {
            service: Some("vela-relay".to_owned()),
            status: Some("ok".to_owned()),
        },
        latency_ms: 12.0,
    });
    assert!(matches!(
        sut.view().endpoints[4].health,
        NetServiceHealth::InvalidResponse { .. }
    ));
}

/// Saved on blur under the URL rule, and reset with the others.
#[test]
fn the_multisig_service_is_saved_refused_and_reset_as_the_others_are() {
    let mut sut = started_with_multisig(NetStoredEndpoints::default());
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::GenesisService,
        value: " http://127.0.0.1:8788 ".to_owned(),
    });
    let ops = sut.dispatch(Event::EndpointBlurred {
        field: NetEndpointField::GenesisService,
    });
    let written = ops
        .iter()
        .find_map(|op| match op {
            Op::WriteServiceEndpoints { endpoints } => Some(endpoints.clone()),
            _ => None,
        })
        .expect("written");
    assert_eq!(written.genesis_service_url, "http://127.0.0.1:8788");

    // Plain http to another computer is refused: nothing written.
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::GenesisService,
        value: "http://192.168.1.5:8788".to_owned(),
    });
    let ops = sut.dispatch(Event::EndpointBlurred {
        field: NetEndpointField::GenesisService,
    });
    assert!(!ops
        .iter()
        .any(|op| matches!(op, Op::WriteServiceEndpoints { .. })));
    assert!(sut.view().endpoints[4].refused);

    let ops = sut.dispatch(Event::ResetEndpointsToDefaults);
    let written = ops
        .iter()
        .find_map(|op| match op {
            Op::WriteServiceEndpoints { endpoints } => Some(endpoints.clone()),
            _ => None,
        })
        .expect("written");
    assert_eq!(written.genesis_service_url, DEFAULT_GENESIS_SERVICE_URL);
    assert_eq!(sut.view().endpoints[4].value, DEFAULT_GENESIS_SERVICE_URL);
}

/// A record written before the field existed reads as the default.
#[test]
fn a_record_without_the_multisig_service_reads_as_its_default() {
    let e: NetServiceEndpoints = serde_json::from_value(serde_json::json!({
        "ethereum_data_url": DEFAULT_ETHEREUM_DATA_URL,
        "passkey_index_url": DEFAULT_PASSKEY_INDEX_URL,
        "bundler_service_url": DEFAULT_BUNDLER_SERVICE_URL,
        "fiat_rates_url": DEFAULT_FIAT_RATES_URL
    }))
    .unwrap();
    assert_eq!(e.genesis_service_url, DEFAULT_GENESIS_SERVICE_URL);
    let stored: NetStoredEndpoints = serde_json::from_value(serde_json::json!({
        "ethereum_data_url": null, "passkey_index_url": null,
        "bundler_service_url": null, "fiat_rates_url": null
    }))
    .unwrap();
    assert_eq!(stored.genesis_service_url, None);
}

/// §41 (P2-8): a URL Vela made from a retired provider key is a third
/// party's endpoint, not the person's node: the network keeps it as its RPC,
/// but it is not "your own node" — "Only use my node" is not offered for it,
/// a switch turned on for it before is turned off and written so, and a
/// pressed switch changes nothing. A node the person entered stays theirs.
#[test]
fn a_retired_key_url_is_not_your_chosen_node() {
    use vela_core::app::network_admin::is_provider_key_url;
    let keys = provider_keys(Some(FAKE_ALCHEMY), None, None);
    let (mut sut, ops) = load_with_keys(vec![], vec![], keys);
    let configs = retired_configs(&ops).expect("configs");
    let migrated = configs.first().expect("one network served").clone();
    assert!(
        is_provider_key_url(&migrated.rpc_url),
        "{}",
        migrated.rpc_url
    );
    sut.resolve(written(true, true));
    sut.resolve(Res::Invalidated);
    let row = |sut: &Sut| {
        sut.view()
            .networks
            .into_iter()
            .find(|r| r.chain_id == migrated.chain_id)
            .expect("listed")
    };
    assert_eq!(row(&sut).rpc_url, migrated.rpc_url, "kept as its RPC");
    assert!(!row(&sut).own_rpc, "not the person's node");
    let ops = sut.dispatch(Event::OnlyMineChanged {
        chain_id: migrated.chain_id,
        on: true,
    });
    assert!(ops.is_empty(), "nothing to keep to: {ops:?}");

    // A switch turned on for it before this rule: off, written, pools read again.
    let mut before = migrated.clone();
    before.only_mine = true;
    let mut sut = Sut::new();
    sut.dispatch(Event::Started);
    let ops = sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![before],
        endpoints: NetStoredEndpoints::default(),
        provider_keys: NetProviderKeys::default(),
        multisig_service: false,
        shown_networks: None,
    });
    let written_off = ops.iter().any(|op| {
        matches!(op, Op::WriteNetworkConfigs { configs }
            if configs.iter().all(|c| !c.only_mine))
    });
    assert!(written_off, "{ops:?}");
    assert!(ops.iter().any(|op| matches!(
        op,
        Op::InvalidatePools { chain_id: Some(id) } if *id == migrated.chain_id
    )));
    assert!(!row(&sut).only_mine);

    // The URL shapes, and a node the person typed.
    for url in [
        "https://eth-mainnet.g.alchemy.com/v2/abc",
        "https://lb.drpc.org/ogrpc?network=ethereum&dkey=abc",
        "https://rpc.ankr.com/eth/abc",
    ] {
        assert!(is_provider_key_url(url), "{url}");
    }
    for url in [
        "https://my-node.example/rpc",
        "https://rpc.ankr.com/eth",
        "http://eth-mainnet.g.alchemy.com/v2/abc",
    ] {
        assert!(!is_provider_key_url(url), "{url}");
    }
}
