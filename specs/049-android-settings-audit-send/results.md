# Results — 049 Android Settings Audit and the Send Path

**Branch**: `049-android-settings-audit-send` (stacked on 048) · **Device**: Xiaomi
alioth `9d5f42fb`, parallel space, fixture Safe `0x88cC…6894` on Gnosis ·
**Date**: 2026-09-13.

## What was wrong, and what landed

| Founder's item | Cause (code read) | Fix | Evidence |
| --- | --- | --- | --- |
| 首字母/图形头像 does nothing | `vela.avatarStyle` was written and ticked; no avatar read it (`IdenticonAvatar.kt` carried spec 015's "no initial-letter rendering") | `IdenticonImage` reads `LocalAvatarStyle` (provided at the root from the preferences) and a `name`; `initials` draws an accent disc with the name's first letter at the web's 0.34 ratio, `V` without a name; the viewer takes `(seed, name)`; 19 sites pass the name they show | Phone: 首字母 → settings row `P`, header `P`, contacts A/B/C/F, empty recipient `V`; relaunch keeps it; 图形头像 restores the artwork (`049-a1…a9`) |
| 数字格式 wrong | (a) the hero drew `"." + decimals` literally → `CN¥3` `.63` beside dot grouping; (b) token amounts printed `toPlainString()`; (c) `Formats.current` was a static — the flow models `remember`ed without it; (d) the sheets showed the mocks' strings; (e) clear signing sent a constant locale | `Formats.current` is Compose state; `BalanceModel.decimalMark`; `Formats.plain()` on every token amount, fee and receipt figure; both flow models key on `Formats.current`; live examples with wire-key ids; `ClearLocale.fromFormats` | Phone under `1.234.567,89`: hero `CN¥3,63`, feed `−0,001 XDAI`, detail `−0,001 XDAI`, form `余额 0,54197` / `0,01 xDAI` / `≈ CN¥0,00`, confirm `0,001 XDAI`; flip back → dots with no relaunch (`049-f*`) |

Also fixed on the way: the transaction detail's fiat line was a hard-coded `$`
(now the display currency, `≈ £0,78`-style, the web's `moneyText`); the 日期/时间
rows and sheets showed drawn strings (now live examples; 自动 says what this
phone resolves to).

## The inventory — after-state

Rows follow `contracts/settings-inventory.md`. "Phone" = driven over adb in the
parallel space and read from a dump, a log line or a screenshot.

| # | Setting | After 049 | Seen |
|---|---|---|---|
| 1 | Account row | accounts sheet from the session | Phone: 「1 个账户 · Parallel space · 创建新账户」; header artwork → viewer with the whole address |
| 2 | 语言 | sheet, `system` + tags | Phone: English → 「Settings / Switch account」; 简体中文; 跟随系统 → 「简体中文 · 系统」 |
| 3 | 字号 | slider | Phone: dot 5 → the page rescaled (`049-i3`) |
| 4 | 主题 | three segments | Phone: 浅色 (`049-i4-light`), 深色, 跟随系统 |
| 5 | 头像样式 | **fixed** | above |
| 6 | 货币 | catalogue → hero, fiat lines | Phone: EUR → 「总余额 · EUR」「€0.46」; CNY back |
| 7 | 数字格式 | **fixed** | above; sheet rows are each preset's 1234567.89 with the drawn notes kept (`049-f0`) |
| 8 | 日期格式 | live, re-keyed | Phone: `13.06.2026` picked → detail `12.09.2026`; back → `2026/09/12`. Day headers were all 昨天 (relative) on this feed |
| 9 | 时间格式 | live, re-keyed | Phone: `1:45 PM` → detail `2:44 PM`; `13:45` → `14:44` |
| 10 | 高级 | disclosure | Phone: rows 网络 / RPC 供应商 / 添加网络 / 服务节点 / 设备存储 / 关于 appear |
| 11 | 网络 | list, detail | Phone: 12 个网络, Gnosis detail: 链 100 · xDAI, 在线 · 45ms, RPC URL, explorer, 内置网络 · 不可移除 |
| 12 | RPC 供应商 | keys, tests, links | Phone: Alchemy 已连接 · 检查密钥 · 支持 12 个网络; dRPC/Ankr 未设置 · 获取密钥 |
| 13 | 服务节点 | four endpoints + 恢复默认 | Phone: chain data / passkey index / relay / fiat rates, all shown |
| 14 | 设备存储 | measure, clear | Phone: 16 KB · 33 条; 交易记录 23 条 · 13 KB; 联系人与分组 7 位 · 1 KB; per-item 清除 present (not pressed — 047 verified the sweep) |
| 15 | 关于 | build + facts | Phone: v1.0 (427e1e3d), Safe v1.4.1, WebAuthn / P-256, ERC-4337, 12 条 EVM 链 |
| 16 | 反馈 | absent (047 ruling) | — |
| 17 | 退出登录 | sheet | Phone: the sheet with its two paragraphs; 取消 (not confirmed: it would sign the test bed out; the path is 047/048's) |
| 18 | 清理数据 | sheet | Phone: the sheet with the loss list; 取消 (not confirmed: destructive; 047 verified the keep-list erase) |
| 19 | Rescue sheets | from the hero status line | not reached this pass (no failed chain at the time); 048 row 12 |

## The send (US4)

| | |
|---|---|
| Path | 转账 → XDAI → recipient `0x7687…D141` → 0.001 → 继续 → confirm (`0.001 XDAI`, `≈ CN¥0.00`, 预估手续费 ~0.01 xDAI) → 确认并发送 → 「交易已提交至网络」 → 「已发送 0.001 XDAI 至 0x7687…D141 · Gnosis」 |
| Hash | `0xa513287b…c4643e` |
| Core | `PersistTxRecords → RecordsPersisted`, `TrackSubmitted → TrackHandedOff`, receipt `SD4C` |
| Safe (RPC) | 0.54197 → 0.53097 xDAI (−0.011 = 0.001 + the 0.01 in-band fee) |
| Recipient (RPC) | 0.015 → 0.016 xDAI |
| Feed | a new `−0.001 XDAI` row at the top of the home (`049-s5`) |

## Gates

- Android unit suite: **523 run, 0 failed** (`:app:testDebugUnitTest`). One
  flake seen once under full-suite load (`DappSignMachineTest` 20 s timeout);
  passes alone in 0.19 s and in the final run.
- Rulers: event parity strong diffs **5 → 5**; dropped judgement **30 → 30**.
- Build: `assembleDebug -PvelaSkipRustBuild` (the core is untouched; the `.so`
  is 048's).

## Deviations

- `WalletLive.Money` is public (was `internal`): `FlowLive.txDetail` takes it.
- The web's clear-signing locale is the same constant Android had; recorded as
  `WEB-CLEAR-LOCALE-1` for the web owner, not changed here.
- The gallery keeps `NUMBER_SAMPLES` (drawn boards); only the live route computes.
- Clear-signing amounts under a preset were not driven on the phone (needs a
  dApp flow); `ClearLocaleTest` covers the mapping.
- Sign-out and erase were taken to their sheets, not confirmed (the test bed).

## Driver gotchas (adb, this pass)

- A Compose button's text node is not `clickable="true"` in the uiautomator
  dump; tap by text, not by the flag.
- `python3 -u` — buffered stdout hid every line of a 10-minute pass.
- Three `back()`s from a detail sheet leave the app; `tap_text` retries then
  cost ~25 s each miss.
