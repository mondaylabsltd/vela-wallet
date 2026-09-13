# Contract — the settings inventory (049)

One row per setting on the Android settings surface. "Shows at" is where the
effect must be observed; "Read back" is how the phone pass proves it. The
"Before 049" column is the code read of 2026-09-13; the results file carries
the after-state.

| # | Setting | Control | Shows at | Read back | Before 049 |
|---|---|---|---|---|---|
| 1 | Account row | tap → accounts sheet; row → switch; 新建 / 登录 | wallet header name+address; settings row | header text after switch | wired (047) |
| 2 | 语言 | sheet, `system` + 15 tags | every label; settings row value | settings title text after pick | wired (047, SC-001) |
| 3 | 字号 | slider 6 steps | every text; the sample | page height / text bounds | wired (048) |
| 4 | 主题 | 浅色 / 深色 / 跟随系统 | background colour of every screen | screenshot pixel | wired (010+) |
| 5 | 头像样式 | 首字母 / 图形头像 | every account/contact avatar; viewer | screenshots of 4 sites | **BROKEN — stored, never read** |
| 6 | 货币 | sheet, catalogue | hero label+glyph, asset fiat, send fiat lines, tx detail fiat | hero text | wired (041); tx detail hard-codes `$` |
| 7 | 数字格式 | sheet, 5 presets | hero mark+grouping, asset fiat, token amounts everywhere, fees, clear signing; row value; sheet examples | hero + feed row text | **BROKEN — hero mark `.`, token amounts unstyled, remembered models never re-key, sheet = drawn strings, clear signing constant** |
| 8 | 日期格式 | sheet, 6 presets | feed day headers, tx detail date, contact activity | day header text | applied but stale until re-keyed; sheet = drawn strings |
| 9 | 时间格式 | sheet, 3 presets | tx detail time | detail fact text | same as 8 |
| 10 | 高级 | disclosure | the advanced rows appear; remembered while the screen lives | rows present after toggle | `rememberSaveable` |
| 11 | 网络 | list; detail (RPC/explorer overrides); delete custom; + 添加网络 | list count on the row; detail values; add wizard | texts | wired (040–048) |
| 12 | RPC 提供商 | key fields, 检查密钥, 获取密钥 | field value persists; test log line | logcat `provider test` | wired (048) |
| 13 | 服务端点 | fields, 恢复默认 | value persists; logo/chain-index base follows | field text after relaunch | wired (041/047) |
| 14 | 设备存储 | measure; per-item 清除; 清除全部缓存 | totals and per-item counts drop | page text before/after | wired (047) |
| 15 | 关于 | version, commit, network count, links | page | texts | wired (047) |
| 16 | 反馈 | (row absent on the live route, 047 ruling) | — | — | by design |
| 17 | 退出登录 | sheet → confirm | welcome screen | route | wired |
| 18 | 清除设备数据 | sheet → confirm | store emptied except the keep-list; signed out | store keys via `run-as` | wired (047) |
| 19 | Rescue sheets | RPC 修复 / 余额明细 / 中继 | from the hero status line | sheet title | wired (048) |

## What "applies" means for a preset

- **Number**: grouping + decimal mark on money (hero, fiat lines); decimal
  mark only on token amounts (the web's `trimBalance` rule); the settings
  row shows `Formats.current` of 1234567.89 at two places; each sheet row is
  its preset's rendering; `auto` resolves from the device locale.
- **Date**: `Formats.current.date` on day headers and detail facts.
- **Time**: `Formats.current.time` on detail facts.
- **Re-render**: `Formats.current` is Compose state; the remembered flow
  models key on it.
- **Clear signing**: `ClearLocale(number, date, time, tz)` from the resolved
  presets at kickoff.
