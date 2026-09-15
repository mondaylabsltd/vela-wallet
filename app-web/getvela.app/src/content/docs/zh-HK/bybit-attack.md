---
title: Bybit 被攻擊嗰次
description: 2025 年 2 月，Bybit 損失約 15 億美元。俾人攻破嘅唔係 Safe 合約，而係個介面。呢一頁講清楚嗰條路，同埋 Vela 嘅設計裡面有乜嘢擋住咗佢。
---

# Bybit 被攻擊嗰次

2025 年 2 月 21 日，Bybit 喺一個 Safe 多簽冷錢包度損失咗大約 **15 億美元**。呢個係
呢個行業史上最大嘅一次失竊，而且值得慢慢讀，因為除咗一件事之外，嗰日幾乎所有環節
都係*啱*嘅。

## 發生咗啲乜

按公開嘅事後檢討，簡短版本係：

1. 攻擊者攻陷咗一部 **`Safe{Wallet}` 開發者嘅機**，將惡意 JavaScript 注入去為
   `Safe{Wallet}` 前端提供服務嘅 AWS S3 儲存桶。代碼喺 2 月 19 日植入，
   2 月 21 日被觸發，目標精準指向 Bybit 嗰個特定嘅 Safe。
2. Bybit 嘅簽名人打開個介面，睇咗一筆睇落再正常不過嘅交易。
3. 真正送到佢哋**硬件錢包**嗰份嘢，並唔係嗰筆交易。佢係一次 `delegatecall`，
   覆寫咗 Safe 代理合約嘅 `masterCopy`——第 0 號儲存槽——將成個帳戶實作換成
   攻擊者嘅。
4. 簽名人批准咗。啲簽名係有效嘅。份合約完全照佢收到嘅指示去做。

歸因方面，公開將佢指向同北韓有關嘅活動（FBI 點名咗 TraderTraitor 集團）。

## 咩嘢*冇*俾人攻破

- **唔係 Safe 合約。** 佢哋執行嘅係一條簽名有效嘅指令。Safe 冇任何漏洞俾人利用。
- **唔係密碼學。** 每一個簽名都係真嘅。
- **唔係硬件錢包。** Ledger 裝置就喺流程入面，而且照樣簽咗——因為硬件錢包顯示嘅係
  交到佢手上嗰樣嘢，而交到佢手上嗰樣正正係嗰份惡意內容。一部解唔到 `delegatecall`
  成人睇得明嘅嘢嘅機，保護嘅係*鎖匙*，唔係*決定*。

俾人攻破嗰樣，係每一個錢包介面底下嗰個假設：**描述呢筆交易嘅畫面，同正在被簽嗰串
位元組，係同一樣嘢。**

## 點解呢個係通例，唔係一次離奇事件

你喺網頁錢包度產生過嘅每一個簽名，都建喺嗰個假設上面。介面砌出內容，介面畫出摘要，
而冇任何獨立嘅嘢去核對兩者一唔一致。一旦為嗰個介面提供服務嘅代碼俾人換咗——
俾人攻陷嘅建置流程、俾人騎劫嘅 CDN、一個惡意依賴、一份俾人偷咗嘅部署憑證——摘要
就會變成攻擊者想要嘅樣，而你個簽名係真嘅。

呢個正正就係 Vela 嘅簽名設計所針對嘅風險。唔係釣魚。唔係私鑰外洩。**係一塊喺度
呃緊你嘅簽名畫面。**

## Vela 為咗呢件事做咗啲乜

**清晰簽署，一路睇到 calldata。** 每一筆交易喺你批准之前都會解成人睇得明嘅意圖
——金額、收款方、呢個調用到底做乜（[ERC-7730](/zh-HK/docs/clear-signing)）。
解唔到嘅調用會**明確標示為解唔到**，而唔係靜靜雞畫成一副冇事咁。Bybit 嗰份嘢係
一次換實作地址嘅 `delegatecall`；嗰樣正正應該令簽名人即刻煞停，而將佢收埋喺一段
友善嘅摘要後面，就係佢冇被煞停嘅原因。

**一條可以反過嚟核對介面嘅獨立路徑。** Vela 而家喺度做一個零建置、零依賴嘅簽名
頁面，佢自己畫出意圖、自己完成 WebAuthn 簽名——一個你可以由頭讀到尾、自己架、
或者當成瀏覽器擴充功能嚟行嘅靜態資料夾。佢存在嘅全部意義，就係畀一個唔同主應用
共用供應鏈嘅第二意見。*狀態：已經做完並測過，未部署。* 上線之後佢係可選嘅，
狀態一變，呢一頁會直說。

**冇一份我哋升級得到嘅合約。** Bybit 嗰次之所以成功，靠嘅係換咗個帳戶嘅實作。
Vela 嘅帳戶係[未經改動嘅 Safe v1.4.1](/zh-HK/docs/account-contract)，Vela 喺上面
冇任何特權角色——冇管理員鎖匙，亦冇一條我哋可能俾人夾硬或者攻陷之後攞嚟用嘅升級
通道。

**每一次簽名都要重新做一次生物認證。** 冇長期有效嘅工作階段鎖匙，所以亦冇一個
「你唔喺場都有人簽得到」嘅時間窗。

**自己部署，係最後嗰道底。** 應用同每一項後端服務都係開源嘅。如果你根本唔想信我哋
嘅建置流程，咁就自己行一份——面對呢一類攻擊，呢個係唯一一個唔使信任何人嘅答案。

## Vela 唔會聲稱啲乜

Vela 嘅前端完全可能以 `Safe{Wallet}` 俾人攻陷嘅同一個方式俾人攻陷。我哋嘅代碼冇
審計過。講得靚過呢句，正正就係呢單事本應終結嗰種保證。

呢套設計想做嘅，係將條路收窄：令內容睇得明而唔係一嚿雲，攞走呢次攻擊所依賴嘅升級
原語，並且畀你一個用「唔係我哋」嘅嘢去核對嘅辦法。誠實嘅總結係：
**呢一類攻擊係靠設計被削弱，唔係被消除**，而嗰啲可以令佢更硬嘅部分，
正以未完成嘅狀態列喺[審計同已知問題](/zh-HK/docs/security-audits)。

## 資料來源

- [SlowMist — Bybit's $1.5 billion theft unveiled: `Safe{Wallet}` front-end code tampered](https://slowmist.medium.com/bybits-1-5-billion-theft-unveiled-safe-wallet-front-end-code-tampered-84b78f0fa9c2)
- [NCC Group — Bybit hack: in-depth technical analysis](https://www.nccgroup.com/research/in-depth-technical-analysis-of-the-bybit-hack/)
- [Sygnia — Investigation into the Bybit hack](https://www.sygnia.co/blog/sygnia-investigation-bybit-hack/)
- [BlockSec — A web2 breach enables the largest crypto hack in history](https://blocksec.com/blog/bybit-incident-a-web2-breach-enables-the-largest-crypto-hack-in-history)
