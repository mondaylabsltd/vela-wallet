# Contract — every control on this client, and what it does

Android 048's forty rows, re-read against iOS after 050–056. "Before 057" is
the code read of 2026-09-15; results.md carries the after-state and how each was
seen.

| Row | Control | State before 057 |
|---|---|---|
| 1 | copy — receive rows, receive address, tx hash, token facts, receipt hash, contact address | **live** (`AddressStrip`, `AddressBlock`, `IdenticonViewerSheet`, the explore bar) |
| 2–3 | activity row / asset row tap → detail | **live** (`onSelectActivity`, `onSelectAsset`) |
| 4 | 保存图片 | **live** since 051 (`ShareCardExport`, album add-only) |
| 5 | contact 转账 / 收款 / 二维码 | **dead** — the three action cards on a contact page do nothing |
| 6 | 群发转账 | **live** since 054 (seeds the split) |
| 7 | 新建分组 · group rename / delete | **half** — delete is live, create and rename have no drawn form (`contacts::GroupSave`, recorded in 056) |
| 8 | SD1 class chips | **dead** — drawn, no filter behind them |
| 9 | 全部网络 pill | **live** (`onPickChain` → the feed's chain filter) |
| 10 | identicon → viewer | **half** — the viewer exists and opens from the wallet header only |
| 11 | hero amount → hide | **live** (`togglePrivacy`) |
| 12 | hero status line → rescue | **dead** — SR2/SR3/SR4 are drawn and unreachable |
| 13 | 字号 | **live** since 056 (tap a stop, or VoiceOver adjust) |
| 14 | scanner 翻转 | **live** since 055 (disabled when there is one lens) |
| 15–17 | 在区块浏览器中查看 | **live** (`explorerLink`) |
| 18 | token detail 转账 | **dead** — opens the send without preselecting the token |
| 19 | group ⋯ | **live** since 054 (import / export / delete; 编辑分组 has no form) |
| 20 | 添加联系人 + | **live** since 054 |
| 21 | contact row swipe 删除 | **live** since 050 |
| 22 | 扫码填写地址 (picker row) | **live** since 055 |
| 23 | 通讯录 row in settings | **removed** — the founder's ruling (056) |
| 24 | copy on the receive card | **live** |
| 25 | account row → switcher | **live** since 056 |
| 26 | asset-limited receive (r3) | **dead** — drawn, unreachable |
| 27 | 添加代币 | **live** since 051 |
| 28 | activity row swipe 删除 | **dead** — no swipe on a feed row |
| 29 | 反馈 | **removed from the home** — the founder's ruling; the sheet is drawn |
| 30 | 语言 · 货币 · three formats | **live** since 056 |
| 31 | 主题 · 头像 | **live** since 056 (头像 was stored and read by nothing until 057) |
| 32 | RPC providers · endpoints | **live** since 056 |
| 33 | 存储 | **drawn numbers** — not real bytes |
| 34 | 关于 | **drawn** — not from the build |
| 35 | 退出登录 | **live** |
| 36 | 抹除此设备 | **live** since 056, never run on the founder's phone |
| 37 | native transaction detail (a3) | **dead** — drawn, unreachable |
| 38 | native-coin token tab (t3b) | **dead** — drawn, unreachable |
| 39 | haptics | **absent** — no policy written |
| 40 | crash report | **absent** |
