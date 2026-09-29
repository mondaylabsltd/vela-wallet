# Research: 083 — the dApp browser on Windows

Evidence for every row is `path:line` at `main` de93634f unless a commit is named. `wry` = wry 0.56.1,
`wv2` = webview2-com 0.38.2 (+ `-sys`), `gpui` = zed c97b7c0 — all in the cargo registry/git cache.

## R0 — Audit: what 079 built for the desktop, as it behaves on WebView2

| Item | Verdict on Windows | Evidence |
|---|---|---|
| Profile folder | ✗ none given → `<exe>\vela-wallet.exe.WebView2`; Program Files refuses (0x80070005) | `webview.rs:446-456`; `wry/webview2/mod.rs:293-351`; installer `{autopf}` + admin (`VelaWallet.iss:36,54`) |
| Failed engine start | ✗ stderr only; rebuilt every paint; one WER report per attempt | `webview.rs:195-199,472-475`; Application log 14:50:45–14:51:07 |
| Load events | ✗ `Started` = ContentLoading, `Finished` = NavigationCompleted; IsErrorPage / IsSuccess / WebErrorStatus dropped | `wry/webview2/mod.rs:707-735` |
| Failure panel (079 US3) | ✗ taken down by Edge's error page (a "commit") | `load_watch.rs:135-146`, `page.rs:13169-13178` |
| Certificate | ✗ Edge interstitial with "continue anyway" | device, `expired.badssl.com` |
| Renderer crash | ✗ `Load::Crashed` is macOS-only; ProcessFailed never subscribed | `webview.rs:118-120,457-461` |
| New windows | ✗ wry's default handler sets Handled with no window | `wry/webview2/mod.rs:846-848` |
| Other schemes / downloads | ✗ engine defaults (Windows "Choose an app"; silent save) | `wry/lib.rs:868`, device |
| Address bar Enter | ✗ gpui keyboard click reopens editing on the old URL | `gpui/src/elements/div.rs:2790-2850`, `page.rs:12627-12635` |
| Demo fixtures in a live session | ✗ host `app.uniswap.org`, tabs "Uniswap · Polymarket" | `page.rs:12306-12312,12511-12514` |
| dApp signing via a phone key | ✗ QR published, never drawn; scan not stopped on close | `passkey.rs:891`, `signing_host.rs:176-177`, `page.rs:13503-13509` |
| Provider, consent, sign, send, switch chain | ✓ | device pass (spec.md "What held up") |

## R1 — The profile folder

**Decision**: `wry::WebContext::new(Some(%LOCALAPPDATA%\VelaWallet\WebView2))`, or `<VELA_STATE_DIR>\WebView2`;
`WebViewBuilder::new_with_web_context` (`wry/lib.rs:921`, `web_context.rs:45`); the context lives in `Browser`.
**Rationale**: local, not roaming (a cache); always the person's to write; isolated runs get their own
cookies. macOS ignores the context (`wkwebview` never reads it), so it stays `cfg(windows)`.
**Alternatives**: a per-user installer (D5 — also fixes it, moves every install); the
`WEBVIEW2_USER_DATA_FOLDER` variable (process-wide, invisible to Erase).
**Residual**: a profile folder that exists but is unusable makes WebView2 wait forever (the
engine's callback never comes); only an artificial ACL produced it here. Not handled.

## R2 — A failed start is a state

**Decision**: `build()` returns `Result<Browser, EngineFailure>`; `FAILED` stops rebuilding until the
person presses Retry; HRESULT `0x80070002` → runtime missing (system browser first), `0x80070005` →
access denied. The panel reuses `connect.browser.loadFailed`, `connect.browser.retry`,
`explore.openInSystemBrowser` (no new keys).

## R3 — The address bar and gpui's keyboard click

**Finding**: gpui arms a keyboard click on Enter/Space key-DOWN for a focused element with
`on_click` (`div.rs:2800-2812`) and fires it on key-UP if focus did not move (`:2832-2850`). The arming
listener is registered before the element's own `on_key_down` (`div.rs:2383` vs `:2392`), so
`prevent_default` there is too late.
**Decision**: the bar ignores `ClickEvent::is_keyboard()`; Enter and Esc move focus to the page root
(`window.focus`, which bumps `focus_generation` and cancels the pending click); the bar names
`LoadWatch::named_url()` (the load asked for, or the one that failed) until a page commits; editing
starts from it; an untouched open bar follows a commit.
**Note**: this replaces 079 US3 AS1's "the bar keeps the committed host until then" with 083 FR-005
("the host being opened"), which is also what Chrome does for navigations the browser starts.

## R4 — WebView2's own events

**Decision**: `src/webview2_events.rs` subscribes on `WebViewExtWindows::webview()`
(`wry/lib.rs:2340-2389`): NavigationStarting (the pending address per NavigationId), ContentLoading
(`IsErrorPage` → not a commit; the view is hidden at once), NavigationCompleted (`!IsSuccess` and the
site never committed → `Failed{status}`), ProcessFailed (renderer / browser exited → `Crashed`),
`ICoreWebView2_14::ServerCertificateErrorDetected` (action CANCEL first; a failure only for the top
navigation's origin). wry's own page-load handler is not installed on Windows.
**Core**: `LoadPlatform::WebView2` (`"webview2"`) with the COREWEBVIEW2_WEB_ERROR_STATUS table:
14 → not a failure; 13 NotFound; 12 Refused; 7 Timeout; 6, 8–11 Offline (8 is how
ERR_EMPTY_RESPONSE arrives, as Android's -1); 1–5 Certificate; else Other.
**Observed statuses on the device**: drop → 8, black hole → 7 at ~40 s (9 when the proxy cut the
tunnel), expired certificate → 2 (certificate).

## R5 — New windows, schemes, downloads

**Decision**: a person's `target=_blank` / `window.open` of an http(s) address opens a new Vela tab
(`NewWindowRequested` + `IsUserInitiated`, read from WebView2 because wry drops the gesture flag);
`mailto:`/`tel:` go to Windows through `executor::opener` only on a tap; every other scheme is refused;
downloads are refused on both desktops. `window.opener` is not supported (one engine).

## R6 — The signing column and a phone-held key (W19)

**Finding**: the owner's request signed as the account whose key is recorded `usb,nfc,ble,hybrid`,
so the route is the phone (`send.rs:185-186`). `run_hybrid` publishes the QR into the ceremony
channel (`passkey.rs:891`); only the Send flow and onboarding draw it. The dApp signing column
shows "签名中…" for 90 s, the page then gets raw `-32603 no phone answered…`, and closing the column
does not stop the Bluetooth scan.
**Decision**: the column draws the QR / touch / timeout cards itself (existing keys), a scan that ran
out is not an answer to the page (the request stays; Retry or ✕), ✕ or Esc stops the scan, and the
host's Drop closes the channel. Routing is unchanged (founder rule: no per-signature chooser).

## R7 — Route choice per host (D3)

**Finding**: one process-wide route index; a timeout through the system proxy moves every caller to
Direct for 60 s (`proxy.rs:196-336`); from China that is the wrong direction.
**Decision**: remember the route per host; a failure moves only that host; a success pins it.

## R8 — Menus over the page (D2)

**Experiment**: `SetWindowRgn` on the `WRY_WEBVIEW` child with a rectangle cut out shows gpui's
pixels through the hole on the real screen while the rest of the page stays
(`evidence/after/d2-region-hole-experiment.jpg`).
**Decision**: on Windows, a menu anchored over the page cuts its rectangle out of the webview's
region instead of hiding the whole page; macOS keeps hiding.
