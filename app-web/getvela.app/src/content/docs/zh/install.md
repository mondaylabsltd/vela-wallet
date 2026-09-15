---
title: 安装 Vela
description: Vela 在浏览器里跑——不用安装，也不用经过应用商店。打开网页钱包，或者先看看你的设备需要满足什么条件才能用通行密钥。
---

# 安装 Vela

Vela **在你的浏览器里**运行——没有东西要下载，也不用经过任何应用商店。打开网页
钱包，一分钟之内就能创建或者恢复一个钱包。

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">打开网页钱包 →</a>

同一个钱包，出自同一份代码，也能在 iOS 和 Android 上跑。**原生手机应用即将上线**——
上线之后，你的通行密钥和钱包会原样跟过去，因为账户住在链上，而不是住在某一个应用里。

## 你的设备需要什么

Vela 用**通行密钥**（WebAuthn）签名，所以你需要一台支持它的设备和浏览器——基本上
这几年的设备都支持：

| 平台 | 通行密钥支持 | 由谁同步 |
| -------- | --------------- | --------- |
| iPhone / iPad / Mac | iOS/iPadOS 16+、较新的 Safari | iCloud 钥匙串 |
| Android | Android 9+、当前版本 Chrome | Google 密码管理器 |
| 桌面 | 当前版本的 Chrome、Edge、Safari、Firefox | 你所在平台的通行密钥服务 |

想让钱包跟着你换到新设备，就把平台的通行密钥同步开着（Apple 上是 iCloud 钥匙串，
Android/Chrome 上是 Google 密码管理器）。它是怎么运作的，见[恢复与登录](/zh/docs/recovery)。

## 只有这两个官方网址

Vela 是开源的，这正是重点——但这也意味着你得确认自己打开的是真货。官方地址只有：

- **getvela.app** —— 这个网站
- **wallet.getvela.app** —— 钱包

如果有人把你引去别的地方「安装 Vela」，先停下来，跟这两个地址对一遍。代码公开在
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet)。

下一步：[创建你的钱包](/zh/docs/create-wallet)。
