//! Two jobs: stamp the build's commit into the binary (every platform), and
//! embed the Windows application icon (Windows only).
//!
//! ## The commit (spec 064 §3)
//!
//! "About" used to show `6ab8f` — the design mock's constant — on every build
//! ever shipped. It now shows `VELA_GIT_COMMIT`, which is, first that answers:
//! the environment variable of that name (CI sets it from the commit it checked
//! out; a container build often cannot ask git itself — `dubious ownership`),
//! then `git rev-parse`, then the word `unknown`. Never a constant that looks
//! like a commit.
//!
//! ## The icon
//!
//! gpui does not take an icon from `WindowOptions`; on Windows it calls
//! `LoadImageW(module, MAKEINTRESOURCE(1), IMAGE_ICON, ..)` against the running
//! executable's own resources, and the call site is
//! `load_icon().unwrap_or_default()`. A miss is therefore silent: the window
//! class registers a null HICON and Windows substitutes its generic
//! application icon in the title bar, the taskbar and Alt-Tab. The Start-menu
//! and desktop shortcuts inherit the same nothing, because Inno Setup points
//! them at this executable.
//!
//! So the icon must exist AND must be resource id 1. `set_icon_with_id` states
//! that explicitly rather than relying on the default numbering.

/// First seven characters of the build's commit, or `unknown`.
fn commit() -> String {
    let from_env = std::env::var("VELA_GIT_COMMIT")
        .ok()
        .filter(|v| !v.trim().is_empty());
    let full = from_env.or_else(|| {
        std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .filter(|out| out.status.success())
            .and_then(|out| String::from_utf8(out.stdout).ok())
    });
    match full {
        Some(sha) if sha.trim().len() >= 7 => sha.trim()[..7].to_owned(),
        _ => "unknown".to_owned(),
    }
}

fn main() {
    println!("cargo:rustc-env=VELA_GIT_COMMIT={}", commit());
    println!("cargo:rerun-if-env-changed=VELA_GIT_COMMIT");
    // A new commit is a new stamp: HEAD moves when the branch does, and the
    // ref file it names moves when a commit lands on that branch.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");
    println!("cargo:rerun-if-changed=build.rs");

    // CARGO_CFG_TARGET_OS, not #[cfg(windows)]: the latter describes the
    // machine running this build script, which is wrong when cross-compiling
    // to Windows from Linux or macOS.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let icon = "packaging/icons/app.getvela.VelaWallet.ico";
    println!("cargo:rerun-if-changed={icon}");
    println!("cargo:rerun-if-changed=build.rs");

    let mut res = winresource::WindowsResource::new();
    res.set_icon_with_id(icon, "1");
    if let Err(e) = res.compile() {
        // Failing loudly here beats shipping an installer whose every shortcut
        // shows a blank icon.
        panic!("failed to embed the Windows application icon from {icon}: {e}");
    }
}
