//! Updates (N-D3): how this app learns of a new version, proves a download
//! is the owner's, and hands it to the OS.
//!
//! **Off until the owner provides a signing key.** [`UPDATE_KEY`] is `None`
//! in this source, and while it is `None` the updater asks no server and
//! shows nothing: an update path that cannot prove where a file came from is
//! not shipped (README "Updates" says which three values turn it on).
//!
//! ## Why not Sparkle / Velopack / WinSparkle
//!
//! Researched for a gpui app (2026-10-31):
//!
//! - **Sparkle 2** (macOS) is the standard, but it is an Objective-C
//!   framework with its own XPC installer services that must be embedded,
//!   signed and sandbox-entitled in the bundle; it would be the only
//!   non-Rust runtime in the app, and it does nothing for Windows or Linux.
//!   Its trust model — an EdDSA-signed appcast plus Developer ID continuity —
//!   is the right one, and is what this file implements in Rust.
//! - **WinSparkle** is the same on Windows (a C++ DLL); **Velopack** covers
//!   all three but replaces the installer and the update format the owner
//!   already ships (Inno Setup, .dmg, Flatpak/RPM/DEB).
//! - **Zed** (gpui's own app) runs its own updater: a JSON release endpoint,
//!   a download, a helper that swaps the bundle — and trusts TLS and OS code
//!   signing for the rest. This file keeps that shape and adds the one thing
//!   a wallet needs on top: a signature by a key that is NOT the web host's.
//!
//! ## The trust model
//!
//! 1. A **release manifest** is fetched from [`FEED_URL`]: an envelope
//!    `{"payload": "<json text>", "signature": "<hex>"}`. The signature is
//!    ECDSA P-256 / SHA-256 over the payload's exact bytes, by the owner's
//!    release key; [`UPDATE_KEY`] is its public half, compiled in. A server
//!    (or anybody who controls DNS, a CDN or the hosting account) can serve
//!    a manifest, but cannot sign one.
//! 2. The payload names the **version**, an **issue time**, and per platform
//!    an artifact URL, its **SHA-256** and its **size**. A version not newer
//!    than this build's is no update (no downgrade), and a manifest issued
//!    before the last one this device accepted is refused (no replay of an
//!    old, vulnerable "latest").
//! 3. The **artifact** is downloaded ([`download`]) and kept only when its
//!    size and SHA-256 are the signed manifest's — never more bytes than were
//!    signed are read. Then the OS's own checks run on top:
//!    - **macOS** ([`install_macos`]): the artifact is the `.app` as a
//!      `ditto` zip. The unpacked bundle must verify (`codesign --verify
//!      --deep --strict`), carry this app's bundle id and this app's
//!      Developer ID **team** — the running app's own, read from its
//!      signature, so no team id lives in this source — pass Gatekeeper, and
//!      say the version the manifest signed. Then it replaces the running
//!      bundle in ONE step (`renamex_np(RENAME_SWAP)`, both on the same
//!      volume) and the app relaunches.
//!    - **Windows** ([`install_windows`]): the artifact is the Inno Setup
//!      installer. When this app is Authenticode-signed the installer must
//!      verify too (`WinVerifyTrust`); the installer opens as a person
//!      starts it (never `/SILENT`), and the app quits so it can upgrade in
//!      place.
//! 4. Nothing installs unasked: the prompt ([`prompt`]) offers, Install
//!    starts it, Later puts it away until the next launch, and an install
//!    waits while a submit is out (`relay::may_close`). **Store builds never
//!    run this**: the Mac App Store, Flathub and distribution packages update
//!    through their store ([`Channel::Store`]); a Linux tarball has no
//!    hand-off and is not offered.
//!
//! ## The local test flavor
//!
//! `--features update-test` (with `dev-fixtures` for a throwaway state
//! directory) compiles a key and a feed from the BUILD's environment and
//! accepts loopback `http://` — the whole path proved on one machine against
//! a key made on it (`scripts/update-e2e.sh`). [`TEST_FLAVOR_MARKER`] is in
//! such a binary, and `check-store-binary.sh` refuses any binary that holds
//! it.

pub mod download;
#[cfg(target_os = "macos")]
pub mod install_macos;
#[cfg(windows)]
pub mod install_windows;
pub mod prompt;

use serde::Deserialize;

/// The owner's release-signing public key: a P-256 SEC1 point, hex
/// (`04…`, 65 bytes). **`None` keeps the updater off.** See
/// README "Updates" for how to make the key and where it lives.
#[cfg(not(feature = "update-test"))]
pub const UPDATE_KEY: Option<&str> = None;
/// The test flavor's key: the public half of a pair made on this machine,
/// from the build's environment.
#[cfg(feature = "update-test")]
pub const UPDATE_KEY: Option<&str> = option_env!("VELA_UPDATE_TEST_KEY");

/// Where the signed manifest is served (owner hosting).
pub const RELEASE_FEED_URL: &str = "https://getvela.app/desktop/releases/stable.json";

/// The feed this build asks.
#[cfg(not(feature = "update-test"))]
pub const FEED_URL: &str = RELEASE_FEED_URL;
#[cfg(feature = "update-test")]
pub const FEED_URL: &str = match option_env!("VELA_UPDATE_TEST_FEED") {
    Some(url) => url,
    None => RELEASE_FEED_URL,
};

/// Whether this is the local test flavor (`--features update-test`).
pub const TEST_FLAVOR: bool = cfg!(feature = "update-test");

/// Present in a test-flavor binary and in no other: what
/// `scripts/check-store-binary.sh` looks for.
#[cfg(feature = "update-test")]
pub const TEST_FLAVOR_MARKER: &str = "vela-update-test-flavor:local-key:loopback-feed";

/// The store record: the last manifest's issue time this device accepted.
pub const STATE_KEY: &str = "vela.desktop.update";

/// The largest manifest read. A manifest is a few kilobytes.
const MAX_MANIFEST: usize = 256 * 1024;

/// This build's version, as the manifest is compared with it. The test
/// flavor may stand in a later one (`VELA_UPDATE_TEST_VERSION`, at build
/// time) to make the "newer" build of the local proof.
#[must_use]
pub fn current_version() -> &'static str {
    #[cfg(feature = "update-test")]
    if let Some(version) = option_env!("VELA_UPDATE_TEST_VERSION") {
        return version;
    }
    env!("CARGO_PKG_VERSION")
}

/// How this copy of the app is updated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    /// Downloaded from the owner (.dmg, Setup.exe, a tarball): this updater.
    Direct,
    /// A store or a package manager updates it: the Mac App Store's sandbox,
    /// a Flatpak, an RPM/DEB under `/usr`. This updater stays out.
    Store,
}

/// This process's channel.
#[must_use]
pub fn channel() -> Channel {
    // The App Sandbox sets this for every sandboxed process; only the Mac
    // App Store build is sandboxed. Flatpak sets FLATPAK_ID.
    if std::env::var_os("APP_SANDBOX_CONTAINER_ID").is_some()
        || std::env::var_os("FLATPAK_ID").is_some()
    {
        return Channel::Store;
    }
    // An RPM or DEB installs under /usr and updates with the system.
    if cfg!(target_os = "linux")
        && std::env::current_exe().is_ok_and(|exe| exe.starts_with("/usr"))
    {
        return Channel::Store;
    }
    Channel::Direct
}

/// Whether this platform has an install hand-off: macOS replaces the
/// bundle, Windows runs the installer. A Linux tarball has none, so it is
/// offered nothing it could not install.
pub const HAS_HAND_OFF: bool = cfg!(any(target_os = "macos", windows));

/// Whether this build may look for updates at all.
#[must_use]
pub fn enabled() -> bool {
    UPDATE_KEY.is_some() && HAS_HAND_OFF && channel() == Channel::Direct
}

/// This build's platform key in a manifest.
#[must_use]
pub fn platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "macos-aarch64",
        ("macos", _) => "macos-x86_64",
        ("windows", "aarch64") => "windows-aarch64",
        ("windows", _) => "windows-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        _ => "linux-x86_64",
    }
}

/// Whether a feed or artifact URL may be fetched: `https://` only — and,
/// in the test flavor alone, `http://` to this machine.
#[must_use]
pub fn url_allowed(url: &str) -> bool {
    url.starts_with("https://") || (TEST_FLAVOR && is_loopback_http(url))
}

fn is_loopback_http(url: &str) -> bool {
    ["http://127.0.0.1:", "http://127.0.0.1/", "http://localhost:", "http://localhost/"]
        .iter()
        .any(|prefix| url.starts_with(prefix))
}

#[derive(Debug, Deserialize)]
struct Envelope {
    payload: String,
    signature: String,
}

/// One platform's download.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Artifact {
    pub url: String,
    /// Lowercase hex.
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Deserialize)]
struct Payload {
    version: String,
    /// Unix seconds.
    issued_at: u64,
    #[serde(default)]
    notes_url: Option<String>,
    platforms: std::collections::BTreeMap<String, Artifact>,
}

/// A newer release for this platform, from a manifest that verified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: String,
    pub issued_at: u64,
    pub notes_url: Option<String>,
    pub artifact: Artifact,
}

/// The update prompt's words (`componentsUi.appUpdate.*`): "Update
/// available", Install, Later — "Try again" after a download or an install
/// that was refused, and why it was refused — in the person's language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PromptWords {
    pub title: gpui::SharedString,
    pub install: gpui::SharedString,
    pub later: gpui::SharedString,
    pub retry: gpui::SharedString,
    /// Each refusal's sentence, in [`Refusal::WORDS`] order.
    refused: [gpui::SharedString; Refusal::WORDS.len()],
}

impl PromptWords {
    #[must_use]
    pub fn of(loc: &crate::loc::Loc) -> Self {
        Self {
            title: loc.t("componentsUi.appUpdate.title"),
            install: loc.t("componentsUi.appUpdate.install"),
            later: loc.t("componentsUi.appUpdate.later"),
            retry: loc.t("componentsUi.relayUnreachable.retryBtn"),
            refused: Refusal::WORDS
                .map(|word| loc.t(&format!("componentsUi.appUpdate.refused.{word}"))),
        }
    }

    /// What happened, and what the person can do: the line under the red
    /// bar.
    #[must_use]
    pub fn refusal(&self, refusal: &Refusal) -> gpui::SharedString {
        let word = refusal.word();
        Refusal::WORDS
            .iter()
            .position(|known| *known == word)
            .and_then(|at| self.refused.get(at))
            .cloned()
            .unwrap_or_default()
    }
}

/// Why a manifest, a download or an install was not taken. None of these is
/// shown as an update; each is logged, and the prompt says a refused
/// download or install in words under its red bar — offering "Try again"
/// only where another attempt can go differently ([`Refusal::retryable`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The updater has no key (this build).
    NoKey,
    /// Not the envelope's shape, or too big.
    Unreadable,
    /// The signature is not the release key's.
    BadSignature,
    /// Signed, but older than a manifest this device already took.
    Replayed,
    /// Not newer than this build.
    NotNewer,
    /// Nothing for this platform.
    NoArtifact,
    /// An artifact URL that is not https.
    InsecureUrl,
    /// The download is not the signed size or hash.
    ArtifactMismatch,
    /// The feed or the download did not answer, or stopped.
    Unreachable,
    /// This copy is not an installed bundle (a `cargo run` binary), so
    /// there is nothing to replace.
    NotInstalled,
    /// The update's code signature is not this app's: another bundle id,
    /// another team, an ad-hoc signature where this app has a team, or one
    /// that does not verify at all.
    SignatureMismatch,
    /// The bundle says another version than the manifest signed.
    VersionMismatch,
    /// The OS refused a step of the install (a folder it may not write, a
    /// swap that failed); the running app is untouched.
    InstallFailed(String),
}

impl Refusal {
    /// The corpus's word for each refusal
    /// (`componentsUi.appUpdate.refused.<word>`).
    pub const WORDS: [&'static str; 13] = [
        "noKey",
        "unreadable",
        "badSignature",
        "replayed",
        "notNewer",
        "noArtifact",
        "insecureUrl",
        "artifactMismatch",
        "unreachable",
        "notInstalled",
        "signatureMismatch",
        "versionMismatch",
        "installFailed",
    ];

    /// This refusal's word in [`Self::WORDS`]. An install the OS refused
    /// keeps its detail for the log (the OS's own text, not the person's
    /// language); the card says the sentence.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Self::NoKey => "noKey",
            Self::Unreadable => "unreadable",
            Self::BadSignature => "badSignature",
            Self::Replayed => "replayed",
            Self::NotNewer => "notNewer",
            Self::NoArtifact => "noArtifact",
            Self::InsecureUrl => "insecureUrl",
            Self::ArtifactMismatch => "artifactMismatch",
            Self::Unreachable => "unreachable",
            Self::NotInstalled => "notInstalled",
            Self::SignatureMismatch => "signatureMismatch",
            Self::VersionMismatch => "versionMismatch",
            Self::InstallFailed(_) => "installFailed",
        }
    }

    /// Whether another attempt can go differently: a download that stopped
    /// or came in damaged, or an OS step that failed. The rest refuse the
    /// same signed release the same way every time, so the card offers no
    /// "Try again" that cannot work.
    #[must_use]
    pub fn retryable(&self) -> bool {
        matches!(
            self,
            Self::Unreachable | Self::ArtifactMismatch | Self::InstallFailed(_)
        )
    }
}

/// `"1.10.2"` → `(1, 10, 2)`. A pre-release or build suffix is no release.
fn parse_version(text: &str) -> Option<(u64, u64, u64)> {
    let mut parts = text.trim().split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    parts.next().is_none().then_some((major, minor, patch))
}

fn hex_bytes(text: &str) -> Option<Vec<u8>> {
    let text = text.trim().trim_start_matches("0x");
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(text.get(at..at + 2)?, 16).ok())
        .collect()
}

/// Verify a manifest against `key_hex` and pick this platform's release.
///
/// `current` is this build's version, `last_issued` the issue time of the
/// last manifest this device accepted (0 for none).
pub fn verify_manifest(
    bytes: &[u8],
    key_hex: &str,
    current: &str,
    platform: &str,
    last_issued: u64,
) -> Result<Release, Refusal> {
    use p256::ecdsa::signature::Verifier as _;
    use p256::ecdsa::{Signature, VerifyingKey};
    if bytes.len() > MAX_MANIFEST {
        return Err(Refusal::Unreadable);
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| Refusal::Unreadable)?;
    let key = hex_bytes(key_hex)
        .and_then(|sec1| VerifyingKey::from_sec1_bytes(&sec1).ok())
        .ok_or(Refusal::NoKey)?;
    let signature = hex_bytes(&envelope.signature)
        .and_then(|raw| {
            Signature::from_slice(&raw)
                .or_else(|_| Signature::from_der(&raw))
                .ok()
        })
        .ok_or(Refusal::BadSignature)?;
    key.verify(envelope.payload.as_bytes(), &signature)
        .map_err(|_| Refusal::BadSignature)?;
    // Only now is the payload the owner's words.
    let payload: Payload =
        serde_json::from_str(&envelope.payload).map_err(|_| Refusal::Unreadable)?;
    if payload.issued_at < last_issued {
        return Err(Refusal::Replayed);
    }
    let newer = match (parse_version(&payload.version), parse_version(current)) {
        (Some(offered), Some(running)) => offered > running,
        _ => false,
    };
    if !newer {
        return Err(Refusal::NotNewer);
    }
    let artifact = payload
        .platforms
        .get(platform)
        .cloned()
        .ok_or(Refusal::NoArtifact)?;
    if !url_allowed(&artifact.url) {
        return Err(Refusal::InsecureUrl);
    }
    Ok(Release {
        version: payload.version,
        issued_at: payload.issued_at,
        notes_url: payload.notes_url,
        artifact,
    })
}

/// The issue time of the last manifest this device accepted.
#[must_use]
pub fn last_issued() -> u64 {
    crate::executor::storage::read_value(STATE_KEY)
        .ok()
        .flatten()
        .and_then(|value| value.get("issued_at").and_then(serde_json::Value::as_u64))
        .unwrap_or(0)
}

/// Ask the feed once (blocking; call off the UI thread). The release on
/// offer, or why there is none (the updater off, the feed unreachable,
/// nothing newer signed). A manifest that verified is remembered by its issue
/// time, so an older one is never taken again.
pub fn check() -> Result<Release, Refusal> {
    let key = UPDATE_KEY.ok_or(Refusal::NoKey)?;
    if !enabled() {
        return Err(Refusal::NoKey);
    }
    #[cfg(feature = "update-test")]
    crate::diag::vlog!("update", "{TEST_FLAVOR_MARKER} feed={FEED_URL}");
    if !url_allowed(FEED_URL) {
        return Err(Refusal::InsecureUrl);
    }
    let body = fetch(FEED_URL, MAX_MANIFEST).ok_or(Refusal::Unreachable)?;
    let verdict = verify_manifest(
        &body,
        key,
        current_version(),
        platform(),
        last_issued(),
    );
    match &verdict {
        Ok(release) => {
            let _ = crate::executor::storage::write_value(
                STATE_KEY,
                serde_json::json!({ "issued_at": release.issued_at }),
            );
            crate::diag::vlog!(
                "update",
                "offered {} (running {}) issued_at={}",
                release.version,
                current_version(),
                release.issued_at
            );
        }
        Err(refusal) => crate::diag::vlog!("update", "manifest not taken: {refusal:?}"),
    }
    verdict
}

/// One GET on the person's proxy routes (spec 082 RD2), at most `limit`
/// bytes read.
fn fetch(url: &str, limit: usize) -> Option<Vec<u8>> {
    use std::io::Read as _;
    let mut response = crate::executor::proxy::with_routes(
        url,
        std::time::Duration::from_secs(20),
        |agent| agent.get(url).call(),
    )
    .ok()?;
    let mut body = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(limit as u64 + 1)
        .read_to_end(&mut body)
        .ok()?;
    (body.len() <= limit).then_some(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use p256::ecdsa::signature::Signer as _;
    use p256::ecdsa::{Signature, SigningKey};

    /// A test-only key: a fixed scalar, never a release key.
    fn signer() -> SigningKey {
        SigningKey::from_slice(&[7u8; 32]).unwrap_or_else(|e| unreachable!("{e}"))
    }

    /// The prompt's words are the corpus's in every language — never a key.
    /// Each refusal is its own sentence, and a language other than English
    /// says it in that language (not English's, fallen back to).
    #[test]
    fn the_prompts_words_resolve_in_every_language() {
        let en = PromptWords::of(&crate::loc::Loc::for_tag("en"));
        for (tag, loc) in crate::loc::Loc::every_language() {
            let words = PromptWords::of(&loc);
            for word in [&words.title, &words.install, &words.later, &words.retry] {
                assert!(
                    !word.is_empty() && !word.contains("componentsUi."),
                    "{tag}: {word}"
                );
            }
            let mut seen = std::collections::HashSet::new();
            for refusal in every_refusal() {
                let word = words.refusal(&refusal);
                assert!(
                    !word.is_empty() && !word.contains("componentsUi."),
                    "{tag}: {refusal:?}: {word}"
                );
                assert!(
                    seen.insert(word.clone()),
                    "{tag}: {refusal:?} says another refusal's {word}"
                );
                if tag != "en" {
                    assert_ne!(word, en.refusal(&refusal), "{tag}: {refusal:?} is English");
                }
            }
        }
        assert_eq!(
            en.refusal(&Refusal::ArtifactMismatch).as_ref(),
            "The download didn't match the signed release, so it wasn't installed. Try again."
        );
        assert_eq!(
            en.refusal(&Refusal::NotInstalled).as_ref(),
            "This copy isn't an installed app, so it can't update itself."
        );
        assert_eq!(
            en.refusal(&Refusal::InstallFailed("EACCES".to_owned())),
            en.refusal(&Refusal::InstallFailed(String::new())),
            "the OS's detail is the log's, not the card's"
        );
        assert_eq!(
            (
                en.title.as_ref(),
                en.install.as_ref(),
                en.later.as_ref(),
                en.retry.as_ref()
            ),
            ("Update available", "Install", "Later", "Try again")
        );
    }

    /// One refusal of each kind. The match makes a new variant a compile
    /// error here until it is listed (and worded in the corpus).
    fn every_refusal() -> Vec<Refusal> {
        let all = vec![
            Refusal::NoKey,
            Refusal::Unreadable,
            Refusal::BadSignature,
            Refusal::Replayed,
            Refusal::NotNewer,
            Refusal::NoArtifact,
            Refusal::InsecureUrl,
            Refusal::ArtifactMismatch,
            Refusal::Unreachable,
            Refusal::NotInstalled,
            Refusal::SignatureMismatch,
            Refusal::VersionMismatch,
            Refusal::InstallFailed("the OS's own words".to_owned()),
        ];
        for refusal in &all {
            match refusal {
                Refusal::NoKey
                | Refusal::Unreadable
                | Refusal::BadSignature
                | Refusal::Replayed
                | Refusal::NotNewer
                | Refusal::NoArtifact
                | Refusal::InsecureUrl
                | Refusal::ArtifactMismatch
                | Refusal::Unreachable
                | Refusal::NotInstalled
                | Refusal::SignatureMismatch
                | Refusal::VersionMismatch
                | Refusal::InstallFailed(_) => {}
            }
        }
        all
    }

    /// Each refusal has its own word, the list's; "Try again" only where
    /// another attempt can go differently.
    #[test]
    fn every_refusal_has_its_own_word() {
        let words: Vec<_> = every_refusal().iter().map(Refusal::word).collect();
        assert_eq!(words, Refusal::WORDS);
        let retryable: Vec<_> = every_refusal()
            .iter()
            .filter(|refusal| refusal.retryable())
            .map(Refusal::word)
            .collect();
        assert_eq!(retryable, ["artifactMismatch", "unreachable", "installFailed"]);
    }

    fn key_hex() -> String {
        let point = signer().verifying_key().to_encoded_point(false);
        point.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
    }

    fn sign(payload: &str, with: &SigningKey) -> Vec<u8> {
        let signature: Signature = with.sign(payload.as_bytes());
        let hex: String = signature
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        serde_json::to_vec(&serde_json::json!({ "payload": payload, "signature": hex }))
            .unwrap_or_default()
    }

    fn payload(version: &str, issued_at: u64, url: &str) -> String {
        serde_json::json!({
            "version": version,
            "issued_at": issued_at,
            "notes_url": "https://getvela.app/releases",
            "platforms": {
                "macos-aarch64": { "url": url, "sha256": "ab".repeat(32), "size": 3 }
            }
        })
        .to_string()
    }

    const URL: &str = "https://getvela.app/d/Vela.dmg";

    #[test]
    fn a_newer_release_signed_by_the_release_key_is_taken() {
        let manifest = sign(&payload("0.10.0", 100, URL), &signer());
        let release = verify_manifest(&manifest, &key_hex(), "0.9.8", "macos-aarch64", 50);
        assert_eq!(
            release.map(|r| (r.version, r.issued_at)),
            Ok(("0.10.0".to_owned(), 100))
        );
    }

    /// openssl's `dgst -sha256 -sign` writes DER, which is what the README's
    /// recipe produces: it verifies as well as the raw r‖s.
    #[test]
    fn a_der_signature_is_read_too() {
        let text = payload("0.10.0", 100, URL);
        let signature: Signature = signer().sign(text.as_bytes());
        let der: String = signature
            .to_der()
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let manifest =
            serde_json::to_vec(&serde_json::json!({ "payload": text, "signature": der }))
                .unwrap_or_default();
        assert!(verify_manifest(&manifest, &key_hex(), "0.9.8", "macos-aarch64", 0).is_ok());
    }

    #[test]
    fn anybody_elses_signature_or_an_edited_payload_is_refused() {
        let stranger = SigningKey::from_slice(&[9u8; 32]).unwrap_or_else(|e| unreachable!("{e}"));
        let forged = sign(&payload("0.10.0", 100, URL), &stranger);
        assert_eq!(
            verify_manifest(&forged, &key_hex(), "0.9.8", "macos-aarch64", 0),
            Err(Refusal::BadSignature)
        );
        // Signed, then the URL swapped on the way.
        let signed = payload("0.10.0", 100, URL);
        let mut envelope: serde_json::Value =
            serde_json::from_slice(&sign(&signed, &signer())).unwrap_or_default();
        envelope["payload"] =
            serde_json::Value::String(signed.replace(URL, "https://evil.example/x.dmg"));
        let tampered = serde_json::to_vec(&envelope).unwrap_or_default();
        assert_eq!(
            verify_manifest(&tampered, &key_hex(), "0.9.8", "macos-aarch64", 0),
            Err(Refusal::BadSignature)
        );
        assert_eq!(
            verify_manifest(b"not json", &key_hex(), "0.9.8", "macos-aarch64", 0),
            Err(Refusal::Unreadable)
        );
    }

    #[test]
    fn no_downgrade_no_replay_no_plain_http() {
        let same = sign(&payload("0.9.8", 100, URL), &signer());
        assert_eq!(
            verify_manifest(&same, &key_hex(), "0.9.8", "macos-aarch64", 0),
            Err(Refusal::NotNewer)
        );
        let older = sign(&payload("0.9.7", 100, URL), &signer());
        assert_eq!(
            verify_manifest(&older, &key_hex(), "0.9.8", "macos-aarch64", 0),
            Err(Refusal::NotNewer)
        );
        let pre = sign(&payload("0.10.0-beta.1", 100, URL), &signer());
        assert_eq!(
            verify_manifest(&pre, &key_hex(), "0.9.8", "macos-aarch64", 0),
            Err(Refusal::NotNewer)
        );
        let old = sign(&payload("0.10.0", 100, URL), &signer());
        assert_eq!(
            verify_manifest(&old, &key_hex(), "0.9.8", "macos-aarch64", 101),
            Err(Refusal::Replayed)
        );
        let http = sign(&payload("0.10.0", 100, "http://getvela.app/x.dmg"), &signer());
        assert_eq!(
            verify_manifest(&http, &key_hex(), "0.9.8", "macos-aarch64", 0),
            Err(Refusal::InsecureUrl)
        );
        assert_eq!(
            verify_manifest(&old, &key_hex(), "0.9.8", "windows-x86_64", 0),
            Err(Refusal::NoArtifact)
        );
    }

    /// Loopback http:// is the test flavor's alone; a release build takes
    /// https:// and nothing else, for the feed and for the artifact.
    #[test]
    fn loopback_http_is_the_test_flavors_alone() {
        let local = sign(
            &payload("0.10.0", 100, "http://127.0.0.1:8765/Vela.zip"),
            &signer(),
        );
        let verdict = verify_manifest(&local, &key_hex(), "0.9.8", "macos-aarch64", 0);
        assert_eq!(verdict.is_ok(), TEST_FLAVOR);
        assert_eq!(url_allowed("http://localhost:1/x"), TEST_FLAVOR);
        assert!(!url_allowed("http://127.0.0.1.evil.example/x"));
        assert!(!url_allowed("http://getvela.app/x"));
        assert!(!url_allowed("file:///tmp/x"));
        assert!(url_allowed("https://getvela.app/x"));
    }

    #[test]
    #[cfg(not(feature = "update-test"))]
    fn this_build_ships_with_the_updater_off() {
        // The owner's key is not in this source: no feed is asked, and no
        // update can be offered, until README "Updates" is followed.
        assert!(UPDATE_KEY.is_none());
        assert!(!enabled());
        assert_eq!(check(), Err(Refusal::NoKey));
        assert_eq!(FEED_URL, RELEASE_FEED_URL);
        assert!(FEED_URL.starts_with("https://"));
        assert_eq!(current_version(), env!("CARGO_PKG_VERSION"));
        assert_eq!(parse_version("1.10.2"), Some((1, 10, 2)));
        assert!(parse_version("1.10").is_none());
    }
}
