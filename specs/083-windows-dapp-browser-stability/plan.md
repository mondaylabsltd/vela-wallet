# Implementation Plan: 083 — the dApp browser works, and tells the truth, on Windows

**Branch**: `083-windows-dapp-browser-stability` | **Date**: 2026-09-28 | **Spec**: [spec.md](spec.md) |
**Research**: [research.md](research.md) | **Tasks**: [tasks.md](tasks.md) | **Device recipe**: [quickstart.md](quickstart.md)

## Summary

The Windows desktop's dApp browser is WebView2 through wry. 079 made the browser hold up on a bad
network, but its desktop design was verified on WKWebView only; on WebView2 the engine's own error
pages count as pages arriving, the installed app's engine cannot start at all, and the address bar
reopens itself on the previous site. 083 gives WebView2 a profile folder the person can write, reads
WebView2's own load / crash / certificate / new-window events beside wry, fixes the address bar's
keyboard click, draws the phone QR inside the signing column, and makes the proxy route per host.

## Technical Context

- **Desktop**: Rust 2024, gpui (zed c97b7c0), wry 0.56.1, webview2-com 0.38.2 + windows-core 0.61.2
  (added as direct Windows dependencies at exactly wry's versions).
- **Core**: `vela-core` `browser_load` (one new platform table); no wire or binding type change.
- **Target**: Windows 10/11 x64 (installer: `C:\Program Files\Vela Wallet`, admin). macOS behaviour
  unchanged except where stated; Linux has no browser.
- **Testing**: desktop unit tests (`cargo test --bin vela-wallet`), core tests (`--features crux`),
  and the device pass in [quickstart.md](quickstart.md) — release + `dev-fixtures` exe, the fault
  proxy, WebView2 over DevTools, PrintWindow captures, the Application log.

## Constitution Check

`.specify/memory/constitution.md` is the unfilled template; the repo's working rules apply instead:
every change verified on the device before it is called done (owner, 2026-09-24/25), one rule in the
core rather than per shell (079), no new i18n keys unless unavoidable (budget), commit per finished
task in the shared tree.

## Risk

- **High**: `webview2_events.rs` is Windows-only COM code no PR check builds — verified on this
  device only; a wry bump that moves webview2-com fails to compile rather than drifting.
- **Medium**: the per-host route change touches every network call; the signing-column QR touches
  the signing host shared with macOS.

## Phases

| Phase | Deliverable | Gate |
|---|---|---|
| 0 | Device pass, findings, evidence | spec.md committed (c2a137c0) |
| 1 (US1) | Profile folder, engine failure state, no demo host/tabs | read-only install opens the dApp; missing runtime → one panel |
| 2 (US2) | Address bar names the site being opened; typing replaces | 3 + 4 typed navigations on device |
| 3 (US3) | WebView2 events; core table | drop / black hole / certificate / crash on device |
| 4 | Signing column: phone QR, Esc, preparing label, plain transfer | owner's phone-key signature on the installed build |
| 5 | New windows, schemes, downloads; toolbar; consent rows; per-host route; menus over the page | device rows N1–N6 |
| 6 | Installer rebuilt, installed, results.md | SC table |

## Design decisions

1. The profile follows `VELA_STATE_DIR` (R1). 2. Engine retries are the person's (R2). 3. The bar
names the load being opened (R3; supersedes 079 US3 AS1). 4. WebView2 statuses are classified in the
core (R4). 5. New windows are tabs; only mailto/tel leave (R5). 6. The signing column owns its
ceremony cards (R6). 7. Routes are per host (R7). 8. Menus cut a hole, they do not hide the page (R8).

## Complexity tracking

| Addition | Why it is needed | Simpler alternative rejected because |
|---|---|---|
| Direct webview2-com dependency | wry drops IsErrorPage, IsSuccess, ProcessFailed, certificate and gesture flags | patching wry — a fork to maintain |
| `webview2_events.rs` state (pending NavigationId) | a failure must name the address asked for, not the engine page's | reading `Source` — it is the previous page until commit |
