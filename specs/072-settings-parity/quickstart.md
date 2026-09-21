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

## Merge verification (2026-09-22, all shells on `072-settings-parity`)

| Gate | Result |
|---|---|
| `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 1692 passed |
| `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings` | clean |
| desktop `cargo test` / `cargo fmt --check` | 492 passed / clean |
| iOS `xcodebuild test -only-testing:VelaWalletTests` (iPhone 17 Pro sim) | 718 passed |
| Android `:app:testDebugUnitTest` | 640 passed |
| web `vitest` | 1526 passed (incl. the extension package after `build:extension`) |
| web e2e (chromium): add-network, settings-desktop, settings-persistence | 15 passed |
| web e2e (chromium): accounts:174, home-truth:107/124, welcome-layout:187 | fail — the same four fail on `main` (dd482131); not this branch |
| `build-web --check`, `gen-core-types --check`, i18n vectors | current |
