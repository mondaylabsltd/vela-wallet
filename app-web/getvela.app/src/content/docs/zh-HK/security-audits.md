---
title: 審計同已知問題
description: Vela 依賴嘅每一份鏈上合約、邊個審計、被審計嘅版本同實際部署嘅係咪一致，以及邊啲部分根本冇審計。
---

「審計過」係一個關於某段特定代碼某個特定版本嘅講法，所以呢一頁唔會攞呢個詞嚟揈兩下
就算——佢列出確切嘅報告、確切嘅部署地址，以及被審計版本同被部署版本之間嘅差異。
佢同時列出邊啲**冇**被審計，因為後面呢份清單同前面嗰份一樣咁重要。

最近一次核對：2026 年 8 月。如果你喺呢度搵到錯，話我哋知，我哋會改。

## 資金路徑

有四層合約掂得到你啲錢。四層全部係有公開審計報告嘅第三方合約，而且每一處部署地址
都係官方嘅標準部署。

### Safe v1.4.1 —— 帳戶本身

你個錢包係一個 [Safe](https://github.com/safe-global/safe-smart-account) 代理：
SafeL2 單例、代理工廠、兼容性 fallback handler，以及用嚟批次交易嘅 MultiSend。

[Ackee Blockchain 審計咗 Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
（終版報告係 2023 年 3 月）：11 項發現，冇關鍵級或者高危。我哋部署嘅 v1.4.1 同被
審計嘅 v1.4.0 之間，淨係差一處單行嘅 ERC-4337 兼容性修正
（[PR #572](https://github.com/safe-global/safe-smart-account/pull/572)）。
MultiSend 嘅邏輯由[經 G0 Group 審計嘅 v1.3.0](https://github.com/safe-global/safe-smart-account/tree/main/docs)
以嚟冇變過。所有地址都同
[safe-deployments](https://github.com/safe-global/safe-deployments) 入面嘅標準部署
一致，而且呢啲合約喺 [Safe Foundation 嘅漏洞賞金](https://docs.safefoundation.org/security/bug-bounty)
範圍內（關鍵級最高 100 萬美元）。

有一樣嘢審計覆蓋唔到：2025 年嘅 Bybit 事件。嗰次攻擊攻陷嘅係 Safe 官方網頁前端嘅
建置流程，唔係合約——
[官方鑑證結論](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
認為 Safe 智能合約入面冇漏洞。我哋將佢讀成一堂關於網頁同維運層嘅課，而嗰一層，
正正就係你都應該攞嚟審視我哋嘅地方。

### Safe4337Module v0.3.0 —— ERC-4337 轉接層

部署喺 `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`，即係 v0.3.0 嘅標準地址
（Sourcify 精確比對——鏈上位元組碼就係被審計嗰份代碼）。
[由 Ackee Blockchain 審計](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
（終版報告係 2024 年 3 月），冇任何高過「提示級」嘅未解決發現。我哋用嘅
v0.3.0 + EntryPoint v0.7 + Safe ≥1.4.1 呢個組合，正正係審計報告同發布說明所描述嘅
配置。

呢個模組嘅歷史上有一個已披露嘅問題：v0.1.0（2023 年）冇為 `initCode` 同
`paymasterAndData` 簽名，構成一個 gas 騷擾向量。佢
[已喺 v0.2.0 修正](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)，
而且 v0.1.0 從來冇離開過測試網。我哋用嘅係 v0.3.0，佢繼承咗呢個修正。

### SafeWebAuthnSharedSigner v0.2.1 —— 通行密鑰簽署器

部署喺 `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`，即係 v0.2.1 嘅標準地址
（經 Safe 嘅單例工廠，喺每條鏈上面都一樣）。

「shared（共用）」係乜意思、唔係乜意思：共用嘅係*呢份合約部署*，就好似 Safe 嘅
單例俾人共用咁。你把鎖匙冇共用。每一個 Safe 都經 delegatecall 呼叫 `configure()`，
將自己嘅 P-256 公鑰擺入自己嘅儲存空間。一個簽署器實例對應嘅，啱啱好就係每個 Safe
一把通行密鑰，人哋個 Safe 用唔到你嗰把。

呢度版本好緊要。v0.2.0 嘅審計報告
[明確寫明](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
共用簽署器唔喺範圍內——嗰陣呢份合約仲未存在。覆蓋我哋所部署內容嘅，係 v0.2.1 嗰幾份：
一場 [Hats Finance 審計比賽](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
（2024 年 6–7 月：零高危、零中危、三項低危——全部已修正），加上
[Certora 對發布 commit 嘅複核](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)，
冇新發現。由發布以嚟冇任何合約級漏洞俾人披露；通行密鑰相關合約亦喺 Safe Foundation
嘅賞金範圍內。

Safe 自己嘅文件建議：將通行密鑰嘅擁有權同一條復原路徑夾埋用，而唔係當單一憑證係
帳戶唯一嘅鎖匙。Vela 點處理呢件事，記喺[復原同登入](/zh-HK/docs/recovery)。

鏈上嘅 P-256 驗證直接用 RIP-7212 預編譯，冇 Solidity 嘅後備驗證器。喺啟用任何網絡
之前，應用都會用一個真實簽名去探測呢個預編譯，驗證失敗就拒絕嗰條網絡。有兩句老實
話：最初嘅 RIP-7212 規範有啲邊緣情況缺陷，
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) 就係為咗修正佢哋而寫
（佢哋唔影響格式正確嘅 WebAuthn 簽名）；而一次探測，亦唔可能覆蓋某條鏈嘅實作喺
唔尋常執行情境下可能有偏差嘅每一種方式。

### EntryPoint v0.7 —— ERC-4337 嘅入口點

部署喺 `0x0000000071727De22E5E9d8BAf0edAc6f37da032`，即係
[v0.7.0 嘅標準部署](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)。
[由 OpenZeppelin 審計](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
（受以太坊基金會委託，2024 年 1 月）：零關鍵級、零高危、五項中危，全部已解決——
而且被審計嗰個 commit 就係被部署嘅版本。EntryPoint v0.7.0 喺以太坊基金會嘅
[ERC-4337 漏洞賞金](https://docs.erc4337.io/community/bug-bounty)範圍內
（最高 25 萬美元）。

## 我哋盯緊嘅已知問題

### EntryPoint 嘅騷擾向量

2026 年 2 月，Trust Security 嘅保安研究員
[披露](https://erc4337.substack.com/p/improving-useroperation-execution)
咗一個影響 v0.9 之前所有 EntryPoint 嘅騷擾同審查向量，包括我哋用緊嘅 v0.7。
攻擊者如果喺一個已簽名嘅 UserOperation 上鏈之前截到佢，就可以將佢放入自己控制嘅
呼叫框架度執行，並強制內層執行回滾——呢筆操作失敗咗，但 gas 照扣。以太坊基金會
為呢個發現畀咗 5 萬美元賞金；基金會將佢歸類為審查／騷擾向量，而唔係偷錢嘅向量，
而且佢從來冇俾人真正利用過。

佢做得到嘅：嘥一筆手續費、拖延一筆交易。佢做唔到嘅：偷走資金，或者偽造簽名。
Vela 嘅暴露面好窄，因為 UserOperation 係直接送去中繼嘅，唔經公開記憶池，
所以幾乎冇機會俾人截——而最壞情況亦由你早就同意咗嗰筆費用封頂。修正淨係存在於
EntryPoint v0.9（2025 年 11 月）；v0.7 本身補唔到。我哋預計會隨住周邊技術棧——
特別係 Safe 嘅 4337 模組系列——支援 v0.9 而遷移過去，到時會喺呢度講。

## 邊啲冇審計

- **Vela 自己嘅合約。** 我哋自己寫嘅兩份細合約，部署喺 Gnosis：
  [通行密鑰公鑰索引](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  （一份只追加嘅登記表，幫你啲裝置搵到你個公鑰）同佢嘅批次輔助合約。佢哋冇經過
  審計。由結構上講，佢哋唔持有任何資金、冇擁有者，亦升唔到級——佢哋係一層發現機制，
  唔係一層授權機制。使錢嘅權限永遠嚟自設定喺你個 Safe 入面嗰把通行密鑰。現實中最壞
  嘅故障係騷擾（有人霸咗一條索引紀錄），咁會令復原冇咁方便，但郁唔到錢。早期費用
  設計嗰份 gas 結算拆分合約，已經唔喺交易流程入面。
- **Multicall3。** 佢自己個 README [直說](https://github.com/mds1/multicall3)：
  「本合約未經審計。」我哋用佢嘅方式，正正係佢作者形容為安全嗰種——批次唯讀呼叫，
  用嚟讀餘額、代幣中繼資料同價格。Vela 從來唔會畀佢任何授權，佢亦從來唔持有資金。
  出 bug 嘅最壞情況係讀到一個唔啱嘅數值。
- **CREATE2 部署器。**
  [Arachnid 確定性部署代理](https://github.com/Arachnid/deterministic-deployment-proxy)
  係生態標準嘅無狀態部署器；佢冇正式審計。如果某條鏈上面佢唔見咗或者俾人改過，
  我哋嘅網絡檢查會直接判失敗。
- **Tempo 同 pathUSD。** Tempo 係我哋十二條內置網絡之一，佢冇原生幣；嗰度嘅 gas
  用 pathUSD 穩定幣結算。截至 2026 年 8 月，Tempo 嘅核心協定同 pathUSD 都冇公開嘅
  保安審計，亦冇漏洞賞金，而且一份獨立嘅
  [DefiLlama 抵押品評估](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  （2026 年 4 月）將 pathUSD 評為高風險。呢個係鏈一級嘅風險，任何錢包都緩解唔到：
  你放喺 Tempo 上面嘅錢，以及喺嗰度嘅 gas 結算，都會繼承佢。請將 Tempo 當成呢張
  名單上面最新、亦最未經考驗嗰條鏈，並據此控制你嘅餘額規模。等審計出咗，
  我哋會更新呢一節。
- **Vela 本身。** 我哋嘅應用同後端服務冇做過第三方審計。呢個係呢一頁上面最大嗰條
  保留，我哋將佢寫喺網站頂部，老實嘅細節喺
  [Vela 仲喺測試階段](/blog/vela-is-in-alpha)。請由細額開始。去讀代碼。

## 自己去查

上面每一個地址都係公開嘅標準部署，你可以對住官方登記去查——
[safe-deployments](https://github.com/safe-global/safe-deployments)、
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
同 [EntryPoint 嘅發布說明](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)：

| 合約 | 地址 |
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
| 通行密鑰公鑰索引（Gnosis） | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
