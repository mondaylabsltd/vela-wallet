---
title: 安装 Vela
description: "运行 Vela 的每一种方式——网页、浏览器扩展、桌面版和手机——各自多少钱、能做什么，以及你的设备需要什么。"
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# 安装 Vela

同一个钱包可以在好几个地方运行，打开的都是同一个地址、用的都是同一组钥匙。按需要选择，也可以
同时用好几个。下载都在[获取 Vela](/zh/get-started) 页面。

| | 是什么 | 费用 | 状态 |
| --- | --- | --- | --- |
| **网页版** | 任意较新浏览器里的 [wallet.getvela.app](https://wallet.getvela.app/) | 免费 | 已上线 |
| **浏览器扩展** | 放在工具栏里的钱包，可连接 dApp | 免费 | 下载后手动加载；尚未上架 Chrome 应用商店 |
| **桌面版** | macOS、Windows、Linux 原生 App | 免费 | 从“获取 Vela”页面或 GitHub 下载 |
| **iPhone、Android** | 原生 App | 在商店一次性买断 | 还没上架；可以从源码编译 |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">打开网页钱包 →</a>

## 网页版

什么都不用装。打开 [wallet.getvela.app](https://wallet.getvela.app/)，创建钱包或登录即可。账户列表
保存在这个浏览器里；换一台设备，用你的任意一把钥匙重新登录就行。

## 浏览器扩展

适用于 Chromium 系浏览器：Chrome、Edge 和 Brave（Chrome 116 及以上）。它把钱包放进工具栏，让 dApp
可以直接连接。在上架 Chrome 应用商店之前：

1. 从[获取 Vela](/zh/get-started) 下载扩展，解压到一个你会一直保留的文件夹——浏览器从那里运行它。
2. 打开 `chrome://extensions`，开启**开发者模式**。
3. 点击**加载已解压的扩展程序**，选择那个文件夹。

它就是同一个钱包：扩展和网页钱包使用同样的 `getvela.app` 通行密钥，所以同样的钥匙打开同一个地址。

## 桌面版

原生 App，而不是套在窗口里的网页：**Windows** 10 和 11（x64 与 ARM）、**macOS** 11 及以上，以及
**Linux**（.deb、.rpm 或 Flatpak，x64 与 ARM）。

- **Windows** 会提示“已保护你的电脑”，因为安装包暂未做代码签名。点**更多信息**，再点**仍要运行**。
- **macOS** 版本要单独经过 Apple 签名和公证，所以可能比其他平台晚。如果 Mac 按钮显示“稍后提供”，
  最近一个经过公证的 Mac 版本可以在 GitHub 发布页找到。
- **Linux**：要使用 USB 安全密钥，系统需要允许 App 访问它——.deb 和 .rpm 安装包会自动装好这条规则。

在 macOS 和 Windows 上，桌面版内置了用于 dApp 的浏览器。每个安装包的校验值都在
[GitHub 发布页](https://github.com/mondaylabsltd/vela-wallet/releases)上。

## iPhone 和 Android

原生 App，支持 iOS 17.4 及以上、Android 10 及以上。它们将在 App Store 和 Google Play 以一次性买断
的方式销售，目前**还没有上架**。代码是开源的，你可以免费自己编译——只有一个区别：你自己签名的版本
不能用手机自带的通行密钥给 getvela.app 钱包签名，但用另一部手机扫码和用 USB 安全密钥都可以。见
[自己编译 App](/zh/docs/self-hosting#web-app)。

## 用 Vela 连接 dApp

<span id="dapps"></span>

dApp 连接 Vela 的方式，和连接任何浏览器钱包一样（EIP-1193 和 EIP-6963）：

- 在电脑浏览器里，通过 **Vela 浏览器扩展**；
- 在**桌面版**（macOS、Windows）、**iPhone App** 和 **Android App** 里，通过它们内置的浏览器。

wallet.getvela.app 上的网页钱包不连接 dApp，也不支持 WalletConnect。dApp 发来的每一个请求，都会在
你签名前被解码并展示给你——见[清晰签名](/zh/docs/clear-signing)。

## 你的设备需要什么

Vela 用**通行密钥**签名，近几年的设备几乎都支持：

| 设备 | 支持情况 |
| --- | --- |
| iPhone、iPad、Mac | iOS / iPadOS 16 及以上，macOS 上较新的 Safari 或 Chrome |
| Android | 带 Google Play 服务的较新 Android，或一把 USB 安全密钥 |
| Windows | Chrome 或 Edge 配合 Windows Hello，或一把安全密钥 |
| Linux | 一把安全密钥，或身边的手机（扫二维码） |

如果你的设备本身无法保存通行密钥，可以用另一部手机或硬件安全密钥。
[签名钥匙与安全密钥](/zh/docs/signers)列出了各个 App 支持哪些种类的钥匙。

## 仅有的官方地址

- **getvela.app**——本站，以及下载
- **wallet.getvela.app**——网页钱包
- **github.com/mondaylabsltd**——代码和发布包

<Callout type="warning" title="安装前先核对">
如果有任何东西把你引到别处去“安装 Vela”或“验证钱包”，请停下。Vela 从不索要助记词——它根本没有助记词。
</Callout>

下一步：[创建钱包](/zh/docs/create-wallet)。
