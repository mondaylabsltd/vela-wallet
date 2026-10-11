# Store Listing Copy — App Store + Google Play

Grounded in real product facts — **rewritten 2026-10-01 by spec 088 from [`specs/080-site-content-accuracy/claim-ledger.md`](../../specs/080-site-content-accuracy/claim-ledger.md)** (claim IDs in brackets below are that file's). Brand voice matches getvela.app ("An Ethereum wallet you actually own." / "A wallet that does less — on purpose.").

**Red lines respected:** no "audited" / "audit planned" claims — Vela's own code has had no third-party audit and none is scheduled (C-audit-1), and the listing does not mention Safe's audits either, so nothing can be misread; open source means *anyone can inspect the code*. No "beta" / "alpha". No price, exchange or ROI claims; no fiat on-ramp or trading claims. Fees: *the exact fee is shown before you sign*, never a multiple of the on-chain cost (C-fee-1). Safety is stated positively. dApps connect through the app's own browser injecting a provider — never QR pairing, WalletConnect or Bluetooth (C-dapp-1). Bluetooth appears only as the way another phone is reached during passkey sign-in.

> **Fixed 2026-10-01 (spec 088, audit B6).** Every item on the old correction list is done, in every language this file contains (en, zh-Hans):
> - "YOUR KEYS. YOUR FACE." / "你的钥匙，你的脸。" — **removed** (retired by spec 059).
> - "sign with your face or fingerprint" as the only way — **replaced**: a wallet's keys are passkeys on this device, another phone or tablet, or a hardware security key, plus the optional Trusted Signer page; each signature is unlocked by that key's own check (C-keys-2, C-auth-1, C-signpage-1).
> - "a passkey stored in your device's secure hardware" — **removed**: a synced passkey is held by the passkey provider, not confined to secure hardware (C-keys-2, C-sign-1).
> - "Your passkey is backed up by iCloud / Google, so you can sign in on a new device" — **replaced**: passkeys are synced, or not, by the person's passkey provider; Vela does not sync or back them up; recovery is signing in with any of the wallet's keys, found through the public registry (C-keys-1, C-sync-1).
> - "No accounts. No tracking." — **replaced** with "No email or phone number. No ads, and no analytics or tracking SDKs in the app." (privacy-evidence.md §0, §4: no analytics or crash SDK in either app; the registry record and relay logs mean "no accounts" was not true).
> - "Vela cannot move, freeze, or recover your funds" — **replaced** with C-custody-1 ("can't move or freeze your funds by itself").
> - "NO BLIND SIGNING" — **replaced** with "What you see is what you sign" plus the limit: undecodable calls carry a blind-signing warning (C-clear-1, C-clear-2).
> - Network list — now all 24 names, all mainnets (C-net-1); address wording follows C-addr-1.
> - The zh App Store text no longer names Google or Android (App Store 2.3.10); zh is now split into an App Store version and a Play version, and only the Play version names Google Password Manager. The en App Store text names no other platform either.
> - Character and byte counts are measured (Python `len`, UTF-8 for keyword bytes), not estimated.

Field limits are noted as `(≤N)`. App Store keywords are limited in **bytes** (a CJK character is 3 bytes in UTF-8); every other limit is characters.

---

## 1) Apple — App Store Connect (English)

**App Name** (≤30) — `Vela Wallet` (11)

**Subtitle** (≤30) — `Self-custody, no seed phrase` (28)

**Promotional Text** (≤170, editable without review) — 145 characters:
> An Ethereum wallet you actually own. No seed phrase: sign with a passkey, another phone or a security key, and see the exact fee before you sign.

**Keywords** (≤100 bytes, comma-separated; "wallet" is already in the name) — 96 bytes:
```
crypto,ethereum,web3,self-custody,passkey,seedless,defi,smart account,erc-4337,base,arbitrum,evm
```

> **Owner check before using the App Store text (audit I10).** A security key reaches the iPhone app two ways. (1) The app's own USB route reads the key through CryptoTokenKit, and works directly with a USB-C FIDO2 key that offers FIDO over its USB smart-card (CCID) interface — a YubiKey on firmware 5.8 or later (`SmartCardCtapCeremony.swift`). It needs **no entitlement**: `com.apple.security.smartcard` is a macOS key, on iOS 16 and later the smart-card slot manager is always available, and Xcode **drops the key at signing** — the spec 088 archive (2026-10-01) has it neither in the .xcent nor in the App Store export's entitlements — so every signed build, TestFlight included, runs this route the same (device-verified 2026-08-28: iPhone 15 Pro, YubiKey 5C, firmware 5.8, in a build without the key). (2) Any other key — a Lightning or NFC key, or older firmware — goes through **Apple's own security-key sheet**: "Insert your security key" offers "Use Apple's security-key sheet", and the app falls through to it by itself when the key does not answer over USB. Before using this text, create or sign with a USB security key on a TestFlight build, once each way. If neither works there, delete the line "• a compatible USB security key." here and "• 兼容的 USB 安全密钥。" in §3, and drop "or a security key" from the first paragraph and the promotional text.

**Description** (≤4000) — 3,793 characters:
```
Vela is a self-custody wallet for Ethereum and other EVM networks, with no seed phrase to write down. Your wallet is controlled by keys you choose, and each signature is unlocked by that key's own check: Face ID, Touch ID, your passcode, or a touch and PIN on a security key.

SIGNING STAYS ON YOUR DEVICE
Signing is done on your device. Your passkey's private key never goes to Vela. Vela holds no key and no role on your wallet, so it can't move or freeze your funds by itself.

YOUR KEYS, YOUR CHOICE
When you create a wallet, choose up to seven keys. Any one of them can sign on its own:
• a passkey on this iPhone, kept by iCloud Keychain or another passkey provider you choose, and synced to your other devices if that provider syncs it;
• another phone or tablet nearby, which scans a code on your screen;
• a compatible USB security key.
Or choose Vela's Trusted Signer page as the way you sign: it decodes each request and works out what to sign by itself, then asks your passkey.
The set is fixed when the wallet is created. Keys can't be added, removed or replaced later, so add a spare before you finish.

NEW PHONE? SIGN IN WITH ANY KEY
Your wallet list stays on each device, and Vela doesn't sync or back up your passkeys. On a new phone, sign in with any one of your wallet's keys: Vela finds the wallet in its public key registry and rebuilds it. No seed phrase, no support ticket.

Creating a wallet writes each key's public key, the wallet's name and key labels, and its address to a public registry on Gnosis Chain. That record is permanent and readable by anyone, so choose a name you're happy to make public.

ONE ADDRESS, 24 NETWORKS
Your address comes from all the keys you create the wallet with, and it's the same on every network: Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable, Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume and XRPL EVM, all mainnets. You can add other networks that meet its requirements. You can receive before the wallet exists on a network; it deploys itself with your first transaction there.

A STANDARD SMART ACCOUNT
Every Vela wallet is an unmodified Safe v1.4.1 smart account, run through ERC-4337 (EntryPoint v0.7). No contract in the funds path was written by Vela.

WHAT YOU SEE IS WHAT YOU SIGN
The confirm screen is decoded from the exact transaction you sign, using ERC-7730 descriptors, token standards and a public function-signature database. Anything Vela can't decode carries a clear blind-signing warning. Unlimited token approvals are shown in red, and an on-chain approval can be capped before you sign. Requests that would hand control of your wallet to someone else are refused.

THE EXACT FEE, BEFORE YOU SIGN
The exact fee is shown before you sign, and it is part of what you sign. Pay it from your wallet in the network's coin or a supported USD stablecoin, with no deposit and no gas account. Fees go to the relay your wallet is set to: Vela's by default, or one you run yourself.

DAPPS IN VELA'S OWN BROWSER
Open web3 apps in Explore, Vela's built-in browser. Sites connect through an injected wallet provider, and every request is decoded and shown to you before you approve it with your key.

PRICES READ ON-CHAIN
Token prices come from on-chain DEX quotes, checked against Chainlink price feeds, instead of a commercial price API.

OPEN SOURCE, RUN IT YOURSELF
Everything is MIT-licensed: the app, the relay, the public-key index, the exchange-rate service and the chain-data directory. In Settings you can point the app at your own relay, index, data services and RPC nodes.

No seed phrase. No email or phone number. No ads, and no analytics or tracking SDKs in the app.
A wallet that does less, on purpose.
```

**What's New** (≤4000; App Store Connect asks for it from the second version on — keep it for the first update) — 369 characters:
```
First release of Vela Wallet:
• Self-custody with no seed phrase: sign with a passkey, another phone or a security key
• An unmodified Safe smart account, with one address on 24 networks
• Decoded confirm screens (ERC-7730) and the exact fee before you sign
• Sign in on a new phone with any of your wallet's keys
• Prices read on-chain, not from a commercial price API
```

---

## 2) Google Play — Play Console (English)

**App title** (≤30) — `Vela Wallet` (11)

**Short description** (≤80) — 76 characters:
> `Self-custody Ethereum wallet. No seed phrase: sign with passkeys you choose.`

**Full description** (≤4000) — 3,841 characters. Play indexes this for search, so the network names and standards are spelled out:
```
Vela is a self-custody wallet for Ethereum and other EVM networks, with no seed phrase to write down. Your wallet is controlled by keys you choose, and each signature is unlocked by that key's own check: your fingerprint, face unlock, your screen lock, or a touch and PIN on a security key.

SIGNING STAYS ON YOUR DEVICE
Signing is done on your device. Your passkey's private key never goes to Vela. Vela holds no key and no role on your wallet, so it can't move or freeze your funds by itself.

YOUR KEYS, YOUR CHOICE
When you create a wallet, choose up to seven keys. Any one of them can sign on its own:
• a passkey on this phone, kept by Google Password Manager or another passkey provider you choose, and synced to your other devices if that provider syncs it;
• another phone or tablet nearby, which scans a code on your screen;
• a FIDO2 security key plugged into your phone's USB port.
Or choose Vela's Trusted Signer page as the way you sign: it decodes each request and works out what to sign by itself, then asks your passkey.
The set is fixed when the wallet is created. Keys can't be added, removed or replaced later, so add a spare before you finish.

NEW PHONE? SIGN IN WITH ANY KEY
Your wallet list stays on each device, and Vela doesn't sync or back up your passkeys. On a new phone, sign in with any one of your wallet's keys: Vela finds the wallet in its public key registry and rebuilds it. No seed phrase, no support ticket.

Creating a wallet writes each key's public key, the wallet's name and key labels, and its address to a public registry on Gnosis Chain. That record is permanent and readable by anyone, so choose a name you're happy to make public.

ONE ADDRESS, 24 NETWORKS
Your address comes from all the keys you create the wallet with, and it's the same on every network: Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable, Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume and XRPL EVM, all mainnets. You can add other networks that meet its requirements. You can receive before the wallet exists on a network; it deploys itself with your first transaction there.

A STANDARD SMART ACCOUNT
Every Vela wallet is an unmodified Safe v1.4.1 smart account, run through ERC-4337 (EntryPoint v0.7). No contract in the funds path was written by Vela.

WHAT YOU SEE IS WHAT YOU SIGN
The confirm screen is decoded from the exact transaction you sign, using ERC-7730 descriptors, token standards and a public function-signature database. Anything Vela can't decode carries a clear blind-signing warning. Unlimited token approvals are shown in red, and an on-chain approval can be capped before you sign. Requests that would hand control of your wallet to someone else are refused.

THE EXACT FEE, BEFORE YOU SIGN
The exact fee is shown before you sign, and it is part of what you sign. Pay it from your wallet in the network's coin or a supported USD stablecoin, with no deposit and no gas account. Fees go to the relay your wallet is set to: Vela's by default, or one you run yourself.

DAPPS IN VELA'S OWN BROWSER
Open web3 apps in Explore, Vela's built-in browser. Sites connect through an injected wallet provider, and every request is decoded and shown to you before you approve it with your key.

PRICES READ ON-CHAIN
Token prices come from on-chain DEX quotes, checked against Chainlink price feeds, instead of a commercial price API.

OPEN SOURCE, RUN IT YOURSELF
Everything is MIT-licensed: the app, the relay, the public-key index, the exchange-rate service and the chain-data directory. In Settings you can point the app at your own relay, index, data services and RPC nodes.

No seed phrase. No email or phone number. No ads, and no analytics or tracking SDKs in the app.
A wallet that does less, on purpose.
```

---

## 3) 简体中文 — App Store(不得出现 Google、Android 等其他平台名称,见 App Store 审核指南 2.3.10)

**App 名称**(≤30)— `Vela Wallet`(11)或 `Vela 钱包`(7)

**副标题**(≤30)— `自我托管钱包，无需助记词`(12 字符)

**推广文本**(≤170)— 51 字符:
> 真正属于你的以太坊钱包。没有助记词：用通行密钥、另一部手机或安全密钥签名，签名前就能看到确切的手续费。

**关键词**(≤100 字节)— 97 字节(51 字符):
```
加密钱包,以太坊,web3,自我托管,通行密钥,无助记词,defi,智能账户,erc4337,base
```

**描述**(≤4000)— 1,556 字符:
```
Vela 是面向以太坊及其他 EVM 网络的自我托管钱包，没有需要抄写的助记词。控制钱包的是你自己选定的钥匙，每次签名都要经过那把钥匙自身的验证——面容 ID、触控 ID、设备密码，或安全密钥上的触碰与 PIN。

签名在你的设备上完成
签名在你的设备上完成。通行密钥的私钥绝不会交给 Vela。Vela 不持有钥匙，在你的钱包上没有任何角色，所以自己无法动用或冻结你的资金。

钥匙由你来选
创建钱包时，最多可以选 7 把钥匙，任意一把都能单独签名：
• 这部 iPhone 上的通行密钥，由 iCloud 钥匙串或你选择的其他通行密钥提供方保管；若该提供方会同步，它就会同步到你的其他设备；
• 附近的另一部手机或平板，扫描你屏幕上的二维码即可；
• 兼容的 USB 安全密钥。
你也可以把 Vela 的可信签名器页面选为签名方式：它会自己解码每一个请求、自己算出要签的内容，再请你的通行密钥签名。
钥匙在创建钱包时就定下来，之后不能再增加、删除或替换，所以完成前请多加一把备用钥匙。

换新手机？用任意一把钥匙登录
账户列表保存在各台设备上，Vela 不会同步或备份你的通行密钥。在新手机上用钱包的任意一把钥匙登录，Vela 会在公开的公钥注册表里找到你的钱包并重建——不需要助记词，也不用联系客服。

创建钱包时，每把钥匙的公钥、钱包名称和钥匙名称，以及钱包地址，会写进 Gnosis 链上的公开注册表。这条记录永久保存、任何人可读，所以请起一个你愿意公开的名字。

一个地址，24 条网络
地址由创建钱包时的全部钥匙推导而来，在每条网络上都相同：以太坊、BNB Chain、Polygon、Arbitrum、Optimism、Base、Avalanche、Gnosis、Unichain、Tempo、Monad、World Chain、Arc、X Layer、Stable、Soneium、MegaETH、Robinhood Chain、Mantle、Kaia、Celo、Ink、Plume、XRPL EVM，均为主网；另可添加满足要求的其他 EVM 网络。钱包还没部署到某条链上时就能在那条链上收款，你在那条链上的第一笔交易会顺带部署它。

标准的智能账户
每个 Vela 钱包都是未经修改的 Safe v1.4.1 智能账户，通过 ERC-4337（EntryPoint v0.7）运行。资金路径上没有一个合约是 Vela 写的。

所见即所签
确认页的内容就是从实际签名的那笔交易解码出来的，依据 ERC-7730 描述文件、代币标准和公共函数选择器数据库。无法解码的交易会明确提示盲签。无限额度的代币授权以红色显示，链上授权可以在签名前设定上限。会把钱包控制权交给别人的请求一律拒绝。

确切手续费，签名前就显示
确切的手续费在你签名前显示，并且写在你签名的内容里。手续费从你的钱包里扣，用网络原生币或支持的美元稳定币支付，没有押金，也没有 gas 账户。手续费付给钱包所设的中继：默认是 Vela 的，也可以换成你自己运行的。

在 Vela 自带的浏览器里用 dApp
在「探索」里打开 web3 应用。网站通过注入的钱包 provider 连接，每一个请求都先解码给你看，再由你用钥匙批准。

链上读取价格
代币价格来自链上 DEX 报价，并与 Chainlink 价格源相互校验，不依赖商业报价接口。

开源，可以自己部署
全部采用 MIT 许可：App、中继、公钥索引、汇率服务和链数据目录。你可以在设置里把 App 指向你自己的中继、索引、数据服务和 RPC 节点。

没有助记词。不需要邮箱或手机号。没有广告，App 里也没有统计分析或追踪 SDK。
一款"刻意做得更少"的钱包。
```

---

## 4) 简体中文 — Google Play

**标题**(≤30)— `Vela Wallet`(11)

**短描述**(≤80)— 31 字符:
> `自我托管的以太坊钱包。无需助记词，用你自己选定的通行密钥签名。`

**完整描述**(≤4000)— 1,559 字符。与 §3 只有三处不同:开头的验证方式、第一种钥匙(Google 密码管理器)和安全密钥那一行:
```
Vela 是面向以太坊及其他 EVM 网络的自我托管钱包，没有需要抄写的助记词。控制钱包的是你自己选定的钥匙，每次签名都要经过那把钥匙自身的验证——指纹、人脸解锁、屏幕锁，或安全密钥上的触碰与 PIN。

签名在你的设备上完成
签名在你的设备上完成。通行密钥的私钥绝不会交给 Vela。Vela 不持有钥匙，在你的钱包上没有任何角色，所以自己无法动用或冻结你的资金。

钥匙由你来选
创建钱包时，最多可以选 7 把钥匙，任意一把都能单独签名：
• 这部手机上的通行密钥，由 Google 密码管理器或你选择的其他通行密钥提供方保管；若该提供方会同步，它就会同步到你的其他设备；
• 附近的另一部手机或平板，扫描你屏幕上的二维码即可；
• 插在手机 USB 接口上的 FIDO2 安全密钥。
你也可以把 Vela 的可信签名器页面选为签名方式：它会自己解码每一个请求、自己算出要签的内容，再请你的通行密钥签名。
钥匙在创建钱包时就定下来，之后不能再增加、删除或替换，所以完成前请多加一把备用钥匙。

换新手机？用任意一把钥匙登录
账户列表保存在各台设备上，Vela 不会同步或备份你的通行密钥。在新手机上用钱包的任意一把钥匙登录，Vela 会在公开的公钥注册表里找到你的钱包并重建——不需要助记词，也不用联系客服。

创建钱包时，每把钥匙的公钥、钱包名称和钥匙名称，以及钱包地址，会写进 Gnosis 链上的公开注册表。这条记录永久保存、任何人可读，所以请起一个你愿意公开的名字。

一个地址，24 条网络
地址由创建钱包时的全部钥匙推导而来，在每条网络上都相同：以太坊、BNB Chain、Polygon、Arbitrum、Optimism、Base、Avalanche、Gnosis、Unichain、Tempo、Monad、World Chain、Arc、X Layer、Stable、Soneium、MegaETH、Robinhood Chain、Mantle、Kaia、Celo、Ink、Plume、XRPL EVM，均为主网；另可添加满足要求的其他 EVM 网络。钱包还没部署到某条链上时就能在那条链上收款，你在那条链上的第一笔交易会顺带部署它。

标准的智能账户
每个 Vela 钱包都是未经修改的 Safe v1.4.1 智能账户，通过 ERC-4337（EntryPoint v0.7）运行。资金路径上没有一个合约是 Vela 写的。

所见即所签
确认页的内容就是从实际签名的那笔交易解码出来的，依据 ERC-7730 描述文件、代币标准和公共函数选择器数据库。无法解码的交易会明确提示盲签。无限额度的代币授权以红色显示，链上授权可以在签名前设定上限。会把钱包控制权交给别人的请求一律拒绝。

确切手续费，签名前就显示
确切的手续费在你签名前显示，并且写在你签名的内容里。手续费从你的钱包里扣，用网络原生币或支持的美元稳定币支付，没有押金，也没有 gas 账户。手续费付给钱包所设的中继：默认是 Vela 的，也可以换成你自己运行的。

在 Vela 自带的浏览器里用 dApp
在「探索」里打开 web3 应用。网站通过注入的钱包 provider 连接，每一个请求都先解码给你看，再由你用钥匙批准。

链上读取价格
代币价格来自链上 DEX 报价，并与 Chainlink 价格源相互校验，不依赖商业报价接口。

开源，可以自己部署
全部采用 MIT 许可：App、中继、公钥索引、汇率服务和链数据目录。你可以在设置里把 App 指向你自己的中继、索引、数据服务和 RPC 节点。

没有助记词。不需要邮箱或手机号。没有广告，App 里也没有统计分析或追踪 SDK。
一款"刻意做得更少"的钱包。
```

---

## 5) Where each claim comes from

| Listing sentence | Source |
|---|---|
| Up to seven keys, chosen at creation, any one signs, none added/removed/replaced later | C-keys-1; `create_wallet.rs:13-17` |
| Kinds of key; synced by the provider "if you allow it" | C-keys-2; `onboarding.create.method*Title` |
| Each signature unlocked by the key's own check | C-auth-1 |
| Private key never goes to Vela; signing on the device | C-sign-1; `onboarding.welcome.heroSubtitle` |
| Can't move or freeze funds by itself | C-custody-1 |
| Trusted Signer decodes and computes what to sign | C-signpage-1 |
| Wallet list not synced; sign in with any key, rebuilt from the registry | C-sync-1 |
| Registry record contents, permanent and public | C-reg-1; `onboarding.create.ack0` |
| 24 networks, all mainnets, add others that meet its requirements | C-net-1, C-p256-1 |
| Address from all founding keys, same on every network | C-addr-1 |
| Receive before deploy; first transaction deploys | C-deploy-1 |
| Unmodified Safe v1.4.1, ERC-4337 EntryPoint v0.7, no Vela contract in the funds path | C-acct-1 |
| What you see is what you sign; decoding ladder; blind-signing warning | C-clear-1, C-clear-2 |
| Unlimited approvals in red, on-chain approvals capped | C-approve-1 |
| Takeover requests refused | C-selfcall-1 |
| Exact fee shown before you sign and signed; coin or USD stablecoin; no deposit; relay replaceable | C-fee-1, C-fee-2, C-fee-3, C-relay-1 |
| dApps through the injected provider in the app's browser | C-dapp-1 |
| Prices from on-chain DEX quotes, checked against Chainlink | `rust/crates/vela-core/src/app/balance_dashboard.rs:35, 545-554` (DEX preferred; Chainlink wins outside the 0.5–2.0 band) |
| MIT, all services; endpoints replaceable in Settings | C-lic-1, C-selfhost-2; privacy-evidence.md §1.1 |
| No email or phone; no ads; no analytics or tracking SDKs | privacy-evidence.md §0, §4, §6.1 |

---

## 6) Notes & open choices

- **Platform names per store.** The App Store texts (en, zh) name only iCloud Keychain; the Play texts name only Google Password Manager. App Store 2.3.10 forbids naming other mobile platforms in App Store metadata.
- **Security key per platform.** iPhone: a USB-C FIDO2 key that offers FIDO over USB smart card (YubiKey firmware 5.8+) works directly on the app's own route, which needs no entitlement — the smartcard entitlement is not needed on iOS 16+ and is dropped at signing; every other key goes through Apple's security-key sheet — see the owner check in §1. Android: FIDO2 key over USB (OTG). Neither app's own route does NFC (on iPhone an NFC key can only go through Apple's sheet), so neither listing mentions it.
- **C-alpha-1 is left out on purpose.** The claim ledger's "Vela is alpha software: it works and holds real funds; start with small amounts" conflicts with this file's no-beta/alpha red line and the owner's no-Beta-banner rule. If you want it in, that is the canonical sentence.
- **Price.** The apps are a one-time purchase (C-plat-1); the store shows the price, so the description doesn't repeat it.
- **App Store subtitle vs Play short description** are different fields with different limits — don't copy one into the other.
- **Screenshots still needed** (separate task): App Store 6.9" iPhone; Play needs ≥2 phone screenshots + a 1024×500 feature graphic + a 512×512 icon (audit A20).
- **More locales:** the app ships 15 locales. Produce listing copy for others (de, es-MX, fr, id, it, ja, ko, pt-BR, ru, tr, vi, zh-HK, zh-TW) from this English text and the claim ledger's zh column; keep the per-store platform rule; get a native read before publishing. Production-only (spec 088 Assumptions).
- The brand tagline `An Ethereum wallet you actually own.` (36) is too long for the App Store subtitle (30), so the ASO line `Self-custody, no seed phrase` stays there; the tagline opens the promotional text.
