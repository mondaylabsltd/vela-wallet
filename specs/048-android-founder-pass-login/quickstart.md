# Quickstart — verifying 048

## Login recovery

**Core**
```
cd rust && cargo test -p vela-core --features i18n-all,crux login session
```
**Web** (own worktree/port per the concurrent-sessions rule)
```
cd app-web/vela-wallet && pnpm vitest run src/lib/onboarding/core/storage.test.ts src/lib/core/effect-loop.test.ts
node ../../rust/scripts/build-web.mjs && node ../../rust/scripts/build-web.mjs --check
pnpm exec playwright test e2e/login-old-shape.spec.ts --output=/tmp/pw-048
```
Manual: in a browser at the site, set `localStorage['vela.accounts']` to the
old-shaped fixture (see `contracts/login-recovery.md`), reload → the wallet
home; set it to `"{"` → the message with the two actions within 2 s.

**Android**
```
adb -s 9d5f42fb shell run-as app.getvela.wallet sh -c 'cat files/datastore/vela.preferences_pb' # inspect
# seed via the debug hook: am start … --es vela.seedAccounts '<old-shaped json>'
```
Then launch: the session leaves loading (wallet or a visible fault), `VelaLog`
shows `session.fault` if unreadable.

## Phone pass (parallel space, Xiaomi `9d5f42fb`)

Launch: `adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.parallelSpace true`.
Driver: `ui.py` (`tap_text`, `texts`, `shot`); tab centres 钱包 134,2150 · 通讯录 405,2150 · 探索 675,2150 · 设置 945,2150.

| Item | Steps | Expect |
|---|---|---|
| copy | 收款 → row copy → 转账 → recipient field → `input keyevent 279` | the field shows the copied address |
| list rows | 活动 全部 → tap a row | the tx detail sheet; Back ×1 → list, Back ×1 → home |
| share image | 收款 → row QR → 保存图片 | `/sdcard/Pictures/Vela/*.png` pulled; `zbarimg` decodes the address |
| contact dock | 通讯录 → contact → 转账 / 收款 / 二维码 | send form to them / receive / code sheet |
| 群发转账 | group → 群发转账 | send form with N split rows |
| groups | 通讯录 → 新建分组 → save; group ⋯ → rename / delete | listed / renamed / gone |
| identicon | each listed site → tap artwork | viewer sheet with that address |
| filters | 转账 → 稳定币 / Gas 币 / 其他; 全部网络 → pick Gnosis | list narrows; pill shows Gnosis |
| explorer | R2 / A2 / T2 → 在区块浏览器中查看 | browser opens the explorer URL (intent in logcat) |
| token detail | 转账 → form with the token; 收款 → its code; row → tx detail | |
| hero | tap amount → hidden; tap status line (airplane) → rescue sheet | |
| scanner | 扫码 → 翻转 | front preview (screenshot differs) |
| slider | 设置 → drag 字号 | `haptic detent` ×steps in logcat; sample text scales |
| haptics | filter chip, network pick, copy → one `haptic select` each; scroll/tab/back → none | |
| settings | network detail override typed; + 添加网络 foot; provider 检查密钥 | |

## Rulers
```
node scripts/check-android-event-parity.mjs
node scripts/check-android-dropped-judgement.mjs
```
