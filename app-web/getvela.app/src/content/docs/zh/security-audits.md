---
title: 审计与已知问题
description: "Vela 依赖的每一个合约、谁审计了哪个版本、审计的版本是否就是部署的版本、我们在关注的未解决问题，以及哪些东西根本没有审计过。"
source: d0bb95c016da
---

“经过审计”是针对特定版本的特定代码的说法，所以这一页列出具体的报告、提交和部署地址——也列出
**没有**审计过的东西，这同样重要。

最近一次核对：2026 年 9 月 22 日。如果你发现错误，告诉我们，我们会改。

## 资金路径

每一个能碰到你资金的合约，都是第三方代码的官方部署，并且都有公开的审查报告。

### Safe v1.4.1——账户本身

你的钱包是一个 [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) 代理合约，
使用 SafeL2 实现合约和 SafeProxyFactory。批量交易经由 MultiSend。

[Ackee Blockchain 审计了 Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
（终版报告 2023 年 3 月 16 日，修复复核 3 月 28 日）：11 项发现，没有严重或高危；两项中危被确认
但未修改。审计范围是 SafeL2、SafeProxyFactory、CompatibilityFallbackHandler、MultiSendCallOnly 和
SignMessageLib。v1.4.1 与 v1.4.0 只有一行功能性差异，是模块初始化中的 ERC-4337 兼容性修复
（[PR #572](https://github.com/safe-global/safe-smart-account/pull/572)）；Safe 征询了 Ackee，
结论是无需重新审计。MultiSend 的逻辑自 v1.3.0 起未变，v1.3.0 由
[G0 Group 审计](https://github.com/safe-global/safe-smart-account/tree/main/docs)。所有地址都与
[safe-deployments](https://github.com/safe-global/safe-deployments) 一致。核心合约在
[Safe 基金会漏洞赏金计划](https://docs.safefoundation.org/security/bug-bounty)范围内，最高一档
赏金可达 100 万美元。

2025 年的 Bybit 事件不是合约层面的问题：攻击者篡改了 Safe 网页界面加载的 JavaScript，Safe 的
[调查声明](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)确认合约没有漏洞。
[我们关于它的那一页](/zh/docs/bybit-attack)解释了为什么同一类攻击关系到每一个钱包界面，包括我们的。

### Safe4337Module v0.3.0——ERC-4337 适配器

部署在 `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`（Sourcify 完全匹配），同时也被设为你 Safe 的
fallback handler。它接受过三次审查——
[报告见此](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)：

- **Ackee Blockchain**，终版报告 2024 年 3 月：一项警告（使用编译器优化器）已确认，没有更高级别的
  未决问题。
- **Certora**，2026 年 8 月：一项**中危**发现，已确认但在 v0.3.0 中**未修复**——*授权变更不会让
  同一批次中已通过验证的后续 UserOperation 失效*。见下文“已知问题”。
- **Nethermind**，2026 年 8 月：没有发现。

在钱包部署时启用该模块的 SafeModuleSetup v0.3.0（`0x2dd6…5b47`）在 Certora 和 Nethermind 的审查
范围内。

这个模块有过一次公开披露的问题：v0.1.0 没有对 `initCode` 和 `paymasterAndData` 签名，存在消耗
gas 的攻击面，[已在 v0.2.0 修复](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)；
据 Safe 所说，v0.1.0 没有在测试网以外使用过。Vela 使用的是 v0.3.0，配合 EntryPoint v0.7 和
Safe 1.4.1，正是该模块发布说明中的配置。

### Safe 通行密钥模块 v0.2.1——签名器

你的第一把钥匙由 **SafeWebAuthnSharedSigner** 验证，地址 `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`。
“共享”指的是合约部署是共享的，就像 Safe 的实现合约一样；你的钥匙并不共享。每个 Safe 都把自己的
P-256 公钥存在自己的存储里。

其余每把钥匙各有一个签名器合约，由 **SafeWebAuthnSignerFactory**（`0x1d31F259eE307358a26dFb23EB365939E8641195`）
创建，作为指向 **SafeWebAuthnSigner 实现合约**（`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`）的代理。

覆盖这些合约 v0.2.1 版本的审查
（[报告](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)）：

- 一次 [Hats Finance 审计竞赛](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  （2024 年 6 月至 7 月）：没有高危或中危发现；三项低危，全部修复。
- **Certora** 对发布提交的审查：没有新发现。（更早的 v0.2.0 审计注明共享签名器当时尚未审计——
  它是在那次审计之后才加入的。）
- **Nethermind**，2026 年 8 月：没有发现。

发布以来没有披露过合约层面的漏洞，通行密钥合约也在 Safe 基金会赏金计划范围内。

通行密钥签名由链上的 **EIP-7951 / RIP-7212** 预编译验证，没有备用验证器。启用一条网络之前，App 会用一个真实
签名检查预编译。两点保留：最初的 RIP-7212 规范有一些边界情况的缺陷，已由
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) 修正（只影响本就该验证失败的输入，不影响格式
正确的 WebAuthn 签名）；另外，一次探测无法发现某条链的实现可能出现偏差的所有方式。

### EntryPoint v0.7——执行你的操作

部署在 `0x0000000071727De22E5E9d8BAf0edAc6f37da032`，即
[官方 v0.7.0 版本](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)。
由 [OpenZeppelin 为以太坊基金会审计](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
（2024 年 1 月）：没有严重或高危，五项中危，全部 24 项发现均已解决；修复复核的提交与发布版本一致。
它在以太坊基金会 [ERC-4337 漏洞赏金计划](https://docs.erc4337.io/community/bug-bounty)范围内（最高
25 万美元）。

## 我们在关注的已知问题

### 同一批次内的授权变更（Safe4337Module，Certora M-01）

EntryPoint 会先验证一个批次里的所有操作，再逐个执行。所以如果某个操作删除了一位所有者，同一批次中
排在后面、由这位所有者签名的操作依然能通过验证并执行。Safe 确认了这一点，没有修改 v0.3.0。

Vela 的 App 从不构造更换所有者的操作，dApp 要是请求一个，也会被直接拒绝，
所以 Vela 自己不会触发这个问题。但对于通过其他 Safe 工具删除一把被盗钥匙的人，
这件事仍然重要：他们不能指望那把钥匙在同一批次里就被切断。

### 已签名操作被截获（v0.9 之前的 EntryPoint）

2026 年 2 月，研究人员[披露](https://erc4337.substack.com/p/improving-useroperation-execution)了
一种影响 v0.9 之前所有 EntryPoint（包括 v0.7）的干扰与审查手法。有人在操作上链前拿到签好名的操作，
就能在自己控制的调用中执行它，并迫使内部执行回滚：操作失败，需要重新签名。（在 Vela 的带内手续费
模式下，手续费转账会随之回滚，所以 gas 由中继而不是你承担。）它影响的是调用了带重入
保护的合约、或能被临时状态弄得回滚的操作；简单转账不受影响。如果反复针对提现流程下手，可能让资金
在一段时间内无法取用。它无法伪造签名，也无法改变资金去向。

Vela 的中继直接提交操作，而不是经过共享内存池；但待上链的 `handleOps` 交易在公共内存池里仍然可见，
所以这只是缩小了暴露面，并没有消除。修复只存在于 EntryPoint v0.9（2025 年 11 月）；v0.7 无法打补丁。
迁移取决于 Safe 的 4337 模块何时支持 v0.9，届时本页会说明。

### Vela 自身防护的缺口

这些不是合约层面的发现，而是钱包对你的保护不如你可能以为的那么多的地方。每一项都已记录待修：

- **授权闸门只拦“无限”金额**（2^200 及以上；Permit2 为 2^152）。金额很大但有限的授权、签名式授权，
  或 NFT 的 `setApprovalForAll`，只会显示警示，不会被拦下。
- **独立签名页还没有接入**任何 App。
- **网站在通行密钥所属的同一域名上加载了第三方统计脚本。**网站通过 Permissions-Policy 响应头禁止自己
  的页面使用通行密钥，并且不在保存着密钥的那个页面上加载这个脚本。

## 没有审计过的

- **Vela 自己的合约。**[公钥注册表](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`（Gnosis；以太坊和 Base 上地址相同）、最初的注册表部署
  `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf`（它的地址是每次登记签名域的一部分），以及被它们取代的早期索引
  （`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`，仅作历史记录读取）。它们没有经过审计。它们不持有
  资金、没有所有者、不能升级；它们是发现层，而不是授权层。花钱的权力只来自配置在你 Safe 里的钥匙。
  现实中最坏的情况，是钱包在新设备上更难被找到，而不是资金被转走。
- **Multicall3。**它的 README [直说](https://github.com/mds1/multicall3)“This contract is unaudited.”
  Vela 只用它做批量读取——余额、代币信息、价格报价——从不涉及授权或资金。
- **确定性部署器**（Arachnid 的 CREATE2 代理和 Safe 的 singleton factory）——生态标准、无状态，没有
  正式审计。它们缺失时，Vela 的网络检查会直接判定不通过；检查的是该地址上有没有代码，而不是逐字节比对。
- **Tempo。**24 条内置网络之一，没有原生币；Vela 在那里用 pathUSD 稳定币支付 gas。截至 2026 年 9 月，
  Tempo 的[安全政策](https://github.com/tempoxyz/.github/blob/main/SECURITY.md)写明协议仍在审计中，
  也没有启用漏洞赏金。你在 Tempo 上持有的资金、在那里支付的 gas，都带有这一链层面的风险；请把它当作
  列表里最新、最未经考验的一条链。
- **Vela 本身。**各个 App、后端服务以及上面那些合约，都没有经过第三方审计，目前也没有排期。这是本页
  最大的保留。详情见 [Vela is in alpha](/blog/vela-is-in-alpha)。请先用小额，并去读代码。

## 自己核对

下面每一个地址都是公开的官方部署。可以对照
[safe-deployments](https://github.com/safe-global/safe-deployments)、
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
和 [EntryPoint 发布页](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)核实：

| 合约                                      | 地址                                         |
| ----------------------------------------- | -------------------------------------------- |
| SafeL2 实现合约 v1.4.1                    | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                   | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                          | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹     | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                    | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                     | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1           | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1          | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| SafeWebAuthnSigner 实现合约 v0.2.1        | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                           | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| 公钥注册表（Vela，未审计）                | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ 添加网络时会检查它；你的 Safe 实际使用 4337 模块作为 fallback handler。
