# "Why We Charge $39.99" — 宣言初稿

> 用途:getvela.app 博客 + Show HN / Crypto Twitter 投放 + 商店 listing 与官网定价区的叙事底稿。
> 语气:创始人第一人称,honest-alpha。所有事实句均对应 claim ledger 条目(见文末核对表)。
> 2026-09-22 按 spec 080 更正:手续费按代码写(预留 gas × 3 × 所选速度价格,签名前显示,付给中继,中继可换可自建),去掉"约 2×/拆分显示";去掉"Apple Pay 同款芯片、私钥不出芯片";网页版不连 dApp;服务许可证按实际写。
> 红线检查:✅ 未称"已审计/审计排期" ✅ 未写 "free app" ✅ "先用小额"当卖点 ✅ 未点名贬低具体竞品。

---

## 正文(英文,~700 词)

# Every wallet is free. Ours costs $39.99. On purpose.

I build Vela, a passkey wallet. The iOS and Android apps cost $39.99, once. In a category where every single competitor is a free download, I owe you an explanation.

## Free wallets are businesses too

A wallet is expensive to build and run. When the download is free, the money comes from somewhere else — a token sale, spreads on in-app swaps, order-flow deals, data partnerships, or a venture round that eventually demands one of the above. None of that is evil. But it means the wallet's real customer is often someone other than you.

I wanted a business model I could explain in two sentences, to anyone, without flinching.

## Our entire business model, in two sentences

**One:** the mobile app costs $39.99, once. No subscription, no upgrade fees, all features, updates included.

**Two:** every transaction pays a fee to the relay that submits it — ours, unless you switch to another relay or run your own. The wallet sets it at three times the gas it reserves for the transaction, priced for the speed you pick, with a one-cent minimum, and shows you the exact amount before you sign. It can't change after you do.

That's it. There is no token — nothing to buy, farm, or speculate on. There are no ads. Our servers see only what running your wallet requires — your public keys, and the transactions our relay submits for you — and there is no ad or data business to feed. You pay, so you are the customer. The incentives point one way: at you.

## What $39.99 actually buys

Your phone can already hold a passkey — a key your operating system or password manager keeps end-to-end encrypted, which signs only after your face, fingerprint or PIN. Vela makes that your signing key, and its private key never goes to us. If you want a key that never leaves one piece of hardware, add a security key when you create the wallet; a wallet can have up to seven keys.

The cheapest hardware wallets start around fifty dollars. They are good products. But they are a second device — one more thing to buy, charge, carry, hide, and lose. $39.99 buys a wallet whose keys live on hardware you already own, with clear signing, a block on unlimited-level approvals until you pick an amount, and a preview of what leaves and enters your wallet before you sign.

One bad blind signature costs more than every app you have ever bought, combined. That is the comparison that matters — not what other apps charge.

## Try everything free first

The web version at wallet.getvela.app is free, forever — and so are the browser extension and the desktop apps. It is not a demo. Create a wallet in your browser, put in a small amount — small amounts first is genuinely how we think you should start — send a transaction, connect a dApp through the extension, read every screen.

If, and only if, it earns your trust: the mobile app is there. The web version is our refund policy — you will know exactly what you are paying for before you pay.

## What I will not tell you

Vela is alpha software, built in the open by a very small team. The Safe smart contracts it stands on are audited and battle-tested, securing billions. Vela's own integration around them has not undergone an independent third-party audit, and none is currently scheduled — a professional audit is a goal for when the project can fund one, not a commitment I can honestly make today.

Everything is open source: the apps, the relay, the exchange-rate service and the chain data under MIT, and the public-key index in public but not yet licensed. You can read the code, build it yourself, and point the apps at your own servers — the relay included. If Vela the company disappears tomorrow, your funds sit in your Safe on-chain, controlled by your keys, and the self-hosting guide explains how to keep going without us. One limit it states plainly: your passkeys belong to getvela.app, so you keep signing through the browser extension or an app you build.

I would rather earn your $39.99 with that paragraph than with a security badge I have not paid for yet.

## The deal

You pay once. You get a signing device you already carry, a business model with nothing hidden in it, and software you can verify instead of trust.

**Try it free in your browser → getvela.app**
**Then, if it earns it: $39.99, once.**

---

## 标题备选

- A: **"Every wallet is free. Ours costs $39.99. On purpose."** — 直接把品类反差做成钩子,适合 HN(推荐)
- B: "The hardware wallet you already own" — 设备定位型,适合商店 listing 首屏/官网 hero,弱化价格先讲价值
- C: "Why we charge for a wallet (and what free really costs)" — 议题型,适合 SEO/长尾

## CTA 备选

- A: "Try it free in your browser" → getvela.app(主 CTA,零风险动作)
- B: "Read the code" → GitHub(C 画像专用,放文末)
- C: "Get Vela for iOS / Android — $39.99, once"(商店页/官网定价区用,价格必须和 "once" 连写)

## 事实核对表(发布前逐条过)

| 正文声明 | 来源 |
|---|---|
| 手续费 = 预留 gas × 3 × 所选速度价格,最低 1 美分,签名前显示确切金额,付给中继;中继可换可自建 | claim ledger C-fee-1;`fee_policy.rs` `INBAND_MARKUP = 3` |
| 无代币,"nothing to buy, farm, or speculate on" | whitepaper.md L227-228 |
| Safe 合约已审计;Vela 自身集成**未**审计且无排期;审计是"有钱后的目标非承诺" | whitepaper.md L230-239 |
| 服务端只见公钥与经中继提交的交易 | privacy policy;claim ledger |
| "If Vela disappears" 自续方案 | whitepaper.md L197 |
| 许可证:App/中继/汇率/链数据 MIT,公钥索引暂无许可证;中继链目录可配置 | claim ledger C-lic-1、C-selfhost-2 |
| "无限"级授权(≥2^200)须改成具体金额才能提交 | claim ledger C-approve-1 |
| 最便宜硬件钱包 ~$50 起 | Trezor Safe 3 $59 / Nano S Plus ≈$44(pricing-analysis.md 已核实) |
| Web 版、扩展、桌面版免费;网页版不连 dApp(扩展连) | 创始人定价决策;claim ledger C-dapp-1 |
| 通行密钥由系统/密码管理器端到端加密保管,私钥不交给 Vela;要"不出硬件"就加安全密钥 | claim ledger C-sign-1、C-keys-1(已删 "Apple Pay 同款芯片/私钥不出芯片":同步的通行密钥会离开设备) |

## 发布注意

- 首发渠道顺序:getvela.app 博客(canonical)→ Show HN(标题 A)→ CT 线程版(拆 8-10 条推,头推用"One bad blind signature costs more than every app you've ever bought")
- HN 评论区预备答案:"为什么不 IAP 试用?"(Web 就是试用)、"开源为什么还收费?"(付的是签名分发+更新+供养维护,欢迎自己编译)、"未审计凭什么收费?"(正文已自答,链接 whitepaper 审计节)
- 本文中文版可后做(面向华语 CT/微信公号),叙事骨架不变
