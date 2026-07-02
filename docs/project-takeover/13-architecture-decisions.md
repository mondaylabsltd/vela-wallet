# 13 — 架构决策记录 (Architecture Decision Records)

> 记录日期 2026-07-02,基线 commit `73d7aac`。每条 ADR:背景 / 决策 / 理由 / 代价 / 替代方案 / 推翻条件。
> 证据规范:事实性陈述给 `文件:行号`、commit 或交接文档编号;创始人口头交代的标注 **创始人陈述(2026-07-02)**,属待持续验证的一手意图,不是代码事实。
> 本文档同时是训练教材:U7(14 号文档)的验收任务是受训者**补写或挑战**这里的任何一条——尤其是"代价"与"推翻条件",这两栏最容易写得自欺。

## 索引

| # | 决策 | 状态 |
|---|------|------|
| ADR-001 | 不用 viem/ethers,链上底层全手写 | 生效 |
| ADR-002 | passkey-only 无助记词 + 公钥索引服务作为恢复链路 | 生效 |
| ADR-003 | counterfactual CREATE2 部署(首笔交易 initCode) | 生效 |
| ADR-004 | bundler 是 gas 价格权威,钱包 RPC 永不否决 | 生效 |
| ADR-005 | 配置全部代码内常量 + AsyncStorage 覆盖,无 .env | 生效 |
| ADR-006 | /android /ios 不入库,持久化改动走 config plugin | 生效(2026-07 接管审计确立) |
| ADR-007 | Tempo 无原生币:稳定币 gas 报销内嵌 MultiSend | 生效 |
| ADR-008 | 单一 SigningSheet 渲染路径(生产与测试 harness 同路) | 生效 |
| ADR-009 | Web 免费 + 商店付费 + bundler relayer 费对架构的约束 | 生效(商店定价未定) |

---

## ADR-001 不用 viem/ethers,链上底层全手写

**背景**
钱包核心是资金操作:ABI 编码、EIP-712 哈希、RPC 调用、Multicall、UserOp 构造。行业默认做法是引入 viem 或 ethers。本仓库没有这两个依赖(`package.json` 全文无 viem/ethers;01 号文档明示"没有 viem/ethers 依赖……全部手写",如 `src/services/abi.ts`、`src/services/safe-transaction.ts`)。

**决策**
所有链上底层原语手写、进本仓库、受本仓库测试门禁约束;不引入通用 web3 库。

**理由**
- **供应链投毒是钱包的头号威胁模型**:签名与编码路径上的第三方依赖一旦被投毒,等于把用户资金交给攻击者。触发这一担忧的直接事件是 axios 生态的投毒事故 —— **创始人陈述(2026-07-02)**。
- 本仓库自身随后被同类风险命中,构成自证:`xlsx` 0.18.5 存在原型污染 GHSA-4r6h-8v6p-xvw6 + ReDoS GHSA-5pgg-2g8v-p4x9,且 npm registry 版本停更无修复,处在用户输入路径(payroll 导入),最终以改用 SheetJS 官方 CDN 0.20.3 解决(`package.json:74`;commit `e18d1ff`;04 号文档 P1-7)。这发生在**非签名路径**的一个工具库上,尚且如此;签名路径依赖的风险敞口只会更大。
- 手写代码量可控且可被 1022 个单测钉住(04 号文档实测记录);依赖面小也让 `npm audit` 门禁(critical/high 归零)有实际意义。

**代价**
- 一切错误自己扛:"错一个字节 = 链上验签失败或资金操作错误,全部手写无 viem 兜底"(03 号文档"最难改区域"表)。
- 没有社区对该密码学/编码代码的持续审查;新维护者上手成本高(这正是本训练体系存在的原因之一)。
- 生态演进(新 EIP、新链特性)要自己跟进实现,无法搭库的便车。

**替代方案(当时被拒)**
- 引入 viem 并锁版本 + vendoring(拷贝进仓库审计后冻结):降低编写成本,但审计一个大型库的成本不低于手写所需子集,且升级时审计要重来。
- ethers v6:同上,且体积更大。
- 混合:仅签名路径手写,读路径用库 —— 依赖仍在 bundle 里,投毒面没有实质缩小。

**推翻条件**
- 团队扩张后,手写底层的维护成本(跟进新 EIP、修 bug)持续超过"锁版本 + 逐行审计 viem 子集"的成本;
- npm 生态出现可验证的供应链保证(签名溯源、可重复构建)且 viem 采用,使投毒威胁模型实质改变;
- 手写编码路径出现一次逃过测试门禁的真实资金相关缺陷 —— 那将证明"自写=自控"的前提失效,应触发重新评估而不是打补丁了事。
(基线诊断中创始人能讲清理由但"代价浅、推翻条件回避",14 号文档 D13;本条的推翻条件栏是 U7 重点对练对象。)

---

## ADR-002 passkey-only 无助记词 + 公钥索引服务作为恢复链路

**背景**
传统钱包用助记词做密钥备份,代价是用户抄写/保管负担与钓鱼面。Vela 的签名密钥是平台认证器(Secure Enclave / Credential Manager / navigator.credentials)里的 P-256 passkey,私钥永不进 JS(`src/modules/passkey/index.ts`;01 号文档技术栈表)。

**决策**
不提供助记词、不提供任何私钥导出/备份路径。跨设备恢复 = (a) 平台 passkey 同步(iCloud Keychain / Google Password Manager)+ (b) 自营公钥索引服务 p256-index.getvela.app 把 credentialId→Safe 地址映射找回来(`src/services/public-key-index.ts`;创建时上传带 3 次重试 + pending 队列,`src/services/public-key-upload.ts`;03 号文档流程 1)。

**理由**
- 消灭助记词 = 消灭最大的用户自伤面(抄错、拍照泄露、钓鱼交出)与新用户门槛;这是产品定位"无助记词、无插件"的根基(01 号文档一句话介绍)。
- 私钥在平台认证器内,恶意 JS/依赖投毒也拿不走签名密钥 —— 与 ADR-001 同属一个威胁模型的两面。
- 索引服务只存公开信息(公钥/地址映射),被攻破不损资金,只损恢复入口(06 号文档故障矩阵:"资金不受影响,可安抚用户")。

**代价**
- **永久锁定场景真实存在**:平台同步与索引服务两者都丢 → 地址找不回,"资金仍在链上,但无签名能力 = 永久锁定"(08 号文档 D 节)。
- **唯一副本窗口**:passkey 刚创建、平台云同步尚未完成(或用户根本没开 iCloud/Google 同步)期间,密钥只有设备上一份,设备丢即全丢;同理,公钥上传失败进 pending 队列期间,设备丢失 = 索引里没有这条映射(03 号文档流程 1 步骤 4;基线诊断中创始人自行推理出该窗口,14 号文档 D7 证据)。
- 索引服务是跨设备恢复单点(04 号文档 P2-6);D1 数据丢失的链上重建脚本尚不存在(08 号文档 C3)。
- 面向用户的恢复说明文档还没写(08 号文档 D 节),用户对"没有助记词"的信任成本高(营销文档列为头号疑虑,memory)。

**替代方案(当时被拒)**
- 助记词/私钥导出:重新引入全部自伤面,且与"私钥不出认证器"的安全承诺矛盾。
- 社交恢复/守护人:合约与产品复杂度大增,单人团队维护不起。
- MPC 托管分片:引入服务端资金权限,违背非托管定位。

**推翻条件**
- 真实用户锁定事故率达到不可辩护的水平(应在上架后开始统计恢复失败工单);
- 平台 passkey 同步被证明不可靠(如 Google/Apple 政策变化);
- 08-C3 重建脚本长期缺位且索引服务出过一次数据事故 —— 届时至少要给"高级用户可选的第二恢复因子"让步。

---

## ADR-003 counterfactual CREATE2 部署(首笔交易 initCode)

**背景**
Safe 智能合约钱包需要链上部署才存在。若创建钱包 = 立刻部署,每条链都要花一笔 gas,12+ 链成本不可接受,且没人替新用户垫付。

**决策**
创建钱包只做本地 CREATE2 地址推导(`src/services/safe-address.ts:192-210`,`computeSafeAddress`,工厂/单例/模块地址全链统一硬编码 `safe-address.ts:19-28`),Safe 不部署;首笔交易时经 UserOp 的 initCode 由 EntryPoint 完成部署(03 号文档流程 1 步骤 3、流程 2)。

**理由**
- 创建零成本、零延迟、离线可完成;地址在所有 EVM 链上一致(CREATE2 确定性),"一个 passkey = 一个全链地址"的产品叙事得以成立。
- 收款先于部署可用:地址算出来即可收款,资金落在未部署地址上是安全的(合约部署后归属同一 owner)。
- 与 4337 天然契合:initCode 是标准机制,不需要自建部署服务。

**代价**
- 首笔交易贵:部署 + 首笔约 3.9–4.1M gas(Tempo 实测注释,`src/services/tempo.ts:103`),用户对"第一笔怎么这么贵"缺乏预期。
- 全代码库要处理"已部署/未部署"双态:nonce 获取、签名验证(EIP-1271 对未部署账户不可用)、余额展示都要分支;提交链为此并行探测 deployed 状态(03 号文档流程 2 步骤 2)。
- 基线诊断显示这是易混淆点(创始人曾答错"MultiSend 负责部署",后自我修正;14 号文档复习队列第一条)——认知成本本身就是代价。

**替代方案(当时被拒)**
- 创建即部署(项目方赞助):12+ 链 × 每个新用户的赞助成本,与免费 Web 版模型(ADR-009)冲突。
- 首次收款时由服务端代部署:引入服务端资金与权限,增加信任面。

**推翻条件**
- 主要目标链的部署成本降到可忽略(L2 费用趋势)且赞助部署能换来可度量的转化率提升;
- 双态处理成为缺陷高发区(可从回归测试红的分布判断)。

---

## ADR-004 bundler 是 gas 价格权威,钱包 RPC 永不否决

**背景**
Gnosis 链上反复出现 "gas price too low" 提交失败与费率显示 "—" 的事故:钱包侧用自己的 RPC 报价去校验/替换 bundler 报价,而两者的 tip 口径不一致,钱包侧"善意的否决"直接把可上链的交易拦死(06 号文档"历史惨案区";memory: wallet↔bundler gas-price parity)。

**决策**
自营 bundler 的 `pimlico_getUserOperationGasPrice` 报价是唯一价格权威(`src/services/safe-transaction.ts:240,1461-1470` `getBundlerGasQuote`;定价优先级见 `safe-transaction.ts:340-361` 与 03 号文档流程 2 步骤 3)。钱包本地估算只做 bundler 不支持该方法时的回退;滥价保护 `isQuoteAbusive`(`safe-transaction.ts:1433`)只拒绝超过 bundler 自报网络价 3× 的报价,且在无独立可信 tip 时 fail-open(`safe-transaction.ts:1271`)——保护存在,但**永不**用钱包 RPC 的价去否决或替换 bundler 的价。

**理由**
- 结算责任决定定价权:是 bundler 的 EOA 在垫 gas、要把这笔 op 真正打上链;它对"多少钱能上链"的判断天然比钱包侧任何 RPC 更接近事实。
- 双权威必然打架:两个价格源意味着永远存在口径分歧(tip 是否含在内、不同 RPC 的基准费波动),历史事故证明分歧的代价由用户承担。
- 单一权威让回归可测:`bundler-service.test.ts` 钉住该原则(05 号文档发布检查)。

**代价**
- 信任集中于自营 bundler:它报错价(bug 或被攻破)时,钱包只有 3× 滥价上限这一层薄防御,且该防御在部分链上 fail-open。
- 跨仓库语义耦合:报价协议、错误文案(`parseBundlerUnderfunded`,`bundler-service.ts:367`)与 vela-bundler 仓库强耦合,改一边必须同步另一边(03/06/07 号文档反复强调)。
- Tempo 链例外处理(见 ADR-007)让"权威"规则带上脚注,增加理解成本。

**替代方案(当时被拒)**
- 多源取中位数:引入更多口径分歧源,恰是事故根因的放大版。
- 钱包侧设硬性下限/上限否决:就是事故前的做法,已被现实证伪。

**推翻条件**
- 接入第三方 bundler(失去对报价端的控制)时,必须重建价格信任模型;
- 自营 bundler 发生一次报价类资金损失事件,证明 3× fail-open 防御不够;
- 4337 生态出现标准化的报价证明机制。

---

## ADR-005 配置全部代码内常量 + AsyncStorage 覆盖,无 .env

**背景**
App 端(钱包本体)不存在 .env / EXPO_PUBLIC_* 体系(01 号文档"配置体系");RPC 列表、bundler 地址、服务端点等全部是代码内常量,用户可在设置里经 AsyncStorage 覆盖(键 `vela.serviceEndpoints`,`src/services/storage.ts:19`;`loadServiceEndpoints` 被 bundler/公钥索引等服务消费)。构建元信息(版本+commit)不例外:`app.config.js` 构建时求值注入 `extra.gitCommit`、经 `expo-constants` 读取,仍非环境变量体系(2026-07-02 前为脚本生成 `build-info.ts` 文件的旧机制,已废除)。注意边界:**getvela.app 子项目(服务端)不适用本条**——它的密钥走 `.dev.vars`(本地,gitignore,经 `git log --all` 验证从未入库)与 `wrangler secret put`(生产)(01 号文档)。

**决策**
App 端零环境变量:配置即代码、随版本审查与回滚;运行时差异只允许两种来源——用户显式覆盖(AsyncStorage)与 `__DEV__`/`dev_unlocked` 门控(01 号文档"环境差异"表)。

**理由**
- App 里**本来就不该有秘密**:任何打进客户端 bundle 的值都是公开的,.env 体系在客户端只会制造"这是秘密"的错觉(基线诊断证明这种错觉的危害:创始人误以为"全系统都不需要密钥",14 号文档 D10 一票否决项——真正的秘密在 Worker 侧)。
- 静态部署(CF Pages)没有服务端注入配置的时机;配置进代码使每个 deployment 自包含、回滚即回滚全部配置。
- 用户覆盖端点 = 主权逃生门:官方服务全挂时用户可自救(06 号文档故障矩阵 getvela.app 行)。

**代价**
- 改任何默认配置(如内置 RPC 列表)必须发版,无远程开关、无灰度;
- 常量散落在各 service 内,没有单一配置清单文件,新维护者需要建立"配置在哪"的地图;
- 与主流 Expo 项目惯例不同,外来贡献者会本能地找 .env。

**替代方案(当时被拒)**
- EXPO_PUBLIC_* 环境变量:多一套构建期状态,换不来任何客户端保密性;
- 远程配置服务:引入一个能改全体用户行为的服务端权力点,与非托管/最小信任定位冲突,也是新的单点。

**推翻条件**
- 出现必须小时级全体生效的配置类安全响应(如某内置 RPC 被劫持)且发版链路来不及——届时至少要为"端点黑名单"这类安全配置开一条经签名验证的远程通道;
- 多环境需求(staging/prod)复杂到 `__DEV__` + parallel space 承载不住。

---

## ADR-006 /android /ios 不入库,持久化改动走 config plugin

**背景**
`/android` 与 `/ios` 是 `expo prebuild` 生成物,被根 `.gitignore:42-43` 忽略(`git ls-files` 确认 0 个文件入库)。接管审计中发现 Android release 签名配置若直接手改 `android/`,会被 `expo prebuild --clean` 抹掉(04 号文档 P1-6)。注:01 号文档"仓库布局"节写"android/ ios/ 原生工程(已提交)"为**过期陈述**,以 .gitignore 与 04/05 号文档为准。

**决策**
原生工程目录不入库;一切需要持久化的原生改动写成 config plugin:`plugins/with-native-modules.js`(注入 `modules/vela-passkey` 等原生模块源码)与 `plugins/with-release-signing.js`(release 签名配置,commit `c250211`),均注册于 `app.json:75-76`,每次 prebuild 重放。

**理由**
- 单一事实源:app.json + plugins 完整描述原生工程,`expo prebuild` 可随时从零重建,消灭"生成物与配置漂移"这一整类 bug;
- 生成物入库的历史包袱(巨量噪音 diff、合并冲突、误手改)全部消失;
- plugin 可离线断言验证(04 号文档:with-release-signing 6/6 断言 + 幂等)。

**代价**
- **手改 android/ 或 ios/ 的任何东西都会静默丢失**——这是新维护者最容易踩的坑,必须形成"改原生 = 写 plugin"的肌肉记忆(05 号文档 Android 节警告);
- 写 config plugin 比直接改文件难一个量级(要理解 Expo config 插件 API 与目标文件结构);
- 无 keystore.properties 时构建回退 debug 签名只打 WARNING,产物"禁止上传"靠人记住(05 号文档)。

**替代方案(当时被拒)**
- bare workflow 原生目录入库:换取直接编辑的便利,代价是永久放弃 prebuild 可重建性,且历史上该仓库正是在"已提交的 android/ 带着 debug 签名"状态下埋了 P1-6。

**推翻条件**
- 原生定制深到 config plugin API 覆盖不了且 patch 手段(如 patch-package 对生成物无效)穷尽——届时 eject 为受管 bare 工程,并同步重写 05 号文档发布流程。

---

## ADR-007 Tempo 无原生币:稳定币 gas 报销内嵌 MultiSend

**背景**
Tempo 链(chainId 4217)没有原生 gas 币,gas 用 TIP-20 稳定币支付。创始人硬性约束:保持 EntryPoint + Safe + passkey 技术栈与全链地址一致性不动摇(memory: project_tempo_gas_integration,**创始人陈述**,先于本次记录)。这排除了"为 Tempo 换一套账户体系"的选项。

**决策**
UserOp 以 maxFee=0 签名(EntryPoint 侧不收原生费),在 UserOp 的 MultiSend 里内嵌一笔 `feeToken.transfer(bundlerEOA, reimbursement)` 直接报销 bundler(`src/services/tempo.ts:16-18`;03 号文档流程 3)。报销额 = 实际成本 × 2 边际(`tempo.ts:144` 附近,`≥1` 下限保证转账不为零);每个子调用 callGasLimit 垫到 380k(`TEMPO_CALL_GAS_PER_SUBCALL`,`tempo.ts:75`),因为 TIP-20 单笔 transfer 实测 ~308k、估算器会低报(`tempo.ts:68-73`,对 Tempo testnet 实测校准)。

**理由**
- 地址一致性保住了:Tempo 上的 Safe 地址与其他链相同(ADR-003 的 CREATE2 推导不变),用户心智不分裂;
- 不引入 paymaster 合约依赖:报销是一笔普通 transfer,审计面最小,bundler 收入路径(ADR-009)直接闭环;
- 全部差异收敛在一个纯函数模块里,可单测(`tempo.ts:23` 自述 "pure and unit-testable")。

**代价**
- 魔法数字与实测值:380k 垫值、2× 边际是实测校准而非推导值,链升级(TIP-20 gas 变化)会让它们悄悄失准;
- 批量逻辑被约束:"改批量逻辑时勿动报销 transfer 的位置"(03 号文档)——一个隐式排序不变量;
- 报销以估算为基准,极端 gas 波动下 bundler 可能亏损或多收(2× 边际是对冲,不是精确结算);
- ADR-004 的"bundler 报价权威"在 Tempo 有例外分支(memory: 钱包 getGasPrices 回退逻辑明确 exclude Tempo),规则复杂度 +1。

**替代方案(当时被拒)**
- ERC-20 paymaster:标准做法,但要部署/维护/审计一个持币合约,单人团队运维负担与攻击面都更大;
- EntryPoint native-map(把稳定币映射为"原生币"):依赖链方基础设施配合,不可控;
- Tempo 单独账户体系:直接违反创始人的地址一致性硬约束。

**推翻条件**
- Tempo 生态出现经审计的官方 paymaster 且多钱包采用;
- 实测校准值造成的报销偏差(亏损或用户投诉多收)超过可接受阈值;
- 需要支持第二条无原生币链时——魔法数字方案不可复制,应升级为通用机制。

---

## ADR-008 单一 SigningSheet 渲染路径(生产与测试 harness 同路)

**背景**
clear-signing 重建期确立(memory: project_clear_signing_rebuild,分支 feat/clear-signing-topgrade):签名确认界面是钱包的最后一道人肉防线,历史上"测试页面走简化渲染、生产走完整渲染"会让测试通过而生产出错,或反之。

**决策**
全部签名确认(dApp 实时请求、测试 harness 场景页、历史签名只读回放)渲染同一个 `SigningSheet` 组件——"the single presentational signing surface"(`src/components/SigningRequestModal.tsx:96,137`;03 号文档流程 4;回放复用见 03 号文档流程 5 与 memory: project_connection_activity_replay)。安全闸门(`enforceNoUnlimited`,`src/hooks/use-dapp-signing.ts:322,367`)钉在这条唯一路径的出口上;"任何新签名路径必须过这两道"(03 号文档)。

**理由**
- 测试的是真东西:E2E 与 harness 场景页锻炼的像素、文案、警告逻辑与生产完全同源,消灭"测试通过但生产渲染不同"这类最阴险的回归;
- 安全不变量只需守一个出口:never-unlimited、blind-sign 警告、模拟结果展示都只有一处可绕过点,审查成本最小;
- 回放(历史签名重现)免费获得与当时一致的展示。

**代价**
- 单组件多模态(实时/harness/只读回放)让 props 面与内部分支复杂,改一处要想三种模态;
- 组件是热点耦合区:签名相关的一切需求都汇到这里,容易长成上帝组件——需要靠"展示层 vs 决策层(hook)"的分界自律维持。

**替代方案(当时被拒)**
- 测试专用简化渲染:重建前的状态,正是要消灭的东西;
- 按方法类型拆多个 sheet:每多一个 sheet 就多一个需要重复部署安全闸门的出口。

**推翻条件**
- 模态分支复杂度实际造成回归(可观测:SigningSheet 相关测试红的频率),且拆分方案能证明每个新出口都机械化地继承全部安全闸门(如出口层共享同一强制中间件)时,才允许拆。

---

## ADR-009 Web 免费 + 商店付费 + bundler relayer 费:商业模型对架构的约束

**背景**
商业模型(01 号文档"用户与商业模型";08 号文档 A3):Web 版免费;iOS/Android 商店版付费下载(定价未定,创始人决策中);叠加自营 bundler relayer 费(约 2×/3× gas 上限加价)。收入核心用户 = 多链活跃转账者(memory: project_marketing_icp——"active multichain transactor = revenue core ≠ acquisition 入口")。单人团队,无融资叙事。

**决策**
把商业模型显式当作架构约束对待,而非营销层面的事。它决定了以下技术形态:

1. **bundler 必须自营且是默认路径**——relayer 费是持续收入的唯一来源(付费下载是一次性)。因此钱包默认指向自营 bundler(`src/services/bundler-service.ts`),ADR-004 的价格权威、跨仓库文案耦合、underfunded 充值闭环都是这条收入管道的工程配套。
2. **免费 Web 版 ⇒ 边际成本必须趋零**——静态导出上 CF Pages(01 号文档)、无服务端用户数据库、无迁移流程(05 号文档"数据迁移"节)、客户端 AsyncStorage 为唯一用户数据层(ADR-005)。可观测性薄弱(06 号文档"如实"节)同样是这个成本函数的产物,不是疏忽。
3. **带服务商 key 的 API 代理是成本泄漏点**——getvela.app 代理持有 Alchemy/Pimlico key 且无速率限制(04 号文档 P2-4),免费 Web 模型下这是被滥用刷爆配额的敞口,控制手段定为 CF WAF 规则 + 提供商用量告警(08 号文档 B3),而非在架构上加用户账号体系(那会破坏第 2 条)。
4. **商店付费版 ⇒ 单代码库双形态**——付费版不能是另一个产品,Expo 单代码库让 Web/商店版功能同源(01 号文档);商店文案不得写 "free app"(08 号文档 A3);付费墙差异只能出现在分发层,不进代码分支。
5. **收入用户 ≠ 获客入口**——免费 Web 承担获客与信任建立("新钱包凭什么信你"是头号障碍,memory),架构上表现为:核心资金功能在 Web 上零阉割,没有"付费解锁安全功能"这类分支。

**理由**
非托管钱包收不了资产管理费;单人团队养不起服务端重资产;relayer 费把收入与用户真实使用量(转账)对齐,与"多链活跃转账者"的 ICP 自洽。

**代价**
- 收入单点 = bundler:它宕机既是可用性事故也是收入事故(06 号文档故障矩阵);它的 EOA 私钥是全系统最敏感凭据(06 号密钥清单;基线诊断中被完全遗漏,14 号文档 D10);
- 免佣金压力恒在:用户可在设置覆盖 bundler 端点绕开 relayer 费(ADR-005 的主权逃生门与收入直接冲突——这是**有意接受的张力**,靠便利性而非锁定留住付费流量);
- 无遥测使"收入用户在流失"这类信号只能靠链上/bundler 侧数据推断;
- 90 天上架决策门槛(memory: project_90day_gates,GREEN=$2.5K 月收)悬在头上,商业模型若被证伪,上述所有约束的前提都要重估。

**替代方案(当时被拒)**
- 订阅制:非托管钱包没有断供手段,订阅无法执行;
- 代币/空投模型:与不募资、不发币的定位冲突;
- 完全免费 + 融资:创始人目标是 $10K MRR 的自持生意(memory: user_founder),不走这条路。

**推翻条件**
- 90 天门槛触 RED(转打工养项目)——架构随即要向"最低维护成本冻结"倾斜;
- relayer 费被规模化绕开(bundler 流水与活跃用户数脱钩)——需重新设计价值捕获点;
- 商店付费下载被验证为转化毒药——回到免费下载 + 何种应用内收入的重新决策。

---

## 维护约定

- 新的重大取舍(引入依赖、改恢复模型、改收入管道、加服务端状态)必须在此追加 ADR 后才动手;
- 推翻任何一条时不删除原文,状态改为"已推翻(见 ADR-0xx)"并新增继任条目;
- U7 训练(14 号文档)验收:受训者任选 ≥2 条,不看本文档口述其"代价"与"推翻条件",再补写一条本文档遗漏的决策。
