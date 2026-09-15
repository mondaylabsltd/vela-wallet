---
title: Bybit 攻擊事件
description: 2025 年 2 月，Bybit 損失約 15 億美元。被攻破的不是 Safe 合約，而是介面。這一頁講清楚那條路徑，以及 Vela 的設計裡有什麼把它擋住了。
---

# Bybit 攻擊事件

2025 年 2 月 21 日，Bybit 從一個 Safe 多簽冷錢包裡損失了大約 **15 億美元**。這是
這個產業史上最大的一次失竊，而且值得仔細讀，因為除了一件事之外，那天幾乎所有
環節都是*正確*的。

## 發生了什麼

按公開的事後檢討，簡短版本是：

1. 攻擊者攻陷了一台 **`Safe{Wallet}` 開發者的機器**，把惡意 JavaScript 注入到為
   `Safe{Wallet}` 前端提供服務的 AWS S3 儲存桶裡。程式碼在 2 月 19 日植入，
   2 月 21 日被觸發，目標精準指向 Bybit 那個特定的 Safe。
2. Bybit 的簽署人打開介面，檢視了一筆看起來再正常不過的交易。
3. 真正送到他們**硬體錢包**裡的內容，並不是那筆交易。它是一次 `delegatecall`，
   覆寫了 Safe 代理合約的 `masterCopy`——第 0 號儲存槽——把整個帳戶實作換成了
   攻擊者的。
4. 簽署人核准了。簽章是有效的。合約嚴格照著它被告知的去做了。

歸因上，公開將其指向與北韓相關的活動（FBI 點名了 TraderTraitor 集團）。

## 什麼*沒有*被攻破

- **不是 Safe 合約。** 它們執行的是一條簽章有效的指令。Safe 沒有任何漏洞被利用。
- **不是密碼學。** 每一個簽章都是真的。
- **不是硬體錢包。** Ledger 裝置就在流程裡，而且照樣簽了——因為硬體錢包顯示的是
  交到它手上的東西，而交到它手上的正是那份惡意內容。一台沒辦法把 `delegatecall`
  解成人類看得懂的東西的裝置，保護的是*金鑰*，不是*決定*。

被攻破的，是每一個錢包介面底下的那個假設：**描述這筆交易的畫面，和正在被簽署的
那串位元組，是同一樣東西。**

## 為什麼這是通例，而不是一次離奇事件

你在網頁錢包裡產生過的每一個簽章，都建立在那個假設上。介面組出內容，介面畫出摘要，
而沒有任何獨立的東西去核對兩者是否一致。一旦替那個介面提供服務的程式碼被替換——
被攻陷的建置流程、被劫持的 CDN、一個惡意相依套件、一份被偷走的部署憑證——摘要就會
變成攻擊者想要的樣子，而你的簽章是真的。

這正是 Vela 的簽署設計所針對的風險。不是釣魚。不是私鑰外洩。**是一塊在騙你的
簽署畫面。**

## Vela 為此做了什麼

**清晰簽署，一路看到 calldata。** 每一筆交易在你核准之前都會被解碼成人看得懂的
意圖——金額、收款方、這個呼叫到底做什麼（[ERC-7730](/zh-TW/docs/clear-signing)）。
解不開的呼叫會被**明確標示為無法解碼**，而不是默默畫成一副沒事的樣子。Bybit 那份
內容是一次替換實作位址的 `delegatecall`；那恰恰是應該讓簽署人當場煞車的東西，而把
它藏在一段友善的摘要後面，正是它沒有被煞住的原因。

**一條能反過來核對介面的獨立路徑。** Vela 正在做一個零建置、零相依的簽署頁面，
它自己畫出意圖、自己完成 WebAuthn 簽章——一個你可以從頭讀到尾、自己架設、或是
當成瀏覽器擴充功能來跑的靜態資料夾。它存在的全部意義，就是給出一個不與主應用共用
供應鏈的第二意見。*狀態：已經做完並測試過，尚未部署。* 上線之後它是選用的，
等狀態變了，這一頁會直說。

**沒有我們能升級的合約。** Bybit 那次之所以奏效，靠的是替換帳戶的實作。Vela 的
帳戶是[未經改動的 Safe v1.4.1](/zh-TW/docs/account-contract)，Vela 在上面沒有任何
特權角色——沒有管理員金鑰，也沒有一條我們可能被脅迫或被攻陷後拿來用的升級管道。

**每一次簽署都要重做一次生物辨識。** 不存在長期有效的工作階段金鑰，所以也不存在
一個「你不在場卻有人能替你簽」的時間窗。

**自行部署，是最後那道底。** 應用和每一項後端服務都是開放原始碼的。如果你根本不想
信任我們的建置流程，那就自己跑一份——面對這一類攻擊，這是唯一一個不需要信任任何人
的答案。

## Vela 不聲稱什麼

Vela 的前端完全可能以 `Safe{Wallet}` 被攻陷的同樣方式被攻陷。我們的程式碼沒有稽核
過。說得比這更漂亮，恰恰是這次事件本該終結的那種保證。

這套設計想做的，是把路徑收窄：讓內容可讀而不是不透明，拿掉這次攻擊所依賴的升級
原語，並且給你一種用「不是我們」的東西去核對的辦法。誠實的總結是：
**這一類攻擊是靠設計被削弱的，不是被消除的**，而那些能讓它更硬的部分，
正以未完成的狀態列在[稽核與已知問題](/zh-TW/docs/security-audits)裡。

## 資料來源

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
