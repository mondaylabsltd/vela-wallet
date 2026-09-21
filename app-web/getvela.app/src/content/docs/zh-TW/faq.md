---
title: 常見問題
description: "關於保管、金鑰、復原、網路、費用、Vela 看得到什麼、開源，以及 Vela 不在了會怎樣的簡短回答。"
source: 0762e55bf54d
---

# 常見問題

## Vela 是自我保管的錢包嗎？

是的。你的錢包是一個 Safe 智慧帳戶，只由你的金鑰控制，而金鑰留在你的裝置、你的密碼管理工具或你的
安全金鑰裡。Vela 不持有任何金鑰，在帳戶上也沒有任何角色，所以它自己無法動用、凍結或找回你的資金。不過，
請你的金鑰簽署的軟體是它寫的——請看[威脅模型](/zh-TW/docs/whitepaper)。

## 真的沒有助記詞嗎？

真的沒有。你的金鑰是密碼金鑰，而密碼金鑰沒有可以抄下來或輸入的祕密。請看[密碼金鑰如何運作](/zh-TW/docs/passkeys)。

## 建立錢包需要什麼？

一台支援密碼金鑰的裝置（有 Face ID、指紋或 Windows Hello 的較新手機或電腦），或兩把硬體安全金鑰。
不需要電子郵件、不需要註冊帳號，也不需要先有餘額。建立錢包時最多可以加入七把金鑰，之後不能再加。請看
[建立你的錢包](/zh-TW/docs/create-wallet)。

## 手機掉了怎麼辦？

在新裝置上用任何另一把金鑰登入：透過 iCloud 鑰匙圈或 Google 密碼管理工具同步過來的同一把密碼金鑰、
另一支手機，或你的安全金鑰。如果那支手機上存著你唯一的金鑰，而且沒有同步，錢包就無法復原。請看
[復原與登入](/zh-TW/docs/recovery)。

## 支援哪些網路和代幣？

24 條內建 EVM 網路，包括 Ethereum、Base、Arbitrum、Optimism、Polygon、BNB Chain、Gnosis 和
Avalanche，另外還可以新增任何符合條件的 EVM 網路。支援原生幣和 ERC-20 代幣。每條網路上都是同一個
位址。請看[網路與費用](/zh-TW/docs/networks-and-fees)。

## 要花多少錢？

- **應用程式：** 網頁錢包、瀏覽器擴充功能和桌面版都免費。iOS 和 Android 應用程式會以一次性買斷的方式在商店販售；
  你也可以從原始碼免費建置任何一個應用程式。
- **每筆交易：** 從你的錢包付給送出它的中繼一筆手續費。它涵蓋 gas 加上中繼的利潤，常常是這筆交易
  鏈上成本的十倍以上，最低約 0.01 美元。確切金額會顯示在確認頁上，也寫在你簽署的內容裡。沒有押金，
  也沒有訂閱費。[手續費怎麼算](/zh-TW/docs/networks-and-fees)。
- **沒有代幣。** Vela 沒有代幣，也沒有發幣計畫。

## 能用 Vela 連接 dApp 嗎？

可以，透過 Vela 瀏覽器擴充功能（Chrome、Edge、Brave），以及桌面版（macOS、Windows）、iOS 和 Android 應用程式
內建的瀏覽器。wallet.getvela.app 上的網頁錢包不會連接 dApp。請看[安裝](/zh-TW/docs/install#dapps)。

## Vela 看得到什麼、能做什麼？

Vela 讀不到你的金鑰，自己也動不了你的資金。它的服務看得到你的 IP 位址，以及應用程式向它們查詢的內容：
索引在註冊新錢包時會看到你的公鑰和錢包名稱，也會看到你查詢的位址；中繼會看到你的位址、你送出的
操作，以及你的應用程式使用的 RPC 節點；鏈資料服務會看到你的應用程式查詢了哪些代幣和合約。哪些內容會公開在鏈上，
列在[建立你的錢包](/zh-TW/docs/create-wallet#what-is-public)。完整、具權威性的版本
以[隱私權政策](/privacy)為準。

## Vela 是開源的嗎？

錢包應用程式、中繼和匯率服務都以 MIT 授權公開在 [GitHub](https://github.com/orgs/mondaylabsltd/repositories)
上；鏈資料目錄也是 MIT。公鑰索引的程式碼是公開的，但目前還沒有授權條款檔案。每一項服務你都可以自己架設——
請看[自架指南](/zh-TW/docs/self-hosting)。

## Vela 有經過稽核嗎？

存放你資金的合約——Safe 和它的模組，以及 ERC-4337 EntryPoint——都經過稽核。Vela 自己的程式碼沒有，目前也
沒有排定稽核。請看[稽核與已知問題](/zh-TW/docs/security-audits)。

## 如果 Vela 收掉了呢？

你的資金留在鏈上你自己的 Safe 裡。對已經存在的錢包來說，Vela 瀏覽器擴充功能和你自己建置的應用程式不靠
getvela.app 也能繼續用，而每一項服務都是開源的，其他人也能接手架設（中繼需要改一處程式碼，才能不再從 Vela 的
伺服器讀取鏈資料）。[自架指南](/zh-TW/docs/self-hosting#if-getvela-app-disappears)
列出了每一條路和它的限制。

## 我的問題這裡沒有。

到 [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues) 上開一個 issue，或透過
[X](https://x.com/realvelawallet)、[Telegram](https://t.me/velawallet) 聯絡我們。
