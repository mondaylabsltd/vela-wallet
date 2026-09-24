# Quickstart — 073 checks

## Suites
```sh
cargo test -p vela-core --lib amount_text                    # 15
cd app-web/vela-wallet && npx vitest run src/lib/flows src/lib/signing
cd app-android/vela-wallet && ./gradlew :app:testDebugUnitTest --tests '*AmountTextTest'
xcodebuild test … -only-testing:VelaWalletTests/AmountTextTests -only-testing:VelaWalletTests/SplitRowsTests
xcodebuild test … -only-testing:VelaWalletUITests/AmountTextDeviceTests   # simulator only
cd app-desktop/vela-wallet && cargo test a_share_and_an_amount
```

## Android phone (Xiaomi, 2026-09-22; the parallel space; number preset decimal-comma — balance reads "$9,72")

| # | Do | Result |
|---|---|---|
| A1 | 转账 → XDAI → type `0,5` (adb, one key event per character) | the field reads **0.5 XDAI**; the core's line: "发送 0,5 加网络费 0,01，共需 0,51 XDAI；当前余额为 0,46767" — it read 0.5. Before 073 the same input did not resolve (071 device pass). |
| A2 | 设置 → 服务节点 → 恢复默认 (072 FR-010) | the sheet asks "恢复默认服务节点？ / 四项服务都将改回 Vela 的地址…"; 取消 left all four addresses as they were |

## iOS simulator (iPhone 17 Pro, region de_DE — the decimal pad's only mark is ",")

| # | Preset | Typed | Field |
|---|---|---|---|
| I1 | automatic (dot-comma) | `0,5` | 0.5 — "总额超过你的余额" against 0.46767, so the core read 0.5 |
| I2 | stored comma-dot (pad and preset disagree) | `0` `,` `5` | 0.5 |

## Gates (2026-09-22, `073-amount-text`)

| Gate | Result |
|---|---|
| `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 1709 passed |
| clippy `-D warnings`, `cargo fmt --check` (rust, desktop) | clean |
| web `vitest` (all, extension built) | 1524 + the cap's refusal test |
| web e2e chromium: batch, send-fee-over-balance, send-fee-fiat, fee-coin-switch | pass |
| Android JVM | 644 |
| iOS unit / UI (AmountTextDeviceTests) | 723 / 2 |
| desktop `cargo test` | 493 |
| `build-web --check`, `gen-core-types --check` | current |
