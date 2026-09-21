---
title: 常见问题
description: "关于托管、钥匙、恢复、网络、费用、Vela 能看到什么、开源，以及 Vela 不在了会怎样的简短回答。"
source: 0762e55bf54d
---

# 常见问题

## Vela 是自托管的吗？

是的。你的钱包是一个 Safe 智能账户，只受你的钥匙控制，而钥匙留在你的设备、你的密码管理器或你的
安全密钥里。Vela 不持有任何钥匙，在账户上也没有任何角色，所以它自己无法动用、冻结或找回你的资金。但
请你的钥匙签名的软件是它写的——见[威胁模型](/zh/docs/whitepaper)。

## 真的没有助记词吗？

真的没有。你的钥匙是通行密钥，而通行密钥没有可以抄下来或输入的秘密。见[通行密钥如何工作](/zh/docs/passkeys)。

## 创建钱包需要什么？

一台支持通行密钥的设备（带面容、指纹或 Windows Hello 的较新手机或电脑），或者两把硬件安全密钥。不需要邮箱、不需要注册账户、也不需要预先充值。创建时最多可以加七把钥匙，之后不能再加。见
[创建钱包](/zh/docs/create-wallet)。

## 手机丢了怎么办？

在新设备上用任意另一把钥匙登录：通过 iCloud 钥匙串或 Google 密码管理器同步过来的同一个通行密钥、
另一部手机，或你的安全密钥。如果那部手机上存着你唯一的钥匙、而且没有同步，钱包就无法恢复。见
[恢复与登录](/zh/docs/recovery)。

## 支持哪些网络和代币？

24 条内置 EVM 网络，包括 Ethereum、Base、Arbitrum、Optimism、Polygon、BNB Chain、Gnosis 和
Avalanche，另外还可以添加任何满足要求的 EVM 网络。支持原生币和 ERC-20 代币。每条网络上都是同一个
地址。见[网络与手续费](/zh/docs/networks-and-fees)。

## 要花多少钱？

- **App：**网页钱包、浏览器扩展和桌面版免费。iOS 和 Android App 将在商店以一次性买断的方式销售；
  你也可以免费从源码编译任何一个 App。
- **每笔交易：**从你的钱包付给提交它的中继一笔手续费。它包含 gas 和中继的利润，常常是这笔交易
  链上成本的十倍甚至更多，最低约 0.01 美元。确切金额显示在确认页上，并写在你签名的内容里。没有押金，
  也没有订阅费。[手续费怎么算](/zh/docs/networks-and-fees)。
- **没有代币。**Vela 没有代币，也不打算发。

## 能用 Vela 连接 dApp 吗？

能，通过 Vela 浏览器扩展（Chrome、Edge、Brave），以及桌面版（macOS、Windows）、iOS 和 Android App
内置的浏览器。wallet.getvela.app 上的网页钱包不连接 dApp。见[安装](/zh/docs/install#dapps)。

## Vela 能看到什么、能做什么？

Vela 读不到你的钥匙，自己也动不了你的资金。它的服务能看到你的 IP 地址，以及 App 向它们询问的内容：
登记新钱包时，索引能看到你的公钥和钱包名称，也能看到你查询名字的地址；中继能看到你的地址、你提交的
操作，以及你的 App 使用的 RPC 节点；链数据服务能看到你的 App 查询了哪些代币和合约。哪些内容会公开在链上，见[创建钱包](/zh/docs/create-wallet#what-is-public)。完整、权威的说明
以[隐私政策](/privacy)为准。

## Vela 开源吗？

钱包 App、中继和汇率服务都以 MIT 许可发布在 [GitHub](https://github.com/orgs/mondaylabsltd/repositories)
上；链数据目录也是 MIT。公钥索引代码公开，但暂时还没有许可证文件。每一项服务你都可以自己运行——
见[自托管指南](/zh/docs/self-hosting)。

## Vela 经过审计吗？

存放你资金的合约——Safe 及其模块、ERC-4337 EntryPoint——都经过审计。Vela 自己的代码没有，目前也
没有排期。见[审计与已知问题](/zh/docs/security-audits)。

## 如果 Vela 关门了呢？

你的资金留在链上你自己的 Safe 里。对于已有的钱包，Vela 浏览器扩展和你自己编译的 App 不依赖
getvela.app 也能继续用，每一项服务都是开源的、别人也能运行（中继需要改一处代码，才能不再从 Vela 的
服务器读取链数据）。[自托管指南](/zh/docs/self-hosting#if-getvela-app-disappears)
列出了每条路及其限制。

## 我的问题这里没有。

在 [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues) 上提 issue，或者通过
[X](https://x.com/realvelawallet)、[Telegram](https://t.me/velawallet) 联系我们。
