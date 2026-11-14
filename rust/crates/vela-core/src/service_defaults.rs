//! Every production service default, in one place, and the rule that a test
//! build never writes to one (docs/multisig-machines.md §28).
//!
//! ## One source of service defaults
//!
//! The key index, the multisig service and its mailbox, the relay (bundler
//! and sponsor), the fiat rates, the PeerConnect relay and its token, the bug
//! report — each shell used to carry its own copy of these addresses
//! (Android's `RegistryClient.DEFAULT_REGISTRY_URL`, iOS's
//! `RegistryClient.defaultURL`, the desktop's `registry::DEFAULT_REGISTRY_URL`,
//! the web's `DEFAULT_SERVICE_ENDPOINTS`). A copy is a second place to forget:
//! the round that pointed one service at a local stand-in left the copy of
//! another on production. Now the constants below are the only spelling;
//! every shell reads [`service_defaults`] (uniffi `service_defaults_json`,
//! the wasm crate's `serviceDefaults`, the web's generated
//! `serviceDefaults.ts`, the desktop by path), and the older names in
//! `app::network_admin` and friends are aliases of these.
//!
//! ## Test builds never write to production
//!
//! Twice a test build wrote to a production service by default — a device
//! pass's genesis service, then a simulator's throwaway passkey account in
//! the production key index. A tester's checklist did not stop either. The
//! build does now: [`write_verdict`] is asked by every shell's transport
//! before a request leaves the device, and in a TEST build
//! ([`BuildFacts::is_test_build`]: dev-fixtures, the device-test flavour, any
//! debug build, the web's dev server or development package) it refuses a
//! WRITE ([`is_write`]) to a production host ([`is_production_host`]).
//! Reads stay allowed, so a pass that needs production reads keeps working.
//! [`ALLOW_PRODUCTION_WRITES`] set to `1` — an environment variable, a launch
//! argument or a dev setting, never on by default — lets a test build write.
//! A release build is never asked: its shells do not install the guard, and
//! this function answers [`WriteVerdict::Allowed`] for it regardless.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// The defaults
// ---------------------------------------------------------------------------

/// Chain metadata, token lists, logos (`ethereum-data`).
pub const ETHEREUM_DATA_URL: &str = "https://ethereum-data.getvela.app";
/// The passkey key index (p256-index v2): where a new account's keys are
/// recorded and a returning passkey finds its wallet.
pub const PASSKEY_INDEX_URL: &str = "https://p256-index-v2.getvela.app";
/// The Vela relay: the ERC-4337 bundler, gas quotes and the sponsor
/// (`/v1/sponsor`, `/v1/treasury`, `/v1/account`).
pub const BUNDLER_SERVICE_URL: &str = "https://vela-relay-cf.getvela.app";
/// Fiat rates (vela-currency, a Frankfurter instance). `base=USD` is required.
pub const FIAT_RATES_URL: &str = "https://vela-currency.getvela.app/v2/rates?base=USD";
/// The multisig service (the Safe Genesis Registry's API): where a multisig's
/// genesis is recorded and its proposals are shared (the mailbox).
pub const GENESIS_SERVICE_URL: &str = "https://safe-genesis.getvela.app";
/// The passkey model directory: answers for models the compiled catalog does
/// not carry. Read only.
pub const AAGUID_DIRECTORY_URL: &str = "https://aaguid-explorer.awesometools.dev";
/// The public PeerConnect relay an outside owner's wallet and Vela meet on.
pub const PEERCONNECT_RELAY_URL: &str = "wss://relay.peerconnect.dev";
/// Where a token for the public PeerConnect relay is asked for.
pub const PEERCONNECT_TOKEN_URL: &str = "https://getvela.app/api/peerconnect-token";
/// PeerConnect's own trial tokens, when ours are not given.
pub const PEERCONNECT_TRIAL_URL: &str = "https://peerconnect.dev/api/trial";
/// Where a bug report is filed.
pub const BUG_REPORT_URL: &str = "https://getvela.app/api/bug-report";
/// The official signing page (spec 071).
pub const SIGNING_PAGE_URL: &str = "https://sign.getvela.app/";

/// The production domain: `getvela.app` and every host under it is a
/// production host, whether or not a default above names it.
pub const PRODUCTION_DOMAIN: &str = "getvela.app";

/// The opt-in that lets a TEST build write to production: an environment
/// variable, a launch argument or a dev setting of this name, set to `1`.
/// Off by default, and meaningless in a release build.
pub const ALLOW_PRODUCTION_WRITES: &str = "VELA_ALLOW_PRODUCTION_WRITES";

/// The words a refused write is said with (`{{host}}`: the service's host).
pub const REFUSED_KEY: &str = "common.testBuild.productionWriteRefused";

/// The same words in English, for a log line and for a transport error that
/// has no catalog to hand.
pub const REFUSED_ENGLISH: &str =
    "This test build won't write to the live service; point it at a local one.";

/// Every production service default, as the shells read them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

/// The defaults (the constants above).
#[must_use]
pub fn service_defaults() -> ServiceDefaults {
    ServiceDefaults {
        ethereum_data_url: ETHEREUM_DATA_URL.to_owned(),
        passkey_index_url: PASSKEY_INDEX_URL.to_owned(),
        bundler_service_url: BUNDLER_SERVICE_URL.to_owned(),
        fiat_rates_url: FIAT_RATES_URL.to_owned(),
        genesis_service_url: GENESIS_SERVICE_URL.to_owned(),
        aaguid_directory_url: AAGUID_DIRECTORY_URL.to_owned(),
        peerconnect_relay_url: PEERCONNECT_RELAY_URL.to_owned(),
        peerconnect_token_url: PEERCONNECT_TOKEN_URL.to_owned(),
        peerconnect_trial_url: PEERCONNECT_TRIAL_URL.to_owned(),
        bug_report_url: BUG_REPORT_URL.to_owned(),
        signing_page_url: SIGNING_PAGE_URL.to_owned(),
    }
}

/// [`service_defaults`] as JSON — what the uniffi and wasm exports hand over.
#[must_use]
pub fn service_defaults_json() -> String {
    serde_json::to_string(&service_defaults()).unwrap_or_default()
}

const ALL_DEFAULTS: [&str; 11] = [
    ETHEREUM_DATA_URL,
    PASSKEY_INDEX_URL,
    BUNDLER_SERVICE_URL,
    FIAT_RATES_URL,
    GENESIS_SERVICE_URL,
    AAGUID_DIRECTORY_URL,
    PEERCONNECT_RELAY_URL,
    PEERCONNECT_TOKEN_URL,
    PEERCONNECT_TRIAL_URL,
    BUG_REPORT_URL,
    SIGNING_PAGE_URL,
];

// ---------------------------------------------------------------------------
// Production hosts
// ---------------------------------------------------------------------------

/// The host of `url` (`scheme://[user@]host[:port]/…`), lower-case, without
/// its port, brackets or trailing dot; `None` when it names none.
#[must_use]
pub fn url_host(url: &str) -> Option<String> {
    let rest = url.trim().split_once("://")?.1;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if let Some(v6) = authority.strip_prefix('[') {
        v6.split_once(']').map_or(v6, |(h, _)| h)
    } else {
        authority.split_once(':').map_or(authority, |(h, _)| h)
    };
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    (!host.is_empty()).then_some(host)
}

/// The production hosts by name: every default's host (sorted, once each).
/// [`is_production_host`] also counts every host under [`PRODUCTION_DOMAIN`].
#[must_use]
pub fn production_hosts() -> Vec<String> {
    let mut hosts: Vec<String> = ALL_DEFAULTS.iter().filter_map(|u| url_host(u)).collect();
    hosts.sort();
    hosts.dedup();
    hosts
}

/// Is `host` a production service host: [`PRODUCTION_DOMAIN`] or a host
/// under it, or the host of one of the defaults (PeerConnect's, the
/// passkey directory's).
#[must_use]
pub fn is_production_host(host: &str) -> bool {
    let host = host.trim().trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return false;
    }
    host == PRODUCTION_DOMAIN
        || host.ends_with(&format!(".{PRODUCTION_DOMAIN}"))
        || production_hosts().contains(&host)
}

// ---------------------------------------------------------------------------
// What a write is
// ---------------------------------------------------------------------------

/// JSON-RPC methods that only read: a POST whose body asks these (one call,
/// or a batch of nothing else) is a read, not a write. Everything else a
/// POST can carry — `eth_sendUserOperation`, `eth_sendRawTransaction`, a
/// sponsor request, a REST body, a method this list does not name — is a
/// write. An allowlist, so a method nobody thought of fails closed.
pub const READ_RPC_METHODS: &[&str] = &[
    "eth_blockNumber",
    "eth_call",
    "eth_chainId",
    "eth_createAccessList",
    "eth_estimateGas",
    "eth_estimateUserOperationGas",
    "eth_feeHistory",
    "eth_gasPrice",
    "eth_getBalance",
    "eth_getBlockByHash",
    "eth_getBlockByNumber",
    "eth_getBlockReceipts",
    "eth_getBlockTransactionCountByHash",
    "eth_getBlockTransactionCountByNumber",
    "eth_getCode",
    "eth_getLogs",
    "eth_getProof",
    "eth_getStorageAt",
    "eth_getTransactionByBlockHashAndIndex",
    "eth_getTransactionByBlockNumberAndIndex",
    "eth_getTransactionByHash",
    "eth_getTransactionCount",
    "eth_getTransactionReceipt",
    "eth_getUserOperationByHash",
    "eth_getUserOperationReceipt",
    "eth_maxPriorityFeePerGas",
    "eth_simulateV1",
    "eth_supportedEntryPoints",
    "eth_syncing",
    "debug_traceCall",
    "net_version",
    "pimlico_getUserOperationGasPrice",
    "pimlico_getUserOperationStatus",
    "vela_getInBandGasQuote",
    "web3_clientVersion",
];

/// The JSON-RPC methods `body` asks, when it is a JSON-RPC call or a
/// non-empty batch of them; `None` for anything else.
fn rpc_methods(body: &str) -> Option<Vec<String>> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let one = |v: &serde_json::Value| v.get("method")?.as_str().map(str::to_owned);
    match &value {
        serde_json::Value::Array(calls) if !calls.is_empty() => calls.iter().map(one).collect(),
        serde_json::Value::Object(_) => one(&value).map(|m| vec![m]),
        _ => None,
    }
}

/// Does a request with this method and body create or change something:
/// any method but `GET`, `HEAD` and `OPTIONS`, unless it is a JSON-RPC read
/// ([`READ_RPC_METHODS`]). A WebSocket's opening handshake is a `GET`.
#[must_use]
pub fn is_write(method: &str, body: Option<&str>) -> bool {
    let method = method.trim().to_ascii_uppercase();
    if matches!(method.as_str(), "GET" | "HEAD" | "OPTIONS" | "") {
        return false;
    }
    if method != "POST" {
        return true;
    }
    match body.and_then(rpc_methods) {
        Some(methods) => !methods
            .iter()
            .all(|m| READ_RPC_METHODS.contains(&m.as_str())),
        None => true,
    }
}

// ---------------------------------------------------------------------------
// Which builds are test builds
// ---------------------------------------------------------------------------

/// What a shell knows about its own build. Any one of them makes it a test
/// build.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BuildFacts {
    /// Built with the parallel space's fixed keys (`dev-fixtures`).
    pub dev_fixtures: bool,
    /// Android's `devicetest` flavour (`app.getvela.wallet.devicetest`), the
    /// iOS "Vela Test" device-test build (`….devicetest`).
    pub device_test: bool,
    /// Any debug build (Android `BuildConfig.DEBUG`, iOS `DEBUG`, Rust
    /// `debug_assertions`).
    pub debug: bool,
    /// The web's dev server (`vite dev`).
    pub dev_server: bool,
    /// A development package (the web extension's `extension/dist`).
    pub dev_package: bool,
}

impl BuildFacts {
    /// A test build: any of the facts holds.
    #[must_use]
    pub fn is_test_build(self) -> bool {
        self.dev_fixtures || self.device_test || self.debug || self.dev_server || self.dev_package
    }
}

/// Is the opt-in set: [`ALLOW_PRODUCTION_WRITES`]'s value, trimmed, is `1`
/// (or `true`). Anything else — unset, empty, `0`, a word — is off.
#[must_use]
pub fn opt_in(value: Option<&str>) -> bool {
    matches!(
        value.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("1" | "true")
    )
}

// ---------------------------------------------------------------------------
// The rule
// ---------------------------------------------------------------------------

/// What happens to one request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "camelCase")]
pub enum WriteVerdict {
    /// It may leave the device.
    Allowed,
    /// A test build's write to a production service: refused before it
    /// leaves, said with [`REFUSED_KEY`] and logged.
    Refused {
        /// The production host it was for.
        host: String,
        /// [`REFUSED_KEY`].
        key: String,
        /// [`REFUSED_ENGLISH`].
        message: String,
    },
}

impl WriteVerdict {
    #[must_use]
    pub fn is_refused(&self) -> bool {
        matches!(self, Self::Refused { .. })
    }
}

/// The rule: in a test build without the opt-in, a write ([`is_write`]) to
/// a production host ([`is_production_host`]) is refused; everything else —
/// every request of a release build, a read, a write to a local stand-in —
/// is allowed.
#[must_use]
pub fn write_verdict(
    test_build: bool,
    allow_production_writes: bool,
    method: &str,
    url: &str,
    body: Option<&str>,
) -> WriteVerdict {
    if !test_build || allow_production_writes {
        return WriteVerdict::Allowed;
    }
    let Some(host) = url_host(url).filter(|h| is_production_host(h)) else {
        return WriteVerdict::Allowed;
    };
    if !is_write(method, body) {
        return WriteVerdict::Allowed;
    }
    WriteVerdict::Refused {
        host,
        key: REFUSED_KEY.to_owned(),
        message: REFUSED_ENGLISH.to_owned(),
    }
}

/// One request a shell asks about, as JSON (the uniffi and wasm door).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WriteAsk {
    pub test_build: bool,
    pub allow_production_writes: bool,
    pub method: String,
    pub url: String,
    pub body: Option<String>,
}

/// [`write_verdict`] over JSON: a [`WriteAsk`] in, a [`WriteVerdict`] out.
/// Only a test build asks (a release build installs no guard), so an ask
/// that does not parse — a shell's bug — is refused: failing closed costs a
/// test build one request, failing open could cost production a record.
#[must_use]
pub fn write_verdict_json(ask_json: &str) -> String {
    let verdict = match serde_json::from_str::<WriteAsk>(ask_json) {
        Ok(ask) => write_verdict(
            ask.test_build,
            ask.allow_production_writes,
            &ask.method,
            &ask.url,
            ask.body.as_deref(),
        ),
        Err(_) => WriteVerdict::Refused {
            host: String::new(),
            key: REFUSED_KEY.to_owned(),
            message: REFUSED_ENGLISH.to_owned(),
        },
    };
    serde_json::to_string(&verdict).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// The relying party a build's passkeys live under (§35, review 0-1)
// ---------------------------------------------------------------------------

/// The release apps' relying party: every Vela account's passkeys are bound
/// to it, and `getvela.app`'s association files (`assetlinks.json`,
/// `apple-app-site-association`) list only the store apps — so only a
/// release build can use them.
pub const RELEASE_RP_ID: &str = "getvela.app";

/// The web dev server's and a simulator's / emulator's relying party. A
/// browser on `localhost` mints passkeys of its own there; a simulator's
/// platform authenticator answers no association for it, so a simulator
/// signs with the parallel space's fixture keys, as it always has.
pub const LOCAL_RP_ID: &str = "localhost";

/// The relying party a TEST build on a real device uses — a debug build, the
/// Android `devicetest` flavour, the iOS "Vela Test" build, a desktop debug
/// build: **getvela.app, the production one** (owner, 2026-11-14: a test
/// build creates and signs in exactly as the store app does, so what is
/// tested is what ships). getvela.app's association files decide which
/// builds may actually use it: iOS lists the app id, which only the team can
/// sign; Android lists only the release certificate, so an Android test build
/// must be signed with that key to use it (`docs/security/hardening.md` §6).
/// A build can still be given a separate domain through its `TEST_RP_DOMAIN`
/// build setting (Android `-PvelaTestRpDomain=…` or the `TEST_RP_DOMAIN`
/// environment variable at build time; iOS the `TEST_RP_DOMAIN` build
/// setting → the `VelaTestRpDomain` Info.plist key; the desktop
/// `TEST_RP_DOMAIN` at compile time); that domain must serve association
/// files of its own.
pub const TEST_RP_DOMAIN: Option<&str> = Some(RELEASE_RP_ID);

/// What a build says when its passkey domain cannot be known: the ask it
/// was given could not be read. It never guesses one.
pub const TEST_RP_DOMAIN_UNSET: &str = "This build's passkey domain could not be read, so no passkey was asked for. Check its TEST_RP_DOMAIN setting (see docs/security/hardening.md, \"Test builds' passkey domain\").";

/// Why a build has no relying party.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RpIdError {
    /// The build's ask could not be read (the JSON door), so it has no
    /// passkey domain; never a guess.
    TestDomainUnset { message: String },
    /// The configured test domain is not a bare host name.
    TestDomainInvalid { domain: String, message: String },
}

impl RpIdError {
    #[must_use]
    pub fn message(&self) -> &str {
        match self {
            Self::TestDomainUnset { message } | Self::TestDomainInvalid { message, .. } => message,
        }
    }
}

/// A bare host name: lowercase letters, digits, `-` and `.`, at least one
/// dot, no empty label, no scheme, port or path.
fn is_host_name(domain: &str) -> bool {
    !domain.is_empty()
        && domain.len() <= 253
        && domain.contains('.')
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// The relying party a NATIVE build's platform passkeys use — the one rule
/// for Android, iOS and the desktop (§35):
///
/// - a release build → [`RELEASE_RP_ID`], always (the configured test domain
///   is never read);
/// - a test build on a simulator or emulator → [`LOCAL_RP_ID`];
/// - a test build on a device → the configured test domain (`configured`,
///   the build's `TEST_RP_DOMAIN` setting), else [`TEST_RP_DOMAIN`], which is
///   getvela.app.
///
/// `test_build` is [`BuildFacts::is_test_build`]. A security key reached over
/// CTAP (USB, NFC, caBLE) is not gated by any association file — every CTAP
/// client names its own relying party — so this rule is for the platform
/// passkeys the association files gate.
pub fn relying_party_id(
    test_build: bool,
    simulator: bool,
    configured: Option<&str>,
) -> Result<String, RpIdError> {
    if !test_build {
        return Ok(RELEASE_RP_ID.to_owned());
    }
    if simulator {
        return Ok(LOCAL_RP_ID.to_owned());
    }
    let domain = configured
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .or(TEST_RP_DOMAIN)
        .ok_or_else(|| RpIdError::TestDomainUnset {
            message: TEST_RP_DOMAIN_UNSET.to_owned(),
        })?;
    if !is_host_name(domain) {
        return Err(RpIdError::TestDomainInvalid {
            domain: domain.to_owned(),
            message: format!(
                "TEST_RP_DOMAIN \"{domain}\" is not a passkey domain: give a bare host name (e.g. getvela.app or test.example.org)."
            ),
        });
    }
    Ok(domain.to_owned())
}

/// The relying party the WEB wallet uses on `host` (the page's hostname) —
/// the rule `passkey.ts` `relyingPartyId` follows, replayed against the
/// shared vector:
///
/// - a packaged extension (`chrome-extension://…`, `safari-web-extension://…`)
///   → [`RELEASE_RP_ID`] (the manifest's `https://getvela.app/*` host
///   permission is what lets it claim it);
/// - `localhost`, `127.0.0.1` or any IPv4 address → itself (the dev server);
/// - `getvela.app` or any host under it → [`RELEASE_RP_ID`];
/// - anything else (a preview deployment, a self-hosted copy) → its own host,
///   so a test deployment's passkeys are its own.
#[must_use]
pub fn web_relying_party_id(host: &str, packaged_extension: bool) -> String {
    if packaged_extension {
        return RELEASE_RP_ID.to_owned();
    }
    let host = host.trim().to_ascii_lowercase();
    let ipv4 = host.split('.').count() == 4
        && host
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()));
    if host == "localhost" || ipv4 {
        return host;
    }
    if host == RELEASE_RP_ID || host.ends_with(".getvela.app") {
        return RELEASE_RP_ID.to_owned();
    }
    host
}

/// One native build's ask, as JSON (the uniffi and wasm door, and the
/// vector's `native` cases).
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RpAsk {
    pub test_build: bool,
    pub simulator: bool,
    pub test_domain: Option<String>,
}

/// [`relying_party_id`] over JSON: `{"rpId": …}` or `{"error": RpIdError}`.
#[must_use]
pub fn relying_party_json(ask_json: &str) -> String {
    let answer = match serde_json::from_str::<RpAsk>(ask_json) {
        Ok(ask) => relying_party_id(ask.test_build, ask.simulator, ask.test_domain.as_deref()),
        Err(_) => Err(RpIdError::TestDomainUnset {
            message: TEST_RP_DOMAIN_UNSET.to_owned(),
        }),
    };
    match answer {
        Ok(rp_id) => serde_json::json!({ "rpId": rp_id }).to_string(),
        Err(error) => serde_json::json!({ "error": error }).to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn hosts_are_read_from_every_scheme() {
        assert_eq!(
            url_host("https://P256-Index-V2.getvela.app./api/register").as_deref(),
            Some("p256-index-v2.getvela.app")
        );
        assert_eq!(
            url_host("wss://relay.peerconnect.dev/#abc").as_deref(),
            Some("relay.peerconnect.dev")
        );
        assert_eq!(
            url_host("http://u:p@127.0.0.1:8787/api").as_deref(),
            Some("127.0.0.1")
        );
        assert_eq!(url_host("http://[::1]:9/x").as_deref(), Some("::1"));
        assert_eq!(url_host("not a url"), None);
    }

    #[test]
    fn production_hosts_are_the_domain_and_the_defaults() {
        assert!(is_production_host("getvela.app"));
        assert!(is_production_host("anything.getvela.app"));
        assert!(is_production_host("relay.peerconnect.dev"));
        assert!(is_production_host("peerconnect.dev"));
        assert!(!is_production_host("notgetvela.app"));
        assert!(!is_production_host("getvela.app.evil.test"));
        assert!(!is_production_host("127.0.0.1"));
        assert!(!is_production_host(""));
    }

    #[test]
    fn a_json_rpc_read_is_a_read_and_a_send_is_a_write() {
        let read =
            r#"{"jsonrpc":"2.0","id":1,"method":"eth_estimateUserOperationGas","params":[]}"#;
        let send = r#"{"jsonrpc":"2.0","id":1,"method":"eth_sendUserOperation","params":[]}"#;
        assert!(!is_write("POST", Some(read)));
        assert!(is_write("POST", Some(send)));
        assert!(is_write("POST", Some(&format!("[{read},{send}]"))));
        assert!(!is_write("POST", Some(&format!("[{read},{read}]"))));
        assert!(is_write("POST", Some("[]")));
        assert!(is_write("POST", Some(r#"{"publicKey":"0x04"}"#)));
        assert!(is_write("POST", None));
        assert!(is_write("PUT", Some(read)));
        assert!(is_write("DELETE", None));
        assert!(!is_write("get", None));
        assert!(!is_write("OPTIONS", None));
    }

    #[test]
    fn only_a_test_builds_production_write_is_refused() {
        let url = "https://p256-index-v2.getvela.app/api/register";
        assert!(write_verdict(true, false, "POST", url, Some("{}")).is_refused());
        assert!(!write_verdict(false, false, "POST", url, Some("{}")).is_refused());
        assert!(!write_verdict(true, true, "POST", url, Some("{}")).is_refused());
        assert!(!write_verdict(true, false, "GET", url, None).is_refused());
        assert!(!write_verdict(
            true,
            false,
            "POST",
            "http://127.0.0.1:9566/api/register",
            None
        )
        .is_refused());
    }

    #[test]
    fn the_opt_in_is_one_or_true_only() {
        assert!(opt_in(Some("1")));
        assert!(opt_in(Some(" true ")));
        assert!(!opt_in(Some("0")));
        assert!(!opt_in(Some("")));
        assert!(!opt_in(Some("yes")));
        assert!(!opt_in(None));
    }

    #[test]
    fn every_fact_makes_a_test_build() {
        assert!(!BuildFacts::default().is_test_build());
        for facts in [
            BuildFacts {
                dev_fixtures: true,
                ..BuildFacts::default()
            },
            BuildFacts {
                device_test: true,
                ..BuildFacts::default()
            },
            BuildFacts {
                debug: true,
                ..BuildFacts::default()
            },
            BuildFacts {
                dev_server: true,
                ..BuildFacts::default()
            },
            BuildFacts {
                dev_package: true,
                ..BuildFacts::default()
            },
        ] {
            assert!(facts.is_test_build(), "{facts:?}");
        }
    }

    #[test]
    fn the_json_door_answers_the_same() {
        let ask = r#"{"testBuild":true,"method":"POST","url":"https://safe-genesis.getvela.app/api/records","body":"{}"}"#;
        let verdict: WriteVerdict = serde_json::from_str(&write_verdict_json(ask)).unwrap();
        assert_eq!(
            verdict,
            WriteVerdict::Refused {
                host: "safe-genesis.getvela.app".into(),
                key: REFUSED_KEY.into(),
                message: REFUSED_ENGLISH.into(),
            }
        );
        let defaults: ServiceDefaults = serde_json::from_str(&service_defaults_json()).unwrap();
        assert_eq!(defaults, service_defaults());
    }
}
