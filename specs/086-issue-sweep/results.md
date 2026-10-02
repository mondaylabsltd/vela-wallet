# 086 results — issue sweep

Status 2026-10-01 night: **all 16 issues have PRs** (#341–#350, #371–#374). Device checks used an integration build of `main` (67e2d193d) plus every fix branch: Android on the Xiaomi alioth in the parallel space; iOS on the iPhone 11 (iOS 26.5.2) and simulators.

| Issue | PR | Root cause (on `main`) | Fix | Device evidence |
|---|---|---|---|---|
| #318 iOS trusted signer | #341 | v0.9.4 had no iOS return channel; fixed after it by `741b7426` / `209b92f7` / `229b4a5b`; iOS was never device-proven | Opt-in round-trip device test plus evidence | Simulator (iOS 18): create → page → passkey → member proof → both answers back, `1 / 7`. iPhone 11: the page opens, the slide works, the system passkey sheet appears |
| #315 ext follows the signed-in account | #342 | The grant was answered for any *held* account; the follow skipped the first account; the published session went stale | Core `granted_to_signed_in`; the worker and root layout publish and announce | e2e 3/3 (fail on `main`); Chrome for Testing |
| #317 ext sheet language | #343 | The worker can't read the saved language; fresh pages had empty `page.params` | One rule in `extension/lib/locales.js`; `open.html` entry | e2e 4/4 |
| #328 asset sheet reopen | #344 | A closed sheet never left the flow stack; `push` ignored the same step | `FlowNavState.sheetClosed`; the web `closeSheet` | Xiaomi: 6/6 rounds, by ×, Back, swipe and scrim |
| #329 favourite named after the error page | #345 | The star took the engine's current title | Core `pinned_title`; Android, iOS and desktop | Xiaomi: a failed host is starred as `vela-test-329.invalid` |
| #330 Manage groups unreachable | #346 | A hidden Favorites dropped its heading and Edit | The heading stays (desktop rule) | Xiaomi: hidden, then restart, then restore |
| #332 scan shows recipient first | #347 | No shell drew the scanned recipient on the picker; Back dropped it | The core marks an outside recipient; a To line on all shells | Xiaomi: Scan → photo picker → "收款人 0xD400…130b" |
| #312 + #326 network-scoped scan, token card | #348 (stacked on #347) | Quick-send chose the top token on any chain; native-coin placeholder; the card had no handler | Core `request_chain_id`, `change_token` | Xiaomi: @100 → form on xDAI; @8453 → Base only + note; @137 → none + note; card → picker, recipient kept |
| #334 + #310 contact Edit / Delete placement | #349 | Footer pinned by `margin-top:auto` / a `flex_1` spacer | Pencil by the name; Delete after the content (web + desktop) | Web screenshots at 1440 and 390 px; desktop gallery |
| #333 import garbles names | #350 | Lenient per-shell decoding | Core `decode_text`, strict UTF-8/UTF-16; refuse anything else | Xiaomi: GBK refused with a message; UTF-16 → "jxjjx测试", "Иван Петров" exact |
| #331 recipient × too close to amount | #371 | Android: a 19 dp field with the ×'s 48 dp touch target reaching 6.9 dp into it; iOS: the same (a 12 pt gap, nearest-button wins) | A full-height amount well; × kept clear (Android + iOS) | Xiaomi: taps on the well's edges never remove a row |
| #322 Sign Out looks like text | #372 | Centred grey text with no row (Android, iOS, phone web) | A settings row with icon + chevron | Xiaomi + iPhone: a row like 关于 |
| #321 Receive too tall | #373 | A fixed 344 dp QR card squeezed sideways kept its height; a ×-only row above the title | A square card capped at the width; × on the title line (Android + iOS) | Xiaomi 15-geometry emulator test; alioth fits |
| #314 backup-keys sheet dense | #374 | The intent drawn as an eyebrow; a heavy no-change simulation card | The intent as headline (4 apps); no-change simulation folded (phones) | Xiaomi: headline, three rows, folded details |

## Decisions taken (to confirm with the owner)

- **#333:** GBK and other legacy encodings are refused, not decoded. A GBK decode silently mis-reads Shift_JIS, EUC-KR, Big5 and cp1252, which covers four of our locales. **Owner ruling 2026-10-02: refusing non-UTF-8 (e.g. GBK) files is kept.**
- **#312:** Vela's own Receive QR stays a bare address (#208 was closed as "not supported"). So a Vela network QR shows recipient-first with all assets. Making it `ethereum:addr@chain` is a one-line follow-up. **Owner ruling 2026-10-02: ERC-681 opt-in, default bare address → spec 090** (`specs/090-receive-network-qr`).
- **i18n budget:** ja+en is at 138,798 / 138,800 after #350.

## Merge order and conflicts

- **Generated-file conflicts:** each core-touching PR commits its own wasm (`assets/wasm/vela_core_bg.<hash>.wasm`) and `rust/pkg-web`. These conflict between PRs; resolve by rebuilding (`node rust/scripts/build-web.mjs`, then `sync-wasm`) after each merge.
- **#345 and #346** add a test at the same place in `ExploreMachineTest.kt` and desktop `page.rs`; resolve by keeping both.
- **#348** is stacked on **#347**: merge #347 first.
