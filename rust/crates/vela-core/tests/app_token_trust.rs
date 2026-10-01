//! Rules of the token-trust machine, one test per rule.
//!
//! The two decision chains the held→trusted cascade rides on — log→transfer
//! acceptance (`transfer-monitor.ts:136-168`) and token→admission
//! (`token-autoadd.ts:32-86`) — are pinned with exhaustive/property-style
//! tests over the pure functions, then the machine tests drive the scan,
//! admission and simulation pipelines exactly the way the shell will:
//! dispatch an event, answer the operations one at a time. Inventory
//! invariants ①–⑧ each have at least one test named after the rule.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::token_trust::{
    address_topic, admission_allows, allowlist_for_chain, auto_add_candidates,
    decode_transfer_logs, derive_asset_deltas, judge_delta, known_token, Event, TokenTrust,
    TrustAssetDelta, TrustCustomToken, TrustDeltaKind, TrustLogsOutcome, TrustMetaEntry,
    TrustNetDelta, TrustOperation as Op, TrustRawLog, TrustReceiptLog, TrustShellResult as Res,
    TrustSimJudgment, TrustTokenMeta, DEFAULT_MONITOR_CHAINS, NATIVE_LOG_ADDRESSES, TRANSFER_TOPIC,
};

type Sut = DomainDriver<TokenTrust>;

const WALLET: &str = "0xAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAaAa";
const WALLET_LC: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PEER: &str = "0x1111111111111111111111111111111111111111";
const ATTACKER: &str = "0x2222222222222222222222222222222222222222";
const WRAPPED: &str = "0x4444444444444444444444444444444444444444";
const STABLE: &str = "0x5555555555555555555555555555555555555555";
const FRESH: &str = "0x6666666666666666666666666666666666666666";
const LISTED: &str = "0x7777777777777777777777777777777777777777";
const HELD: &str = "0x8888888888888888888888888888888888888888";
const T_SENT: &str = "0x9999999999999999999999999999999999999999";
const KNOWN_USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
const NATIVE_SENTINEL: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn word(v: u128) -> String {
    format!("0x{v:064x}")
}

fn raw_log(
    contract: &str,
    from: &str,
    to: &str,
    value: u128,
    tx: &str,
    block: u64,
    idx: u32,
) -> TrustRawLog {
    TrustRawLog {
        address: contract.to_owned(),
        topics: vec![
            TRANSFER_TOPIC.to_owned(),
            address_topic(from),
            address_topic(to),
        ],
        data: word(value),
        transaction_hash: tx.to_owned(),
        block_number: Some(format!("0x{block:x}")),
        log_index: Some(format!("0x{idx:x}")),
    }
}

fn receipt(contract: &str, from: &str, to: &str, value: u128) -> TrustReceiptLog {
    TrustReceiptLog {
        address: contract.to_owned(),
        topics: vec![
            TRANSFER_TOPIC.to_owned(),
            address_topic(from),
            address_topic(to),
        ],
        data: word(value),
    }
}

fn custom(chain_id: u32, addr: &str, symbol: &str) -> TrustCustomToken {
    TrustCustomToken {
        id: format!("{chain_id}_{}", addr.to_lowercase()),
        chain_id,
        contract_address: addr.to_owned(),
        symbol: symbol.to_owned(),
        name: symbol.to_owned(),
        decimals: 18,
    }
}

fn meta(symbol: &str, decimals: u32) -> TrustTokenMeta {
    TrustTokenMeta {
        symbol: symbol.to_owned(),
        decimals,
    }
}

fn meta_entry(addr: &str, m: Option<TrustTokenMeta>) -> TrustMetaEntry {
    TrustMetaEntry {
        addr: addr.to_owned(),
        meta: m,
    }
}

fn erc20_delta(token: &str, delta: &str) -> TrustAssetDelta {
    TrustAssetDelta {
        kind: TrustDeltaKind::Erc20,
        token: Some(token.to_owned()),
        delta: delta.to_owned(),
    }
}

fn native_delta(delta: &str) -> TrustAssetDelta {
    TrustAssetDelta {
        kind: TrustDeltaKind::Native,
        token: None,
        delta: delta.to_owned(),
    }
}

fn sentinels() -> Vec<String> {
    NATIVE_LOG_ADDRESSES
        .iter()
        .map(|s| (*s).to_owned())
        .collect()
}

/// A machine that has adopted `WALLET` with the given held chains.
fn booted(chain_ids: Vec<u32>) -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::HeldChainsSnapshot {
        address: WALLET.to_owned(),
        chain_ids,
    });
    sut
}

fn block_number(chain_id: u32, latest: u64) -> Res {
    Res::BlockNumber {
        address: WALLET_LC.to_owned(),
        chain_id,
        block_hex: Some(format!("0x{latest:x}")),
    }
}

fn logs_ok(chain_id: u32, logs: Vec<TrustRawLog>) -> Res {
    Res::Logs {
        address: WALLET_LC.to_owned(),
        chain_id,
        outcome: TrustLogsOutcome::Ok { logs },
    }
}

fn timestamp(chain_id: u32, block: u64, sec: Option<f64>) -> Res {
    Res::BlockTimestamp {
        address: WALLET_LC.to_owned(),
        chain_id,
        block_number: block as f64,
        timestamp_sec: sec,
        now_ms: 1_700_000_123_456.0,
    }
}

// ===========================================================================
// Decision chain 1 — log → transfer acceptance (pure, invariant ①)
// ===========================================================================

#[test]
fn address_topic_pads_and_lowercases() {
    assert_eq!(
        address_topic(WALLET),
        format!("0x{}{}", "0".repeat(24), &WALLET_LC[2..])
    );
    // The `slice(2)` is blind — a short input just yields the padding.
    assert_eq!(address_topic("0x"), format!("0x{}", "0".repeat(24)));
}

/// Invariant ① — the RPC's topic filter is never trusted: only a log whose
/// `topics[2]` is actually this wallet is accepted, so a malicious pool
/// endpoint cannot fabricate a "Received" row.
#[test]
fn decode_verifies_recipient_locally_never_trusting_the_rpc() {
    let good = raw_log(STABLE, PEER, WALLET, 5, "0xabc", 999, 1);
    // Recipient topic case-insensitivity: an uppercase topic still matches.
    let mut shouty = good.clone();
    shouty.topics[2] = shouty.topics[2].to_uppercase();
    shouty.log_index = Some("0x2".to_owned());
    // A foreign recipient the endpoint returned anyway — rejected locally.
    let foreign = raw_log(STABLE, PEER, ATTACKER, 5, "0xdef", 999, 3);
    // Structurally deficient logs.
    let mut two_topics = good.clone();
    two_topics.topics.truncate(2);
    let mut no_topics = good.clone();
    no_topics.topics.clear();

    let out = decode_transfer_logs(
        &[good.clone(), shouty, foreign, two_topics, no_topics],
        WALLET,
        1,
    );
    assert_eq!(out.len(), 2, "only the two logs actually addressed to us");
    assert!(out.iter().all(|t| t.token.as_deref() == Some(STABLE)));
    assert_eq!(out[0].id, "1-0xabc-1");
    assert_eq!(out[1].id, "1-0xabc-2");
    assert_eq!(out[0].value, 5);
    assert_eq!(out[0].block_number, 999);
}

#[test]
fn decode_skips_zero_value_and_malformed_logs() {
    let zero = raw_log(STABLE, PEER, WALLET, 0, "0x1", 999, 0);
    let mut empty_data = raw_log(STABLE, PEER, WALLET, 5, "0x2", 999, 1);
    empty_data.data = "0x".to_owned(); // reads as 0x0 → zero → skipped
    let mut garbage_data = raw_log(STABLE, PEER, WALLET, 5, "0x3", 999, 2);
    garbage_data.data = "0xzz".to_owned(); // BigInt throws → skipped
    let mut garbage_index = raw_log(STABLE, PEER, WALLET, 5, "0x4", 999, 3);
    garbage_index.log_index = Some("0xnope".to_owned()); // fail-closed
                                                         // A value beyond u128 — JS BigInt carries it, this core fails closed.
    let mut huge = raw_log(STABLE, PEER, WALLET, 5, "0x5", 999, 4);
    huge.data = format!("0x{}2{}", "0".repeat(31), "0".repeat(32)); // 2^129
    let good = raw_log(STABLE, PEER, WALLET, 7, "0x6", 999, 5);

    let out = decode_transfer_logs(
        &[zero, empty_data, garbage_data, garbage_index, huge, good],
        WALLET,
        1,
    );
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].value, 7);
}

#[test]
fn decode_flags_native_sentinels_and_lowercases_erc20() {
    let mut logs: Vec<TrustRawLog> = NATIVE_LOG_ADDRESSES
        .iter()
        .enumerate()
        .map(|(i, sentinel)| raw_log(sentinel, PEER, WALLET, 9, "0xn", 999, i as u32))
        .collect();
    logs.push(raw_log(
        &STABLE.to_uppercase().replace("0X", "0x"),
        PEER,
        WALLET,
        5,
        "0xe",
        999,
        7,
    ));
    let out = decode_transfer_logs(&logs, WALLET, 1);
    assert_eq!(out.len(), 4);
    for native in &out[..3] {
        assert!(native.is_native);
        assert_eq!(native.token, None);
    }
    assert!(!out[3].is_native);
    assert_eq!(out[3].token.as_deref(), Some(STABLE), "contract lowercased");
}

/// Ported verbatim quirks: the guard is `topics.length < 3`, so a four-topic
/// log with non-zero data passes; an ERC-721 transfer (four topics, empty
/// data) is excluded by its zero value; and the sender's case is preserved
/// (`'0x' + topics[1].slice(26)`).
#[test]
fn decode_quirks_are_ported_verbatim() {
    let mut four_topics = raw_log(STABLE, PEER, WALLET, 5, "0x1", 999, 0);
    four_topics.topics.push(word(1));
    let mut erc721 = raw_log(STABLE, PEER, WALLET, 0, "0x2", 999, 1);
    erc721.topics.push(word(7));
    erc721.data = "0x".to_owned();
    let mut shouty_sender = raw_log(STABLE, PEER, WALLET, 5, "0x3", 999, 2);
    shouty_sender.topics[1] = format!(
        "0x{}{}",
        "0".repeat(24),
        "ABCDEF0000000000000000000000000000000000"
    );

    let out = decode_transfer_logs(&[four_topics, erc721, shouty_sender], WALLET, 1);
    assert_eq!(out.len(), 2);
    assert_eq!(out[0].id, "1-0x1-0", "4 topics + value passes (< 3 guard)");
    assert_eq!(
        out[1].from, "0xABCDEF0000000000000000000000000000000000",
        "sender case preserved, verbatim"
    );
}

// ===========================================================================
// Receipt-log netting (pure) — the admission's input
// ===========================================================================

#[test]
fn derive_asset_deltas_nets_per_asset_and_keeps_first_seen_order() {
    let logs = vec![
        receipt(FRESH, PEER, WALLET, 100),
        receipt(FRESH, WALLET, PEER, 30),          // nets to +70
        receipt(T_SENT, WALLET, PEER, 5),          // net outflow
        receipt(STABLE, WALLET, WALLET, 50),       // self-transfer → cancels to 0
        receipt(NATIVE_SENTINEL, PEER, WALLET, 9), // native inflow
        receipt(LISTED, ATTACKER, PEER, 42),       // not our wallet → ignored
    ];
    let out = derive_asset_deltas(&logs, WALLET);
    assert_eq!(
        out,
        vec![
            TrustNetDelta {
                is_native: false,
                token: Some(FRESH.to_owned()),
                delta: 70
            },
            TrustNetDelta {
                is_native: false,
                token: Some(T_SENT.to_owned()),
                delta: -5
            },
            TrustNetDelta {
                is_native: true,
                token: None,
                delta: 9
            },
        ]
    );
}

/// Unlike the getLogs decoder, the netting rule is strict: exactly three
/// topics and the Transfer signature (`sim-assets.ts:139-142`).
#[test]
fn derive_asset_deltas_only_counts_canonical_fungible_transfers() {
    let mut four_topics = receipt(FRESH, PEER, WALLET, 100);
    four_topics.topics.push(word(1));
    let mut wrong_sig = receipt(FRESH, PEER, WALLET, 100);
    wrong_sig.topics[0] = word(0);
    let mut garbage_value = receipt(FRESH, PEER, WALLET, 100);
    garbage_value.data = "0xzz".to_owned(); // BigInt throws → 0n → skipped
    assert!(derive_asset_deltas(&[four_topics, wrong_sig, garbage_value], WALLET).is_empty());
}

/// JS BigInt is unbounded; this core fails closed — an amount beyond i128
/// poisons that asset's netting and it is dropped, never mis-signed.
#[test]
fn derive_asset_deltas_overflow_poisons_the_asset_fail_closed() {
    let mut huge = receipt(FRESH, PEER, WALLET, 1);
    huge.data = format!("0x{}2{}", "0".repeat(31), "0".repeat(32)); // 2^129
    let fine = receipt(STABLE, PEER, WALLET, 3);
    let out = derive_asset_deltas(&[huge, fine], WALLET);
    assert_eq!(out.len(), 1, "poisoned asset dropped, sibling unaffected");
    assert_eq!(out[0].token.as_deref(), Some(STABLE));
}

#[test]
fn auto_add_candidates_are_net_received_erc20_only() {
    let logs = vec![
        receipt(FRESH, PEER, WALLET, 100),
        receipt(FRESH, PEER, WALLET, 1), // same token again → still one candidate
        receipt(T_SENT, WALLET, PEER, 5), // outflow → never a candidate
        receipt(STABLE, PEER, WALLET, 10),
        receipt(STABLE, WALLET, PEER, 10), // wash → net 0 → not "received"
        receipt(NATIVE_SENTINEL, PEER, WALLET, 9), // native → not an ERC-20
    ];
    assert_eq!(auto_add_candidates(&logs, WALLET), vec![FRESH.to_owned()]);
}

// ===========================================================================
// Decision chain 2 — token → admission (pure, invariants ⑤⑧)
// ===========================================================================

/// The admission predicate, exhaustively: net-received ∧ ¬listed ∧ ¬held ∧
/// ¬known ∧ symbol-resolved — and nothing else — admits.
#[test]
fn admission_gate_is_exhaustive() {
    for net in [true, false] {
        for listed in [true, false] {
            for held in [true, false] {
                for known in [true, false] {
                    for symbol in [None, Some(""), Some("TOK")] {
                        let expected = net
                            && !listed
                            && !held
                            && !known
                            && symbol.is_some_and(|s: &str| !s.is_empty());
                        assert_eq!(
                            admission_allows(net, listed, held, known, symbol),
                            expected,
                            "net={net} listed={listed} held={held} known={known} symbol={symbol:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn known_token_table_matches_the_ts_canon() {
    assert_eq!(known_token(KNOWN_USDC), Some(meta("USDC", 6)));
    assert_eq!(
        known_token(&KNOWN_USDC.to_uppercase().replace("0X", "0x")),
        Some(meta("USDC", 6)),
        "case-insensitive"
    );
    // The Arbitrum USDT entry the drifted clear_signing copy lacks.
    assert_eq!(
        known_token("0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9"),
        Some(meta("USDT", 6))
    );
    assert_eq!(known_token(FRESH), None);
}

// ===========================================================================
// The allowlist (pure, invariant ②)
// ===========================================================================

#[test]
fn allowlist_is_sentinels_plus_stables_plus_own_customs_only() {
    let stables = vec![STABLE.to_owned(), STABLE.to_owned()]; // registry dupe
    let customs = vec![
        custom(1, &LISTED.to_uppercase().replace("0X", "0x"), "MINE"),
        custom(137, FRESH, "ELSEWHERE"), // другая chain → excluded
    ];
    let mut expected = sentinels();
    expected.push(STABLE.to_owned());
    expected.push(LISTED.to_owned());
    assert_eq!(allowlist_for_chain(&stables, &customs, 1), expected);
    // Held-but-never-added tokens are NOT inputs to this function at all —
    // listening to them is exactly how spam slips in (invariant ②).
}

// ===========================================================================
// Asymmetric sim judgment (pure, invariants ⑥⑦)
// ===========================================================================

/// The (received, meta, trusted) grid: SENT renders whenever metadata
/// resolved; RECEIVED needs the trusted set; no metadata is always
/// unverified — and unknown never becomes a default.
#[test]
fn judge_delta_asymmetric_grid_is_exhaustive() {
    let m = meta("TOK", 6);
    for (delta, received) in [("-50", false), ("0", false), ("50", true)] {
        for meta_case in [None, Some(&m)] {
            for trusted in [true, false] {
                let d = erc20_delta(FRESH, delta);
                let expected_trustworthy = meta_case.is_some() && (!received || trusted);
                let verdict = judge_delta(&d, meta_case, trusted);
                if expected_trustworthy {
                    assert_eq!(
                        verdict,
                        TrustSimJudgment::Erc20Trusted {
                            token: FRESH.to_owned(),
                            delta: delta.to_owned(),
                            symbol: "TOK".to_owned(),
                            decimals: 6,
                            // FRESH is in no table: the set is all it has.
                            in_trusted_set: trusted,
                        },
                        "delta={delta} meta={} trusted={trusted}",
                        meta_case.is_some()
                    );
                } else {
                    assert_eq!(
                        verdict,
                        TrustSimJudgment::Erc20Unverified {
                            token: Some(FRESH.to_owned()),
                            delta: delta.to_owned(),
                        },
                        "delta={delta} meta={} trusted={trusted}",
                        meta_case.is_some()
                    );
                }
            }
        }
    }
}

#[test]
fn judge_delta_edges_fail_toward_unverified() {
    let m = meta("TOK", 6);
    // A curated known token is trusted even outside the passed set.
    assert_eq!(
        judge_delta(
            &erc20_delta(KNOWN_USDC, "50"),
            Some(&meta("USDC", 6)),
            false
        ),
        TrustSimJudgment::Erc20Trusted {
            token: KNOWN_USDC.to_owned(),
            delta: "50".to_owned(),
            symbol: "USDC".to_owned(),
            decimals: 6,
            in_trusted_set: true,
        }
    );
    // No token address → unverified even with metadata in hand.
    let no_token = TrustAssetDelta {
        kind: TrustDeltaKind::Erc20,
        token: None,
        delta: "50".to_owned(),
    };
    assert_eq!(
        judge_delta(&no_token, Some(&m), true),
        TrustSimJudgment::Erc20Unverified {
            token: None,
            delta: "50".to_owned()
        }
    );
    // A garbled delta (no JS counterpart — bigints can't be malformed) and an
    // empty symbol both fail closed.
    for garbage in ["", "abc", "--1", "1.5"] {
        assert!(matches!(
            judge_delta(&erc20_delta(FRESH, garbage), Some(&m), true),
            TrustSimJudgment::Erc20Unverified { .. }
        ));
    }
    assert!(matches!(
        judge_delta(&erc20_delta(FRESH, "-1"), Some(&meta("", 6)), true),
        TrustSimJudgment::Erc20Unverified { .. }
    ));
    // ERC-20 `decimals` is a uint8: a token claiming more has no readable
    // amount — caution and direction, never a figure at a lying width (the
    // sheet's formatter has none to give, 082 review) and never a hidden row.
    for lying in [256, u32::MAX] {
        assert!(matches!(
            judge_delta(&erc20_delta(FRESH, "-1"), Some(&meta("TOK", lying)), true),
            TrustSimJudgment::Erc20Unverified { .. }
        ));
    }
    assert!(matches!(
        judge_delta(&erc20_delta(FRESH, "-1"), Some(&meta("TOK", 255)), true),
        TrustSimJudgment::Erc20Trusted { decimals: 255, .. }
    ));
    // Native always passes through — naming is the shell's.
    assert_eq!(
        judge_delta(&native_delta("-42"), None, false),
        TrustSimJudgment::Native {
            delta: "-42".to_owned()
        }
    );
}

/// The cascade property: a token outside held ∪ registry ∪ known ∪ customs
/// is invisible to every trusting surface at once — not in the getLogs
/// allowlist, and its received amount never renders confidently.
#[test]
fn untrusted_tokens_are_invisible_to_every_surface() {
    let strangers = [
        "0x0101010101010101010101010101010101010101",
        "0x0202020202020202020202020202020202020202",
        "0xf3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3f3",
    ];
    let allow = allowlist_for_chain(&[STABLE.to_owned()], &[custom(1, LISTED, "MINE")], 1);
    for stranger in strangers {
        assert!(!allow.contains(&stranger.to_owned()), "② not watched");
        assert!(
            matches!(
                judge_delta(
                    &erc20_delta(stranger, "1000000"),
                    Some(&meta("SCAM", 6)),
                    false
                ),
                TrustSimJudgment::Erc20Unverified { .. }
            ),
            "⑥ received amount never rendered confidently"
        );
        assert!(known_token(stranger).is_none());
    }
}

// ===========================================================================
// Machine — the scan pipeline
// ===========================================================================

/// Invariant ② machine-side: the poll reads customs fresh, then queries each
/// held chain with exactly its allowlist — sentinels + registry stables +
/// that chain's customs.
#[test]
fn poll_scans_each_held_chain_with_its_allowlist() {
    let mut sut = booted(vec![1, 1, 137]); // duplicate chain de-duped
    sut.dispatch(Event::RegistryTokensSnapshot {
        chain_id: 1,
        stables: vec![STABLE.to_owned()],
        wrapped_native: Some(WRAPPED.to_owned()),
    });
    let ops = sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    assert_eq!(ops, vec![Op::ReadCustomTokens]);
    assert!(sut.view().scanning);

    let ops = sut.resolve(Res::CustomTokens {
        tokens: Some(vec![custom(1, LISTED, "MINE"), custom(137, FRESH, "POLY")]),
    });
    assert_eq!(
        ops,
        vec![
            Op::RpcBlockNumber {
                address: WALLET_LC.to_owned(),
                chain_id: 1
            },
            Op::RpcBlockNumber {
                address: WALLET_LC.to_owned(),
                chain_id: 137
            },
        ]
    );

    let ops = sut.resolve(block_number(1, 1000));
    let mut chain1_contracts = sentinels();
    chain1_contracts.push(STABLE.to_owned());
    chain1_contracts.push(LISTED.to_owned());
    assert_eq!(
        ops,
        vec![Op::RpcGetLogs {
            address: WALLET_LC.to_owned(),
            chain_id: 1,
            from_block: "0x384".to_owned(), // 1000 - 100 (the +1-block quirk)
            to_block: "0x3e8".to_owned(),
            recipient_topic: address_topic(WALLET),
            contracts: chain1_contracts,
        }]
    );

    // Chain 137's registry is cold: sentinels + its own custom only. The
    // wrapped native is trust-set vocabulary, never an allowlist entry.
    let ops = sut.resolve(block_number(137, 500));
    let mut chain137_contracts = sentinels();
    chain137_contracts.push(FRESH.to_owned());
    match &ops[0] {
        Op::RpcGetLogs {
            chain_id,
            contracts,
            ..
        } => {
            assert_eq!(*chain_id, 137);
            assert_eq!(contracts, &chain137_contracts);
        }
        other => panic!("expected getLogs, got {other:?}"),
    }
}

#[test]
fn brand_new_wallet_polls_the_default_payment_chains() {
    let mut sut = booted(vec![]);
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    let ops = sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    let chains: Vec<u32> = ops
        .iter()
        .map(|op| match op {
            Op::RpcBlockNumber { chain_id, .. } => *chain_id,
            other => panic!("expected blockNumber, got {other:?}"),
        })
        .collect();
    assert_eq!(chains, DEFAULT_MONITOR_CHAINS.to_vec());
}

/// Invariant ① machine-side: a malicious/caching endpoint returning someone
/// else's transfer (despite the topic filter) never reaches the feed.
#[test]
fn fake_recipient_from_malicious_endpoint_never_reaches_feed() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(block_number(1, 1000));
    let ops = sut.resolve(logs_ok(
        1,
        vec![
            raw_log(NATIVE_SENTINEL, PEER, ATTACKER, 1_000_000, "0xbad", 999, 0),
            raw_log(NATIVE_SENTINEL, PEER, WALLET, 5, "0xgood", 999, 1),
        ],
    ));
    assert_eq!(ops.len(), 1, "one distinct block to timestamp");
    sut.resolve(timestamp(1, 999, Some(1_700_000_000.0)));

    let view = sut.view();
    assert!(!view.scanning);
    assert_eq!(
        view.incoming.len(),
        1,
        "the attacker's log died at local re-verification"
    );
    assert_eq!(view.incoming[0].id, "1-0xgood-1");
    assert_eq!(view.incoming[0].value, "5");
    assert!(view.incoming[0].is_native);
    assert_eq!(view.incoming[0].timestamp_sec, 1_700_000_000.0);
}

/// Invariant ④ — a span-capped endpoint gets ONE conservative retry, never a
/// chunked fan-out; a second cap ends the chain for this tick.
#[test]
fn range_cap_retries_once_and_never_fans_out() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(block_number(1, 1000));
    let ops = sut.resolve(Res::Logs {
        address: WALLET_LC.to_owned(),
        chain_id: 1,
        outcome: TrustLogsOutcome::RangeCapped { cap: 50 },
    });
    match &ops[0] {
        Op::RpcGetLogs {
            from_block,
            to_block,
            ..
        } => {
            assert_eq!(
                from_block, "0x3b7",
                "latest - (cap - 1) = 951: exactly 50 blocks"
            );
            assert_eq!(to_block, "0x3e8");
        }
        other => panic!("expected the capped retry, got {other:?}"),
    }
    let ops = sut.resolve(Res::Logs {
        address: WALLET_LC.to_owned(),
        chain_id: 1,
        outcome: TrustLogsOutcome::RangeCapped { cap: 50 },
    });
    assert!(
        ops.is_empty(),
        "a second cap must NOT produce a third getLogs"
    );
    assert!(!sut.view().scanning);
    assert!(sut.outstanding().is_empty());

    // A cap with no parsable number stays conservative: 100 blocks.
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(block_number(1, 1000));
    let ops = sut.resolve(Res::Logs {
        address: WALLET_LC.to_owned(),
        chain_id: 1,
        outcome: TrustLogsOutcome::RangeCapped { cap: 0 },
    });
    match &ops[0] {
        Op::RpcGetLogs { from_block, .. } => assert_eq!(from_block, "0x385"), // 901
        other => panic!("expected the capped retry, got {other:?}"),
    }
}

#[test]
fn failing_chain_yields_nothing_this_tick() {
    // A hard getLogs failure.
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(block_number(1, 1000));
    let ops = sut.resolve(Res::Logs {
        address: WALLET_LC.to_owned(),
        chain_id: 1,
        outcome: TrustLogsOutcome::Failed,
    });
    assert!(ops.is_empty());
    assert!(!sut.view().scanning);
    assert!(sut.view().incoming.is_empty());

    // An unreadable block number never even reaches getLogs.
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    let ops = sut.resolve(Res::BlockNumber {
        address: WALLET_LC.to_owned(),
        chain_id: 1,
        block_hex: None,
    });
    assert!(ops.is_empty());
    assert!(!sut.view().scanning);

    // `latest <= 0` is invalid, exactly as `hexToNumber` + the guard has it.
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    let ops = sut.resolve(block_number(1, 0));
    assert!(ops.is_empty());
    assert!(!sut.view().scanning);
}

#[test]
fn timestamps_come_from_blocks_with_now_fallback() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(block_number(1, 1000));
    let ops = sut.resolve(logs_ok(
        1,
        vec![
            raw_log(NATIVE_SENTINEL, PEER, WALLET, 5, "0xa", 999, 1),
            raw_log(NATIVE_SENTINEL, PEER, WALLET, 6, "0xb", 998, 0),
        ],
    ));
    assert_eq!(
        ops,
        vec![
            Op::RpcGetBlockByNumber {
                address: WALLET_LC.to_owned(),
                chain_id: 1,
                block: "0x3e7".to_owned()
            },
            Op::RpcGetBlockByNumber {
                address: WALLET_LC.to_owned(),
                chain_id: 1,
                block: "0x3e6".to_owned()
            },
        ]
    );
    sut.resolve(timestamp(1, 999, Some(1_700_000_000.0)));
    sut.resolve(timestamp(1, 998, None)); // header lookup failed → now
    let view = sut.view();
    assert_eq!(view.incoming[0].timestamp_sec, 1_700_000_000.0);
    assert_eq!(
        view.incoming[1].timestamp_sec, 1_700_000_123.0,
        "floor(now_ms / 1000) fallback, clock from the shell's result"
    );
}

/// Invariant ③ — an ERC-20 whose metadata can't resolve is withheld from the
/// feed (persisting it would show "+0 tokens"); native rows are unaffected.
/// The dud is memoised for the session and not re-queried.
#[test]
fn unresolvable_metadata_withholds_the_erc20_but_not_native() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::RegistryTokensSnapshot {
        chain_id: 1,
        stables: vec![STABLE.to_owned()],
        wrapped_native: None,
    });
    let logs = vec![
        raw_log(STABLE, PEER, WALLET, 1_230_000, "0xa", 999, 0),
        raw_log(NATIVE_SENTINEL, PEER, WALLET, 5, "0xa", 999, 1),
    ];
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(block_number(1, 1000));
    sut.resolve(logs_ok(1, logs.clone()));
    let ops = sut.resolve(timestamp(1, 999, Some(1_700_000_000.0)));
    assert_eq!(
        ops,
        vec![Op::MulticallErc20Meta {
            chain_id: 1,
            addrs: vec![STABLE.to_owned()]
        }]
    );
    sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(STABLE, None)],
    });
    let view = sut.view();
    assert_eq!(
        view.incoming.len(),
        1,
        "the unresolvable stable is withheld"
    );
    assert!(view.incoming[0].is_native);

    // Next poll, same window: the negative memo answers — no new multicall,
    // and the token stays withheld rather than gaining invented decimals.
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(block_number(1, 1000));
    let ops = sut.resolve(logs_ok(1, logs));
    sut.resolve(timestamp(1, 999, Some(1_700_000_000.0)));
    assert!(
        !sut.outstanding()
            .iter()
            .any(|op| matches!(op, Op::MulticallErc20Meta { .. })),
        "session negative memo — no re-query (ops after logs: {ops:?})"
    );
    assert_eq!(sut.view().incoming.len(), 1);
}

#[test]
fn resolved_metadata_admits_the_erc20_with_symbol_and_decimals() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::RegistryTokensSnapshot {
        chain_id: 1,
        stables: vec![STABLE.to_owned()],
        wrapped_native: None,
    });
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(block_number(1, 1000));
    sut.resolve(logs_ok(
        1,
        vec![raw_log(STABLE, PEER, WALLET, 1_230_000, "0xa", 999, 0)],
    ));
    sut.resolve(timestamp(1, 999, Some(1_700_000_000.0)));
    sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(STABLE, Some(meta("USDX", 6)))],
    });
    let view = sut.view();
    assert_eq!(view.incoming.len(), 1);
    let row = &view.incoming[0];
    assert_eq!(row.value, "1230000", "raw amount as a decimal string");
    assert_eq!(row.symbol.as_deref(), Some("USDX"));
    assert_eq!(row.decimals, Some(6));
    assert_eq!(row.token.as_deref(), Some(STABLE));
    assert_eq!(row.from, PEER, "sender recovered from topics[1]");
}

#[test]
fn feed_sorts_newest_first_and_dedupes_across_overlapping_polls() {
    let mut sut = booted(vec![1]);
    let older = raw_log(NATIVE_SENTINEL, PEER, WALLET, 1, "0xt1", 998, 0);
    let newer_low = raw_log(NATIVE_SENTINEL, PEER, WALLET, 2, "0xt2", 999, 1);
    let newer_high = raw_log(NATIVE_SENTINEL, PEER, WALLET, 3, "0xt2", 999, 2);

    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(block_number(1, 1000));
    sut.resolve(logs_ok(
        1,
        vec![older.clone(), newer_low.clone(), newer_high.clone()],
    ));
    sut.resolve(timestamp(1, 998, Some(1.0)));
    sut.resolve(timestamp(1, 999, Some(2.0)));
    let view = sut.view();
    let ids: Vec<&str> = view.incoming.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["1-0xt2-2", "1-0xt2-1", "1-0xt1-0"]);

    // The next poll's window overlaps and returns one of them again — the
    // stable id de-dupes, exactly why there is no checkpoint to maintain.
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(block_number(1, 1001));
    sut.resolve(logs_ok(1, vec![newer_high]));
    sut.resolve(timestamp(1, 999, Some(2.0)));
    assert_eq!(
        sut.view().incoming.len(),
        3,
        "no duplicates from window overlap"
    );
}

/// The resident-machine rule: results are dropped by construction when the
/// account changed — including a switch away and back to the same address.
#[test]
fn account_switch_drops_stale_scan_results_by_construction() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    // Switch accounts while the block-number probe is in flight.
    sut.dispatch(Event::HeldChainsSnapshot {
        address: OTHER.to_owned(),
        chain_ids: vec![1],
    });
    let ops = sut.resolve(block_number(1, 1000));
    assert!(
        ops.is_empty(),
        "old-account block number must start nothing"
    );
    assert!(!sut.view().scanning);
    assert!(sut.view().incoming.is_empty());

    // Away and back: same address, but the attempt generation moved on.
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.dispatch(Event::HeldChainsSnapshot {
        address: OTHER.to_owned(),
        chain_ids: vec![],
    });
    sut.dispatch(Event::HeldChainsSnapshot {
        address: WALLET.to_owned(),
        chain_ids: vec![1],
    });
    let ops = sut.resolve(block_number(1, 1000));
    assert!(
        ops.is_empty(),
        "an address tag alone is not enough — the attempt catches this"
    );
}

#[test]
fn poll_is_single_flight() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::PollRequested {
        address: WALLET.to_owned(),
    });
    assert!(
        sut.dispatch(Event::PollRequested {
            address: WALLET.to_owned()
        })
        .is_empty(),
        "ignored while reading customs"
    );
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    assert!(
        sut.dispatch(Event::PollRequested {
            address: WALLET.to_owned()
        })
        .is_empty(),
        "ignored while chains are scanning"
    );
}

// ===========================================================================
// Machine — asymmetric simulation trust (invariants ⑤⑥⑦)
// ===========================================================================

/// Invariant ⑥ — SENT renders on metadata alone; RECEIVED renders only for
/// registry stables, the wrapped native, held tokens, or curated knowns.
#[test]
fn sim_sent_is_trusted_received_needs_the_trusted_set() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::RegistryTokensSnapshot {
        chain_id: 1,
        stables: vec![STABLE.to_owned()],
        wrapped_native: Some(WRAPPED.to_owned()),
    });
    sut.dispatch(Event::HeldTokensSnapshot {
        address: WALLET.to_owned(),
        chain_id: 1,
        tokens: vec![HELD.to_owned()],
    });
    let ops = sut.dispatch(Event::SimDeltasComputed {
        address: WALLET.to_owned(),
        chain_id: 1,
        deltas: vec![
            native_delta("100"),
            erc20_delta(T_SENT, "-50"),
            erc20_delta(FRESH, "50"),
            erc20_delta(HELD, "7"),
            erc20_delta(STABLE, "3"),
            erc20_delta(WRAPPED, "1"),
            erc20_delta(KNOWN_USDC, "9"),
        ],
    });
    // Known-table tokens skip resolution; the rest are fetched once, sorted.
    assert_eq!(
        ops,
        vec![Op::MulticallErc20Meta {
            chain_id: 1,
            addrs: vec![
                WRAPPED.to_owned(),
                STABLE.to_owned(),
                FRESH.to_owned(),
                HELD.to_owned(),
                T_SENT.to_owned(),
            ],
        }]
    );
    let sim = sut.view().sim.expect("session rendered while resolving");
    assert!(!sim.ready);

    sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![
            meta_entry(WRAPPED, Some(meta("WETH", 18))),
            meta_entry(STABLE, Some(meta("USDX", 6))),
            meta_entry(FRESH, Some(meta("SCAM", 18))),
            meta_entry(HELD, Some(meta("MINE", 18))),
            meta_entry(T_SENT, Some(meta("OUT", 18))),
        ],
    });
    let sim = sut.view().sim.expect("judged");
    assert!(sim.ready);
    assert_eq!(sim.address, WALLET_LC);
    assert_eq!(
        sim.judgments,
        vec![
            TrustSimJudgment::Native {
                delta: "100".to_owned()
            },
            TrustSimJudgment::Erc20Trusted {
                token: T_SENT.to_owned(),
                delta: "-50".to_owned(),
                symbol: "OUT".to_owned(),
                decimals: 18,
                // Rendered on metadata alone: an outflow of a coin the
                // wallet does not know says so (083 F1 review).
                in_trusted_set: false,
            },
            // The attacker-shaped case: a fake gain with a resolvable symbol
            // STILL renders unverified — trust, not just availability.
            TrustSimJudgment::Erc20Unverified {
                token: Some(FRESH.to_owned()),
                delta: "50".to_owned(),
            },
            TrustSimJudgment::Erc20Trusted {
                token: HELD.to_owned(),
                delta: "7".to_owned(),
                symbol: "MINE".to_owned(),
                decimals: 18,
                in_trusted_set: true,
            },
            TrustSimJudgment::Erc20Trusted {
                token: STABLE.to_owned(),
                delta: "3".to_owned(),
                symbol: "USDX".to_owned(),
                decimals: 6,
                in_trusted_set: true,
            },
            TrustSimJudgment::Erc20Trusted {
                token: WRAPPED.to_owned(),
                delta: "1".to_owned(),
                symbol: "WETH".to_owned(),
                decimals: 18,
                in_trusted_set: true,
            },
            TrustSimJudgment::Erc20Trusted {
                token: KNOWN_USDC.to_owned(),
                delta: "9".to_owned(),
                symbol: "USDC".to_owned(),
                decimals: 6,
                in_trusted_set: true,
            },
        ]
    );
}

/// Invariant ⑤ — the simulation path can produce NO write: a hostile dApp's
/// fake `Transfer(_, you, big)` with a working `symbol()` judges, renders,
/// and never touches storage.
#[test]
fn sim_path_never_writes_a_token() {
    let mut sut = booted(vec![1]);
    let mut everything: Vec<Op> = Vec::new();
    everything.extend(sut.dispatch(Event::SimDeltasComputed {
        address: WALLET.to_owned(),
        chain_id: 1,
        deltas: vec![erc20_delta(FRESH, "1000000000000000000000000")],
    }));
    everything.extend(sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(FRESH, Some(meta("USDC", 6)))], // spoofed symbol!
    }));
    assert!(
        everything.iter().all(|op| !matches!(
            op,
            Op::WriteCustomToken { .. } | Op::InvalidateTokenCache { .. }
        )),
        "the sim pipeline reached storage: {everything:?}"
    );
    assert!(sut.outstanding().is_empty());
    // And the spoofed gain renders unverified (⑥).
    assert!(matches!(
        sut.view().sim.expect("judged").judgments[0],
        TrustSimJudgment::Erc20Unverified { .. }
    ));
}

/// Invariant ⑦ — unknown metadata is "no information": the judgment falls to
/// unverified (never an invented default), the dud is memoised, and a second
/// look costs no second query.
#[test]
fn sim_unknown_metadata_is_unverified_and_memoised() {
    let mut sut = booted(vec![1]);
    let ops = sut.dispatch(Event::SimDeltasComputed {
        address: WALLET.to_owned(),
        chain_id: 1,
        deltas: vec![erc20_delta(FRESH, "-5")],
    });
    assert_eq!(ops.len(), 1);
    sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(FRESH, None)],
    });
    let sim = sut.view().sim.expect("judged");
    assert!(sim.ready);
    // Even the SENT side is unverified without metadata — no 18-decimals guess.
    assert_eq!(
        sim.judgments,
        vec![TrustSimJudgment::Erc20Unverified {
            token: Some(FRESH.to_owned()),
            delta: "-5".to_owned(),
        }]
    );

    let ops = sut.dispatch(Event::SimDeltasComputed {
        address: WALLET.to_owned(),
        chain_id: 1,
        deltas: vec![erc20_delta(FRESH, "-5")],
    });
    assert!(ops.is_empty(), "negative memo — no re-query");
    assert!(sut.view().sim.expect("re-judged").ready);
}

#[test]
fn sim_latest_wins() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::SimDeltasComputed {
        address: WALLET.to_owned(),
        chain_id: 1,
        deltas: vec![erc20_delta(FRESH, "1")],
    });
    // A newer preview replaces the resolving one.
    sut.dispatch(Event::SimDeltasComputed {
        address: WALLET.to_owned(),
        chain_id: 1,
        deltas: vec![erc20_delta(T_SENT, "-2")],
    });
    // The FIRST request's answer arrives: absorbed as a fact, but the view
    // belongs to the newer session, which is still resolving.
    sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(FRESH, Some(meta("A", 18)))],
    });
    let sim = sut.view().sim.expect("newest session");
    assert!(!sim.ready);
    // Its own answer completes it.
    sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(T_SENT, Some(meta("B", 18)))],
    });
    let sim = sut.view().sim.expect("newest session judged");
    assert!(sim.ready);
    assert_eq!(
        sim.judgments,
        vec![TrustSimJudgment::Erc20Trusted {
            token: T_SENT.to_owned(),
            delta: "-2".to_owned(),
            symbol: "B".to_owned(),
            decimals: 18,
            in_trusted_set: false,
        }]
    );
}

// ===========================================================================
// Machine — the admission pipeline (invariants ⑤⑧)
// ===========================================================================

/// The happy path, op by op: authentic receipt logs → fresh custom read →
/// metadata → write with the `{chainId}_{addr}` id and symbol-as-name →
/// cache invalidation for exactly this account.
#[test]
fn receipt_confirmation_drives_the_admission_pipeline() {
    let mut sut = booted(vec![1]);
    let ops = sut.dispatch(Event::ReceiptLogsConfirmed {
        from: WALLET.to_owned(),
        chain_id: 1,
        logs: vec![receipt(
            &FRESH.to_uppercase().replace("0X", "0x"),
            PEER,
            WALLET,
            100,
        )],
    });
    assert_eq!(ops, vec![Op::ReadCustomTokens]);

    let ops = sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    assert_eq!(
        ops,
        vec![Op::MulticallErc20Meta {
            chain_id: 1,
            addrs: vec![FRESH.to_owned()]
        }]
    );

    let ops = sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(FRESH, Some(meta("NEWT", 9)))],
    });
    assert_eq!(
        ops,
        vec![Op::WriteCustomToken {
            token: TrustCustomToken {
                id: format!("1_{FRESH}"),
                chain_id: 1,
                contract_address: FRESH.to_owned(),
                symbol: "NEWT".to_owned(),
                name: "NEWT".to_owned(), // list shows the symbol; no extra call
                decimals: 9,
            },
        }]
    );

    let ops = sut.resolve(Res::TokenWritten { ok: true });
    assert_eq!(
        ops,
        vec![Op::InvalidateTokenCache {
            address: WALLET_LC.to_owned()
        }],
        "without this the token hides behind the 5-min fetchTokens TTL"
    );
    assert!(sut.resolve(Res::CacheInvalidated).is_empty());
    assert!(sut.outstanding().is_empty());
}

#[test]
fn admission_skips_listed_held_and_known_tokens() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::HeldTokensSnapshot {
        address: WALLET.to_owned(),
        chain_id: 1,
        tokens: vec![HELD.to_owned()],
    });
    sut.dispatch(Event::ReceiptLogsConfirmed {
        from: WALLET.to_owned(),
        chain_id: 1,
        logs: vec![
            receipt(LISTED, PEER, WALLET, 1),
            receipt(HELD, PEER, WALLET, 2),
            receipt(KNOWN_USDC, PEER, WALLET, 3),
            receipt(FRESH, PEER, WALLET, 4),
        ],
    });
    let ops = sut.resolve(Res::CustomTokens {
        tokens: Some(vec![custom(1, LISTED, "MINE")]),
    });
    assert_eq!(
        ops,
        vec![Op::MulticallErc20Meta {
            chain_id: 1,
            addrs: vec![FRESH.to_owned()]
        }],
        "already-visible tokens are never re-admitted"
    );
    let ops = sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(FRESH, Some(meta("NEWT", 18)))],
    });
    assert_eq!(ops.len(), 1, "exactly one write — the fresh token");
    match &ops[0] {
        Op::WriteCustomToken { token } => assert_eq!(token.contract_address, FRESH),
        other => panic!("expected the write, got {other:?}"),
    }
}

/// Invariant ⑧ — a token whose symbol can't resolve is never listed ("don't
/// seed a '?'"), and with nothing admitted the cache is left alone.
#[test]
fn no_symbol_means_no_listing_and_no_invalidation() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::ReceiptLogsConfirmed {
        from: WALLET.to_owned(),
        chain_id: 1,
        logs: vec![
            receipt(FRESH, PEER, WALLET, 100),
            receipt(T_SENT, PEER, WALLET, 5),
        ],
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    let ops = sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![
            meta_entry(FRESH, None),                // unresolvable
            meta_entry(T_SENT, Some(meta("", 18))), // empty symbol
        ],
    });
    assert!(ops.is_empty(), "no write, no invalidation: {ops:?}");
    assert!(sut.outstanding().is_empty());
}

#[test]
fn wash_or_net_outflow_is_never_received() {
    let mut sut = booted(vec![1]);
    let ops = sut.dispatch(Event::ReceiptLogsConfirmed {
        from: WALLET.to_owned(),
        chain_id: 1,
        logs: vec![
            receipt(FRESH, PEER, WALLET, 10),
            receipt(FRESH, WALLET, PEER, 10), // wash → net 0
            receipt(T_SENT, WALLET, PEER, 5), // pure outflow
        ],
    });
    assert!(ops.is_empty(), "nothing net-received → no session at all");
    // And the degenerate guards.
    assert!(sut
        .dispatch(Event::ReceiptLogsConfirmed {
            from: String::new(),
            chain_id: 1,
            logs: vec![receipt(FRESH, PEER, WALLET, 1)]
        })
        .is_empty());
    assert!(sut
        .dispatch(Event::ReceiptLogsConfirmed {
            from: WALLET.to_owned(),
            chain_id: 1,
            logs: vec![]
        })
        .is_empty());
}

/// Invariant ⑧ — duplicate transfers of one token collapse to one write, and
/// the cache invalidation fires once after all writes land.
#[test]
fn duplicates_collapse_and_invalidation_fires_once_after_writes() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::ReceiptLogsConfirmed {
        from: WALLET.to_owned(),
        chain_id: 1,
        logs: vec![
            receipt(FRESH, PEER, WALLET, 1),
            receipt(FRESH, ATTACKER, WALLET, 2), // same token, another sender
            receipt(T_SENT, PEER, WALLET, 3),
        ],
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    let ops = sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![
            meta_entry(FRESH, Some(meta("A", 18))),
            meta_entry(T_SENT, Some(meta("B", 6))),
        ],
    });
    assert_eq!(ops.len(), 2, "one write per admitted token, no duplicates");
    assert!(
        sut.resolve(Res::TokenWritten { ok: true }).is_empty(),
        "not yet — one write left"
    );
    let ops = sut.resolve(Res::TokenWritten { ok: true });
    assert_eq!(
        ops,
        vec![Op::InvalidateTokenCache {
            address: WALLET_LC.to_owned()
        }]
    );
}

/// A failed save suppresses the invalidation — in TS the throw skips
/// `clearTokenCache` (`token-autoadd.ts:79-83`); fail-closed here too.
#[test]
fn failed_write_suppresses_cache_invalidation() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::ReceiptLogsConfirmed {
        from: WALLET.to_owned(),
        chain_id: 1,
        logs: vec![receipt(FRESH, PEER, WALLET, 100)],
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(FRESH, Some(meta("NEWT", 18)))],
    });
    let ops = sut.resolve(Res::TokenWritten { ok: false });
    assert!(
        ops.is_empty(),
        "no invalidation after a failed save: {ops:?}"
    );
    assert!(sut.outstanding().is_empty());
}

/// A failed custom-token read aborts the admission with nothing written —
/// the TS outer catch returns 0 (`token-autoadd.ts:83-85`).
#[test]
fn custom_read_failure_aborts_the_admission_fail_closed() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::ReceiptLogsConfirmed {
        from: WALLET.to_owned(),
        chain_id: 1,
        logs: vec![receipt(FRESH, PEER, WALLET, 100)],
    });
    let ops = sut.resolve(Res::CustomTokens { tokens: None });
    assert!(ops.is_empty());
    assert!(sut.outstanding().is_empty());
}

#[test]
fn second_receipt_queues_behind_the_active_session() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::ReceiptLogsConfirmed {
        from: WALLET.to_owned(),
        chain_id: 1,
        logs: vec![receipt(FRESH, PEER, WALLET, 1)],
    });
    let ops = sut.dispatch(Event::ReceiptLogsConfirmed {
        from: WALLET.to_owned(),
        chain_id: 137,
        logs: vec![receipt(T_SENT, PEER, WALLET, 2)],
    });
    assert!(ops.is_empty(), "queued behind the running admission");

    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    // The first session ends with nothing admissible → the queued one starts.
    let ops = sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(FRESH, None)],
    });
    assert_eq!(
        ops,
        vec![Op::ReadCustomTokens],
        "the second session's fresh read"
    );
    let ops = sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    assert_eq!(
        ops,
        vec![Op::MulticallErc20Meta {
            chain_id: 137,
            addrs: vec![T_SENT.to_owned()]
        }]
    );
}

/// The admission is a global-storage pipeline, not account-display state: a
/// mid-flight account switch must not strand a half-written admission (the
/// TS call completes regardless of the active account).
#[test]
fn admission_survives_an_account_switch() {
    let mut sut = booted(vec![1]);
    sut.dispatch(Event::ReceiptLogsConfirmed {
        from: WALLET.to_owned(),
        chain_id: 1,
        logs: vec![receipt(FRESH, PEER, WALLET, 100)],
    });
    sut.dispatch(Event::HeldChainsSnapshot {
        address: OTHER.to_owned(),
        chain_ids: vec![1],
    });
    let ops = sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    assert_eq!(
        ops,
        vec![Op::MulticallErc20Meta {
            chain_id: 1,
            addrs: vec![FRESH.to_owned()]
        }]
    );
    sut.resolve(Res::ErcMeta {
        chain_id: 1,
        entries: vec![meta_entry(FRESH, Some(meta("NEWT", 18)))],
    });
    let ops = sut.resolve(Res::TokenWritten { ok: true });
    assert_eq!(
        ops,
        vec![Op::InvalidateTokenCache {
            address: WALLET_LC.to_owned()
        }],
        "invalidation targets the receipt's account, not the active one"
    );
}
