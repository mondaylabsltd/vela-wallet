# Implementation Plan: 095 — Mac App Store (TestFlight)

**Branch**: `095-mac-app-store` | **Date**: 2026-10-02 | **Spec**: [spec.md](spec.md)
**Input**: the 2026-10-02 audit of `main` @ `ec033f231` (blockers + should-fixes),
the owner's four decisions, the iOS store documents (`docs/store-submission/`).

## Summary

Add a Mac App Store path beside the Developer ID one, remove the private API
the store refuses, make the sandboxed app work as the unsandboxed one does,
and write the store documents. Most of the work is in the desktop shell and
its packaging; the one cross-shell change (About links) goes to all four
shells through the corpus.

## Technical context

- Desktop: Rust 2024, gpui (zed `c97b7c0`, git), wry 0.56 (WKWebView), objc2
  0.5 for the shell's own AppKit calls; builds arm64 + x86_64 on this Mac.
- Signing on this Mac: Apple Distribution, Apple Development (two same-named),
  Developer ID, 3rd Party Mac Developer Installer — all team `F9W689P9NE`;
  development profile `4fd1598b…` lists this Mac.
- Constraints: no upload, no push, no phones; the GUI only when the Mac has
  been idle ≥ 90 s; never touch `~/Library/Application Support/VelaWallet`.

## Principles check

| Principle | How |
|---|---|
| Rules once in the core | About links are a list each shell already owns (no core model exists); their words are corpus keys. The browser's scheme rule stays the desktop's one `scheme_of`/`other_app_leave` (083), now fed on macOS too — the iOS rule (main frame + link activated) expressed in it. |
| Parity | About links on desktop, iOS, Android, web. Menu bar, sandbox, popups: macOS only (no other shell has a menu bar or a sandboxed desktop build; iOS/Android browsers already open popups in place). |
| Stable experience | Store binary has no developer switch; panels and migration keep a tester's files and wallets where they expect them. |
| Minimal diffs | gpui vendored as upstream + 4 edits; MAS script reuses `build-macos-app.sh` for the build and assembly. |

## Design decisions

1. **Vendoring, not forking**: `app-desktop/vendor/gpui_macos` is upstream at
   the pinned commit (first commit byte-identical), the manifest's workspace
   inheritance written out, then exactly the private-API edits. `[patch]`
   on the zed git source; zed's own crates stay on the same source string so
   cargo unifies them. Lints: zed's `unexpected_cfgs = allow` carried.
2. **One developer gate**: `dev_env::var!/var_os!/flag!` expand to an env read
   under `cfg(any(test, debug_assertions, feature = "dev-fixtures"))` and to
   `None` otherwise — the switch name is not in a release binary, which makes
   the gate checkable on the artefact. `test` keeps `cargo test --release` on
   `VELA_STATE_DIR`. Debug builds keep honouring the switches (every existing
   recipe and `sweep-gallery.sh` is `cargo run`/`cargo build`); store builds
   are release builds without `dev-fixtures`, so they never do. Switches with
   their own `dev-fixtures`-only gate (`VELA_PARALLEL_SPACE`, `VELA_DEV_PROXY`)
   and reads in `#[cfg(test)]` code are unchanged.
3. **Profile-driven signing**: the identity is the keychain certificate the
   profile names (by SHA-1), as `release-macos-local.sh` already does for
   Developer ID; the profile's kind is read from what it provisions.
4. **Migration only in the store build**: measured that macOS moves the
   folder (no symlink); the `--dev` bundle never carries the file, so a
   developer's real wallet is never moved by a test run.
5. **Save panels**: `dirs::download_dir()` — `$HOME/Downloads`, the person's
   folder unsandboxed and the container's link to it sandboxed (the sandboxed
   panel ignores a directory outside the container; measured).
6. **Popups on macOS**: WebKit enforces the gesture
   (`javaScriptCanOpenWindowsAutomatically = NO`), wry's new-window hook turns
   a web address into a tab; a forwarding `WKNavigationDelegate` in front of
   wry's reads `navigationType`/`sourceFrame` for `mailto:`/`tel:` and
   forwards everything else.
7. **Menus**: gpui OS actions for the clipboard items (responder chain),
   Undo/Redo forwarded via `targetForAction:` (never to gpui's delegate),
   Edit chords bound in a context nothing sets so wells keep them; labels
   from `componentsUi.appMenu.*`; AppKit's duplicate "Enter Full Screen" off
   (`NSFullScreenMenuItemEverywhere`).
8. **Sandbox smoke**: the dev-signed store bundle + a `dev-fixtures` twin
   (same entitlements/signature) for the signed-in screens, plus a developer
   probe (`VELA_SANDBOX_PROBE`) for facts not visible on screen.

## Risks

- wry sets five WebKit preference keys by KVC (strings) — documented; not
  symbols or selectors.
- Universal Purchase gives the Mac the iOS price while the `.dmg` is free —
  owner's call.
- Generated i18n artefacts conflict with any branch that adds corpus keys
  (path ledger, catalogs, wasm hash) — regenerate on merge.
