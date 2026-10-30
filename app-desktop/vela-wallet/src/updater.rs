//! Updates (N-D3): how this app learns of a new version, and how it proves a
//! download is the owner's before anything runs it.
//!
//! **Off until the owner provides a signing key.** [`UPDATE_KEY`] is `None`
//! in this source, and while it is `None` the updater asks no server and
//! shows nothing: an update path that cannot prove where a file came from is
//! not shipped (README "Updates" lists what turns it on).
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
//! 3. The **artifact** is downloaded and accepted only when its size and
//!    SHA-256 are the signed manifest's ([`verify_artifact`]). Then — in the
//!    install hand-off, NOT built yet — the OS's own checks run on top:
//!    on macOS the downloaded app must carry this app's Developer ID team
//!    (`codesign`), on Windows the installer's Authenticode signature
//!    (`WinVerifyTrust`) — the owner's signing identities, neither of which
//!    this source holds.
//! 4. Nothing installs silently. macOS opens the verified disk image for
//!    the person to drag (as a first install did); Windows starts the
//!    verified installer, which upgrades in place. **Store builds never run
//!    this**: the Mac App Store, Flathub and distribution packages update
//!    through their store ([`Channel::Store`]).

use serde::Deserialize;

/// The owner's release-signing public key: a P-256 SEC1 point, hex
/// (`04…`, 65 bytes). **`None` keeps the updater off.** See
/// README "Updates" for how to make the key and where it lives.
pub const UPDATE_KEY: Option<&str> = None;

/// Where the signed manifest is served (owner hosting).
pub const FEED_URL: &str = "https://getvela.app/desktop/releases/stable.json";

/// The store record: the last manifest's issue time this device accepted.
pub const STATE_KEY: &str = "vela.desktop.update";

/// The largest manifest read. A manifest is a few kilobytes.
const MAX_MANIFEST: usize = 256 * 1024;

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

/// Whether this build may look for updates at all.
#[must_use]
pub fn enabled() -> bool {
    UPDATE_KEY.is_some() && channel() == Channel::Direct
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

/// Why a manifest or a download was not taken. None of these is shown as an
/// update; each is logged.
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
    if text.len() % 2 != 0 {
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
    if !artifact.url.starts_with("https://") {
        return Err(Refusal::InsecureUrl);
    }
    Ok(Release {
        version: payload.version,
        issued_at: payload.issued_at,
        notes_url: payload.notes_url,
        artifact,
    })
}

/// Whether downloaded bytes are the signed artifact: its size and SHA-256.
pub fn verify_artifact(bytes: &[u8], artifact: &Artifact) -> Result<(), Refusal> {
    let digest = vela_core::primitives::sha256(bytes);
    let expected = hex_bytes(&artifact.sha256).ok_or(Refusal::ArtifactMismatch)?;
    if bytes.len() as u64 != artifact.size || digest != expected {
        return Err(Refusal::ArtifactMismatch);
    }
    Ok(())
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

/// Ask the feed once (blocking; call off the UI thread). `None` when the
/// updater is off, the feed is unreachable or nothing newer is signed.
#[must_use]
pub fn check() -> Option<Release> {
    let key = UPDATE_KEY?;
    if channel() != Channel::Direct {
        return None;
    }
    let body = fetch(FEED_URL, MAX_MANIFEST)?;
    match verify_manifest(
        &body,
        key,
        env!("CARGO_PKG_VERSION"),
        platform(),
        last_issued(),
    ) {
        Ok(release) => {
            let _ = crate::executor::storage::write_value(
                STATE_KEY,
                serde_json::json!({ "issued_at": release.issued_at }),
            );
            Some(release)
        }
        Err(refusal) => {
            crate::diag::vlog!("update", "manifest not taken: {refusal:?}");
            None
        }
    }
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

    #[test]
    fn a_download_is_the_signed_bytes_or_nothing() {
        let bytes = b"abc";
        let digest: String = vela_core::primitives::sha256(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        let artifact = Artifact {
            url: URL.to_owned(),
            sha256: digest,
            size: 3,
        };
        assert_eq!(verify_artifact(bytes, &artifact), Ok(()));
        assert_eq!(
            verify_artifact(b"abd", &artifact),
            Err(Refusal::ArtifactMismatch)
        );
        assert_eq!(
            verify_artifact(b"abcd", &artifact),
            Err(Refusal::ArtifactMismatch)
        );
    }

    #[test]
    fn this_build_ships_with_the_updater_off() {
        // The owner's key is not in this source: no feed is asked, and no
        // update can be offered, until README "Updates" is followed.
        assert!(UPDATE_KEY.is_none());
        assert!(!enabled());
        assert!(check().is_none());
        assert!(FEED_URL.starts_with("https://"));
        assert_eq!(parse_version("1.10.2"), Some((1, 10, 2)));
        assert!(parse_version("1.10").is_none());
    }
}
