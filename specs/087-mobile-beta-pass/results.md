# 087 results — mobile beta device pass

Devices: **Xiaomi alioth** (Android 13, 1080×2400) and **iPhone 11** (iOS 26.5.2), plus iOS 18 / 26.2 and Android 11 / 14 emulators and simulators. The owner's own wallet was read only. Real sends (dust, Gnosis) used the parallel space's golden Safe. Every fix was re-checked on an integration build of `main` plus every 086/087/088/089 branch (local branch `087-integration`).

## What works end to end (both phones)

- **Home:** balance, activity, assets, Receive (QR decodes; save; explorer).
- **Send:** real 0.001 xDAI on Gnosis.
  - Android: about 5 s.
  - iPhone: submitted → confirm countdown → "已发送" with tx hash.
- **Scan → send:** photo picker on Android; pay links on iOS.
- **Contacts:** import, including the encoding cases.
- **Explore and the dApp browser:**
  - connect sheet, `personal_sign`, SafeTx refusal, unlimited-approval warning with cap editor, EIP-6963 announce, checksummed `accountsChanged`;
  - failure panel with retry on a blocked site.
- **Settings:** language, text size, theme, region formats, advanced, feedback preview, Sign Out, keys, backup sheet.
- **Trusted signer:** iOS round trip (#318) and the Android owner's route.

## Findings → status

| # | Sev | Finding | Status |
|---|---|---|---|
| F01/F02 | S2/S3 | "Touch ID or Windows Hello" on a Face ID iPhone; "create" in sign-in | **#354**, verified on the iPhone ("Face ID", "扫码") |
| F03 | S3 | iOS empty Assets blank | **#355** |
| F04/F05 | S1/S2 | dApp records "处理中" forever; record id shown as the hash | **#353**, verified on the Xiaomi (未知, no hash row) |
| F07/F11/F15 | S3 | Chainlist chip split; blank amount cell; a refused page called "unstable network" | **#358**, Chainlist verified |
| F08 | S2 | "N 个网络 RPC 不可用" permanent on Home in China (networks with zero holdings) | **Owner ruling 2026-10-02:** keep the notice for every unreachable network (holdings are unknowable while it is down); no jargon; tap shows all → **spec 092** (`092-unreachable-network-notice`) |
| F09/F13 | S3 | a11y: test ids spoken (slider); "扫描二维码" on a show-QR button | **#357**, F13 verified on the iPhone |
| F10 | S3 | hero total stale next to fresh rows until refresh | open (minor) |
| F12 | S2 | fee shown is limit × max bid (Arbitrum ≈ CN¥4.3 to deploy + send); default speed 超快 | **Owner ruling 2026-10-02:** the fee display and the 超快 default are kept |
| F14 | S3 | Android notification permission re-asked | **#356** |
| F16 | S3 | favourite labelled with the full page title | open (minor) |
| F17 | S3 | activity rows truncate at the largest text size | open |
| F18 | S3 | iOS/Android differences in Settings value previews | open (minor) |
| F19 | S3 | every dApp row titled "dApp 交易" | open |
| F20 | S3 | unnamed contact shown in lowercase | open (minor) |
| F21 | S2 | Android browser white screen (no bar or ×) during a slow first load | **#360** (root cause: unclipped WebView); emulator-verified |
| F22/F31 | S3 | test ids as a11y labels (iOS `send.amount`, `explore.searchField`; Android split rows) | open; same class as #357 |
| F23 | S3 | empty recipient shows an identicon | open (minor) |
| F24 | S3 | feedback "无法连接的 RPC" lists a chain id ("1625") | open (minor) |
| F25 | S3 | approval sheet abbreviates the spender two ways | open (minor) |
| F26/F27 | S2 | iOS: leaving Send left the journey alive, so the next 转账 resumed it with a stale scanned recipient and scope | **#375**, verified on the iPhone |
| F28 | S2 | iOS decimal keypad can't be dismissed; covers 继续; balance truncates | **#376** (+ **#377** on top of #348), verified on the iPhone |
| F29 | S3 | iOS pending receipt says "UserOp 哈希"; hash copy button labelled "复制地址" | open |
| F30 | S3 | Android splash icon is a hard-edged square | open (088 #352 already replaced the robot) |
| F32 | S2 | backing up public keys on Ethereum mainnet quoted ≈ CN¥68 | **Owner ruling 2026-10-02:** the Ethereum backup cost is accepted |

## Not covered by the device pass

- **Onboarding (create / sign-in) on a fresh Android install:** it would wipe the owner's phone. Copy is covered by #354; the iOS create round trip is covered by #318's test.
- **Final Face ID / fingerprint on the trusted-signer page:** needs a person. The page, the slide and the system passkey sheet were reached on the iPhone.
