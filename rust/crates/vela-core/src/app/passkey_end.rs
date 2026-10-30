//! How a passkey prompt ended, as every shell reports it (send-11).
//!
//! The platforms report a prompt that ran out of time exactly as one the
//! person closed: WebAuthn's `NotAllowedError`, Android Credential Manager's
//! `GetCredentialCancellationException`, iOS's `ASAuthorizationError.canceled`.
//! What tells them apart is the clock: a refusal that comes at the prompt's
//! own deadline is the deadline's. This is that rule, once — the web had it
//! as `classifySign` (`onboarding/core/passkey.ts`), the natives had none and
//! said every timeout as "closed".
//!
//! The shell keeps the platform error's classification ([`FailureKind`]) and
//! how long the prompt was up; the core says which end it was, and with it
//! which result the send and the signing sheet hear
//! (`SendSubmitFailure::PasskeyTimedOut` / `PasskeyCancelled`,
//! `SignSubmitOutcome::PasskeyTimedOut` / `PasskeyCancelled`). Both are the
//! same refusal to every rule; only the words on the screen differ.

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::FailureKind;

/// The signing prompt's deadline, ms: what a shell asks the platform for
/// (WebAuthn's `timeout`, the native prompt's own), so the rule below reads
/// the same clock everywhere.
pub const SIGN_TIMEOUT_MS: f64 = 120_000.0;

/// How close to [`SIGN_TIMEOUT_MS`] a refusal still counts as the deadline's:
/// a platform closes its prompt a little before or after the figure it was
/// given, and the shell's own clock starts a moment before the prompt shows.
pub const SIGN_TIMEOUT_SLACK_MS: f64 = 2_000.0;

/// How a passkey prompt ended without a signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum CeremonyEnd {
    /// The person closed it.
    Cancelled,
    /// It ran out of time ([`SIGN_TIMEOUT_MS`]): a refusal at the deadline.
    TimedOut,
    /// No passkey can be used here.
    NotSupported,
    /// A credential sign-in would never offer (issue #1).
    NotDiscoverable,
    /// Any other failure; the shell's diagnostics carry what it was.
    Other,
}

impl CeremonyEnd {
    /// Closed or out of time: nothing went wrong, the confirm stays for
    /// another try. Every other end is a failure of the passkey.
    #[must_use]
    pub const fn is_refusal(self) -> bool {
        matches!(self, Self::Cancelled | Self::TimedOut)
    }

    /// The `SendSubmitFailure` wire name a refusal is reported as
    /// (`passkey_cancelled`, `passkey_timed_out`); `None` for a failure,
    /// which the shell reports with its diagnostics.
    #[must_use]
    pub const fn send_failure(self) -> Option<&'static str> {
        match self {
            Self::Cancelled => Some("passkey_cancelled"),
            Self::TimedOut => Some("passkey_timed_out"),
            Self::NotSupported | Self::NotDiscoverable | Self::Other => None,
        }
    }
}

/// How a prompt that failed as `kind`, `elapsed_ms` after it was raised,
/// ended: a cancel at (or past) the deadline, less [`SIGN_TIMEOUT_SLACK_MS`],
/// is [`CeremonyEnd::TimedOut`]; any other cancel is the person's. A failure
/// that is not a cancel is never a timeout, whenever it came. A clock that
/// cannot be read (NaN, infinite, negative) says nothing: the person closed
/// it.
#[must_use]
pub fn ceremony_end(kind: FailureKind, elapsed_ms: f64) -> CeremonyEnd {
    match kind {
        FailureKind::Cancelled
            if elapsed_ms.is_finite() && elapsed_ms >= SIGN_TIMEOUT_MS - SIGN_TIMEOUT_SLACK_MS =>
        {
            CeremonyEnd::TimedOut
        }
        FailureKind::Cancelled => CeremonyEnd::Cancelled,
        FailureKind::NotSupported => CeremonyEnd::NotSupported,
        FailureKind::NotDiscoverable => CeremonyEnd::NotDiscoverable,
        FailureKind::Other => CeremonyEnd::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cancel_at_the_deadline_is_a_timeout_and_before_it_the_persons() {
        let at = SIGN_TIMEOUT_MS - SIGN_TIMEOUT_SLACK_MS;
        assert_eq!(
            ceremony_end(FailureKind::Cancelled, at),
            CeremonyEnd::TimedOut
        );
        assert_eq!(
            ceremony_end(FailureKind::Cancelled, SIGN_TIMEOUT_MS + 5_000.0),
            CeremonyEnd::TimedOut
        );
        assert_eq!(
            ceremony_end(FailureKind::Cancelled, at - 1.0),
            CeremonyEnd::Cancelled
        );
        assert_eq!(
            ceremony_end(FailureKind::Cancelled, 3_000.0),
            CeremonyEnd::Cancelled
        );
        for unreadable in [f64::NAN, f64::INFINITY, -1.0] {
            assert_eq!(
                ceremony_end(FailureKind::Cancelled, unreadable),
                CeremonyEnd::Cancelled,
                "{unreadable}"
            );
        }
    }

    #[test]
    fn a_failure_is_never_a_timeout() {
        for (kind, end) in [
            (FailureKind::NotSupported, CeremonyEnd::NotSupported),
            (FailureKind::NotDiscoverable, CeremonyEnd::NotDiscoverable),
            (FailureKind::Other, CeremonyEnd::Other),
        ] {
            assert_eq!(ceremony_end(kind, SIGN_TIMEOUT_MS), end);
            assert!(!end.is_refusal());
            assert_eq!(end.send_failure(), None);
        }
    }

    #[test]
    fn a_refusal_is_reported_as_the_send_machine_reads_it() {
        assert_eq!(
            CeremonyEnd::Cancelled.send_failure(),
            Some("passkey_cancelled")
        );
        assert_eq!(
            CeremonyEnd::TimedOut.send_failure(),
            Some("passkey_timed_out")
        );
        // The names are the send machine's own.
        for end in [CeremonyEnd::Cancelled, CeremonyEnd::TimedOut] {
            let wire = serde_json::json!({ "type": end.send_failure() });
            assert!(
                serde_json::from_value::<super::super::send::SendSubmitFailure>(wire).is_ok(),
                "{end:?}"
            );
        }
    }
}
