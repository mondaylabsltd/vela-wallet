//! A passkey prompt's failure, said in the person's language (round 10,
//! real-device S3).
//!
//! Every shell drew the platform's own words inside a translated sheet:
//! Android's "[50152] RP ID cannot be validated" and Google Password
//! Manager's "device can't connect", iOS's `ASAuthorizationError`
//! `localizedDescription` ("The operation couldn't be completed…"), the web's
//! `DOMException.message` — English, or the system's language rather than
//! Vela's, under a Chinese title. The words were the shell's; this is the
//! rule that replaces them, once.
//!
//! A shell hands over the error's FACTS — which platform, the error's domain
//! (an exception class, an `NSError` domain, a `DOMException` name), its code,
//! and its raw message — and gets back a coarse [`PasskeyErrorKind`], the
//! corpus key the sheet draws, and the [`FailureKind`] the onboarding machines
//! branch on. The raw message stays the shell's, for the bug report and the
//! technical details; it is never the sentence on the screen.
//!
//! Classification reads identities, never prose: a class name, a domain, a
//! number. The one exception is a token the platform writes in English
//! whatever the locale and that names a status code — Play Services'
//! `[50152]` at the head of the message when no code is passed separately,
//! and Apple's "is not associated with domain" for an app whose
//! associated-domains check failed (both developer-facing strings, not
//! localised). Anything unrecognised is [`PasskeyErrorKind::Other`] — "The
//! passkey couldn't be used. Try again." — never the raw text.

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::FailureKind;

/// What kind of failure a passkey prompt ended in, as far as the person
/// needs to know.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum PasskeyErrorKind {
    /// The person closed the prompt (or the platform reports a timeout the
    /// same way — [`super::passkey_end::ceremony_end`] tells those apart by
    /// the clock).
    Cancelled,
    /// This device, browser or provider cannot do a passkey here.
    NotSupported,
    /// Sign-in found no passkey for Vela on this device.
    NoCredential,
    /// The platform could not verify that this app may use Vela's passkeys:
    /// Android's asset links (`[50152] RP ID cannot be validated`), Apple's
    /// associated domains, a WebAuthn `SecurityError`. Often the domain's
    /// file was unreachable, so the words ask to check the connection first.
    AppNotAssociated,
    /// The network the prompt needed was not there.
    NoNetwork,
    /// The other device (a phone, by QR) did not connect, or the link
    /// dropped — the core's own link-failure words
    /// ([`crate::cable::conn::is_link_failure`]).
    PhoneUnreachable,
    /// The authenticator already holds one of this wallet's keys
    /// (`InvalidStateError` with `excludeCredentials`,
    /// `ASAuthorizationError.matchedExcludedCredential`).
    AlreadyRegistered,
    /// The platform says the prompt ran out of time.
    TimedOut,
    /// Anything else.
    Other,
}

impl PasskeyErrorKind {
    /// The corpus key the sheet draws for this kind — a full i18n path.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Cancelled => "onboarding.common.passkeyError.cancelled",
            Self::NotSupported => "onboarding.common.passkeyError.notSupported",
            Self::NoCredential => "onboarding.common.passkeyError.noCredential",
            Self::AppNotAssociated => "onboarding.common.passkeyError.appNotAssociated",
            Self::NoNetwork => "onboarding.common.passkeyError.noNetwork",
            Self::PhoneUnreachable => "onboarding.common.phoneLinkFailed",
            Self::AlreadyRegistered => "onboarding.common.passkeyError.alreadyRegistered",
            Self::TimedOut => "onboarding.common.passkeyError.timedOut",
            Self::Other => "onboarding.common.passkeyError.other",
        }
    }

    /// The onboarding machines' vocabulary ([`FailureKind`]) for this kind.
    /// A platform timeout is a refusal, as WebAuthn's `NotAllowedError` at
    /// the deadline always was; no credential is a failed sign-in, never
    /// "this device cannot" (it just did the ceremony).
    #[must_use]
    pub const fn failure(self) -> FailureKind {
        match self {
            Self::Cancelled | Self::TimedOut => FailureKind::Cancelled,
            Self::NotSupported => FailureKind::NotSupported,
            Self::NoCredential
            | Self::AppNotAssociated
            | Self::NoNetwork
            | Self::PhoneUnreachable
            | Self::AlreadyRegistered
            | Self::Other => FailureKind::Other,
        }
    }
}

/// A platform passkey error, as a shell reads it off the error object.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct PasskeyErrorFacts {
    /// `"android"`, `"ios"`, `"macos"`, `"web"` (anything else reads as
    /// unknown; the domain still decides).
    pub platform: String,
    /// The error's identity:
    /// - Android: the exception's class name, simple or qualified
    ///   (`androidx.credentials.exceptions.NoCredentialException`) — or its
    ///   `type` string; for a `*PublicKeyCredentialDomException`, its
    ///   `domError.type` (`androidx.credentials.TYPE_INVALID_STATE_ERROR`).
    /// - Apple: the `NSError` domain (`ASAuthorizationErrorDomain` /
    ///   `com.apple.AuthenticationServices.AuthorizationError`,
    ///   `NSURLErrorDomain`, `WKErrorDomain`).
    /// - Web: the `DOMException` name (`NotAllowedError`, …).
    #[serde(default)]
    pub domain: String,
    /// The `NSError` code, or a Play Services status code when the shell has
    /// one apart from the message. `None` when there is none.
    #[serde(default)]
    pub code: Option<i32>,
    /// The platform's raw message — read only for the stable tokens named
    /// in the module doc; kept by the shell for the report.
    #[serde(default)]
    pub message: String,
}

/// What a shell draws for a failed passkey prompt.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct PasskeyErrorWords {
    pub kind: PasskeyErrorKind,
    /// The corpus key of the sentence ([`PasskeyErrorKind::key`]).
    pub key: String,
    /// The onboarding machines' classification ([`PasskeyErrorKind::failure`]).
    pub failure: FailureKind,
}

/// The rule: a platform's passkey error facts → the words a sheet draws.
#[must_use]
pub fn passkey_error_words(
    platform: &str,
    domain: &str,
    code: Option<i32>,
    message: &str,
) -> PasskeyErrorWords {
    let kind = classify(platform, domain, code, message);
    PasskeyErrorWords {
        kind,
        key: kind.key().to_owned(),
        failure: kind.failure(),
    }
}

/// [`passkey_error_words`] over [`PasskeyErrorFacts`].
#[must_use]
pub fn words_of(facts: &PasskeyErrorFacts) -> PasskeyErrorWords {
    passkey_error_words(&facts.platform, &facts.domain, facts.code, &facts.message)
}

/// [`passkey_error_words`] over JSON text ([`PasskeyErrorFacts`] in,
/// [`PasskeyErrorWords`] out). Facts that do not read are an unknown error:
/// the generic words, never nothing.
#[must_use]
pub fn passkey_error_words_json(facts_json: &str) -> String {
    let facts: PasskeyErrorFacts = serde_json::from_str(facts_json).unwrap_or_default();
    serde_json::to_string(&words_of(&facts)).unwrap_or_default()
}

/// The last segment of a class name, type string or domain — the identity,
/// whichever spelling the shell had: `androidx.credentials.exceptions.
/// NoCredentialException` → `NoCredentialException`,
/// `…TYPE_CREATE_PUBLIC_KEY_CREDENTIAL_DOM_EXCEPTION/androidx.credentials.
/// TYPE_INVALID_STATE_ERROR` → `TYPE_INVALID_STATE_ERROR`.
fn identity(domain: &str) -> &str {
    domain
        .trim()
        .rsplit(['.', '$', '/'])
        .next()
        .unwrap_or_default()
}

fn classify(platform: &str, domain: &str, code: Option<i32>, message: &str) -> PasskeyErrorKind {
    use PasskeyErrorKind as K;
    // The core's own words for a link to the other device that failed.
    if crate::cable::conn::is_link_failure(message.trim()) {
        return K::PhoneUnreachable;
    }
    let platform = platform.trim().to_ascii_lowercase();
    let domain = domain.trim();
    let id = identity(domain);
    let apple = matches!(platform.as_str(), "ios" | "macos");

    // Apple's `NSError` domains: the code is the identity.
    if domain == "ASAuthorizationErrorDomain"
        || domain == "com.apple.AuthenticationServices.AuthorizationError"
    {
        return match code {
            Some(1001) => K::Cancelled,
            Some(1003 | 1005) => K::NotSupported,
            Some(1006) => K::AlreadyRegistered,
            // `failed` is also what an app whose associated domain did not
            // verify is told ("Application with identifier … is not
            // associated with domain getvela.app") — a developer string, not
            // localised.
            Some(1004) if message.contains("is not associated with domain") => K::AppNotAssociated,
            _ => K::Other,
        };
    }
    if domain == "NSURLErrorDomain" {
        return match code {
            Some(-1001) => K::TimedOut,
            _ => K::NoNetwork,
        };
    }
    if domain == "WKErrorDomain" {
        return K::Other;
    }

    let by_identity = match id {
        // WebAuthn's `DOMException` names (the web, a web view) and Android's
        // DOM error types, which carry the same names.
        "NotAllowedError" | "AbortError" | "TYPE_NOT_ALLOWED_ERROR" | "TYPE_ABORT_ERROR" => {
            Some(K::Cancelled)
        }
        "InvalidStateError" | "TYPE_INVALID_STATE_ERROR" => Some(K::AlreadyRegistered),
        "NotSupportedError"
        | "ConstraintError"
        | "TYPE_NOT_SUPPORTED_ERROR"
        | "TYPE_CONSTRAINT_ERROR" => Some(K::NotSupported),
        "SecurityError" | "TYPE_SECURITY_ERROR" => Some(K::AppNotAssociated),
        "TimeoutError" | "TYPE_TIMEOUT_ERROR" => Some(K::TimedOut),
        "NetworkError" | "TYPE_NETWORK_ERROR" => Some(K::NoNetwork),
        // Android Credential Manager, by class and by `type`.
        "GetCredentialCancellationException"
        | "CreateCredentialCancellationException"
        | "TYPE_USER_CANCELED" => Some(K::Cancelled),
        "NoCredentialException" | "TYPE_NO_CREDENTIAL" => Some(K::NoCredential),
        "GetCredentialProviderConfigurationException"
        | "CreateCredentialProviderConfigurationException"
        | "GetCredentialUnsupportedException"
        | "CreateCredentialUnsupportedException"
        | "CreateCredentialNoCreateOptionException"
        | "TYPE_NO_CREATE_OPTIONS"
        | "TYPE_GET_CREDENTIAL_PROVIDER_CONFIGURATION_EXCEPTION"
        | "TYPE_CREATE_CREDENTIAL_PROVIDER_CONFIGURATION_EXCEPTION"
        | "TYPE_GET_CREDENTIAL_UNSUPPORTED_EXCEPTION"
        | "TYPE_CREATE_CREDENTIAL_UNSUPPORTED_EXCEPTION" => Some(K::NotSupported),
        // Interrupted, unknown, custom: the status code (below) decides.
        _ => None,
    };
    if let Some(kind) = by_identity {
        return kind;
    }

    // A Play Services status code: passed apart, or at the head of the
    // message as `[50152] …` (a stable token, not prose). Never for Apple,
    // whose codes are their domain's.
    if !apple {
        let status = code.or_else(|| bracketed_code(message));
        match status {
            // `RP ID cannot be validated`: the app's asset links did not
            // verify for getvela.app.
            Some(50152) => return K::AppNotAssociated,
            // CommonStatusCodes: NETWORK_ERROR, TIMEOUT, CANCELED.
            Some(7) => return K::NoNetwork,
            Some(15) => return K::TimedOut,
            Some(16) => return K::Cancelled,
            _ => {}
        }
    }
    K::Other
}

/// `[50152] RP ID cannot be validated.` → `50152`: a bracketed number at the
/// head of the message, where Play Services writes its status code.
fn bracketed_code(message: &str) -> Option<i32> {
    let rest = message.trim_start().strip_prefix('[')?;
    let (digits, _) = rest.split_once(']')?;
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn the_status_code_is_read_only_from_a_bracketed_head() {
        assert_eq!(
            bracketed_code("[50152] RP ID cannot be validated."),
            Some(50152)
        );
        assert_eq!(bracketed_code("  [16] Cancelled"), Some(16));
        assert_eq!(bracketed_code("error [50152]"), None);
        assert_eq!(bracketed_code("[abc] x"), None);
        assert_eq!(bracketed_code("[] x"), None);
    }

    #[test]
    fn identity_is_the_last_segment() {
        assert_eq!(
            identity("androidx.credentials.exceptions.NoCredentialException"),
            "NoCredentialException"
        );
        assert_eq!(
            identity(
                "androidx.credentials.TYPE_CREATE_PUBLIC_KEY_CREDENTIAL_DOM_EXCEPTION/androidx.credentials.TYPE_INVALID_STATE_ERROR"
            ),
            "TYPE_INVALID_STATE_ERROR"
        );
        assert_eq!(identity("NotAllowedError"), "NotAllowedError");
    }

    #[test]
    fn every_kind_names_a_key_and_a_failure() {
        for kind in [
            PasskeyErrorKind::Cancelled,
            PasskeyErrorKind::NotSupported,
            PasskeyErrorKind::NoCredential,
            PasskeyErrorKind::AppNotAssociated,
            PasskeyErrorKind::NoNetwork,
            PasskeyErrorKind::PhoneUnreachable,
            PasskeyErrorKind::AlreadyRegistered,
            PasskeyErrorKind::TimedOut,
            PasskeyErrorKind::Other,
        ] {
            assert!(kind.key().starts_with("onboarding.common."), "{kind:?}");
        }
    }

    #[test]
    fn unreadable_json_facts_are_the_generic_words() {
        let words: PasskeyErrorWords =
            serde_json::from_str(&passkey_error_words_json("{")).expect("words");
        assert_eq!(words.kind, PasskeyErrorKind::Other);
        assert_eq!(words.key, "onboarding.common.passkeyError.other");
    }
}
