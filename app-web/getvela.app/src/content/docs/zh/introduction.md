---
title: 简介
description: "用六行说清 Vela 是什么，再列出大家最常带着来的问题——每个问题都直接链接到答案。"
source: f1879437a0b5
---

# Vela 文档

Vela 是一个**开源、可自行部署的以太坊及 EVM 网络钱包**，没有助记词。你的钱包是一个未经修改的
[Safe](/zh/docs/account-contract) 智能账户，你用**通行密钥**签名——可以在你的手机或电脑上、另一部
手机上，或一把硬件安全密钥上。

- **离开我们也能运行。**App 以及背后的每一项服务——中继、公钥索引、链数据和汇率——都采用 MIT 许可。照着[自托管指南](/zh/docs/self-hosting)自己编译、自己运行；限制条件也写在里面。
- **你的钥匙，最多七把。**创建钱包时选定；任意一把都能签名。Vela 从不持有它们，在你的钱包上也没有任何角色。
- **24 条网络，同一个地址。**另外还可以添加任何满足要求的 EVM 网络。
- **先看懂，再签名。**交易会被解码成平实的语言；解不开的会明确标出。
- **Alpha 阶段。**它可以正常使用、承载真实资金，但还很年轻：请先用小额。
  [这里的 alpha 指什么](/blog/vela-is-in-alpha)。

## 找答案

| 我想知道…… | 看这里 |
| --- | --- |
| 怎么把一切都换成自己运行的 | [自托管指南](/zh/docs/self-hosting) |
| 怎么运行自己的中继，手续费归谁 | [自托管指南 → 中继](/zh/docs/self-hosting#relay) · [网络与手续费 → 手续费](/zh/docs/networks-and-fees#fee) |
| getvela.app 下线了会怎样 | [自托管指南 → 没有 getvela.app 时](/zh/docs/self-hosting#if-getvela-app-disappears) |
| 一笔交易为什么是这个价 | [网络与手续费](/zh/docs/networks-and-fees) |
| 我的链支不支持，怎么添加 | [网络与手续费](/zh/docs/networks-and-fees) · [链设置](/zh/chain-setup) |
| Vela 有没有审计 | [审计与已知问题](/zh/docs/security-audits) |
| 怎么确认自己签的到底是什么 | [清晰签名](/zh/docs/clear-signing) · [Bybit 事件](/zh/docs/bybit-attack) |
| 该装哪个 App，要花多少钱 | [安装 Vela](/zh/docs/install) · [获取 Vela](/zh/get-started) |
| 怎么创建钱包，用哪些钥匙 | [创建钱包](/zh/docs/create-wallet) · [签名钥匙与安全密钥](/zh/docs/signers) |
| 手机丢了或删了通行密钥怎么办 | [恢复与登录](/zh/docs/recovery) |
| 之后还能不能加钥匙、换钥匙 | [签名钥匙与安全密钥](/zh/docs/signers) |
| 怎么用 Vela 连接 dApp | [安装 Vela → dApp](/zh/docs/install#dapps) |
| 我的钱包有哪些信息是公开的 | [创建钱包 → 哪些是公开的](/zh/docs/create-wallet#what-is-public) · [隐私政策](/privacy) |

## 深入阅读

- [我们为什么做 Vela](/zh/docs/why-vela)——来龙去脉，以及我们选择的取舍。
- [白皮书](/zh/docs/whitepaper)——架构，以及你到底在信任什么。
- [账户合约](/zh/docs/account-contract)——你的钱放在哪些合约里。

[博客](/blog)记录了 Vela 是怎样一步步做出来的。
