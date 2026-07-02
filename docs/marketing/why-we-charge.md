# "Why We Charge $39.99" — 宣言初稿

> 用途:getvela.app 博客 + Show HN / Crypto Twitter 投放 + 商店 listing 与官网定价区的叙事底稿。
> 语气:创始人第一人称,honest-alpha。所有事实句均对应 whitepaper 行号(见文末核对表)。
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

**Two:** when you send a transaction, Vela's relayer charges roughly the network fee itself — you pay about twice the raw on-chain cost, and the wallet shows you the split, side by side, before you confirm.

That's it. There is no token — nothing to buy, farm, or speculate on. There are no ads. Our servers see your public key and a name you choose; there is nothing else to monetize. You pay, so you are the customer. The incentives point one way: at you.

## What $39.99 actually buys

Your phone already contains a dedicated security chip — the same hardware that guards Apple Pay. Vela turns it into your signing device: your key is generated inside it, never leaves it, and every transaction requires your face or fingerprint.

The cheapest hardware wallets start around fifty dollars. They are good products. But they are a second device — one more thing to buy, charge, carry, hide, and lose. $39.99 buys the hardware-wallet security model on hardware you already own, with clear signing, spending caps on every approval (the app will not let you sign an unlimited one), and a balance preview before every signature.

One bad blind signature costs more than every app you have ever bought, combined. That is the comparison that matters — not what other apps charge.

## Try everything free first

The web version at getvela.app is free, full-featured, forever. It is not a demo. Create a wallet in your browser, put in a small amount — small amounts first is genuinely how we think you should start — connect a dApp, send a transaction, read every screen.

If, and only if, it earns your trust: the mobile app is there. The web version is our refund policy — you will know exactly what you are paying for before you pay.

## What I will not tell you

Vela is alpha software, built in the open by a very small team. The Safe smart contracts it stands on are audited and battle-tested, securing billions. Vela's own integration around them has not undergone an independent third-party audit, and none is currently scheduled — a professional audit is a goal for when the project can fund one, not a commitment I can honestly make today.

Everything is MIT open source — the app and all three backend services. You can read the code, build it yourself, and point the app at your own servers. If Vela the company disappears tomorrow, your funds sit in your Safe on-chain, controlled by your passkey, and the docs explain how to keep going without us.

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
| relayer ≈ 网络费本身,约 2× 原始链上成本,确认前显示拆分 | whitepaper.md L119-125 |
| 无代币,"nothing to buy, farm, or speculate on" | whitepaper.md L227-228 |
| Safe 合约已审计;Vela 自身集成**未**审计且无排期;审计是"有钱后的目标非承诺" | whitepaper.md L230-239 |
| 服务端只见公钥+名字 | FAQ(leads #24) |
| "If Vela disappears" 自续方案 | whitepaper.md L197 |
| MIT,三个后端全开源可自托管 | leads #6/#25 |
| 永不无限授权(签不出 unlimited) | approval-guard.ts,leads #17 |
| 最便宜硬件钱包 ~$50 起 | Trezor Safe 3 $59 / Nano S Plus ≈$44(pricing-analysis.md 已核实) |
| Web 版免费、全功能 | 创始人定价决策 2026-07-02 |
| ⚠️ "guards Apple Pay" 类比 | leads #22 既有话术;iOS 成立,Android 对应 StrongBox——若在 Android 商店页复用,把 Apple Pay 换成 "hardware-backed keystore" |

## 发布注意

- 首发渠道顺序:getvela.app 博客(canonical)→ Show HN(标题 A)→ CT 线程版(拆 8-10 条推,头推用"One bad blind signature costs more than every app you've ever bought")
- HN 评论区预备答案:"为什么不 IAP 试用?"(Web 就是试用)、"开源为什么还收费?"(付的是签名分发+更新+供养维护,欢迎自己编译)、"未审计凭什么收费?"(正文已自答,链接 whitepaper 审计节)
- 本文中文版可后做(面向华语 CT/微信公号),叙事骨架不变
