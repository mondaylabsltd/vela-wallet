//! The update's download: the signed artifact, to a file, with its progress
//! — and kept only when it is the signed size and SHA-256.
//!
//! The hash is taken as the bytes arrive, and never more bytes than the
//! manifest signed are read: a server that sends more is refused at the
//! first byte past the size, not after it has filled the disk.

use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use sha2::Digest as _;

use super::{Artifact, Refusal, url_allowed};

/// How long a whole download may take. ureq's global timeout covers the
/// body, and an installer is tens of megabytes on a slow line.
const DOWNLOAD_BUDGET: Duration = Duration::from_secs(30 * 60);

/// One read's size.
const CHUNK: usize = 64 * 1024;

/// Download `artifact` to `dest` (replaced). `progress` hears the bytes
/// received so far, at most once per chunk; `wanted` is asked between chunks
/// and a `false` stops the download (the person pressed Later). A file that
/// is not the signed bytes is removed, never left for an installer to find.
pub fn download(
    artifact: &Artifact,
    dest: &Path,
    progress: &dyn Fn(u64),
    wanted: &dyn Fn() -> bool,
) -> Result<(), Refusal> {
    if !url_allowed(&artifact.url) {
        return Err(Refusal::InsecureUrl);
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| Refusal::InstallFailed(format!("staging folder: {error}")))?;
    }
    let url = artifact.url.as_str();
    let mut response =
        crate::executor::proxy::with_routes(url, DOWNLOAD_BUDGET, |agent| agent.get(url).call())
            .map_err(|_| Refusal::Unreachable)?;
    let file = std::fs::File::create(dest)
        .map_err(|error| Refusal::InstallFailed(format!("download file: {error}")))?;
    let outcome = receive(
        response.body_mut().as_reader(),
        std::io::BufWriter::new(file),
        artifact,
        progress,
        wanted,
    );
    if outcome.is_err() {
        let _ = std::fs::remove_file(dest);
    }
    outcome
}

/// The body to `out`, hashed on the way: `Ok` only for exactly the signed
/// size and digest. The seam the tests drive.
pub fn receive(
    mut body: impl Read,
    mut out: impl Write,
    artifact: &Artifact,
    progress: &dyn Fn(u64),
    wanted: &dyn Fn() -> bool,
) -> Result<(), Refusal> {
    let mut hasher = sha2::Sha256::new();
    let mut received: u64 = 0;
    let mut buffer = vec![0u8; CHUNK];
    loop {
        if !wanted() {
            return Err(Refusal::Unreachable);
        }
        let read = match body.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => return Err(Refusal::Unreachable),
        };
        received += read as u64;
        if received > artifact.size {
            // More than was signed: not the artifact, and not read further.
            return Err(Refusal::ArtifactMismatch);
        }
        let bytes = buffer.get(..read).ok_or(Refusal::Unreachable)?;
        hasher.update(bytes);
        out.write_all(bytes)
            .map_err(|error| Refusal::InstallFailed(format!("download write: {error}")))?;
        progress(received);
    }
    out.flush()
        .map_err(|error| Refusal::InstallFailed(format!("download write: {error}")))?;
    let digest: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if received != artifact.size || digest != artifact.sha256.trim().to_ascii_lowercase() {
        return Err(Refusal::ArtifactMismatch);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact_of(bytes: &[u8]) -> Artifact {
        Artifact {
            url: "https://getvela.app/d/Vela.zip".to_owned(),
            sha256: vela_core::primitives::sha256(bytes)
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect(),
            size: bytes.len() as u64,
        }
    }

    fn always() -> bool {
        true
    }

    #[test]
    fn the_signed_bytes_are_kept_with_their_progress() {
        let bytes: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
        let artifact = artifact_of(&bytes);
        let seen = std::cell::RefCell::new(Vec::new());
        let mut out = Vec::new();
        let verdict = receive(
            bytes.as_slice(),
            &mut out,
            &artifact,
            &|done| seen.borrow_mut().push(done),
            &always,
        );
        assert_eq!(verdict, Ok(()));
        assert_eq!(out, bytes);
        let seen = seen.into_inner();
        assert!(seen.len() >= 3, "progress per chunk: {seen:?}");
        assert!(seen.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(seen.last(), Some(&200_000));
    }

    #[test]
    fn a_wrong_hash_a_short_or_a_long_body_is_refused() {
        let bytes = b"the signed artifact".to_vec();
        let artifact = artifact_of(&bytes);
        let mut flipped = bytes.clone();
        flipped[3] ^= 1;
        for (body, why) in [
            (flipped, "a byte changed"),
            (bytes[..10].to_vec(), "short"),
            ([bytes.as_slice(), b"!"].concat(), "long"),
        ] {
            let verdict = receive(body.as_slice(), Vec::new(), &artifact, &|_| {}, &always);
            assert_eq!(verdict, Err(Refusal::ArtifactMismatch), "{why}");
        }
    }

    /// A body past the signed size stops being read at the first byte over.
    #[test]
    fn nothing_past_the_signed_size_is_read() {
        let artifact = artifact_of(&[0u8; 10]);
        let endless = std::io::repeat(0);
        let mut out = Vec::new();
        let verdict = receive(endless, &mut out, &artifact, &|_| {}, &always);
        assert_eq!(verdict, Err(Refusal::ArtifactMismatch));
        assert!(out.len() <= 10, "wrote {} bytes", out.len());
    }

    #[test]
    fn later_stops_the_download() {
        let bytes = vec![1u8; 300_000];
        let artifact = artifact_of(&bytes);
        let reads = std::cell::Cell::new(0);
        let verdict = receive(
            bytes.as_slice(),
            Vec::new(),
            &artifact,
            &|_| {},
            &|| {
                reads.set(reads.get() + 1);
                reads.get() < 2
            },
        );
        assert_eq!(verdict, Err(Refusal::Unreachable));
    }

    #[test]
    fn an_insecure_url_is_never_fetched() {
        let mut artifact = artifact_of(b"x");
        artifact.url = "http://getvela.app/x.zip".to_owned();
        let dest = std::env::temp_dir().join("vela-update-insecure-test.zip");
        assert_eq!(
            download(&artifact, &dest, &|_| {}, &always),
            Err(Refusal::InsecureUrl)
        );
        assert!(!dest.exists());
    }
}
