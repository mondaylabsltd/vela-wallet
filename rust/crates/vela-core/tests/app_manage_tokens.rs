//! Rules of manual token management, one test per rule.
//!
//! The three inventory invariants each get their own tests: ① one
//! `${chainId}_${addr}` dedupe implementation (session guard, storage
//! re-read, the shared pure fn), ② no symbol ⇒ no listing, ③ a confirmed
//! mutation invalidates the fetchTokens cache. The recorded quirks (deleted
//! id stays "added"; a literal `"?"` symbol is listed) and the recorded
//! deviation (late probe answers for a superseded address are dropped) are
//! tested by name.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::manage_tokens::{
    admissible_erc20_meta, native_alias_token, token_id, Event, ManageTokens, MtokCustomToken,
    MtokNetwork, MtokOperation as Op, MtokShellResult as Res, MtokTokenMeta,
};

type Sut = DomainDriver<ManageTokens>;

/// Mixed case on purpose — the probe must carry it verbatim, the save must
/// lowercase it.
const ADDR: &str = "0xA0b86991c6218B36c1d19D4a2e9Eb0cE3606eB48";
const ADDR_LOWER: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

fn meta(name: &str, symbol: &str, decimals: u8) -> MtokTokenMeta {
    MtokTokenMeta {
        name: name.to_owned(),
        symbol: symbol.to_owned(),
        decimals,
    }
}

fn usdc() -> MtokTokenMeta {
    meta("USD Coin", "USDC", 6)
}

fn networks() -> Vec<MtokNetwork> {
    vec![
        MtokNetwork {
            chain_id: 8453,
            name: "Base".to_owned(),
        },
        MtokNetwork {
            chain_id: 1,
            name: "Ethereum".to_owned(),
        },
        MtokNetwork {
            chain_id: 42161,
            name: "Arbitrum".to_owned(),
        },
    ]
}

/// A probe answer for the canonical input — the echo carries [`ADDR`].
fn resolved(chain_id: u32, meta: Option<MtokTokenMeta>) -> Res {
    Res::ChainMetaResolved {
        chain_id,
        address: ADDR.to_owned(),
        meta,
    }
}

fn stored(chain_id: u32, symbol: &str) -> MtokCustomToken {
    MtokCustomToken {
        id: token_id(chain_id, ADDR),
        chain_id,
        contract_address: ADDR_LOWER.to_owned(),
        symbol: symbol.to_owned(),
        name: symbol.to_owned(),
        decimals: 6,
        network_name: "Base".to_owned(),
    }
}

/// Open the panel with `existing` already in storage.
fn opened(existing: Vec<MtokCustomToken>) -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Start);
    assert_eq!(ops, vec![Op::ReadCustomTokens]);
    let ops = sut.resolve(Res::CustomTokensLoaded { tokens: existing });
    assert!(ops.is_empty());
    sut
}

/// Open, type the address, probe all three networks.
fn probing(existing: Vec<MtokCustomToken>) -> Sut {
    let mut sut = opened(existing);
    assert!(sut
        .dispatch(Event::AddressInput { s: ADDR.to_owned() })
        .is_empty());
    let ops = sut.dispatch(Event::DetectRequested {
        networks: networks(),
    });
    assert_eq!(
        ops,
        vec![
            Op::MulticallErc20Meta {
                chain_id: 8453,
                address: ADDR.to_owned()
            },
            Op::MulticallErc20Meta {
                chain_id: 1,
                address: ADDR.to_owned()
            },
            Op::MulticallErc20Meta {
                chain_id: 42161,
                address: ADDR.to_owned()
            },
        ],
        "one probe per network, in parallel, address exactly as typed"
    );
    sut
}

/// Probe with USDC found on Base and Arbitrum, a miss on Ethereum.
fn detected() -> Sut {
    let mut sut = probing(vec![]);
    sut.resolve(resolved(8453, Some(usdc())));
    sut.resolve(resolved(1, None));
    sut.resolve(resolved(42161, Some(usdc())));
    assert!(!sut.view().detecting);
    sut
}

// ---------------------------------------------------------------------------
// Opening & input
// ---------------------------------------------------------------------------

/// The mount effect: opening loads the already-added list.
#[test]
fn start_loads_the_custom_token_list() {
    let sut = opened(vec![stored(8453, "USDC")]);
    let view = sut.view();
    assert_eq!(view.custom_tokens.len(), 1);
    assert_eq!(view.custom_tokens[0].symbol, "USDC");
}

#[test]
fn start_is_single_shot() {
    let mut sut = opened(vec![]);
    assert!(sut.dispatch(Event::Start).is_empty());
}

/// `ADDRESS_RE` gates the search button; an invalid input never probes.
#[test]
fn invalid_address_never_probes() {
    let mut sut = opened(vec![]);
    sut.dispatch(Event::AddressInput {
        s: "0xnot-an-address".to_owned(),
    });
    assert!(!sut.view().address_valid);
    let ops = sut.dispatch(Event::DetectRequested {
        networks: networks(),
    });
    assert!(ops.is_empty(), "no probe for an invalid address");
}

/// Typing clears the found cards (`setFoundTokens([])`).
#[test]
fn typing_clears_the_found_cards() {
    let mut sut = detected();
    assert_eq!(sut.view().found.len(), 2);
    sut.dispatch(Event::AddressInput { s: "0x".to_owned() });
    assert!(sut.view().found.is_empty());
}

// ---------------------------------------------------------------------------
// Detection — the parallel probe and its aggregate
// ---------------------------------------------------------------------------

/// Answers land in arbitrary order; the cards keep registry order, and only
/// chains whose metadata resolved are listed.
#[test]
fn aggregate_keeps_registry_order_and_skips_misses() {
    let mut sut = probing(vec![]);
    // Arbitrum answers first, then the Ethereum miss, then Base.
    sut.resolve(resolved(42161, Some(meta("USD Coin", "USDC", 6))));
    assert!(sut.view().detecting, "still waiting on two chains");
    sut.resolve(resolved(1, None));
    sut.resolve(resolved(8453, Some(meta("USD Coin", "USDC", 6))));

    let view = sut.view();
    assert!(!view.detecting);
    assert_eq!(
        view.found.iter().map(|f| f.chain_id).collect::<Vec<_>>(),
        vec![8453, 42161],
        "registry order, miss skipped"
    );
    assert_eq!(view.found[0].network_name, "Base");
    assert_eq!(view.found[1].network_name, "Arbitrum");
    assert!(!view.not_found);
}

/// Invariant ② — a missing symbol (or name) is a miss, never a card. The
/// shell already collapses `!name || !symbol` to `None`, but the core is the
/// one gate: even a `Some` with an empty field is refused.
#[test]
fn missing_symbol_or_name_is_never_listed() {
    let mut sut = probing(vec![]);
    sut.resolve(resolved(8453, Some(meta("Nameless Symbol", "", 18))));
    sut.resolve(resolved(1, Some(meta("", "GHOST", 18))));
    sut.resolve(resolved(42161, None));
    let view = sut.view();
    assert!(view.found.is_empty(), "no admissible metadata anywhere");
    assert!(view.not_found, "zero hits concludes as not-found");
}

/// Ported verbatim: the `'?'` of the invariant is the RENDER fallback for a
/// missing symbol — a contract whose `symbol()` literally returns `"?"`
/// passes today's `!symbol` check and is listed.
#[test]
fn a_literal_question_mark_symbol_is_listed_verbatim() {
    assert!(admissible_erc20_meta(&meta("Weird", "?", 18)));
    assert!(!admissible_erc20_meta(&meta("Weird", "", 18)));
}

/// Zero hits across every chain raises the not-found alert flag; the next
/// keystroke clears it.
#[test]
fn nothing_found_anywhere_raises_not_found() {
    let mut sut = probing(vec![]);
    sut.resolve(resolved(8453, None));
    sut.resolve(resolved(1, None));
    sut.resolve(resolved(42161, None));
    assert!(sut.view().not_found);
    sut.dispatch(Event::AddressInput { s: ADDR.to_owned() });
    assert!(!sut.view().not_found);
}

/// The search button is disabled while a probe runs (`loading` gate).
#[test]
fn detect_is_gated_while_a_probe_is_in_flight() {
    let mut sut = probing(vec![]);
    let ops = sut.dispatch(Event::DetectRequested {
        networks: networks(),
    });
    assert!(ops.is_empty(), "no second probe while one runs");
}

/// Recorded deviation from verbatim (016 staleness rule): today an address
/// edit lets the old probe's results land under the new input
/// (`AddTokenPanel.tsx:159-185`); here they are dropped.
#[test]
fn late_answers_for_a_superseded_address_are_dropped() {
    let mut sut = probing(vec![]);
    sut.dispatch(Event::AddressInput {
        s: "0x1111111111111111111111111111111111111111".to_owned(),
    });
    // The old probe's answers arrive now — all three must be ignored.
    sut.resolve(resolved(8453, Some(usdc())));
    sut.resolve(resolved(1, Some(usdc())));
    sut.resolve(resolved(42161, Some(usdc())));
    let view = sut.view();
    assert!(view.found.is_empty(), "stale metadata must not surface");
    assert!(!view.detecting);
    assert!(!view.not_found);
}

/// The sharper cut of the same deviation: re-detecting a NEW address
/// repopulates the pending set with the very same chain ids, so the pending
/// check alone would admit the old probe's answers. The address echo is what
/// keeps token A's metadata from being saved under address B.
#[test]
fn old_probe_answers_never_land_under_a_redetected_address() {
    const ADDR_B: &str = "0x1111111111111111111111111111111111111111";
    let mut sut = probing(vec![]); // probe #1, for ADDR — answers still owed
    sut.dispatch(Event::AddressInput {
        s: ADDR_B.to_owned(),
    });
    let ops = sut.dispatch(Event::DetectRequested {
        networks: networks(),
    });
    assert_eq!(ops.len(), 3, "probe #2, for ADDR_B");

    // Probe #1's answers arrive now, echoing ADDR — all dropped, and the
    // chains stay owed to probe #2.
    sut.resolve(resolved(8453, Some(usdc())));
    sut.resolve(resolved(1, Some(usdc())));
    sut.resolve(resolved(42161, Some(usdc())));
    let view = sut.view();
    assert!(
        view.found.is_empty(),
        "ADDR metadata must not surface under ADDR_B"
    );
    assert!(view.detecting, "probe #2 is still owed its answers");

    // Probe #2's own answers land normally.
    sut.resolve(Res::ChainMetaResolved {
        chain_id: 8453,
        address: ADDR_B.to_owned(),
        meta: Some(meta("Beta", "BETA", 18)),
    });
    sut.resolve(Res::ChainMetaResolved {
        chain_id: 1,
        address: ADDR_B.to_owned(),
        meta: None,
    });
    sut.resolve(Res::ChainMetaResolved {
        chain_id: 42161,
        address: ADDR_B.to_owned(),
        meta: None,
    });
    let view = sut.view();
    assert!(!view.detecting);
    assert_eq!(view.found.len(), 1);
    assert_eq!(view.found[0].symbol, "BETA");
}

// ---------------------------------------------------------------------------
// Save — invariants ① and ③
// ---------------------------------------------------------------------------

/// The full save: fresh dedupe read, then the write with the lowercased
/// address and the network's display name — and, once confirmed, the
/// fetchTokens cache invalidation (invariant ③).
#[test]
fn save_rereads_storage_then_writes_then_invalidates() {
    let mut sut = detected();
    let ops = sut.dispatch(Event::SaveRequested { chain_id: 8453 });
    assert_eq!(
        ops,
        vec![Op::ReadCustomTokens],
        "dedupe reads storage fresh"
    );

    let ops = sut.resolve(Res::CustomTokensLoaded { tokens: vec![] });
    let expected = MtokCustomToken {
        id: format!("8453_{ADDR_LOWER}"),
        chain_id: 8453,
        contract_address: ADDR_LOWER.to_owned(),
        symbol: "USDC".to_owned(),
        name: "USD Coin".to_owned(),
        decimals: 6,
        network_name: "Base".to_owned(),
    };
    assert_eq!(
        ops,
        vec![Op::WriteCustomToken {
            token: expected.clone()
        }]
    );
    assert!(sut.view().saving);

    let ops = sut.resolve(Res::Saved);
    assert_eq!(
        ops,
        vec![Op::InvalidateTokenCache],
        "invariant ③ — the picker must see the token now, not after the TTL"
    );
    let view = sut.view();
    assert!(!view.saving);
    assert_eq!(view.custom_tokens, vec![expected]);
    let base_card = &view.found[0];
    assert!(base_card.added, "the Base card flips to added");
    assert!(!view.found[1].added, "the Arbitrum card does not");
}

/// Invariant ① — an id already in storage (an earlier session, or the
/// auto-add path mid-session) is marked added WITHOUT a second write. The
/// fresh read is exactly what catches the cross-path duplicate.
#[test]
fn save_of_an_already_stored_id_marks_added_without_writing() {
    let mut sut = detected();
    let ops = sut.dispatch(Event::SaveRequested { chain_id: 8453 });
    assert_eq!(ops, vec![Op::ReadCustomTokens]);
    // The auto-add path stored the same id since our last read.
    let ops = sut.resolve(Res::CustomTokensLoaded {
        tokens: vec![stored(8453, "USDC")],
    });
    assert!(ops.is_empty(), "never a duplicate write (invariant ①)");
    let view = sut.view();
    assert!(view.found[0].added);
    assert!(!view.saving);
}

/// Invariant ① — the session guard: once added, tapping save again does
/// nothing at all.
#[test]
fn second_save_of_an_added_token_is_a_no_op() {
    let mut sut = detected();
    sut.dispatch(Event::SaveRequested { chain_id: 8453 });
    sut.resolve(Res::CustomTokensLoaded { tokens: vec![] });
    sut.resolve(Res::Saved);
    sut.resolve(Res::CacheInvalidated);
    let ops = sut.dispatch(Event::SaveRequested { chain_id: 8453 });
    assert!(ops.is_empty(), "the added guard blocks a re-save");
}

/// Invariant ① — the shared pure fn is the one `${chainId}_${addr}`
/// implementation, lowercasing included; the auto-add machine reuses it.
#[test]
fn token_id_lowercases_and_matches_the_ts_shape() {
    assert_eq!(token_id(8453, ADDR), format!("8453_{ADDR_LOWER}"));
    assert_eq!(token_id(1, ADDR_LOWER), format!("1_{ADDR_LOWER}"));
}

/// One save at a time: while one is in flight, another card's save is
/// ignored (the shared `saving` gate).
#[test]
fn saves_are_serialized() {
    let mut sut = detected();
    sut.dispatch(Event::SaveRequested { chain_id: 8453 });
    let ops = sut.dispatch(Event::SaveRequested { chain_id: 42161 });
    assert!(ops.is_empty(), "second save gated while the first runs");
}

/// A failed write flags the alert, does NOT mark the card added, and leaves
/// retry open.
#[test]
fn save_failure_flags_the_error_and_allows_retry() {
    let mut sut = detected();
    sut.dispatch(Event::SaveRequested { chain_id: 8453 });
    sut.resolve(Res::CustomTokensLoaded { tokens: vec![] });
    let ops = sut.resolve(Res::SaveFailed);
    assert!(ops.is_empty(), "no cache invalidation for a failed save");
    let view = sut.view();
    assert!(view.save_error);
    assert!(!view.found[0].added);
    assert!(!view.saving);
    assert!(view.custom_tokens.is_empty());
    // Retry runs the whole pipeline again.
    let ops = sut.dispatch(Event::SaveRequested { chain_id: 8453 });
    assert_eq!(ops, vec![Op::ReadCustomTokens]);
}

/// A save request for a chain with no resolved metadata must never write —
/// fail-closed against stray events (invariant ② at the write door).
#[test]
fn save_without_resolved_metadata_is_refused() {
    let mut sut = detected();
    let ops = sut.dispatch(Event::SaveRequested { chain_id: 1 });
    assert!(ops.is_empty(), "chain 1 was a miss — nothing to save");
    let ops = sut.dispatch(Event::SaveRequested { chain_id: 999 });
    assert!(ops.is_empty(), "chain 999 was never probed");
}

// ---------------------------------------------------------------------------
// Delete
// ---------------------------------------------------------------------------

/// Delete removes the row only once storage confirms, then invalidates the
/// fetchTokens cache (the onChanged refresh, as a core rule).
#[test]
fn delete_removes_after_confirmation_and_invalidates() {
    let id = token_id(8453, ADDR);
    let mut sut = opened(vec![stored(8453, "USDC")]);
    let ops = sut.dispatch(Event::DeleteRequested { id: id.clone() });
    assert_eq!(ops, vec![Op::RemoveCustomToken { id: id.clone() }]);
    assert_eq!(
        sut.view().custom_tokens.len(),
        1,
        "row stays until confirmed"
    );

    let ops = sut.resolve(Res::Removed { id });
    assert_eq!(ops, vec![Op::InvalidateTokenCache]);
    assert!(sut.view().custom_tokens.is_empty());
}

/// Fail-closed for the TS unhandled rejection: a failed remove keeps the row
/// (storage still has it) and issues no invalidation.
#[test]
fn delete_failure_keeps_the_row() {
    let id = token_id(8453, ADDR);
    let mut sut = opened(vec![stored(8453, "USDC")]);
    sut.dispatch(Event::DeleteRequested { id: id.clone() });
    let ops = sut.resolve(Res::RemoveFailed { id: id.clone() });
    assert!(ops.is_empty());
    assert_eq!(sut.view().custom_tokens.len(), 1);
    // And the delete may be retried.
    let ops = sut.dispatch(Event::DeleteRequested { id: id.clone() });
    assert_eq!(ops, vec![Op::RemoveCustomToken { id }]);
}

/// A delete for an id that is not on screen is ignored.
#[test]
fn delete_of_an_unknown_id_is_ignored() {
    let mut sut = opened(vec![]);
    let ops = sut.dispatch(Event::DeleteRequested {
        id: "8453_0xdead".to_owned(),
    });
    assert!(ops.is_empty());
}

/// Ported verbatim (quirk): `handleDelete` never touches `addedTokenIds`, so
/// after add-then-delete the found card still reads "added" and cannot
/// re-add for the rest of the session.
#[test]
fn deleted_token_still_reads_added_on_its_card() {
    let mut sut = detected();
    sut.dispatch(Event::SaveRequested { chain_id: 8453 });
    sut.resolve(Res::CustomTokensLoaded { tokens: vec![] });
    sut.resolve(Res::Saved);
    sut.resolve(Res::CacheInvalidated);

    let id = token_id(8453, ADDR);
    sut.dispatch(Event::DeleteRequested { id: id.clone() });
    sut.resolve(Res::Removed { id });
    sut.resolve(Res::CacheInvalidated);

    let view = sut.view();
    assert!(view.custom_tokens.is_empty(), "the row is gone");
    assert!(
        view.found[0].added,
        "but the card still reads added — verbatim"
    );
    assert!(
        sut.dispatch(Event::SaveRequested { chain_id: 8453 })
            .is_empty(),
        "and a re-add is blocked for the session — verbatim"
    );
}

/// Arc's native coin wears an ERC-20 interface at `0x3600…0000` (spec 060).
/// That contract answers `name`, `symbol` and `decimals` perfectly well, so
/// nothing about the probe says "stop" — and listing it would show one balance
/// as two rows and double the person's total.
#[test]
fn arcs_native_coin_is_refused_as_a_token_and_says_why() {
    const ARC: u32 = 5_042;
    let alias = native_alias_token(ARC).expect("Arc has a native alias");

    let mut sut = opened(vec![]);
    assert!(sut
        .dispatch(Event::AddressInput {
            s: alias.to_owned()
        })
        .is_empty());
    let ops = sut.dispatch(Event::DetectRequested {
        networks: vec![MtokNetwork {
            chain_id: ARC,
            name: "Arc".to_owned(),
        }],
    });
    assert_eq!(
        ops,
        vec![Op::MulticallErc20Meta {
            chain_id: ARC,
            address: alias.to_owned()
        }]
    );

    // The contract resolves — this is exactly the case a "no metadata" gate
    // would miss.
    sut.resolve(Res::ChainMetaResolved {
        chain_id: ARC,
        address: alias.to_owned(),
        meta: Some(meta("USD Coin", "USDC", 6)),
    });

    let view = sut.view();
    assert!(
        view.found.is_empty(),
        "the native coin is not a listable token"
    );
    assert!(
        view.native_alias,
        "the refusal must carry its reason, not read as 'not found'"
    );
}

/// The alias is per-CHAIN data, not a banned address: the same contract on
/// another chain is an ordinary token and must still be addable.
#[test]
fn the_same_address_on_another_chain_is_an_ordinary_token() {
    assert_eq!(native_alias_token(1), None);
    // Stable's USDT0 mirror is per-chain data too.
    assert_eq!(
        native_alias_token(988),
        Some("0x779Ded0c9e1022225f8E0630b35a9b54bE713736")
    );
    let alias = native_alias_token(5_042).unwrap();

    let mut sut = opened(vec![]);
    sut.dispatch(Event::AddressInput {
        s: alias.to_owned(),
    });
    sut.dispatch(Event::DetectRequested {
        networks: vec![MtokNetwork {
            chain_id: 1,
            name: "Ethereum".to_owned(),
        }],
    });
    sut.resolve(Res::ChainMetaResolved {
        chain_id: 1,
        address: alias.to_owned(),
        meta: Some(meta("Some Token", "SOME", 18)),
    });

    let view = sut.view();
    assert_eq!(view.found.len(), 1);
    assert!(!view.native_alias);
}
