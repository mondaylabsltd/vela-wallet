---
title: 帳戶合約
description: "你的 Vela 錢包是一個未經修改的 Safe v1.4.1。通往你資金的路徑上，沒有任何一份合約是 Vela 寫的——這裡說清楚到底是哪些合約、換來了什麼、代價又是什麼。"
source: 17fbc25a3149
---

# 帳戶合約

你的錢包不是某個應用程式私有的資料結構。它是一個 **Safe v1.4.1** 智慧帳戶——許多大型鏈上金庫用的也是這份合約——完全照 Safe
發布的樣子部署，沒有任何修改。

## 路徑上沒有我們的合約

每一份碰得到你資金的合約，都出自 Safe 或 ERC-4337 作者之手：

| 合約 | 在你錢包裡的角色 | 作者 |
| --- | --- | --- |
| [Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)（SafeL2，透過代理合約） | 帳戶本身：擁有者、門檻、執行 | Safe |
| [Safe 4337 模組 v0.3.0](https://github.com/safe-global/safe-modules/tree/4337/v0.3.0/modules/4337) | 讓 EntryPoint 能操作這個 Safe；同時也是它的 fallback handler | Safe |
| [SafeWebAuthnSharedSigner v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) | 驗證第一把金鑰的 P-256 簽章 | Safe |
| [SafeWebAuthnSignerFactory v0.2.1](https://github.com/safe-global/safe-modules/tree/passkey/v0.2.1/modules/passkey) 及其建立的簽署器 | 其餘每把金鑰各對應一個小的簽署器合約 | Safe |
| [ERC-4337 EntryPoint v0.7](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0) | 執行你簽好的操作 | ERC-4337 作者 |

Vela 自己寫的合約都不在這張表裡：記錄每個錢包的金鑰、讓新裝置能找到它的**公鑰註冊表**
（見[復原](/zh-TW/docs/recovery)）、搭配它的網域註冊表，以及被它們取代的早期索引合約。它們
不持有資金，在你的 Safe 裡也沒有任何角色。

錢包的程式碼儲存庫裡一行 Solidity 都沒有——一個指令就能確認：

```bash
git clone https://github.com/mondaylabsltd/vela-wallet
find vela-wallet -name '*.sol'   # 什麼都不會輸出
```

Vela 在你的帳戶上沒有任何特權：沒有管理員金鑰，沒有升級途徑，也不能加入模組。只有你的金鑰
能變更你的 Safe。

## 為什麼「未經修改」這幾個字最要緊

很多錢包說自己建立在「一個 Safe」、Safe 的分叉版本，或受 Safe 啟發的帳戶上。差別體現在三方面。

**稽核涵蓋的正是你在用的東西。** Safe 的稽核涵蓋的就是這些版本，或只差少量有記錄變更的早期版本
（詳見[稽核頁面](/zh-TW/docs/security-audits)）。分叉版本的稽核針對的是分叉之前的程式碼；改動的那部分，
沒有人稽核過。

**整個生態系把你的帳戶當成 Safe，因為它就是 Safe。** 區塊瀏覽器能解析它，Safe 的工具能讀取它、
替它組出交易。但要替這些交易*簽署*，程式必須能向你的金鑰要一個給 `getvela.app`（你的
密碼金鑰所屬的網域）的簽章——所以從別的網域提供服務的 Safe 官方網頁應用程式，無法替你簽署。
哪些方式可以，請看[自架指南](/zh-TW/docs/self-hosting#if-getvela-app-disappears)。

**攻擊面有很多人一起盯著。** 自己打造的帳戶合約，基本上只有作者在看。Safe 的核心合約，每個把錢放在
Safe 裡的人都在看；4337 模組和密碼金鑰模組的關注者少一些，但確實有人在看。

## 代價

採用標準不是沒有成本的：

- **Gas。** 你的簽章要在鏈上驗證，交易還要經過 EntryPoint。依照我們 2026 年 9 月在 Gnosis 上的實測，
  從已部署的 Vela 錢包做一次簡單轉帳，鏈上大約消耗 140,000 到 170,000 gas；一般帳戶轉一筆 ETH 只要 21,000。
  除了 gas，中繼還會收取手續費——請看[網路與費用](/zh-TW/docs/networks-and-fees)。
- **帳戶必須部署。** 你的位址是在鏈上什麼都還沒有時就用 `CREATE2` 算好的，所以馬上就能收款；
  你在每條網路上的第一筆轉出交易，會支付部署合約的費用。
- **不是每條鏈都符合條件。** 密碼金鑰簽章是用 **EIP-7951 / RIP-7212** 預編譯合約驗證的，而它的位址是每個錢包初始化資料的
  一部分，所以沒有它的網路完全無法執行 Vela。
- **Safe 的風險就是你的風險。** 信任一份被廣泛使用的合約，依然是在信任一份合約。Vela 沒有在
  你資金的路徑上再加一份自己的合約要你信任。

## 哪些稽核過，哪些沒有

Safe 的合約、它的 4337 模組和密碼金鑰模組，以及 EntryPoint v0.7，都有公開的第三方稽核報告。
**Vela 自己的程式碼——各個應用程式、後端服務和註冊表合約——沒有經過第三方稽核，目前也沒有排定時程**；
這是專案有能力出資時的目標，而不是一個有日期的承諾。每份合約、它的稽核報告，以及我們在追蹤的
問題，都列在[稽核與已知問題](/zh-TW/docs/security-audits)裡。

## 自己去看

你的帳戶在鏈上。錢包部署之後，在區塊瀏覽器裡打開你的位址：它是一個 Safe 代理合約，實作合約
是 Safe 官方部署的 SafeL2 v1.4.1，每條網路上都一樣。

下一步：[稽核與已知問題](/zh-TW/docs/security-audits)。
