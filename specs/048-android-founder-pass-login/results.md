# Results — 048 Android Founder Pass and Login Recovery

**Branch**: `048-android-founder-pass-login` (stacked on 047) · **Device**: Xiaomi
alioth `9d5f42fb`, parallel space, Gnosis, fixture Safe `0x88cC…6894` (dust
only) · **Dates**: 2026-09-12 → 2026-09-13.

## What landed, phase by phase

| Phase | Delivered | Evidence |
| --- | --- | --- |
| 3 login recovery (`c1e3e3d7`) | Core: `Account`, `AccountKey`, `PendingUpload`, `PendingUploadMember` read the retired client's spelling (`publicKeyHex`, `createdAt`, `credentialId`) through hand-written `Deserialize` readers (`either()`), snake_case written only. Web: `loadAccounts` normalises old records and rewrites the list once; `keys` tolerated absent; the effect loop turns a refused shell answer into the machine's own failure once per effect (`toFailure`), `onError` optional with a console reporter; the login page and the session store show the prompt (sign in again / reset this browser's copy). Android: `CoreDriver.resolve` answers the machine with the escaped failure; `SessionController.onFault` logs and renders. | core `tests/app_login.rs` (`an_expo_era_record_still_opens_the_wallet`, with and without keys), `tests/app_session.rs`; web `storage.test.ts`, `effect-loop.test.ts`, `e2e/login-old-shape.e2e.ts`; Android `SessionOldShapeTest`; wasm rebuilt and `--check` current. Phone: an old-shaped list seeded through `run-as` → the session left loading and the account showed; `"{"` seeded → `session.fault` logged, the visible fault state, never `loading` forever |
| 4 every button (`2853e52f`) | `Clipboard.copy` (one helper, tick only on success), list rows carry their id (`FlowNav.push` refuses a detail step without one), the share image redrawn after `share-image.ts` with a real code and `Gallery.savePng` into `Pictures/Vela`, the contacts dock, 群发转账 through `seedSplit`, group new/rename/delete, the identicon viewer hosted once at the NavHost and opened from twelve artworks | the device pass below (rows 1–7, 10, 24) |
| 5 filters, links, second tier (this branch tip) | class chips (`sendTokenClass` ported), 全部网络 pill → `ChainFilterSheet` → assets/pick/feed narrow, explorer links, token detail 转账/收款/rows, hero tap-to-hide + status-line rescue, scanner flip, picker 扫码/group rows + per-row pick, add-token 原生币 tab, settings fields and links, contacts 全部/swipe/export choice | rows 8–9, 11–12, 14–23, 26–30, 34–39 |
| 6 slider + haptics | `VelaTextScaleSlider` drags and taps (thumb on local state, commit on release, one `Detent` per step crossed); `VelaHaptic` (Detent/Select/Success/Reject, `performHapticFeedback`, one `haptic <class>` log line) with the policy in the design-system doc; `Select` on copy, class/chain chips, network/account picks, favourite, visibility, segmented controls, the scanner flip; `Detent` on the signing slider threshold | rows 13, 33 and SC-005 below |
| 7 looks | scanner brackets at the web's `--border-emphasis`, one status line; settings home without 通讯录/反馈 rows on the live route (047's ruling, photographed again); the feedback box editable | rows 31, 40 |
| 8 closeout | rulers re-run, this file, `docs/KNOWN-BUGS.md` (WEB-LOGIN-1, DESKTOP-1, ANDROID-8), memory | below |

## The 40 audit rows — after-state

Numbers follow `contracts/android-affordances.md`. "Phone" = driven over adb
in the parallel space and read back from a dump, a log line or a screenshot.

| Row | Control | After 048 | Seen |
|---|---|---|---|
| 1 | copy (R1/R2/A2/T2/SD4, contact 复制地址) | `Clipboard.copy`, tick on success, `haptic select` | Phone: 收款 row copy → 转账 recipient field → `input keyevent 279` pasted the same address; `clipboard copied` logged |
| 2–3 | A1 / T1 row tap | id carried; sheet opens; no phantom step | Phone: 活动 全部 → row → tx detail sheet; Back ×1 → list, ×1 → home |
| 4 | 保存图片 | real code, web composition (foot, icon, wordmark, chain pill), `Pictures/Vela` + snackbar; share second | Phone: PNG pulled from `/sdcard/Pictures/Vela/`, `zbarimg` decoded the wallet address |
| 5 | contact 转账 / 收款 / 二维码 | send.open prefilled / receive / contact code sheet | Phone: all three from a contact's detail |
| 6 | 群发转账 | send.open + `seedSplit(members)`; one member → the plain form | Phone: group → N split rows |
| 7 | 新建分组; group ⋯ rename / delete | `GroupEditSheet` → `saveGroup`; menu → rename / delete with confirm | Phone: created, renamed, deleted |
| 8 | SD1 class chips | `sendClassFilter` → `visibleTokens`, `haptic select` | Phone: 稳定币 → 0 rows, Gas 币 → XDAI, 其他 → 0, 全部 → XDAI (`send.pick class=… visible=n of 1`) |
| 9 | 全部网络 pill (SD1/A1/T1) | chain sheet → `chainFilter` → lists narrow, feed `chain_filter_changed`, pill named | Phone: Ethereum → history 「此网络暂无交易」+ pill "Ethereum", pick rows empty; all-row → 全部网络, XDAI back |
| 10 | identicon (12 sites) | NavHost-hosted viewer | Phone: the viewer from each listed site |
| 11 | hero amount | `togglePrivacy` + `haptic select` | Phone: 「••••」+ 显示余额, tap again → CN¥3.63. Found on the way: the fixture's "$1,383 · USD" leaked into the hidden state — now the live currency with •••• |
| 12 | hero status line | rescue sheet | Phone: 离线 line → the settings' 修复 RPC sheet (Polygon failed) |
| 13 | 字号 | slider: drag + tap, snapping, one Detent per step crossed, commit on release | Phone: drag → 5 detents (6 from the far end), drag back → 5, dot tap → 1, half drag → 4; the thumb follows the finger; the page rescales when the finger lifts |
| 14 | scanner 翻转 | lens flip when a front camera exists | Phone: `haptic select`, the preview swapped |
| 15–17 | 在区块浏览器中查看 (R2/A2/T2) | `explorerUrl` from the chain's `explorer_url`, `openUrl` | Phone: R2 → Chrome resumed on `etherscan.io/address/0x88cC…`; A2/T2 fill through the same builder, not photographed separately |
| 18 | T2 转账 | `send.open(preselected_symbol, preselected_network)` → form | Phone: 发送 XDAI with the token chosen (`send.selectToken` logged) |
| 19 | group ⋯ | menu sheet | row 7 |
| 20 | network detail overrides | `VelaUrlField` editable → `override_field_edited/blurred` | Phone: Gnosis detail shows the live RPC/explorer; override typed → `editOverride chain=100` logged |
| 21 | "+ 添加网络" foot | add-network page | Phone: foot → the add page |
| 22 | picker 扫码 row | scanner → recipient | wired (`FlowHost.kt`); the scanner itself photographed (row 31); the row not driven separately |
| 23 | picker group rows | `seedSplit(members)` | Phone: group Team → 3 split rows (`group seed` logged) |
| 24 | contact 复制地址 | row 1 | Phone: `clipboard copied` from the contact detail |
| 25 | home rows | recorded (P3, the sheet animation) — no change | — |
| 26 | T2 收款 | the token's own code | Phone: 「使用这个地址接收 Gnosis 上的资产」+ XDAI; the network row's QR → the Gnosis sheet |
| 27 | T2 activity rows | tx detail by id | wired through the same `onSelect(id)`; not photographed |
| 28 | A2 删除记录 | feed delete with confirm | wired; not driven on the phone (the fixture's rows are real transfers) |
| 29 | T3 原生币 tab + chain pick | settings add-network page | Phone: `settings page requested` logged, the page seen |
| 30 | split card 通讯录 pick | `openRowPicker(id)` | Phone: per-row icons, the row picker opens |
| 31 | scanner look | brackets at the web weight, one status line | Phone: `scan-048.png` — the debug badge overlaps the close disc in the parallel space only |
| 32 | share card look | row 4 | — |
| 33 | 字号 haptic | row 13 | — |
| 34 | contact 最近往来·全部 | history filtered to the contact | Phone: 全部 → history |
| 35 | contacts swipe 转账/删除 | send.open / delete confirm | wired; the swipe reveal not driven by the script |
| 36 | export | CSV / JSON sheet | Phone: the choice sheet |
| 37 | add-network custom RPC + 重新检查 | editable field; `addNetworkByChainId` recheck | wired; the failed-check state that shows them was not reached on the phone (chain 99999 → the UB Smart Chain candidate, the checks did not render in 15 s) |
| 38 | provider 检查密钥/获取密钥, drpc link | `provider_test_requested`; `openUrl` | Phone: 检查密钥 → `provider test Alchemy`; 获取密钥 → the link chooser |
| 39 | language sheet contribute link | `openUrl` | not applicable: the live language sheet carries no footer link |
| 40 | settings home rows; feedback box | rows absent on the live route; editable box | Phone: the settings screenshot of the slider pass shows no 通讯录/反馈 row; the box is editable in code, the sheet unreachable since 047's ruling removed the row |

The founder's nine items map to rows 13 (字号), 10 (identicon), 2–3 (list
rows), 14 + 31 (scanner), 9 (全部网络), 8 (chips), 4 (保存图片), 7 (管理 →
新建分组), 6 (群发转账). The cross-shell login bug is phase 3.

## The login fix, layer by layer

- **Cause**: `wallet.getvela.app` moved to the SvelteKit shell on 2026-09-11;
  the retired client's `vela.accounts` records are camelCase; the core's
  `Account` refused them; the web's login and session effect loops swallowed
  the refusal, so the passkey signed and nothing moved.
- **Core**: hand-written readers instead of `#[serde(alias)]` — ts-rs reads
  the attribute as one it does not know and warns, and CI's `cargo clippy
  -D warnings` would refuse the build. Unit tests assert both spellings read
  equal.
- **Web**: normaliser + one-time rewrite (`storage.ts`), `keys ?? []`
  (`services/accounts.ts`), the refusal path (`effect-loop.ts`), the prompt
  on the login page and in the session store, corpus keys
  `onboarding.storage.*` in all 15 locales (pins 1633 paths). vitest and the
  Playwright seed test passed in an isolated `--output`.
- **Android**: the escaped failure answered to the machine, `onFault`,
  `SessionOldShapeTest` with a `FakeStore`. `AccountStore.readList` now throws
  on an unreadable stored list instead of answering an empty array (see
  ANDROID-8 below).
- **Desktop**: not exposed — its store skips unreadable records silently
  (`DESKTOP-1`, recorded, not changed).

## Haptics — the policy and SC-005

Policy (design-system doc): Press, Detent, Select, Success, Reject; one per
gesture; never on navigation, scroll, tabs, Back, sheets, typing or
programmatic changes. Every call logs `haptic <class>`.

| Gesture on the phone | `haptic` lines |
|---|---|
| slider drag across six dots | 5 `detent` (6 when starting from the far end: the first move snaps to dot 0) |
| slider drag back | 5 `detent` |
| dot tap | 1 `detent` |
| class chip, chain pick, copy, visibility toggle, segmented control, scanner flip | 1 `select` each |
| scroll, tab switch, Back, sheet open/close | 0 |

## The rulers

- Web as the checklist: strong diffs **6 → 5** across 24 machines.
  `PaymentRequestEvent.asset_picked` closed (row 26). Remaining: `FeeEvent`
  `select_fee_asset / requote / leave_confirm` (the fixture Safe has one fee
  asset), `BalanceEvent.fix_chain_resolved`, `DpermEvent.popup_request`
  (the extension's popup).
- Dropped judgement: 30 fields across 48 views, unchanged.

## Deviations from the plan

- `#[serde(alias)]` → hand-written `Deserialize` impls (ts-rs warning, CI).
- The slider commits the scale when the finger lifts, not per step: a per-step
  commit re-laid the whole settings page out under the drag and the gesture
  died after one detent (the founder: 滑动很不跟手). The thumb and the detents
  are live; the page rescales at release.
- The web's `onError` became optional with a default reporter rather than
  required (15 callers; a required parameter was the plan's wording).
- The parallel-space fixture add now refuses to run when the stored list
  cannot be read or the session shows fewer records than the store holds
  (ANDROID-8), and logs `parallel enter refused`.
- Rows 22, 27, 28, 35 are wired but were not driven separately on the phone;
  row 37's failed-check state was not reached; row 39 has nothing to wire.

## Gates

- Android unit suite: **510 run, 0 failed** (`:app:testDebugUnitTest`).
- Core: `cargo test -p vela-core --features i18n-all,crux` green at phase 3
  (the login/session old-shape tests included).
- Web: vitest (`storage`, `effect-loop`) and the Playwright seed test green
  at phase 3; `node rust/scripts/build-web.mjs --check`: current (wasm
  3,737,056 bytes, source `b66f3b78…`).
- Rulers: no new strong diffs (6 → 5).
- Build: full `assembleDebug` (NDK included) for the phone; the two rulers
  and the corpus gates (`gen:i18n`, `lint:i18n`, `verify:i18n`) run.

## Known bugs written

- `WEB-LOGIN-1` (fixed): the sign-in that signed and then nothing.
- `DESKTOP-1` (open): the desktop skips an unreadable account record silently.
- `ANDROID-8` (open): the test phone's own account record (0x7687…D141) was
  dropped from `vela.accounts` at 22:42:21 UTC on 2026-09-12 while the
  scripted pass launched the dozing phone; only the fixture remains. The
  founder's passkey signs in again and restores it. Hardened: an unreadable
  list is refused (never "empty"), every upsert logs before/after counts, the
  fixture add refuses when it cannot see the store's records. Re-entered on
  the phone after the guard: `parallel leave` → `parallel enter keys=3` →
  `accounts upsert before=0 after=1` — the phone's store now holds the
  fixture alone, which is the loss itself, seen from the log.

## For the web owner

The web changes on this branch need a review by whoever owns `app-web`:
`src/lib/core/effect-loop.ts`, `onboarding/core/storage.ts`,
`services/accounts.ts`, `onboarding/core/sessions.ts`,
`session/core/session.svelte.ts`, `routes/[locale]/+page.svelte`, the two
unit tests, `e2e/login-old-shape.e2e.ts`, and the corpus keys
`onboarding.storage.*` in the 15 locale files.
