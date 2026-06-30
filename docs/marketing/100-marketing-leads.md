# Vela Wallet — 100 条营销线索（按重要程度排序）

> 用途：为 Vela Wallet 创作营销内容、提升转化、驱动分发与曝光、帮助潜在用户做"是否使用"的决策。
> 每条线索 = 一个角度 / 钩子 + 一个**真实代码或文档可验证的事实** + 营销用法 / 转化逻辑。
> 全部线索均落地到源码、docs、whitepaper、store-listing 中，未做无依据的发挥（遵守"内容必须有据可依"原则）。
> 术语规范：品牌名（Vela）、代币符号（ETH/USDC）、技术标识（passkey、ERC-4337、Safe、RPC）保持英文，不翻译。
> **排序原则：以"潜在客户在意程度"为第一标尺**（不是产品自评的技术含量）。先读"第一节 潜在客户分析"，它定义了五类客户、各自最在意什么，并给出客户视角的真实优先级与对原排序的修正。

---

## 〇、先读：定位、受众、语气、红线

**一句话定位**：自我托管的智能合约钱包 —— 没有助记词，用人脸/指纹签名，每一笔交易都能读懂再批准。
**品牌口号**：`Your keys. Your face.`（你的钥匙，你的脸）/ `A wallet that does less — on purpose.`（一款"刻意做得更少"的钱包）。
**核心受众**：想要真正自托管、但被助记词坑过或怕被坑的人；"能解锁手机就能安全用钱包"。
**语气**：诚实优先于营销（whitepaper 原话："favors honesty over marketing"）；温暖、克制、不夸张。

**红线（任何文案都不能踩，否则伤害信任）**：
1. **不得宣称"已审计 / 审计已排期"**。Safe 合约本身经独立审计，但 Vela 自己的集成**未经第三方审计、且当前无排期**；开源 ≠ 审计。现阶段的审查 = 开源 + 社区阅读 + AI 辅助，不能等同专业审计。
2. **不用恐吓式"Beta / 请容忍 bug"横幅**。可以坦诚 alpha 阶段、"先用小额"，但用"激进透明=可信"的方式表达，而不是免责声明。
3. **dApp Connect 是扫码 / relay（WalletPair），不是蓝牙 / BLE**。早期 README 写过 BLE，已被 WalletPair WebSocket relay 取代，文案以此为准。
4. **不做价格 / 收益 / 交易 / 法币入金承诺**。Vela 无代币、无 on-ramp、无理财。
5. **网络数量以 12 为准**（`src/models/chains.ts`），早期 README 写的 8 已过时。
6. **恢复要讲"诚实的限制"**：同时丢失设备+云端 passkey 且无其他副本 = 无法找回。这是卖点不是软肋——把它讲成"我们连帮你找回的能力都没有，所以也没人能偷"。

---

## 一、潜在客户分析（最重要：先懂客户在意什么，再谈排序）

> 方法论说明：以下分群是**基于产品事实 + 加密钱包用户已知画像的推理假设**，不是访谈/问卷数据（置信度：中）。
> 落地前应用真实素材验证（见文末"研究缺口"）。但有一条是**硬约束、非假设**——它决定了"谁才是最重要的客户"：

### 0. 商业前提（硬约束）：免费 + 无代币 ⇒ 只有"用户真的发交易"才产生收入
- Vela 不收订阅、无 token，唯一收入是 **relayer fee ≈ 一倍网络 gas**（whitepaper / FAQ）。收入 ≈ **交易笔数 × gas 价 × relayer 系数**。
- **装了但不交易的用户 = ¥0 收入。** 一个躺平的持币者，价值远低于一个每天用 dApp 的活跃地址。
- **由此得出两条互不相同的客户线**，营销必须分层打：
  - **获客最易入口 = "无助记词 / 安全 / 简单"** → 吸引面最广，但很多是低频用户。
  - **变现核心 = "活跃的多链交易者"** → 人数少，但贡献绝大部分 relayer 收入。
- **战略含义**：用 S 梯队的安全/简单钩子**拉新铺量**，用"功能深度 + 信任"把其中的**活跃交易者**留下来并提升交易频率。**给营销排序时，对"能提升交易频率/敢上大额"的线索做收入加权**。

### 1. 五类潜在客户（按"战略价值 = 在意强度 × 可触达 × 变现"排序）

**Persona A — 多链 DeFi 重度玩家（变现核心，第一优先）**
- 画像：常年在 5–12 条链操作，天天用 dApp，持很多代币，被 MetaMask 的盲签/无限授权坑过或目睹朋友被 drain。中高净值、技术中上。
- 核心 JTBD：**"让我跨链安全又快地操作，别再让我签看不懂的东西。"**
- 最在意（排序）：① 拒绝盲签 / clear-signing → ② 永不无限授权 → ③ 一个地址通所有链 → ④ 交易前余额模拟 → ⑤ dApp 扫码连接 + 多链组合视图 → ⑥ RPC 不掉线 → ⑦ gas 透明 → ⑧ Split/Sweep 批量。
- 异议：支持我的链/dApp 吗？签名有 MetaMask 快吗？能自配 RPC 吗？新钱包敢放大额吗？
- 在哪找到：Crypto Twitter、DeFi Discord、安全研究员圈、Bankless 类受众。
- 命中线索：**4、16、17、8、19、18、42、54、39/40、53、73、82**。

**Persona B — 自托管转化者 / "被助记词坑过的人"（获客主力，第二优先）**
- 画像：持币，用交易所或助记词钱包，丢过/怕丢助记词或被钓鱼，想真正自托管但怕"footgun"。技术中等。
- 核心 JTBD：**"让我自己掌握资产，但别用助记词那套吓人的东西。"**
- 最在意（排序）：① 没有助记词 → ② 不会被钓鱼 → ③ 丢手机怎么办（恢复）→ ④ 公司动不了我的钱 → ⑤ 简单、人脸签名 → ⑥ 这玩意儿靠谱吗（信任）。
- 异议：**"新钱包凭什么信你？" / "丢手机是不是就全没了？" / "审计了吗？"**
- 在哪找到：X 安全话题、r/ethereum、r/CryptoCurrency、YouTube 科普、应用商店搜索。
- 命中线索：**1、5、11、3、2、33、22、23、7**。

**Persona C — 隐私 / 主权极客（信誉放大器 + 布道者，第三优先）**
- 画像：cypherpunk 倾向，可能自跑节点，极度反托管/反追踪。人数少但影响力与转发力极强。
- 核心 JTBD：**"给我一个能完全验证、能自托管、不向家里打电话的钱包。"**
- 最在意（排序）：① 端到端开源 + 三个服务全可自托管 → ② 无 KYC/邮箱/追踪 → ③ 链上报价无第三方 API → ④ 无代币 → ⑤ "如果 Vela 消失了" → ⑥ 你信什么/不信什么收敛清单。
- 异议：有没有闭源部分？有遥测吗？rpId 域名锁定怎么破？
- 在哪找到：Hacker News、Nostr、Lobsters、隐私类 subreddit、GitHub。
- 命中线索：**6、25、30、9、31、21、13、79、94**。

**Persona D — 加密新手 / 大众（漏斗顶端 + 多语言市场，第四优先）**
- 画像：刚入圈，被助记词吓退过，可能因朋友/空投/某 App 而来。非技术。
- 核心 JTBD：**"我想持有加密资产，又不想感觉随时会全部弄丢。"**
- 最在意（排序）：① 能解锁手机就能用 → ② 人脸/指纹 → ③ 免费 → ④ 免下载、浏览器即用 → ⑤ 丢手机能找回 → ⑥ 看起来不吓人。
- 异议：安全吗？会不会太复杂？我会不会把钱弄丢？
- 在哪找到：TikTok / YouTube Shorts、应用商店、熟人推荐、非英语本地化市场（中文/日韩/拉美）。
- 命中线索：**2、1、10、11、14、51、98、46**。

**Persona E — 开发者 / 建设者 / 新链 BD（分发倍增器，第五优先）**
- 画像：在评估 account abstraction / passkey 钱包；潜在集成方或自托管者；想要钱包支持的新链。
- 核心 JTBD：**"给我看一个真实、开放、工程过硬的 AA + passkey 参考实现。"**
- 最在意（排序）：① 开源代码质量 → ② ERC-4337/EIP-1271/ERC-7730/RIP-7212 标准落地 → ③ 一套代码三端 → ④ 故障注入演练台 → ⑤ Tempo 稳定币 gas 等前沿 → ⑥ 可自托管、可加自定义链。
- 异议：维护活跃吗？文档全吗？真能自托管/接我的链吗？
- 在哪找到：GitHub、HN、开发者 Twitter、ETH 会议、新链生态 BD。
- 命中线索：**6、77、34、69、70、57、78、94、99**。

### 2. 跨所有客户的头号转化障碍：「新钱包，凭什么把钱交给你」
无论投给哪类人，**信任**都是第一道闸门（钱包决策里"会不会丢钱"压倒一切）。所以**信任类线索必须前置、反复出现**：
**6（开源可自托管）、7（Safe 数十亿 TVL 背书）、3（不可冻结/没收）、9（无代币不割你）、15（诚实 alpha）、25（MIT）、31（如果我们消失）、87（私钥不出安全芯片）。**
> 关键：**"先用小额"要当卖点讲**（透明=可信），别当免责声明讲；**绝不**说"已审计/审计已排期"。

### 3. 客户在意度 ▶ 对原"产品力排序"的修正
原 100 条按"产品差异化"排，下面按"**潜在客户在意度 × 触达广度 × 变现**"给出**客户视角的真实优先级**（当两者冲突，以客户为准）：

- **应当上调（客户比产品自评更在意）**：
  - **#11 丢手机/恢复** → 进 S 梯队。这是 Persona B/D 的**头号异议**，几乎决定转化，不能埋在 A 梯队。
  - **#16 余额模拟** 与 **#17 永不无限授权** → 对 Persona A 是核心买点，建议并入第一梯队"安全演示"组。
  - **#46 到账提醒 + 触觉** → 对新手是高情绪价值的"惊喜时刻"，做 demo 极易传播，可上调。
- **应当下调（产品很自豪，但多数客户不直接在意）**：
  - 第四梯队 C 的纯工程细节（**#74 抖动、#75 去重、#76 分级超时、#84 multicall、#85 缓存**）→ 主要打动 Persona E 与做信誉背书，**对 A/B/C/D 的购买决策影响小**，保持低位、只在技术内容里用。
- **保持高位**：#1、#2、#3、#4、#5 —— 既是产品力也是客户最在意，排序无需动。

**→ 客户视角 Top 12（建议作为投放/首屏主排序）**：
1（无助记词）→ 2（人脸签名）→ 3（不可冻结）→ 11（丢手机能找回）→ 4（拒绝盲签）→ 5（无可钓鱼）→ 17（永不无限授权）→ 16（余额模拟）→ 7（Safe 背书）→ 8（一址多链）→ 9（免费无代币）→ 6（开源自托管）。

### 4. 置信度与研究缺口（诚实标注）
- 上述分群与排序为**中/低置信度推理**，未经一手数据验证。
- **建议验证来源**（真实"水坑"研究，我可代做）：竞品应用商店 1–3 星差评（MetaMask、Rainbow、Coinbase Wallet、Argent、以及 passkey/AA 钱包如 Soul/Braavos）；r/ethereum、r/CryptoCurrency；Crypto Twitter 关于"被 drain / 无限授权 / 助记词丢失"的吐槽；自家 Telegram/X 社群的真实提问。
- **要重点采集**：用户**原话词汇**（用于文案）、真实**触发事件**（什么时候开始找新钱包）、最常见**异议**（验证"新钱包凭什么信"是否真是第一障碍）。
- 需要的话，我可以按 customer-research 的"水坑研究"流程，产出带原话引用的 VOC 词库 + 验证后的人物画像。

---

## 第一梯队 S — 核心买点 / 一句话钩子（决定"要不要用"，且为客户最在意项）

**1. 没有助记词（这是头号钩子）。** 签名密钥是设备安全硬件里的 passkey，没有 12 个单词要抄、要藏、要怕丢。→ 直击加密用户最大痛点与恐惧，放在所有落地页首屏、应用商店副标题（`Self-custody, no seed phrase`）。

**2. 用人脸/指纹签名，和解锁手机同一个动作。** WebAuthn 流程 `userVerification:'required'`（`src/modules/passkey/index.ts`），每次签名都走系统生物识别。→ "If you can unlock your phone, you can use Vela." 降低"加密很难"的心理门槛，主打小白也敢用。

**3. 自我托管：Vela 无法转移、冻结、没收你的资金。** whitepaper "What Vela cannot do" 明确列出——只有你的 passkey 能授权 Safe。→ 对标托管型钱包/交易所的最强差异化；FTX 之后这是最有共鸣的信任叙事。

**4. 拒绝盲签：每笔交易解析成人类可读意图（ERC-7730）。** 未知调用被**标记**而非隐藏（whitepaper、`docs/clear-signing-design.md`）。→ "盲签是钱包被盗的最大来源之一"——把行业痛点变成你的卖点，对安全敏感用户极具说服力。

**5. 没有任何东西可被钓鱼。** passkey 不是"能输入的秘密"，钓鱼网站无法让你"输入 passkey"（`docs/passkeys.md`）。→ "Nothing to phish." 一句话讲清为什么比助记词钱包更安全，适合做对比图/短视频。

**6. 端到端开源 + 可自托管。** App 与全部三个后端服务（chain data、passkey index、bundler）MIT 开源，设置内可换自建端点（Settings → Advanced → Service Endpoints）。→ "Verify, don't trust." 面向硬核用户与技术 KOL 的信誉背书。

**7. 每个钱包都是真正的 Safe 智能账户（v1.4.1）。** 基于久经考验、管理着数十亿美元资产的 Safe 合约 + ERC-4337。→ 借用 Safe 的品牌与安全资产为新钱包背书，降低"新项目不敢用"的顾虑。

**8. 12 条网络，一个地址。** Ethereum、Base、Arbitrum、Optimism、Polygon、BNB、Avalanche、Gnosis、Unichain、Tempo、Monad、World Chain + 自定义网络，地址完全相同（`src/models/chains.ts`）。→ "记一个地址，收所有链。"主打多链用户的便利与心智简化。

**9. 完全免费，没有代币。** "There is nothing to buy, farm, or speculate on."（whitepaper）→ 在充斥 token 套路的赛道里，"无代币"本身是信任信号；强调激励对齐，不会割你。

**10. 浏览器即用，无需下载。** 同一套代码跑 iOS / Android / Web，Web 端打开链接就能用。→ 把"试用门槛"降到零，是投放落地页/Product Hunt 的转化利器："Try it now, no install."

**11. 丢手机，钱包还在。** passkey 经 iCloud Keychain / Google Password Manager 同步，新设备同账号登录即恢复（`docs/recovery.md`）。→ 主动回答用户最大的恐惧问题"丢手机怎么办"，把无助记词从"风险"转成"放心"。

**12. 每次签名都要重新生物识别，没有长效会话密钥。** whitepaper："no long-lived session key"。→ 对比那些"授权一次、后台随便签"的钱包，强调"每一笔都你亲自点头"。

**13. 链上报价，没有第三方价格 API 窥探你的资产。** DEX 报价（Uniswap V3/PancakeSwap/Aerodrome）+ Chainlink 预言机兜底（`src/services/price-service.ts`）。→ 隐私 + 去中心化双卖点："No CoinGecko watching your portfolio."

**14. 无账号、无邮箱、无追踪、无助记词。** store 文案收尾句，服务端只存你的**公钥**和你起的名字。→ 隐私党最爱的"四无"清单，做成 hero 区 bullet 或贴纸式视觉。

**15. 诚实的 alpha = 最强信任信号。** whitepaper 公开"未审计、无排期、先用小额、连 if Vela disappears 都写明"。→ 在人人吹牛的赛道里，"敢说自己的局限"反而是最稀缺的可信度；把透明做成品牌资产。

---

## 第二梯队 A — 信任证据与决策支撑（把"心动"变成"敢用"）

**16. 交易前先模拟"余额会怎么变"。** `BalanceChangePreview` 显示净资产变化（+绿 / −），或"预计会失败（但仍付 gas）"警告（`src/components/signing/BalanceChangePreview.tsx`、`tx-simulation.ts` 用 `eth_simulateV1`）。→ "签之前就看到结果"，是把抽象安全感落到具体的杀手级演示点。

**17. 永不无限授权：把被动警告改成主动控制。** `EditableApproveCard` 不提供"Unlimited/Max"选项，用户必须选有限额度或撤销，否则确认键禁用（`approval-guard.ts` 静态封顶 2^200）。→ 直接掐死"无限 approve 被盗"这一最常见 drain 场景，做成"我们从设计上就让你签不出无限授权"的硬核卖点。

**18. gas 费用透明拆分，确认前全看见。** 屏幕展示 network fee / relayer fee / total，价格由 bundler 报、钱包不自行加价，且**拒绝高于约 3× 网络费率的报价**（whitepaper、FAQ）。→ 对比"看不懂的 gas"，强调"我们把每一分钱拆给你看"。

**19. bundler 拿到的是已签名交易，改一个字段签名就失效。** whitepaper："cannot change the recipient, amount, or any field"。→ 解释"中继者也偷不了你"，回应"那个帮我发交易的服务会不会作恶"的疑虑。

**20. 地址是确定性、反事实生成的——部署前就能收款。** 由 passkey 公钥经 CREATE2 算出，首笔交易时账户用自己余额自部署（whitepaper "Account model"）。→ "还没花一分钱，地址已经能收款"，技术叙事中很惊艳的细节。

**21. 你需要信任的，被收敛到极小。** whitepaper："reduces to audited Safe contracts + 你的 OS passkey 保险库 + 一个可替换/自建的 relay（仅 liveness）"。→ 适合做一张"信任边界"信息图，理性用户看完就放心。

**22. 和 Apple Pay 同款硬件保护。** 私钥存在 Secure Enclave / StrongБox，App 永远拿不到、只能"请求硬件签名"（`docs/passkeys.md`）。→ 用大众熟悉的 Apple Pay 类比，瞬间建立"这很安全"的直觉。

**23. 坦诚设备安全的边界。** 文档明确："passkey 防远程攻击和钓鱼极好，但防不住拿到你已解锁手机并能过生物识别的人——请设锁屏密码。"→ 这种"连自己的短板都告诉你"的诚实，本身就是高级别信任营销。

**24. 公司只能看到你的公钥和一个名字。** 没有私钥、余额读自公链、无邮箱注册（FAQ "What can Vela see"）。→ 直接量化"我们对你的数据知道得有多少"，隐私党友好。

**25. MIT 许可证，连后端都能自建。** 不是"开源个前端"做样子，三个服务全可自托管。→ 对"去中心化纯度"敏感的用户，这是最高分；做成"自托管指南"长尾内容。

**26. 链上用 RIP-7212 精确验证 P-256 签名。** Safe 在链上校验 WebAuthn P-256（whitepaper）。→ 技术深度证明"passkey 钱包不是噱头，是真在链上能验签"。

**27. 没有 paymaster 在替你（或卡你）付 gas。** gas 从你自己钱包余额出，没有第三方赞助或门禁（whitepaper）。→ 回应"免 gas 钱包"背后的隐性控制风险，强调"没人能借 gas 卡住你"。

**28. 威胁模型公开列举。** 丢/被盗设备、钓鱼/恶意 dApp、服务器被攻破、供应链风险——逐条给出对策（whitepaper "Threats considered"）。→ 给安全研究者/审计向读者看的"我们认真想过攻击面"，可做技术博客。

**29. 服务器被黑，最坏也只是"服务降级，不是丢钱"。** whitepaper："blast radius is degraded service, not loss of funds."→ 一句话化解"用第三方服务会不会被一锅端"的担忧。

**30. 没有 KYC、没有邮箱、没有 cookie 追踪。** 官网用无 cookie、自托管分析（whitepaper "Privacy"）。→ 面向隐私优先地区/人群的强卖点，也是合规简洁度卖点。

**31. "如果 Vela 消失了会怎样"——他们真写了这一页。** 资金在你的链上 Safe，bundler 可替换；唯一诚实警告是 rpId 域名依赖，并已附开源 WebAuthn proxy 扩展做灾备（whitepaper "If Vela disappears"）。→ 极少有钱包敢写"我们倒闭了你怎么办"，这页本身就是病毒级信任内容。

**32. 公钥发布在 Gnosis 链上的智能合约里，不依赖 Vela 服务器存活。** （`docs/recovery.md`、`public-key-index.ts`）→ "恢复你账户的能力，不绑在我们活不活着上"。

**33. 公私钥分离的恢复设计：任一半都动不了你的钱。** 链上公钥让任何新安装能"找到"你的账户，平台同步的私钥才能"授权"（`docs/recovery.md` Callout）。→ 把恢复机制讲成"两把锁"，既安全又好懂。

**34. 一套代码三端一致体验。** 平台差异全收敛在 `src/services/platform.ts`（Alert/Clipboard/Haptics/Linking）。→ "iOS、Android、Web 体验一致"是给跨设备用户的承诺，也是给开发者看的工程品味。

**35. 签名记录可只读"回放"。** 在 Connections 里点历史签名，会用同一个 SigningSheet 只读重现当时看到的意图与字段（`SigningReplaySheet.tsx`）。→ "你签过的每一笔，事后都能原样复盘"，审计感拉满。

**36. dApp 交易和普通转账同等严谨，绝不盲提交。** dApp 签名会估算真实交易、镜像 Send 的 gas/资金/反馈打磨（项目规范）。→ 对比那些"连 dApp 来的大额操作都直接弹窗签"的钱包。

**37. 收款人地址自动解析成可读名字。** 并行查询 ENS、Basename、.bnb、.arb、passkey index，全用链上 RPC 无第三方 API，正向结果缓存 24h（`recipient-identity.ts`、README "Recipient Identity"）。→ "转错地址"是大额损失主因，把"看到名字再转"做成防错卖点。

**38. 能认出其他 Vela 用户。** passkey index 按 walletRef 反查，转账时识别对方是 Vela 用户（`recipient-identity.ts` 优先级 1）。→ 天然的网络效应/裂变钩子："给朋友转账，能看到 ta 也在用 Vela。"

---

## 第三梯队 B — 功能深度与体验亮点（讲"好用 + 有品")

**39. Split：一种代币一次发给 N 个人。** 原子化 MultiSend、一次签名、一次 gas（`batch-send.ts`、`sendBatchCalls`）。→ 主打"发工资/空投/AA 制"场景，省 gas + 省事。

**40. Sweep：把 N 种零散代币一次归集到一个地址。** 同样一笔 UserOp 原子完成，自动预留 native gas 防止 revert（`batch-send.ts` `reserveNativeGas`）。→ "一键清理钱包碎片"，体验型 demo 视频素材。

**41. Max 发送会自动预留 gas，绝不让交易因余额不足失败。** （README "Max Send"、`reserveNativeGas`）→ 一个看似小、却天天救人的细节，体现"为用户兜底"的产品心。

**42. 多源 RPC 自动故障转移 + 延迟打分。** 6 级来源、按延迟+成功率打分、坏端点临时/永久封禁、10 分钟刷新（`rpc-pool.ts`）。→ "一个 RPC 挂了，你根本不会察觉"——可靠性卖点，对被"节点宕机"坑过的人有共鸣。

**43. 余额"流式"加载，哪条链先好先显示。** 每条链单次 multicall、18s 超时隔离、`onProgress` 边算边出（`wallet-api.ts`）。→ 对比"转圈等所有链"，强调"一条死链不拖累整个首页"。

**44. clear-signing 三层信息架构，按风险配色。** L1 意图（大字+配色）/ L2 实质（金额、地址卡片）/ L3 细节（默认折叠的 raw calldata）；安全=绿、警告=琥珀、危险=红（`docs/clear-signing-design.md`）。→ "先读懂'要做什么'，再看金额"——可做成精美对比图，视觉冲击强。

**45. 15 种语言，含粤语（zh-HK）区别于书面中文。** 目录 `src/i18n/locales/`：en, zh, zh-TW, zh-HK, ja, ko, vi, id, tr, es-MX, pt-BR, fr, de, it, ru；区域变体用地道词汇（es 用墨西哥 `tú/billetera`）。→ 国际化 SEO + 多市场触达；"母语级而非机翻"是质量信号。

**46. 入账实时检测 + 触觉提醒。** transfer-monitor 发现到账并持久化，付款类活动是首页一等公民（`activity.ts`、README "Deposit detection"）。→ "钱到账，手机轻轻一震"——做成体验亮点短视频，情绪价值高。

**47. 高级品质的自定义交互，而非系统默认。** 品牌化下拉刷新 `VelaRefresh`（手势跟随+SVG 弧线+触觉）、滑动确认 `SlideToConfirmButton`（防误触大额）（`src/components/ui/`）。→ "每个交互都为品牌重做"，对标大厂级打磨，做 UI/UX 种草内容。

**48. 滑动确认防手滑误签。** 危险级签名/向陌生地址转账用拖动确认（Coinbase/Revolut 风格），armed 时轻触觉、成功时成功触觉。→ "大额操作，刻意让你多一个动作"，安全与体验兼顾。

**49. 金额永不折行的动态显示。** `AmountText`：自适应缩放→紧凑记数（$1.23M）→整数大/小数小双层排版（`docs/dynamic-amount-display.md`）。→ "再大再小的数字都好看好读"，细节控的种草点。

**50. 文字可缩放 0.85×–1.28×，无障碍友好。** 全局 `useStyles` 即时重算样式，切换无闪烁（`docs/text-scale-architecture.md`）。→ 面向老年/视障/无障碍合规，是商店审核与 ESG 叙事加分项。

**51. 温暖精确的设计语言。** 暖中性底色 `#FAFAF8` + 单一橙色强调 `#E8572A`，靠阴影而非边框分层，4px 栅格（`DESIGN_SYSTEM.md`）。→ "不像冷冰冰的加密 App"——视觉差异化，吸引非极客人群。

**52. 动效有目的、不超 400ms。** 按压 spring 0.97、入场 FadeInDown、状态脉冲（`DESIGN_SYSTEM.md` §7）。→ "克制的高级感"，给设计圈/产品圈传播的调性内容。

**53. EIP-5792 批量调用（wallet_sendCalls）原生支持。** dApp 可一次性请求多步操作（`use-dapp-signing.ts`、walletpair capabilities）。→ 面向 DeFi 重度用户/dApp 开发者的兼容性卖点。

**54. 桌面 dApp 扫码即连，手机审批。** WalletPair WebSocket relay，前后台/断网自动重连、25s 心跳（`walletpair-transport.ts`）。→ "桌面操作，手机把关"，对 DeFi 玩家很实用；强调连接稳定性。

**55. 法币显示按地区本地化。** Chainlink 法币喂价（16 种：EUR/GBP/JPY/CNY/KRW…）+ 可配置 FX 端点（默认 Frankfurter/ECB），数字/日期/时间用显式格式预设（`fiat-rates.ts`、`currency.ts`）。→ "看到的是你熟悉的货币和写法"，多市场转化友好。

**56. 离线也能先出价。** 法币汇率内存+持久化缓存，冷启动先用缓存渲染（`fiat-rates.ts`）。→ "弱网/离线打开也不白屏"，可靠性体验点。

**57. 自定义网络支持。** 任意 EVM 链按 chainId 添加，校验 ERC-4337/P-256 兼容（`add-network.ts`）。→ 给极客/新链生态留口子，也是和新链 BD 合作的接口。

**58. 稳定币按 ≈$1 兜底显示价值。** 价格源宕机时，USDT/USDC/DAI 仍按面值显示（`activity.ts` `isStable`，连 Tether 符号 ₮ 都折叠成 T）。→ "价格服务挂了，你的稳定币也不会显示成 0"，细节可靠性。

**59. 零余额代币默认不显示，首页干净。** 可选开启显示 dust（`wallet-api.ts`）。→ "不被一堆垃圾币刷屏"，整洁体验。

**60. 多链组合视图，一眼看完全部资产。** native + 稳定币 + wrapped + 自定义 ERC-20，按 USD 价值排序（`wallet-api.ts`）。→ "所有链的钱，一个列表"，资产管理卖点。

**61. 收款用二维码，不依赖任何中心化服务。** （expo-camera + jsQR，README 平台表）→ 标准、无依赖的收款体验。

**62. 一键提交 bug，社区共建。** 应用内反馈→ getvela.app bug-report 后端代理（PAT 仅服务端），并保留 URL 兜底（项目规划 `BugReportModal`）。→ "你的反馈直达"，把用户当共建者，社区温度。

**63. 不用恐吓横幅，用安静的 Settings"反馈"入口。** 反馈引导到预填的 GitHub bug.yml（产品规范）。→ 体现"信任 > 免责声明"的产品价值观，可做品牌理念内容。

**64. 签名只有唯一渲染路径（生产=测试），安全 UI 永不重复实现。** `SigningSheet` 同时服务生产弹窗与 clear-signing 测试台（`SigningRequestModal.tsx`）。→ 给工程读者证明"安全界面不会出现两套、留后门"。

**65. 历史交易能复盘"当时到底动了哪些资产"。** 模拟结果序列化持久化，可回放（`tx-simulation.ts` `StoredAssetSim`）。→ "事后审计你自己的每一笔"，专业用户友好。

**66. 名称解析只缓存成功结果，错的不会被记住。** 24h TTL、仅正向缓存（`recipient-identity.ts`）。→ 细节正确性，体现工程严谨。

**67. 并行查多个命名服务，返回最高优先级的第一个匹配。** Promise.allSettled（`recipient-identity.ts`）。→ "又快又准地认出收款人"，性能+正确兼顾。

**68. 待发交易在"提交时"就持久化，关页面/刷新都不丢。** （项目 connection-activity-replay）→ "你的交易记录永远不会因为关了页面而消失"，可靠性安心点。

---

## 第四梯队 C — 工程可信度 / 技术 SEO / 硬核细节（给极客 & 搜索引擎）

**69. 内建故障注入演练台。** Web 控制台 `vela.failRpc / slowRpc / flakyRpc / nullPrice / status`，零开销 active flag（`src/services/dev/fault-injection.ts`）。→ 工程博客金矿："我们如何主动把自己的钱包搞坏来验证失败态 UX"。

**70. Tempo 链：没有 native gas 币，用稳定币付 gas。** chain 4217，`gasModel:'tempo'`，UserOp 内 MultiSend 追加报销转账（`chains.ts`、`safe-transaction.ts`）。→ 前沿叙事："4337 钱包在没有原生币的链上如何结算 gas"，技术领先感。

**71. gas 分档定价透明。** Slow ×1.1 / Standard ×1.2 / Rapid ×1.5 / Fast ×2.0，bundler 报价优先、本地兜底（`safe-transaction.ts`）。→ "你能选快慢，且每档怎么算都摊开"。

**72. 已部署 vs 未部署账户 gas 预算天差地别。** 验签 gas 300k vs 首次创建 2M（`safe-transaction.ts`）。→ 解释"为什么第一笔贵一点"，避免用户误解，降客诉。

**73. 模拟采用非对称信任模型，防日志伪造盗刷。** 收到的代币只有"已持有过"才采信，发出的总采信（`tx-simulation.ts`）。→ "连'假装给你转了币'的骗局都防"，安全深度内容。

**74. 全局重试加 0–1000ms 抖动，避免惊群。** （`rpc-pool.ts`）→ 体现"为规模化稳定性认真设计"的工程细节。

**75. 并发取数自动去重，共享同一个 promise。** （`wallet-api.ts` in-flight dedup）→ 性能工程细节，开发者向。

**76. 读 RPC 8s、bundler 15s、ping 3s 的分级超时。** （`rpc-pool.ts`）→ "宁可用稍旧的缓存，也不让你干等"——把工程权衡讲成体验承诺。

**77. EIP-1271 合约签名封装 WebAuthn 断言。** 把 P-256 断言编码成 Safe 可验的合约签名（whitepaper、`safe-transaction.ts`）。→ 证明 passkey→链上的完整工程闭环，硬核可信。

**78. ENSIP-19 Basename 反向解析全支持。** Base 链 reverseRegistrar 流程（`recipient-identity.ts`、README 表）。→ 命名服务覆盖度，DeFi/L2 用户友好。

**79. WebAuthn proxy 扩展做域名灾备 & 开发态调试。** 当 rpId 域名不可用时通过扩展自有 origin 代理 WebAuthn（README、扩展 self-heal 提示）。→ "连'域名没了 passkey 怎么办'都准备了后路"。

**80. i18n key 类型化、编译期校验。** `i18next.d.ts` 增强，写错 key 直接 `tsc` 报错（`docs/localization.md`）。→ "翻译不会漏不会错"的质量保证，给本地化合作方信心。

**81. ~600 个 key × 15 语言 100% 对齐，脚本守门。** parity-check 检查缺失/多余/占位符（`docs/localization.md`）。→ 本地化工程化程度，国际化叙事支撑。

**82. clear-signing 覆盖大量 ERC-7730 描述符。** 通用 ERC 标准 ERC-20/721/4626/2612/7540 + 链特定与 EIP-712 描述符，无匹配则显式盲签警告（`docs/clear-signing-design.md`）。→ "覆盖广、且诚实地承认覆盖不到的"。

**83. 切换语言/货币/主题即时生效、无重启无闪烁。** 同步内存更新→Context→重渲染（`docs/text-scale-architecture.md`）。→ 体验工程范式，可做"如何做到设置秒变"的技术文。

**84. Multicall3 把余额+DEX 报价+Chainlink 喂价打成一个 eth_call。** 每链一次调用（`wallet-api.ts`、`price-service.ts`）。→ "10 条链并行、每链一次请求"的性能叙事。

**85. 价格 3 分钟缓存、符号别名映射（POL→MATIC、xDAI→DAI）。** （`price-service.ts`）→ 工程正确性细节。

**86. 失败链会被明确回调上报，而不是静默吞掉。** `onFailedChains`（`wallet-api.ts`）。→ "出错会告诉你哪条链出错"，透明性。

**87. 账户数据仅存设备本地 AsyncStorage，不存私钥只存 credential ID。** （`storage.ts`、`CreateWalletScreen.tsx`）→ "我们手机上都只存一个 ID，私钥根本不出安全芯片"。

**88. 上传公钥到 index 用幂等键去重、自动重试退避。** Idempotency-Key、3 次重试 1s/2s 退避（`public-key-index.ts`、`CreateWalletScreen.tsx`）。→ "创建钱包稳如老狗"的可靠性细节。

---

## 第五梯队 D — 分发、渠道与内容打法（怎么把线索变成曝光）

**89. 受众锚点："被助记词坑过的人"。** introduction 原话："for people who've been burned by it before"。→ 所有广告/内容的情绪锚点，做"我曾经丢过助记词"主题 UGC/故事征集。

**90. 对比页是高意图 SEO 金矿。** 做 `Vela vs MetaMask`、`Vela vs Coinbase Wallet`、`passkey wallet vs seed phrase wallet`、`self-custody vs 交易所`。→ 用真实差异（无助记词、clear-signing、不可冻结）填充，承接高购买意图搜索。

**91. "A wallet that does less — on purpose." 是反共识定位。** 对应 whitepaper 设计原则 "Do less"。→ 在堆功能的赛道里反向站位，做品牌记忆点，适合 X 长推/宣言式内容。

**92. docs 本身就是 SEO/AEO 内容库。** introduction/faq/passkeys/recovery/networks-and-fees/clear-signing 已是问答式、可被 AI 引用的结构。→ 直接复用为博客/知识库，针对"what is a passkey wallet""no seed phrase wallet"等长尾词与 AI 答案优化。

**93. whitepaper 是稀缺的"诚实"传播资产。** 含"你信什么/不信什么""审计现状""如果我们消失"。→ 做成可分享的长图/线程，吸引技术 KOL 转发背书。

**94. GitHub 开源 = 社会证明 + 开发者获客。** 仓库公开、MIT、可自托管（github.com/mondaylabsltd/vela-wallet）。→ 把 star/PR/issue 做成增长指标；写"自己搭一套 Vela 后端"的教程引流开发者。

**95. 社群渠道现成：X `@realvelawallet` + Telegram `t.me/velawallet`。** （FAQ 底部）→ 所有内容统一引流到这两处，做 build-in-public 日更与社区运营。

**96. ASO 关键词已盘好。** `crypto,ethereum,web3,self-custody,passkey,seedless,defi,smart account,erc-4337,base,arbitrum,evm`（store-listing-copy）。→ 直接用于商店、落地页 meta、Google/Apple Search Ads。

**97. 15 语言 = 多市场分发杠杆。** 商店文案已有简中版，可快速产出 13 语言变体（机翻后需母语校对，per i18n 规范）。→ 低成本进入非英语市场，尤其中文/日韩/拉美。

**98. "passkey 科普"做免费教育型获客。** passkeys.md 把 Secure Enclave、为何不可钓鱼讲得通俗。→ 做短视频/图解"什么是 passkey、为什么比助记词安全"，蹭 passkey 大趋势流量。

**99. build-in-public 创始故事天然有料。** blog "the story of how Vela is being built"、alpha 公开。→ 单人/小团队、诚实造钱包的叙事，适合做创始人 IP 与 Indie Hacker/HN 社区传播。

**100. 网络效应裂变钩子：Vela 用户能互相识别。** passkey index 让转账时认出对方也是 Vela 用户（线索 38）。→ 设计"邀请朋友→转账时看到 ta 在用 Vela"的轻量推荐机制，把产品功能直接变成增长回路。

---

## 附：渠道速配建议

- **应用商店 / 落地页首屏**：1、2、3、5、8、10、14（核心钩子 + 四无）。
- **安全敏感用户 / DeFi 老手**：4、16、17、18、19、28、44、73、82。
- **隐私优先人群**：5、13、24、30、95。
- **技术 KOL / 开发者 / HN**：6、25、31、69、70、77、93、94、99。
- **新手 / 被助记词坑过**：1、2、11、89、98。
- **SEO / AEO 长尾**：90、92、96、97。
- **品牌 / 传播记忆点**：9、15、91、100。

> 维护提示：本文档事实均可在源码中复验。修改产品后请同步更新对应线索（尤其网络数量、语言数、gas 费率措辞、审计/alpha 表述），避免营销与实现脱节。
