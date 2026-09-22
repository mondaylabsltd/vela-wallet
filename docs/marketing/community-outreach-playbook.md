# 进入以太坊与 Safe 社区：可执行手册

> 面向创始人本人。目标：让以太坊和 Safe 社区的核心成员看到 Vela、看懂 Vela、给出建议——在推向市场之前。
> 写作日期 2026-09-22，所有事实当天核过，附链接。英文模板可直接复制。
> 关联：[distribution-plan.md](distribution-plan.md)（9 渠道分发计划）、[marketing-onboarding.md](marketing-onboarding.md)（同事带训手册）。

---

## 0. 先接受三个不舒服的事实

**一、Safe 论坛的技术版块基本没人回。** 论坛已搬到 [forum.safefoundation.org](https://forum.safefoundation.org)（旧域名 301 跳转）。2025–2026 年 *Developer Discussion* 版块的每一个主题**回复数都是 0**（除三个客服问题），*Protocol* 和 *Infrastructure* 版块今年**一个主题都没有**。社区电话会也停了，最后一次公告是 2025-07-16 的治理会。

**二、"给我的钱包提点建议"这种帖子，注定 0 回复。** 有实证：Magicians 上有人 2026-02 发了一个 AA 设计帖，之后自己回了三条"实现进展"，**七个月零外部回复**（[27789](https://ethereum-magicians.org/t/operational-primitives-for-aa-lane-separation-fixed-execution-phases-and-versioned-upgrades/27789)）。Safe 论坛上的"Dead Man Switch Recovery Signer"（2026-09-02）、"Safe Unified Account"（2026-02-10）同样 0 回复。**展示型帖子只有浏览量，没有对话。**

**三、没有"AA 开发者例会"可以参加了。** 2026 年 ethereum/pm 上的例会是 ACDE / ACDC / ACDT / FOCIL / Frame Transaction 等，**没有 ERC-4337 专属例会**；"Future of EOA/AA Breakout" 已标记为 inactive。而且 ethereum/pm 明确写：**应用层和 ERC 通常不作为核心会议议题**。

**结论：不要"发布"，要"参与"。** 唯一有效的姿势是：**带着别人没有的数据，去回答别人正在争论的问题。** 你手里正好有这种数据——24 条链上的 P-256 预编译实测、passkey 签名者在真实链上的部署结果、bundler 互操作情况、EIP-7951 与 RIP-7212 的行为差异。这是稀缺品，全世界没几个人有。

---

## 1. 你的开场资产（先想清楚"你带来了什么"）

按社区价值排序，不是按你投入的精力排序：

| 你有的东西 | 对谁有价值 | 为什么稀缺 |
|---|---|---|
| **24 条链的 P-256 预编译实测**（哪些链有、行为是否一致、gas 差异、EIP-7951 与 RIP-7212 的实现差异） | EIP-7951 作者、RIP-7212 作者、想上 passkey 的 L2 | 几乎没人跨 24 条链实测过 |
| **Safe passkey 模块在生产中的真实用法**（SharedSigner 第一把钥匙 + SignerFactory 第 2–7 把；多钥匙钱包在缺 factory 的链上无法部署这个坑） | Safe 合约维护者、其他想做 passkey 钱包的人 | 这是踩过的坑，不是读文档能知道的 |
| **ERC-4337 v0.7 + in-band 付费的实现报告**（不用 paymaster，费用写进 MultiSend 由用户签名） | ERC-7769（Draft 状态）、ERC-7562（Review 状态）的相关讨论 | 一种少见的付费模型 |
| **清晰签名的降级阶梯**（ERC-7730 描述文件 → 代币形状 → 4byte → 盲签警告）和"获取来的描述文件不能标成已验证"这个判断 | 钱包 UX / 安全方向 | 有实现、有取舍、有诚实结论 |
| **全栈自托管**（中继、公钥索引、链数据、汇率都能自己跑，MIT） | Safe 生态、自主权方向的人 | 大部分"开源钱包"做不到 |

**反过来，以下东西不要当开场**：价格（$39.99）、上架计划、用户数、"求反馈"、"求 star"。

---

## 2. 五条路径（按性价比排序，全部异步、全部书面）

### 路径 A（最高优先）：在已有的规格讨论帖里贴实测数据

不是新开帖，是**回复别人已经在讨论的帖子**。四个现成的入口：

1. **EIP-7951 讨论帖**（P-256 预编译，Final，Fusaka 已上主网）——[ethereum-magicians.org/t/24360](https://ethereum-magicians.org/t/eip-7951-precompile-for-secp256r1-curve-support/24360)。作者：Carl Beekhuizen、Ulaş Erdoğan、Doğan Alpaslan（[EIP 页面](https://eips.ethereum.org/EIPS/eip-7951)公开署名）。
2. **RIP-7212 讨论帖**——[ethereum-magicians.org/t/14789](https://ethereum-magicians.org/t/eip-7212-precompiled-for-secp256r1-curve-support/14789)。这个帖子的历史很说明问题：**得到回复的全是报告真实实现的人**（Ledger 的优化论文、黑客松部署、"各家 P256 实现开始出现分歧"的观察）。这就是你的模板。
3. **ERC-7769**（4337 的 JSON-RPC 部分，**Draft**）——[规格](https://eips.ethereum.org/EIPS/eip-7769)。Draft 意味着还在收实现反馈，你的中继/bundler 互操作报告正好是它缺的。
4. **ERC-7562**（4337 验证规则，**Review**）——[规格](https://eips.ethereum.org/EIPS/eip-7562)，Magicians 上 2026-09-14 还有新讨论。

### 路径 B：Safe 的 GitHub（不是论坛）

Safe 今年拆成了两个组织，**发错地方=石沉大海**：

- **合约与模块** → [github.com/safe-fndn](https://github.com/safe-fndn)（`safe-smart-account`、`safe-modules`、`safe-singleton-factory`，都在 2026-09 有更新）
- **产品与服务** → [github.com/safe-global](https://github.com/orgs/safe-global/repositories?sort=updated)（钱包前端、client-gateway、transaction-service、`safe-docs`）

最便宜的自我介绍：**给 [safe-docs](https://github.com/safe-global/safe-docs/blob/main/CONTRIBUTING.md) 提一个文档 PR**（文风要求 Microsoft Style Guide，可用 `vale` 本地检查）。你读过的 Safe 文档里，凡是和 passkey 模块、4337 模块相关的过时或不清楚之处，都是机会。注意 safe-global 的 PR 需要签 CLA。

另外：**[Safe 生态数据库](https://github.com/safe-global/safe-ecosystem-database)** 用 GitHub issue 模板提交收录（条件：真的用了 Safe{Core}、且在维护；明确排除"废弃的黑客松作品"）。这是一次正当的"被看见"，不是推销。

### 路径 C：Safe 基金会的 Spontaneous Grants（邮件，纯异步）

[safefoundation.org/grants](https://safefoundation.org/grants)：随时可投、**邮件提交**、4 周内回复、按里程碑发放，重点方向包含 **Security 和 Developer Tooling**。
对你的价值不只是钱——**这是让基金会里的技术人正式读你东西的渠道**。申请里最好包含"我们发现 Safe 多钥匙钱包在缺少 SignerFactory 的链上无法部署"这类具体发现。

### 路径 D：ethereum/pm 的书面议题（不用开口说话）

两个对你合适、且**明确欢迎外部人**的会：

- **EIP Editing Office Hour**（每周二 16:00 UTC）：在 agenda issue（如 [#2229](https://github.com/ethereum/pm/issues/2229)）下留言，附上你的问题；官方话术是 "Authors are always welcome to join, ask questions, and participate."
- **RPC Standards call**（约每两周，15:00 UTC，如 [#2228](https://github.com/ethereum/pm/issues/2228)）：4337 的 RPC / bundler 互操作正好属于这里。

ethereum/pm 的规矩是：**"To add an item to an agenda, simply add a comment"**，但必须是技术性、底层的议题。**你可以只留书面问题，不必到场发言。**

### 路径 E：漏洞（如果你发现了）

**私下上报，绝不公开。** Safe：bounty@safefoundation.org（范围包含 **Safe 4337 Module v0.3.0 和 Safe Passkey Module v0.2.1**，最高 $1M；[规则](https://docs.safefoundation.org/security/bug-bounty)明写"公开披露即失去资格"）。以太坊基金会：[bbp-form.ethereum.org](https://bbp-form.ethereum.org/)。
这是最快获得核心成员尊重的方式，也是最快毁掉自己的方式——顺序反了就全毁。

---

## 3. 怎么找到"人"（而且不越界）

**方法：顺着公开署名走，永远在公开帖里说话。**

1. 规格页的 **Authors 字段**（EIP-7951、RIP-7212、ERC-4337 都公开署名 GitHub handle）
2. → 该规格的 **discussions-to 帖子**（就是上面路径 A 的链接）
3. → 相关 repo 的 **issue/PR reviewer**、[safe-global 组织成员页](https://github.com/orgs/safe-global/people)（成员自己选择公开的）
4. → ethereum/pm 的 **agenda issue**

**红线：不要私信。** ethereum.org 官方安全页（2026-06 更新）写着：**"Real moderators will never DM you first"**、**"There is no 'Ethereum support team'"**（[scams](https://ethereum.org/en/community/support/scams/)）。一个陌生账号私信过来、还带着钱包链接——这正是骗子的标准模式，你会被直接拉黑。

**正确做法：在他们自己写的帖子下面公开 @ 他们**，并且只在你有具体问题时。

---

## 4. 雷区清单（踩一个就前功尽弃）

| 雷 | 为什么 | 怎么做 |
|---|---|---|
| **私信陌生人** | 官方安全指引把"主动私信"定义为诈骗特征 | 公开回帖 + @ |
| **开场就发产品链接** | Magicians 服务条款禁止"为第三方站点引流的商业内容" | 先给数据，链接放最后一行、当引用 |
| **提代币/空投** | Safe 论坛有官方"识别空投农民"悬赏帖（举报者拿回收代币的 25%），社区对这个词极其敏感 | **主动写明"本项目没有代币，也不会有"** |
| **公开谈未修复的漏洞** | Safe 与 Immunefi 规则：公开即失去资格，且被视为敌意行为 | 私下上报，等修复后再谈 |
| **AI 味的长文** | curl 的政策被广泛引用："如果别人能看出是 AI 写的，说明你没做够功课"；Magicians 条款禁止机器生成内容 | 短句、具体数字、你自己的观察；宁可语法不完美 |
| **自问自答连发更新** | 上面那个七个月 0 回复的真实案例 | 一个帖子只问一个问题，没人回就换地方，别自己顶帖 |
| **在 r/ethereum 发产品** | 规则含 "No product promotion"、"No spamming or drive by posting"（规则文本来自 2024 存档，当前文本未能核实） | 不要把 Reddit 当首发渠道 |
| **在 ethresear.ch 发实现报告** | 置顶帖明确：不讨论具体 EIP（去 Magicians）、不做技术问答，低信噪比会被封 | 只用 Magicians |
| **在 ACD 核心会议提应用层话题** | ethereum/pm 明写应用与 ERC 通常不作为议题 | 走 EIP Office Hour / RPC Standards |
| **假装英语很好** | 翻译腔 + 完美语法 = 像 AI；说明反而加分 | 见下一节的声明句 |

---

## 5. 英文表达模板（直接复制，按需替换方括号）

### 5.1 身份与语言声明（每个首帖都放，一句就够）

> I build [Vela](https://getvela.app), an open-source Safe + ERC-4337 wallet that signs with passkeys. English is not my first language, so this is written with translation help — please ask me to clarify anything that reads oddly.

（**为什么要写**：Rust 社区的贡献指南明确建议非母语者说明使用了翻译工具，否则维护者可能把翻译文本误当成 AI 垃圾。）

### 5.2 路径 A 的帖子骨架（贴数据，问一个问题）

标题格式：`[实测对象]: what I measured across [范围], and one question about [争议点]`

> **What I did.** Vela is a Safe v1.4.1 account whose owners are WebAuthn P-256 keys, verified on-chain by the precompile at `0x100`. It runs on 24 mainnets today, so I ended up with cross-chain data on EIP-7951 / RIP-7212 behaviour.
>
> **What I measured.** [一个小表格：链 / 有无预编译 / gas / 行为差异]
>
> **What surprised me.** [1–3 条具体发现，例如边界情况在不同链上返回不同结果]
>
> **My question.** [一个具体的、别人需要拿主意的问题，例如：Should wallets treat a chain that implements RIP-7212 semantics differently from one that implements EIP-7951, or is the interface difference safe to ignore for WebAuthn-shaped signatures?]
>
> Everything is MIT and reproducible: [仓库链接]. No token, nothing to buy — I am asking because the answer changes what our wallet does.

### 5.3 Safe 论坛的 "request for feedback"（这个格式在 Safe 论坛被验证有效）

> **Title:** Request for feedback: running Safe passkey signers across 24 chains (findings + 3 open questions)
>
> **The problem.** [两句话：你想解决什么]
> **What we built.** [三句话：Safe v1.4.1 + 4337 module v0.3.0 + passkey module v0.2.1，多钥匙 1-of-n，地址由全部钥匙决定]
> **What we found.** [具体发现，例如：a multi-key Safe cannot be deployed on a chain that lacks SafeWebAuthnSignerFactory, and our network check did not catch it]
> **Open questions.** 1) … 2) … 3) …
> **Not asking for.** No grant request here, no token, no launch announcement — just the three questions above.

（Safe 论坛上真正有回复的技术帖就是这个结构：问题 → 做法 → 现状 → 明确的提问。例如 [Chain Abstraction with Safe and ERC-4337](https://forum.safefoundation.org/t/5941)，Safe 联合创始人亲自回了。）

### 5.4 GitHub issue（safe-fndn / safe-global）

> **Environment.** Safe v1.4.1 (SafeL2), Safe4337Module v0.3.0, SafeWebAuthnSharedSigner v0.2.1, chain [x], EntryPoint v0.7.
> **What I expected.** …
> **What happened.** …
> **Minimal reproduction.** [链接到一个最小仓库或一段脚本]
> **Why it matters.** [一句话：谁会受影响]
> I am happy to open a PR if you tell me which direction you prefer.

### 5.5 公开 @ 某人（只在你有具体问题时）

> @[handle] you wrote the [spec/PR], so you are the right person to sanity-check this: [一句话问题]. No rush, and no need for a long answer — a yes/no plus a pointer is plenty.

### 5.6 漏洞私下上报

> Subject: Possible issue in [component] — coordinated disclosure
>
> I think I found a problem in [component/version]. I have not discussed it anywhere public and will not until you tell me it is fixed or out of scope.
> **Impact.** … **Steps.** … **Affected versions.** … **My contact.** …

### 5.7 绝对不要出现的句子

- ❌ "Please check out my wallet and let me know what you think."（无人可答，必然 0 回复）
- ❌ "We are the first/best/most secure …"
- ❌ "Would love to hop on a quick call!"（你听说都吃力，而且社区偏好异步）
- ❌ "Happy to share our token/airdrop plans."
- ❌ 一次贴 2000 字纯文字、没有数据表格的长文

---

## 6. 前 30 天节奏（每周一件事，不贪多）

| 周 | 做什么 | 产出 |
|---|---|---|
| 第 1 周 | 写"跨链 P-256 实测报告"：24 条链的表格 + 3 条发现 + 1 个问题。先写中文，再翻译，最后自己逐句读一遍去掉翻译腔 | 一个 Markdown |
| 第 2 周 | 发到 **EIP-7951 讨论帖**（不是新开帖）。同一周给 **safe-docs 提一个文档 PR** | 两次公开露面 |
| 第 3 周 | 把 Safe 多钥匙 SignerFactory 的坑整理成 **safe-fndn 的 issue**；同时提交 **Safe 生态数据库** 收录 | 一个 issue + 一次收录 |
| 第 4 周 | 投 **Safe Spontaneous Grants**（邮件）；在 **ERC-7769（Draft）** 帖子里贴 bundler 互操作观察 | 一封申请 + 一次技术贡献 |

之后每两周维持一次"有数据的公开发言"即可。**质量远比频率重要**——七个月 0 回复的那个反例，问题不是发得少，是发的东西没人需要回答。

---

## 7. 中文圈（同步进行，成本低、反馈快）

这些场子用中文即可，且 2026 年活跃：

- **ETHPanda 论坛**：[forum.ethpanda.org](https://forum.ethpanda.org/)
- **以太坊中文周会**：每周一 14:00，会议纪要公开
- **以太坊之夏 2026**（LXDAO × ETHPanda 城市系列）：[ethpanda.substack.com](https://ethpanda.substack.com/p/2026)
- **LXDAO 社区周会**：[forum.lxdao.io](https://forum.lxdao.io/t/topic/3688)

用法：先在中文圈把"跨链 P-256 报告"讲一遍，收集问题、修正说法，**再翻译成英文发到 Magicians**。等于免费的同行评审。

---

## 8. 活动（可选，异步友好的才考虑）

- **Devcon 8**：2026-11-03 至 11-06，孟买（[devcon.org](https://devcon.org/en/)，票价 $499 ETH / $999 法币）。不必去；去之前应该先在论坛有存在感。
- **ETHGlobal**：[ethglobal.com/events](https://ethglobal.com/events)。关键事实：**允许单人参赛，赞助方奖项是异步评审**（只有决赛才需要现场讲）；**Continuity 赛道允许提交已有代码库上的新工作**（需声明）。这是英语口语弱的人最划算的曝光方式。近期没有 Safe / AA 专项奖池。
- **EF ESP**：[esp.ethereum.foundation](https://esp.ethereum.foundation/applicants) 书面申请，3–6 周回复；但 Office Hours 是 20 分钟**视频通话**，不适合你。
- **EF 的 Reddit AMA**（2026-09-16 刚办过，**问题提前书面提交**）：每年 9 月左右一次，是英语弱者最友好的问答场合，明年留意。

---

## 9. 没能核实的事（别拿它做计划）

- Farcaster 上 AA/钱包频道的活跃度（频道存在，但看不到发帖频率；`/aa` 不是账户抽象，是个音乐频道，别发错）
- r/ethereum 的当前规则原文（Reddit 抓取被封，引用的是 2024-11 的存档）
- Safe 是否有官方"我们不会主动私信"页面（未找到；以太坊官方那页可以引用）
- Devconnect 2026/2027、Devcon 9、ETHGlobal 线上黑客松（均未公布）
- Safe 的 Stack Exchange `safe-core` 标签活跃度

---

## 10. 一句话总结

**不要发布产品，去回答问题。** 你唯一真正稀缺的资产是"24 条链上真实运行的 Safe + passkey + 4337 数据"；把它放进别人已经在争论的帖子里，署名公开、语言诚实、没有代币、不发私信——社区会自己找过来。
