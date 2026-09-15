---
title: 安裝 Vela
description: Vela 在瀏覽器裡跑——不用安裝，也不用經過應用程式商店。打開網頁錢包，或是先看看你的裝置要符合什麼條件才能用密碼金鑰。
---

# 安裝 Vela

Vela **在你的瀏覽器裡**運作——沒有東西要下載，也不用經過任何應用程式商店。打開
網頁錢包，一分鐘之內就能建立或復原一個錢包。

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">打開網頁錢包 →</a>

同一個錢包，出自同一份程式碼，也能在 iOS 和 Android 上跑。**原生行動應用即將推出**
——上線之後，你的密碼金鑰和錢包會原封不動跟過去，因為帳戶住在鏈上，而不是住在
某一個應用程式裡。

## 你的裝置需要什麼

Vela 用**密碼金鑰**（WebAuthn）簽署，所以你需要一台支援它的裝置和瀏覽器——基本上
這幾年的裝置都支援：

| 平台 | 密碼金鑰支援 | 由誰同步 |
| -------- | --------------- | --------- |
| iPhone / iPad / Mac | iOS/iPadOS 16+、較新的 Safari | iCloud 鑰匙圈 |
| Android | Android 9+、目前版本的 Chrome | Google 密碼管理工具 |
| 桌面 | 目前版本的 Chrome、Edge、Safari、Firefox | 你所在平台的密碼金鑰服務 |

想讓錢包跟著你換到新裝置，就把平台的密碼金鑰同步開著（Apple 上是 iCloud 鑰匙圈，
Android/Chrome 上是 Google 密碼管理工具）。它怎麼運作，見[復原與登入](/zh-TW/docs/recovery)。

## 只有這兩個官方網址

Vela 是開放原始碼的，這正是重點——但這也表示你得確認自己打開的是真貨。官方網址
只有：

- **getvela.app** —— 這個網站
- **wallet.getvela.app** —— 錢包

如果有人把你帶去別的地方「安裝 Vela」，先停下來，跟這兩個網址對一遍。程式碼公開在
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet)。

下一步：[建立你的錢包](/zh-TW/docs/create-wallet)。
