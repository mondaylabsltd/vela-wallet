---
title: Bybit 被攻擊事件，以及攻擊者走的那條路
description: "2025 年 2 月，Bybit 損失約 15 億美元。被攻破的不是 Safe 合約，而是介面。本頁說明那條攻擊路徑，以及 Vela 的設計中有甚麼堵住這條路。"
source: 14ae76da6694
---

# Bybit 被攻擊事件，以及攻擊者走的那條路

2025 年 2 月 21 日，Bybit 從一個 Safe 多簽冷錢包中損失了約 **15 億美元**。這是業界歷史上最大的一宗
盜竊案，值得仔細閱讀，因為除了一件事之外，當中幾乎每個環節都是*正確*的。

## 發生了甚麼

根據公開的事後分析，簡短版本如下：

1. 攻擊者入侵了一部 **`Safe{Wallet}` 開發人員的電腦**，把惡意 JavaScript 注入為 `Safe{Wallet}` 前端
   提供內容的 AWS S3 儲存桶。程式碼在 2 月 19 日植入，2 月 21 日觸發，目標正是 Bybit 那個特定的 Safe。
2. Bybit 的簽署人打開介面，審閱了一筆看起來很普通的交易。
3. 實際送到他們**硬件錢包**的載荷，並不是那筆交易。那是一個 `delegatecall`，覆寫了 Safe 代理合約的
   `masterCopy`——即第 0 號儲存槽——把整個帳戶實作換成攻擊者的版本。
4. 簽署人批准了。簽名是有效的。合約完全按照指令執行。

事件被公開歸咎於與北韓有關的活動（FBI 點名 TraderTraitor 組織）。

## 甚麼*沒有*被攻破

- **不是 Safe 合約。**它們執行的是一條有效簽署的指令。攻擊沒有利用 Safe 的任何漏洞。
- **不是密碼學。**每一個簽名都是真的。
- **不是硬件錢包。**Ledger 裝置就在流程之中，而且照樣簽署了——因為硬件錢包顯示的是交給它的內容，而
  交給它的正是那份惡意載荷。一部無法把 `delegatecall` 解碼成人類可以判斷的內容的裝置，保護的是*鑰匙*，
  而不是*決定*。

被攻破的，是每個錢包介面底下的那個假設：**描述交易的畫面，與正在被簽署的位元組，是同一回事。**

## 為甚麼這是普遍情況，而不是偶發事件

網頁錢包產生的大多數簽名，都建立在這個假設之上。介面建構載荷，介面呈現摘要，卻沒有任何獨立的東西核對
兩者是否一致。一旦提供該介面的程式碼被替換——被入侵的建置流程、被劫持的 CDN、惡意的依賴套件、被盜的
部署憑證——摘要就會變成攻擊者想要的任何樣子，而你的簽名是真的。

這正是 Vela 的簽署設計所針對的風險。不是釣魚。不是私鑰外洩。**而是一個在欺騙你的簽署畫面。**

## Vela 如何應對

**清晰簽署，一直看到 calldata。**每一筆交易在你批准前都會被解碼成人看得懂的意圖——金額、收款方、這個
呼叫實際做甚麼（[ERC-7730](/zh-HK/docs/clear-signing)）。我們無法解碼的呼叫會被**標示為無法解碼**，而不是
悄悄呈現得好像沒有問題。Bybit 那份載荷是一個調換實作地址的 `delegatecall`；這正正是應該令簽署人立即停手
的那類東西，而把它藏在友善的摘要後面，就是它沒有被攔住的原因。

有兩件事需要說清楚。dApp 無法直接要求 Vela 執行 `delegatecall`——
網頁能發出的請求只會產生普通的呼叫——所以 Bybit 那份載荷本身無法經這條路進來。
而網頁如果請求由你的 Safe 呼叫它自己，會被**直接拒絕，而不只是解碼出來**：
`enableModule`、`addOwnerWithThreshold`、`swapOwner`、`setFallbackHandler`、`setGuard`
以及同一系列的其他方法，只要目標是你自己的錢包就會被攔下，
放進批量交易裡、放進 `MultiSend` 裡同樣攔；
任何帶 `delegatecall` 的分支，不論目標是誰也一樣攔，`SafeTx` 類型化資料簽名亦然。
錢包會告訴你它拒絕的是哪一個呼叫，而且不會拿出任何可簽的東西。
其中任何一個，只要簽署一次，就會像 Bybit 那份載荷一樣徹底交出帳戶——
被啟用的模組之後可以自行執行 `delegatecall`。而如果 Vela 自己的程式碼被替換，就像 `Safe{Wallet}`
的那樣，解碼也會變成攻擊者的解碼——這就是下一點要解決的問題。

**一條可以核對介面的獨立路徑。**Vela 已經做好一個零建置、零依賴的[簽名頁](/zh-HK/docs/clear-signing-self-host)，
它會自行解碼請求並完成 WebAuthn 簽署——一個由靜態檔案組成的資料夾，你可以從頭讀到尾、自行架設，或作為
瀏覽器擴充功能載入。它的目的是提供一個不與主 App 共用供應鏈的第二意見。*狀態：已完成並測試；尚未發佈，
目前也沒有任何 Vela App 會向它發送請求。*情況有變時，本頁會清楚說明。

**我們沒有可被奪去的管理員角色。**Vela 的帳戶是[未經修改的 Safe v1.4.1](/zh-HK/docs/account-contract)，
Vela 在上面沒有任何特權角色——沒有管理員密鑰，也沒有一條屬於我們、可能在被脅迫或被入侵後被動用的升級
途徑。但要清楚這*沒有*消除甚麼：Bybit 攻擊者使用的那個基本手段——由擁有者簽署、改寫帳戶實作的
`delegatecall`——在每一個 Safe 中仍然存在，Vela 的也不例外（Vela 自己的批量交易就是以 `delegatecall`
呼叫 Safe 的 MultiSend）。它需要你其中一把鑰匙的有效簽名。防止你被人說服簽下它的，是上面提到的解碼和
獨立核對。

**每一次簽署都重新確認。**每一次簽署都需要你的鑰匙親自確認——Face ID、指紋、PIN，或在安全密鑰上觸碰並
輸入 PIN。沒有長期有效的會話密鑰，所以不存在一段「你不在場也有東西可以代你簽署」的時間。

**自行架設作為最後防線。**App 和後端服務都是開源的。如果你完全不想信任我們的建置流程，可以自行編譯
擴充功能或 App，並運行你需要的服務——[自行架設指南](/zh-HK/docs/self-hosting)會一步步說明。這樣就把
我們的建置流程從信任鏈中移除；你仍然要信任你自己編譯的程式碼，所以請閱讀它。

## Vela 沒有聲稱甚麼

Vela 的前端可能以 `Safe{Wallet}` 被入侵的同一方式被入侵。我們的程式碼沒有經過審計。說得比這更好聽，
正是這次事件本應終結的那種保證。

這套設計嘗試做的，是把這條路收窄：讓載荷清晰可讀而不是模糊不清，不持有任何可能被濫用來對付你的管理員
角色，並讓你有辦法用不是我們的東西去核實。誠實的總結是：**這類攻擊在設計上得到緩解，但沒有被消除**，
而能進一步加強防禦的部分，仍以未完成的狀態列在[審計與已知問題](/zh-HK/docs/security-audits)中。

## 資料來源

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
