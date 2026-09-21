---
title: 安裝 Vela
description: "運行 Vela 的每一種方式——網頁版、瀏覽器擴充功能、桌面版和手機——各自的費用、各自能做甚麼，以及你的裝置需要甚麼。"
source: f88fdfac1001
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# 安裝 Vela

同一個錢包可以在幾個地方運行，打開的都是同一個地址，用的都是同一組鑰匙。按你的需要選擇，也可以
同時使用多個。下載都在[取得 Vela](/zh-HK/get-started) 頁面。

| | 是甚麼 | 費用 | 狀態 |
| --- | --- | --- | --- |
| **網頁版** | 在任何較新瀏覽器中打開 [wallet.getvela.app](https://wallet.getvela.app/) | 免費 | 已推出 |
| **瀏覽器擴充功能** | 放在工具列中的錢包，可以連接 dApp | 免費 | 下載後手動載入；尚未上架 Chrome 線上應用程式商店 |
| **桌面版** | macOS、Windows 及 Linux 原生 App | 免費 | 從「取得 Vela」頁面或 GitHub 下載 |
| **iPhone、Android** | 原生 App | 在應用程式商店一次性購買 | 尚未上架；可以從原始碼自行編譯 |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">打開網頁版錢包 →</a>

## 網頁版

不用安裝任何東西。打開 [wallet.getvela.app](https://wallet.getvela.app/)，建立錢包或登入，錢包就在
那裏。你的帳戶列表儲存在這個瀏覽器中；換到另一部裝置，只要用你其中一把鑰匙重新登入即可。

## 瀏覽器擴充功能

適用於 Chromium 瀏覽器：Chrome、Edge 和 Brave（Chrome 116 或以上）。它把錢包放進工具列，讓 dApp
可以直接連接。在上架 Chrome 線上應用程式商店之前：

1. 從[取得 Vela](/zh-HK/get-started) 下載擴充功能，解壓縮到一個你會保留的資料夾——瀏覽器會從那裏運行它。
2. 打開 `chrome://extensions`，開啟**開發人員模式**。
3. 按**載入未封裝項目**，然後選擇該資料夾。

它就是同一個錢包：擴充功能和網頁版錢包使用同一批 `getvela.app` 通行密鑰，所以同樣的鑰匙會打開同一個地址。

## 桌面版

這是原生 App，不是套在視窗裏的網頁：**Windows** 10 及 11（x64 和 ARM）、**macOS** 11 或以上，以及
**Linux**（.deb、.rpm 或 Flatpak，x64 和 ARM）。

- **Windows** 會提示「已保護您的電腦」，因為安裝程式尚未有程式碼簽署。請按**其他資訊**，再按**仍要執行**。
- **macOS** 版本要另外經過 Apple 簽署和公證，所以可能比其他平台遲推出。如果 Mac 按鈕顯示「稍後提供」，
  最新一個經過公證的 Mac 版本可以在 GitHub 發佈頁面找到。
- **Linux**：要使用 USB 安全密鑰，你的系統必須允許 App 存取它——.deb 和 .rpm 套件會自動為你安裝這條規則。

在 macOS 和 Windows 上，桌面版內置了用來開啟 dApp 的瀏覽器。每個套件的校驗碼都在
[GitHub 發佈頁面](https://github.com/mondaylabsltd/vela-wallet/releases)上。

## iPhone 及 Android

原生 App，支援 iOS 17.4 或以上及 Android 10 或以上。它們將以一次性購買的方式在 App Store 和
Google Play 發售，目前**尚未上架**。程式碼是開源的，所以你可以免費自行編譯——只有一點不同：你自己
簽署的版本不能用手機本身的通行密鑰為 getvela.app 錢包簽署，但用另一部手機掃描 QR 碼和用 USB 安全密鑰
都可以。見[自行編譯 App](/zh-HK/docs/self-hosting#web-app)。

## 用 Vela 連接 dApp

<span id="dapps"></span>

dApp 連接 Vela 的方式，和連接任何瀏覽器錢包一樣（EIP-1193 和 EIP-6963）：

- 在電腦的瀏覽器中，透過 **Vela 瀏覽器擴充功能**；
- 在**桌面版**（macOS、Windows）、**iPhone App** 和 **Android App** 中，透過它們的內置瀏覽器。

wallet.getvela.app 上的網頁版錢包不會連接 dApp，也不支援 WalletConnect。dApp 發出的每一個請求，都會在
你簽署前被解碼並顯示給你——見[清晰簽署](/zh-HK/docs/clear-signing)。

## 你的裝置需要甚麼

Vela 用**通行密鑰**簽署，近幾年的裝置幾乎都支援：

| 裝置 | 支援情況 |
| --- | --- |
| iPhone、iPad、Mac | iOS / iPadOS 16 或以上；macOS 上較新版本的 Safari 或 Chrome |
| Android | 有 Google Play 服務的較新版本 Android，或一把 USB 安全密鑰 |
| Windows | Windows Hello 配合 Chrome 或 Edge，或一把安全密鑰 |
| Linux | 一把安全密鑰，或身邊的一部手機（掃描 QR 碼） |

如果你的裝置本身無法儲存通行密鑰，可以用另一部手機或硬件安全密鑰。
[簽署鑰匙與安全密鑰](/zh-HK/docs/signers)列出了每個 App 支援哪些種類的鑰匙。

## 僅有的官方網址

- **getvela.app**——本網站，以及各個下載
- **wallet.getvela.app**——網頁版錢包
- **github.com/mondaylabsltd**——程式碼和發佈套件

<Callout type="warning" title="安裝前請先核對">
如果有任何東西把你帶到別處去「安裝 Vela」或「驗證你的錢包」，請停下來。Vela 從不會要求你提供助記詞——它根本沒有助記詞。
</Callout>

下一步：[建立錢包](/zh-HK/docs/create-wallet)。
