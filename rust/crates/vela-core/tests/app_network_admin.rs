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
    build_provider_rpc_url, clean_endpoint_value, default_endpoint, explorer_base_url,
    is_builtin_chain, is_code_deployed, is_localhost_http, p256_call_indicates_support,
    provider_chain_ids, rank_search, Event, NetChainIndexEntry, NetCustomNetwork, NetEndpointField,
    NetHealthBody, NetNetworkConfig, NetOperation as Op, NetOverrideField, NetProbeHealth,
    NetProviderId, NetProviderKeys, NetRawChainData, NetRpcFailureKind, NetServiceEndpoints,
    NetServiceHealth, NetShellResult as Res, NetStoredEndpoints, NetWizardErrorKind,
    NetWizardPhase, NetworkAdmin, BUILTIN_CHAINS, DEFAULT_BUNDLER_SERVICE_URL,
    DEFAULT_ETHEREUM_DATA_URL, DEFAULT_FIAT_RATES_URL, DEFAULT_PASSKEY_INDEX_URL, P256_PRECOMPILE,
    REQUIRED_CONTRACTS, SEARCH_DEBOUNCE_MS,
};

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
    assert_eq!(view.networks.len(), 25, "24 builtins + 1 custom");
    let custom = &view.networks[24];
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
    assert!(sut
        .dispatch(Event::ProviderKeyBlurred {
            provider: NetProviderId::Alchemy
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

    // The confirm is inert — nothing is written, nothing enters the ledger.
    assert!(sut
        .dispatch(Event::AddConfirmed {
            now_iso: NOW_ISO.to_owned()
        })
        .is_empty());
    assert_eq!(sut.view().networks.len(), 24);
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
    assert!(sut
        .dispatch(Event::AddConfirmed {
            now_iso: NOW_ISO.to_owned()
        })
        .is_empty());
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
    ] {
        let with = |value: &str| {
            let mut e = NetServiceEndpoints::default();
            let slot = match field {
                NetEndpointField::EthereumData => &mut e.ethereum_data_url,
                NetEndpointField::PasskeyIndex => &mut e.passkey_index_url,
                NetEndpointField::BundlerService => &mut e.bundler_service_url,
                NetEndpointField::FiatRates => &mut e.fiat_rates_url,
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
            }]
        }]
    );
    let view = sut.view();
    assert_eq!(view.last_added_chain_id, Some(NEW_CHAIN));
    assert_eq!(view.wizard.phase, NetWizardPhase::Idle, "reset + close");
    assert_eq!(view.networks.len(), 25);
    assert!(view.networks[24].is_custom);
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
    assert_eq!(sut.view().networks.len(), 24, "nothing saved");
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
        }],
        endpoints: NetStoredEndpoints {
            bundler_service_url: Some("https://my-relay.example".to_owned()),
            ..Default::default()
        },
        provider_keys: NetProviderKeys::default(),
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
        }],
        endpoints: NetStoredEndpoints {
            bundler_service_url: Some("https://my-relay.example".to_owned()),
            ..Default::default()
        },
        provider_keys: NetProviderKeys::default(),
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
    assert_eq!(sut.view().networks.len(), 24);
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

/// Health badges NEVER gate saves (current behavior, kept): even a URL the
/// badge will flag as not-HTTPS persists on blur.
#[test]
fn health_badges_never_gate_endpoint_saves() {
    let mut sut = started();
    sut.dispatch(Event::EndpointEdited {
        field: NetEndpointField::PasskeyIndex,
        value: "http://insecure.example".to_owned(),
    });
    let ops = sut.dispatch(Event::EndpointBlurred {
        field: NetEndpointField::PasskeyIndex,
    });
    let Some(Op::WriteServiceEndpoints { endpoints }) = ops.first() else {
        panic!("expected the endpoints write, got {ops:?}");
    };
    assert_eq!(endpoints.passkey_index_url, "http://insecure.example");
    assert_eq!(
        endpoint_view(&sut, NetEndpointField::PasskeyIndex),
        NetServiceHealth::NotHttps,
        "the badge complains — but the save went through"
    );
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
// Invariant ⑦ — provider keys: id match, removal, pool flush, staleness
// ===========================================================================

fn alchemy_url(chain_id: u32, key: &str) -> String {
    build_provider_rpc_url(NetProviderId::Alchemy, chain_id, key).expect("supported chain")
}

fn sut_with_alchemy_key() -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::Started);
    sut.resolve(Res::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![],
        endpoints: NetStoredEndpoints::default(),
        provider_keys: NetProviderKeys {
            alchemy: Some("abc".to_owned()),
            ..Default::default()
        },
    });
    sut
}

#[test]
fn opening_the_provider_modal_auto_tests_configured_providers() {
    let mut sut = sut_with_alchemy_key();
    let ops = sut.dispatch(Event::ProvidersOpened);
    let expected: Vec<Op> = provider_chain_ids(NetProviderId::Alchemy)
        .into_iter()
        .map(|cid| Op::ProbeRpc {
            url: alchemy_url(cid, "abc"),
        })
        .collect();
    assert_eq!(ops, expected, "one unified probe per supported chain");
    assert_eq!(
        ops.len(),
        23,
        "one probe per Alchemy-served built-in; XRPL EVM has no slug"
    );
}

/// Invariant ⑦ core: ok requires reported == target — a probe answering the
/// WRONG chain id is unavailable, never "supported and fast".
#[test]
fn a_provider_probe_must_report_the_target_chain_id() {
    let mut sut = sut_with_alchemy_key();
    sut.dispatch(Event::ProvidersOpened);
    // Chain 1 answers correctly; chain 56 answers chain 1 — wrong.
    sut.resolve(probe(&alchemy_url(1, "abc"), Some(1), 120.0));
    sut.resolve(probe(&alchemy_url(56, "abc"), Some(1), 80.0));
    let view = sut.view();
    let test = view.providers[0].test.as_ref().expect("test running");
    assert!(!test.done, "10 probes still outstanding");
    let eth = test.results.iter().find(|r| r.chain_id == 1).expect("row");
    let bnb = test.results.iter().find(|r| r.chain_id == 56).expect("row");
    assert!(eth.ok);
    assert_eq!(eth.latency_ms, 120.0);
    assert!(!bnb.ok, "wrong reported id ⇒ unavailable");
}

#[test]
fn clearing_a_key_removes_the_provider_entirely() {
    let mut sut = sut_with_alchemy_key();
    sut.dispatch(Event::ProvidersOpened);
    sut.dispatch(Event::ProviderKeyEdited {
        provider: NetProviderId::Alchemy,
        value: "   ".to_owned(),
    });
    let ops = sut.dispatch(Event::ProviderKeyBlurred {
        provider: NetProviderId::Alchemy,
    });
    assert_eq!(
        ops,
        vec![
            Op::WriteRpcProviders {
                keys: NetProviderKeys::default()
            },
            Op::InvalidatePools { chain_id: None },
        ],
        "the entry is DROPPED (not stored empty) + every pool flushes"
    );
    assert!(sut.view().providers[0].test.is_none());
    assert!(!sut.view().providers[0].has_key);
}

#[test]
fn a_key_blur_persists_trimmed_flushes_pools_and_tests() {
    let mut sut = started();
    sut.dispatch(Event::ProvidersOpened);
    sut.dispatch(Event::ProviderKeyEdited {
        provider: NetProviderId::Ankr,
        value: "  xyz  ".to_owned(),
    });
    let ops = sut.dispatch(Event::ProviderKeyBlurred {
        provider: NetProviderId::Ankr,
    });
    assert_eq!(
        ops[0],
        Op::WriteRpcProviders {
            keys: NetProviderKeys {
                ankr: Some("xyz".to_owned()),
                ..Default::default()
            }
        }
    );
    assert_eq!(ops[1], Op::InvalidatePools { chain_id: None });
    // Ankr serves 8 of the 12 chains — 8 probes.
    assert_eq!(ops.len(), 2 + 8);
    assert!(matches!(&ops[2], Op::ProbeRpc { url } if url == "https://rpc.ankr.com/eth/xyz"));
}

#[test]
fn editing_a_key_drops_stale_test_results_by_construction() {
    let mut sut = sut_with_alchemy_key();
    sut.dispatch(Event::ProvidersOpened);
    sut.resolve(probe(&alchemy_url(1, "abc"), Some(1), 120.0));
    assert!(sut.view().providers[0].test.is_some());
    // A keystroke — the old latencies must never show against the new key.
    sut.dispatch(Event::ProviderKeyEdited {
        provider: NetProviderId::Alchemy,
        value: "abcd".to_owned(),
    });
    assert!(sut.view().providers[0].test.is_none());
    // A late probe from the orphaned wave changes nothing.
    assert!(sut
        .resolve(probe(&alchemy_url(56, "abc"), Some(56), 60.0))
        .is_empty());
    assert!(sut.view().providers[0].test.is_none());
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
}

// ===========================================================================
// The unified probe vocabulary
// ===========================================================================

/// The inventory's "four eth_chainId probes become ONE operation word":
/// wizard candidates, override RPC health and provider tests all speak
/// `ProbeRpc`; nothing else probes a chain id.
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

    let mut sut = sut_with_alchemy_key();
    let provider_ops = sut.dispatch(Event::ProvidersOpened);
    assert!(provider_ops
        .iter()
        .all(|op| matches!(op, Op::ProbeRpc { .. })));
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

#[test]
fn provider_urls_follow_the_slug_maps() {
    assert_eq!(
        build_provider_rpc_url(NetProviderId::Alchemy, 1, "k").as_deref(),
        Some("https://eth-mainnet.g.alchemy.com/v2/k")
    );
    assert_eq!(
        build_provider_rpc_url(NetProviderId::Drpc, 8453, "k").as_deref(),
        Some("https://lb.drpc.org/ogrpc?network=base&dkey=k")
    );
    assert_eq!(
        build_provider_rpc_url(NetProviderId::Ankr, 100, "k").as_deref(),
        Some("https://rpc.ankr.com/gnosis/k")
    );
    // Ankr has no Tempo; nobody has a URL without a key.
    assert_eq!(build_provider_rpc_url(NetProviderId::Ankr, 4217, "k"), None);
    assert_eq!(build_provider_rpc_url(NetProviderId::Alchemy, 1, ""), None);
    // Canonical order, filtered per provider.
    assert_eq!(provider_chain_ids(NetProviderId::Alchemy).len(), 23);
    assert_eq!(provider_chain_ids(NetProviderId::Drpc).len(), 23);
    assert_eq!(
        provider_chain_ids(NetProviderId::Ankr),
        vec![1, 56, 137, 42161, 10, 8453, 43114, 100]
    );
}

/// Arc (spec 060) — the thirteenth built-in network. Every field here is load
/// bearing: the native symbol drives the $1 peg (and therefore the $0.01 fee
/// floor), and the RPC and explorer are what a person actually reaches.
#[test]
fn arc_is_a_builtin_network_with_the_fields_the_rest_of_the_wallet_reads() {
    assert_eq!(BUILTIN_CHAINS.len(), 24);
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

/// Arc testnet is reachable through the add-a-network flow, deliberately NOT
/// as a built-in (spec 060 scope decision).
#[test]
fn arc_testnet_is_not_a_builtin_network() {
    assert!(BUILTIN_CHAINS.iter().all(|c| c.chain_id != 5_042_002));
}

/// Provider slugs were verified against the live providers, not guessed: a
/// wrong slug enters the RPC pool at the highest tier and poisons every read.
#[test]
fn arc_provider_slugs_match_the_providers_that_actually_serve_it() {
    assert_eq!(
        build_provider_rpc_url(NetProviderId::Alchemy, 5_042, "key"),
        Some("https://arc-mainnet.g.alchemy.com/v2/key".to_owned())
    );
    assert_eq!(
        build_provider_rpc_url(NetProviderId::Drpc, 5_042, "key"),
        Some("https://lb.drpc.org/ogrpc?network=arc&dkey=key".to_owned())
    );
    // Ankr does not serve Arc, so it must not be offered for it.
    assert_eq!(
        build_provider_rpc_url(NetProviderId::Ankr, 5_042, "key"),
        None
    );
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
    // Arc TESTNET is added by hand, like any local or internal chain.
    assert!(!is_builtin_chain(5_042_002));
    assert!(
        !is_builtin_chain(31_337),
        "a local devnet is nobody else's to fund"
    );
    assert!(!is_builtin_chain(1_337));
}

/// The eleven admitted with spec 061. Each field is what a person reaches or
/// what a money rule reads; XRPL EVM is the one no provider serves.
#[test]
fn the_eleven_are_built_in_with_verified_provider_slugs() {
    let by_id = |id: u32| {
        BUILTIN_CHAINS
            .iter()
            .find(|c| c.chain_id == id)
            .expect("built in")
    };
    assert_eq!(by_id(988).native_symbol, "USDT0");
    assert_eq!(by_id(196).native_symbol, "OKB");
    assert_eq!(by_id(1_440_000).native_symbol, "XRP");
    assert_eq!(
        build_provider_rpc_url(NetProviderId::Alchemy, 196, "k"),
        Some("https://xlayer-mainnet.g.alchemy.com/v2/k".to_owned()),
        "X Layer's slug was dead data until the chain was built in"
    );
    assert_eq!(
        build_provider_rpc_url(NetProviderId::Alchemy, 1_440_000, "k"),
        None
    );
    assert_eq!(
        build_provider_rpc_url(NetProviderId::Drpc, 1_440_000, "k"),
        None
    );
}
