# Quickstart — 072 device checks

## Android (Xiaomi, 2026-09-22, the 072 debug build over a store an older build wrote)

| # | Check | Result |
|---|---|---|
| D1 | Launch over a store holding `theme_preference=light` (DataStore), `vela.language=system`, `textScale` inside `vela.localePrefs` | migrated once: `vela.theme=light`, the DataStore key gone, `vela.language=auto`, `vela.textScale=small`, `vela.localePrefs` without `textScale`, the number format kept; the app still light, Chinese, small text |
| D2 | Settings → account sheet | every account's total ("6 个账户 · 总计 $1 608,16") — was blank |
| D3 | Currency sheet, search "yen" | only JPY |
| D4 | Device storage | rows from the core catalog, sizes in 1024s ("30 条 · 21 KB") |
| — | Erase | NOT run on this phone (it also holds a real wallet); covered by JVM tests (sweep, keep-list, verify) |
| — | Provider keys, network removal | no saved keys / custom networks on the phone; covered by the core regression test and JVM |
