//! Vela's side of an outside owner's session, said (the web's
//! `ExternalSignSheet` words): where the session stands, why an ask ended,
//! and how this side is getting onto the relay. The session machine decides
//! each; these are its words.

use vela_core::app::multisig_outside::relay::{RelayStatus, RelayTrouble};
use vela_core::app::multisig_outside::session::{Failure, OutsideSessionView, Phase};

use super::words;
use crate::loc::Loc;

fn chain_name(view: &OutsideSessionView) -> String {
    view.chain
        .as_deref()
        .and_then(|c| {
            c.strip_prefix("0x")
                .map_or_else(|| c.parse().ok(), |h| u64::from_str_radix(h, 16).ok())
        })
        .and_then(|id: u64| u32::try_from(id).ok())
        .map_or_else(String::new, crate::executor::custom_tokens::network_name)
}

/// Where the session stands, in one line.
#[must_use]
pub fn phase(loc: &Loc, view: &OutsideSessionView) -> String {
    if view.away {
        return words::t(loc, "external.pageAway");
    }
    match view.phase {
        Phase::Waiting if view.resumed => words::tv(
            loc,
            "external.resumeBody",
            &[("wallet", view.wallet.as_deref().unwrap_or_default())],
        ),
        Phase::Waiting => words::t(loc, "external.scanBody"),
        Phase::Connecting => words::t(loc, "external.connecting"),
        Phase::WrongAccount => words::tv(
            loc,
            "external.wrongAccount",
            &[(
                "account",
                &view
                    .accounts
                    .first()
                    .map(|a| words::short(a))
                    .unwrap_or_default(),
            )],
        ),
        Phase::Switching => words::tv(loc, "external.switching", &[("chain", &chain_name(view))]),
        Phase::Confirming => words::t(loc, "external.confirming"),
        Phase::Done => match &view.paired_account {
            Some(owner) => words::tv(loc, "external.signedBy", &[("owner", &words::short(owner))]),
            None => words::t(loc, "external.confirming"),
        },
    }
}

/// Why the ask ended without an answer.
#[must_use]
pub fn failure(loc: &Loc, failure: &Failure) -> String {
    match failure {
        Failure::ConnectDeclined => words::t(loc, "external.connectDeclined"),
        Failure::WalletDeclined | Failure::NoAnswer | Failure::Wallet { .. } => {
            words::t(loc, "external.declined")
        }
        Failure::OtherChain => words::tv(loc, "external.otherChain", &[("chain", "")]),
        Failure::BadSignature => words::t(loc, "external.badSignature"),
        Failure::StoppedWaiting => words::t(loc, "external.stoppedWaiting"),
        // Round 16 leftover 4: its `reason` is a transport error's text —
        // never drawn.
        Failure::NoRoom { .. } => words::t(loc, "proposal.reason.unknown"),
    }
}

/// The relay's host, when `detail` is one (`relay.example:443`) — never a
/// transport error's text.
fn relay_host(detail: &str) -> Option<String> {
    let (name, port) = detail.split_once(':').unwrap_or((detail, ""));
    let host_ok = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    let port_ok = port.chars().all(|c| c.is_ascii_digit()) && !detail.ends_with(':');
    (host_ok && port_ok).then(|| detail.to_owned())
}

/// How this side is getting onto the relay, when it is news. Round 16
/// leftover 4 (every shell's rule): the reason is the catalog's — the
/// relay's refusal, a pass not got — or, unreached, the relay's host; with
/// nothing sayable as a reason, "Vela went offline — reconnecting…".
#[must_use]
pub fn relay(loc: &Loc, status: &RelayStatus, now_ms: f64) -> Option<String> {
    let reason = |w: &RelayTrouble, detail: &str| match w {
        RelayTrouble::Refused => Some(words::t(loc, "external.relayRefused")),
        RelayTrouble::NoToken => Some(words::t(loc, "external.relayNoToken")),
        RelayTrouble::Unreachable => relay_host(detail),
    };
    if let RelayStatus::Retrying { why: w, detail, .. } | RelayStatus::Failed { why: w, detail } =
        status
        && reason(w, detail).is_none()
    {
        return Some(words::t(loc, "external.velaOffline"));
    }
    let why = |w: &RelayTrouble, detail: &str| reason(w, detail).unwrap_or_default();
    match status {
        RelayStatus::Connecting | RelayStatus::Up => None,
        RelayStatus::Retrying { why: w, detail, at } => Some(match at {
            Some(at) => words::tv(
                loc,
                "external.velaRelayRetrying",
                &[
                    ("reason", &why(w, detail)),
                    (
                        "seconds",
                        &(((at - now_ms) / 1000.).max(0.).ceil()).to_string(),
                    ),
                ],
            ),
            None => words::tv(
                loc,
                "external.velaRelayUnreachable",
                &[("reason", &why(w, detail))],
            ),
        }),
        RelayStatus::Failed { why: w, detail } => Some(words::tv(
            loc,
            "external.velaRelayFailed",
            &[("reason", &why(w, detail))],
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn every_phase_has_words() {
        let loc = Loc::for_tag("en");
        for p in [
            Phase::Waiting,
            Phase::Connecting,
            Phase::WrongAccount,
            Phase::Switching,
            Phase::Confirming,
            Phase::Done,
        ] {
            let view = OutsideSessionView {
                phase: p,
                ..OutsideSessionView::default()
            };
            let line = phase(&loc, &view);
            assert!(
                !line.is_empty() && !line.contains("home.multisig"),
                "{p:?}: {line}"
            );
        }
    }

    #[test]
    fn a_relay_that_is_up_says_nothing() {
        let loc = Loc::for_tag("en");
        assert_eq!(relay(&loc, &RelayStatus::Up, 0.), None);
        let failed = relay(
            &loc,
            &RelayStatus::Failed {
                why: RelayTrouble::Refused,
                detail: String::new(),
            },
            0.,
        )
        .unwrap();
        assert!(failed.contains("pass"), "{failed}");
    }

    /// Round 16 leftover 4: a transport error is never the reason — the
    /// relay's host, else "offline"; a room not opened is "something went
    /// wrong", never its error's text.
    #[test]
    fn a_transport_error_is_never_drawn() {
        let loc = Loc::for_tag("en");
        let unreached = |detail: &str| {
            relay(
                &loc,
                &RelayStatus::Failed {
                    why: RelayTrouble::Unreachable,
                    detail: detail.to_owned(),
                },
                0.,
            )
            .unwrap()
        };
        let raw = unreached("Os { code: 61, kind: ConnectionRefused }");
        assert!(!raw.contains("ConnectionRefused"), "{raw}");
        assert_eq!(raw, words::t(&loc, "external.velaOffline"));
        assert!(unreached("relay.getvela.app:443").contains("relay.getvela.app:443"));
        let room = failure(
            &loc,
            &Failure::NoRoom {
                reason: "WebSocket error: timed out".into(),
            },
        );
        assert!(!room.contains("WebSocket"), "{room}");
        assert_eq!(room, words::t(&loc, "proposal.reason.unknown"));
    }
}
