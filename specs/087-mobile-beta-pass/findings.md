# 087 findings log (device pass, main @ 67e2d193d)

| # | Platform | Area | Severity | Finding | Evidence |
|---|---|---|---|---|---|
| F01 | iOS (+core copy) | Onboarding chooser | S2 | "这台设备" subtitle reads "Touch ID 或 Windows Hello" on a Face ID iPhone (11, 16 sim) | shots/ios-318-signin.png, sim318-now.png |
| F02 | all (core copy) | Sign-in chooser | S3 | "手机或平板 · 扫码，用附近设备创建" in the SIGN-IN sheet — "创建" is wrong there | ios-318-signin |
| F03 | iOS | Home | S3 | 资产 section blank (no empty state) when there are no assets (¥0.00 account) | ios-318-launch.png |
| F04 | Android (core rule) | Activity | S1 | dApp records from 2026-09-28 still "处理中" 3 days later; record never got an op hash | and/home.png, act1.png |
| F05 | Android (core) | Activity detail | S2 | "哈希" row shows internal id `dapp-17905…796-tx`, copy copies it | and/act1.png |
| F06 | Android | Release config | S2 (088) | versionCode=1 for 0.9.5 — must be monotonic per upload | dumpsys |
| F07 | Android | Fix RPC sheet | S3 | "Chainlist" chip wraps mid-word ("Chainlis/t") | and/rpc-notice-s.png |
| F08 | Android (core net_health) | Home notice | S2 | "4 个网络 RPC 不可用" opens Settings tab + a sheet for ONE network (BNB); jargon "RPC" on Home; other 3 not shown. In China ~5 of 24 networks' public RPCs are permanently unreachable, so the Home notice never goes away even for networks with zero balance — noise; should warn only when an unreachable network holds (or last held) the person's assets | and/rpc-notice-s.png |
| F09 | Android | a11y | S3 | text-size slider content-desc = test ids ("text-scale-slider", "text-scale-0"…) read aloud by TalkBack | dump |
| F10 | Android | Home | S3 | hero total CN¥36.20 shown next to rows summing CN¥36.54 until a refresh (stale total vs fresh rows) | and/home.png vs refresh |
| F11 | Android | Activity row | S3 | "处理中 · 觉得九点半" dApp row has a blank amount cell | dump |
| F12 | all (relay pricing) | Send fee | S2 (owner) | Arbitrum USDT send, Safe not deployed: fee shown 0.00024 ETH ≈ CN¥4.32 (超快) / 0.000161 (标准) = ~4M gas × max bid (0.06/0.04 gwei) while base is 0.02 gwei; the number shown is a ceiling, likely 3× the actual charge. Relay-side; owner decision | and/speed |
| F13 | Android | Receive list | S3 | per-network QR icon content-desc "扫描二维码" (scan) though it SHOWS the code | dump |
| F14 | Android | Permissions | S3 | POST_NOTIFICATIONS asked on EVERY Receipt-stage composition (VelaNavHost.kt:871-877) — re-asked after a language switch recreated the activity; should ask once, in context | and dialog after language switch |
| F15 | Android | Browser | S3 | failed-load panel for a refused loopback says "网络不稳定" (network unstable) | ps-explore |
| F16 | Android | Explore | S3 | favourite labelled with the full <title> "Uniswap | Trade Crypto on DeFi's…" (cut "Uniswap | Tr…") — a site name/host would read better | ps-explore-home |
| F17 | Android | Activity rows | S3 | largest text size: titles/subtitles truncate ("dApp Trans…", "To 0×D400…1…") instead of wrapping | and/en-home-xl |
| F18 | iOS vs Android | Settings | S3 | parity: currency row "CNY · ¥8,277.48" (iOS) vs "CNY · ¥" (Android); time format default 13:45 (iOS) vs 1:45 PM (Android); About "Vela 钱包 v0.9.5" vs "v0.9.5 (67e2d19)" | ios-087-settings-strip |
| F19 | Android | Activity rows | S3 | rows titled "dApp 交易" for every dApp call (approve, swap…) — no intent wording when the record has one | ps-home |
| F20 | Android | Contacts | S3 | an unnamed contact is listed by its lowercase address "0xd400…130b" (elsewhere EIP-55 "0xD400…130b") | contacts-ps |
| F21 | Android | dApp browser | S2 | first load of a slow/unreachable site (app.uniswap.org, China direct network): 14–20 s of a fully white screen — the address bar, × and progress are in the view tree but covered (WebView over the chrome) — until the timeout panel appears; the person can't see what loads or close it | and/uni-timeline.png |
| F22 | iOS | a11y | S3 | accessibility labels are test ids: Send amount 'send.amount', Explore search 'explore.searchField' (VoiceOver reads it) | int-form tree |
| F23 | Android | Send form | S3 | an EMPTY recipient field ("0x…" placeholder) still shows an identicon (the same turtle every time) — reads like a real recipient | dark-strip2, send-form-s |
| F24 | Android | Feedback | S3 | "无法连接的 RPC" lists a raw chain id "1625" among network names (Optimism, BNB Chain, Polygon, 1625, Celo, Ink) | feedback |
| F25 | Android | dApp approval sheet | S3 | the spender is abbreviated two ways on one sheet: "0x111111...111111" (6+6, ASCII dots) in the summary vs "0x1111…1111" (4+4, ellipsis) below | approve-s |
| F26 | iOS | Send flow | S3 | leaving Send through the tab bar keeps that tab's flow; coming back to 钱包 resumes it (stale scanned recipient + network scope visible) instead of Home — per-tab state is iOS-standard, but a scoped scan lingers | int-stale-scope |
| F27 | iOS | Send journey | S2 | Back out of Send only pops the shell stack — SendStore.back() is never called, the core journey never closes, and the next Home 转账 resumes the abandoned journey (stale scanned recipient + network scope); Android closes it. Pre-existing on main, more visible with #332/#312 | int-stale-scope |
| F28 | iOS | Send form | S2 | the decimal keypad can't be dismissed (tap outside / drag do nothing, no Done key) and covers 继续 until the person scrolls the form up; the token card truncates the balance ("余额 0.1…") next to the new chevron | ios-send-filled, ios-send-scroll |
| F29 | iOS | Send receipt | S3 | while confirming, the screen shows "UserOp 哈希" (jargon) with the full op hash; its copy button (and the tx-hash one after) is labelled "复制地址" (copy ADDRESS) | ios-sent, ios-sent2 |
| F30 | Android | Splash | S3 | the new Vela splash (088 fixed the robot) is a hard-edged orange square tile, not the circle-masked adaptive icon | splash-f05 |
| F31 | Android | Send split rows | S3 | contentDescription carries test ids read aloud: 'pick-contact-收款人 1', 'recipient-address-…', 'recipient-amount-…' (same class as F09) | v331 dump |
| F32 | all (product) | Backup public keys | S2 (owner) | the Ethereum-mainnet backup is quoted ~0.0038 ETH ≈ CN¥68.71 at the default 超快 speed (owner's wallet holds ETH only on Arbitrum → 'ETH 余额不足以支付 Gas 费'); most beta users won't pay that — consider an L2 registry, sponsorship, or 标准 default for this sheet | v314 |
