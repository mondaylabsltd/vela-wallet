---
title: 安裝 Vela
description: Vela 喺瀏覽器行——唔使安裝，亦唔使經應用程式商店。打開網頁錢包，或者先睇吓你部機要符合啲乜先用到通行密鑰。
---

# 安裝 Vela

Vela **喺你個瀏覽器入面**行——冇嘢要下載，亦唔使經任何應用程式商店。打開網頁錢包，
一分鐘之內就開得到或者復原到一個錢包。

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">打開網頁錢包 →</a>

同一個錢包，出自同一份代碼，喺 iOS 同 Android 上面一樣行得。**原生手機應用即將推出**
——上線之後，你把通行密鑰同個錢包會原封不動跟過去，因為個帳戶住喺鏈上，
唔係住喺某一個應用入面。

## 你部機需要啲乜

Vela 用**通行密鑰**（WebAuthn）簽名，所以你需要一部支援佢嘅機同瀏覽器——基本上
呢幾年嘅機都支援：

| 平台 | 通行密鑰支援 | 由邊個同步 |
| -------- | --------------- | --------- |
| iPhone / iPad / Mac | iOS/iPadOS 16+、較新嘅 Safari | iCloud 鑰匙串 |
| Android | Android 9+、現行版本 Chrome | Google 密碼管理工具 |
| 桌面 | 現行版本嘅 Chrome、Edge、Safari、Firefox | 你所在平台嘅通行密鑰服務 |

想個錢包跟你換去新機，就開住平台嘅通行密鑰同步（Apple 係 iCloud 鑰匙串，
Android/Chrome 係 Google 密碼管理工具）。佢點運作，睇[復原同登入](/zh-HK/docs/recovery)。

## 淨係得呢兩個官方網址

Vela 係開源嘅，呢樣正正係重點——但同時亦即係話你要肯定自己打開嘅係真嘢。官方地址
淨係得：

- **getvela.app** —— 呢個網站
- **wallet.getvela.app** —— 錢包

如果有人叫你去第度「安裝 Vela」，先停低，同呢兩個地址對一次。代碼公開喺
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet)。

下一步：[開你個錢包](/zh-HK/docs/create-wallet)。
