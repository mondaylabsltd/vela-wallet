---
title: 帳戶合約
description: "你的 Vela 錢包是一個未經修改的 Safe v1.4.1。通往你資金的路徑上，沒有一份合約是 Vela 寫的——本頁清楚列出是哪些合約、這樣做帶來甚麼好處，以及代價是甚麼。"
source: 17fbc25a3149
---

# 帳戶合約

你的錢包不是某個 App 的私有資料結構。它是一個 **Safe v1.4.1** 智能帳戶——許多大型鏈上金庫都在使用這份
合約——完全按照 Safe 發佈的版本部署，沒有任何修改。

## 路徑上沒有我們的合約

每一份能接觸你資金的合約，都出自 Safe 或 ERC-4337 作者之手：

| 合約 | 在你錢包中的角色 | 作者 |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)（SafeL2，經代理合約） | 帳戶本身：擁有者、門檻、執行 | Safe |
| [Safe 4337 模組 v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | 讓 EntryPoint 能操作這個 Safe；同時也是它的後備處理器（fallback handler） | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | 驗證第一把鑰匙的 P-256 簽名 | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) 及其建立的簽署器 | 其餘每把鑰匙各有一份小型簽署器合約 | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | 執行你已簽署的操作 | ERC-4337 作者 |

Vela 自己的合約都不在這張表中：記錄每個錢包的鑰匙、讓新裝置能找到錢包的**公鑰註冊表**
（見[復原](/zh-HK/docs/recovery)）、與它配套的域名註冊表，以及被它們取代的早期索引。它們不持有資金，
在你的 Safe 中也沒有任何角色。

錢包的程式碼庫中完全沒有 Solidity——一條指令就能核實：

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # 甚麼也不會輸出
```

Vela 在你的帳戶上沒有任何特權角色：沒有管理員密鑰，沒有升級途徑，也不能加入任何模組。只有你的鑰匙才能
更改你的 Safe。

## 為甚麼「未經修改」這個詞最重要

很多錢包建基於「一個 Safe」、Safe 的分叉版本，或受 Safe 啟發的帳戶。分別體現在三方面。

**審計涵蓋的正是你實際在用的東西。**Safe 的審計涵蓋這些版本，或與它們只有少量已記錄改動的較早版本
（詳情見[審計頁面](/zh-HK/docs/security-audits)）。分叉版本的審計涵蓋的是分叉之前的程式碼；改動的部分，
沒有人審計過。

**整個生態系統把你的帳戶當作 Safe，因為它本來就是。**區塊瀏覽器能解析它，Safe 的工具能讀取它並為它
建構交易。但要*簽署*這些交易，程式必須能向你的鑰匙請求一個針對 `getvela.app`（你的通行密鑰所屬的域名）
的簽名——所以在另一個域名上運行的 Safe 官方網頁 App 無法為你簽署。
[自行架設指南](/zh-HK/docs/self-hosting#if-getvela-app-disappears)列出了哪些方式可以。

**這個攻擊面有很多人一同盯着。**度身訂造的帳戶合約，基本上只有作者自己在看。Safe 的核心合約，所有把錢
放在 Safe 裏的人都在看；4337 模組和通行密鑰模組的關注者較少，但確實有人在看。

## 代價

採用標準並非沒有代價：

- **Gas。**你的簽名在鏈上驗證，交易還要經過 EntryPoint。按我們 2026 年 9 月在 Gnosis 上的實測，從已部署
  的 Vela 錢包進行一次簡單轉賬，在鏈上約消耗 140,000 至 170,000 gas；從普通帳戶轉出一筆 ETH 則只需
  21,000。在 gas 之外，中繼還會收取手續費——見[網絡與費用](/zh-HK/docs/networks-and-fees)。
- **帳戶必須部署。**你的地址是在鏈上還沒有任何東西時，就用 `CREATE2` 計算出來的，所以你可以立即在這個
  地址收款；你在每條網絡上的第一筆轉出交易會支付部署合約的費用。
- **不是每條鏈都符合資格。**通行密鑰簽名由 **EIP-7951 / RIP-7212** 預編譯合約驗證，而它的地址是每個錢包初始設定
  資料的一部分，所以沒有它的網絡根本無法運行 Vela。
- **Safe 的風險就是你的風險。**信任一份廣泛使用的合約，仍然是在信任一份合約。Vela 沒有在你資金的路徑上
  再加一份自己的合約讓你信任。

## 哪些經過審計，哪些沒有

Safe 的合約、它的 4337 模組和通行密鑰模組，以及 EntryPoint v0.7，都有公開的第三方審計報告。**Vela 自己
的程式碼——各個 App、後端服務和註冊表合約——沒有經過第三方審計，目前亦沒有排期**；這是項目有能力出資時
的目標，而不是一個有日期的承諾。每一份合約、它的審計報告，以及我們正在追蹤的問題，都列在
[審計與已知問題](/zh-HK/docs/security-audits)中。

## 親自查看

你的帳戶在鏈上。錢包部署後，在區塊瀏覽器中打開你的地址：它是一個 Safe 代理合約，其實作合約是 Safe 官方
部署的 SafeL2 v1.4.1，每條網絡上都一樣。

下一步：[審計與已知問題](/zh-HK/docs/security-audits)。
