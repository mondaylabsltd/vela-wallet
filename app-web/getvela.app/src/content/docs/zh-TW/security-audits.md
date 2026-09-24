---
title: 稽核與已知問題
description: "Vela 依賴的每一份合約、誰稽核了哪個版本、稽核的版本是否就是部署的版本、我們在關注的未解決問題，以及哪些東西根本沒有稽核過。"
source: d0bb95c016da
---

「經過稽核」是針對特定版本、特定程式碼的說法，所以這一頁列出具體的報告、commit 和部署位址——也列出
**沒有**稽核過的東西，這一樣重要。

最近一次核對：2026 年 9 月 22 日。如果你發現錯誤，請告訴我們，我們會修正。

## 資金路徑

每一份碰得到你資金的合約，都是第三方程式碼的官方部署，而且都有公開的審查報告。

### Safe v1.4.1——帳戶本身

你的錢包是一個 [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) 代理合約，
使用 SafeL2 實作合約（singleton）和 SafeProxyFactory。批次交易經由 MultiSend 執行。

[Ackee Blockchain 稽核了 Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
（最終報告 2023 年 3 月 16 日，修正複查 3 月 28 日）：11 項發現，沒有嚴重或高風險；兩項中風險被確認，
但沒有修改。稽核範圍是 SafeL2、SafeProxyFactory、CompatibilityFallbackHandler、MultiSendCallOnly 和
SignMessageLib。v1.4.1 和 v1.4.0 只有一行功能上的差異，是模組初始化中的 ERC-4337 相容性修正
（[PR #572](https://github.com/safe-global/safe-smart-account/pull/572)）；Safe 諮詢了 Ackee，
結論是不需要重新稽核。MultiSend 的邏輯從 v1.3.0 起就沒變過，v1.3.0 由
[G0 Group 稽核](https://github.com/safe-global/safe-smart-account/tree/main/docs)。所有位址都和
[safe-deployments](https://github.com/safe-global/safe-deployments) 一致。核心合約在
[Safe 基金會漏洞賞金計畫](https://docs.safefoundation.org/security/bug-bounty)的範圍內，最高一級
賞金可達 100 萬美元。

2025 年的 Bybit 事件不是合約層面的發現：攻擊者竄改的是 Safe 網頁介面載入的 JavaScript，Safe 的
[鑑識聲明](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)確認合約沒有漏洞。
[我們談這起事件的那一頁](/zh-TW/docs/bybit-attack)說明了為什麼同一類攻擊和每一個錢包介面都有關，包括我們的。

### Safe4337Module v0.3.0——ERC-4337 轉接模組

部署在 `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`（Sourcify 完全比對相符），同時也被設為你 Safe 的
fallback handler。它經過三次審查——
[報告在這裡](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)：

- **Ackee Blockchain**，最終報告 2024 年 3 月：一項警告（使用編譯器最佳化）已確認，沒有更高等級的
  未解決問題。
- **Certora**，2026 年 8 月：一項**中風險**發現，已確認，但在 v0.3.0 中**沒有修正**——*授權變更不會讓
  同一批次裡已通過驗證的後續 UserOperation 失效*。請看下方「已知問題」。
- **Nethermind**，2026 年 8 月：沒有發現。

在錢包部署時啟用這個模組的 SafeModuleSetup v0.3.0（`0x2dd6…5b47`），也在 Certora 和 Nethermind 的審查
範圍內。

這個模組有過一次公開揭露的問題：v0.1.0 沒有對 `initCode` 和 `paymasterAndData` 簽署，留下可被用來惡意
消耗 gas 的攻擊面，[已在 v0.2.0 修正](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)；
據 Safe 表示，v0.1.0 沒有在測試網以外使用過。Vela 使用的是 v0.3.0，搭配 EntryPoint v0.7 和
Safe 1.4.1，正是這個模組發布說明裡描述的組合。

### Safe 密碼金鑰模組 v0.2.1——簽署器

你的第一把金鑰由 **SafeWebAuthnSharedSigner** 驗證，位址 `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`。
「共用（Shared）」指的是合約部署是共用的，就像 Safe 的實作合約一樣；你的金鑰並不共用。每個 Safe 都把自己的
P-256 公鑰存在自己的儲存空間裡。

其餘每一把金鑰各有自己的簽署器合約，由 **SafeWebAuthnSignerFactory**（`0x1d31F259eE307358a26dFb23EB365939E8641195`）
建立，是指向 **SafeWebAuthnSigner 實作合約**（`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`）的代理合約。

涵蓋這些合約 v0.2.1 版本的審查
（[報告](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)）：

- 一場 [Hats Finance 稽核競賽](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  （2024 年 6 月到 7 月）：沒有高風險或中風險發現；三項低風險，全部修正。
- **Certora** 對發布版本 commit 的審查：沒有新發現。（更早的 v0.2.0 稽核有註明，共用簽署器當時還沒稽核——
  它是在那次稽核之後才加入的。）
- **Nethermind**，2026 年 8 月：沒有發現。

發布以來沒有揭露過合約層面的漏洞，密碼金鑰合約也在 Safe 基金會賞金計畫的範圍內。

密碼金鑰簽章由鏈上的 **EIP-7951 / RIP-7212** 預編譯合約驗證，沒有備用驗證器。啟用一條網路之前，應用程式會用一個真實
簽章檢查預編譯合約。有兩點保留：最初的 RIP-7212 規格有一些邊界情況的缺陷，已由
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) 修正（只影響本來就該驗證失敗的輸入，不影響格式
正確的 WebAuthn 簽章）；另外，一次探測無法抓出某條鏈的實作可能偏離規格的每一種方式。

### EntryPoint v0.7——執行你的操作

部署在 `0x0000000071727De22E5E9d8BAf0edAc6f37da032`，也就是
[官方 v0.7.0 版本](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)。
由 [OpenZeppelin 替以太坊基金會稽核](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
（2024 年 1 月）：沒有嚴重或高風險，五項中風險，全部 24 項發現都已解決；修正複查的 commit 和發布版本一致。
它在以太坊基金會 [ERC-4337 漏洞賞金計畫](https://docs.erc4337.io/community/bug-bounty)的範圍內（最高
25 萬美元）。

## 我們在關注的已知問題

### 同一批次內的授權變更（Safe4337Module，Certora M-01）

EntryPoint 會先驗證一個批次裡的所有操作，才開始逐一執行。所以如果某個操作移除了一位擁有者，同一批次中
排在後面、由這位擁有者簽署的操作，仍然能通過驗證並執行。Safe 確認了這一點，沒有修改 v0.3.0。

Vela 的應用程式從來不會組出更換擁有者的操作，dApp 若是請求一個，也會被直接拒絕，
所以 Vela 自己不會觸發這個問題。但對透過其他 Safe 工具移除一把被盜金鑰的人來說，
這件事仍然重要：他們不能指望那把金鑰在同一批次裡就被切斷。

### 已簽署的操作被攔截（v0.9 之前的 EntryPoint）

2026 年 2 月，研究人員[揭露](https://erc4337.substack.com/p/improving-useroperation-execution)了
一種影響 v0.9 之前所有 EntryPoint（包括 v0.7）的干擾與審查手法。有人只要在操作上鏈前拿到已簽署的操作，
就能在自己控制的呼叫裡執行它，並迫使內部執行回滾（revert）：操作失敗，必須重新簽署。（在 Vela 的帶內手續費
模式下，手續費轉帳會跟著回滾，所以 gas 由中繼而不是你承擔。）受影響的是會呼叫有重入保護的合約、或能被
暫時狀態弄到回滾的操作；簡單的轉帳不受影響。如果反覆針對提領流程下手，可能讓資金在一段時間內無法動用。
它無法偽造簽章，也無法改變資金去向。

Vela 的中繼是直接送出操作，而不是經過共用的交易池（mempool）；但等待上鏈的 `handleOps` 交易在公開交易池裡
仍然看得到，所以這只是縮小了暴露範圍，並沒有消除它。修正只存在於 EntryPoint v0.9（2025 年 11 月）；
v0.7 無法修補。能不能遷移，取決於 Safe 的 4337 模組什麼時候支援 v0.9，到時候這一頁會說明。

### Vela 自身防護的缺口

這些不是合約層面的發現，而是錢包對你的保護不如你可能以為的那麼多的地方。每一項都已列入待修：

- **授權防護只擋「無上限」的金額**（2^200 以上；Permit2 則是 2^152）。金額很大但有上限的授權、簽章式授權，
  或 NFT 的 `setApprovalForAll`，只會附上警示，不會被擋下。
- **獨立簽署頁還沒有接到**任何應用程式。
- **網站在密碼金鑰所屬的同一個網域上載入了第三方分析指令碼。** 網站透過 Permissions-Policy 標頭禁止自己
  的頁面使用密碼金鑰，也不會在存放金鑰的那個頁面上載入這段指令碼。

## 沒有稽核過的

- **Vela 自己的合約。** [公鑰註冊表](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`（Gnosis；以太坊和 Base 上的位址相同）、最初的註冊表部署
  `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf`（它的位址是每一次註冊簽章網域的一部分），以及被它們取代的早期索引
  （`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`，只作為唯讀的歷史紀錄）。它們都沒有經過稽核。它們不持有
  資金、沒有擁有者、無法升級；它們是用來找到錢包的一層，而不是授權的一層。動用資金的權力只來自設定在你 Safe 裡的金鑰。
  現實中最糟的情況，是錢包在新裝置上比較難被找到，而不是資金被轉走。
- **Multicall3。** 它的 README [直接寫著](https://github.com/mds1/multicall3)「This contract is unaudited.」
  Vela 只用它做批次讀取——餘額、代幣資訊、報價——從來不涉及授權或資金。
- **確定性部署器**（Arachnid 的 CREATE2 代理和 Safe 的 singleton factory）——生態系標準、沒有狀態，但沒有
  正式稽核。它們不存在時，Vela 的網路檢查會直接判定不通過；檢查的是那個位址上有沒有程式碼，而不是逐位元組比對。
- **Tempo。** 24 條內建網路之一，沒有原生幣；Vela 在那裡用 pathUSD 穩定幣支付 gas。截至 2026 年 9 月，
  Tempo 的[安全政策](https://github.com/tempoxyz/.github/blob/main/SECURITY.md)寫明協定仍在稽核中，
  也沒有正在進行的漏洞賞金計畫。你在 Tempo 上持有的資金、在那裡支付的 gas，都帶有這層鏈本身的風險；請把它當成
  清單上最新、最未經考驗的一條鏈。
- **Vela 本身。** 各個應用程式、後端服務和上面那些合約，都沒有經過第三方稽核，目前也沒有排定時程。這是這一頁
  最大的保留。詳情請看 [Vela is in alpha](/blog/vela-is-in-alpha)。請先從小額開始，並去讀程式碼。

## 自己核對

下面每一個位址都是公開的官方部署。可以對照
[safe-deployments](https://github.com/safe-global/safe-deployments)、
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
和 [EntryPoint 發布頁面](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)核對：

| 合約                                      | 位址                                         |
| ----------------------------------------- | -------------------------------------------- |
| SafeL2 實作合約 v1.4.1                    | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                   | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                          | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹     | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                    | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                     | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1           | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1          | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| SafeWebAuthnSigner 實作合約 v0.2.1        | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                           | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| 公鑰註冊表（Vela，未稽核）                | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ 新增網路時會檢查它；你的 Safe 實際上是以 4337 模組作為 fallback handler。
