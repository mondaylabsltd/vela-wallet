//! §28: every production service default, from the core, and the rule that a
//! test build never writes to one (`vela_core::service_defaults`). The wasm
//! twins are `vela-core-wasm/src/service_defaults.rs`.
//!
//! The phones read their defaults here — no `DEFAULT_REGISTRY_URL` of their
//! own — and ask [`production_write_refusal`] from their transports (Android
//! `VelaHttp`'s interceptor and `RegistryClient`, iOS's `ProductionWriteGuard`
//! URL protocol) before a request leaves the device, in a test build only.

use vela_core::service_defaults as rule;

/// Every production service default.
#[derive(uniffi::Record, Clone, Debug, PartialEq, Eq)]
pub struct ServiceDefaults {
    pub ethereum_data_url: String,
    pub passkey_index_url: String,
    pub bundler_service_url: String,
    pub fiat_rates_url: String,
    pub genesis_service_url: String,
    pub aaguid_directory_url: String,
    pub peerconnect_relay_url: String,
    pub peerconnect_token_url: String,
    pub peerconnect_trial_url: String,
    pub bug_report_url: String,
    pub signing_page_url: String,
}

/// The core's service defaults (`service_defaults::service_defaults`).
#[uniffi::export]
#[must_use]
pub fn service_defaults() -> ServiceDefaults {
    let d = rule::service_defaults();
    ServiceDefaults {
        ethereum_data_url: d.ethereum_data_url,
        passkey_index_url: d.passkey_index_url,
        bundler_service_url: d.bundler_service_url,
        fiat_rates_url: d.fiat_rates_url,
        genesis_service_url: d.genesis_service_url,
        aaguid_directory_url: d.aaguid_directory_url,
        peerconnect_relay_url: d.peerconnect_relay_url,
        peerconnect_token_url: d.peerconnect_token_url,
        peerconnect_trial_url: d.peerconnect_trial_url,
        bug_report_url: d.bug_report_url,
        signing_page_url: d.signing_page_url,
    }
}

/// Is a build with these facts a test build (`BuildFacts::is_test_build`):
/// dev-fixtures, the device-test flavour, or any debug build.
#[uniffi::export]
#[must_use]
pub fn is_test_build(dev_fixtures: bool, device_test: bool, debug: bool) -> bool {
    rule::BuildFacts {
        dev_fixtures,
        device_test,
        debug,
        ..rule::BuildFacts::default()
    }
    .is_test_build()
}

/// Does this value of the opt-in (a launch input or a build flag) opt in:
/// `1` or `true`.
#[uniffi::export]
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn production_writes_opt_in(value: Option<String>) -> bool {
    rule::opt_in(value.as_deref())
}

/// The catalog key a refused write is said with.
#[uniffi::export]
#[must_use]
pub fn production_write_refused_key() -> String {
    rule::REFUSED_KEY.to_owned()
}

/// The same words in English, for a log line or a transport error.
#[uniffi::export]
#[must_use]
pub fn production_write_refused_english() -> String {
    rule::REFUSED_ENGLISH.to_owned()
}

/// The rule (`service_defaults::write_verdict`): the production host a test
/// build's write is refused for, or `None` — the request may leave. A
/// release build answers `None` for everything (and never asks).
#[uniffi::export]
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn production_write_refusal(
    test_build: bool,
    allow_production_writes: bool,
    method: String,
    url: String,
    body: Option<String>,
) -> Option<String> {
    match rule::write_verdict(
        test_build,
        allow_production_writes,
        &method,
        &url,
        body.as_deref(),
    ) {
        rule::WriteVerdict::Allowed => None,
        rule::WriteVerdict::Refused { host, .. } => Some(host),
    }
}

/// Is `host` a production service host.
#[uniffi::export]
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn is_production_host(host: String) -> bool {
    rule::is_production_host(&host)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn the_record_is_the_cores() {
        let d = service_defaults();
        let core = rule::service_defaults();
        assert_eq!(d.passkey_index_url, core.passkey_index_url);
        assert_eq!(d.genesis_service_url, core.genesis_service_url);
        assert_eq!(d.signing_page_url, core.signing_page_url);
    }

    #[test]
    fn the_vector_answers_through_this_door() {
        let v: serde_json::Value = serde_json::from_str(include_str!(
            "../../vela-core/tests/vectors/rules/production-writes.json"
        ))
        .unwrap();
        for case in v["cases"].as_array().unwrap() {
            let refused = production_write_refusal(
                case["testBuild"].as_bool().unwrap(),
                case["allowProductionWrites"].as_bool().unwrap(),
                case["method"].as_str().unwrap().to_owned(),
                case["url"].as_str().unwrap().to_owned(),
                case["body"].as_str().map(str::to_owned),
            );
            assert_eq!(
                refused.as_deref(),
                case["host"].as_str(),
                "{}",
                case["about"]
            );
        }
        assert!(!is_test_build(false, false, false));
        assert!(is_test_build(true, false, false));
        assert!(is_test_build(false, true, false));
        assert!(is_test_build(false, false, true));
    }
}
