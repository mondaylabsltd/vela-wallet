//! Round 22, "signing truth" (docs/multisig-machines.md §36,
//! docs/security/hardening.md): the rules that keep a "plain" verdict from
//! resting on one source an adversary controls.
//!
//! - 1-4: only a description changing which would mean changing the app
//!   (BuiltIn, PinnedMatch) may grade a call green.
//! - 1-3: every check that could not be read is "… anyway"; a batch leg
//!   nobody reads makes the batch blind.

#![cfg(feature = "crux")]

mod support;

use serde_json::json;
use support::DomainDriver;
use vela_core::app::clear_signing::{
    ClearLocale, ClearOperation as Op, ClearProvenance, ClearRisk, ClearShellResult as Res,
    ClearSigning, Event,
};

type Sut = DomainDriver<ClearSigning>;

const NOW: f64 = 1_754_700_000_000.0;
/// A contract nobody built into the app.
const DROP: &str = "0xc1a1c1a1c1a1c1a1c1a1c1a1c1a1c1a1c1a1c1a1";

fn pad_u128(v: u128) -> String {
    format!("{v:064x}")
}

/// `claim(uint256)` — what a drainer calls its drain.
fn claim_data(id: u128) -> String {
    let selector = vela_core::abi::compute_selector("claim(uint256)").expect("selector");
    format!("{selector}{}", pad_u128(id))
}

/// An ERC-7730 descriptor for [`DROP`] whose words say "claim", as the
/// descriptor service would serve one (unauthenticated).
fn claim_descriptor(intent: &str) -> String {
    json!({
        "context": {"$id": "Rewards", "contract": {"deployments": [
            {"chainId": 1, "address": DROP}
        ]}},
        "metadata": {"owner": "Rewards Inc"},
        "display": {"formats": {"claim(uint256 id)": {
            "intent": intent,
            "fields": [{"label": "Reward", "path": "id"}]
        }}}
    })
    .to_string()
}

fn read_fetched(intent: &str) -> vela_core::app::clear_signing::ClearSignResult {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::ResolveTransaction {
        to: Some(DROP.to_owned()),
        data: Some(claim_data(7)),
        value: Some("0x0".to_owned()),
        chain_id: 1,
        locale: ClearLocale::default(),
        from: None,
        probe_code: false,
    });
    assert_eq!(ops, vec![Op::Now]);
    let ops = sut.resolve(Res::Clock { now_ms: NOW });
    assert_eq!(
        ops,
        vec![Op::HttpGet {
            path: format!("/erc7730/calldata/eip155-1/{DROP}.json"),
        }]
    );
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{DROP}.json"),
        json: Some(claim_descriptor(intent)),
    });
    sut.view().result.expect("read by the fetched descriptor")
}

/// Review 1-4: a fetched descriptor is anybody's words. "Claim", "deposit",
/// "stake" from one is never green — at most the plain grade.
#[test]
fn a_fetched_descriptor_saying_claim_is_never_graded_safe() {
    for intent in ["Claim", "Claim rewards", "Deposit", "Stake", "Supply"] {
        let result = read_fetched(intent);
        assert_eq!(result.provenance, ClearProvenance::Fetched, "{intent}");
        assert_ne!(result.risk, ClearRisk::Safe, "{intent}");
        assert_eq!(result.risk, ClearRisk::Normal, "{intent}");
    }
}

// ---------------------------------------------------------------------------
// 1-3: a batch leg nobody reads makes the batch blind
// ---------------------------------------------------------------------------

/// The shell that knows nothing: no descriptor, no selector, every probe
/// reverts, every timer fires.
fn answer_knowing_nothing(op: &Op) -> Res {
    use vela_core::app::clear_signing::ClearProbe;
    match op {
        Op::Now => Res::Clock { now_ms: NOW },
        Op::RpcGetCode { chain_id, address } => Res::RpcAnswer {
            probe: ClearProbe::Code,
            chain_id: *chain_id,
            to: address.clone(),
            result: None,
            rpc_error: false,
        },
        Op::HttpGet { path } => Res::DescriptorFetched {
            path: path.clone(),
            json: None,
        },
        Op::RpcEthCall {
            chain_id,
            to,
            probe,
            ..
        } => Res::RpcAnswer {
            probe: *probe,
            chain_id: *chain_id,
            to: to.clone(),
            result: None,
            rpc_error: true,
        },
        Op::SelectorDbLookup { .. } => Res::SelectorCandidates { sigs: vec![] },
        Op::Timer { token, .. } => Res::TimedOut { token: *token },
    }
}

fn batch_event(calls: serde_json::Value) -> Event {
    Event::ResolveBatch {
        params_json: json!([{
            "version": "2.0.0",
            "chainId": "0x1",
            "from": "0x88cca0eedbf2c4426110bbfc998f048689266894",
            "atomicRequired": true,
            "calls": calls,
        }])
        .to_string(),
        chain_id: 1,
        locale: ClearLocale::default(),
        from: None,
        probe_code: false,
    }
}

/// Review 1-3: one call of a `wallet_sendCalls` batch that nobody can read
/// makes the whole batch blind — "Sign anyway", never a plain Confirm beside
/// a row that says "unable to decode".
#[test]
fn a_batch_with_a_leg_nobody_reads_signs_only_anyway() {
    let mut sut = Sut::new();
    let unknown = format!("0xdeadbeef{}", pad_u128(7));
    sut.dispatch(batch_event(json!([
        { "to": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045", "value": "0x1" },
        { "to": "0x1234567890abcdef1234567890abcdef12345678", "data": unknown, "value": "0x0" },
    ])));
    for _ in 0..500 {
        let Some(op) = sut.outstanding().first().cloned() else {
            break;
        };
        sut.resolve(answer_knowing_nothing(&op));
    }
    let view = sut.view();
    assert_eq!(
        view.surface,
        vela_core::app::clear_signing::ClearSurface::Batch
    );
    assert!(view.sign_anyway, "a leg nobody reads: the batch is blind");

    // Two plain sends are read in full: no "… anyway".
    let mut sut = Sut::new();
    sut.dispatch(batch_event(json!([
        { "to": "0xd8da6bf26964af9d7eed9e03e53415d37aa96045", "value": "0x1" },
        { "to": "0x1234567890abcdef1234567890abcdef12345678", "value": "0x2" },
    ])));
    assert!(!sut.view().sign_anyway);
}

// ---------------------------------------------------------------------------
// 1-2 / 1-3: the multisig review on real fork answers
// ---------------------------------------------------------------------------

fn vector(name: &str) -> serde_json::Value {
    let path = format!(
        "{}/tests/vectors/multisig-review/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("vector")).expect("json")
}

fn review(input: &serde_json::Value) -> serde_json::Value {
    let out =
        vela_core::multisig::call_json("multisigReview", &json!({ "input": input }).to_string())
            .expect("multisigReview");
    serde_json::from_str(&out).expect("review json")
}

/// The answer as a second, independent node gave it: the same envelope.
fn witnessed(envelope: &serde_json::Value, witness: serde_json::Value) -> serde_json::Value {
    let mut e = envelope.clone();
    e["witness"] = witness;
    e
}

/// Review 1-3: in a multisig review, one of the two safety checks unread is
/// no longer a plain press — it signs only "… anyway". (The state diff is
/// read and corroborated here; only the simulation is out.)
#[test]
fn one_unread_check_of_two_signs_only_anyway() {
    let v = vector("fork-add-owner-trace");
    let mut input = v["input"].clone();
    input["trace"] = witnessed(&input["trace"], input["trace"].clone());
    // No probe (whose balances would say what moves) and no simulation.
    input["probe"] = json!({"error": {"code": -32601, "message": "not offered"}});
    input["simReply"] = json!(r#"{"unreachable":true}"#);
    let got = review(&input);
    assert_eq!(got["state"]["verdict"]["kind"], json!("declared"));
    assert_eq!(got["sign"]["allowed"], json!(true));
    assert_eq!(
        got["sign"]["anyway"],
        json!(true),
        "the simulation is unread"
    );
}

/// Review 1-2: without a second, independent node's word nothing on the
/// fork vector is believed — the state diff and what moves are unread, and
/// the review signs only "… anyway". The honest second node makes it plain.
#[test]
fn a_review_on_one_node_s_word_is_unread_and_two_agreeing_nodes_make_it_plain() {
    let v = vector("fork-add-owner-trace");
    let alone = review(&v["input"]);
    assert_eq!(alone["state"]["verdict"]["kind"], json!("unavailable"));
    assert_eq!(alone["state"]["verdict"]["reason"], json!("uncorroborated"));
    assert_eq!(alone["simulation"]["kind"], json!("unconfirmed"));
    assert_eq!(alone["sign"], json!({"allowed": true, "anyway": true}));

    let mut input = v["input"].clone();
    input["trace"] = witnessed(&input["trace"], input["trace"].clone());
    input["probe"] = witnessed(&input["probe"], input["probe"].clone());
    let reply: serde_json::Value =
        serde_json::from_str(input["simReply"].as_str().expect("simReply")).expect("reply");
    let mut both = reply.clone();
    both["witness"] = reply;
    input["simReply"] = json!(both.to_string());
    let plain = review(&input);
    assert_eq!(plain["state"], v["expect"]["state"]);
    assert_eq!(plain["simulation"]["kind"], json!("deltas"));
    assert_eq!(plain["simulation"]["source"], json!("corroborated"));
    assert_eq!(plain["sign"], json!({"allowed": true, "anyway": false}));
}

/// Review 1-2, the lie: a node rewrites the state diff to "nothing of the
/// multisig changes" (and the simulation to "no asset changes"); the honest
/// node beside it read the member change. They disagree — never a plain
/// verdict.
#[test]
fn a_lying_node_cannot_hide_the_honest_node_s_refusal() {
    // Bybit: the honest trace writes slot 0 (undeclared). A lying first node
    // answers a diff that moves only the nonce; the honest witness's
    // refusal stands — whichever of the two is asked first.
    let v = vector("fork-bybit-trace");
    let honest_trace = v["input"]["trace"].clone();
    let safe = "0x0e96ddc491878645db7e3a47d2a6ccbc94f17f32";
    let nonce = "0x0000000000000000000000000000000000000000000000000000000000000005";
    let lie = json!({"result": {
        "pre": {safe: {"storage": {nonce: format!("0x{:064x}", 0)}}},
        "post": {safe: {"storage": {nonce: format!("0x{:064x}", 1)}}}
    }});
    for (first, second) in [(&lie, &honest_trace), (&honest_trace, &lie)] {
        let mut input = v["input"].clone();
        input["trace"] = witnessed(first, second.clone());
        let got = review(&input);
        assert_eq!(
            got["state"]["verdict"]["kind"],
            json!("undeclared"),
            "{got}"
        );
        assert_eq!(got["sign"]["allowed"], json!(false), "{got}");
    }
}

#[test]
fn a_lying_node_beside_an_honest_one_never_yields_a_plain_verdict() {
    let v = vector("fork-add-owner-trace");
    let honest_trace = v["input"]["trace"].clone();
    // The diff with the owner writes removed: only the nonce moves.
    let mut lie = honest_trace.clone();
    let safe = "0x0e96ddc491878645db7e3a47d2a6ccbc94f17f32";
    for side in ["pre", "post"] {
        if let Some(storage) = lie["result"][side][safe]["storage"].as_object_mut() {
            storage.retain(|slot, _| {
                slot == "0x0000000000000000000000000000000000000000000000000000000000000005"
            });
        }
    }
    for (first, second) in [(&lie, &honest_trace), (&honest_trace, &lie)] {
        let mut input = v["input"].clone();
        input["trace"] = witnessed(first, second.clone());
        input["probe"] = json!({"error": {"code": -32601, "message": "not offered"}});
        input["simReply"] = json!(r#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#);
        let got = review(&input);
        assert_ne!(
            got["sign"],
            json!({"allowed": true, "anyway": false}),
            "{got}"
        );
        assert_ne!(got["state"]["verdict"]["kind"], json!("unchanged"), "{got}");
        assert_ne!(got["state"]["verdict"]["kind"], json!("declared"), "{got}");
    }
}

// ---------------------------------------------------------------------------
// The shared vectors (`tests/vectors/rules/signing-truth.json`)
// ---------------------------------------------------------------------------

fn truth_vectors() -> serde_json::Value {
    let path = format!(
        "{}/tests/vectors/rules/signing-truth.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("vectors")).expect("json")
}

#[test]
fn the_signing_truth_vectors_hold() {
    use vela_core::app::corroborate::provider_of;
    use vela_core::app::parent_safe_guard::{classify_corroborated, SafeOwnerAnswer};
    use vela_core::app::sim_outcome::verdict_of_envelope;
    let v = truth_vectors();
    let user = v["user"].as_str().expect("user");
    let sim = v["sim"].as_array().expect("sim");
    assert!(sim.len() >= 12);
    for case in sim {
        let name = case["name"].as_str().expect("name");
        let got = verdict_of_envelope(&case["envelope"], user);
        let expect = &case["expect"];
        assert_eq!(json!(got.kind), expect["kind"], "{name}");
        assert_eq!(json!(got.source), expect["source"], "{name}");
        assert_eq!(json!(got.notice_key), expect["notice_key"], "{name}");
        let deltas: Vec<&str> = got.deltas.iter().map(|d| d.delta.as_str()).collect();
        assert_eq!(json!(deltas), expect["deltas"], "{name}");
        if let Some(key) = expect.get("source_key") {
            assert_eq!(json!(got.source_key), *key, "{name}");
        }
        // The wasm/UniFFI export reads the same envelope as text.
        let text = vela_core::app::sim_outcome::verdict_json(user, &case["envelope"].to_string());
        let again: serde_json::Value = serde_json::from_str(&text).expect("verdict json");
        assert_eq!(again["kind"], expect["kind"], "{name}");
    }
    for case in v["owner"].as_array().expect("owner") {
        let name = case["name"].as_str().expect("name");
        let answer: SafeOwnerAnswer =
            serde_json::from_value(case["answer"].clone()).expect("answer");
        let witness: Option<SafeOwnerAnswer> =
            serde_json::from_value(case["witness"].clone()).expect("witness");
        let got = classify_corroborated(
            &answer,
            witness.as_ref(),
            case["own_node"].as_bool().expect("own"),
        );
        assert_eq!(json!(got), case["reading"], "{name}");
    }
    for case in v["provider"].as_array().expect("provider") {
        let url = case["url"].as_str().expect("url");
        assert_eq!(json!(provider_of(url)), case["provider"], "{url}");
    }
}

// ---------------------------------------------------------------------------
// The pool: a corroborated read asks two independent, chain-checked nodes
// ---------------------------------------------------------------------------

mod pool {
    use super::support::DomainDriver;
    use vela_core::app::rpc_pool::{
        Event, RpcCallVerdict, RpcEndpointSeed, RpcKind, RpcOperation as Op, RpcPool,
        RpcShellResult as Res, RpcSource, RpcTransportOutcome as Out, PING_TIMEOUT_MS,
        RPC_READ_TIMEOUT_MS,
    };

    type Sut = DomainDriver<RpcPool>;
    const CHAIN: u32 = 56;
    const T0: f64 = 1_700_000_000_000.0;
    const A1: &str = "https://a1.alpha.example/rpc";
    const A2: &str = "https://a2.alpha.example/rpc";
    const B: &str = "https://rpc.beta.example";
    const C: &str = "https://rpc.gamma.example";
    const MINE: &str = "http://127.0.0.1:9701";

    fn seed(url: &str, source: RpcSource) -> RpcEndpointSeed {
        RpcEndpointSeed {
            url: url.to_owned(),
            source,
        }
    }

    fn call(id: &str, corroborate: bool) -> Event {
        Event::CallRequested {
            call_id: id.to_owned(),
            chain_id: CHAIN,
            kind: RpcKind::Rpc,
            method: "eth_simulateV1".to_owned(),
            now_ms: T0,
            person: false,
            corroborate,
        }
    }

    fn config(rpc: Vec<RpcEndpointSeed>, own_only: bool) -> Res {
        Res::PoolConfig {
            chain_id: CHAIN,
            rpc_endpoints: rpc,
            bundler_endpoints: vec![],
            own_only,
            now_ms: T0,
        }
    }

    fn probe(url: &str) -> Op {
        Op::ProbeChainId {
            chain_id: CHAIN,
            url: url.to_owned(),
            timeout_ms: PING_TIMEOUT_MS,
        }
    }

    fn probed(url: &str, reported: Option<u32>) -> Res {
        Res::ChainIdProbed {
            chain_id: CHAIN,
            url: url.to_owned(),
            reported,
            latency_ms: 10.0,
            now_ms: T0,
        }
    }

    fn post(id: &str, url: &str) -> Op {
        Op::JsonRpcPost {
            call_id: id.to_owned(),
            url: url.to_owned(),
            method: "eth_simulateV1".to_owned(),
            x_rpc_url: None,
            timeout_ms: RPC_READ_TIMEOUT_MS,
            must_send: false,
        }
    }

    fn answered(id: &str, url: &str) -> Res {
        Res::PostOutcome {
            call_id: id.to_owned(),
            url: url.to_owned(),
            outcome: Out::Response { error: None },
            latency_ms: 20.0,
            now_ms: T0,
        }
    }

    fn concluded(ops: &[Op]) -> Option<RpcCallVerdict> {
        ops.iter().find_map(|op| match op {
            Op::Conclude { verdict, .. } => Some(verdict.clone()),
            _ => None,
        })
    }

    /// The first answer, then a second from ANOTHER provider (`a2` is the
    /// same provider as `a1` and is passed over), each endpoint checked for
    /// its chain id first; never hedged.
    #[test]
    fn a_corroborated_read_asks_a_second_independent_chain_checked_node() {
        let mut sut = Sut::new();
        assert_eq!(
            sut.dispatch(call("c1", true)),
            vec![Op::LoadPoolConfig { chain_id: CHAIN }]
        );
        let ops = sut.resolve(config(
            vec![
                seed(A1, RpcSource::Default),
                seed(A2, RpcSource::Default),
                seed(B, RpcSource::Public),
            ],
            false,
        ));
        assert_eq!(ops, vec![probe(A1)]);
        assert!(!sut.view().pending_urls.contains_key("c1"), "never hedged");
        assert_eq!(sut.resolve(probed(A1, Some(CHAIN))), vec![post("c1", A1)]);
        assert_eq!(sut.resolve(answered("c1", A1)), vec![probe(B)]);
        assert_eq!(sut.resolve(probed(B, Some(CHAIN))), vec![post("c1", B)]);
        let ops = sut.resolve(answered("c1", B));
        assert_eq!(
            concluded(&ops),
            Some(RpcCallVerdict::Respond {
                url: A1.to_owned(),
                maybe_delivered: false,
                witness_url: Some(B.to_owned()),
                own_node: false,
            })
        );
    }

    /// An endpoint that names another chain is no source, and is never asked
    /// anything about this one again (an ordinary read included).
    #[test]
    fn a_node_of_another_chain_is_never_a_source_nor_asked_again() {
        let mut sut = Sut::new();
        sut.dispatch(call("c1", true));
        sut.resolve(config(
            vec![
                seed(A1, RpcSource::Default),
                seed(B, RpcSource::Public),
                seed(C, RpcSource::Public),
            ],
            false,
        ));
        // a1 serves chain 1: skipped; b is asked.
        assert_eq!(sut.resolve(probed(A1, Some(1))), vec![probe(B)]);
        assert_eq!(sut.resolve(probed(B, Some(CHAIN))), vec![post("c1", B)]);
        // The witness is c, never a1.
        assert_eq!(sut.resolve(answered("c1", B)), vec![probe(C)]);
        assert_eq!(sut.resolve(probed(C, Some(CHAIN))), vec![post("c1", C)]);
        sut.resolve(answered("c1", C));
        sut.resolve(Res::Concluded);
        // An ordinary read skips a1 too.
        let ops = sut.dispatch(call("c2", false));
        assert_eq!(ops, vec![post("c2", B)]);
    }

    /// Only one provider in the pool: the answer goes back alone — the core
    /// then believes no plain verdict on it.
    #[test]
    fn one_provider_alone_is_no_witness() {
        let mut sut = Sut::new();
        sut.dispatch(call("c1", true));
        sut.resolve(config(
            vec![seed(A1, RpcSource::Default), seed(A2, RpcSource::Default)],
            false,
        ));
        sut.resolve(probed(A1, Some(CHAIN)));
        let ops = sut.resolve(answered("c1", A1));
        assert_eq!(
            concluded(&ops),
            Some(RpcCallVerdict::Respond {
                url: A1.to_owned(),
                maybe_delivered: false,
                witness_url: None,
                own_node: false,
            })
        );
    }

    /// "Only use my node": the person's node alone, trusted alone, and said.
    #[test]
    fn only_my_node_is_trusted_alone() {
        let mut sut = Sut::new();
        sut.dispatch(call("c1", true));
        sut.resolve(config(
            vec![seed(MINE, RpcSource::User), seed(B, RpcSource::Public)],
            true,
        ));
        assert_eq!(
            sut.resolve(probed(MINE, Some(CHAIN))),
            vec![post("c1", MINE)]
        );
        let ops = sut.resolve(answered("c1", MINE));
        assert_eq!(
            concluded(&ops),
            Some(RpcCallVerdict::Respond {
                url: MINE.to_owned(),
                maybe_delivered: false,
                witness_url: None,
                own_node: true,
            })
        );
    }

    /// The person's own node, and one default beside it (not own-only).
    #[test]
    fn my_node_and_one_default() {
        let mut sut = Sut::new();
        sut.dispatch(call("c1", true));
        sut.resolve(config(
            vec![seed(MINE, RpcSource::User), seed(B, RpcSource::Default)],
            false,
        ));
        sut.resolve(probed(MINE, Some(CHAIN)));
        assert_eq!(sut.resolve(answered("c1", MINE)), vec![probe(B)]);
        sut.resolve(probed(B, Some(CHAIN)));
        let ops = sut.resolve(answered("c1", B));
        assert_eq!(
            concluded(&ops),
            Some(RpcCallVerdict::Respond {
                url: MINE.to_owned(),
                maybe_delivered: false,
                witness_url: Some(B.to_owned()),
                own_node: false,
            })
        );
    }

    /// An ordinary read is exactly as before: no probe, no witness.
    #[test]
    fn an_ordinary_read_is_unchanged() {
        let mut sut = Sut::new();
        sut.dispatch(call("c1", false));
        let ops = sut.resolve(config(
            vec![seed(A1, RpcSource::Default), seed(B, RpcSource::Public)],
            false,
        ));
        assert_eq!(ops, vec![post("c1", A1)]);
        let ops = sut.resolve(answered("c1", A1));
        assert_eq!(
            concluded(&ops),
            Some(RpcCallVerdict::Respond {
                url: A1.to_owned(),
                maybe_delivered: false,
                witness_url: None,
                own_node: false,
            })
        );
    }
}

// ---------------------------------------------------------------------------
// 1-3: the confirm's friction, and "Refuse blind signing"
// ---------------------------------------------------------------------------

mod posture {
    use super::support::DomainDriver;
    use vela_core::app::approval_guard::ApprovalGuard;
    use vela_core::app::clear_signing::{ClearSigning, ClearSurface};
    use vela_core::app::sign_confirm::{
        confirm_state, ConfirmBlock, ConfirmInput, BLIND_REFUSED_KEY, HOLD_TO_CONFIRM_MS,
    };
    use vela_core::app::sign_request::SignRequest;
    use vela_core::app::sim_outcome::{verdict_of, SimOutcome, SimVerdict};

    fn input(surface: ClearSurface, sim: Option<SimVerdict>, refuse_blind: bool) -> ConfirmInput {
        let mut sign = DomainDriver::<SignRequest>::new().view();
        sign.confirm_gate_open = true;
        sign.confirm_block = None;
        let mut clear = DomainDriver::<ClearSigning>::new().view();
        clear.resolving = false;
        clear.surface = surface;
        // As the machine's view says it for these surfaces.
        clear.sign_anyway = matches!(
            surface,
            ClearSurface::BlindTransaction | ClearSurface::BlindTypedData
        );
        ConfirmInput {
            sign,
            guard: DomainDriver::<ApprovalGuard>::new().view(),
            clear,
            // A typed-data signature has no fee; for a transaction the gate
            // waits for one, so these read the posture on a message-like
            // surface and on a transaction's own fields.
            fee: None,
            speed_tier: None,
            message_unread: false,
            sim,
            refuse_blind,
        }
    }

    fn corroborated(outcome: SimOutcome) -> SimVerdict {
        let mut v = verdict_of(outcome);
        v.source = Some("corroborated".into());
        v
    }

    /// A message read in full: a plain press. Blind typed data: "… anyway",
    /// held. Refusing blind signing: refused, said.
    #[test]
    fn a_blind_signature_is_held_and_refusable() {
        let plain = confirm_state(&input(ClearSurface::MessageSign, None, false));
        assert!(
            plain.enabled && !plain.anyway && plain.hold_ms == 0,
            "{plain:?}"
        );

        let blind = confirm_state(&input(ClearSurface::BlindTypedData, None, false));
        assert!(blind.enabled && blind.anyway, "{blind:?}");
        assert_eq!(blind.hold_ms, HOLD_TO_CONFIRM_MS);

        let refused = confirm_state(&input(ClearSurface::BlindTypedData, None, true));
        assert!(!refused.enabled);
        assert_eq!(refused.block, Some(ConfirmBlock::BlindRefused));
        assert_eq!(refused.key.as_deref(), Some(BLIND_REFUSED_KEY));
        // A message read in full is never refused by it.
        assert!(confirm_state(&input(ClearSurface::MessageSign, None, true)).enabled);
        // eth_sign: a danger, held.
        let eth_sign = confirm_state(&input(ClearSurface::EthSign, None, false));
        assert!(eth_sign.danger && eth_sign.hold_ms == HOLD_TO_CONFIRM_MS);
    }

    /// A transaction's simulation: a corroborated answer is plain; a revert,
    /// an unconfirmed answer, none at all are "… anyway" (a revert a danger
    /// too) — every one held.
    #[test]
    fn a_transaction_is_plain_only_on_a_corroborated_simulation() {
        use vela_core::app::sign_confirm::posture;
        let base = input(ClearSurface::ClearSign, None, false);
        let p = |sim: Option<SimVerdict>| posture(&base.sign, &base.clear, sim.as_ref());
        let ok = p(Some(corroborated(SimOutcome::Deltas { deltas: vec![] })));
        assert!(!ok.anyway && !ok.danger && ok.hold_ms == 0, "{ok:?}");
        let reverts = p(Some(verdict_of(SimOutcome::Reverts { reason: None })));
        assert!(reverts.anyway && reverts.danger && reverts.hold_ms == HOLD_TO_CONFIRM_MS);
        for unread in [
            Some(verdict_of(SimOutcome::Unconfirmed { disagreed: false })),
            Some(verdict_of(SimOutcome::Unconfirmed { disagreed: true })),
            Some(verdict_of(SimOutcome::NotOffered)),
            Some(verdict_of(SimOutcome::Unreachable)),
            None,
        ] {
            let got = p(unread.clone());
            assert!(
                got.anyway && got.hold_ms == HOLD_TO_CONFIRM_MS,
                "{unread:?}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// The multisig door: held, and refusable
// ---------------------------------------------------------------------------

#[test]
fn the_multisig_door_holds_anyway_and_refuses_it_when_asked() {
    use vela_core::app::multisig_review_state::{door_with, ReviewState, BLIND_REFUSED_KEY};
    use vela_core::app::sign_confirm::HOLD_TO_CONFIRM_MS;
    let anyway = json!({"sign": {"allowed": true, "anyway": true}, "tone": "caution"});
    let plain = json!({"sign": {"allowed": true, "anyway": false}, "tone": "ok"});
    let a = door_with(Some(&ReviewState::Ready), Some(&anyway), false);
    assert!(a.enabled && a.anyway && a.hold_ms == HOLD_TO_CONFIRM_MS);
    let p = door_with(Some(&ReviewState::Ready), Some(&plain), false);
    assert!(p.enabled && !p.anyway && p.hold_ms == 0);
    let r = door_with(Some(&ReviewState::Ready), Some(&anyway), true);
    assert!(!r.enabled);
    assert_eq!(r.reason_key.as_deref(), Some(BLIND_REFUSED_KEY));
    assert!(door_with(Some(&ReviewState::Ready), Some(&plain), true).enabled);
    // Through the rule shells call.
    let out = vela_core::multisig_link::call_json(
        "review.door",
        &json!({"state": {"kind": "ready"}, "review": anyway, "refuseBlind": true}).to_string(),
    )
    .expect("door");
    let out: serde_json::Value = serde_json::from_str(&out).expect("json");
    assert_eq!(out["enabled"], json!(false));
    assert_eq!(out["reason_key"], json!(BLIND_REFUSED_KEY));
}

#[test]
fn refuse_blind_signing_is_off_until_turned_on() {
    use vela_core::prefs::{refuse_blind_signing, REFUSE_BLIND_SIGNING_DEFAULT};
    const { assert!(!REFUSE_BLIND_SIGNING_DEFAULT) };
    assert!(!refuse_blind_signing(None));
    assert!(!refuse_blind_signing(Some("off")));
    assert!(!refuse_blind_signing(Some("yes please")));
    assert!(refuse_blind_signing(Some("on")));
}

// ---------------------------------------------------------------------------
// 1-5: a site's request is hashed only as it was reviewed
// ---------------------------------------------------------------------------

mod attest {
    use serde_json::json;
    use vela_core::signed_review::{
        dapp_message_hash, dapp_review_of, dapp_safe_op_hash, DappReview,
    };
    use vela_core::user_op::{
        build_execute_call_data, build_in_band_fee_leg, build_native_call_data, MultiSendCall,
        UserOperation,
    };

    const SAFE: &str = "0x1100000000000000000000000000000000045aC0";
    const DAPP: &str = "0x2222222222222222222222222222222222222222";
    const RELAY: &str = "0x9999999999999999999999999999999999999999";
    const ATTACKER: &str = "0xbad0000000000000000000000000000000000bad";
    const USDC: &str = "0x3600000000000000000000000000000000000000";

    fn op(call_data: Vec<u8>) -> UserOperation {
        UserOperation {
            sender: SAFE.to_owned(),
            nonce: "0x1".to_owned(),
            init_code: Vec::new(),
            call_data,
            verification_gas_limit: 1,
            call_gas_limit: 1,
            pre_verification_gas: 1,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            paymaster_and_data: Vec::new(),
            valid_after: 0,
            valid_until: 1_800_000_000,
            signature: Vec::new(),
        }
    }

    fn data(hex: &str) -> Vec<u8> {
        vela_core::primitives::from_hex(hex).expect("hex")
    }

    fn review_of_tx(data_hex: &str) -> DappReview {
        let params =
            json!([{ "from": SAFE, "to": DAPP, "value": "0x10", "data": data_hex }]).to_string();
        dapp_review_of("eth_sendTransaction", &params, 100, SAFE, Some(RELAY)).expect("review")
    }

    fn call(to: &str, value: &str, d: &str) -> MultiSendCall {
        MultiSendCall {
            to: to.to_owned(),
            value_hex: value.to_owned(),
            data: data(d),
        }
    }

    #[test]
    fn a_transaction_is_hashed_only_as_reviewed() {
        let review = review_of_tx("0xa9059cbb0001");
        // The call alone, and the call with its fee leg to the relay.
        let alone = build_execute_call_data(DAPP, "0x10", &data("0xa9059cbb0001")).expect("cd");
        assert!(dapp_safe_op_hash(&op(alone), 100, &review).is_ok());
        let with_fee = build_native_call_data(
            &[
                call(DAPP, "0x10", "0xa9059cbb0001"),
                build_in_band_fee_leg(Some(USDC), RELAY, 5).expect("fee"),
            ],
            false,
        )
        .expect("cd");
        assert!(dapp_safe_op_hash(&op(with_fee), 100, &review).is_ok());
        // Another calldata, another value, another chain, another leg: refused.
        let drifted = build_execute_call_data(DAPP, "0x10", &data("0xa9059cbb0002")).expect("cd");
        assert!(dapp_safe_op_hash(&op(drifted), 100, &review).is_err());
        let more = build_execute_call_data(DAPP, "0x11", &data("0xa9059cbb0001")).expect("cd");
        assert!(dapp_safe_op_hash(&op(more), 100, &review).is_err());
        let alone = build_execute_call_data(DAPP, "0x10", &data("0xa9059cbb0001")).expect("cd");
        assert!(dapp_safe_op_hash(&op(alone), 1, &review).is_err());
        let stolen = build_native_call_data(
            &[
                call(DAPP, "0x10", "0xa9059cbb0001"),
                build_in_band_fee_leg(Some(USDC), ATTACKER, 5).expect("leg"),
            ],
            false,
        )
        .expect("cd");
        assert!(dapp_safe_op_hash(&op(stolen), 100, &review).is_err());
    }

    #[test]
    fn a_batch_is_hashed_only_as_reviewed_in_order() {
        let params = json!([{
            "version": "2.0.0", "chainId": "0x64", "from": SAFE, "atomicRequired": true,
            "calls": [
                { "to": DAPP, "value": "0x1", "data": "0x01" },
                { "to": USDC, "value": "0x0", "data": "0x02" },
            ]
        }])
        .to_string();
        let review =
            dapp_review_of("wallet_sendCalls", &params, 100, SAFE, Some(RELAY)).expect("r");
        let ok = build_native_call_data(
            &[call(DAPP, "0x1", "0x01"), call(USDC, "0x0", "0x02")],
            false,
        )
        .expect("cd");
        assert!(dapp_safe_op_hash(&op(ok), 100, &review).is_ok());
        let swapped = build_native_call_data(
            &[call(USDC, "0x0", "0x02"), call(DAPP, "0x1", "0x01")],
            false,
        )
        .expect("cd");
        assert!(dapp_safe_op_hash(&op(swapped), 100, &review).is_err());
        let dropped = build_execute_call_data(DAPP, "0x1", &data("0x01")).expect("cd");
        assert!(dapp_safe_op_hash(&op(dropped), 100, &review).is_err());
    }

    #[test]
    fn a_signature_wraps_only_the_digest_reviewed() {
        let typed = json!({
            "types": {"EIP712Domain": [{"name": "name", "type": "string"}], "Mail": [{"name": "text", "type": "string"}]},
            "primaryType": "Mail",
            "domain": {"name": "Ether Mail"},
            "message": {"text": "hello"}
        });
        let params = json!([SAFE, typed]).to_string();
        let review = dapp_review_of("eth_signTypedData_v4", &params, 100, SAFE, None).expect("r");
        let digest =
            vela_core::sign_message::original_hash("eth_signTypedData_v4", &params).expect("d");
        assert_eq!(
            dapp_message_hash(&digest, SAFE, &review).expect("ok"),
            digest
        );
        let mut other = typed.clone();
        other["message"]["text"] = json!("goodbye");
        let other_params = json!([SAFE, other]).to_string();
        let swapped = vela_core::sign_message::original_hash("eth_signTypedData_v4", &other_params)
            .expect("d");
        assert!(dapp_message_hash(&swapped, SAFE, &review).is_err());
        assert!(dapp_message_hash(&digest, DAPP, &review).is_err());
        // A transaction's review never wraps a signature.
        assert!(dapp_message_hash(&digest, SAFE, &review_of_tx("0x")).is_err());
    }
}
