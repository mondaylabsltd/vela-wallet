# Vela Wallet 分发·曝光·转化计划

> 2026-07-02。约束:单人创始人(时间是唯一稀缺资源)、默认零广告预算、隐私立场(App 内无追踪,度量只能用服务端/商店后台)。
> 原则:**不做 139 件事,做 9 件事,每件打透。** 所有渠道按"画像命中 × 单人可持续 × 转化距离"排序。
> 依赖文档:[pricing-analysis.md](pricing-analysis.md)(定价与画像)、[why-we-charge.md](why-we-charge.md)(宣言)、[100-marketing-leads.md](100-marketing-leads.md)(素材库)。

---

## 〇、漏斗架构(先定骨架,再谈渠道)

```
曝光 ──→ getvela.app(免费 Web 版试用)──→ 激活 ──→ $39.99 商店购买 ──→ 布道
         「零门槛,浏览器即开」        「小额入金+首笔交易     「Web 内 4 个升级    「Founders/
                                      +连一次 dApp」          时刻(定价文档§5)」  开源社区」
```

**核心设计:商店不是获客入口,是变现终点。** 一切曝光都导向免费 Web 版;$39.99 的说服工作在 Web 使用体验 + 宣言里完成,商店页只负责收尾。冷流量直接看到 $39.99 → 跳出,是预期内损耗,不要为它优化。

**度量(隐私兼容)**:
| 漏斗层 | 指标 | 工具 |
|---|---|---|
| 曝光 | getvela.app 访问量、来源(UTM 只打在落地页链接上) | Cloudflare Web Analytics(无 cookie,与品牌一致) |
| 试用 | Web 版打开数(静态托管请求数近似) | CF Pages 分析 |
| 激活 | 无法直接测(App 内不追踪)——用代理指标:bundler 新钱包首笔 UserOp 数/周 | vela-relay 服务端日志(链上公开行为,非用户追踪) |
| 购买 | 商店销量、商店页转化率、来源(Apple: App Analytics;Google: Console) | 两店后台 |
| 布道 | GitHub stars、被引用/转发 | GitHub/社媒 |

每周五 30 分钟看一次全漏斗,只回答一个问题:**这周瓶颈在哪一层?下周的时间投给那一层。**

---

## 一、九个渠道打法(按优先级)

### P1 — 启动期三板斧(一次性大事件,前 6 周)

**1. Show HN:宣言首发**(画像 C/E → 扩散到 A)
- 素材已备:[why-we-charge.md](why-we-charge.md),标题用 "Every wallet is free. Ours costs $39.99. On purpose."
- HN 吃两样东西:反直觉商业模式 + 开源可验证。两样都有
- 准备:发帖当天全天守评论区(评论质量决定存活);预备答案已写在宣言文档"发布注意"节
- 成功标准:首页停留 >4 小时,getvela.app 单日 >5k 访问
- 时间成本:1 天写发帖版 + 1 天守评论

**2. Product Hunt:主推免费 Web 版**(画像 B/D/E)
- 定位 "Try now, no install"(lead #10),付费移动版作为 pricing 区的 upsell 出现,不做主角
- 准备:6-8 张产品图(clear-signing/never-unlimited/余额模拟 截图)、30 秒演示视频、首评(maker comment)讲"为什么 Web 免费移动收费"
- 与 HN 错开 ≥1 周,素材互相复用
- 时间成本:3 天准备 + 1 天守

**3. 目录铺设(一次做完,长期收租)**(SEO 权重 + 长尾发现)
- 通用:AlternativeTo(挂在 MetaMask/Rainbow alternatives 下)、SaaSHub、Product Hunt 存档页
- 加密专属(逐个核对收录标准,勿假设):**ethereum.org 钱包查找页**(官方 find-wallet,按功能筛选,passkey/无助记词/开源是筛选项——命中即高质量流量)、WalletConnect/dApp 目录、L2 生态官方钱包列表(Base/Arbitrum/Optimism 生态页)
- 开源侧:GitHub README 当落地页维护(记忆里 README 已过期——先修)、相关 awesome-lists(awesome-account-abstraction 等)提 PR
- 时间成本:集中 2-3 天做完 20+ 提交,之后零维护

### P2 — 常青内容引擎(每周节奏,启动后持续)

**4. 安全事件即时评论(newsjacking)**(画像 A,变现核心的头号触达法)
- 每次行业 drain/钓鱼/无限授权盗币事件 = 一次免费投放窗口:24h 内发 CT 线程,内容固定三段式——事件技术拆解 → "如果在 Vela 里会发生什么"(clear-signing 截图演示同一笔恶意交易长什么样)→ 不贬损受害钱包,克制收尾
- 这是唯一能反复触达 A 画像的场景:**他们只在出事时重新评估钱包**
- 准备一次:恶意交易演示模板(clear-signing-test 场景页就是现成演示台,20+ 场景截图一次备齐)
- 时间成本:事件驱动,每次 2-3 小时;素材库一次 1 天建好

**5. 对比演示短内容(每周 1 条)**(画像 A/B)
- 格式:同一笔交易,MetaMask(盲签十六进制)vs Vela(人类可读意图 + 支出上限)并排 30 秒录屏;发 CT + YouTube Shorts + 小红书/推特华语区(14 语言是差异化,轮流做本地化字幕)
- 选题库直接用 leads 第一/二梯队:#4 拒绝盲签、#17 永不无限、#16 余额模拟、#5 无可钓鱼、#11 丢手机恢复、#1 无助记词
- 时间成本:每条 2 小时,可批量录

**6. Reddit/论坛真诚答题(每周 3-5 条)**(画像 B,获客主力)
- 蹲点:r/ethereum、r/CryptoCurrency 的 "lost seed phrase / got drained / which wallet" 帖
- 规则:先真诚解决问题,末尾一句 "I build a passkey wallet that removes seed phrases entirely — free in browser if you want to see the model"(透明利益申报,Reddit 唯一活法)
- 副产品:这些帖子的原话就是 VOC 词库(leads 研究缺口的既定计划),喂回文案
- 时间成本:每周 1-2 小时

### P3 — 结构性放大器(有余力再做)

**7. 本地化市场逐个点火**(画像 D + 区域 PPP 价)
- 14 语言 + 新兴市场区域价($19.99–24.99)是别人没有的组合;每季度选 1 个市场(建议顺序:日本→韩国→巴西→东南亚),把宣言+对比视频本地化,投当地社区(日本:X 日语加密圈;韩国:Kakao/Naver 社区;巴西:葡语 CT)
- 时间成本:每市场 2-3 天,可外包翻译校对(机器翻译底稿已有)

**8. 开发者布道(DevRel-lite)**(画像 E,分发倍增器)
- 素材即产品:AA+passkey 参考实现、fault-injection 演练台、parallel space 测试环境——写 2-3 篇工程博客(如 "我们如何测试一个不能出错的签名路径"),投 HN/dev CT
- 目标不是转化 E,是让 E 在"推荐个 AA 钱包看看"时说出 Vela
- 时间成本:每篇 1 天,素材全是现成工程事实

**9. 小型加密播客巡回**(画像 A/C,建立"人"的信任)
- 单人创始人 + 反主流商业模式 + honest-alpha = 好故事;从 50-500 听众的小播客开始(好约、长尾),话题就是宣言
- 时间成本:每期 1.5 小时,启动后每月 1-2 期

---

## 二、转化率优化(漏斗每层的具体动作)

### 曝光→试用(落地页)
- getvela.app 首屏 = lead 客户视角 Top 12 的前三条:无助记词 / 人脸签名 / 不可冻结;CTA 唯一:"Try it free in your browser"(零风险动词)
- 宣言、对比视频、目录、Reddit 全部链到**同一个落地页**,UTM 区分来源

### 试用→激活(Web 版内)
- 新钱包空状态引导三步:收一笔小额 → 发一笔 → 连一个 dApp(激活定义)
- "先用小额"文案化为卖点(既定红线):"Start with $20. Seriously."

### 激活→购买(已在定价文档 §5,工程待办)
- Web 内 4 个升级时刻(首笔成功回执/首次 dApp 连接/到账时/官网定价区)——**这是唯一需要写代码的营销项**,建议排进下个迭代
- 商店页 = pro 购买页:首屏 "The hardware wallet you already own",截图顺序 = clear-signing → 支出上限 → 余额模拟 → 恢复,评分请求只在成功交易后弹(两店原生 API)

### 购买→布道
- 购买后首屏 "You are the customer" 致谢页(呼应宣言);引导:GitHub star / 推荐给一个朋友(暂不做返利,与品牌不符)

---

## 三、90 天节奏表

| 周 | 主线 |
|---|---|
| 1-2 | 基建:README 更新、落地页首屏改版、目录 20+ 提交、clear-signing 截图素材库、CF Analytics 接好 |
| 3 | **Show HN**(宣言) |
| 4-5 | 消化 HN 流量;开始每周对比视频 + Reddit 节奏 |
| 6 | **Product Hunt**(Web 版) |
| 7-12 | 常青引擎跑满(视频/Reddit/事件评论);第 8 周复盘漏斗,决定 P3 里先点哪个;第 12 周做首次全量复盘(各层转化率基线成型) |

**时间预算(2026-07-02 修订:第一个 90 天创始人主导、同事辅助)**:窗口内产品只修 bug,创始人时间发布周 ~60-70%、引擎期 ~40% 投营销;同事承担生产支持(素材/目录/看板/VOC/本地化初稿),对外发声全部是创始人。90 天对表后按 [marketing-onboarding.md](marketing-onboarding.md) 移交。优先级不变:超载先砍 P3,保 P2;P2 里事件评论 > 对比视频 > Reddit。

## 四、明确不做的事(和为什么)

- **付费广告**:$39.99 单价 × 无归因(App 内不追踪)→ 无法算 ROAS;等漏斗基线成型再议
- **返利/联盟计划**:与"你是客户不是产品"叙事冲突,且加密联盟流量质量差
- **ASO 深度投入**:付费 App 商店排名天然弱,只做品牌词 + "passkey wallet / no seed phrase" 长尾,listing 转化优先于关键词
- **空投/积分/任何代币化增长**:红线,whitepaper 承诺无代币
- **买 KOL 推广**:信任型产品买推广 = 自毁;播客对谈(不付费)替代
