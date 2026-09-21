---
title: 審計與已知問題
description: "Vela 依賴的每一份合約、誰審計了哪個版本、審計的版本是否就是部署的版本、我們正在關注的未解決發現，以及哪些東西根本沒有經過審計。"
source: 47e7102c4188
---

「經過審計」是針對特定版本的特定程式碼而言的，所以本頁會引用具體的報告、提交紀錄和部署地址——也會列出
**沒有**經過審計的東西，這同樣重要。

最後核對日期：2026 年 9 月 22 日。如發現錯誤，請告訴我們，我們會更正。

## 資金路徑

每一份能接觸你資金的合約，都是第三方程式碼的官方部署，並且有公開的審查報告。

### Safe v1.4.1——帳戶本身

你的錢包是一個 [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) 代理合約，使用 SafeL2
單例合約（singleton）和 SafeProxyFactory。批量交易經由 MultiSend 執行。

[Ackee Blockchain 審計了 Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
（最終報告 2023 年 3 月 16 日，修正覆核 3 月 28 日）：共 11 項發現，沒有嚴重或高風險級別；兩項中風險發現
獲確認但沒有修改。審計範圍是 SafeL2、SafeProxyFactory、CompatibilityFallbackHandler、MultiSendCallOnly 和
SignMessageLib。v1.4.1 與 v1.4.0 只有一行功能性差異，是模組設定中的一項 ERC-4337 兼容性修正
（[PR #572](https://github.com/safe-global/safe-smart-account/pull/572)）；Safe 諮詢了 Ackee，結論是無需
重新審計。MultiSend 的邏輯自 v1.3.0 起沒有改變，而 v1.3.0 由
[G0 Group 審計](https://github.com/safe-global/safe-smart-account/tree/main/docs)。所有地址都與
[safe-deployments](https://github.com/safe-global/safe-deployments) 相符。核心合約屬於
[Safe Foundation 漏洞賞金計劃](https://docs.safefoundation.org/security/bug-bounty)的範圍，最高一級的賞金
可達 1,000,000 美元。

2025 年的 Bybit 事件並非合約層面的發現：攻擊者篡改了 Safe 網頁介面所載入的 JavaScript，而 Safe 的
[調查聲明](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)沒有發現合約存在漏洞。
[我們關於此事的頁面](/zh-HK/docs/bybit-attack)解釋了為甚麼同一類攻擊與每一個錢包介面都有關，包括我們的。

### Safe4337Module v0.3.0——ERC-4337 轉接器

部署於 `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`（Sourcify 完全相符），同時也被設為你 Safe 的後備處理器
（fallback handler）。它經過三次審查——
[報告在此](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)：

- **Ackee Blockchain**，最終報告 2024 年 3 月：一項警告（使用編譯器優化器）已獲確認，沒有更高級別的未解決
  問題。
- **Certora**，2026 年 8 月：一項**中風險**發現，已獲確認，但在 v0.3.0 中**沒有修正**——*授權變更不會令
  同一批次中已通過驗證的後續 UserOperation 失效*。見下文「已知問題」。
- **Nethermind**，2026 年 8 月：沒有發現。

在錢包部署時啟用該模組的 SafeModuleSetup v0.3.0（`0x2dd6…5b47`），也在 Certora 和 Nethermind 的審查範圍內。

這個模組曾有一個已披露的問題：v0.1.0 沒有簽署 `initCode` 和 `paymasterAndData`，構成一個惡意消耗 gas
的攻擊途徑，[已在 v0.2.0 修正](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)；
據 Safe 表示，v0.1.0 沒有在測試網以外使用。Vela 使用 v0.3.0，配合 EntryPoint v0.7 和 Safe 1.4.1，正是
該模組發佈說明所描述的配置。

### Safe 通行密鑰模組 v0.2.1——簽署器

你的第一把鑰匙由位於 `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` 的 **SafeWebAuthnSharedSigner** 驗證。
「共享」指的是合約部署是共享的，就像 Safe 單例合約一樣；你的鑰匙並不共享。每個 Safe 都把自己的 P-256
公鑰存放在自己的儲存空間中。

其餘每把鑰匙都有自己的簽署器合約，由位於 `0x1d31F259eE307358a26dFb23EB365939E8641195` 的
**SafeWebAuthnSignerFactory** 建立，作為指向位於 `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` 的
**SafeWebAuthnSigner 單例合約**的代理。

涵蓋這些合約 v0.2.1 版本的審查
（[報告](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)）：

- 一次 [Hats Finance 審計競賽](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  （2024 年 6 月至 7 月）：沒有高風險或中風險發現；三項低風險，全部已修正。
- **Certora** 對發佈提交的審查：沒有新發現。（較早的 v0.2.0 審計註明共享簽署器當時尚未審計——它是在那次
  審計之後才加入的。）
- **Nethermind**，2026 年 8 月：沒有發現。

發佈以來沒有披露過合約層面的漏洞，通行密鑰合約也屬於 Safe Foundation 賞金計劃的範圍。

通行密鑰簽名由鏈上的 **RIP-7212** 預編譯合約驗證，沒有後備驗證器。啟用一條網絡之前，App 會用一個真實
簽名檢查預編譯合約。有兩點需要留意：原本的 RIP-7212 規範有一些邊緣情況的缺陷，已由
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) 修正（只影響本來就應該驗證失敗的輸入，不影響格式正確
的 WebAuthn 簽名）；另外，一次探測無法發現某條鏈的實作可能出現偏差的所有情況。

### EntryPoint v0.7——執行你的操作

部署於 `0x0000000071727De22E5E9d8BAf0edAc6f37da032`，即
[官方 v0.7.0 版本](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)。由
[OpenZeppelin 為以太坊基金會審計](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
（2024 年 1 月）：沒有嚴重或高風險發現，五項中風險，全部 24 項發現均已解決；修正覆核的提交與發佈版本一致。
它屬於以太坊基金會 [ERC-4337 漏洞賞金計劃](https://docs.erc4337.io/community/bug-bounty)的範圍（最高
250,000 美元）。

## 我們正在關注的已知問題

### 同一批次內的授權變更（Safe4337Module，Certora M-01）

EntryPoint 會先驗證一個批次中的所有操作，然後才執行其中任何一個。所以如果某個操作移除了一個擁有者，一個
由該擁有者簽署、排在同一批次後面的操作仍然會通過驗證並執行。Safe 確認了這一點，沒有修改 v0.3.0。

Vela 的 App 從不建構更換擁有者的操作，所以 Vela 本身不會觸發這個問題。但它仍然重要：dApp 可以請求你的
錢包更改它自己的擁有者（見下文「缺口」），而任何人透過其他 Safe 工具移除一把已外洩的鑰匙時，也不能指望
它在同一批次中就被切斷。

### 已簽署操作被攔截（v0.9 之前的 EntryPoint）

2026 年 2 月，研究人員[披露](https://erc4337.substack.com/p/improving-useroperation-execution)了一種影響
v0.9 之前所有 EntryPoint（包括 v0.7）的惡意干擾及審查手法。有人若在操作上鏈前取得已簽署的操作，就可以
在自己控制的呼叫中執行它，並迫使內部執行回退：操作失敗，需要重新簽署。（在 Vela 的帶內手續費模式下，
手續費轉賬也會隨之回退，所以承擔 gas 的是中繼而不是你。）它影響的是呼叫有重入保護的合約、或可因暫時
狀態而被迫回退的操作；簡單轉賬不受影響。若反覆針對提款流程使用，可能令資金在一段時間內無法動用。它無法
偽造簽名，也無法改變資金去向。

Vela 的中繼直接提交操作，而不是經過共享的記憶池（mempool）；但待處理的 `handleOps` 交易在公共記憶池中
仍然可見，所以這只是縮小了風險範圍，並沒有消除。修正只存在於 EntryPoint v0.9（2025 年 11 月）；v0.7
無法修補。遷移要視乎 Safe 的 4337 模組何時支援 v0.9，屆時本頁會說明。

### Vela 自身防禦的缺口

這些不是合約層面的發現，而是錢包對你的保護不及你可能以為的地方。每一項都已記錄在案，有待修正：

- **由你的錢包呼叫它自己的操作不會被攔截。**dApp 可以請求在你自己的 Safe 上執行 `enableModule`、
  `addOwnerWithThreshold`、`setFallbackHandler` 或 `setGuard`；其中任何一個，只要簽署一次，就會交出帳戶。
  Vela 會解碼這些呼叫，但不會阻止。請拒絕任何目標是你自己地址的請求。
- **授權防護只會攔截「無限」金額**（2^200 或以上；Permit2 為 2^152）。金額龐大但有上限的授權、簽名式
  授權，或 NFT 的 `setApprovalForAll`，只會顯示警示，不會被攔截。
- **取得的描述檔沒有經過認證。**來自鏈數據伺服器的描述檔只要與合約相符，就會顯示為「已驗證」；它的可信
  程度只等於那部伺服器。
- **獨立簽名頁尚未接入**任何 App。
- **網絡檢查不會查找 Safe 的通行密鑰簽署器工廠**，而第二至第七把鑰匙需要它；在沒有它的情況下加入的網絡
  上，只有第一把鑰匙能簽署。
- **網站在通行密鑰所屬的同一域名上載入第三方分析程式碼。**網站透過 Permissions-Policy 標頭禁止自己的頁面
  使用通行密鑰，並且不會在持有鑰匙的頁面上載入該程式碼。

## 沒有經過審計的部分

- **Vela 自己的合約。**位於 `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` 的
  [公鑰註冊表](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)（Gnosis；在以太坊和 Base
  上地址相同）、位於 `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf` 的最初註冊表部署（它的地址是每次登記的
  簽署域的一部分），以及被它們取代的早期索引（`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`，只作唯讀
  歷史紀錄）。它們都沒有經過審計。它們不持有資金、沒有擁有者，也不能升級；它們是發現層，而不是授權層。
  動用資金的權力只來自你 Safe 中設定的鑰匙。現實中最壞的情況，是錢包在新裝置上較難被找到，而不是資金
  被轉走。
- **Multicall3。**它的 README [寫明](https://github.com/mds1/multicall3)「This contract is unaudited.」
  Vela 只用它進行批量讀取——餘額、代幣資料、報價——從不涉及授權或資金。
- **確定性部署器**（Arachnid 的 CREATE2 代理和 Safe 的單例工廠）——業界標準、無狀態，沒有正式審計。
  如果它們不存在，Vela 的網絡檢查會直接判定不通過；檢查的是該地址上是否有程式碼，而不是逐個位元組比對。
- **Tempo。**24 條內置網絡之一，沒有原生幣；Vela 在那裏用 pathUSD 穩定幣支付 gas。截至 2026 年 9 月，
  Tempo 的[安全政策](https://github.com/tempoxyz/.github/blob/main/SECURITY.md)表示協議仍在審計中，也沒有
  生效中的漏洞賞金計劃。存放在 Tempo 上的資金，以及在那裏支付的 gas，都帶有這種鏈層面的風險；請把它視為
  列表中最新、最未經考驗的一條鏈。
- **Vela 本身。**各個 App、後端服務和上述合約都沒有經過第三方審計，目前亦沒有排期。這是本頁最大的保留。
  詳情見 [Vela is in alpha](/blog/vela-is-in-alpha)。請先用小額，並閱讀程式碼。

## 親自核對

以下每一個地址都是公開的官方部署。你可以對照
[safe-deployments](https://github.com/safe-global/safe-deployments)、
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
和 [EntryPoint 發佈頁面](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)核實：

| 合約                                      | 地址                                         |
| ----------------------------------------- | -------------------------------------------- |
| SafeL2 單例合約 v1.4.1                    | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                   | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                          | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹     | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                    | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                     | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1           | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1          | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| SafeWebAuthnSigner 單例合約 v0.2.1        | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                           | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| 公鑰註冊表（Vela，未經審計）              | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ 加入網絡時會檢查它；你的 Safe 實際以 4337 模組作為後備處理器。
