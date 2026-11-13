//! The macOS install hand-off: a verified download becomes the app.
//!
//! The artifact is the `.app` as a `ditto` zip (`build-macos-app.sh --zip`).
//! It is unpacked NEXT TO the running bundle — the same volume, so the swap
//! below is one rename — and then held to the running app's own signature:
//!
//! 1. `codesign --verify --deep --strict`: every nested piece is sealed.
//! 2. The bundle id is this app's, and the **team** is the running app's,
//!    read from its own signature rather than from this source; with a team,
//!    `codesign -R` also asks for Apple's anchor and that team's leaf. An
//!    ad-hoc signature (no team) is refused wherever the running app has a
//!    team — and, in a release build, even where it has none: a build that
//!    cannot say whose it is never replaces itself. Only the local test
//!    flavor lets an ad-hoc bundle replace an ad-hoc one.
//! 3. Gatekeeper (`spctl --assess`) passes it, as it would a download the
//!    person opened — notarized (release builds; the test flavor's ad-hoc
//!    bundles are not).
//! 4. Its `CFBundleShortVersionString` is the version the manifest signed:
//!    a genuinely signed but OLDER bundle under a newer manifest is a
//!    downgrade, and refused.
//!
//! Then `renamex_np(RENAME_SWAP)` exchanges the new bundle and the running
//! one in a single step — there is no moment with no app, or half of one —
//! the old one is removed, and the app relaunches from the same path.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{Refusal, Release, TEST_FLAVOR};

/// The bundle id every package carries (`Info.plist.in`).
pub const BUNDLE_ID: &str = "app.getvela.VelaWallet";

/// What a code signature says about whose it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CodeSignature {
    /// `Identifier=`: the bundle id the signature seals.
    pub identifier: Option<String>,
    /// `TeamIdentifier=`: `None` for an ad-hoc signature ("not set").
    pub team: Option<String>,
}

/// Read `codesign -d --verbose=2`'s report (it writes to stderr).
#[must_use]
pub fn parse_codesign_display(report: &str) -> CodeSignature {
    let field = |name: &str| {
        report
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "not set")
            .map(str::to_owned)
    };
    CodeSignature {
        identifier: field("Identifier="),
        team: field("TeamIdentifier="),
    }
}

/// Whether `update` may replace `running`: the same bundle id, and the same
/// team. Two ad-hoc signatures match only when `allow_ad_hoc` (the test
/// flavor); a release build refuses an ad-hoc update outright.
pub fn signature_acceptable(
    running: &CodeSignature,
    update: &CodeSignature,
    allow_ad_hoc: bool,
) -> Result<(), Refusal> {
    if update.identifier.as_deref() != Some(BUNDLE_ID)
        || running.identifier.as_deref() != Some(BUNDLE_ID)
    {
        return Err(Refusal::SignatureMismatch);
    }
    match (&running.team, &update.team) {
        (Some(ours), Some(theirs)) if ours == theirs => Ok(()),
        (None, None) if allow_ad_hoc => Ok(()),
        _ => Err(Refusal::SignatureMismatch),
    }
}

/// The bundle this process runs from: `<X>.app/Contents/MacOS/<exe>`.
/// `None` for a bare binary (`cargo run`), which has nothing to replace.
#[must_use]
pub fn running_bundle() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?.canonicalize().ok()?;
    bundle_of(&exe)
}

fn bundle_of(exe: &Path) -> Option<PathBuf> {
    let macos = exe.parent()?;
    let contents = macos.parent()?;
    let bundle = contents.parent()?;
    (macos.file_name()? == "MacOS"
        && contents.file_name()? == "Contents"
        && bundle.extension()? == "app")
        .then(|| bundle.to_path_buf())
}

fn run(program: &str, args: &[&std::ffi::OsStr]) -> Option<std::process::Output> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .ok()
}

fn signature_of(app: &Path) -> Result<CodeSignature, Refusal> {
    let output = run(
        "/usr/bin/codesign",
        &["-d".as_ref(), "--verbose=2".as_ref(), app.as_os_str()],
    )
    .filter(|output| output.status.success())
    .ok_or(Refusal::SignatureMismatch)?;
    Ok(parse_codesign_display(&String::from_utf8_lossy(
        &output.stderr,
    )))
}

fn verifies(app: &Path, requirement: Option<&str>) -> bool {
    let requirement = requirement.map(|text| format!("={text}"));
    let mut args: Vec<&std::ffi::OsStr> =
        vec!["--verify".as_ref(), "--deep".as_ref(), "--strict".as_ref()];
    if let Some(requirement) = &requirement {
        args.push("-R".as_ref());
        args.push(requirement.as_ref());
    }
    args.push(app.as_os_str());
    run("/usr/bin/codesign", &args).is_some_and(|output| output.status.success())
}

fn gatekeeper_accepts(app: &Path) -> bool {
    run(
        "/usr/sbin/spctl",
        &[
            "--assess".as_ref(),
            "--type".as_ref(),
            "execute".as_ref(),
            app.as_os_str(),
        ],
    )
    .is_some_and(|output| output.status.success())
}

fn bundle_version(app: &Path) -> Option<String> {
    let plist = app.join("Contents/Info.plist");
    let output = run(
        "/usr/bin/plutil",
        &[
            "-extract".as_ref(),
            "CFBundleShortVersionString".as_ref(),
            "raw".as_ref(),
            "-o".as_ref(),
            "-".as_ref(),
            plist.as_os_str(),
        ],
    )?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// Every check above, on an unpacked bundle, against the running one.
pub fn check_bundle(update: &Path, running: &Path, release: &Release) -> Result<(), Refusal> {
    if !verifies(update, None) {
        return Err(Refusal::SignatureMismatch);
    }
    let ours = signature_of(running)?;
    let theirs = signature_of(update)?;
    signature_acceptable(&ours, &theirs, TEST_FLAVOR)?;
    if let Some(team) = &ours.team {
        let requirement = format!(
            "identifier \"{BUNDLE_ID}\" and anchor apple generic and certificate leaf[subject.OU] = \"{team}\""
        );
        if !verifies(update, Some(&requirement)) {
            return Err(Refusal::SignatureMismatch);
        }
    }
    if !TEST_FLAVOR && !gatekeeper_accepts(update) {
        return Err(Refusal::SignatureMismatch);
    }
    if bundle_version(update).as_deref() != Some(release.version.as_str()) {
        return Err(Refusal::VersionMismatch);
    }
    Ok(())
}

/// Unpack, check and swap in the verified `zip`. Answers the bundle's path
/// (unchanged: the new app now lives where the old one did). On any refusal
/// the running app is untouched and the staging folder is gone.
pub fn install(zip: &Path, release: &Release) -> Result<PathBuf, Refusal> {
    let running = running_bundle().ok_or(Refusal::NotInstalled)?;
    let parent = running.parent().ok_or(Refusal::NotInstalled)?.to_path_buf();
    let stage = parent.join(format!(".vela-update-{}", release.version));
    let _ = std::fs::remove_dir_all(&stage);
    std::fs::create_dir(&stage).map_err(|error| {
        Refusal::InstallFailed(format!("{} is not writable: {error}", parent.display()))
    })?;
    let outcome = unpack_check_swap(zip, &stage, &running, release);
    // Whatever happened, the staging folder holds nothing worth keeping:
    // the update that failed a check, or (after the swap) the old app.
    let _ = std::fs::remove_dir_all(&stage);
    outcome.map(|()| running)
}

fn unpack_check_swap(
    zip: &Path,
    stage: &Path,
    running: &Path,
    release: &Release,
) -> Result<(), Refusal> {
    let unpacked = run(
        "/usr/bin/ditto",
        &[
            "-x".as_ref(),
            "-k".as_ref(),
            zip.as_os_str(),
            stage.as_os_str(),
        ],
    )
    .is_some_and(|output| output.status.success());
    if !unpacked {
        return Err(Refusal::InstallFailed("the archive does not unpack".into()));
    }
    let apps: Vec<PathBuf> = std::fs::read_dir(stage)
        .map_err(|error| Refusal::InstallFailed(error.to_string()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "app"))
        .collect();
    let [update] = apps.as_slice() else {
        return Err(Refusal::InstallFailed(format!(
            "the archive holds {} apps, not one",
            apps.len()
        )));
    };
    check_bundle(update, running, release)?;
    swap(update, running)
}

/// Exchange two directories on one volume, atomically.
fn swap(update: &Path, running: &Path) -> Result<(), Refusal> {
    use std::os::unix::ffi::OsStrExt as _;
    let c = |path: &Path| std::ffi::CString::new(path.as_os_str().as_bytes()).ok();
    let (Some(from), Some(to)) = (c(update), c(running)) else {
        return Err(Refusal::InstallFailed("a path with a NUL".into()));
    };
    // SAFETY: two valid NUL-terminated paths; renamex_np reads them only.
    let status = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_SWAP) };
    if status == 0 {
        Ok(())
    } else {
        Err(Refusal::InstallFailed(format!(
            "the swap failed: {}",
            std::io::Error::last_os_error()
        )))
    }
}

/// Start the app at `bundle` once this process has exited: a detached
/// `/bin/sh` waits for our pid, then `open -n`s the bundle, as a person
/// would. A developer build passes its `VELA_*` switches on (`open --env`),
/// so a relaunch keeps its throwaway state directory.
pub fn relaunch(bundle: &Path) -> Result<(), Refusal> {
    use std::os::unix::process::CommandExt as _;
    let mut args: Vec<std::ffi::OsString> = vec![
        "-c".into(),
        "pid=$1; shift; while kill -0 \"$pid\" 2>/dev/null; do sleep 0.2; done; exec /usr/bin/open -n \"$@\""
            .into(),
        "vela-relaunch".into(),
        std::process::id().to_string().into(),
    ];
    #[cfg(any(debug_assertions, feature = "dev-fixtures"))]
    for (name, value) in std::env::vars_os() {
        if name.to_string_lossy().starts_with("VELA_") {
            let mut pair = name;
            pair.push("=");
            pair.push(value);
            args.push("--env".into());
            args.push(pair);
        }
    }
    // The local proof reads the relaunched app's log (`update-e2e.sh`).
    #[cfg(any(debug_assertions, feature = "dev-fixtures"))]
    if let Some(log) = crate::dev_env::var_os!("VELA_UPDATE_RELAUNCH_LOG") {
        args.push("--stderr".into());
        args.push(log);
    }
    args.push(bundle.as_os_str().to_owned());
    Command::new("/bin/sh")
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .map(drop)
        .map_err(|error| Refusal::InstallFailed(format!("relaunch: {error}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEVELOPER_ID: &str = "Executable=/Applications/Vela Wallet.app/Contents/MacOS/vela-wallet
Identifier=app.getvela.VelaWallet
Format=app bundle with Mach-O universal (x86_64 arm64)
Authority=Developer ID Application: Example Ltd (ABCDE12345)
Authority=Developer ID Certification Authority
Authority=Apple Root CA
TeamIdentifier=ABCDE12345";

    const AD_HOC: &str = "Executable=/tmp/Vela Wallet.app/Contents/MacOS/vela-wallet
Identifier=app.getvela.VelaWallet
Format=app bundle with Mach-O thin (arm64)
Signature=adhoc
TeamIdentifier=not set";

    fn sig(identifier: &str, team: Option<&str>) -> CodeSignature {
        CodeSignature {
            identifier: Some(identifier.to_owned()),
            team: team.map(str::to_owned),
        }
    }

    #[test]
    fn codesigns_report_is_read() {
        assert_eq!(
            parse_codesign_display(DEVELOPER_ID),
            sig(BUNDLE_ID, Some("ABCDE12345"))
        );
        assert_eq!(parse_codesign_display(AD_HOC), sig(BUNDLE_ID, None));
        assert_eq!(parse_codesign_display(""), CodeSignature::default());
    }

    #[test]
    fn only_this_apps_team_may_replace_it() {
        let ours = sig(BUNDLE_ID, Some("ABCDE12345"));
        assert_eq!(signature_acceptable(&ours, &ours, false), Ok(()));
        for update in [
            sig(BUNDLE_ID, Some("ZZZZZ99999")),
            sig(BUNDLE_ID, None),
            sig("com.example.other", Some("ABCDE12345")),
        ] {
            assert_eq!(
                signature_acceptable(&ours, &update, true),
                Err(Refusal::SignatureMismatch),
                "{update:?}"
            );
        }
        // Ad-hoc over ad-hoc: the local test flavor only. A release build
        // that cannot say whose it is never replaces itself.
        let ad_hoc = sig(BUNDLE_ID, None);
        assert_eq!(
            signature_acceptable(&ad_hoc, &ad_hoc, false),
            Err(Refusal::SignatureMismatch)
        );
        assert_eq!(signature_acceptable(&ad_hoc, &ad_hoc, true), Ok(()));
        assert_eq!(
            signature_acceptable(&ad_hoc, &ours, true),
            Err(Refusal::SignatureMismatch),
            "a team update over an ad-hoc app is not the same app either"
        );
    }

    #[test]
    fn a_bundle_is_found_from_its_executable_only() {
        assert_eq!(
            bundle_of(Path::new(
                "/Applications/Vela Wallet.app/Contents/MacOS/vela-wallet"
            )),
            Some(PathBuf::from("/Applications/Vela Wallet.app"))
        );
        assert_eq!(
            bundle_of(Path::new("/repo/target/release/vela-wallet")),
            None
        );
    }

    /// The swap is one step: both names exist before and after, and their
    /// contents trade places.
    #[test]
    fn the_swap_trades_two_folders_in_one_step() {
        let root = std::env::temp_dir().join(format!("vela-swap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (old, new) = (
            root.join("Vela Wallet.app"),
            root.join("stage/Vela Wallet.app"),
        );
        for (dir, text) in [(&old, "old"), (&new, "new")] {
            std::fs::create_dir_all(dir).unwrap_or_else(|e| unreachable!("{e}"));
            std::fs::write(dir.join("v"), text).unwrap_or_else(|e| unreachable!("{e}"));
        }
        assert_eq!(swap(&new, &old), Ok(()));
        let read = |dir: &Path| std::fs::read_to_string(dir.join("v")).unwrap_or_default();
        assert_eq!(
            (read(&old), read(&new)),
            ("new".to_owned(), "old".to_owned())
        );
        let _ = std::fs::remove_dir_all(&root);
    }
}
