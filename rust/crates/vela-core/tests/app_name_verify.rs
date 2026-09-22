//! Forward-verified names: a reverse record is a claim until the name resolves
//! back to the same address (spec 081, FR-010).
//!
//! The machine is a pure function of a transcript, so the tests are the
//! transcript: a scripted RPC answers each `eth_call` the core asks for, and
//! what comes out the other end is the verdict four shells will draw from.

#![cfg(feature = "crux")]

use vela_core::app::name_verify::{step, step_json, ForwardState, VerifyStep};
use vela_core::registry_lookup::{LookupAnswer, LookupOutcome, LookupRequest};

/// The ENS registry on Ethereum, as every shell's table holds it.
const REGISTRY: &str = "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e";
/// The address a reverse record was read for.
const MINE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
/// Somebody else entirely.
const THEIRS: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";
/// A resolver contract.
const RESOLVER: &str = "0x231b0ee14048e9dccd1d247744d114a4eb5e8e63";

/// A 32-byte word carrying a left-padded address, as `eth_call` returns one.
fn address_word(address: &str) -> String {
    format!("0x{:0>64}", address.trim_start_matches("0x").to_lowercase())
}

fn zero_word() -> String {
    format!("0x{}", "0".repeat(64))
}

/// An ENSIP-10 `resolve` return: `bytes` wrapping one `addr` word.
fn resolve_return(address: &str) -> String {
    format!(
        "0x{:064x}{:064x}{}",
        32,
        32,
        address_word(address).trim_start_matches("0x")
    )
}

fn ok(id: &str, body: &str) -> LookupAnswer {
    LookupAnswer {
        id: id.to_owned(),
        outcome: LookupOutcome::Ok,
        body: Some(body.to_owned()),
    }
}

fn failed(id: &str) -> LookupAnswer {
    LookupAnswer {
        id: id.to_owned(),
        outcome: LookupOutcome::Failed,
        body: None,
    }
}

/// Drive the machine to its verdict, answering each request with `answer`.
/// Returns the verdict and the whole transcript of what was asked.
fn run(
    address: &str,
    name: &str,
    mut answer: impl FnMut(&str, &str, &str) -> Option<String>,
) -> (VerifyStep, Vec<(String, String, String)>) {
    let mut answers: Vec<LookupAnswer> = Vec::new();
    let mut asked: Vec<(String, String, String)> = Vec::new();
    for _ in 0..16 {
        let next = step(1, REGISTRY, address, name, &answers);
        let VerifyStep::Ask { requests } = next else {
            return (next, asked);
        };
        for request in requests {
            let LookupRequest::EthCall { id, to, data, .. } = request else {
                unreachable!("this machine only asks for eth_call");
            };
            asked.push((id.clone(), to.clone(), data.clone()));
            answers.push(match answer(&id, &to, &data) {
                Some(body) => ok(&id, &body),
                None => failed(&id),
            });
        }
    }
    unreachable!("the machine never finished");
}

fn verdict(stepped: &VerifyStep) -> (ForwardState, Option<String>) {
    match stepped {
        VerifyStep::Done {
            forward_state,
            name,
        } => (*forward_state, name.clone()),
        VerifyStep::Ask { .. } => unreachable!("still asking"),
    }
}

/// The whole point of the module, end to end: an attacker sets the reverse
/// record of an address they control to a name that is not theirs. Resolving
/// that name forward lands on the real owner's address, not on theirs, so the
/// wallet is handed no name at all — and draws the bare address instead.
#[test]
fn a_reverse_record_that_resolves_elsewhere_yields_no_name() {
    let (stepped, asked) = run(MINE, "vitalik.eth", |id, _to, _data| match id {
        "resolver:0" => Some(address_word(RESOLVER)),
        // The forward record is the truth, and it names somebody else.
        "addr" => Some(address_word(THEIRS)),
        _ => None,
    });
    assert_eq!(
        verdict(&stepped),
        (ForwardState::Mismatch, None),
        "a name that resolves to another address is not this address's name"
    );
    assert_eq!(asked.len(), 2, "one resolver lookup, one addr lookup");
    // And nothing leaks the claimed name to the caller on the way out.
    assert!(
        !serde_json::to_string(&stepped)
            .unwrap_or_default()
            .contains("vitalik"),
        "a refused name must not travel back for something to draw"
    );
}

/// The honest case. The verified string comes back FROM the core, normalised,
/// so what a shell draws is what was checked.
#[test]
fn a_name_that_resolves_back_is_verified_and_names_itself() {
    let (stepped, asked) = run(MINE, "Alice.ETH", |id, _to, _data| match id {
        "resolver:0" => Some(address_word(RESOLVER)),
        "addr" => Some(address_word(MINE)),
        _ => None,
    });
    assert_eq!(
        verdict(&stepped),
        (ForwardState::Verified, Some("alice.eth".to_owned()))
    );
    // The forward lookup is asked of the registry first, then of the resolver
    // the registry named — never of an address the reverse record chose.
    assert_eq!(asked[0].1, REGISTRY);
    assert!(asked[0].2.starts_with("0x0178b8bf"), "resolver(bytes32)");
    assert_eq!(asked[1].1, RESOLVER);
    assert!(asked[1].2.starts_with("0x3b3b57de"), "addr(bytes32)");
}

/// EIP-55 casing is a display convention. Two spellings of one address must
/// not read as two addresses — which would refuse every honest name.
#[test]
fn the_address_comparison_ignores_checksum_casing() {
    let (stepped, _) = run(
        "0x88CCA0EEDBF2C4426110BBFC998F048689266894",
        "alice.eth",
        |id, _to, _data| match id {
            "resolver:0" => Some(address_word(RESOLVER)),
            "addr" => Some(address_word("0x88cca0eedbf2c4426110bbfc998f048689266894")),
            _ => None,
        },
    );
    assert_eq!(verdict(&stepped).0, ForwardState::Verified);
}

/// The node the forward lookup asks about is the namehash of the claimed name
/// — EIP-137's own worked example, so a wrong hash cannot pass as a right one.
#[test]
fn the_forward_lookup_asks_about_the_namehash_of_the_claimed_name() {
    let (_, asked) = run(MINE, "foo.eth", |id, _to, _data| match id {
        "resolver:0" => Some(address_word(RESOLVER)),
        "addr" => Some(address_word(MINE)),
        _ => None,
    });
    const FOO_ETH: &str = "de9b09fd7c5f901e23a3f19fecc54828e9c848539801e86591bd9801b019f84f";
    assert_eq!(asked[0].2, format!("0x0178b8bf{FOO_ETH}"));
    assert_eq!(asked[1].2, format!("0x3b3b57de{FOO_ETH}"));
}

/// A name nobody resolves — no resolver at the name, none at any ancestor.
#[test]
fn a_name_with_no_resolver_anywhere_is_a_mismatch() {
    let (stepped, _) = run(MINE, "nobody.eth", |_id, _to, _data| Some(zero_word()));
    assert_eq!(verdict(&stepped), (ForwardState::Mismatch, None));
}

/// The name exists, the resolver exists, and it holds no address: the record
/// points at nobody, so it names nobody.
#[test]
fn a_resolver_that_answers_the_zero_address_is_a_mismatch() {
    let (stepped, _) = run(MINE, "alice.eth", |id, _to, _data| match id {
        "resolver:0" => Some(address_word(RESOLVER)),
        "addr" => Some(zero_word()),
        _ => None,
    });
    assert_eq!(verdict(&stepped), (ForwardState::Mismatch, None));
}

/// Nobody answered. The name is NOT shown — failing open here would let an
/// attacker choose the moment: any name passes while the RPC is unreachable.
#[test]
fn an_rpc_that_does_not_answer_leaves_the_name_unverified() {
    let (registry_silent, _) = run(MINE, "alice.eth", |_id, _to, _data| None);
    assert_eq!(verdict(&registry_silent), (ForwardState::Unavailable, None));

    let (resolver_silent, _) = run(MINE, "alice.eth", |id, _to, _data| match id {
        "resolver:0" => Some(address_word(RESOLVER)),
        _ => None,
    });
    assert_eq!(verdict(&resolver_silent), (ForwardState::Unavailable, None));
}

/// ENSIP-10: most Basenames and every wildcard subdomain have no resolver at
/// their own node. The nearest ancestor's resolver is asked through
/// `resolve(dnsEncode(name), addr(node))` instead.
#[test]
fn a_wildcard_name_is_verified_through_its_ancestors_resolver() {
    let (stepped, asked) = run(MINE, "alice.base.eth", |id, _to, _data| match id {
        // No resolver at `alice.base.eth` …
        "resolver:0" => Some(zero_word()),
        // … but `base.eth` has one, and it answers for its children.
        "resolver:1" => Some(address_word(RESOLVER)),
        "resolve" => Some(resolve_return(MINE)),
        _ => None,
    });
    assert_eq!(
        verdict(&stepped),
        (ForwardState::Verified, Some("alice.base.eth".to_owned()))
    );
    let resolve_call = &asked[2];
    assert_eq!(resolve_call.1, RESOLVER);
    assert!(
        resolve_call.2.starts_with("0x9061b923"),
        "resolve(bytes,bytes)"
    );
    // The DNS-encoded name is in the calldata: 05 "alice" 04 "base" 03 "eth" 00.
    assert!(
        resolve_call.2.contains("05616c69636504626173650365746800"),
        "the wildcard resolver is handed the DNS-encoded name"
    );
    // And the inner call it is asked to perform is `addr(namehash(name))`.
    assert!(resolve_call.2.contains("3b3b57de"));
}

/// The same wildcard path, with the attack on it: the ancestor's resolver
/// answers with somebody else's address.
#[test]
fn a_wildcard_name_that_resolves_elsewhere_is_a_mismatch() {
    let (stepped, _) = run(MINE, "alice.base.eth", |id, _to, _data| match id {
        "resolver:0" => Some(zero_word()),
        "resolver:1" => Some(address_word(RESOLVER)),
        "resolve" => Some(resolve_return(THEIRS)),
        _ => None,
    });
    assert_eq!(verdict(&stepped), (ForwardState::Mismatch, None));
}

/// An extended resolver may implement `resolve` and nothing else, so an `addr`
/// call that comes back unreadable is worth one more question before the name
/// is given up on.
#[test]
fn a_resolver_that_only_speaks_ensip_10_still_verifies() {
    let (stepped, asked) = run(MINE, "alice.eth", |id, _to, _data| match id {
        "resolver:0" => Some(address_word(RESOLVER)),
        // Not a word: an extended resolver that does not implement `addr`.
        "addr" => Some("0x1234".to_owned()),
        "resolve" => Some(resolve_return(MINE)),
        _ => None,
    });
    assert_eq!(verdict(&stepped).0, ForwardState::Verified);
    assert_eq!(asked.len(), 3);
}

/// The records that are not names at all. None of them costs a round trip:
/// the machine refuses them before it asks anybody anything.
#[test]
fn a_record_that_is_not_a_name_is_refused_without_a_single_call() {
    for claimed in [
        "Vitalik Buterin",
        "alice",
        "",
        ".eth",
        "alice.",
        "a..eth",
        "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        "alice\u{202e}.eth",
    ] {
        let stepped = step(1, REGISTRY, MINE, claimed, &[]);
        assert_eq!(
            verdict(&stepped),
            (ForwardState::Mismatch, None),
            "{claimed:?} is not a name anything resolves"
        );
    }
}

/// The addresses that must not reach the network. The zero address is a
/// mint/burn counterparty, not somebody with a name.
#[test]
fn an_address_nobody_holds_is_never_asked_about() {
    for address in [
        "0x0000000000000000000000000000000000000000",
        "0x123",
        "",
        "0xZZcCA0EeDbF2C4426110bbFc998F048689266894",
    ] {
        let stepped = step(1, REGISTRY, address, "alice.eth", &[]);
        assert_eq!(verdict(&stepped), (ForwardState::Mismatch, None));
    }
}

/// A crafted name cannot turn one lookup into sixty: the ancestor walk is
/// capped, and a name that runs out of ancestors is simply not verified.
#[test]
fn the_ancestor_walk_is_bounded() {
    let deep = "a.b.c.d.e.f.g.h.eth";
    let (stepped, asked) = run(MINE, deep, |_id, _to, _data| Some(zero_word()));
    assert_eq!(verdict(&stepped), (ForwardState::Mismatch, None));
    assert!(asked.len() <= 5, "asked {} times", asked.len());
}

/// The binding's surface: JSON in, JSON out, and a transcript that does not
/// parse starts the walk over rather than failing into a verified name.
#[test]
fn the_json_surface_round_trips() {
    let first = step_json(1, REGISTRY, MINE, "alice.eth", "[]");
    assert!(first.contains(r#""type":"ask""#), "{first}");
    assert!(first.contains("0178b8bf"));

    let transcript = serde_json::to_string(&vec![
        ok("resolver:0", &address_word(RESOLVER)),
        ok("addr", &address_word(MINE)),
    ])
    .unwrap_or_default();
    let done = step_json(1, REGISTRY, MINE, "alice.eth", &transcript);
    assert_eq!(
        done,
        r#"{"type":"done","forward_state":"verified","name":"alice.eth"}"#
    );

    // Junk is read as an empty transcript, not as a verdict.
    assert_eq!(step_json(1, REGISTRY, MINE, "alice.eth", "not json"), first);
}

/// The chain the reverse record was read on is the chain the forward lookup is
/// asked on. Verifying a `.bnb` name against Ethereum would refuse every real
/// one and accept nothing — the whole rule has to travel with its chain.
#[test]
fn the_forward_lookup_stays_on_the_chain_it_was_asked_about() {
    let stepped = step(56, REGISTRY, MINE, "alice.bnb", &[]);
    let VerifyStep::Ask { requests } = stepped else {
        unreachable!("expected a request");
    };
    let LookupRequest::EthCall { chain_id, .. } = &requests[0] else {
        unreachable!("expected an eth_call");
    };
    assert_eq!(*chain_id, 56);
}
