# Quickstart — seeing each success criterion on the running apps

## Prerequisites

```sh
# web
cd app-web/vela-wallet && pnpm install
# desktop (parallel space: no authenticator needed)
cd app-desktop/vela-wallet
# core
cargo test -p vela-core --features i18n-all,crux
```

## Part A

| SC | How to see it |
| --- | --- |
| 411/412 | `pnpm dev` → `/en?intro` at 1440×900: rail + column; at 390×844: unchanged phone layout. e2e: `pnpm e2e -- welcome-layout` (new intro cases). |
| 413/414/415 | Clear site data → load `/en`: no Welcome frame before the intro. `pnpm test -- gate` asserts `app.html` ↔ `gate.ts`. `pnpm e2e -- welcome-ssr` stays green. |
| 416/417 | `rm` the desktop state file (`VELA_STATE_DIR=/tmp/vela038 cargo run --features dev-fixtures`): launch animation → intro (drag, ←/→, button, Skip) → Welcome; relaunch: no intro. `cargo test intro_art` pins the paths to the contract. |
| 418/419/420 | Desktop Welcome: no Settings in the rail. Point `vela.registry.endpoint` at `https://127.0.0.1:9` → the endpoint card appears under the actions; Close closes; Create / sign-in remain pressable. |
| 422 | Relaunch within 7 days: no animation; set `vela.launch.played` to 8 days ago: animation. `cargo test launch_gate`. |
| 423/424 | Side by side at 100%: web `/en` vs desktop Welcome — wordmark, headline, button label same face and weight. `cargo test fonts_bundled` asserts the four TTFs load. |
| 425/426 | `VELA_PARALLEL_SPACE=1 cargo run --features dev-fixtures` documented in the desktop README; running the bare binary and choosing "This device" shows the application-identifier sentence, not the fingerprint one. |
| 427 | Desktop sign-in → Phone or tablet → scan → cancel on the phone: the "Check your phone" card is gone, the failure sheet names the cancel, Back works. `cargo test hybrid_teardown` closes the port under the client. |
| 428 | Method rows on web `/en` (create AND sign-in) and desktop Welcome show icons; on this Mac in Chrome "This device" = Chrome-on-Mac mark, in Safari = Apple mark; desktop = Apple mark. |

## Part B

| SC | How to see it |
| --- | --- |
| 430 | On a Mac with only System Settings proxy set (no env): desktop reaches the registry (`cargo test --features dev-fixtures the_deployed_registry_answers_its_health_probe` run from a shell with `env -i`). |
| 431 | `all_proxy=socks5://127.0.0.1:1 cargo run …` with a working system proxy: reaches the network; with `scutil` proxies off and no env, `networksetup -setnetworkserviceenabled Wi-Fi off`: the sheet says this machine could not get out. |
| 432 | `VELA_TEST_PANIC=1` (new dev seam) → the failure sheet with "Report this error"; the window stays. |
| 433 | Fault harness `vela.bundlerDown()` (web) / relay unreachable (desktop): fee line reads as an unavailable estimate. |
| 434 | Clear state, cut the network, launch: skeleton + "unreachable" line, never $0.00 — web via `vela.rpcDown()` in the console, desktop via `VELA_STATE_DIR` fresh + network off. |
| 435 | Web: DevTools → Network → Offline → Online: the line appears/disappears, balances refresh without reload. Deploy a new build to the preview while a tab is open: next navigation reloads. |

## Part C

| SC | How to see it |
| --- | --- |
| 436 | Web home with 7 networks: figure changes once at settle; no "couldn't be priced" mid-stream. `cargo test balance_settled_figure`. |
| 437 | Incognito `/en`: sailboat in the tab. |
| 438 | Add-passkey rows on both shells carry the contract's icons (see 428); no bordered boxes remain. |
| 439 | Contacts: tap a history-suggested row → name it → reload: name kept. |
