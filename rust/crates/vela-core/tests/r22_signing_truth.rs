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
        // §41 (P2-3): what each move is — an NFT and its id, a grant, a log
        // nobody could read — and what the sheet may say of it.
        if let Some(kinds) = expect.get("kinds") {
            let got_kinds: Vec<serde_json::Value> =
                got.deltas.iter().map(|d| json!(d.kind)).collect();
            assert_eq!(json!(got_kinds), *kinds, "{name}");
        }
        if let Some(ids) = expect.get("token_ids") {
            let got_ids: Vec<Option<&str>> =
                got.deltas.iter().map(|d| d.token_id.as_deref()).collect();
            assert_eq!(json!(got_ids), *ids, "{name}");
        }
        if let Some(key) = expect.get("no_change_key") {
            assert_eq!(json!(got.no_change_key), *key, "{name}");
        }
        if let Some(unlimited) = expect.get("grants_unlimited") {
            assert_eq!(
                json!(vela_core::app::sim_outcome::grants_unlimited(&got)),
                *unlimited,
                "{name}"
            );
        }
        if let Some(unread) = expect.get("changes_unread") {
            assert_eq!(
                json!(vela_core::app::sim_outcome::changes_unread(&got)),
                *unread,
                "{name}"
            );
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
    // §39 (r22-2): one provider vs two agreeing vs two disagreeing — the
    // member's own flow and a guard against a site.
    for case in v["authority"].as_array().expect("authority") {
        let name = case["name"].as_str().expect("name");
        for (flow, own) in [("own", true), ("site", false)] {
            let got = vela_core::app::corroborate::authority_read(&case["envelope"], own);
            assert_eq!(json!(got), case[flow], "{name} ({flow})");
        }
    }
    // §39: a site's fee shown vs the quote at the approve.
    for case in v["fee_rose"].as_array().expect("fee_rose") {
        let name = case["name"].as_str().expect("name");
        let quoted = |k: &str| -> Option<vela_core::app::sign_request::SignQuotedFee> {
            serde_json::from_value(case[k].clone()).expect("fee")
        };
        let got = vela_core::app::sign_request::fee_rose(
            quoted("shown").as_ref(),
            quoted("now").as_ref(),
        );
        assert_eq!(json!(got), case["rose"], "{name}");
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
        RPC_READ_TIMEOUT_MS, WITNESS_BUDGET_MS,
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

    fn probed_at(url: &str, reported: Option<u32>, now_ms: f64) -> Res {
        Res::ChainIdProbed {
            chain_id: CHAIN,
            url: url.to_owned(),
            reported,
            latency_ms: 10.0,
            now_ms,
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
                sole_provider: false,
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

    /// §39 (r22-2): Arc's chain index lists two hosts that answer nothing
    /// ahead of the one that does. The witness looks past them — every other
    /// provider, within its time budget, not two asks — and finds dRPC.
    #[test]
    fn a_witness_is_looked_for_past_dead_providers_within_the_budget() {
        const Q: &str = "https://rpc.quicknode.example";
        const BD: &str = "https://rpc.blockdaemon.example";
        const D: &str = "https://arc-testnet.drpc.example";
        let mut sut = Sut::new();
        sut.dispatch(call("c1", true));
        sut.resolve(config(
            vec![
                seed(A1, RpcSource::Default),
                seed(Q, RpcSource::Public),
                seed(BD, RpcSource::Public),
                seed(D, RpcSource::Public),
            ],
            false,
        ));
        sut.resolve(probed(A1, Some(CHAIN)));
        assert_eq!(sut.resolve(answered("c1", A1)), vec![probe(Q)]);
        // Two hosts that never say which chain they serve…
        assert_eq!(
            sut.resolve(probed_at(Q, None, T0 + 3_000.0)),
            vec![probe(BD)]
        );
        assert_eq!(
            sut.resolve(probed_at(BD, None, T0 + 6_000.0)),
            vec![probe(D)]
        );
        // …and the third provider answers: a witness.
        assert_eq!(
            sut.resolve(probed_at(D, Some(CHAIN), T0 + 6_100.0)),
            vec![post("c1", D)]
        );
        let ops = sut.resolve(answered("c1", D));
        assert_eq!(
            concluded(&ops),
            Some(RpcCallVerdict::Respond {
                url: A1.to_owned(),
                maybe_delivered: false,
                witness_url: Some(D.to_owned()),
                own_node: false,
                sole_provider: false,
            })
        );
        sut.resolve(Res::Concluded);
        // The next corroborated read asks the dead hosts last.
        sut.dispatch(call("c2", true));
        assert_eq!(sut.resolve(answered("c2", A1)), vec![post("c2", D)]);
    }

    /// §39: past the budget, the first answer goes back alone — and it is
    /// NOT "the only provider": others exist, they did not answer in time
    /// ("couldn't check", never "… anyway").
    #[test]
    fn past_the_budget_the_answer_is_alone_but_not_the_sole_provider() {
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
        sut.resolve(probed(A1, Some(CHAIN)));
        assert_eq!(sut.resolve(answered("c1", A1)), vec![probe(B)]);
        let ops = sut.resolve(probed_at(B, None, T0 + WITNESS_BUDGET_MS + 1.0));
        assert_eq!(
            concluded(&ops),
            Some(RpcCallVerdict::Respond {
                url: A1.to_owned(),
                maybe_delivered: false,
                witness_url: None,
                own_node: false,
                sole_provider: false,
            })
        );
    }

    /// Only one provider in the pool: the answer goes back alone — the core
    /// then believes no plain verdict on it. §39: and it says so — the pool
    /// holds no other provider (`sole_provider`), which only the member's
    /// own multisig flows may go on with, as "… anyway".
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
                sole_provider: true,
            })
        );
    }

    /// §39: a node proven another chain's is no provider of this one — the
    /// pool with it and one default holds a sole provider.
    #[test]
    fn a_wrong_chain_node_leaves_the_sole_provider() {
        let mut sut = Sut::new();
        sut.dispatch(call("c1", true));
        sut.resolve(config(
            vec![seed(MINE, RpcSource::User), seed(B, RpcSource::Default)],
            false,
        ));
        assert_eq!(sut.resolve(probed(MINE, Some(1))), vec![probe(B)]);
        sut.resolve(probed(B, Some(CHAIN)));
        let ops = sut.resolve(answered("c1", B));
        assert_eq!(
            concluded(&ops),
            Some(RpcCallVerdict::Respond {
                url: B.to_owned(),
                maybe_delivered: false,
                witness_url: None,
                own_node: false,
                sole_provider: true,
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
                sole_provider: false,
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
                sole_provider: false,
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
                sole_provider: false,
            })
        );
    }
}

/// §39 (R22 follow-up, S4, desktop): with "Only use my node" and a node that
/// dropped the logs, a drain of 0.2 native read "Balance changes / No asset
/// changes / Checked by your own node only" beside "Send −0.2". The value
/// the wallet decoded itself is never erased: it is drawn as the outflow.
#[test]
fn a_node_never_erases_the_value_the_wallet_decoded() {
    use vela_core::app::sim_outcome::{verdict_of_envelope, with_decoded_value};
    let user = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
    let nothing_moves = json!({
        "result": [{"number": "0x10", "calls": [{"status": "0x1", "returnData": "0x", "gasUsed": "0x5208", "logs": []}]}],
        "ownNode": true
    });
    let read = verdict_of_envelope(&nothing_moves, user);
    assert_eq!(read.kind, "deltas");
    assert!(
        read.deltas.is_empty() && read.no_change_key.is_some(),
        "the node's word: {read:?}"
    );
    let shown = with_decoded_value(read.clone(), Some("200000000000000000"));
    assert_eq!(
        shown.no_change_key, None,
        "never \"No asset changes\" beside a value sent"
    );
    assert_eq!(shown.deltas.len(), 1);
    assert_eq!(shown.deltas[0].token, None);
    assert_eq!(shown.deltas[0].delta, "-200000000000000000");
    assert_eq!(
        shown.source_key, read.source_key,
        "still said as the own node's check"
    );
    // No value: the node's word stands; hex reads the same as decimal.
    assert_eq!(with_decoded_value(read.clone(), Some("0x0")), read);
    assert_eq!(with_decoded_value(read.clone(), None), read);
    assert_eq!(
        with_decoded_value(read, Some("0x2c68af0bb140000")).deltas[0].delta,
        "-200000000000000000"
    );
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
    use vela_core::app::token_trust::{TrustAssetDelta, TrustDeltaKind};

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
        // Still checking: nothing judged yet — not "… anyway" (the gate
        // holds the confirm shut "Checking…" meanwhile).
        let mut checking = base.sign.clone();
        checking.sim_checking = true;
        let waiting = posture(&checking, &base.clear, None);
        assert!(!waiting.anyway && waiting.hold_ms == 0, "{waiting:?}");
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

    /// §41 (P2-6): a reading only the public selector database (4byte) or a
    /// fetched descriptor gave describes; it vouches for nothing. A plain
    /// press needs the simulation corroborated; otherwise "… anyway" (held,
    /// refused by Refuse blind signing). A signature has no simulation: such
    /// a reading of one is "… anyway". A built-in reading is unchanged.
    #[test]
    fn a_fetched_or_selector_db_reading_lifts_nothing_out_of_anyway() {
        use vela_core::app::sign_confirm::posture;
        let reading = |provenance: &str,
                       sign_type: &str|
         -> vela_core::app::clear_signing::ClearSigningView {
            let mut clear = input(ClearSurface::ClearSign, None, false).clear;
            clear.result = Some(
                serde_json::from_value(serde_json::json!({
                    "intent": "Contract call", "contract_name": null, "owner": null,
                    "fields": [], "risk": "caution", "contract_address": null,
                    "verified": false, "provenance": provenance, "sign_type": sign_type,
                    "partial": false, "best_effort": provenance == "selector_db",
                    "to_own_token": false
                }))
                .expect("a reading"),
            );
            clear
        };
        let base = input(ClearSurface::ClearSign, None, false);
        let ok = corroborated(SimOutcome::Deltas { deltas: vec![] });
        let unconfirmed = verdict_of(SimOutcome::Unconfirmed { disagreed: false });
        for provenance in ["selector_db", "fetched"] {
            let clear = reading(provenance, "transaction");
            let plain = posture(&base.sign, &clear, Some(&ok));
            assert!(
                !plain.anyway && plain.hold_ms == 0,
                "{provenance}: {plain:?}"
            );
            let held = posture(&base.sign, &clear, Some(&unconfirmed));
            assert!(
                held.anyway && held.hold_ms == HOLD_TO_CONFIRM_MS,
                "{provenance}"
            );
        }
        // A fetched descriptor reading a signature: nothing corroborates it.
        let signature = reading("fetched", "signature");
        let p = posture(&base.sign, &signature, None);
        assert!(p.anyway && p.hold_ms == HOLD_TO_CONFIRM_MS, "{p:?}");
        let mut input_refused = input(ClearSurface::ClearSign, None, true);
        input_refused.clear = signature;
        assert_eq!(
            confirm_state(&input_refused).block,
            Some(ConfirmBlock::BlindRefused)
        );
        // Vela's own reading of the same signature: a plain press.
        let built_in = posture(&base.sign, &reading("built_in", "signature"), None);
        assert!(!built_in.anyway && built_in.hold_ms == 0, "{built_in:?}");
    }

    /// §41 (P2-7): a transaction whose reading never started

    /// (`ClearSurface::None`) is not "not a transaction": the confirm stays
    /// shut "Reading…" until it reads, and its posture is not a plain press.
    #[test]
    fn a_transaction_whose_reading_never_started_is_unconfirmable() {
        use vela_core::app::sign_confirm::posture;
        use vela_core::app::sign_request::SignRequestView;
        let request = |method: &str, kind: &str| -> SignRequestView {
            serde_json::from_value(serde_json::json!({
                "id": "rid-1", "method": method, "kind": kind,
                "params_json": "[{\"to\":\"0x76875e38fc6bc2dedcaed807ce00782db5c0d141\",\"value\":\"0x1\"}]",
                "origin": "https://site.example", "dapp": null, "chain_id": 8453,
                "signer_address": null, "first_party": false
            }))
            .expect("a request view")
        };
        for (method, kind) in [
            ("eth_sendTransaction", "transaction"),
            ("wallet_sendCalls", "batch"),
        ] {
            let mut never = input(ClearSurface::None, None, false);
            never.sign.request = Some(request(method, kind));

            let state = confirm_state(&never);
            assert!(!state.enabled, "{method}: {state:?}");
            assert_eq!(state.block, Some(ConfirmBlock::Reading), "{method}");
            let p = posture(&never.sign, &never.clear, None);
            assert!(
                p.anyway && p.hold_ms == HOLD_TO_CONFIRM_MS,
                "{method}: {p:?}"
            );
        }
        // A message with nothing drawn is the reading's own matter, as before.
        let mut message = input(ClearSurface::None, None, false);
        message.sign.request = Some(request("personal_sign", "personal_sign"));
        assert_ne!(confirm_state(&message).block, Some(ConfirmBlock::Reading));
    }

    /// §41 (P2-3 / P2-4): what a corroborated answer says decides too — a log

    /// naming the account that nobody could read is "… anyway"; an unbounded
    /// grant the simulation found (whatever carried it) is a held danger; a
    /// bounded one is shown and is not.
    #[test]
    fn what_the_simulation_found_sets_the_posture() {
        use vela_core::app::sign_confirm::posture;
        let base = input(ClearSurface::ClearSign, None, false);
        let p = |deltas: Vec<TrustAssetDelta>| {
            posture(
                &base.sign,
                &base.clear,
                Some(&corroborated(SimOutcome::Deltas { deltas })),
            )
        };
        let delta = |kind: TrustDeltaKind, delta: &str, spender: Option<&str>| TrustAssetDelta {
            kind,
            token: Some("0x5af0d9827e0c53e4799bb226655a1de152a425a5".into()),
            delta: delta.into(),
            token_id: None,
            spender: spender.map(str::to_owned),
        };
        let thief = Some("0x76875e38fc6bc2dedcaed807ce00782db5c0d141");
        let unread = p(vec![delta(TrustDeltaKind::Unread, "1", None)]);
        assert!(
            unread.anyway && unread.hold_ms == HOLD_TO_CONFIRM_MS,
            "{unread:?}"
        );
        let all = p(vec![delta(
            TrustDeltaKind::Approval,
            &alloy_primitives::U256::MAX.to_string(),
            thief,
        )]);
        assert!(
            all.danger && !all.anyway && all.hold_ms == HOLD_TO_CONFIRM_MS,
            "{all:?}"
        );
        let bounded = p(vec![delta(TrustDeltaKind::Approval, "5000000", thief)]);
        assert!(!bounded.danger && bounded.hold_ms == 0, "{bounded:?}");
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

// ---------------------------------------------------------------------------
// r21-2: a fee is never shown below what is charged; Max sends what it shows
// ---------------------------------------------------------------------------

/// The Round 21 pairs (shown → charged, at 18 decimals): each fee now reads
/// UP at the row's six places.
#[test]
fn a_fee_reads_up_never_below_the_charge() {
    use vela_core::app::fee_policy::{fee_figure, fee_figure_text};
    let e18 = |s: &str| {
        let (i, f) = s.split_once('.').expect("a decimal");
        format!("{i}{f:0<18}")
            .trim_start_matches('0')
            .parse::<u128>()
            .expect("units")
    };
    for (charged, shown) in [
        ("0.1925565", "0.192557"),
        ("0.10236552", "0.102366"),
        ("0.018078621", "0.018079"),
        ("0.018078", "0.018078"),
    ] {
        assert_eq!(
            fee_figure(e18(charged), 18, 6).as_deref(),
            Some(shown),
            "{charged}"
        );
    }
    // A token fee at 4 places, a dust fee's two significant digits, up.
    assert_eq!(fee_figure(123_456, 6, 4).as_deref(), Some("0.1235"));
    assert_eq!(
        fee_figure(1_234, 18, 6).as_deref(),
        Some("0.0000000000000013")
    );
    assert_eq!(fee_figure(0, 18, 6).as_deref(), Some("0"));
    assert_eq!(
        fee_figure_text("1234567000000000000000", 18, 6).as_deref(),
        Some("1234.567")
    );
    assert_eq!(fee_figure_text("x", 18, 6), None);
}

/// Max: the figure shown is the amount sent — the sendable amount cut DOWN
/// to the field's places (r21-2: "Max 0.116999" charged 0.116999359).
#[test]
fn max_sends_exactly_what_it_shows() {
    use vela_core::app::send::{max_figure, max_shown_exact};
    for (exact, sent) in [
        ("0.116999359", "0.116999"),
        ("0.1169995", "0.116999"),
        ("1.23456789", "1.2345"),
        ("1234.5678", "1234.56"),
        ("0.000000123456", "0.00000012"),
        ("5", "5"),
        ("0.10", "0.1"),
    ] {
        let got = max_shown_exact(exact);
        assert_eq!(got, sent, "{exact}");
        assert_eq!(
            max_figure(&got),
            got,
            "the field shows exactly what is sent: {exact}"
        );
    }
}
