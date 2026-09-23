# Store Listing Copy — App Store + Google Play

Grounded in real product facts — **re-checked 2026-09-22 by spec 080**: 24 built-in networks (`rust/crates/vela-core/src/app/network_admin.rs` `BUILTIN_CHAINS`); ERC-7730 clear signing shipped; dApps connect through an injected provider (extension + in-app browsers) — **WalletPair was dropped**, and there is no Bluetooth; Safe v1.4.1 + ERC-4337 + one to seven passkeys or security keys. Brand voice matches getvela.app ("An Ethereum wallet you actually own." / "A wallet that does less — on purpose."). *"Your keys. Your face." was retired 2026-09-15 by spec 059 — if it still appears in a submitted listing, it is stale copy.*

**Red lines respected:** no "audited"/"audit planned" claims (open-source = *anyone can inspect the code*, not a third-party audit); no "beta/alpha"; no price/exchange/ROI claims; no fiat on-ramp/trading claims; dApp connection described as the app's own browser injecting a provider — never as QR pairing, a relay, WalletConnect or Bluetooth.

> **Before submitting — corrections from spec 080 (2026-09-22).** The listings below were drafted before several product changes. Fix these in every language first; [`specs/080-site-content-accuracy/claim-ledger.md`](../../specs/080-site-content-accuracy/claim-ledger.md) has the canonical wording.
> - "sign with your face or fingerprint" / "a passkey stored in your device's secure hardware": a key can be a synced passkey (held by the password manager, not confined to secure hardware), another phone, or a hardware security key; face/fingerprint/PIN only *unlock* it (C-keys-2, C-auth-1).
> - "Your passkey is backed up by iCloud/Google, so you can sign in on a new device": recovery is signing in with any of up to seven founding keys, found through the on-chain registry (C-keys-1, C-sync-1).
> - "No tracking": the website uses cookieless analytics and the relay logs operations for a limited time; say what the privacy policy says.
> - Network count is now 24 (fixed below).
> - "Pair with desktop dApps by scanning a QR code": WalletPair was never built. dApps connect through the provider injected by the app's own browser (and by the Chrome extension); there is no pairing step and no WalletConnect (C-dapp-1). **Fixed below in en and zh — check the other languages before submitting.**

Field character limits noted as `(≤N)`. Counts verified for the English fields.

---

## 1) Apple — App Store Connect

**App Name** (≤30) — `Vela Wallet`

**Subtitle** (≤30) — `Self-custody, no seed phrase` (28)

**Promotional Text** (≤170, editable without review):
> An Ethereum wallet you actually own. Self-custodial, no seed phrase — sign with Face ID, recover across devices, and read every transaction before you approve it.

**Keywords** (≤100, comma-separated, no spaces wasted; "wallet" already in the title so it's omitted here):
```
crypto,ethereum,web3,self-custody,passkey,seedless,defi,smart account,erc-4337,base,arbitrum,evm
```
(97 chars)

**Description** (≤4000):
```
Vela is a self-custodial crypto wallet for Ethereum and EVM networks. You hold your own keys — but there's no twelve-word seed phrase to write down, hide, or lose. You sign with a passkey, using your face or fingerprint.

YOUR KEYS. YOUR FACE.
Your signing key is a passkey stored in your device's secure hardware. There's no seed phrase to phish, screenshot, or misplace. If you can unlock your phone, you can use your wallet.

A REAL SMART ACCOUNT
Every Vela wallet is a Safe smart account (v1.4.1) running on ERC-4337 account abstraction — a standard trusted across Ethereum. One address works on every network.

24 NETWORKS, ONE ADDRESS
Ethereum, Base, Arbitrum, Optimism, Polygon, BNB Chain, Avalanche, Gnosis, Unichain, Tempo, Monad, and World Chain — plus any custom EVM network you add. See every balance and USD value in one portfolio.

NO BLIND SIGNING
Vela decodes transactions into plain, human-readable intent (ERC-7730) before you approve. Unknown calls are flagged, not hidden — so you always know what you're signing.

LOSE YOUR PHONE? KEEP YOUR WALLET.
Cross-device recovery is built in. Your passkey is backed up by iCloud, so you can sign in on a new device — no seed phrase, no support ticket.

CONNECT TO APPS
Open a web3 app in Vela's own browser. Every request is decoded and shown to you before you approve it with your key.

ON-CHAIN PRICING
Prices come straight from on-chain DEX quotes with a Chainlink oracle fallback — no third-party price API watching your portfolio.

OPEN SOURCE, END TO END
The wallet and all of its backend services are public on GitHub. Inspect exactly what Vela does, or self-host every service for full independence. Vela cannot move, freeze, or recover your funds — only you can.

No accounts. No email. No tracking. No seed phrase.
A wallet that does less — on purpose.
```

**What's New** (release notes, v1.0):
```
The first public release of Vela Wallet:
• Self-custody with no seed phrase — sign with Face ID or Touch ID
• A Safe smart account across 24 EVM networks, one address
• Clear transaction signing (ERC-7730) — read every call before you approve
• Cross-device recovery via your passkey
• On-chain pricing, no third-party price API
```

---

## 2) Google Play — Play Console

**App title** (≤30) — `Vela Wallet`

**Short description** (≤80):
> Self-custodial crypto wallet. No seed phrase — sign with your face or fingerprint. (81 → trim to:)
> `Self-custodial crypto wallet — no seed phrase, sign with your face.` (66)

**Full description** (≤4000) — Play indexes this for search, so keywords are woven in naturally:
```
Vela is a self-custodial crypto wallet for Ethereum and EVM networks. You hold your own keys — but there's no twelve-word seed phrase to write down, hide, or lose. You sign with a passkey, using your face or fingerprint.

YOUR KEYS. YOUR FACE.
Your signing key is a passkey held in your device's secure hardware and synced by Google. No seed phrase to phish, screenshot, or misplace. If you can unlock your phone, you can use your wallet.

A REAL SMART ACCOUNT
Every Vela wallet is a Safe smart account (v1.4.1) on ERC-4337 account abstraction — a standard trusted across Ethereum. One address works on every network.

24 NETWORKS, ONE ADDRESS
Ethereum, Base, Arbitrum, Optimism, Polygon, BNB Chain, Avalanche, Gnosis, Unichain, Tempo, Monad, and World Chain — plus any custom EVM network you add. Every balance and USD value in one portfolio.

NO BLIND SIGNING
Vela decodes transactions into human-readable intent (ERC-7730) before you approve. Unknown calls are flagged, not hidden — you always know what you're signing.

LOSE YOUR PHONE? KEEP YOUR WALLET.
Cross-device recovery is built in. Your passkey is backed up by Google, so you can sign in on a new device — no seed phrase, no support ticket.

CONNECT TO APPS
Open a web3 app in Vela's own browser. Every request is decoded and shown to you before you approve it with your key.

ON-CHAIN PRICING
Prices come from on-chain DEX quotes with a Chainlink oracle fallback — no third-party price API watching your portfolio.

OPEN SOURCE, END TO END
The wallet and all of its backend services are public on GitHub. Inspect exactly what Vela does, or self-host every service. Vela cannot move, freeze, or recover your funds — only you can.

No accounts. No email. No tracking. No seed phrase.
A wallet that does less — on purpose.
```

---

## 3) 简体中文(App Store + Google Play 通用,按需微调字数)

**App 名称 / 标题**(≤30)— `Vela Wallet` (或 `Vela 钱包`)

**副标题 / 短描述**(App Store 副标题 ≤30;Play 短描述 ≤80):
> `自我托管钱包,无需助记词` (App Store 副标题用)
> `自我托管加密钱包 —— 无需助记词,用面容或指纹签名。` (Play 短描述用)

**关键词**(App Store ≤100):
```
加密钱包,以太坊,web3,自我托管,通行密钥,助记词,defi,智能账户,erc4337,base,arbitrum
```

**描述**(≤4000):
```
Vela 是面向以太坊及 EVM 网络的自我托管加密钱包。私钥由你掌握,却不再有 12 个助记词需要抄写、藏匿或担心丢失 —— 你用面容或指纹,通过通行密钥(passkey)签名。

你的钥匙,你的脸。
签名密钥是一枚存放在设备安全硬件中的通行密钥。没有助记词可被钓鱼、截图或遗失。能解锁手机,就能用钱包。

真正的智能账户
每个 Vela 钱包都是一个 Safe 智能账户(v1.4.1),基于 ERC-4337 账户抽象运行。一个地址,通行所有网络。

24 条网络,一个地址
以太坊、Base、Arbitrum、Optimism、Polygon、BNB Chain、Avalanche、Gnosis、Unichain、Tempo、Monad、World Chain,以及你自定义的任意 EVM 网络 —— 所有余额与美元估值,尽在一个资产视图。

拒绝盲签
签名前,Vela 会把交易解析成清晰、可读的意图(ERC-7730)。未知调用会被标记,而不是被隐藏 —— 你始终清楚自己在签什么。

手机丢了,钱包还在
内置跨设备恢复。通行密钥由 iCloud / Google 备份,换新设备直接登录 —— 不需要助记词,也不用联系客服。

连接 dApp
在 Vela 自带的浏览器里打开 web3 应用。每一次请求都先解码给你看,再由你用钥匙批准。

链上报价
价格直接来自链上 DEX 报价,并以 Chainlink 预言机兜底 —— 没有第三方报价接口窥探你的资产。

完全开源
钱包及其全部后端服务均在 GitHub 公开。你可以亲自查验 Vela 到底做了什么,或自建全部服务实现彻底独立。Vela 无法转移、冻结或找回你的资金 —— 只有你可以。

无账号,无邮箱,无追踪,无助记词。
一款"刻意做得更少"的钱包。
```

---

## 4) Notes & open choices

- **Cross-device recovery wording per store:** the iOS listing says "backed up by iCloud", the Android listing says "backed up by Google" — each store's listing should name only that platform's mechanism (BlockStore/Google Password Manager).
- **App Store subtitle vs Play short description** are different fields with different limits — don't copy one into the other.
- **Screenshots still needed** (separate task): App Store 6.9"/6.7" iPhone; Play needs ≥2 phone screenshots + a 1024×500 feature graphic.
- **More locales:** the app ships ~15 locales. I can produce listing copy for any of them (de, es-MX, fr, id, it, ja, ko, pt-BR, ru, tr, vi, zh-HK, zh-TW) from this English canonical — say which markets to prioritize. Machine-translated store copy should get a native eyeball before publishing (per the i18n-localization note).
- The brand tagline `An Ethereum wallet you actually own.` (36) is too long for an App Store subtitle (30), so the ASO line `Self-custody, no seed phrase` stays — it wins on search anyway. The tagline belongs in the promotional text above.
```
