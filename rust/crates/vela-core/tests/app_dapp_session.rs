//! Rules of the dApp connection session machine, one test per rule.
//!
//! Inventory `dapp_session` invariants ① – ⑨, each pinned by name, plus one
//! test per timer micro-semantic — this is the densest timer module in the
//! app (4s grace never extended, 45s stuck, 120s join, 60s deadline, 8s
//! dropIfDead, 1s·2ⁿ ≤ 30s backoff), and every one of those windows has a
//! rule that must not drift in the port.
//!
//! Sources of record: `src/models/dapp-connection.tsx:54-620, 963-1027`,
//! `src/services/walletpair-transport.ts`, `src/services/dapp-transport.ts:
//! 262-334`, `src/screens/connect/ConnectScreen.tsx:46-79`. The fake clock is
//! `TimerFired` by id plus the epoch riding on `NetworkOnline` — the core
//! never owns time.

#![cfg(feature = "crux")]

mod support;

use serde_json::json;
use support::DomainDriver;
use vela_core::app::dapp_session::{
    assert_request_chain_context, caip2_to_chain_id, classify_connect_input, declared_chain_id,
    DappSession, DsessChainContextError, DsessConnectionType, DsessDappInfo, DsessError,
    DsessInput, DsessOperation as Op, DsessReconnectCause as Cause, DsessRemoteInjectSession,
    DsessShellResult as Res, DsessStatus, DsessTimerKind as Timer, Event, DROP_IF_DEAD_MS,
    JOIN_TIMEOUT_MS, RECONNECT_DEADLINE_MS, RECONNECT_GRACE_MS, RECONNECT_STUCK_MS,
};

type Sut = DomainDriver<DappSession>;

const WP_URI: &str = "walletpair:v1?relay=wss%3A%2F%2Frelay.example&name=Uniswap";
const RI_URL: &str = "https://relay.example/s/abc123?n=n1&k=k1";

fn dapp() -> DsessDappInfo {
    DsessDappInfo {
        name: "Uniswap".to_owned(),
        url: "https://app.uniswap.org".to_owned(),
        icon: None,
    }
}

fn ri_session() -> DsessRemoteInjectSession {
    DsessRemoteInjectSession {
        server_url: "https://relay.example".to_owned(),
        session_id: "abc123".to_owned(),
        nonce: "n1".to_owned(),
        secret: "k1".to_owned(),
    }
}

/// Ack queued fire-and-forget operations (timers, disconnects, storage,
/// pushes) until the front of the queue is a result-bearing operation or the
/// queue is empty — so the next `resolve` targets the operation under test.
fn ack_housekeeping(sut: &mut Sut) {
    loop {
        let front = match sut.outstanding().first() {
            None => return,
            Some(op) => op.clone(),
        };
        match front {
            Op::StartTimer { .. }
            | Op::CancelTimer { .. }
            | Op::DisconnectTransport { .. }
            | Op::PingTransport { .. }
            | Op::PushWalletInfo { .. }
            | Op::SaveRemoteInjectSession { .. }
            | Op::ClearRemoteInjectSession
            | Op::ClearWalletPairSnapshot
            | Op::OpenBrowser { .. }
            | Op::AlertInvalidLink => {
                sut.resolve(Res::Ack);
            }
            _ => return,
        }
    }
}

/// WalletPair pairing prepared: the 4-digit code is on screen, nothing joined
/// (invariant ① midpoint). Pending handle = 1.
fn prepared() -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::InputSubmitted {
        raw: WP_URI.to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::PrepareWalletPair {
            session_ref: 1,
            uri: WP_URI.to_owned(),
        }]
    );
    let ops = sut.resolve(Res::WalletPairPrepared {
        fingerprint: "0421".to_owned(),
        dapp: dapp(),
    });
    assert!(ops.is_empty(), "prepare only renders — no join yet");
    sut
}

/// Confirmed, transport-connected WalletPair session (handle 1); counters
/// persisted, wallet info pushed, queue drained.
fn connected_walletpair() -> Sut {
    let mut sut = prepared();
    let ops = sut.dispatch(Event::FingerprintConfirmed);
    assert_eq!(ops, vec![Op::ConfirmWalletPairJoin { session_ref: 1 }]);
    let ops = sut.resolve(Res::JoinFinished { connected: true });
    assert!(ops.is_empty());
    let ops = sut.dispatch(Event::TransportConnected { session_ref: 1 });
    // ⑦ — the counter persist comes FIRST; the push only after its ack.
    assert_eq!(ops, vec![Op::PersistWalletPairCounters { session_ref: 1 }]);
    let ops = sut.resolve(Res::CountersPersisted { ok: true });
    assert_eq!(
        ops,
        vec![Op::PushWalletInfo {
            session_ref: 1,
            chain_id: 1,
        }]
    );
    sut.resolve(Res::Ack);
    sut
}

/// Connected + one relay blip: grace(1) 4s, deadline(2) 60s and backoff(3) 1s
/// armed; queue drained. Status still shows "connected" (the grace hold).
fn blipped() -> Sut {
    let mut sut = connected_walletpair();
    let ops = sut.dispatch(Event::TransportReconnecting { session_ref: 1 });
    assert_eq!(
        ops,
        vec![
            Op::StartTimer {
                id: 1,
                kind: Timer::Grace,
                ms: RECONNECT_GRACE_MS,
            },
            Op::StartTimer {
                id: 2,
                kind: Timer::Deadline,
                ms: RECONNECT_DEADLINE_MS,
            },
            Op::StartTimer {
                id: 3,
                kind: Timer::Backoff,
                ms: 1_000,
            },
        ]
    );
    assert_eq!(
        sut.view().status,
        DsessStatus::Connected,
        "grace holds 'connected' until the 4s window elapses"
    );
    ack_housekeeping(&mut sut);
    sut
}

/// Connected remote-inject session (handle 1), dApp info fetched, queue
/// drained.
fn connected_remote_inject() -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::InputSubmitted {
        raw: RI_URL.to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::ConnectRemoteInject {
            session_ref: 1,
            session: ri_session(),
        }]
    );
    // The fresh path commits the session immediately (dapp-connection.tsx:482).
    assert_eq!(sut.view().session, Some(ri_session()));
    let ops = sut.resolve(Res::RemoteInjectConnectFinished);
    assert_eq!(
        ops,
        vec![
            Op::FetchDappInfo { session_ref: 1 },
            Op::SaveRemoteInjectSession {
                session: ri_session(),
            },
        ]
    );
    let ops = sut.resolve(Res::DappInfoFetched { info: Some(dapp()) });
    assert!(ops.is_empty());
    sut.resolve(Res::Ack); // the session save
    let ops = sut.dispatch(Event::TransportConnected { session_ref: 1 });
    // Remote-inject has no counters — the push goes straight out.
    assert_eq!(
        ops,
        vec![Op::PushWalletInfo {
            session_ref: 1,
            chain_id: 1,
        }]
    );
    sut.resolve(Res::Ack);
    sut
}

// ---------------------------------------------------------------------------
// Entry classification (invariant ⑨) — the pure five-way decision
// ---------------------------------------------------------------------------

/// `walletpair:` wins before everything else, whitespace-trimmed
/// (ConnectScreen.tsx:47-53; walletpair-transport.ts:107-109).
#[test]
fn classify_walletpair_scheme_first() {
    assert_eq!(
        classify_connect_input("  walletpair:v1?x=1 "),
        DsessInput::WalletPair {
            uri: "walletpair:v1?x=1".to_owned(),
        }
    );
}

/// Both accepted remote-inject shapes parse to the same session record
/// (dapp-transport.ts:262-286).
#[test]
fn classify_remote_inject_both_shapes() {
    assert_eq!(
        classify_connect_input(RI_URL),
        DsessInput::RemoteInject {
            session: ri_session(),
        }
    );
    assert_eq!(
        classify_connect_input("https://relay.example/bridge?session=abc123&n=n1&k=k1"),
        DsessInput::RemoteInject {
            session: ri_session(),
        }
    );
}

/// ⑨ — a remote-inject connect link IS an https URL; the browser fallback may
/// only run once the remote-inject parse returned nothing (ARCHITECTURE §7).
/// Removing one credential turns the very same address into a plain web page.
#[test]
fn classify_remote_inject_before_browser_fallback() {
    assert!(matches!(
        classify_connect_input(RI_URL),
        DsessInput::RemoteInject { .. }
    ));
    assert_eq!(
        classify_connect_input("https://relay.example/s/abc123?n=n1"),
        DsessInput::Browser {
            url: "https://relay.example/s/abc123?n=n1".to_owned(),
        }
    );
}

/// A bare host is a typed web address: default https, require a dot
/// (dapp-transport.ts:316-334).
#[test]
fn classify_bare_host_defaults_to_https() {
    assert_eq!(
        classify_connect_input("app.uniswap.org"),
        DsessInput::Browser {
            url: "https://app.uniswap.org/".to_owned(),
        }
    );
    assert_eq!(
        classify_connect_input("uniswap.org/swap"),
        DsessInput::Browser {
            url: "https://uniswap.org/swap".to_owned(),
        }
    );
}

/// Full URLs are normalized the way `new URL().toString()` does: scheme+host
/// lowercased, default port dropped, path case kept.
#[test]
fn classify_normalizes_full_urls() {
    assert_eq!(
        classify_connect_input("HTTPS://App.Uniswap.ORG:443/Swap?in=ETH"),
        DsessInput::Browser {
            url: "https://app.uniswap.org/Swap?in=ETH".to_owned(),
        }
    );
}

/// Everything else is invalid — and a parseable URL with a non-http scheme is
/// rejected outright, never retried as a bare host: `javascript:` and even
/// `localhost:8080` (scheme "localhost"!) — ported verbatim.
#[test]
fn classify_rejects_non_web_input() {
    assert_eq!(
        classify_connect_input("javascript:alert(1)"),
        DsessInput::Invalid
    );
    assert_eq!(
        classify_connect_input("localhost:8080"),
        DsessInput::Invalid
    );
    assert_eq!(classify_connect_input("hello"), DsessInput::Invalid);
    assert_eq!(classify_connect_input("foo bar.com"), DsessInput::Invalid);
    assert_eq!(classify_connect_input(""), DsessInput::Invalid);
}

/// A web address routes to the browser without touching the live session
/// (ConnectScreen.tsx:56-63 routes and returns).
#[test]
fn browser_input_never_touches_the_live_session() {
    let mut sut = connected_walletpair();
    let ops = sut.dispatch(Event::InputSubmitted {
        raw: "https://docs.uniswap.org/guide".to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::OpenBrowser {
            url: "https://docs.uniswap.org/guide".to_owned(),
        }]
    );
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Connected);
    assert_eq!(view.connection_type, Some(DsessConnectionType::WalletPair));
}

/// Unclassifiable input alerts and changes nothing.
#[test]
fn invalid_input_alerts() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::InputSubmitted {
        raw: "not a link".to_owned(),
    });
    assert_eq!(ops, vec![Op::AlertInvalidLink]);
    assert_eq!(sut.view().status, DsessStatus::Disconnected);
}

// ---------------------------------------------------------------------------
// Fingerprint gate (invariants ① ②)
// ---------------------------------------------------------------------------

/// ① — prepare shows the code and stops. No join operation exists anywhere,
/// and even a spurious transport event for the pending handle builds nothing
/// (dapp-connection.tsx:509-514; walletpair-protocol.ts:694-697).
#[test]
fn fingerprint_unconfirmed_never_becomes_a_session() {
    let mut sut = prepared();
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Connecting);
    assert_eq!(view.pending_fingerprint.as_deref(), Some("0421"));
    assert_eq!(view.dapp_info, Some(dapp()));
    assert!(sut.outstanding().is_empty(), "no join in flight");

    let ops = sut.dispatch(Event::TransportConnected { session_ref: 1 });
    assert!(ops.is_empty(), "a pending pairing is not a live transport");
    assert_eq!(sut.view().status, DsessStatus::Connecting);
    assert_eq!(sut.view().connection_type, None);
}

/// ① — only the user's confirmation reaches `confirmJoin`.
#[test]
fn confirmation_is_the_only_road_to_join() {
    let mut sut = prepared();
    let ops = sut.dispatch(Event::FingerprintConfirmed);
    assert_eq!(ops, vec![Op::ConfirmWalletPairJoin { session_ref: 1 }]);
    assert_eq!(sut.view().pending_fingerprint, None);
}

/// `if (!transport) return` — confirming with nothing pending is a no-op.
#[test]
fn confirm_without_pending_pairing_is_a_noop() {
    let mut sut = Sut::new();
    assert!(sut.dispatch(Event::FingerprintConfirmed).is_empty());
}

/// ② — cancelling a pending pairing releases the ephemeral X25519 key: the
/// explicit `DisconnectTransport` for the pending handle
/// (dapp-connection.tsx:563-572).
#[test]
fn cancel_releases_the_pending_key() {
    let mut sut = prepared();
    let ops = sut.dispatch(Event::FingerprintCancelled);
    assert_eq!(ops, vec![Op::DisconnectTransport { session_ref: 1 }]);
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Disconnected);
    assert_eq!(view.pending_fingerprint, None);
    assert_eq!(view.dapp_info, None);
    assert_eq!(view.error, None);
}

/// ② — replacing a pending pairing with a new connect releases the old key
/// first (dapp-connection.tsx:461-474). The old dApp card is deliberately NOT
/// cleared until the new prepare answers — ported verbatim.
#[test]
fn replacing_a_pending_pairing_releases_the_old_key() {
    let mut sut = prepared();
    let ops = sut.dispatch(Event::InputSubmitted {
        raw: WP_URI.to_owned(),
    });
    assert_eq!(
        ops,
        vec![
            Op::DisconnectTransport { session_ref: 1 },
            Op::PrepareWalletPair {
                session_ref: 2,
                uri: WP_URI.to_owned(),
            },
        ]
    );
    assert_eq!(
        sut.view().dapp_info,
        Some(dapp()),
        "old card until new prepare"
    );
    ack_housekeeping(&mut sut);
    let ops = sut.resolve(Res::WalletPairPrepared {
        fingerprint: "9999".to_owned(),
        dapp: dapp(),
    });
    assert!(ops.is_empty());
    assert_eq!(sut.view().pending_fingerprint.as_deref(), Some("9999"));
}

/// A failed prepare is a surfaced error (bad pairing URI).
#[test]
fn prepare_failure_shows_error() {
    let mut sut = Sut::new();
    sut.dispatch(Event::InputSubmitted {
        raw: WP_URI.to_owned(),
    });
    let ops = sut.resolve(Res::WalletPairPrepareFailed {
        message: "invalid pairing URI".to_owned(),
    });
    assert!(ops.is_empty());
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Error);
    assert_eq!(
        view.error,
        Some(DsessError::Transport {
            message: "invalid pairing URI".to_owned(),
        })
    );
}

// ---------------------------------------------------------------------------
// Join watchdog — 120s (invariant ④)
// ---------------------------------------------------------------------------

/// `confirmJoin` resolved but the transport is not yet live — the relay may
/// have silently dropped the join, so the 120s watchdog arms
/// (dapp-connection.tsx:534-552).
#[test]
fn unsettled_join_arms_the_watchdog() {
    let mut sut = prepared();
    sut.dispatch(Event::FingerprintConfirmed);
    let ops = sut.resolve(Res::JoinFinished { connected: false });
    assert_eq!(
        ops,
        vec![Op::StartTimer {
            id: 1,
            kind: Timer::Join,
            ms: JOIN_TIMEOUT_MS,
        }]
    );
}

/// Quirk ported verbatim: a join timeout ends `disconnected`, NOT `error` —
/// the TS `setStatus('error')` is synchronously clobbered by the
/// `disconnected` handler that `transport.disconnect()` triggers. The
/// semantic error is still surfaced.
#[test]
fn join_timeout_ends_disconnected_not_error() {
    let mut sut = prepared();
    sut.dispatch(Event::FingerprintConfirmed);
    sut.resolve(Res::JoinFinished { connected: false });
    ack_housekeeping(&mut sut);
    let ops = sut.dispatch(Event::TimerFired { id: 1 });
    assert_eq!(ops, vec![Op::DisconnectTransport { session_ref: 1 }]);
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Disconnected, "clobber quirk");
    assert_eq!(view.error, Some(DsessError::JoinTimeout));
}

/// Connecting before the deadline cancels the watchdog, and its late fire is
/// stale by id.
#[test]
fn join_watchdog_cancelled_on_connect() {
    let mut sut = prepared();
    sut.dispatch(Event::FingerprintConfirmed);
    sut.resolve(Res::JoinFinished { connected: false });
    let ops = sut.dispatch(Event::TransportConnected { session_ref: 1 });
    assert_eq!(
        ops,
        vec![
            Op::CancelTimer { id: 1 },
            Op::PersistWalletPairCounters { session_ref: 1 },
        ]
    );
    ack_housekeeping(&mut sut);
    sut.resolve(Res::CountersPersisted { ok: true });
    ack_housekeeping(&mut sut);
    let ops = sut.dispatch(Event::TimerFired { id: 1 });
    assert!(ops.is_empty(), "cancelled watchdog id is stale");
    assert_eq!(sut.view().status, DsessStatus::Connected);
}

/// A failed join releases the transport (retry loop + join key) and ends
/// `disconnected` with the message — same clobber quirk as the timeout.
#[test]
fn join_failure_releases_the_transport() {
    let mut sut = prepared();
    sut.dispatch(Event::FingerprintConfirmed);
    let ops = sut.resolve(Res::JoinFailed {
        message: "relay refused".to_owned(),
    });
    assert_eq!(ops, vec![Op::DisconnectTransport { session_ref: 1 }]);
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Disconnected);
    assert_eq!(
        view.error,
        Some(DsessError::Transport {
            message: "relay refused".to_owned(),
        })
    );
}

// ---------------------------------------------------------------------------
// Grace — 4s, armed once, never extended (invariant ③)
// ---------------------------------------------------------------------------

/// ③ — a repeated blip while the grace window is pending arms NOTHING: no new
/// grace, no second deadline, no second backoff (dapp-connection.tsx:439
/// early-return; walletpair-transport.ts:361, 485).
#[test]
fn repeated_blips_never_extend_the_grace_window() {
    let mut sut = blipped();
    let ops = sut.dispatch(Event::TransportReconnecting { session_ref: 1 });
    assert!(ops.is_empty(), "grace/deadline/backoff already armed");
    assert_eq!(sut.view().status, DsessStatus::Connected);
}

/// The grace window elapsing flips the UI to "reconnecting" and arms the 45s
/// stuck prompt.
#[test]
fn grace_expiry_surfaces_reconnecting() {
    let mut sut = blipped();
    let ops = sut.dispatch(Event::TimerFired { id: 1 });
    assert_eq!(
        ops,
        vec![Op::StartTimer {
            id: 4,
            kind: Timer::Stuck,
            ms: RECONNECT_STUCK_MS,
        }]
    );
    assert_eq!(sut.view().status, DsessStatus::Reconnecting);
    assert!(!sut.view().reconnect_stuck);
}

/// A blip that self-heals within the grace window never shows at all: the
/// pending flip is cancelled and the late fire is stale
/// (dapp-connection.tsx:406-418).
#[test]
fn recovery_within_grace_never_flickers() {
    let mut sut = blipped();
    let ops = sut.dispatch(Event::TransportConnected { session_ref: 1 });
    assert_eq!(
        ops,
        vec![
            Op::CancelTimer { id: 1 }, // grace
            Op::CancelTimer { id: 2 }, // deadline
            Op::CancelTimer { id: 3 }, // backoff
            Op::PersistWalletPairCounters { session_ref: 1 },
        ]
    );
    assert_eq!(sut.view().status, DsessStatus::Connected);
    ack_housekeeping(&mut sut);
    sut.resolve(Res::CountersPersisted { ok: true });
    ack_housekeeping(&mut sut);
    let ops = sut.dispatch(Event::TimerFired { id: 1 });
    assert!(ops.is_empty(), "cancelled grace id is stale");
    assert_eq!(sut.view().status, DsessStatus::Connected);
}

/// ③ — a manual "Reconnect now" bypasses the grace window: cancel it, show
/// "Reconnecting…" immediately, arm the stuck prompt, reconnect
/// (dapp-connection.tsx:586-595).
#[test]
fn manual_reconnect_bypasses_grace() {
    let mut sut = blipped();
    let ops = sut.dispatch(Event::ManualReconnect);
    assert_eq!(
        ops,
        vec![
            Op::CancelTimer { id: 1 },
            Op::StartTimer {
                id: 4,
                kind: Timer::Stuck,
                ms: RECONNECT_STUCK_MS,
            },
            Op::ReconnectTransport {
                session_ref: 1,
                cause: Cause::Manual,
            },
        ]
    );
    assert_eq!(sut.view().status, DsessStatus::Reconnecting);
}

/// The `reconnectNonce` bump: a manual reconnect while ALREADY reconnecting
/// re-arms the stuck timer and clears the prompt even though the status value
/// does not change (dapp-connection.tsx:222-224, 593).
#[test]
fn manual_reconnect_rearms_the_stuck_prompt() {
    let mut sut = blipped();
    sut.dispatch(Event::TimerFired { id: 1 }); // grace → reconnecting, stuck(4)
    sut.dispatch(Event::TimerFired { id: 4 }); // stuck prompt up
    assert!(sut.view().reconnect_stuck);
    let ops = sut.dispatch(Event::ManualReconnect);
    assert_eq!(
        ops,
        vec![
            Op::StartTimer {
                id: 5,
                kind: Timer::Stuck,
                ms: RECONNECT_STUCK_MS,
            },
            Op::ReconnectTransport {
                session_ref: 1,
                cause: Cause::Manual,
            },
        ]
    );
    assert!(
        !sut.view().reconnect_stuck,
        "prompt cleared on manual retry"
    );
}

/// A manual reconnect with no live transport is a no-op (`if (!transport)
/// return`).
#[test]
fn manual_reconnect_without_transport_is_a_noop() {
    let mut sut = Sut::new();
    assert!(sut.dispatch(Event::ManualReconnect).is_empty());
}

// ---------------------------------------------------------------------------
// Stuck 45s + deadline 60s — the double insurance (invariant ④)
// ---------------------------------------------------------------------------

/// ④ — an auto-reconnect dragging past 45s surfaces the manual-recovery
/// prompt instead of spinning forever (dapp-connection.tsx:237-242).
#[test]
fn stuck_prompt_after_45_seconds() {
    let mut sut = blipped();
    sut.dispatch(Event::TimerFired { id: 1 });
    let ops = sut.dispatch(Event::TimerFired { id: 4 });
    assert!(ops.is_empty());
    assert!(sut.view().reconnect_stuck);
    assert_eq!(sut.view().status, DsessStatus::Reconnecting);
}

/// ④ — the 60s deadline surfaces a recoverable error while KEEPING the
/// session: the UI stops promising recovery, nothing is torn down
/// (walletpair-transport.ts:360-371).
#[test]
fn deadline_errors_without_tearing_down() {
    let mut sut = blipped();
    sut.dispatch(Event::TimerFired { id: 1 }); // grace → reconnecting
    let ops = sut.dispatch(Event::TimerFired { id: 2 }); // deadline
    assert!(ops.is_empty());
    let view = sut.view();
    assert_eq!(view.error, Some(DsessError::ReconnectDeadline));
    assert_eq!(view.status, DsessStatus::Reconnecting, "status untouched");
    assert_eq!(
        view.dapp_info,
        Some(dapp()),
        "session survives the deadline"
    );
    assert_eq!(view.connection_type, Some(DsessConnectionType::WalletPair));
}

// ---------------------------------------------------------------------------
// Backoff — 1s·2ⁿ, capped at 30s
// ---------------------------------------------------------------------------

/// The exponential ladder: 1s, 2s, 4s, 8s, 16s, then pinned at 30s
/// (walletpair-transport.ts:484-492).
#[test]
fn backoff_doubles_to_the_30s_cap() {
    let mut sut = blipped(); // backoff(3) armed at 1s
    let mut backoff_id = 3;
    for expected_ms in [2_000, 4_000, 8_000, 16_000, 30_000, 30_000] {
        let ops = sut.dispatch(Event::TimerFired { id: backoff_id });
        assert_eq!(
            ops,
            vec![Op::ReconnectTransport {
                session_ref: 1,
                cause: Cause::Backoff,
            }]
        );
        ack_housekeeping(&mut sut);
        let ops = sut.resolve(Res::ReconnectFinished {
            cause: Cause::Backoff,
            ok: false,
        });
        match ops.as_slice() {
            [Op::StartTimer {
                id,
                kind: Timer::Backoff,
                ms,
            }] => {
                assert_eq!(*ms, expected_ms, "ladder step");
                backoff_id = *id;
            }
            other => panic!("expected one backoff timer, got {other:?}"),
        }
    }
}

/// A backoff retry that resolves cleanly schedules nothing — the transport's
/// own phase events drive what happens next.
#[test]
fn successful_backoff_retry_schedules_no_more() {
    let mut sut = blipped();
    sut.dispatch(Event::TimerFired { id: 3 });
    ack_housekeeping(&mut sut);
    let ops = sut.resolve(Res::ReconnectFinished {
        cause: Cause::Backoff,
        ok: true,
    });
    assert!(ops.is_empty());
}

/// `transport.reconnect()` emits 'reconnecting' — a backoff retry re-arms the
/// grace debounce when the previous window already elapsed.
#[test]
fn backoff_retry_rearms_an_elapsed_grace() {
    let mut sut = blipped();
    sut.dispatch(Event::TimerFired { id: 1 }); // grace elapsed → stuck(4)
    let ops = sut.dispatch(Event::TimerFired { id: 3 }); // backoff
    assert_eq!(
        ops,
        vec![
            Op::StartTimer {
                id: 5,
                kind: Timer::Grace,
                ms: RECONNECT_GRACE_MS,
            },
            Op::ReconnectTransport {
                session_ref: 1,
                cause: Cause::Backoff,
            },
        ]
    );
}

/// Quirk ported verbatim: a failed MANUAL reconnect schedules no backoff (the
/// TS catch is empty) — only an already-armed backoff timer resumes the auto
/// retry.
#[test]
fn failed_manual_reconnect_schedules_no_backoff() {
    let mut sut = blipped();
    sut.dispatch(Event::ManualReconnect);
    ack_housekeeping(&mut sut);
    let ops = sut.resolve(Res::ReconnectFinished {
        cause: Cause::Manual,
        ok: false,
    });
    assert!(ops.is_empty(), "no backoff scheduled from a manual failure");
    // The backoff armed by the original blip still carries the retry loop —
    // and re-arms the grace debounce the manual tap cancelled (the retry's
    // `reconnect()` emits 'reconnecting', exactly as today).
    let ops = sut.dispatch(Event::TimerFired { id: 3 });
    assert_eq!(
        ops,
        vec![
            Op::StartTimer {
                id: 5,
                kind: Timer::Grace,
                ms: RECONNECT_GRACE_MS,
            },
            Op::ReconnectTransport {
                session_ref: 1,
                cause: Cause::Backoff,
            },
        ]
    );
}

// ---------------------------------------------------------------------------
// Foreground / network recovery
// ---------------------------------------------------------------------------

/// Foregrounding with the transport down forces an immediate reconnect:
/// cancel the frozen backoff, retry now (walletpair-transport.ts:399-420).
#[test]
fn foreground_while_down_forces_reconnect() {
    let mut sut = blipped();
    let ops = sut.dispatch(Event::AppForegrounded {
        backgrounded_ms: 25_000.0,
    });
    assert_eq!(
        ops,
        vec![
            Op::CancelTimer { id: 3 }, // the frozen backoff
            Op::ReconnectTransport {
                session_ref: 1,
                cause: Cause::Foreground,
            },
        ]
    );
}

/// A brief blur with the socket still up is just a ping.
#[test]
fn foreground_brief_blur_pings() {
    let mut sut = connected_walletpair();
    let ops = sut.dispatch(Event::AppForegrounded {
        backgrounded_ms: 19_999.0,
    });
    assert_eq!(ops, vec![Op::PingTransport { session_ref: 1 }]);
}

/// Quirk ported verbatim: foregrounding a still-`connected` session after
/// ≥ 20s backgrounded does NOTHING — the TS stale branch only reconnects from
/// phase 'disconnected' and skips the ping (walletpair-transport.ts:412-419).
#[test]
fn foreground_stale_but_connected_does_nothing() {
    let mut sut = connected_walletpair();
    let ops = sut.dispatch(Event::AppForegrounded {
        backgrounded_ms: 20_000.0,
    });
    assert!(ops.is_empty(), "the stale branch skips a connected session");
}

/// Nothing to recover before pairing has started ('idle'/'awaiting'), and
/// AppState recovery exists only on the WalletPair transport.
#[test]
fn foreground_ignored_while_joining_and_for_remote_inject() {
    let mut sut = prepared();
    sut.dispatch(Event::FingerprintConfirmed);
    sut.resolve(Res::JoinFinished { connected: false });
    ack_housekeeping(&mut sut);
    let ops = sut.dispatch(Event::AppForegrounded {
        backgrounded_ms: 60_000.0,
    });
    assert!(ops.is_empty(), "no recovery before the join settled");

    let mut sut = connected_remote_inject();
    let ops = sut.dispatch(Event::AppForegrounded {
        backgrounded_ms: 60_000.0,
    });
    assert!(ops.is_empty(), "AppState recovery is WalletPair-only");
}

/// Web recovery while down: arm grace + deadline, cancel the pending backoff,
/// reconnect now — `recoverNow` order (walletpair-transport.ts:461-474).
#[test]
fn network_online_while_down_recovers() {
    let mut sut = blipped();
    sut.dispatch(Event::TimerFired { id: 1 }); // grace elapsed → stuck(4)
    sut.dispatch(Event::TimerFired { id: 2 }); // deadline elapsed → error
    let ops = sut.dispatch(Event::NetworkOnline { now_ms: 50_000.0 });
    assert_eq!(
        ops,
        vec![
            Op::StartTimer {
                id: 5,
                kind: Timer::Grace,
                ms: RECONNECT_GRACE_MS,
            },
            Op::StartTimer {
                id: 6,
                kind: Timer::Deadline,
                ms: RECONNECT_DEADLINE_MS,
            },
            Op::CancelTimer { id: 3 },
            Op::ReconnectTransport {
                session_ref: 1,
                cause: Cause::Online,
            },
        ]
    );
}

/// `online` + `visibilitychange` firing together must not double-reconnect:
/// the 3s throttle (walletpair-transport.ts:464).
#[test]
fn network_online_is_throttled_to_3s() {
    let mut sut = connected_walletpair();
    let ops = sut.dispatch(Event::NetworkOnline {
        now_ms: 1_000_000.0,
    });
    assert_eq!(ops, vec![Op::PingTransport { session_ref: 1 }]);
    ack_housekeeping(&mut sut);
    let ops = sut.dispatch(Event::NetworkOnline {
        now_ms: 1_002_999.0,
    });
    assert!(ops.is_empty(), "within the throttle window");
    let ops = sut.dispatch(Event::NetworkOnline {
        now_ms: 1_003_000.0,
    });
    assert_eq!(ops, vec![Op::PingTransport { session_ref: 1 }]);
}

/// Web recovery is WalletPair-only, exactly like AppState recovery.
#[test]
fn network_online_ignored_for_remote_inject() {
    let mut sut = connected_remote_inject();
    let ops = sut.dispatch(Event::NetworkOnline {
        now_ms: 1_000_000.0,
    });
    assert!(ops.is_empty());
}

// ---------------------------------------------------------------------------
// Restore — remote-inject first, dead channels wiped (invariants ⑤ ⑥)
// ---------------------------------------------------------------------------

/// ⑥ — when both stores hold something, remote-inject wins and the walletpair
/// snapshot is left exactly as-is (today's early return,
/// dapp-connection.tsx:967-987).
#[test]
fn restore_prefers_remote_inject() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::RestoreLoaded {
        remote_inject: Some(ri_session()),
        wallet_pair: Some(dapp()),
    });
    assert_eq!(
        ops,
        vec![Op::ConnectRemoteInject {
            session_ref: 1,
            session: ri_session(),
        }]
    );
    assert!(
        !sut.outstanding().iter().any(|op| matches!(
            op,
            Op::RestoreWalletPair { .. } | Op::ClearWalletPairSnapshot
        )),
        "the walletpair snapshot is neither restored nor cleared"
    );
}

/// The restore path commits the session only on connect success — until then
/// the view carries none (dapp-connection.tsx:974-979).
#[test]
fn remote_inject_restore_commits_session_on_success() {
    let mut sut = Sut::new();
    sut.dispatch(Event::RestoreLoaded {
        remote_inject: Some(ri_session()),
        wallet_pair: None,
    });
    assert_eq!(sut.view().session, None, "not committed until connect");
    let ops = sut.resolve(Res::RemoteInjectConnectFinished);
    assert_eq!(
        ops,
        vec![
            Op::FetchDappInfo { session_ref: 1 },
            Op::SaveRemoteInjectSession {
                session: ri_session(),
            },
        ]
    );
    assert_eq!(sut.view().session, Some(ri_session()));
}

/// ⑥ — a failed remote-inject restore cleans up silently: disconnect, wipe
/// the store, no error shown (dapp-connection.tsx:980-985).
#[test]
fn failed_remote_inject_restore_cleans_silently() {
    let mut sut = Sut::new();
    sut.dispatch(Event::RestoreLoaded {
        remote_inject: Some(ri_session()),
        wallet_pair: None,
    });
    let ops = sut.resolve(Res::RemoteInjectConnectFailed {
        message: "401 session expired".to_owned(),
    });
    assert_eq!(
        ops,
        vec![
            Op::DisconnectTransport { session_ref: 1 },
            Op::ClearRemoteInjectSession,
        ]
    );
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Disconnected);
    assert_eq!(view.error, None, "stale sessions never surface an error");
}

/// A restored WalletPair session reconnects under the grace debounce; the
/// launch UX stays 'disconnected' until the window elapses
/// (dapp-connection.tsx:989-1012).
#[test]
fn walletpair_restore_reconnects_under_grace() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::RestoreLoaded {
        remote_inject: None,
        wallet_pair: Some(dapp()),
    });
    assert_eq!(ops, vec![Op::RestoreWalletPair { session_ref: 1 }]);
    let ops = sut.resolve(Res::WalletPairRestoreFinished { restored: true });
    assert_eq!(
        ops,
        vec![
            Op::StartTimer {
                id: 1,
                kind: Timer::Grace,
                ms: RECONNECT_GRACE_MS,
            },
            Op::ReconnectTransport {
                session_ref: 1,
                cause: Cause::Restore,
            },
        ]
    );
    assert_eq!(sut.view().dapp_info, Some(dapp()));
    assert_eq!(sut.view().status, DsessStatus::Disconnected);
}

/// ⑤ — BUG-5/6: a restored session not live 8s after its reconnect attempt
/// has a dead channel. Drop it AND wipe the snapshot so the next launch
/// starts clean and never collides with a fresh pairing on the relay
/// (dapp-connection.tsx:996-1012).
#[test]
fn dead_restored_channel_is_dropped_and_wiped() {
    let mut sut = Sut::new();
    sut.dispatch(Event::RestoreLoaded {
        remote_inject: None,
        wallet_pair: Some(dapp()),
    });
    sut.resolve(Res::WalletPairRestoreFinished { restored: true });
    ack_housekeeping(&mut sut);
    let ops = sut.resolve(Res::ReconnectFinished {
        cause: Cause::Restore,
        ok: true,
    });
    assert_eq!(
        ops,
        vec![Op::StartTimer {
            id: 2,
            kind: Timer::DropIfDead,
            ms: DROP_IF_DEAD_MS,
        }]
    );
    let ops = sut.dispatch(Event::TimerFired { id: 2 });
    assert_eq!(
        ops,
        vec![
            Op::CancelTimer { id: 1 }, // the still-armed grace
            Op::DisconnectTransport { session_ref: 1 },
            Op::ClearWalletPairSnapshot,
        ]
    );
    assert_eq!(sut.view().status, DsessStatus::Disconnected);
}

/// A restored channel that proves live before the 8s check survives it.
#[test]
fn live_restored_channel_survives_drop_if_dead() {
    let mut sut = Sut::new();
    sut.dispatch(Event::RestoreLoaded {
        remote_inject: None,
        wallet_pair: Some(dapp()),
    });
    sut.resolve(Res::WalletPairRestoreFinished { restored: true });
    ack_housekeeping(&mut sut);
    sut.resolve(Res::ReconnectFinished {
        cause: Cause::Restore,
        ok: true,
    });
    sut.dispatch(Event::TransportConnected { session_ref: 1 });
    ack_housekeeping(&mut sut);
    sut.resolve(Res::CountersPersisted { ok: true });
    ack_housekeeping(&mut sut);
    let ops = sut.dispatch(Event::TimerFired { id: 2 }); // dropIfDead
    assert!(ops.is_empty(), "a live channel is left alone");
    assert_eq!(sut.view().status, DsessStatus::Connected);
}

/// ⑤ — `reconnect()` throwing during restore means the channel is dead right
/// now: drop and wipe immediately (the TS catch calls dropIfDead directly).
#[test]
fn restore_reconnect_throw_drops_immediately() {
    let mut sut = Sut::new();
    sut.dispatch(Event::RestoreLoaded {
        remote_inject: None,
        wallet_pair: Some(dapp()),
    });
    sut.resolve(Res::WalletPairRestoreFinished { restored: true });
    ack_housekeeping(&mut sut);
    let ops = sut.resolve(Res::ReconnectFinished {
        cause: Cause::Restore,
        ok: false,
    });
    assert_eq!(
        ops,
        vec![
            Op::CancelTimer { id: 1 },
            Op::DisconnectTransport { session_ref: 1 },
            Op::ClearWalletPairSnapshot,
        ]
    );
}

/// ⑥ — an invalid snapshot cleans up silently: wipe, no error, no session
/// (walletpair-transport.ts:204-236; dapp-connection.tsx:1017-1020).
#[test]
fn invalid_snapshot_is_wiped_silently() {
    let mut sut = Sut::new();
    sut.dispatch(Event::RestoreLoaded {
        remote_inject: None,
        wallet_pair: Some(dapp()),
    });
    let ops = sut.resolve(Res::WalletPairRestoreFinished { restored: false });
    assert_eq!(ops, vec![Op::ClearWalletPairSnapshot]);
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Disconnected);
    assert_eq!(view.error, None);
    assert_eq!(view.dapp_info, None);
}

/// Restore is single-shot and never runs over an active flow — fail-closed
/// deviation (the TS effect would clobber a live transport ref).
#[test]
fn restore_is_single_shot_and_never_clobbers() {
    let mut sut = Sut::new();
    assert!(sut
        .dispatch(Event::RestoreLoaded {
            remote_inject: None,
            wallet_pair: None,
        })
        .is_empty());
    let ops = sut.dispatch(Event::RestoreLoaded {
        remote_inject: Some(ri_session()),
        wallet_pair: None,
    });
    assert!(ops.is_empty(), "second restore report is dropped");

    let mut sut = connected_walletpair();
    let ops = sut.dispatch(Event::RestoreLoaded {
        remote_inject: Some(ri_session()),
        wallet_pair: None,
    });
    assert!(ops.is_empty(), "a live session is never clobbered");
    assert_eq!(
        sut.view().connection_type,
        Some(DsessConnectionType::WalletPair)
    );
}

// ---------------------------------------------------------------------------
// Counters before ciphertext (invariant ⑦)
// ---------------------------------------------------------------------------

/// ⑦ — the wallet-info push happens only after the counter persist
/// acknowledged; a failed persist NEVER pushes and instead closes + wipes
/// (mirrors `abandonUnsafeSession`; walletpair-protocol.ts:395-406).
#[test]
fn failed_counter_persist_never_produces_ciphertext() {
    let mut sut = prepared();
    sut.dispatch(Event::FingerprintConfirmed);
    sut.resolve(Res::JoinFinished { connected: true });
    let ops = sut.dispatch(Event::TransportConnected { session_ref: 1 });
    assert_eq!(ops, vec![Op::PersistWalletPairCounters { session_ref: 1 }]);
    let ops = sut.resolve(Res::CountersPersisted { ok: false });
    assert_eq!(
        ops,
        vec![
            Op::DisconnectTransport { session_ref: 1 },
            Op::ClearWalletPairSnapshot,
        ]
    );
    assert_eq!(sut.view().status, DsessStatus::Disconnected);
    assert!(
        !sut.outstanding()
            .iter()
            .any(|op| matches!(op, Op::PushWalletInfo { .. })),
        "no ciphertext-producing push after a failed persist"
    );
}

/// ⑦ — a wallet change while connected sequences persist → push with the new
/// chain (dapp-connection.tsx:283-292).
#[test]
fn wallet_change_persists_counters_before_pushing() {
    let mut sut = connected_walletpair();
    let ops = sut.dispatch(Event::WalletChanged { chain_id: 137 });
    assert_eq!(ops, vec![Op::PersistWalletPairCounters { session_ref: 1 }]);
    let ops = sut.resolve(Res::CountersPersisted { ok: true });
    assert_eq!(
        ops,
        vec![Op::PushWalletInfo {
            session_ref: 1,
            chain_id: 137,
        }]
    );
}

/// `pushWalletInfo` guards on `_connected` — if the channel dropped while the
/// persist was in flight, nothing is pushed (walletpair-transport.ts:305).
#[test]
fn persist_resolving_after_a_drop_pushes_nothing() {
    let mut sut = connected_walletpair();
    sut.dispatch(Event::WalletChanged { chain_id: 137 });
    sut.dispatch(Event::TransportReconnecting { session_ref: 1 });
    let ops = sut.resolve(Res::CountersPersisted { ok: true });
    assert!(ops.is_empty(), "channel down — nothing to push into");
}

/// Remote-inject has no counters: the push goes straight out on connect and
/// on wallet changes.
#[test]
fn remote_inject_pushes_without_counters() {
    let mut sut = connected_remote_inject();
    let ops = sut.dispatch(Event::WalletChanged { chain_id: 10 });
    assert_eq!(
        ops,
        vec![Op::PushWalletInfo {
            session_ref: 1,
            chain_id: 10,
        }]
    );
}

/// A wallet change while not connected only records the chain — today's
/// `status === 'connected' && transport.connected` guard.
#[test]
fn wallet_change_while_down_only_records_the_chain() {
    let mut sut = blipped(); // status 'connected' but transport down
    let ops = sut.dispatch(Event::WalletChanged { chain_id: 137 });
    assert!(ops.is_empty(), "transport down — no push");
    assert_eq!(sut.view().chain_id, 137);
}

// ---------------------------------------------------------------------------
// Plaintext chainId vs encrypted CAIP-2 context (invariant ⑧) — pure
// ---------------------------------------------------------------------------

/// ⑧ — hex, decimal-string and numeric plaintext declarations all normalize
/// against the encrypted context (walletpair-transport.ts:56-68).
#[test]
fn chain_context_accepts_matching_declarations() {
    let caip2 = "eip155:8453";
    for params in [
        json!([{ "chainId": "0x2105" }]),
        json!([{ "chainId": 8453 }]),
        json!([{ "chainId": "8453" }]),
    ] {
        assert_eq!(
            assert_request_chain_context("eth_sendTransaction", &params, caip2),
            Ok(())
        );
    }
    assert_eq!(
        assert_request_chain_context("wallet_sendCalls", &json!([{ "chainId": "0x2105" }]), caip2),
        Ok(())
    );
}

/// ⑧ — a plaintext chainId disagreeing with the encrypted CAIP-2 context is
/// rejected (walletpair-transport.ts:78-81).
#[test]
fn chain_context_rejects_mismatches() {
    assert_eq!(
        assert_request_chain_context(
            "eth_sendTransaction",
            &json!([{ "chainId": "0x1" }]),
            "eip155:8453"
        ),
        Err(DsessChainContextError::ChainMismatch)
    );
    // A declaration that fails to normalize (fractional) is also a mismatch.
    assert_eq!(
        assert_request_chain_context(
            "eth_sendTransaction",
            &json!([{ "chainId": 5.5 }]),
            "eip155:5"
        ),
        Err(DsessChainContextError::ChainMismatch)
    );
}

/// An absent or null chainId defers to the encrypted context alone.
#[test]
fn chain_context_allows_absent_declarations() {
    assert_eq!(
        assert_request_chain_context("eth_sendTransaction", &json!([{}]), "eip155:1"),
        Ok(())
    );
    assert_eq!(
        assert_request_chain_context(
            "eth_sendTransaction",
            &json!([{ "chainId": null }]),
            "eip155:1"
        ),
        Ok(())
    );
    // Methods without an embedded chain are never inspected.
    assert_eq!(
        assert_request_chain_context("personal_sign", &json!([{ "chainId": 999 }]), "eip155:1"),
        Ok(())
    );
}

/// Typed-data domains are checked in both object and string form; a string
/// that fails to parse as JSON passes HERE — the signing validator owns that
/// rejection (ported verbatim, walletpair-transport.ts:69-77).
#[test]
fn chain_context_checks_typed_data_domains() {
    // A typed-data request is read as its ONE document (audit 2026-10-01):
    // the method's two params, a real account, an EIP-712 document.
    let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
    let doc = json!({"types": {"EIP712Domain": []}, "primaryType": "Mail",
                     "domain": {"chainId": 137}, "message": {}});
    assert_eq!(
        assert_request_chain_context("eth_signTypedData_v4", &json!([account, doc]), "eip155:137"),
        Ok(())
    );
    assert_eq!(
        assert_request_chain_context(
            "eth_signTypedData_v4",
            &json!([account, doc.to_string()]),
            "eip155:1"
        ),
        Err(DsessChainContextError::ChainMismatch)
    );
    assert_eq!(
        assert_request_chain_context(
            "eth_signTypedData_v4",
            &json!(["0xabc", "not json"]),
            "eip155:1"
        ),
        Ok(()),
        "unparseable typed data is the signing validator's rejection"
    );
    // No fallback any more (audit 2026-10-01): a one-element request is not a
    // typed-data request — the signing validator refuses it -32602, so the
    // chain check lets it pass rather than answer it twice.
    assert_eq!(
        assert_request_chain_context(
            "eth_signTypedData",
            &json!([{ "domain": { "chainId": 1 } }]),
            "eip155:2"
        ),
        Ok(())
    );
}

/// `caip2ToChainId` quirks, ported verbatim: parseInt leading-digit semantics
/// ("eip155:5x" → 5) and NO prefix check (any 7 chars are sliced off); zero
/// and empty are invalid.
#[test]
fn caip2_parse_quirks_ported_verbatim() {
    assert_eq!(caip2_to_chain_id("eip155:8453"), Some(8453));
    assert_eq!(
        caip2_to_chain_id("eip155:5x"),
        Some(5),
        "parseInt semantics"
    );
    assert_eq!(
        caip2_to_chain_id("ZZZZZZ:7"),
        Some(7),
        "prefix never checked"
    );
    assert_eq!(caip2_to_chain_id("eip155:0"), None);
    assert_eq!(caip2_to_chain_id("eip155:"), None);
    assert_eq!(caip2_to_chain_id("short"), None);
    assert_eq!(
        assert_request_chain_context("eth_sendTransaction", &json!([{}]), "eip155:x"),
        Err(DsessChainContextError::InvalidCaip2)
    );
}

/// `declaredChainId` — number / 0x-hex / decimal string; everything else is
/// not a declaration.
#[test]
fn declared_chain_id_forms() {
    assert_eq!(declared_chain_id(&json!(31)), Some(31));
    assert_eq!(declared_chain_id(&json!("0x1f")), Some(31));
    assert_eq!(declared_chain_id(&json!("31")), Some(31));
    assert_eq!(declared_chain_id(&json!(-1)), None);
    assert_eq!(declared_chain_id(&json!(5.5)), None);
    assert_eq!(declared_chain_id(&json!(true)), None);
    assert_eq!(declared_chain_id(&json!("0x")), None);
}

// ---------------------------------------------------------------------------
// Disconnect, terminal drops and the error-message quirks
// ---------------------------------------------------------------------------

/// `disconnectBridge` — tear down, clear the session surface and wipe BOTH
/// stores (dapp-connection.tsx:575-584).
#[test]
fn explicit_disconnect_wipes_both_stores() {
    let mut sut = connected_walletpair();
    let ops = sut.dispatch(Event::DisconnectRequested);
    assert_eq!(
        ops,
        vec![
            Op::DisconnectTransport { session_ref: 1 },
            Op::ClearRemoteInjectSession,
            Op::ClearWalletPairSnapshot,
        ]
    );
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Disconnected);
    assert_eq!(view.connection_type, None);
    assert_eq!(view.session, None);
    assert_eq!(view.dapp_info, None);
}

/// Quirk ported verbatim: a terminal transport drop keeps `session` and
/// `dappInfo` — only an explicit disconnect clears them
/// (dapp-connection.tsx:420-431 touches neither).
#[test]
fn terminal_drop_keeps_session_and_dapp_info() {
    let mut sut = connected_remote_inject();
    let ops = sut.dispatch(Event::TransportDisconnected { session_ref: 1 });
    assert!(
        ops.is_empty(),
        "the transport is already gone — no disconnect op"
    );
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Disconnected);
    assert_eq!(view.connection_type, None);
    assert_eq!(view.session, Some(ri_session()), "kept");
    assert_eq!(view.dapp_info, Some(dapp()), "kept");
}

/// A terminal WalletPair drop also cancels every armed timer for the episode.
#[test]
fn terminal_drop_cancels_all_timers() {
    let mut sut = blipped();
    let ops = sut.dispatch(Event::TransportDisconnected { session_ref: 1 });
    assert_eq!(
        ops,
        vec![
            Op::CancelTimer { id: 1 }, // grace
            Op::CancelTimer { id: 2 }, // deadline
            Op::CancelTimer { id: 3 }, // backoff
        ]
    );
    assert_eq!(sut.view().status, DsessStatus::Disconnected);
}

/// Transport errors set the message WITHOUT touching the status — a deadline
/// error coexists with "reconnecting" (dapp-connection.tsx:453-455).
#[test]
fn transport_error_leaves_status_alone() {
    let mut sut = connected_walletpair();
    let ops = sut.dispatch(Event::TransportError {
        session_ref: 1,
        message: "relay hiccup".to_owned(),
    });
    assert!(ops.is_empty());
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Connected);
    assert_eq!(
        view.error,
        Some(DsessError::Transport {
            message: "relay hiccup".to_owned(),
        })
    );
}

/// Quirk ported verbatim: `errorMessage` is cleared neither by recovery (the
/// `connected` handler never touches it) nor by `disconnectBridge`.
#[test]
fn error_message_survives_recovery_and_disconnect() {
    let mut sut = blipped();
    sut.dispatch(Event::TransportError {
        session_ref: 1,
        message: "channel_not_found".to_owned(),
    });
    sut.dispatch(Event::TransportConnected { session_ref: 1 });
    assert_eq!(
        sut.view().error,
        Some(DsessError::Transport {
            message: "channel_not_found".to_owned(),
        }),
        "recovery never clears the message"
    );
    ack_housekeeping(&mut sut);
    sut.resolve(Res::CountersPersisted { ok: true });
    ack_housekeeping(&mut sut);
    sut.dispatch(Event::DisconnectRequested);
    assert_eq!(
        sut.view().error,
        Some(DsessError::Transport {
            message: "channel_not_found".to_owned(),
        }),
        "disconnectBridge never clears it either"
    );
}

/// Quirk ported verbatim: a FRESH remote-inject connect failure only nulls
/// the ref — no `DisconnectTransport` is issued (the TS leak) — and shows the
/// error (dapp-connection.tsx:494-498).
#[test]
fn fresh_remote_inject_failure_shows_error_without_disconnect() {
    let mut sut = Sut::new();
    sut.dispatch(Event::InputSubmitted {
        raw: RI_URL.to_owned(),
    });
    let ops = sut.resolve(Res::RemoteInjectConnectFailed {
        message: "ECONNREFUSED".to_owned(),
    });
    assert!(
        ops.is_empty(),
        "TS only nulls the ref — leak ported verbatim"
    );
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Error);
    assert_eq!(
        view.error,
        Some(DsessError::Transport {
            message: "ECONNREFUSED".to_owned(),
        })
    );
}

/// `setDappInfo(info)` — a failed fetch sets null, verbatim.
#[test]
fn dapp_info_fetch_failure_clears_the_card() {
    let mut sut = Sut::new();
    sut.dispatch(Event::InputSubmitted {
        raw: RI_URL.to_owned(),
    });
    sut.resolve(Res::RemoteInjectConnectFinished);
    let ops = sut.resolve(Res::DappInfoFetched { info: None });
    assert!(ops.is_empty());
    assert_eq!(sut.view().dapp_info, None);
}

// ---------------------------------------------------------------------------
// Staleness — released handles and superseded flows go quiet
// ---------------------------------------------------------------------------

/// Transport events keyed by an unknown handle are dropped — the shell-side
/// zombie-listener guard (fail-closed deviation, module doc).
#[test]
fn stale_transport_events_are_dropped() {
    let mut sut = connected_walletpair();
    assert!(sut
        .dispatch(Event::TransportConnected { session_ref: 99 })
        .is_empty());
    assert!(sut
        .dispatch(Event::TransportReconnecting { session_ref: 99 })
        .is_empty());
    assert!(sut
        .dispatch(Event::TransportDisconnected { session_ref: 99 })
        .is_empty());
    assert!(sut
        .dispatch(Event::TransportError {
            session_ref: 99,
            message: "zombie".to_owned(),
        })
        .is_empty());
    let view = sut.view();
    assert_eq!(view.status, DsessStatus::Connected);
    assert_eq!(view.error, None);
}

/// A superseded flow's shell result is dropped by the attempt guard: a
/// prepare resolving after the user already disconnected builds nothing.
#[test]
fn stale_prepare_result_is_dropped() {
    let mut sut = Sut::new();
    sut.dispatch(Event::InputSubmitted {
        raw: WP_URI.to_owned(),
    });
    let ops = sut.dispatch(Event::DisconnectRequested);
    assert_eq!(
        ops,
        vec![
            Op::DisconnectTransport { session_ref: 1 }, // the in-flight prepare's handle
            Op::ClearRemoteInjectSession,
            Op::ClearWalletPairSnapshot,
        ]
    );
    let ops = sut.resolve(Res::WalletPairPrepared {
        fingerprint: "0421".to_owned(),
        dapp: dapp(),
    });
    assert!(ops.is_empty(), "attempt guard drops the stale result");
    assert_eq!(sut.view().pending_fingerprint, None);
    assert_eq!(sut.view().status, DsessStatus::Disconnected);
}
