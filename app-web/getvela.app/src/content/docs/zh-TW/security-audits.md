---
title: 稽核與已知問題
description: Vela 依賴的每一份鏈上合約、誰稽核的、被稽核的版本和實際部署的是否一致，以及哪些部分根本沒有稽核。
---

「稽核過」是一個關於某段特定程式碼某個特定版本的說法，所以這一頁不打算把這個詞拿來
揮一揮就算——它列出確切的報告、確切的部署位址，以及被稽核版本與被部署版本之間的
差異。它同時列出哪些**沒有**被稽核，因為後面這份清單和前面那份一樣承重。

最近一次核對：2026 年 8 月。如果你在這裡發現錯誤，告訴我們，我們會改。

## 資金路徑

有四層合約碰得到你的錢。四層全部是帶有公開稽核報告的第三方合約，而且每一處部署
位址都是官方的標準部署。

### Safe v1.4.1 —— 帳戶本身

你的錢包是一個 [Safe](https://github.com/safe-global/safe-smart-account) 代理：
SafeL2 單例、代理工廠、相容性 fallback handler，以及用於批次交易的 MultiSend。

[Ackee Blockchain 稽核了 Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
（最終報告為 2023 年 3 月）：11 項發現，沒有關鍵級或高風險。我們部署的 v1.4.1 與
被稽核的 v1.4.0 之間，只差一處單行的 ERC-4337 相容性修正
（[PR #572](https://github.com/safe-global/safe-smart-account/pull/572)）。
MultiSend 的邏輯自[經 G0 Group 稽核的 v1.3.0](https://github.com/safe-global/safe-smart-account/tree/main/docs)
以來沒有變過。所有位址都與
[safe-deployments](https://github.com/safe-global/safe-deployments) 裡的標準部署一致，
而且這些合約在 [Safe Foundation 的漏洞獎金](https://docs.safefoundation.org/security/bug-bounty)
範圍內（關鍵級最高 100 萬美元）。

有一件事稽核涵蓋不到：2025 年的 Bybit 事件。那次攻擊攻陷的是 Safe 官方網頁前端的
建置流程，而不是合約——
[官方鑑識結論](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
認為 Safe 智慧合約中沒有漏洞。我們把它讀成一堂關於網頁與維運層的課，而那一層，
正是你也應該拿來檢視我們的地方。

### Safe4337Module v0.3.0 —— ERC-4337 轉接層

部署在 `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`，也就是 v0.3.0 的標準位址
（Sourcify 精確比對——鏈上位元組碼就是被稽核的那份程式碼）。
[由 Ackee Blockchain 稽核](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
（最終報告為 2024 年 3 月），沒有任何高於「提示級」的未解決發現。我們使用的
v0.3.0 + EntryPoint v0.7 + Safe ≥1.4.1 這個組合，正是稽核報告與發行說明所描述的配置。

這個模組的歷史上有一個已揭露的問題：v0.1.0（2023 年）沒有對 `initCode` 和
`paymasterAndData` 簽章，構成一個 gas 騷擾向量。它
[已在 v0.2.0 修正](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)，
而且 v0.1.0 從未離開測試網。我們用的是 v0.3.0，它繼承了這個修正。

### SafeWebAuthnSharedSigner v0.2.1 —— 密碼金鑰簽署器

部署在 `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`，也就是 v0.2.1 的標準位址
（透過 Safe 的單例工廠，在每條鏈上都相同）。

「shared（共用）」的意思是什麼、不是什麼：共用的是*這份合約部署*，就像 Safe 的
單例被共用一樣。你的金鑰並不共用。每一個 Safe 都透過 delegatecall 呼叫
`configure()`，把自己的 P-256 公鑰存進自己的儲存空間。一個簽署器實體對應的，
正好是每個 Safe 的一把密碼金鑰，別人的 Safe 用不了你的。

這裡版本很重要。v0.2.0 的稽核報告
[明確寫明](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
共用簽署器不在範圍內——當時這份合約還不存在。涵蓋我們所部署內容的，是 v0.2.1 的
那幾份：一場 [Hats Finance 稽核競賽](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
（2024 年 6–7 月：零高風險、零中風險、三項低風險——全部已修正），加上
[Certora 對發行 commit 的複核](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)，
沒有新的發現。自發行以來沒有任何合約層級的漏洞被揭露；密碼金鑰相關合約也在
Safe Foundation 的獎金範圍內。

Safe 自己的文件建議：把密碼金鑰的擁有權與一條復原路徑搭配使用，而不是把單一憑證
當成帳戶唯一的金鑰。Vela 怎麼處理這件事，記在[復原與登入](/zh-TW/docs/recovery)裡。

鏈上的 P-256 驗證直接使用 RIP-7212 預編譯，沒有 Solidity 的後備驗證器。在啟用任何
網路之前，應用都會用一個真實簽章去探測這個預編譯，驗證失敗就拒絕該網路。有兩句
老實話：最初的 RIP-7212 規範存在一些邊界情況缺陷，
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) 正是為了修正它們而寫
（它們不影響格式正確的 WebAuthn 簽章）；而一次探測，也不可能涵蓋某條鏈的實作在
不尋常執行情境下可能出現偏差的每一種方式。

### EntryPoint v0.7 —— ERC-4337 的進入點

部署在 `0x0000000071727De22E5E9d8BAf0edAc6f37da032`，也就是
[v0.7.0 的標準部署](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)。
[由 OpenZeppelin 稽核](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
（受以太坊基金會委託，2024 年 1 月）：零關鍵級、零高風險、五項中風險，全部已解決
——而且被稽核的那個 commit 就是被部署的版本。EntryPoint v0.7.0 在以太坊基金會的
[ERC-4337 漏洞獎金](https://docs.erc4337.io/community/bug-bounty)範圍內
（最高 25 萬美元）。

## 我們正在盯著的已知問題

### EntryPoint 的騷擾向量

2026 年 2 月，Trust Security 的資安研究員
[揭露](https://erc4337.substack.com/p/improving-useroperation-execution)
了一個影響 v0.9 之前所有 EntryPoint 的騷擾與審查向量，其中包括我們在用的 v0.7。
攻擊者如果在一個已簽署的 UserOperation 被打包之前攔截它，就可以把它放進自己控制的
呼叫框架裡執行，並強制內層執行回復——這筆操作失敗了，但 gas 仍然被扣掉。以太坊
基金會為這個發現支付了 5 萬美元獎金；基金會將它歸類為審查／騷擾向量，而不是竊取
資金的向量，而且它從未被實際利用過。

它做得到的：浪費一筆手續費、拖延一筆交易。它做不到的：偷走資金，或是偽造簽章。
Vela 的暴露面很窄，因為 UserOperation 是直接送到中繼的，不經過公開的交易池，
所以幾乎沒有攔截的機會——而最壞情況也被你早就同意的那筆費用框住。修正只存在於
EntryPoint v0.9（2025 年 11 月）；v0.7 本身沒辦法打修補。我們預期會隨著周邊技術堆疊
——特別是 Safe 的 4337 模組系列——支援 v0.9 而遷移過去，到那時會在這裡說明。

## 哪些沒有稽核

- **Vela 自己的合約。** 我們自己寫的兩份小合約，部署在 Gnosis 上：
  [密碼金鑰公鑰索引](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  （一份只追加的登記表，幫你的裝置找到你的公鑰）和它的批次輔助合約。它們沒有經過
  稽核。從結構上說，它們不持有任何資金、沒有擁有者，也不能升級——它們是一層發現
  機制，不是一層授權機制。花錢的權限永遠來自設定在你 Safe 內部的那把密碼金鑰。
  現實中最壞的故障是騷擾（有人搶佔一筆索引紀錄），這會讓復原變得比較不方便，
  但動不了錢。早期費用設計裡的那份 gas 結算拆分合約，已經不在交易流程中。
- **Multicall3。** 它自己的 README [直說了](https://github.com/mds1/multicall3)：
  「本合約未經稽核。」我們使用它的方式，正是其作者描述為安全的那一種——批次唯讀
  呼叫，用於讀取餘額、代幣中繼資料和價格。Vela 從不給它任何授權，它也從不持有資金。
  出錯的最壞情況是讀到一個不正確的數值。
- **CREATE2 部署器。**
  [Arachnid 確定性部署代理](https://github.com/Arachnid/deterministic-deployment-proxy)
  是生態標準的無狀態部署器；它沒有正式稽核。如果某條鏈上它缺失或被改動過，
  我們的網路檢查會直接判定失敗。
- **Tempo 與 pathUSD。** Tempo 是我們十二條內建網路之一，它沒有原生幣；那裡的 gas
  用 pathUSD 穩定幣結算。截至 2026 年 8 月，Tempo 的核心協定和 pathUSD 都沒有公開的
  安全稽核，也沒有漏洞獎金，而且一份獨立的
  [DefiLlama 擔保品評估](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  （2026 年 4 月）把 pathUSD 評為高風險。這是鏈層級的風險，任何錢包都無法緩解：
  你放在 Tempo 上的資金，以及在那裡的 gas 結算，都會繼承它。請把 Tempo 當成這份
  名單上最新、也最未經考驗的一條鏈，並據此控制你的餘額規模。等稽核發布，
  我們會更新這一節。
- **Vela 本身。** 我們的應用和後端服務沒有做過第三方稽核。這是這一頁上最大的一條
  保留意見，我們把它寫在網站頂部，老實的細節在
  [Vela 仍在 alpha](/blog/vela-is-in-alpha)。請從小額開始。去讀程式碼。

## 自己去查核

上面每一個位址都是公開的標準部署，你可以對照官方登記查核——
[safe-deployments](https://github.com/safe-global/safe-deployments)、
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
以及 [EntryPoint 的發行說明](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)：

| 合約 | 位址 |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| 密碼金鑰公鑰索引（Gnosis） | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
