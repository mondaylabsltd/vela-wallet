# Quickstart — verifying 049

## Unit

```
cd app-android/vela-wallet && JAVA_HOME="/Applications/Android Studio.app/Contents/jbr/Contents/Home" ./gradlew :app:testDebugUnitTest -PvelaSkipRustBuild
```

## Phone (parallel space, Xiaomi `9d5f42fb`)

Build + install (the core is untouched, so the skip-flag `.so` is current):
```
cd app-android/vela-wallet && JAVA_HOME=… ./gradlew :app:assembleDebug -PvelaSkipRustBuild
adb -s 9d5f42fb install -r app/build/outputs/apk/debug/app-debug.apk
adb -s 9d5f42fb shell am start -W -n app.getvela.wallet/.MainActivity --ez vela.parallelSpace true
```
Driver: `ui.py` (`tap_text`, `texts`, `shot`); tabs 钱包 134,2150 · 通讯录 405,2150 · 探索 675,2150 · 设置 945,2150.

| Item | Steps | Expect |
|---|---|---|
| avatar | 设置 → 首字母 → shots of the account row, 钱包 header, a contact row, 转账 recipient; → 图形头像 → the same; relaunch | letter discs / identicons; choice holds |
| number | 设置 → 数字格式 → `1.234.567,89` → 钱包 hero, asset row, feed row; 转账 form + confirm; a tx detail | comma mark everywhere; flip to `1,234,567.89` → dots, without relaunch |
| time | 时间格式 → `1:45 PM` → tx detail | AM/PM |
| sheet | 数字格式 sheet rows | each row = its preset's 1234567.89; 自动 = the locale's |
| inventory | contracts/settings-inventory.md rows 1–19 | as listed |
| send | 转账 → XDAI → 觉得九点半 → 0.001 → 继续 → confirm → receipt | 已确认; RPC balance −(0.001+fee) |

Balance read:
```
curl -s -X POST https://rpc.gnosischain.com -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"eth_getBalance","params":["0x88cCA0EeDbF2C4426110bbFc998F048689266894","latest"]}'
```

## Rulers
```
node scripts/check-android-event-parity.mjs
node scripts/check-android-dropped-judgement.mjs
```
