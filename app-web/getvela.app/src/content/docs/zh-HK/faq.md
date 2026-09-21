---
title: 常見問題
description: "關於託管、鑰匙、復原、網絡、費用、Vela 能看到甚麼、開源，以及 Vela 不再營運時會怎樣的簡短解答。"
source: 0762e55bf54d
---

# 常見問題

## Vela 是自我託管的嗎？

是。你的錢包是一個只由你的鑰匙控制的 Safe 智能帳戶，而鑰匙留在你的裝置、你的密碼管理工具或你的安全密鑰
中。Vela 不持有任何鑰匙，在錢包上也沒有任何角色，所以它無法自行轉移、凍結或取回你的資金。但請你的鑰匙
簽署的軟件是它寫的——見[威脅模型](/zh-HK/docs/whitepaper)。

## 真的沒有助記詞嗎？

真的沒有。你的鑰匙是通行密鑰，而通行密鑰沒有可以抄寫或輸入的秘密。見[通行密鑰如何運作](/zh-HK/docs/passkeys)。

## 建立錢包需要甚麼？

一部支援通行密鑰的裝置（有 Face ID、指紋或 Windows Hello 的較新手機或電腦），或兩把硬件安全密鑰。不需要
電郵、不需要註冊帳戶，也不需要預先有餘額。建立錢包時最多可以設定七把鑰匙；之後不能再加。見
[建立錢包](/zh-HK/docs/create-wallet)。

## 手機遺失了怎麼辦？

在新裝置上用任何另一把鑰匙登入：透過 iCloud 鑰匙圈或 Google 密碼管理工具同步的同一把通行密鑰、另一部
手機，或你的安全密鑰。如果那部手機存放着你唯一的鑰匙，而那把鑰匙沒有同步，錢包就無法復原。見
[復原與登入](/zh-HK/docs/recovery)。

## 支援哪些網絡和代幣？

24 條內置 EVM 網絡，包括 Ethereum、Base、Arbitrum、Optimism、Polygon、BNB Chain、Gnosis 和 Avalanche，
另外還可以加入任何符合要求的 EVM 網絡。支援原生幣和 ERC-20 代幣。每條網絡上都是同一個地址。見
[網絡與費用](/zh-HK/docs/networks-and-fees)。

## 要花多少錢？

- **App：**網頁版錢包、瀏覽器擴充功能和桌面版都是免費的。iOS 和 Android App 將在應用程式商店以一次性購買
  的方式發售；你也可以免費從原始碼編譯任何一個 App。
- **每筆交易：**從你的錢包支付給提交交易的中繼的一筆手續費。它涵蓋 gas 和中繼的利潤，往往是這筆交易鏈上
  成本的十倍或以上，最低約 0.01 美元。確切金額顯示在確認畫面上，並且是你所簽署內容的一部分。沒有按金，
  也沒有訂閱費。[手續費如何計算](/zh-HK/docs/networks-and-fees)。
- **沒有代幣。**Vela 沒有代幣，也不打算發行。

## 可以用 Vela 連接 dApp 嗎？

可以，透過 Vela 瀏覽器擴充功能（Chrome、Edge、Brave），以及桌面版（macOS、Windows）、iOS 和 Android App
的內置瀏覽器。wallet.getvela.app 上的網頁版錢包不會連接 dApp。見[安裝](/zh-HK/docs/install#dapps)。

## Vela 能看到甚麼、能做甚麼？

Vela 無法讀取你的鑰匙，也無法自行轉移你的資金。它的服務會看到你的 IP 地址，以及 App 向它們查詢的內容：
索引在登記新錢包時會看到你的公鑰和錢包名稱，也會看到你查詢的地址；中繼會看到你的地址、你提交的操作，以及
你的 App 使用的 RPC 端點；鏈數據服務會看到你的 App 查詢了哪些代幣和合約。哪些內容會在鏈上公開，列在
[建立錢包](/zh-HK/docs/create-wallet#what-is-public)中。完整而權威的說明以[私隱政策](/privacy)為準。

## Vela 是開源的嗎？

錢包 App、中繼和匯率服務都以 MIT 授權發佈在 [GitHub](https://github.com/orgs/mondaylabsltd/repositories)
上；鏈數據目錄也採用 MIT 授權。公鑰索引的程式碼是公開的，但暫時還沒有授權條款檔案。每一項服務你都可以
自行運行——見[自行架設指南](/zh-HK/docs/self-hosting)。

## Vela 經過審計嗎？

存放你資金的合約——Safe 及其模組，以及 ERC-4337 EntryPoint——都經過審計。Vela 自己的程式碼沒有，目前也
沒有安排任何審計。見[審計與已知問題](/zh-HK/docs/security-audits)。

## 如果 Vela 結業呢？

你的資金留在鏈上你自己的 Safe 裏。對於現有的錢包，Vela 瀏覽器擴充功能和你自行編譯的 App 在沒有
getvela.app 的情況下仍然可以繼續使用，而每一項服務都是開源的，其他人也可以運行（中繼需要修改程式碼，才能
不再從 Vela 的伺服器讀取鏈數據）。[自行架設指南](/zh-HK/docs/self-hosting#if-getvela-app-disappears)列出了
各條途徑及其限制。

## 我的問題不在這裏。

請在 [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues) 上開一個 issue，或透過
[X](https://x.com/realvelawallet) 或 [Telegram](https://t.me/velawallet) 聯絡我們。
