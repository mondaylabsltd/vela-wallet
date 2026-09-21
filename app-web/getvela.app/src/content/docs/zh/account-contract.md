---
title: 账户合约
description: "你的 Vela 钱包是一个未经修改的 Safe v1.4.1。通往你资金的路径上，没有一个合约是 Vela 写的——这里讲清楚具体是哪些合约、换来了什么、代价是什么。"
---

# 账户合约

你的钱包不是某个 App 的私有数据结构。它是一个 **Safe v1.4.1** 智能账户——管理着远比 Vela
将来可能经手的资金大得多的金库的，正是同一个合约——完全按 Safe 发布的样子部署，没有任何修改。

## 路径上没有我们的合约

每一个能碰到你资金的合约，都出自 Safe 或 ERC-4337 的作者之手：

| 合约 | 在你钱包里的作用 | 作者 |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)（SafeL2，经由代理合约） | 账户本身：所有者、阈值、执行 | Safe |
| [Safe 4337 模块 v0.3.0](https://github.com/safe-global/safe-modules/tree/main/modules/4337) | 让 EntryPoint 能操作这个 Safe；同时是它的 fallback handler | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) | 验证第一把钥匙的 P-256 签名 | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/main/modules/passkey) 及其创建的签名器 | 其余每把钥匙各对应一个小的签名器合约 | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | 执行你签好名的操作 | ERC-4337 作者 |

Vela 自己写的合约都不在这张表里：记录每个钱包的钥匙、让新设备能找到它的**公钥注册表**
（见[恢复](/zh/docs/recovery)）、与之配套的域名注册表，以及被它们取代的早期索引合约。它们
不持有资金，在你的 Safe 里也没有任何角色。

钱包代码仓库里一行 Solidity 都没有——一条命令就能验证：

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # 什么也不会输出
```

Vela 在你的账户上没有任何特权：没有管理员密钥，没有升级通道，也不能添加模块。只有你的钥匙
能改动你的 Safe。

## 为什么“未经修改”这个词最要紧

很多钱包说自己基于“一个 Safe”、Safe 的分叉，或受 Safe 启发的账户。分叉是顶着旧名声的新合约。

**审计覆盖的正是你在用的东西。**Safe 的审计报告针对的就是这些版本。分叉的审计针对的是分叉
之前的代码；改动的那部分，没有人审过。

**整个生态把你的账户当作 Safe，因为它就是 Safe。**区块浏览器能解析它，Safe 的工具能读取它、
为它构造交易。但要给这些交易*签名*，程序必须能向你的钥匙请求一个针对 `getvela.app`（你的
通行密钥所属的域名）的签名——所以从别的域名提供服务的 Safe 官方网页 App 无法替你签名。
哪些方式可以，见[自托管指南](/zh/docs/self-hosting#if-getvela-app-disappears)。

**攻击面是所有人都在盯着的那一个。**定制的账户合约，基本只有作者自己在看。而这个合约，所有
把钱放在 Safe 里的人都在看。

## 代价

采用标准不是没有成本的：

- **Gas。**你的签名在链上验证，交易还要经过 EntryPoint。钱包部署之后，从 Vela 钱包做一次
  简单转账大约消耗 14 万到 17 万 gas；普通账户转一笔 ETH 只要 2.1 万。gas 之外，中继还会收取
  手续费——见[网络与手续费](/zh/docs/networks-and-fees)。
- **账户需要部署。**你的地址用 `CREATE2` 在链上尚无任何东西时就算好了，所以马上就能收款；
  在每条网络上的第一笔转出交易会支付部署合约的费用。
- **不是每条链都符合条件。**通行密钥签名用 **RIP-7212** 预编译验证。没有它的网络，Vela
  直接拒绝启用，而不是退回到更慢的验证方式。
- **Safe 的风险就是你的风险。**信任一个被广泛使用的合约，依然是在信任一个合约。Vela 没有在
  你资金的路径上再加一个自己的合约让你去信任。

## 哪些审计过，哪些没有

Safe 的合约、它的 4337 模块和通行密钥模块，以及 EntryPoint v0.7，都有公开的第三方审计报告。
**Vela 自己的代码——各个 App、后端服务和注册表合约——没有经过第三方审计，目前也没有排期**；
这是项目有能力出资时的目标，而不是一个有日期的承诺。每个合约、它的审计报告，以及我们在跟踪的
问题，都列在[审计与已知问题](/zh/docs/security-audits)里。

## 自己去看

你的账户在链上。钱包部署后，在区块浏览器里打开你的地址：它是一个 Safe 代理合约，实现合约
是 Safe 官方部署的 SafeL2 v1.4.1，每条网络上都一样。

下一步：[审计与已知问题](/zh/docs/security-audits)。
