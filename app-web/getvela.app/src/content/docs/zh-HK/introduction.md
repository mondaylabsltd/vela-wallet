---
title: 簡介
description: "用六行說明 Vela 是甚麼，再列出大家最常帶着來的問題——每個問題都直接連結到答案。"
source: f1879437a0b5
---

# Vela 說明文件

Vela 是一個**開源、可自行架設、適用於以太坊及其他 EVM 網絡的錢包**，無需助記詞。你的錢包是一個未經修改的
[Safe](/zh-HK/docs/account-contract) 智能帳戶，你用**通行密鑰**簽署——可以在你的手機或電腦上、
在另一部手機上，或在一把硬件安全密鑰上。

- **不靠我們也能運作。**App 以及背後的每一項服務——中繼、公鑰索引、鏈數據和匯率服務——都採用 MIT 授權。按照[自行架設指南](/zh-HK/docs/self-hosting)自行編譯、自行運行；指南亦列明了限制條件。
- **你的鑰匙，最多七把。**在建立錢包時選定；其中任何一把都能簽署。Vela 從不持有這些鑰匙，在你的錢包上也沒有任何角色。
- **24 條網絡，同一個地址。**另外還可以加入任何符合要求的 EVM 網絡。
- **先看清楚，再簽署。**交易會被解碼成淺白的文字；無法解碼的部分會清楚標示。
- **Alpha 階段。**它可以正常使用、存放真實資金，但仍很年輕：請先用小額。
  [這裏的 alpha 是甚麼意思](/blog/vela-is-in-alpha)。

## 找答案

| 我想知道…… | 請看 |
| --- | --- |
| 怎樣把一切都改由自己運行 | [自行架設指南](/zh-HK/docs/self-hosting) |
| 怎樣運行自己的中繼，手續費歸誰 | [自行架設指南 → 中繼](/zh-HK/docs/self-hosting#relay) · [網絡與費用 → 手續費](/zh-HK/docs/networks-and-fees#fee) |
| getvela.app 下線了會怎樣 | [自行架設指南 → 沒有 getvela.app 時](/zh-HK/docs/self-hosting#if-getvela-app-disappears) |
| 一筆交易為甚麼收這個價錢 | [網絡與費用](/zh-HK/docs/networks-and-fees) |
| 我的鏈是否支援，或怎樣加入一條鏈 | [網絡與費用](/zh-HK/docs/networks-and-fees) · [鏈設定](/zh-HK/chain-setup) |
| Vela 有沒有經過審計 | [審計與已知問題](/zh-HK/docs/security-audits) |
| 怎樣確認自己實際簽署的是甚麼 | [清晰簽署](/zh-HK/docs/clear-signing) · [Bybit 被攻擊事件](/zh-HK/docs/bybit-attack) |
| 應該安裝哪個 App，要花多少錢 | [安裝 Vela](/zh-HK/docs/install) · [取得 Vela](/zh-HK/get-started) |
| 怎樣建立錢包，用哪些鑰匙 | [建立錢包](/zh-HK/docs/create-wallet) · [簽署鑰匙與安全密鑰](/zh-HK/docs/signers) |
| 手機遺失或刪除了通行密鑰怎麼辦 | [復原與登入](/zh-HK/docs/recovery) |
| 之後還能否加入或更換鑰匙 | [簽署鑰匙與安全密鑰](/zh-HK/docs/signers) |
| 怎樣用 Vela 連接 dApp | [安裝 Vela → dApp](/zh-HK/docs/install#dapps) |
| 我的錢包有哪些資料是公開的 | [建立錢包 → 哪些內容會公開](/zh-HK/docs/create-wallet#what-is-public) · [私隱政策](/privacy) |

## 深入閱讀

- [我們為甚麼做 Vela](/zh-HK/docs/why-vela)——來龍去脈，以及我們選擇的取捨。
- [白皮書](/zh-HK/docs/whitepaper)——架構，以及你究竟需要信任甚麼。
- [帳戶合約](/zh-HK/docs/account-contract)——你的錢存放在哪些合約裏。

[網誌](/blog)記錄了 Vela 的開發過程。
