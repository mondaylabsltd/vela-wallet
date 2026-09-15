# Contract — the settings inventory, on iOS

One row per setting. **"Shows at" is where the effect must be observed** — 049's
lesson, and the reason a format picker previewing itself proves nothing.

| # | Setting | Shows at | Before 057 |
|---|---|---|---|
| 1 | Account row → switcher | header name + address | live (056) |
| 2 | 语言 | every label | **half** — written and read at launch; a live change does not re-resolve `Loc` |
| 3 | 字号 | every text | live (056) |
| 4 | 主题 | every background | live |
| 5 | 头像样式 | every avatar | **was stored and read by nothing**; live in 057 |
| 6 | 货币 | hero, asset rows, transaction detail | live (056) |
| 7 | 数字格式 | every figure | live (056) |
| 8 | 日期格式 | feed days, transaction detail | live (056) |
| 9 | 时间格式 | transaction detail | live (056) |
| 10 | 网络列表 | the chain sheet, the picker | live (050) |
| 11 | RPC 提供商 | which endpoint a read uses | live (056) |
| 12 | 端点 | the same | live (056) |
| 13 | 存储 | the numbers on the page | **drawn** |
| 14 | 清除缓存 | balances re-fetch | live (056) |
| 15 | 关于 | version, commit | **drawn** |
| 16 | 反馈 | — | off the home (founder's ruling) |
| 17 | 退出登录 | the welcome screen | live |
| 18 | 抹除此设备 | everything gone | live, never run here |
| 19 | 通讯录 row | — | removed (founder's ruling) |
