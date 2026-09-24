---
title: Bybit 攻擊事件，以及它走的那條路
description: "2025 年 2 月，Bybit 損失約 15 億美元。被攻破的不是 Safe 合約，而是介面。這一頁說明那條路徑，以及 Vela 的設計裡有什麼把它堵住。"
source: 14ae76da6694
---

# Bybit 攻擊事件，以及它走的那條路

2025 年 2 月 21 日，Bybit 從一個 Safe 多簽冷錢包損失了大約 **15 億美元**。這是
這個產業史上最大的一次竊案，值得仔細讀，因為除了一件事之外，那天幾乎每個環節
都是*正確*的。

## 發生了什麼事

根據公開的事後檢討報告，簡短版本是：

1. 攻擊者入侵了一台 **`Safe{Wallet}` 開發者的電腦**，把惡意 JavaScript 植入替
   `Safe{Wallet}` 前端提供服務的 AWS S3 儲存貯體。程式碼在 2 月 19 日植入，
   2 月 21 日觸發，目標鎖定 Bybit 那一個特定的 Safe。
2. Bybit 的簽署人打開介面，檢查了一筆看起來再普通不過的交易。
3. 實際送到他們**硬體錢包**上的酬載，並不是那筆交易。它是一次 `delegatecall`，
   覆寫了 Safe 代理合約的 `masterCopy`——第 0 號儲存槽——把整個帳戶實作換成了
   攻擊者的。
4. 簽署人核准了。簽章是有效的。合約完全照著它收到的指令執行。

在歸因上，公開說法指向與北韓有關的活動（FBI 點名了 TraderTraitor 集團）。

## 什麼*沒有*被攻破

- **不是 Safe 合約。** 它們執行的是一道簽章有效的指令。Safe 沒有任何漏洞被利用。
- **不是密碼學。** 每一個簽章都是真的。
- **不是硬體錢包。** Ledger 裝置就在流程裡，而且照樣簽了——因為硬體錢包顯示的是
  交到它手上的東西，而交到它手上的正是那份惡意酬載。一台無法把 `delegatecall`
  解讀成人能判斷的內容的裝置，保護的是*金鑰*，不是*決定*。

被攻破的，是每一個錢包介面底下的那個假設：**描述這筆交易的畫面，和正在被簽署的
那串位元組，是同一件事。**

## 為什麼這是普遍情況，而不是一次離奇事件

網頁錢包產生的大多數簽章，都建立在那個假設上。介面組出酬載，介面算繪摘要，
卻沒有任何獨立的東西去核對兩者是否一致。一旦替那個介面提供服務的程式碼被換掉——
被入侵的建置流程、被劫持的 CDN、一個惡意的相依套件、一份被偷走的部署憑證——摘要就會
變成攻擊者想要的樣子，而你的簽章是真的。

這正是 Vela 的簽署設計所針對的風險。不是釣魚。不是私鑰外洩。**是一個在騙你的
簽署畫面。**

## Vela 為此做了什麼

**清晰簽署，一路看到 calldata。** 每一筆交易在你核准之前，都會被解碼成人看得懂的
意圖——金額、收款人、這個呼叫到底在做什麼（[ERC-7730](/zh-TW/docs/clear-signing)）。
解不開的呼叫會被**明確標示為無法解碼**，而不是悄悄算繪成一副沒事的樣子。Bybit 那份
酬載是一次替換實作位址的 `delegatecall`；那恰恰是應該讓簽署人當場停手的東西，而把它
藏在一段友善的摘要後面，正是它沒有被擋下的原因。

有兩件事要說清楚。dApp 無法直接要求 Vela 執行 `delegatecall`——
網頁能發出的請求只會產生一般的呼叫——所以 Bybit 那份酬載本身沒辦法從這條路進來。
而網頁如果請求從你的 Safe 呼叫它自己，會被**直接拒絕，而不只是解碼出來**：
`enableModule`、`addOwnerWithThreshold`、`swapOwner`、`setFallbackHandler`、`setGuard`
以及同一族的其他方法，只要目標是你自己的錢包就會被擋下來，
放進批次交易裡、放進 `MultiSend` 裡同樣會擋；
任何帶 `delegatecall` 的分支，不管目標是誰也一樣擋，`SafeTx` 型別化資料簽署也一樣。
錢包會告訴你它拒絕的是哪一個呼叫，而且不會拿出任何可以簽的東西。
這裡面任何一個，只要簽一次，就會像 Bybit 那份酬載一樣把帳戶整個交出去——
被啟用的模組接著就能自己執行 `delegatecall`。而如果 Vela 自己的程式碼被換掉，
就像 `Safe{Wallet}` 那樣，解碼結果也就成了攻擊者的解碼——這正是下一點要處理的。

**一條能反過來核對介面的獨立路徑。** Vela 已經做好一個零建置、零相依的
[簽署頁](/zh-TW/docs/clear-signing-self-host)，它自己解碼請求、自己完成 WebAuthn 簽署——
只是一個裝著靜態檔案的資料夾，你可以從頭讀到尾、自己架設，或當成瀏覽器擴充功能載入。它的用意是提供一個
不和主應用程式共用供應鏈的第二意見。*狀態：已完成並測試過；尚未發布，目前也還沒有任何 Vela 應用程式
會把請求交給它。* 等狀態改變，這一頁會直接寫出來。

**我們沒有可以被奪走的管理員角色。** Vela 的帳戶是[未經改動的 Safe v1.4.1](/zh-TW/docs/account-contract)，
Vela 在上面沒有任何特權角色——沒有管理員金鑰，也沒有任何一條我們可能被脅迫或被入侵之後拿來用的升級
途徑。但要說清楚這*沒有*消除什麼：Bybit 攻擊者用的那個基本手段——由擁有者簽署、改寫帳戶實作的
`delegatecall`——在每個 Safe 裡都存在，Vela 的也一樣（Vela 自己的批次交易就是透過 `delegatecall`
呼叫 Safe 的 MultiSend）。它需要你其中一把金鑰的有效簽章。防止你被騙著簽下去的，是上面的解碼，以及那條
獨立的核對路徑。

**每一次簽署都要重新確認。** 每次簽署都需要你的金鑰自己確認——Face ID、指紋、PIN 碼，或在
安全金鑰上觸碰並輸入 PIN 碼。沒有長期有效的工作階段金鑰，所以也沒有一段「你不在場、卻有東西能替你簽」
的空窗期。

**自架，是最後一道防線。** 各個應用程式和後端服務都是開源的。如果你完全不想信任
我們的建置流程，就自己建置擴充功能或應用程式，再架設你需要的服務——[自架指南](/zh-TW/docs/self-hosting)
一步步說明怎麼做。這樣就把我們的建置流程從信任鏈裡拿掉了；你仍然要信任自己建置的程式碼，所以請去讀它。

## Vela 不宣稱什麼

Vela 的前端也可能用 `Safe{Wallet}` 被入侵的同一種方式被入侵。我們的程式碼沒有經過稽核。
說得比這更好聽，正是這次事件本該終結的那種保證。

這套設計想做的，是把路徑收窄：讓酬載看得懂而不是一團黑箱，不持有任何可能被濫用來對付你的
管理員角色，並給你一種用「不是我們」的東西來核對的方法。誠實的總結是：
**這一類攻擊是靠設計降低的，不是被消除的**，而那些能讓防護更完整的部分，
正以未完成的狀態列在[稽核與已知問題](/zh-TW/docs/security-audits)裡。

## 資料來源

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
