# Contract — 057's eleven, and who else has them

057 left eleven controls drawn and unreachable on iOS. The founder's rule for
this cut: **learn the interaction from Android or from the phone web app; only
draw a new design when neither has one.** So each row names its precedent
first. `results.md` carries the after-state.

| Row | Control | Precedent to copy | Plan |
|---|---|---|---|
| 5 | contact 转账 / 收款 / 二维码 | Android: `VelaNavHost.kt:1335` routes `contacts.action.*` into the send flow, the receive flow and the QR sheet | port |
| 7 | 新建分组 / 编辑分组 | Android: `feature/contacts/components/GroupSheets.kt`; the copy exists (`contacts.groupNew`, `contacts.groupRename`, `contacts.moveGroup`) | port |
| 10 | identicon → viewer, from every avatar | Android: the avatar component itself opens it through `LocalIdenticonViewer` (`IdenticonImage.kt:70`), so eleven call sites need no edit | port the mechanism, not the call sites |
| 12 | hero status line → rescue | Android `VelaNavHost.kt:1058` and web `+page.svelte` agree: a failed chain opens its RPC fix, anything else the balance breakdown | port |
| 26 | asset-limited receive (r3) | Android since 048 — see `four-areas.md` R1 | port |
| 28 | activity 删除 | **web only**, and not by swiping: the web deletes from the open transaction detail (`deleteSelectedTx`). Android has `deleteActivity` with no caller — dead on both phones | port the web's placement to both phones |
| 33 | 存储 | Android `DeviceStorage.kt` | port |
| 34 | 关于 | Android `SettingsLive.kt:457` | port |
| 37 | native transaction detail (a3) | none needed | **record**: the live detail already renders both directions. What a3 draws and live omits is the 代币合约 row, and no client can fill it — `LocalTransaction` stores a symbol, not a contract |
| 38 | native-coin token tab (t3b) | none anywhere | **record and recommend deleting**: dead on Android too (both live builders hard-code the ERC-20 tab). What it describes is adding a NETWORK, which `network_admin`'s wizard does properly. Deleting drawn UI is the founder's call |
| 39 | haptics | Android `core/platform/VelaHaptic.kt` + `Haptics.kt` — a named vocabulary, not scattered calls | port the vocabulary |
| 40 | crash report | Android `core/diagnostics/CrashReport.kt` + `CrashSheet.kt`, armed in debug by `--ez vela.testPanic true` | port |

**No row needs a new design.** That is the answer to the founder's "如果 web
android 都没有设计稿的话,那你先出设计稿": two rows (37, 38) want a decision
rather than a drawing, and the other nine are ports.
