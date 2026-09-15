---
title: 审计与已知问题
description: Vela 依赖的每一个链上合约、谁审计的、审计过的版本和实际部署的是否一致，以及哪些部分根本没有审计。
---

「审计过」是一个关于某段特定代码某个特定版本的说法，所以这一页不打算把这个词拿来
挥一挥就算——它列出确切的报告、确切的部署地址，以及被审计版本与被部署版本之间的
差异。它同时列出哪些**没有**被审计，因为后面这份清单和前面那份一样承重。

最近一次核对：2026 年 8 月。如果你在这里发现错误，告诉我们，我们会改。

## 资金路径

有四层合约能碰到你的钱。四层全部是带有公开审计报告的第三方合约，而且每一处部署
地址都是官方的规范部署。

### Safe v1.4.1 —— 账户本身

你的钱包是一个 [Safe](https://github.com/safe-global/safe-smart-account) 代理：
SafeL2 单例、代理工厂、兼容性 fallback handler，以及用于批量交易的 MultiSend。

[Ackee Blockchain 审计了 Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
（终版报告为 2023 年 3 月）：11 项发现，没有关键级或高危级。我们部署的 v1.4.1 与
被审计的 v1.4.0 之间，只差一处单行的 ERC-4337 兼容性修复
（[PR #572](https://github.com/safe-global/safe-smart-account/pull/572)）。
MultiSend 的逻辑自[经 G0 Group 审计的 v1.3.0](https://github.com/safe-global/safe-smart-account/tree/main/docs)
以来没有变过。所有地址都与
[safe-deployments](https://github.com/safe-global/safe-deployments) 里的规范部署一致，
而且这些合约在 [Safe Foundation 的漏洞赏金](https://docs.safefoundation.org/security/bug-bounty)
范围内（关键级最高 100 万美元）。

有一件事审计覆盖不到：2025 年的 Bybit 事件。那次攻击攻陷的是 Safe 官方网页前端的
构建流水线，而不是合约——
[官方取证结论](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
认为 Safe 智能合约中没有漏洞。我们把它读作一堂关于网页与运维层的课，而那一层，
正是你也应该拿来审视我们的地方。

### Safe4337Module v0.3.0 —— ERC-4337 适配器

部署在 `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`，即 v0.3.0 的规范地址
（Sourcify 精确匹配——链上字节码就是被审计的那份代码）。
[由 Ackee Blockchain 审计](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
（终版报告为 2024 年 3 月），没有任何高于「提示级」的未解决发现。我们使用的
v0.3.0 + EntryPoint v0.7 + Safe ≥1.4.1 这个组合，正是审计报告和发布说明所描述的配置。

这个模块的历史上有一个已披露的问题：v0.1.0（2023 年）没有对 `initCode` 和
`paymasterAndData` 签名，构成一个 gas 骚扰向量。它
[已在 v0.2.0 中修复](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)，
而且 v0.1.0 从未离开过测试网。我们用的是 v0.3.0，它继承了这个修复。

### SafeWebAuthnSharedSigner v0.2.1 —— 通行密钥签名器

部署在 `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`，即 v0.2.1 的规范地址
（通过 Safe 的单例工厂，在每条链上都相同）。

「shared（共享）」的意思是什么、不是什么：共享的是*这份合约部署*，就像 Safe 的
单例被共享一样。你的钥匙并不共享。每一个 Safe 都通过 delegatecall 调用
`configure()`，把自己的 P-256 公钥存进自己的存储里。一个签名器实例对应的，恰好是
每个 Safe 的一把通行密钥，别人的 Safe 用不了你的。

这里版本很重要。v0.2.0 的审计报告
[明确写明](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
共享签名器不在范围内——当时这份合约还不存在。覆盖我们所部署内容的，是 v0.2.1 的
那几份：一场 [Hats Finance 审计竞赛](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
（2024 年 6–7 月：零高危、零中危、三项低危——全部已修复），加上
[Certora 对发布 commit 的复核](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)，
没有新的发现。自发布以来没有任何合约级漏洞被披露；通行密钥相关合约也在
Safe Foundation 的赏金范围内。

Safe 自己的文档建议：把通行密钥所有权与一条恢复路径搭配使用，而不是把单一凭证当成
账户唯一的钥匙。Vela 怎么处理这件事，记在[恢复与登录](/zh/docs/recovery)里。

链上的 P-256 验证直接使用 RIP-7212 预编译，没有 Solidity 的回退验证器。在启用任何
网络之前，应用都会用一个真实签名去探测这个预编译，验证失败就拒绝该网络。有两条
老实话：最初的 RIP-7212 规范存在一些边缘情况缺陷，
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) 正是为修复它们而写的
（它们不影响格式正确的 WebAuthn 签名）；而一次探测，也不可能覆盖某条链的实现在
不寻常执行上下文里可能发生偏差的每一种方式。

### EntryPoint v0.7 —— ERC-4337 的入口点

部署在 `0x0000000071727De22E5E9d8BAf0edAc6f37da032`，即
[v0.7.0 的规范部署](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)。
[由 OpenZeppelin 审计](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
（受以太坊基金会委托，2024 年 1 月）：零关键级、零高危、五项中危，全部已解决——
而且被审计的那个 commit 就是被部署的版本。EntryPoint v0.7.0 在以太坊基金会的
[ERC-4337 漏洞赏金](https://docs.erc4337.io/community/bug-bounty)范围内
（最高 25 万美元）。

## 我们正在盯着的已知问题

### EntryPoint 的骚扰向量

2026 年 2 月，Trust Security 的安全研究员
[披露](https://erc4337.substack.com/p/improving-useroperation-execution)
了一个影响 v0.9 之前所有 EntryPoint 的骚扰与审查向量，其中包括我们在用的 v0.7。
攻击者如果在一个已签名的 UserOperation 被打包之前截获它，就可以把它放进自己控制的
调用帧里执行，并强制内层执行回滚——这笔操作失败了，但 gas 仍然被扣掉。以太坊基金会
为这个发现支付了 5 万美元赏金；基金会将其归类为审查/骚扰向量，而不是盗取资金的
向量，而且它从未被实际利用过。

它能做到的：浪费一笔手续费、拖延一笔交易。它做不到的：偷走资金，或者伪造签名。
Vela 的暴露面很窄，因为 UserOperation 是直接送到中继的，不经过公开的内存池，
所以几乎没有截获的机会——而最坏情况也被你早已同意的那笔费用所限定。修复只存在于
EntryPoint v0.9（2025 年 11 月）；v0.7 本身没法打补丁。我们预计会随着周边技术栈
——特别是 Safe 的 4337 模块系列——支持 v0.9 而迁移过去，到那时会在这里说明。

## 哪些没有审计

- **Vela 自己的合约。** 我们自己写的两个小合约，部署在 Gnosis 上：
  [通行密钥公钥索引](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  （一个只追加的注册表，帮你的设备找到你的公钥）和它的批量辅助合约。它们没有经过
  审计。从构造上说，它们不持有任何资金、没有所有者，也不能升级——它们是一层发现
  机制，不是一层授权机制。花钱的权限永远来自配置在你 Safe 内部的那把通行密钥。
  现实中最坏的故障是骚扰（有人抢占一条索引记录），这会让恢复变得不那么方便，
  但动不了钱。早期费用设计里的那个 gas 结算拆分合约，已经不在交易流程中。
- **Multicall3。** 它自己的 README [直说了](https://github.com/mds1/multicall3)：
  「本合约未经审计。」我们用它的方式，正是其作者描述为安全的那一种——批量只读调用，
  用于读取余额、代币元数据和价格。Vela 从不给它任何授权，它也从不持有资金。
  出 bug 的最坏情况是读到一个错误的数值。
- **CREATE2 部署器。**
  [Arachnid 确定性部署代理](https://github.com/Arachnid/deterministic-deployment-proxy)
  是生态标准的无状态部署器；它没有正式审计。如果某条链上它缺失或被改动过，
  我们的网络检查会直接判定失败。
- **Tempo 与 pathUSD。** Tempo 是我们十二条内置网络之一，它没有原生币；那里的 gas
  用 pathUSD 稳定币结算。截至 2026 年 8 月，Tempo 的核心协议和 pathUSD 都没有公开的
  安全审计，也没有漏洞赏金，而且一份独立的
  [DefiLlama 抵押品评估](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  （2026 年 4 月）将 pathUSD 评为高风险。这是链一级的风险，任何钱包都无法缓解：
  你放在 Tempo 上的资金，以及在那里的 gas 结算，都会继承它。请把 Tempo 当成这份
  名单上最新、也最未经检验的一条链，并据此控制你的余额规模。等审计发布，
  我们会更新这一节。
- **Vela 本身。** 我们的应用和后端服务没有做过第三方审计。这是这一页上最大的一条
  保留意见，我们把它写在站点顶部，老实的细节在
  [Vela 仍在内测](/blog/vela-is-in-alpha)。请从小额开始。去读代码。

## 自己去核实

上面每一个地址都是公开的规范部署，你可以对照官方注册表核实——
[safe-deployments](https://github.com/safe-global/safe-deployments)、
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
以及 [EntryPoint 的发布说明](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)：

| 合约 | 地址 |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| 通行密钥公钥索引（Gnosis） | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
