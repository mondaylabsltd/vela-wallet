# Implementation Plan: 090 — Receive code: opt-in network (ERC-681)

**Branch**: `090-receive-network-qr` | **Date**: 2026-10-02 | **Spec**: [spec.md](spec.md)
**Input**: Owner ruling 2026-10-02 on #312 (ERC-681 opt-in, default bare address).

## Summary

One rule in the core, drawn by four shells. `payment_request` gains an address-mode switch: off → the
code is the bare address; on → `ethereum:<address>@<chain>` for the asset on screen. The view says
whether the switch is offered, where it sits and whether the calm hint shows. Every shell draws the switch
and hint from that view, encodes `qr_value` on screen and in the saved share card, and tells the machine
which network the code on screen is about.

## Technical Context

**Language/Version**: Rust (vela-core, desktop gpui), TypeScript/Svelte 5 (web + extension), Swift/SwiftUI
(iOS), Kotlin/Compose (Android)
**Primary Dependencies**: crux_core machines; wasm (web), UniFFI JSON bridges (iOS/Android), direct link
(desktop)
**Storage**: none. The switch is session-scoped: every `Start` turns it off.
**Testing**: cargo (core + desktop), vitest node + browser + Playwright e2e (web), Swift Testing + XCUITest
(iOS), JUnit (Android)
**Constraints**: corpus-only strings, 15 locales, keys ≤3 segments; SC-005 ja+en residency budget raised
138,800 → 139,800 (owner-approved)

## Design

### Core (`rust/crates/vela-core/src/app/payment_request.rs`)

- `Model.include_network: bool`. It resets on `Start`.
- `Event::IncludeNetworkChanged { include }`.
- View: `network_switch = mode == Address && recipient != ""`,
  `network_hint = network_switch && include_network`, and `include_network` itself.
- `qr_value`: request mode → unchanged (the request URI). Address mode with `network_hint` →
  `build_eip681(recipient, asset.chain_id, None, _, "")` = `ethereum:<recipient>@<chain>`. Otherwise the
  bare recipient.
- `copy_payload` stays unchanged: the bare address in address mode.
- A token's code names its chain only (no `/transfer`, no amount).

### How each shell tells the machine which network is on screen

| Shell | Session lifetime (`Start`) | Asset pick | Stale guard |
|---|---|---|---|
| Web + ext | a `payment_request` session in the wallet route while any `r*`/`dr*` state is up (new) | `$effect` sends `receiveAssetPicked(...)` (the same `receiveSubject` the builder draws from) when it differs | `liveReceiveQr` uses `qr_value` only when the core's asset is this screen's; otherwise the bare address and no switch |
| Desktop | `watch_money` forgets the resident on each receive visit (new) | `keep_receive_asset` sends `AssetPicked` from `receive_chain` / the DR3 token when it differs (new) | not needed: the machine is synced in the same frame |
| iOS | `enterReceive()` at every door into Receive (it was a start per R1 step) | `onReceiveNetwork` / `receiveSelectedToken` (existing) | `FlowsLive.receiveCode` |
| Android | `openReceive` keyed on being in the flow (existing) | `pendingReceiveAsset` → `assetPicked` (existing) | not needed: the sheet is drawn from the machine's asset |

### Drawing

- One switch per shell: web `Switch.svelte`, desktop `switch_row`, iOS `VelaSwitchRow`, Android
  `FlowSwitchRow`. No shell had one before.
- Monochrome: the ink track means on. The accent belongs to the money-moving action (web rule 4).
- On press, the thumb stretches from 16 to 20. iOS and Android play one `select` haptic.
- The hint sits under the switch in subtle ink, at the warning's size. It is not a warning colour.
- Order under the code: switch → hint (only when on) → the existing network warning → Save image →
  Explorer.

### Share image

- Web: `share.code = encodeShareQr(value)`.
- Desktop: unchanged; it already used `pay.qr_value`.
- iOS: `FlowsLive.shareCard(pay:)`. The card's chain is now the machine's asset. Before, a token's 收款
  left `receiveNetwork` at the last row.
- Android: `FlowLive.shareCard(code = request.qr_value)`.

## Constitution Check

The rule is decided once, in the core. Shells only draw it. All four shells have the surface; the extension
renders the web UI. Every layer has tests. Strings live only in the corpus.

## Project Structure

Feature docs: `specs/090-receive-network-qr/{spec,plan,tasks,results}.md`. No new modules; the switch
primitives live beside each shell's existing flow components.

## Complexity Tracking

- **Web stale guard**: a pure equality check between the core's asset and the subject on screen.
  - Why: the route's `$effect` reaches the core one frame after the screen changes.
  - Without it, a frame could pair a Gnosis code with a Base title. The bare address is always safe.
- **iOS stale guard**: same reason. The row tap and the core's view are two separate updates.
