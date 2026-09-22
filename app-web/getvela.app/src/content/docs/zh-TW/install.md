---
title: 安裝 Vela
description: "執行 Vela 的每一種方式——網頁、瀏覽器擴充功能、桌面版和手機——各要多少錢、各能做什麼，以及你的裝置需要什麼。"
source: f88fdfac1001
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# 安裝 Vela

同一個錢包可以在好幾個地方執行，打開的都是同一個位址、用的都是同一組金鑰。依需要挑選，也可以
同時用好幾個。下載都在[取得 Vela](/zh-TW/get-started) 頁面。

| | 是什麼 | 費用 | 狀態 |
| --- | --- | --- | --- |
| **網頁版** | 在任何較新的瀏覽器裡開啟 [wallet.getvela.app](https://wallet.getvela.app/) | 免費 | 已上線 |
| **瀏覽器擴充功能** | 放在工具列上的錢包，可以連接 dApp | 免費 | 下載後手動載入；尚未上架 Chrome 線上應用程式商店 |
| **桌面版** | macOS、Windows、Linux 原生應用程式 | 免費 | 從「取得 Vela」頁面或 GitHub 下載 |
| **iPhone、Android** | 原生應用程式 | 在商店一次性買斷 | 還沒上架；可以從原始碼自行建置 |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">開啟網頁錢包 →</a>

## 網頁版

什麼都不用裝。打開 [wallet.getvela.app](https://wallet.getvela.app/)，建立錢包或登入就好。帳戶清單
存在這個瀏覽器裡；換一台裝置，用你任何一把金鑰重新登入即可。

## 瀏覽器擴充功能

適用於 Chromium 系列瀏覽器：Chrome、Edge 和 Brave（Chrome 116 以上）。它把錢包放進工具列，讓 dApp
可以直接連接。在上架 Chrome 線上應用程式商店之前：

1. 從[取得 Vela](/zh-TW/get-started) 下載擴充功能，解壓縮到一個你會一直保留的資料夾——瀏覽器會從那裡執行它。
2. 開啟 `chrome://extensions`，啟用「開發人員模式」。
3. 點選「載入未封裝項目」，選擇那個資料夾。

它就是同一個錢包：擴充功能和網頁錢包用的是同樣的 `getvela.app` 密碼金鑰，所以同樣的金鑰會打開同一個位址。

## 桌面版

原生應用程式，而不是包在視窗裡的網頁：**Windows** 10 和 11（x64 與 ARM）、**macOS** 11 以上，以及
**Linux**（.deb、.rpm 或 Flatpak，x64 與 ARM）。

- **Windows** 會提示「Windows 已保護您的電腦」，因為安裝程式還沒有程式碼簽章。點「其他資訊」，再點「仍要執行」。
- **macOS** 版本要另外經過 Apple 簽署和公證，所以可能比其他平台晚推出。如果 Mac 按鈕顯示「稍後提供」，
  最新一個通過公證的 Mac 版本可以在 GitHub 的發布頁面找到。
- **Linux**：要使用 USB 安全金鑰，系統必須允許應用程式存取它——.deb 和 .rpm 套件會自動幫你裝好這條規則。

在 macOS 和 Windows 上，桌面版內建一個用來開 dApp 的瀏覽器。每個套件的檢查碼都在
[GitHub 發布頁面](https://github.com/mondaylabsltd/vela-wallet/releases)上。

## iPhone 和 Android

原生應用程式，支援 iOS 17.4 以上和 Android 10 以上。它們會以一次性買斷的方式在 App Store 和
Google Play 販售，目前**還沒有上架**。程式碼是公開的，你可以免費自行建置——只有一個差別：你自己簽署的
版本不能用手機本身的密碼金鑰替 getvela.app 錢包簽署，不過用另一支手機掃描和用 USB 安全金鑰都可以。
請看[自行建置應用程式](/zh-TW/docs/self-hosting#web-app)。

## 用 Vela 連接 dApp

<span id="dapps"></span>

dApp 連接 Vela 的方式，和連接任何瀏覽器錢包一樣（EIP-1193 和 EIP-6963）：

- 在電腦的瀏覽器裡，透過 **Vela 瀏覽器擴充功能**；
- 在**桌面版**（macOS、Windows）、**iPhone 應用程式**和 **Android 應用程式**裡，透過它們內建的瀏覽器。

wallet.getvela.app 上的網頁錢包不會連接 dApp，也不支援 WalletConnect。dApp 送來的每一個請求，都會在
你簽署之前解碼並顯示給你看——請看[清晰簽署](/zh-TW/docs/clear-signing)。

## 你的裝置需要什麼

Vela 用**密碼金鑰**簽署，這幾年的裝置幾乎都支援：

| 裝置 | 支援情形 |
| --- | --- |
| iPhone、iPad、Mac | iOS / iPadOS 16 以上，macOS 上較新的 Safari 或 Chrome |
| Android | 有 Google Play 服務的較新 Android，或一把 USB 安全金鑰 |
| Windows | Chrome 或 Edge 搭配 Windows Hello，或一把安全金鑰 |
| Linux | 一把安全金鑰，或身邊的一支手機（掃描 QR 碼） |

如果你的裝置本身無法存放密碼金鑰，可以改用另一支手機或硬體安全金鑰。
[簽署金鑰與安全金鑰](/zh-TW/docs/signers)列出了各個應用程式支援哪些種類的金鑰。

## 僅有的官方網址

- **getvela.app**——這個網站，以及下載
- **wallet.getvela.app**——網頁錢包
- **github.com/mondaylabsltd**——程式碼和發布套件

<Callout type="warning" title="安裝前先核對">
如果有任何東西把你帶到別的地方去「安裝 Vela」或「驗證錢包」，請先停下來。Vela 從來不會要你的助記詞——它根本沒有助記詞。
</Callout>

下一步：[建立你的錢包](/zh-TW/docs/create-wallet)。
