//! PR 3, fix C — the confirm waits for the simulation's verdict.
//!
//! The balance changes are the one part of a signing sheet the site being
//! signed for cannot write. Until this fix the confirm gate looked at the
//! request, the reading, the approval guard and the fee, and not at the
//! simulation: a person could confirm before the verdict was on screen.
//!
//! One test per rule: held until the verdict; released on the verdict;
//! released at the deadline as "could not check"; never held where no
//! simulation exists; and the wait can neither be skipped by a stale tap nor
//! be left standing by a request that went away.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::approval_guard::ApprovalGuard;
use vela_core::app::clear_signing::{ClearSigning, ClearSurface};
use vela_core::app::fee_policy::{FeeAssetView, FeeEstimateView, FeePolicy, FeeTier};
use vela_core::app::sign_confirm::{
    confirm_state, ConfirmBlock, ConfirmInput, ConfirmState, SIM_CHECKING_KEY,
};
use vela_core::app::sign_request::{
    Event, SignAccountRef, SignApproveOpts, SignOperation as Op, SignRequest,
    SignShellResult as Res, SignView, SIM_VERDICT_WAIT_MS,
};
use vela_core::app::sim_outcome::KEY_UNAVAILABLE;

type Sut = DomainDriver<SignRequest>;

const ACCT: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const PEER: &str = "0x3333333333333333333333333333333333333333";
const NOW: f64 = 1_700_000_000_000.0;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A machine with a network and an account — the shell's boot sequence.
fn boot() -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::NetworksChanged { chain_ids: vec![1] });
    sut.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: ACCT.to_owned(),
            credential_id: "cred-0".to_owned(),
        }],
        active_index: 0,
    });
    sut
}

fn arrive(sut: &mut Sut, id: &str, method: &str, params_json: &str) {
    let ops = sut.dispatch(Event::RequestArrived {
        id: id.to_owned(),
        method: method.to_owned(),
        params_json: params_json.to_owned(),
        origin: "https://dapp.example".to_owned(),
        transport_id: "wp-1".to_owned(),
        dedicated_transport: false,
        per_request_chain: Some(1),
        dapp: None,
        granted_address: None,
        requested_address: None,
        request_ts_ms: None,
        now_ms: NOW,
        first_party: false,
    });
    assert!(ops.is_empty(), "an arrival asks the shell nothing: {ops:?}");
}

/// A transaction on the sheet: a plain send of 1 coin.
fn arrive_tx(sut: &mut Sut, id: &str) {
    let params = format!(r#"[{{"to":"{PEER}","data":"0x","value":"0xde0b6b3a7640000"}}]"#);
    arrive(sut, id, "eth_sendTransaction", &params);
}

fn started(sut: &mut Sut, id: &str) -> Vec<Op> {
    sut.dispatch(Event::SimStarted { id: id.to_owned() })
}

fn settled(sut: &mut Sut, id: &str) -> Vec<Op> {
    sut.dispatch(Event::SimSettled { id: id.to_owned() })
}

/// The deadline of the wait `round` of request `id` passes.
fn deadline(sut: &mut Sut, id: &str, round: u32) -> Vec<Op> {
    let wanted = (id.to_owned(), round);
    sut.resolve_matching(
        move |op| {
            matches!(op, Op::SimVerdictTimer { id, round, .. } if (id.clone(), *round) == wanted)
        },
        Res::SimVerdictTimerFired {
            id: id.to_owned(),
            round,
        },
    )
}

/// The one gate over this request's view, with every OTHER part ready: a
/// calm guard, an on-chain reading and a priced fee. What it says is then
/// the simulation's doing alone.
fn gate(sign: SignView) -> ConfirmState {
    let guard = DomainDriver::<ApprovalGuard>::new().view();
    let mut clear = DomainDriver::<ClearSigning>::new().view();
    clear.resolving = false;
    clear.surface = ClearSurface::BlindTransaction;
    clear.result = None;
    let mut fee = DomainDriver::<FeePolicy>::new().view();
    fee.busy = false;
    fee.failed = None;
    fee.fee = Some(FeeEstimateView {
        chain_id: 1,
        total_wei: "1".to_owned(),
        max_fee_per_gas: "1".to_owned(),
        network_fee_per_gas: "1".to_owned(),
        relayer_fee_per_gas: "0".to_owned(),
        bundler_gas_price: "1".to_owned(),
        in_band_gas_basis: "1".to_owned(),
        effective_gas_price: None,
        max_gas_price: None,
        total_gas: "1".to_owned(),
        deployed: true,
        tier: FeeTier::Fast,
        quoted: true,
        fee_asset: FeeAssetView::Native,
        fee_recipient: None,
    });
    fee.confirm_fee_ready = true;
    confirm_state(&ConfirmInput {
        sign,
        guard,
        clear,
        fee: Some(fee),
        speed_tier: Some(FeeTier::Fast),
    })
}

fn held() -> ConfirmState {
    ConfirmState {
        enabled: false,
        block: Some(ConfirmBlock::SimChecking),
        key: Some(SIM_CHECKING_KEY.to_owned()),
    }
}

fn open() -> ConfirmState {
    ConfirmState {
        enabled: true,
        block: None,
        key: None,
    }
}

// ---------------------------------------------------------------------------
// The rules
// ---------------------------------------------------------------------------

/// Held until the verdict, with one line; released by the verdict. The
/// deadline that then passes changes nothing.
#[test]
fn the_confirm_is_held_until_the_verdict_and_released_by_it() {
    let mut sut = boot();
    arrive_tx(&mut sut, "r1");
    assert_eq!(gate(sut.view()), open(), "nothing announced, nothing held");

    // The shell sends its simulation: the wait starts, and its deadline.
    let ops = started(&mut sut, "r1");
    assert_eq!(
        ops,
        vec![Op::SimVerdictTimer {
            id: "r1".to_owned(),
            round: 1,
            ms: SIM_VERDICT_WAIT_MS,
        }],
        "the core's own deadline — the shell runs it and decides nothing"
    );
    let view = sut.view();
    assert!(view.sim_checking);
    assert_eq!(view.sim_waited_out_key, None);
    assert!(
        view.confirm_gate_open,
        "the request's own gate is not what holds it: the line is the gate's last, not its first"
    );
    assert_eq!(gate(view), held());

    // The verdict lands on the sheet: the confirm opens.
    assert!(settled(&mut sut, "r1").is_empty());
    let view = sut.view();
    assert!(!view.sim_checking);
    assert_eq!(view.sim_waited_out_key, None);
    assert_eq!(gate(view), open());

    // Its deadline passes afterwards: the verdict stays, nothing is said.
    assert!(deadline(&mut sut, "r1", 1).is_empty());
    let view = sut.view();
    assert_eq!(
        (view.sim_checking, view.sim_waited_out_key.clone()),
        (false, None)
    );
    assert_eq!(gate(view), open());
}

/// The wait has an end. At the deadline the verdict is "could not check" —
/// the sentence a node that cannot simulate already draws — and the confirm
/// opens. A verdict that still arrives replaces the line and holds nothing.
#[test]
fn the_deadline_releases_the_confirm_as_could_not_check() {
    let mut sut = boot();
    arrive_tx(&mut sut, "r1");
    started(&mut sut, "r1");
    assert_eq!(gate(sut.view()), held());

    assert!(deadline(&mut sut, "r1", 1).is_empty());
    let view = sut.view();
    assert!(!view.sim_checking, "bounded: the wait is over");
    assert_eq!(view.sim_waited_out_key.as_deref(), Some(KEY_UNAVAILABLE));
    assert_eq!(
        KEY_UNAVAILABLE, "componentsUi.signing.simUnavailableWarning",
        "words the corpus already has"
    );
    assert_eq!(gate(view), open());

    // The node answers after all: its verdict is drawn, the line goes, and
    // the confirm — already open — is not shut again.
    settled(&mut sut, "r1");
    let view = sut.view();
    assert_eq!(
        (view.sim_checking, view.sim_waited_out_key.clone()),
        (false, None)
    );
    assert_eq!(gate(view), open());
}

/// Where no simulation exists nothing is ever held: a message, a
/// transaction whose shell runs none, and a chain that answers "not offered"
/// at once (the shell settles in the same breath).
#[test]
fn nothing_is_held_where_no_simulation_exists() {
    // A message: no simulation is started for one.
    let mut sut = boot();
    arrive(
        &mut sut,
        "m1",
        "personal_sign",
        r#"["0x68656c6c6f","0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"]"#,
    );
    let view = sut.view();
    assert!(view.confirm_gate_open && !view.sim_checking);
    assert_eq!(view.sim_waited_out_key, None);
    assert!(sut.outstanding().is_empty(), "no deadline is started");

    // A transaction on a surface that simulates nothing.
    let mut sut = boot();
    arrive_tx(&mut sut, "r1");
    assert!(!sut.view().sim_checking);
    assert_eq!(gate(sut.view()), open());
    assert!(sut.outstanding().is_empty());

    // A chain with no simulation: the node says so at once, and the shell
    // settles as it draws "could not check". Never held on any view a person
    // could tap.
    let mut sut = boot();
    arrive_tx(&mut sut, "r2");
    started(&mut sut, "r2");
    settled(&mut sut, "r2");
    assert_eq!(gate(sut.view()), open());

    // A stale start — a request that is not the one on the sheet — holds
    // nothing and starts no deadline.
    let ops = started(&mut sut, "someone-else");
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(gate(sut.view()), open());
}

/// The wait cannot be tapped through: an approve from a frame drawn before
/// the hold is not taken, exactly as one behind a previous transaction is
/// not. Once the verdict lands the same tap goes ahead.
#[test]
fn an_approve_ahead_of_the_verdict_is_not_taken() {
    let mut sut = boot();
    arrive_tx(&mut sut, "r1");
    started(&mut sut, "r1");

    let ops = sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    assert!(ops.is_empty(), "held: {ops:?}");
    let view = sut.view();
    assert!(!view.is_signing && view.sim_checking);

    settled(&mut sut, "r1");
    let ops = sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    assert!(
        matches!(ops.as_slice(), [Op::CheckBundlerFunding { .. }]),
        "the verdict is on the sheet: the approve starts its pipeline: {ops:?}"
    );
}

/// A wait belongs to its request and to its own start. Another request's
/// deadline, or an earlier start's, ends nothing; a request that left takes
/// its wait with it; and a simulation asked again waits again, bounded again.
#[test]
fn a_wait_belongs_to_its_request_and_its_round() {
    let mut sut = boot();
    arrive_tx(&mut sut, "r1");
    started(&mut sut, "r1");

    // The request is refused: the sheet clears, and a new request does not
    // inherit the wait.
    sut.dispatch(Event::RejectTapped);
    while sut
        .outstanding()
        .iter()
        .any(|op| matches!(op, Op::SendResponse { .. }))
    {
        sut.resolve_matching(|op| matches!(op, Op::SendResponse { .. }), Res::Responded);
    }
    arrive_tx(&mut sut, "r2");
    assert!(!sut.view().sim_checking);
    assert_eq!(gate(sut.view()), open());

    // r2's own simulation starts; r1's old deadline fires and is no part of it.
    let ops = started(&mut sut, "r2");
    assert!(matches!(
        ops.as_slice(),
        [Op::SimVerdictTimer { round: 2, .. }]
    ));
    assert!(deadline(&mut sut, "r1", 1).is_empty());
    let view = sut.view();
    assert!(view.sim_checking, "another request's deadline ends nothing");
    assert_eq!(view.sim_waited_out_key, None);

    // The shell asks again (a second start): a fresh deadline; the first
    // one's passing does not end the second wait.
    let ops = started(&mut sut, "r2");
    assert!(matches!(
        ops.as_slice(),
        [Op::SimVerdictTimer { round: 3, .. }]
    ));
    assert!(deadline(&mut sut, "r2", 2).is_empty());
    assert!(
        sut.view().sim_checking,
        "an earlier start's deadline ends nothing"
    );
    // Its own does.
    deadline(&mut sut, "r2", 3);
    let view = sut.view();
    assert!(!view.sim_checking);
    assert_eq!(view.sim_waited_out_key.as_deref(), Some(KEY_UNAVAILABLE));
    assert_eq!(gate(view), open());
}

/// The line is the gate's LAST: a request still being read, an amount to
/// choose or a fee being worked out says its own line first, and "checking"
/// stands only when the verdict is all that is missing.
#[test]
fn the_checking_line_stands_only_when_the_verdict_is_all_that_is_missing() {
    let mut sut = boot();
    arrive_tx(&mut sut, "r1");
    started(&mut sut, "r1");
    let sign = sut.view();

    let guard = DomainDriver::<ApprovalGuard>::new().view();
    let mut clear = DomainDriver::<ClearSigning>::new().view();
    clear.resolving = true;
    let mut fee = DomainDriver::<FeePolicy>::new().view();
    fee.busy = true;
    let mut input = ConfirmInput {
        sign,
        guard,
        clear,
        fee: Some(fee),
        speed_tier: None,
    };
    assert_eq!(confirm_state(&input).block, Some(ConfirmBlock::Reading));
    input.clear.resolving = false;
    input.clear.surface = ClearSurface::BlindTransaction;
    assert_eq!(
        confirm_state(&input).block,
        Some(ConfirmBlock::FeeMeasuring)
    );
    assert_eq!(gate(input.sign.clone()), held());
}

/// The line exists, in every language, and a view from a shell or a build
/// that predates the wait reads as "nothing to wait for".
#[test]
fn the_line_is_in_the_corpus_and_the_fields_default() {
    use vela_core::i18n::{Catalog, I18n, Options};
    const LOCALES: [&str; 15] = [
        "en", "zh", "zh-TW", "zh-HK", "ja", "ko", "vi", "id", "tr", "es-MX", "pt-BR", "fr", "de",
        "ru", "it",
    ];
    let mut lines = std::collections::BTreeSet::new();
    for lng in LOCALES {
        let mut i18n = I18n::embedded().expect("the embedded corpus");
        if lng != "en" {
            let catalog = Catalog::embedded(lng)
                .unwrap_or_else(|error| panic!("{lng} is compiled in under i18n-all: {error:?}"));
            i18n.load_catalog(catalog);
            i18n.change_language(lng);
        }
        let line = i18n
            .t(SIM_CHECKING_KEY, &Options::default())
            .unwrap_or_else(|error| panic!("{lng} has {SIM_CHECKING_KEY}: {error:?}"));
        assert!(
            line.ends_with('…') && line != SIM_CHECKING_KEY,
            "{lng}: a calm line in progress: {line:?}"
        );
        lines.insert(line);
    }
    assert_eq!(lines.len(), LOCALES.len(), "each language says it itself");

    let mut json = serde_json::to_value(boot().view()).expect("a view serialises");
    let map = json.as_object_mut().expect("an object");
    assert_eq!(map.remove("sim_checking"), Some(serde_json::json!(false)));
    assert_eq!(
        map.remove("sim_waited_out_key"),
        Some(serde_json::Value::Null)
    );
    let old: SignView = serde_json::from_value(json).expect("an older view still reads");
    assert!(!old.sim_checking && old.sim_waited_out_key.is_none());

    // The shell's half of the wire, as it is written.
    assert_eq!(
        serde_json::to_value(Event::SimStarted {
            id: "r1".to_owned()
        })
        .expect("serialises"),
        serde_json::json!({ "type": "sim_started", "id": "r1" })
    );
    assert_eq!(
        serde_json::to_value(Event::SimSettled {
            id: "r1".to_owned()
        })
        .expect("serialises"),
        serde_json::json!({ "type": "sim_settled", "id": "r1" })
    );
    assert_eq!(
        serde_json::to_value(Op::SimVerdictTimer {
            id: "r1".to_owned(),
            round: 1,
            ms: SIM_VERDICT_WAIT_MS,
        })
        .expect("serialises"),
        serde_json::json!({ "type": "sim_verdict_timer", "id": "r1", "round": 1, "ms": 4000 })
    );
    let fired: Res = serde_json::from_value(
        serde_json::json!({ "type": "sim_verdict_timer_fired", "id": "r1", "round": 1 }),
    )
    .expect("the shell's answer reads");
    assert_eq!(
        fired,
        Res::SimVerdictTimerFired {
            id: "r1".to_owned(),
            round: 1
        }
    );
    assert_eq!(
        serde_json::to_value(ConfirmBlock::SimChecking).expect("serialises"),
        serde_json::json!("sim_checking")
    );
}
