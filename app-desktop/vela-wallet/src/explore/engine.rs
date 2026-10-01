//! Why the dApp browser's engine did not start (spec 083 W1b).
//!
//! On Windows the engine is WebView2, and its creation can be refused in two
//! ways a person can act on: the runtime is not installed, or the profile
//! folder may not be written. Until 083 a failed start was one line on stderr
//! and a blank page — retried on every paint, each attempt filing a Windows
//! Error Reporting event. Now it is a state the page draws, retried when the
//! person asks. Platform-neutral, so every host's tests cover it.
// Linux has no in-app browser (spec 032): only the tests use this there.
#![cfg_attr(target_os = "linux", allow(dead_code))]

/// `HRESULT_FROM_WIN32(ERROR_FILE_NOT_FOUND)`: no WebView2 runtime to start.
const RUNTIME_MISSING: u32 = 0x8007_0002;
/// `HRESULT_FROM_WIN32(ERROR_ACCESS_DENIED)`: the profile folder was refused.
const ACCESS_DENIED: u32 = 0x8007_0005;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EngineFailure {
    RuntimeMissing,
    AccessDenied,
    /// Anything else, with the platform's code when it gave one.
    Other(Option<u32>),
}

impl EngineFailure {
    #[must_use]
    pub fn from_hresult(code: Option<i32>) -> Self {
        match code.map(i32::cast_unsigned) {
            Some(RUNTIME_MISSING) => Self::RuntimeMissing,
            Some(ACCESS_DENIED) => Self::AccessDenied,
            other => Self::Other(other),
        }
    }

    /// The profile probe's refusal, as the engine says it when it does not
    /// wait instead (083 H7). Any other probe error is the engine's to judge.
    #[must_use]
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn from_profile_probe(error: &std::io::Error) -> Option<Self> {
        (error.kind() == std::io::ErrorKind::PermissionDenied).then_some(Self::AccessDenied)
    }

    /// The code as support searches for it: `"0x80070005"`.
    #[must_use]
    pub fn code(self) -> Option<String> {
        let code = match self {
            Self::RuntimeMissing => RUNTIME_MISSING,
            Self::AccessDenied => ACCESS_DENIED,
            Self::Other(code) => code?,
        };
        Some(format!("{code:#010X}").replacen("0X", "0x", 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_two_failures_a_person_can_act_on_are_named() {
        assert_eq!(
            EngineFailure::from_hresult(Some(0x8007_0002_u32.cast_signed())),
            EngineFailure::RuntimeMissing
        );
        assert_eq!(
            EngineFailure::from_hresult(Some(0x8007_0005_u32.cast_signed())),
            EngineFailure::AccessDenied
        );
        assert_eq!(
            EngineFailure::from_hresult(None),
            EngineFailure::Other(None)
        );
        assert_eq!(
            EngineFailure::AccessDenied.code().as_deref(),
            Some("0x80070005")
        );
        assert_eq!(EngineFailure::Other(None).code(), None);
        assert_eq!(
            EngineFailure::from_hresult(Some(0x8000_4005_u32.cast_signed()))
                .code()
                .as_deref(),
            Some("0x80004005")
        );
    }

    /// 083 H7: a profile folder that refuses the probe is the panel's
    /// "folder refused"; anything else is left to the engine.
    #[test]
    fn a_refused_profile_probe_is_access_denied() {
        use std::io::{Error, ErrorKind};
        assert_eq!(
            EngineFailure::from_profile_probe(&Error::from(ErrorKind::PermissionDenied)),
            Some(EngineFailure::AccessDenied)
        );
        assert_eq!(
            EngineFailure::from_profile_probe(&Error::from(ErrorKind::StorageFull)),
            None
        );
        #[cfg(windows)]
        assert_eq!(
            // ERROR_ACCESS_DENIED, as a deny entry on the folder answers.
            EngineFailure::from_profile_probe(&Error::from_raw_os_error(5)),
            Some(EngineFailure::AccessDenied)
        );
    }
}
