//! `multisig_review`: the blocks, the state-diff guard (trace and probe), the
//! gate. The trace fixtures are anvil's own answers on the Gnosis fork
//! (`scripts/onchain/multisig-fork`), the slots cross-checked against it.

use super::*;

const SAFE: &str = "0x0e96ddc491878645dB7E3a47d2a6Ccbc94f17f32";
const A: &str = "0x1111111111111111111111111111111111111111";
const B: &str = "0x2222222222222222222222222222222222222222";
const C: &str = "0x3333333333333333333333333333333333333333";
const ATTACKER: &str = "0xbAd00000000000000000000000000000000000bA";
const SINGLETON: &str = "0x29fcB43b46531BcA003ddC8FCB67FFE91900C762";
const ZERO: &str = "0x0000000000000000000000000000000000000000";

fn a(s: &str) -> Address {
    s.parse().unwrap()
}

fn w(s: &str) -> String {
    hex(&word_addr(a(s)))
}

fn n(v: u64) -> String {
    hex(&word(U256::from(v)))
}

fn tx(to: &str, value: &str, data: &str, operation: u8) -> SafeTx {
    SafeTx {
        to: to.into(),
        value: value.into(),
        data: data.into(),
        operation,
        safe_tx_gas: "0x0".into(),
        base_gas: "0x0".into(),
        gas_price: "0x0".into(),
        gas_token: ZERO.into(),
        refund_receiver: ZERO.into(),
        nonce: "0x0".into(),
    }
}

fn add_owner(owner: &str, threshold: u64) -> String {
    let mut d = keccak256(b"addOwnerWithThreshold(address,uint256)")[..4].to_vec();
    d.extend_from_slice(&word_addr(a(owner)));
    d.extend_from_slice(&word(U256::from(threshold)));
    hex(&d)
}

fn reads(t: SafeTx) -> ReadsInput {
    ReadsInput {
        chain_id: 100,
        safe: SAFE.into(),
        tx: Some(t),
        owners: vec![A.into(), B.into()],
        threshold: 1,
        deployed: true,
        ..ReadsInput::default()
    }
}

fn review_with(t: SafeTx, trace: Option<Value>, probe: Option<Value>) -> MultisigReview {
    multisig_review(&ReviewInput {
        reads: reads(t),
        trace,
        probe,
        sim_reply: Some(r#"{"error":{"code":-32601,"message":"method not supported"}}"#.into()),
        ..ReviewInput::default()
    })
    .unwrap()
}

/// A prestate-diff answer with only the Safe's storage.
fn diff(pre: &[(String, String)], post: &[(String, String)]) -> Value {
    let side = |kv: &[(String, String)]| {
        let storage: Map<String, Value> = kv.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
        json!({ SAFE.to_lowercase(): { "storage": storage } })
    };
    json!({ "result": { "pre": side(pre), "post": side(post) } })
}

fn s(slot: B256) -> String {
    slot_hex(slot)
}

fn not_offered_answer() -> Value {
    json!({"error": {"code": -32601, "message": "method not supported: debug_traceCall"}})
}

// -- the slots, as the chain has them ------------------------------------------

#[test]
fn slots_match_the_fork() {
    // anvil, Gnosis fork, a 1.4.1 Safe with owners A, B: owners[SENTINEL] and
    // owners[C] as `debug_traceCall` named them, and the approvedHashes slot
    // its override took.
    assert_eq!(
        s(entry(SENTINEL, SLOT_OWNERS)),
        "0xe90b7bceb6e7df5418fb78d8ee546e97c83a08bbccc01a0644d599ccd2a7c2e0"
    );
    assert_eq!(
        s(entry(a(C), SLOT_OWNERS)),
        "0xc042e0f5eb0064bf1d9722ec18e39f827a2cf491b49d2d4c34a5ecf7a61b44b7"
    );
    let t = tx(SAFE, "0x0", &add_owner(C, 1), 0);
    let hash = multisig::safe_tx_hash(100, SAFE, &t).unwrap();
    assert_eq!(
        hash,
        "0xfdcb7570d9f7f723878d1fdbad751e53424b3f3dffb5690d3741886f28ecbe49"
    );
    assert_eq!(
        s(approved_entry(a(A), hash.parse().unwrap())),
        "0x3b72b4e92e42edcdd19669d6f57e4540db775cd367461984b879e8477bfe6044"
    );
}

#[test]
fn probe_calldata_is_the_abi_encoding() {
    let inner = Inner {
        to: a("0x0000000000000000000000000000000000000001"),
        value: U256::from(7),
        data: vec![0xde, 0xad, 0xbe, 0xef],
        operation: 1,
    };
    let got = probe_calldata(
        &inner,
        &[a("0x0000000000000000000000000000000000000003")],
        &[
            a("0x0000000000000000000000000000000000000004"),
            a("0x0000000000000000000000000000000000000005"),
        ],
    );
    // `cast calldata "probe(address,uint256,bytes,uint8,address[],address[])" …`
    assert_eq!(
        hex(&got),
        concat!(
            "0xc15543bc",
            "0000000000000000000000000000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000007",
            "00000000000000000000000000000000000000000000000000000000000000c0",
            "0000000000000000000000000000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000100",
            "0000000000000000000000000000000000000000000000000000000000000140",
            "0000000000000000000000000000000000000000000000000000000000000004",
            "deadbeef00000000000000000000000000000000000000000000000000000000",
            "0000000000000000000000000000000000000000000000000000000000000001",
            "0000000000000000000000000000000000000000000000000000000000000003",
            "0000000000000000000000000000000000000000000000000000000000000002",
            "0000000000000000000000000000000000000000000000000000000000000004",
            "0000000000000000000000000000000000000000000000000000000000000005"
        )
    );
}

// -- the reads -----------------------------------------------------------------

#[test]
fn reads_for_a_deployed_safe() {
    let r = review_reads(&reads(tx(SAFE, "0x0", &add_owner(C, 1), 0))).unwrap();
    assert!(r.need_code.is_empty() && r.unavailable.is_none());
    assert_eq!(r.sim_calls.len(), 1);
    assert_eq!(r.recipients, vec![checksum(a(C))]);
    let trace = r.trace.unwrap();
    assert_eq!(trace.method, "debug_traceCall");
    let overrides = &trace.params[2]["stateOverrides"][checksum(a(SAFE))]["stateDiff"];
    assert_eq!(
        overrides["0x3b72b4e92e42edcdd19669d6f57e4540db775cd367461984b879e8477bfe6044"],
        json!(n(1))
    );
    assert_eq!(trace.params[2]["tracerConfig"]["diffMode"], json!(true));
    let probe = r.probe.unwrap();
    assert_eq!(probe.method, "eth_call");
    assert_eq!(probe.params[0]["to"], json!(checksum(a(SAFE))));
    assert_eq!(
        probe.params[2][checksum(probe_address())]["code"],
        json!(PROBE_CODE.trim())
    );
    assert!(probe.params[0]["data"]
        .as_str()
        .unwrap()
        .starts_with("0xb4faba09"));
}

#[test]
fn reads_for_a_safe_not_deployed_put_it_in_place() {
    let g = multisig::multisig_genesis(&[A.into(), B.into()], 1, "Team", "0x1").unwrap();
    let safe = g.address.clone();
    let mut input = ReadsInput {
        chain_id: 100,
        safe: safe.clone(),
        tx: Some(tx(A, "0x1", "0x", 0)),
        deployed: false,
        genesis: Some(g.genesis.clone()),
        ..ReadsInput::default()
    };
    // First: the singleton's code is needed.
    let first = review_reads(&input).unwrap();
    assert_eq!(first.need_code.len(), 1);
    assert!(first.trace.is_none() && first.probe.is_none());
    // Code that is not the canonical singleton's: no state can be put in place.
    input
        .codes
        .insert(first.need_code[0].clone(), Some("0x6001".into()));
    assert_eq!(
        review_reads(&input).unwrap().unavailable.as_deref(),
        Some("singletonCode")
    );
    // A refused genesis: none either.
    let mut bad = g.genesis.clone();
    bad.factory = A.into();
    input.genesis = Some(bad);
    assert_eq!(
        review_reads(&input).unwrap().unavailable.as_deref(),
        Some("genesisRefused")
    );
}

/// Real-device S2: an undeployed multisig's first transaction was simulated
/// from a bare address and said "expected to fail" — then ran. The
/// simulation puts the multisig in place as its deployment will (the same
/// code and storage the probe uses); a deployed one needs none.
#[test]
fn the_simulation_of_an_undeployed_safe_includes_its_deployment() {
    let g = multisig::multisig_genesis(&[A.into(), B.into()], 1, "Team", "0x1").unwrap();
    let safe = g.address.clone();
    let mut input = ReadsInput {
        chain_id: 100,
        safe: safe.clone(),
        tx: Some(tx(A, "0x1", "0x", 0)),
        deployed: false,
        genesis: Some(g.genesis.clone()),
        ..ReadsInput::default()
    };
    let first = review_reads(&input).unwrap();
    assert!(first.sim_state_overrides.is_none(), "nothing to place yet");
    let singleton = first.need_code[0].clone();
    let dir = format!("{}/tests/vectors/safe-code", env!("CARGO_MANIFEST_DIR"));
    let code = ["safe-l2-1.4.1", "safe-1.4.1"]
        .iter()
        .map(|n| std::fs::read_to_string(format!("{dir}/{n}.hex")).unwrap())
        .map(|c| c.trim().to_owned())
        .find(|c| multisig::canonical_singleton_code(singleton.parse().unwrap(), c))
        .expect("a canonical singleton's code");
    input.codes.insert(singleton, Some(code));
    let reads = review_reads(&input).unwrap();
    let overrides = reads.sim_state_overrides.expect("the multisig placed");
    let placed = &overrides[&checksum(safe.parse().unwrap())];
    assert!(placed["code"].as_str().is_some_and(|c| c.len() > 2));
    assert!(placed["stateDiff"]
        .as_object()
        .is_some_and(|s| !s.is_empty()));
    // Deployed: the chain already has it.
    input.deployed = true;
    input.owners = vec![A.into(), B.into()];
    input.threshold = 1;
    assert!(review_reads(&input).unwrap().sim_state_overrides.is_none());
}

// -- the guard, from a trace ---------------------------------------------------

/// anvil's own answer for `addOwnerWithThreshold(C, 1)` executed by A.
fn add_owner_trace() -> Value {
    diff(
        &[(n(3), n(2)), (s(entry(SENTINEL, SLOT_OWNERS)), w(A))],
        &[
            (n(3), n(3)),
            (n(5), n(1)),
            (s(entry(a(C), SLOT_OWNERS)), w(A)),
            (s(entry(SENTINEL, SLOT_OWNERS)), w(C)),
        ],
    )
}

#[test]
fn a_declared_member_change_passes() {
    let r = review_with(
        tx(SAFE, "0x0", &add_owner(C, 1), 0),
        Some(add_owner_trace()),
        None,
    );
    assert_eq!(r.state.source, StateSource::Trace);
    assert_eq!(r.state.verdict, StateVerdict::Declared);
    assert!(r.sign.allowed);
    assert_eq!(r.recipients[0].role, RecipientRole::Member);
}

#[test]
fn the_same_writes_undeclared_are_refused() {
    // The proposal says "add C with 2 approvals"; the execution adds C and leaves 1.
    let r = review_with(
        tx(SAFE, "0x0", &add_owner(C, 2), 0),
        Some(add_owner_trace()),
        None,
    );
    assert!(
        matches!(&r.state.verdict, StateVerdict::Undeclared { writes }
        if writes.iter().any(|w| matches!(w, CriticalWrite::Threshold { .. })))
    );
    assert_eq!(r.tone, Tone::Refused);
    assert!(!r.sign.allowed);
}

#[test]
fn bybit_reads_as_a_transfer_and_writes_slot_zero() {
    // A proposal that reads as a plain transfer — but executing it replaces
    // the singleton (Bybit, 2025: the proxy's implementation swapped).
    let t = tx("0x4444444444444444444444444444444444444444", "0x1", "0x", 0);
    let trace = diff(
        &[(n(0), w(SINGLETON))],
        &[(n(0), w(ATTACKER)), (n(5), n(1))],
    );
    let r = review_with(t, Some(trace), None);
    assert!(r.review.action.is_some(), "it reads benign");
    assert_eq!(
        r.state.verdict,
        StateVerdict::Undeclared {
            writes: vec![CriticalWrite::Singleton {
                before: checksum(a(SINGLETON)),
                after: checksum(a(ATTACKER)),
            }]
        }
    );
    assert!(r.concerns.contains(&Concern::StateChange));
    assert_eq!(
        r.sign,
        SignRule {
            allowed: false,
            anyway: false
        }
    );
    // The gate turns it into "blocked", whatever the chain says of the network.
    let gate = signing_gate(&GateInput {
        chain_id: 100,
        findings: Some(vec![]),
        sign: Some(r.sign),
    });
    assert_eq!(gate, Gate::Blocked);
}

#[test]
fn bybit_delegatecall_shape_is_refused_twice() {
    // The real shape: a DELEGATECALL whose data is `transfer(address,uint256)`.
    let mut d = keccak256(b"transfer(address,uint256)")[..4].to_vec();
    d.extend_from_slice(&word_addr(a(ATTACKER)));
    d.extend_from_slice(&word(U256::ZERO));
    let t = tx(
        "0x96221423681A6d52E184D440a8eFCEbB105C7242",
        "0x0",
        &hex(&d),
        1,
    );
    let trace = diff(
        &[(n(0), w(SINGLETON))],
        &[(n(0), w(ATTACKER)), (n(5), n(1))],
    );
    let r = review_with(t, Some(trace), None);
    assert!(r.review.action.is_none());
    assert!(r
        .concerns
        .iter()
        .any(|c| matches!(c, Concern::NotSigned { .. })));
    assert!(r.concerns.contains(&Concern::StateChange));
    assert!(!r.sign.allowed);
    assert!(review_reads(&reads(tx(
        "0x96221423681A6d52E184D440a8eFCEbB105C7242",
        "0x0",
        &hex(&d),
        1
    )))
    .unwrap()
    .sim_calls
    .is_empty());
}

/// r9-9: a refusal says its own reason, once — a delegatecall "runs another
/// contract's code as the multisig", a batch too deep "can't be read" — and
/// never the state guard's words, nor its reassurance beside the refusal.
#[test]
fn each_refusal_says_its_own_reason_and_no_reassurance() {
    let mut d = keccak256(b"transfer(address,uint256)")[..4].to_vec();
    d.extend_from_slice(&word_addr(a(C)));
    d.extend_from_slice(&word(U256::from(1)));
    let delegate = tx(
        "0x5555555555555555555555555555555555555555",
        "0x0",
        &hex(&d),
        1,
    );
    // The probe answered "nothing about who controls it changes".
    let k = watch_of(&delegate, &[]);
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(k);
    let mut after = snap(&[A, B]);
    after.watched = zero_watch(k);
    let r = review_with(
        delegate,
        Some(not_offered_answer()),
        Some(probe_envelope(&before, &after, true)),
    );
    assert!(!r.sign.allowed);
    let refused = r.refused.clone().unwrap();
    assert_eq!(refused.why, RefusedWhy::Delegatecall);
    assert_eq!(refused.key, "home.multisig.review.refusedWhy.delegatecall");
    assert_eq!(
        r.state.verdict,
        StateVerdict::Withheld,
        "no \"changes nothing\" beside a refusal"
    );

    // Four batches deep.
    let transfer = format!("a9059cbb{:0>64}{:064x}", &C[2..], 1u64);
    let mut inner = transfer;
    for _ in 0..4 {
        let leg = format!(
            "00{}{:064x}{:064x}{}",
            &multisig::MULTI_SEND_CALL_ONLY[2..],
            0,
            inner.len() / 2,
            inner
        );
        let pad = (64 - (leg.len() % 64)) % 64;
        inner = format!(
            "8d80ff0a{:064x}{:064x}{}{}",
            0x20,
            leg.len() / 2,
            leg,
            "0".repeat(pad)
        );
    }
    let deep = tx(
        "0x5555555555555555555555555555555555555555",
        "0x0",
        &format!("0x{inner}"),
        0,
    );
    let r = review_with(deep, Some(not_offered_answer()), None);
    assert!(!r.sign.allowed);
    assert_eq!(r.tone, Tone::Refused);
    assert_eq!(r.refused.as_ref().unwrap().why, RefusedWhy::TooDeep);
    // r10 follow-up: one refusal box — the "can't be read" concern the
    // refusal came from is not said beside it, by any shell.
    assert!(r.reading.unreadable);
    assert!(!r.concerns.contains(&Concern::UnreadableBatch));
    assert_eq!(r.state.verdict, StateVerdict::Withheld);
    assert!(!r
        .concerns
        .iter()
        .any(|c| matches!(c, Concern::StateUnread { .. })));

    // An undeclared change is the guard's own refusal, said as such.
    let t = tx(SAFE, "0x0", &add_owner(C, 1), 0);
    let k = watch_of(&t, &[C]);
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(k);
    let mut swapped = snap(&[C, A, B]);
    swapped.watched = zero_watch(k);
    swapped.singleton = a(ATTACKER);
    let r = review_with(
        t,
        Some(not_offered_answer()),
        Some(probe_envelope(&before, &swapped, true)),
    );
    assert_eq!(r.refused.unwrap().key, "home.multisig.review.blocked");

    // A transaction it signs refuses nothing.
    let ok = tx(C, "0x1", "0x", 0);
    assert!(review_with(ok, Some(not_offered_answer()), None)
        .refused
        .is_none());
}

/// A refused delegatecall (or a batch too deep) says nothing about what
/// moves: the probe read the Safe's balances unchanged, yet "No asset
/// changes" under "Vela won't sign this" is false comfort — the call runs
/// another contract's code as the multisig. The simulation is `withheld`:
/// no deltas, no notice, no no-change line, and no could-not-check either.
#[test]
fn a_refusal_withholds_what_moves() {
    let mut d = keccak256(b"transfer(address,uint256)")[..4].to_vec();
    d.extend_from_slice(&word_addr(a(C)));
    d.extend_from_slice(&word(U256::from(1)));
    let delegate = tx(
        "0x5555555555555555555555555555555555555555",
        "0x0",
        &hex(&d),
        1,
    );
    let k = watch_of(&delegate, &[]);
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(k);
    let mut after = snap(&[A, B]);
    after.watched = zero_watch(k);
    let r = review_with(
        delegate,
        Some(not_offered_answer()),
        Some(probe_envelope(&before, &after, true)),
    );
    assert_eq!(r.refused.as_ref().unwrap().why, RefusedWhy::Delegatecall);
    assert_eq!(r.simulation, sim_withheld());
    assert_eq!(r.simulation.kind, "withheld");
    assert!(r.simulation.deltas.is_empty());
    assert_eq!(r.simulation.no_change_key, None, "no \"No asset changes\"");
    assert_eq!(r.simulation.notice_key, None);
    assert!(!r
        .concerns
        .iter()
        .any(|c| matches!(c, Concern::Unsimulated | Concern::Reverts)));
    assert_eq!(r.tone, Tone::Refused);
    assert!(!r.sign.allowed);
    // On the wire: the kind the shells branch on.
    let wire = serde_json::to_value(&r).unwrap();
    assert_eq!(wire["simulation"]["kind"], "withheld");
    assert!(wire["simulation"]["no_change_key"].is_null());

    // A batch too deep, with no reading at all: withheld too, not "couldn't check".
    let transfer = format!("a9059cbb{:0>64}{:064x}", &C[2..], 1u64);
    let mut inner = transfer;
    for _ in 0..4 {
        let leg = format!(
            "00{}{:064x}{:064x}{}",
            &multisig::MULTI_SEND_CALL_ONLY[2..],
            0,
            inner.len() / 2,
            inner
        );
        let pad = (64 - (leg.len() % 64)) % 64;
        inner = format!(
            "8d80ff0a{:064x}{:064x}{}{}",
            0x20,
            leg.len() / 2,
            leg,
            "0".repeat(pad)
        );
    }
    let deep = tx(
        "0x5555555555555555555555555555555555555555",
        "0x0",
        &format!("0x{inner}"),
        0,
    );
    let r = review_with(deep, Some(not_offered_answer()), None);
    assert_eq!(r.refused.as_ref().unwrap().why, RefusedWhy::TooDeep);
    assert_eq!(r.simulation.kind, SIM_WITHHELD);
    assert!(!r.concerns.contains(&Concern::Unsimulated));

    // The state guard's own refusal keeps what moves: there the chain's
    // answer is the evidence.
    let t = tx(SAFE, "0x0", &add_owner(C, 1), 0);
    let k = watch_of(&t, &[C]);
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(k);
    let mut swapped = snap(&[C, A, B]);
    swapped.watched = zero_watch(k);
    swapped.singleton = a(ATTACKER);
    let r = review_with(
        t,
        Some(not_offered_answer()),
        Some(probe_envelope(&before, &swapped, true)),
    );
    assert_eq!(r.refused.as_ref().unwrap().why, RefusedWhy::StateChange);
    assert_eq!(r.simulation.kind, "deltas");

    // A transaction it signs: what moves, as always.
    let ok = tx(C, "0x1", "0x", 0);
    assert_ne!(
        review_with(ok, Some(not_offered_answer()), None)
            .simulation
            .kind,
        SIM_WITHHELD
    );
}

#[test]
fn an_undeclared_add_owner_through_a_nested_call_is_refused() {
    // A call to another contract that, through a module or a re-entry, adds
    // the attacker as an owner. The attacker is in the call's data.
    let mut d = keccak256(b"doSomething(address)")[..4].to_vec();
    d.extend_from_slice(&word_addr(a(ATTACKER)));
    let t = tx(
        "0x5555555555555555555555555555555555555555",
        "0x0",
        &hex(&d),
        0,
    );
    let trace = diff(
        &[(n(3), n(2)), (s(entry(SENTINEL, SLOT_OWNERS)), w(A))],
        &[
            (n(3), n(3)),
            (n(5), n(1)),
            (s(entry(a(ATTACKER), SLOT_OWNERS)), w(A)),
            (s(entry(SENTINEL, SLOT_OWNERS)), w(ATTACKER)),
        ],
    );
    let r = review_with(t.clone(), Some(trace), None);
    let StateVerdict::Undeclared { writes } = &r.state.verdict else {
        panic!("{:?}", r.state.verdict)
    };
    assert!(writes.contains(&CriticalWrite::Owners {
        added: vec![checksum(a(ATTACKER))],
        removed: vec![],
    }));
    assert!(!r.sign.allowed);

    // The attacker nowhere in the data: its entry is a slot Vela cannot name.
    let hidden = diff(
        &[(n(3), n(2)), (s(entry(SENTINEL, SLOT_OWNERS)), w(A))],
        &[
            (n(3), n(3)),
            (n(5), n(1)),
            (
                s(entry(
                    a("0x7777777777777777777777777777777777777777"),
                    SLOT_OWNERS,
                )),
                w(A),
            ),
            (
                s(entry(SENTINEL, SLOT_OWNERS)),
                w("0x7777777777777777777777777777777777777777"),
            ),
        ],
    );
    let r = review_with(t, Some(hidden), None);
    let StateVerdict::Undeclared { writes } = &r.state.verdict else {
        panic!("{:?}", r.state.verdict)
    };
    assert!(writes
        .iter()
        .any(|w| matches!(w, CriticalWrite::Storage { .. })));
    assert!(writes
        .iter()
        .any(|w| matches!(w, CriticalWrite::Owners { .. })));
}

#[test]
fn a_guard_or_module_set_is_refused() {
    let t = tx("0x5555555555555555555555555555555555555555", "0x0", "0x", 0);
    let guard = diff(
        &[],
        &[(GUARD_STORAGE_SLOT.into(), w(ATTACKER)), (n(5), n(1))],
    );
    let r = review_with(t.clone(), Some(guard), None);
    assert_eq!(
        r.state.verdict,
        StateVerdict::Undeclared {
            writes: vec![CriticalWrite::Guard {
                before: checksum(Address::ZERO),
                after: checksum(a(ATTACKER))
            }]
        }
    );
    // enableModule(ATTACKER): modules[SENTINEL] → ATTACKER, modules[ATTACKER] → SENTINEL.
    let module = diff(
        &[(
            s(entry(SENTINEL, SLOT_MODULES)),
            w("0x0000000000000000000000000000000000000001"),
        )],
        &[
            (s(entry(SENTINEL, SLOT_MODULES)), w(ATTACKER)),
            (n(5), n(1)),
        ],
    );
    let r = review_with(t, Some(module), None);
    assert!(
        matches!(&r.state.verdict, StateVerdict::Undeclared { writes }
        if writes.contains(&CriticalWrite::Module { address: checksum(a(ATTACKER)), enabled: true }))
    );
}

#[test]
fn a_transfer_that_touches_nothing_is_unchanged() {
    let t = tx("0x4444444444444444444444444444444444444444", "0x1", "0x", 0);
    let r = review_with(t, Some(diff(&[], &[(n(5), n(1))])), None);
    assert_eq!(r.state.verdict, StateVerdict::Unchanged);
    // A nonce that moves by more than the execution's own step is not.
    let t = tx("0x4444444444444444444444444444444444444444", "0x1", "0x", 0);
    let r = review_with(t, Some(diff(&[], &[(n(5), n(9))])), None);
    assert!(
        matches!(&r.state.verdict, StateVerdict::Undeclared { writes } if writes == &vec![CriticalWrite::Nonce])
    );
}

// -- the guard, from the probe -------------------------------------------------

struct Snap {
    singleton: Address,
    owners: Vec<Address>,
    threshold: u64,
    modules: Vec<Address>,
    balance: u64,
    watched: Vec<(U256, U256)>,
    tokens: Vec<Option<u64>>,
}

fn snap(owners: &[&str]) -> Snap {
    Snap {
        singleton: a(SINGLETON),
        owners: owners.iter().map(|o| a(o)).collect(),
        threshold: 1,
        modules: vec![],
        balance: 100,
        watched: vec![],
        tokens: vec![],
    }
}

fn words(s: &Snap) -> Vec<U256> {
    let addr_word = |a: Address| U256::from_be_slice(&word_addr(a));
    let mut out = vec![
        addr_word(s.singleton),
        U256::from(s.owners.len()),
        U256::from(s.threshold),
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::ZERO,
        U256::from(s.balance),
    ];
    for list in [&s.owners, &s.modules] {
        out.push(U256::from(1));
        out.push(U256::from(list.len()));
        out.extend(list.iter().map(|a| addr_word(*a)));
    }
    for (o, m) in &s.watched {
        out.push(*o);
        out.push(*m);
    }
    for t in &s.tokens {
        out.push(U256::from(u64::from(t.is_some())));
        out.push(U256::from(t.unwrap_or(0)));
    }
    out
}

/// The revert payload `simulateAndRevert` answers with, as `eth_call`'s error.
fn probe_envelope(before: &Snap, after: &Snap, success: bool) -> Value {
    let enc_array = |v: &[U256]| {
        let mut o = word(U256::from(v.len())).to_vec();
        v.iter().for_each(|x| o.extend_from_slice(&word(*x)));
        o
    };
    let (b, af) = (enc_array(&words(before)), enc_array(&words(after)));
    let mut body = Vec::new();
    body.extend_from_slice(&word(U256::from(128)));
    body.extend_from_slice(&word(U256::from(128 + b.len())));
    body.extend_from_slice(&word(U256::from(u64::from(success))));
    body.extend_from_slice(&word(U256::from(128 + b.len() + af.len())));
    body.extend(b);
    body.extend(af);
    body.extend_from_slice(&word(U256::ZERO));
    let mut out = word(U256::from(1)).to_vec();
    out.extend_from_slice(&word(U256::from(body.len())));
    out.extend(body);
    json!({"error": {"code": 3, "message": "execution reverted", "data": hex(&out)}})
}

/// The watched list `review_reads` builds for `t` (sentinel, members, named, data).
fn watch_of(t: &SafeTx, declared_named: &[&str]) -> usize {
    let named: Vec<Address> = declared_named.iter().map(|x| a(x)).collect();
    watched(t, &[a(A), a(B)], &named).len()
}

fn zero_watch(count: usize) -> Vec<(U256, U256)> {
    vec![(U256::ZERO, U256::ZERO); count]
}

#[test]
fn probe_declared_change_passes_and_slot_zero_is_refused() {
    let t = tx(SAFE, "0x0", &add_owner(C, 1), 0);
    let k = watch_of(&t, &[C]);
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(k);
    let mut after = snap(&[C, A, B]);
    after.watched = zero_watch(k);
    let r = review_with(
        t.clone(),
        Some(not_offered_answer()),
        Some(probe_envelope(&before, &after, true)),
    );
    assert_eq!(r.state.source, StateSource::Probe);
    assert_eq!(r.state.verdict, StateVerdict::Declared);

    let mut swapped = snap(&[C, A, B]);
    swapped.watched = zero_watch(k);
    swapped.singleton = a(ATTACKER);
    let r = review_with(
        t,
        Some(not_offered_answer()),
        Some(probe_envelope(&before, &swapped, true)),
    );
    assert!(
        matches!(&r.state.verdict, StateVerdict::Undeclared { writes }
        if writes.iter().any(|w| matches!(w, CriticalWrite::Singleton { .. })))
    );
    assert!(!r.sign.allowed);
}

#[test]
fn probe_sees_a_signing_right_outside_the_list() {
    // owners[ATTACKER] set without linking it: `checkNSignatures` would
    // accept the attacker's signature, `getOwners` would not show it.
    let mut d = keccak256(b"doSomething(address)")[..4].to_vec();
    d.extend_from_slice(&word_addr(a(ATTACKER)));
    let t = tx(
        "0x5555555555555555555555555555555555555555",
        "0x0",
        &hex(&d),
        0,
    );
    let watch = watched(&t, &[a(A), a(B)], &[]);
    let at = watch.iter().position(|x| *x == a(ATTACKER)).unwrap();
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(watch.len());
    let mut after = snap(&[A, B]);
    after.watched = zero_watch(watch.len());
    after.watched[at].0 = U256::from(1);
    // The called contract is read as a token too (it may be one): it answers no balance.
    before.tokens = vec![None];
    after.tokens = vec![None];
    let r = review_with(
        t,
        Some(not_offered_answer()),
        Some(probe_envelope(&before, &after, true)),
    );
    assert_eq!(
        r.state.verdict,
        StateVerdict::Undeclared {
            writes: vec![CriticalWrite::OwnerEntry {
                address: checksum(a(ATTACKER))
            }]
        }
    );
}

#[test]
fn probe_reads_what_moves_where_no_simulation_is_offered() {
    let token = "0x6666666666666666666666666666666666666666";
    let mut d = keccak256(b"transfer(address,uint256)")[..4].to_vec();
    d.extend_from_slice(&word_addr(a(C)));
    d.extend_from_slice(&word(U256::from(5)));
    let t = tx(token, "0x0", &hex(&d), 0);
    let k = watch_of(&t, &[]);
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(k);
    before.tokens = vec![Some(50)];
    let mut after = snap(&[A, B]);
    after.watched = zero_watch(k);
    after.tokens = vec![Some(45)];
    let r = review_with(
        t,
        Some(not_offered_answer()),
        Some(probe_envelope(&before, &after, true)),
    );
    assert_eq!(r.simulation_source, "probe");
    assert_eq!(r.simulation.kind, "deltas");
    assert_eq!(r.simulation.deltas.len(), 1);
    assert_eq!(r.simulation.deltas[0].delta, "-5");
    assert_eq!(r.simulation.deltas[0].token.as_deref(), Some(token));
    assert_eq!(r.state.verdict, StateVerdict::Unchanged);
}

/// r9-2: Arc's USDC is ONE balance — the coin (18 decimals) and its ERC-20
/// interface at `0x3600…0000` (6). A 0.01 transfer through the interface
/// moves both readings; the review says it once, as the coin.
#[test]
fn arc_usdc_moved_through_its_erc20_interface_is_said_once() {
    let usdc = "0x3600000000000000000000000000000000000000";
    let mut d = keccak256(b"transfer(address,uint256)")[..4].to_vec();
    d.extend_from_slice(&word_addr(a(C)));
    d.extend_from_slice(&word(U256::from(10_000)));
    let t = tx(usdc, "0x0", &hex(&d), 0);
    let mut input = reads(t.clone());
    input.chain_id = 5_042_002;
    let k = watched(&t, &[a(A), a(B)], &[]).len();
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(k);
    before.balance = 50_000_000_000_000_000;
    before.tokens = vec![Some(50_000)];
    let mut after = snap(&[A, B]);
    after.watched = zero_watch(k);
    after.balance = 40_000_000_000_000_000;
    after.tokens = vec![Some(40_000)];
    let review = |reads: ReadsInput| {
        multisig_review(&ReviewInput {
            reads,
            trace: Some(not_offered_answer()),
            probe: Some(probe_envelope(&before, &after, true)),
            sim_reply: Some(r#"{"error":{"code":-32601,"message":"method not supported"}}"#.into()),
            ..ReviewInput::default()
        })
        .unwrap()
    };
    let r = review(input.clone());
    assert_eq!(r.simulation_source, "probe");
    assert_eq!(r.simulation.deltas.len(), 1, "{:?}", r.simulation.deltas);
    assert_eq!(r.simulation.deltas[0].token, None);
    assert_eq!(r.simulation.deltas[0].delta, "-10000000000000000");

    // The same address on another chain is an ordinary token: both said.
    input.chain_id = 100;
    assert_eq!(review(input).simulation.deltas.len(), 2);
}

/// r5reg-1: what moves in a batch of native sends, as a node that traces
/// transfers AND logs native moves itself (Arc) answers — to an account and
/// to a smart account (a Vela account's `SafeReceived` beside it), alone and
/// together: each send once, never the traced log and the chain's log both.
#[test]
fn a_native_batch_simulated_where_the_chain_logs_it_too_counts_each_send_once() {
    const SMART: &str = "0x412f2165f299Dfc2363ED08F8Fc6DE4353E81fF6";
    const EOA: &str = "0x9181a394E0a993E9eDe4DddC96Ac0B2Fe5ee039e";
    let transfer_topic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
    let safe_received = "0x3d0ce9bfc3ed7d6862dbb28b2dea94561fe714a1b4d019aa8af39730d1ad7c3d";
    let thousandth = U256::from(1_000_000_000_000_000u64);
    let leg = |to: &str| {
        let mut logs = vec![
            json!({"address": "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
                   "topics": [transfer_topic, w(SAFE), w(to)], "data": hex(&word(thousandth))}),
            json!({"address": "0xfffffffffffffffffffffffffffffffffffffffe",
                   "topics": [transfer_topic, w(SAFE), w(to)], "data": hex(&word(thousandth))}),
        ];
        if to == SMART {
            logs.push(json!({"address": to.to_lowercase(),
                             "topics": [safe_received, w(SAFE)], "data": hex(&word(thousandth))}));
        }
        json!({"status": "0x1", "returnData": "0x", "gasUsed": "0x5208", "logs": logs})
    };
    for legs in [
        vec![SMART],
        vec![EOA],
        vec![SMART, SMART],
        vec![EOA, EOA],
        vec![SMART, EOA],
    ] {
        let transfers: Vec<crate::multisig::Transfer> = legs
            .iter()
            .map(|to| crate::multisig::Transfer {
                token: None,
                to: (*to).into(),
                amount: "1000000000000000".into(),
            })
            .collect();
        let t = crate::multisig::transfers_tx(&transfers, "0x0").unwrap();
        let reply =
            json!({"result": [{"calls": legs.iter().map(|to| leg(to)).collect::<Vec<_>>()}]});
        let r = multisig_review(&ReviewInput {
            reads: reads(t),
            trace: Some(not_offered_answer()),
            sim_reply: Some(reply.to_string()),
            ..ReviewInput::default()
        })
        .unwrap();
        assert_eq!(r.simulation_source, "simulate", "{legs:?}");
        assert_eq!(r.simulation.deltas.len(), 1, "{legs:?}");
        let want = format!("-{}", thousandth * U256::from(legs.len()));
        assert_eq!(r.simulation.deltas[0].delta, want, "{legs:?}");
    }
}

#[test]
fn probe_inner_revert_is_a_danger_with_sign_anyway() {
    let t = tx("0x4444444444444444444444444444444444444444", "0x1", "0x", 0);
    let k = watch_of(&t, &[]);
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(k);
    let mut after = snap(&[A, B]);
    after.watched = zero_watch(k);
    let r = review_with(
        t,
        Some(not_offered_answer()),
        Some(probe_envelope(&before, &after, false)),
    );
    assert_eq!(r.state.verdict, StateVerdict::Reverts);
    assert_eq!(r.simulation.kind, "reverts");
    assert!(r.concerns.contains(&Concern::Reverts));
    assert_eq!(r.tone, Tone::Danger);
    assert_eq!(
        r.sign,
        SignRule {
            allowed: true,
            anyway: true
        }
    );
}

#[test]
fn no_source_says_so_and_decoding_stands() {
    let t = tx("0x4444444444444444444444444444444444444444", "0x1", "0x", 0);
    let r = review_with(t, Some(not_offered_answer()), Some(not_offered_answer()));
    assert_eq!(r.state.source, StateSource::None);
    assert_eq!(
        r.state.verdict,
        StateVerdict::Unavailable {
            reason: "notOffered".into()
        }
    );
    assert!(r.concerns.contains(&Concern::StateUnread {
        reason: "notOffered".into()
    }));
    assert!(r.concerns.contains(&Concern::Unsimulated));
    assert_eq!(r.tone, Tone::Caution);
    assert!(r.sign.allowed);
    let t = tx("0x4444444444444444444444444444444444444444", "0x1", "0x", 0);
    let r = review_with(
        t,
        Some(json!({"unreachable": true})),
        Some(json!({"unreachable": true})),
    );
    assert_eq!(
        r.state.verdict,
        StateVerdict::Unavailable {
            reason: "unreachable".into()
        }
    );
}

// -- the blocks ------------------------------------------------------------------

#[test]
fn the_review_carries_hashes_fields_and_typed_data() {
    let t = tx(SAFE, "0x0", &add_owner(C, 1), 0);
    let r = review_with(t.clone(), Some(add_owner_trace()), None);
    let h = multisig::safe_tx_hashes(100, SAFE, &t).unwrap();
    assert_eq!(r.hashes, h);
    let names: Vec<&str> = r.fields.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "to",
            "value",
            "data",
            "operation",
            "safeTxGas",
            "baseGas",
            "gasPrice",
            "gasToken",
            "refundReceiver",
            "nonce"
        ]
    );
    assert_eq!(r.typed_data["primaryType"], json!("SafeTx"));
    assert_eq!(r.typed_data["domain"]["chainId"], json!(100));
    assert!(r.member_changes.is_some());
    // The opinion is bound to this proposal on this chain.
    assert_eq!(
        r.advice_request["id"],
        json!(format!("multisig:100:{}", h.safe_tx_hash))
    );
    assert_eq!(r.advice_request["kind"], json!("transaction"));
    let params: Value =
        serde_json::from_str(r.advice_request["params_json"].as_str().unwrap()).unwrap();
    assert_eq!(params["safeTxHash"], json!(h.safe_tx_hash));
}

#[test]
fn recipients_first_time_contract_and_lookalike() {
    let poisoned = "0x1234000000000000000000000000000000005678";
    let friend = "0x1234ffffffffffffffffffffffffffffffff5678";
    let t = tx(poisoned, "0x1", "0x", 0);
    let r = multisig_review(&ReviewInput {
        reads: reads(t),
        recipient_facts: vec![RecipientFacts {
            address: checksum(a(poisoned)),
            is_contract: Some(true),
            first_time: Some(true),
        }],
        known: vec![Known {
            address: friend.into(),
            label: "Alice".into(),
            kind: KnownKind::Contact,
        }],
        sim_reply: Some(r#"{"unreachable":true}"#.into()),
        ..ReviewInput::default()
    })
    .unwrap();
    let rc = &r.recipients[0];
    assert_eq!(rc.role, RecipientRole::Recipient);
    assert_eq!(rc.first_time, Some(true));
    assert_eq!(rc.lookalike.as_ref().unwrap().label, "Alice");
    assert!(r
        .concerns
        .iter()
        .any(|c| matches!(c, Concern::Lookalike { .. })));
    assert!(r
        .concerns
        .iter()
        .any(|c| matches!(c, Concern::FirstTime { .. })));
    assert!(r
        .concerns
        .iter()
        .any(|c| matches!(c, Concern::ToContract { .. })));
    assert_eq!(r.tone, Tone::Danger);
    assert!(r.sign.anyway);
    assert!(rc.known.is_none());
    assert!(rc.first_time_cue && rc.contract_cue);
}

/// What the review says of one paid address with `facts`, known as `known`.
fn paid_to(to: &str, is_contract: bool, first_time: bool, known: Vec<Known>) -> MultisigReview {
    multisig_review(&ReviewInput {
        reads: reads(tx(to, "0x1", "0x", 0)),
        recipient_facts: vec![RecipientFacts {
            address: checksum(a(to)),
            is_contract: Some(is_contract),
            first_time: Some(first_time),
        }],
        known,
        sim_reply: Some(r#"{"unreachable":true}"#.into()),
        ..ReviewInput::default()
    })
    .unwrap()
}

fn says_first_time_or_contract(r: &MultisigReview) -> (bool, bool) {
    (
        r.concerns
            .iter()
            .any(|c| matches!(c, Concern::FirstTime { .. })),
        r.concerns
            .iter()
            .any(|c| matches!(c, Concern::ToContract { .. })),
    )
}

#[test]
fn own_accounts_and_members_are_never_strangers() {
    // r4a-18, r5reg-8: a Vela account of this device is a smart account the
    // person controls — no "first time", no "coins lost in a contract".
    let known = |kind| {
        vec![Known {
            address: B.to_lowercase(),
            label: "Bob".into(),
            kind,
        }]
    };
    let own = paid_to(B, true, true, known(KnownKind::Own));
    let rc = &own.recipients[0];
    assert_eq!(rc.known.as_ref().unwrap().kind, KnownKind::Own);
    assert_eq!(rc.known.as_ref().unwrap().label, "Bob");
    assert!(!rc.first_time_cue && !rc.contract_cue);
    assert_eq!(says_first_time_or_contract(&own), (false, false));
    // A member of the multisig: the same.
    let member = paid_to(B, true, true, known(KnownKind::Member));
    assert!(!member.recipients[0].first_time_cue && !member.recipients[0].contract_cue);
    assert_eq!(says_first_time_or_contract(&member), (false, false));
    // A contact is still somebody else's: both are said.
    let contact = paid_to(B, true, true, known(KnownKind::Contact));
    assert_eq!(says_first_time_or_contract(&contact), (true, true));
    // An unknown contract and an unknown plain account: said as before.
    assert_eq!(
        says_first_time_or_contract(&paid_to(B, true, true, vec![])),
        (true, true)
    );
    assert_eq!(
        says_first_time_or_contract(&paid_to(B, false, true, vec![])),
        (true, false)
    );
    assert_eq!(
        says_first_time_or_contract(&paid_to(B, false, false, vec![])),
        (false, false)
    );
}

#[test]
fn the_closest_known_wins() {
    let both = vec![
        Known {
            address: B.into(),
            label: "Bob (contact)".into(),
            kind: KnownKind::Contact,
        },
        Known {
            address: B.to_uppercase().replace("0X", "0x"),
            label: "Bob".into(),
            kind: KnownKind::Own,
        },
    ];
    let r = paid_to(B, true, true, both);
    assert_eq!(r.recipients[0].known.as_ref().unwrap().label, "Bob");
    assert_eq!(says_first_time_or_contract(&r), (false, false));
}

#[test]
fn a_member_being_added_is_sent_nothing() {
    // r5reg-16: nothing is sent to a new member — no "first time sending here".
    let r = multisig_review(&ReviewInput {
        reads: reads(tx(SAFE, "0x0", &add_owner(C, 1), 0)),
        recipient_facts: vec![RecipientFacts {
            address: checksum(a(C)),
            is_contract: Some(true),
            first_time: Some(true),
        }],
        sim_reply: Some(r#"{"unreachable":true}"#.into()),
        ..ReviewInput::default()
    })
    .unwrap();
    let rc = &r.recipients[0];
    assert_eq!(rc.role, RecipientRole::Member);
    assert!(!rc.first_time_cue && !rc.contract_cue);
    assert_eq!(says_first_time_or_contract(&r), (false, false));
}

#[test]
fn looks_like_is_start_and_end() {
    let x = "0x1234000000000000000000000000000000005678";
    assert!(looks_like(x, "0x1234aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa5678"));
    assert!(!looks_like(x, x), "the same address is not a lookalike");
    assert!(!looks_like(x, "0x1235aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa5678"));
    assert!(!looks_like(x, "0x1234aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa5679"));
    assert!(!looks_like(x, "not an address"));
}

#[test]
fn a_blind_call_signs_anyway() {
    let t = tx(
        "0x5555555555555555555555555555555555555555",
        "0x0",
        "0x12345678",
        0,
    );
    let r = multisig_review(&ReviewInput {
        reads: reads(t),
        trace: Some(diff(&[], &[(n(5), n(1))])),
        sim_reply: Some(r#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#.into()),
        blind: true,
        ..ReviewInput::default()
    })
    .unwrap();
    assert!(r.concerns.contains(&Concern::Blind));
    assert_eq!(
        r.sign,
        SignRule {
            allowed: true,
            anyway: true
        }
    );
}

#[test]
fn a_batch_simulates_each_call() {
    // MultiSendCallOnly: two native transfers.
    let leg = |to: &str, v: u64| {
        let mut l = vec![0u8];
        l.extend_from_slice(a(to).as_slice());
        l.extend_from_slice(&word(U256::from(v)));
        l.extend_from_slice(&word(U256::ZERO));
        l
    };
    let mut packed = leg(C, 1);
    packed.extend(leg(B, 2));
    let mut d = keccak256(b"multiSend(bytes)")[..4].to_vec();
    d.extend_from_slice(&word(U256::from(32)));
    d.extend_from_slice(&word(U256::from(packed.len())));
    d.extend_from_slice(&packed);
    d.resize(4 + 64 + packed.len().div_ceil(32) * 32, 0);
    let t = tx(MULTI, "0x0", &hex(&d), 1);
    let r = review_reads(&reads(t)).unwrap();
    assert_eq!(r.sim_calls.len(), 2);
    assert_eq!(r.sim_calls[1].value, "0x2");
    assert_eq!(r.recipients.len(), 2);
}

const MULTI: &str = crate::multisig::MULTI_SEND_CALL_ONLY;

// -- the gate --------------------------------------------------------------------

#[test]
fn the_gate_in_order() {
    let genesis = json!({"from": "genesis", "chainId": 100, "reason": {"kind": "module"}});
    let chain = json!({"from": "chain", "chainId": 100, "risk": {"kind": "module"}});
    let elsewhere = json!({"from": "genesis", "chainId": 8453, "reason": {"kind": "module"}});
    let gate = |findings: Option<Vec<Value>>, sign: Option<SignRule>| {
        signing_gate(&GateInput {
            chain_id: 100,
            findings,
            sign,
        })
    };
    assert_eq!(gate(None, None), Gate::Unchecked);
    assert_eq!(gate(Some(vec![]), None), Gate::Open);
    assert_eq!(gate(Some(vec![elsewhere.clone()]), None), Gate::Open);
    assert_eq!(
        gate(Some(vec![genesis.clone(), chain.clone()]), None),
        Gate::Refused {
            findings: vec![genesis]
        }
    );
    let blocked = Some(SignRule {
        allowed: false,
        anyway: false,
    });
    assert_eq!(gate(Some(vec![chain.clone()]), blocked), Gate::Blocked);
    assert_eq!(
        gate(Some(vec![chain.clone()]), None),
        Gate::Risk {
            findings: vec![chain]
        }
    );
    assert_eq!(
        gate(
            Some(vec![]),
            Some(SignRule {
                allowed: true,
                anyway: true
            })
        ),
        Gate::Open
    );
}

#[test]
fn call_json_doors() {
    let t = tx(SAFE, "0x0", &add_owner(C, 1), 0);
    let input = json!({
        "chainId": 100, "safe": SAFE, "tx": t, "owners": [A, B], "threshold": 1, "deployed": true,
        "trace": add_owner_trace(),
        "simReply": "{\"unreachable\":true}"
    });
    let out: Value = serde_json::from_str(
        &multisig::call_json("multisigReview", &json!({ "input": input }).to_string()).unwrap(),
    )
    .unwrap();
    assert_eq!(out["state"]["verdict"]["kind"], json!("declared"));
    assert_eq!(out["sign"]["allowed"], json!(true));
    let reads: Value = serde_json::from_str(
        &multisig::call_json("reviewReads", &json!({ "input": input }).to_string()).unwrap(),
    )
    .unwrap();
    assert_eq!(reads["trace"]["method"], json!("debug_traceCall"));
    let gate: Value = serde_json::from_str(
        &multisig::call_json(
            "signingGate",
            &json!({ "input": { "chainId": 100, "findings": null } }).to_string(),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(gate["kind"], json!("unchecked"));
}

#[test]
fn a_reverted_trace_is_never_clean() {
    // A traced execTransaction that reverted (a queued nonce, a guard, a
    // signature the override did not satisfy) leaves an empty diff: that is
    // no reading, not "nothing changes".
    let t = tx("0x4444444444444444444444444444444444444444", "0x1", "0x", 0);
    let r = review_with(t.clone(), Some(diff(&[], &[])), None);
    assert_eq!(r.state.source, StateSource::None);
    assert!(matches!(r.state.verdict, StateVerdict::Unavailable { .. }));
    // The probe, asked beside it, speaks instead — here, the Bybit write.
    let k = watch_of(&t, &[]);
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(k);
    let mut after = snap(&[A, B]);
    after.watched = zero_watch(k);
    after.singleton = a(ATTACKER);
    let r = review_with(
        t,
        Some(diff(&[], &[])),
        Some(probe_envelope(&before, &after, true)),
    );
    assert_eq!(r.state.source, StateSource::Probe);
    assert!(!r.sign.allowed);
}

#[test]
fn the_trace_runs_a_queued_proposal_at_its_own_number() {
    let mut t = tx(SAFE, "0x0", &add_owner(C, 1), 0);
    t.nonce = "0x7".into();
    let r = review_reads(&reads(t)).unwrap();
    let diff = &r.trace.unwrap().params[2]["stateOverrides"][checksum(a(SAFE))]["stateDiff"];
    assert_eq!(diff[s(slot(SLOT_NONCE))], json!(n(7)));
}

/// r10-6: a refusal for what it is never says "expected to fail" either —
/// a probe that reverted is withheld like one that changed nothing (deep4
/// still showed it on desktop and Android).
#[test]
fn a_refusal_withholds_a_revert_too() {
    let mut d = keccak256(b"transfer(address,uint256)")[..4].to_vec();
    d.extend_from_slice(&word_addr(a(C)));
    d.extend_from_slice(&word(U256::from(1)));
    let delegate = tx(
        "0x5555555555555555555555555555555555555555",
        "0x0",
        &hex(&d),
        1,
    );
    let k = watch_of(&delegate, &[]);
    let mut before = snap(&[A, B]);
    before.watched = zero_watch(k);
    let mut after = snap(&[A, B]);
    after.watched = zero_watch(k);
    let r = review_with(
        delegate,
        Some(not_offered_answer()),
        Some(probe_envelope(&before, &after, false)),
    );
    assert_eq!(r.refused.as_ref().unwrap().why, RefusedWhy::Delegatecall);
    assert_eq!(r.state.verdict, StateVerdict::Withheld);
    assert!(!r.concerns.iter().any(|c| matches!(c, Concern::Reverts)));
    assert_eq!(r.simulation.kind, "withheld");
}
