---
title: Bybit 被攻击那次，以及它走的那条路
description: 2025 年 2 月，Bybit 损失约 15 亿美元。被攻破的不是 Safe 合约，而是界面。这一页讲清楚那条路径，以及 Vela 的设计里有什么把它堵上了。
---

# Bybit 被攻击那次，以及它走的那条路

2025 年 2 月 21 日，Bybit 从一个 Safe 多签冷钱包里损失了大约 **15 亿美元**。这是
这个行业历史上最大的一次失窃，而且值得仔细读，因为除了一件事之外，那天几乎所有
环节都是*正确*的。

## 发生了什么

按公开的事后复盘，简短版本是：

1. 攻击者攻陷了一台 **`Safe{Wallet}` 开发者的机器**，把恶意 JavaScript 注入到为
   `Safe{Wallet}` 前端提供服务的 AWS S3 存储桶里。代码在 2 月 19 日植入，
   2 月 21 日被触发，目标精确指向 Bybit 那个特定的 Safe。
2. Bybit 的签名人打开界面，审阅了一笔看起来再正常不过的交易。
3. 真正送到他们**硬件钱包**里的载荷，并不是那笔交易。它是一次 `delegatecall`，
   覆写了 Safe 代理合约的 `masterCopy`——第 0 号存储槽——把整个账户实现换成了
   攻击者的。
4. 签名人批准了。签名是有效的。合约严格按照它被告知的去做了。

归因上，公开将其指向与朝鲜相关的活动（FBI 点名了 TraderTraitor 集群）。

## 什么*没有*被攻破

- **不是 Safe 合约。** 它们执行的是一条签名有效的指令。Safe 没有任何漏洞被利用。
- **不是密码学。** 每一个签名都是真的。
- **不是硬件钱包。** Ledger 设备就在流程里，而且照样签了——因为硬件钱包显示的是
  交到它手里的东西，而交到它手里的正是那份恶意载荷。一台没办法把 `delegatecall`
  解成人类看得懂的东西的设备，保护的是*钥匙*，不是*决定*。

被攻破的，是每一个钱包界面底下的那个假设：**描述这笔交易的屏幕，和正在被签名的
那串字节，是同一样东西。**

## 为什么这是普遍情况，而不是一次离奇事件

你在网页钱包里产生过的每一个签名，都建立在那个假设上。界面构造载荷，界面渲染摘要，
而没有任何独立的东西去核对两者是否一致。一旦给那个界面提供服务的代码被替换——
被攻陷的构建流水线、被劫持的 CDN、一个恶意依赖、一份被偷走的部署凭证——摘要就会
变成攻击者想要的样子，而你的签名是真的。

这正是 Vela 的签名设计所针对的风险。不是钓鱼。不是私钥泄露。**是一块在骗你的
签名屏幕。**

## Vela 为此做了什么

**清晰签名，一直看到 calldata。** 每一笔交易在你批准之前都会被解码成人能读懂的
意图——金额、收款方、这个调用到底做什么（[ERC-7730](/zh/docs/clear-signing)）。
解不开的调用会被**明确标记为无法解码**，而不是悄悄渲染成一副没事的样子。Bybit 那份
载荷是一次替换实现地址的 `delegatecall`；那恰恰是应该让签名人当场刹车的东西，而把它
藏在一段友好的摘要后面，正是它没有被刹住的原因。

**一条能反过来核对界面的独立路径。** Vela 正在做一个零构建、零依赖的签名页面，
它自己渲染意图、自己完成 WebAuthn 签名——一个你可以从头读到尾、自己托管、或者当成
浏览器扩展来跑的静态文件夹。它存在的全部意义，就是给出一个不与主应用共享供应链的
第二意见。*状态：已经做完并测试过，尚未部署。* 上线之后它是可选的，等状态变了，
这一页会直说。

**没有我们能升级的合约。** Bybit 那份载荷之所以奏效，靠的是替换账户的实现。Vela 的
账户是[未经改动的 Safe v1.4.1](/zh/docs/account-contract)，Vela 在上面没有任何特权
角色——没有管理员密钥，也没有一条我们可能被胁迫或被攻陷后拿来用的升级通道。

**每一次签名都要重新做一次生物识别。** 不存在长期有效的会话密钥，所以也不存在
一个「你不在场却有人能替你签」的时间窗。

**自行部署，是最后那道兜底。** 应用和每一项后端服务都是开源的。如果你压根不想信任
我们的构建流水线，那就自己跑一份——面对这一类攻击，这是唯一一个不需要信任任何人的
答案。

## Vela 不声称什么

Vela 的前端完全可能以 `Safe{Wallet}` 被攻陷的同样方式被攻陷。我们的代码没有审计过。
说得比这更漂亮，恰恰是这次事件本该终结的那种保证。

这套设计想做的，是把路径收窄：让载荷可读而不是不透明，去掉这次攻击所依赖的升级
原语，并且给你一种用「不是我们」的东西去核对的办法。诚实的总结是：
**这一类攻击是靠设计被削弱的，不是被消除的**，而那些能让它更硬的部分，
正以未完成的状态列在[审计与已知问题](/zh/docs/security-audits)里。

## 资料来源

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
