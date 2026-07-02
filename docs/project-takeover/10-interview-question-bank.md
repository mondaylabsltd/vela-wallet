# 10 — 接管面试题库(受训者可读版)

- **基线 commit**:`73d7aac`(2026-07-02)。所有题目与追问以该版本代码为准。
- **本文件只含题目与追问**。答案、代码证据与评分规则在 `11-interview-answer-key.md`,该文件为面试官专用,**模拟面试前后受训者均不得查看**。
- 难度标尺:1 = 代码定位/基础事实,3 = 机制精读,5 = 修改影响/深水推演。
- 共 6 个知识域 × 6 题 + 4 道综合面试题。

---

## D3-identity-chain — 身份与地址链条

考察范围:passkey/WebAuthn 注册与签名的平台分派、公钥索引服务与新设备恢复、Safe 地址推导(CREATE2/setupData/verifier 配置)——"你是谁、地址从哪来、换机怎么找回"。

### D3-identity-chain-Q1 · 难度 1/5

**题干**:用户点击签名后,Vela 调用 passkey 做 WebAuthn 断言。这个"到底走浏览器 WebAuthn 还是走原生模块"的分派逻辑在哪个文件?sign() 里的判断顺序是什么(共三层)?iOS 和 Android 在 JS 层有没有分开的分支?

**追问**:
1. parallel space 的固定密钥签名器为什么装在这一层而不是更上层的业务代码?生产包里这段代码去哪了?
2. 在 pages.dev 预览部署上,rpId 会变成什么?这对"同一个 passkey 跨环境用"意味着什么?
3. PasskeyErrorCode.CANCELLED 和 FAILED 的区分对上层 UI 有什么用?哪个错误名会被 web 端归为 CANCELLED?

### D3-identity-chain-Q2 · 难度 2/5

**题干**:public-key-upload.ts 里有个 validateCreateClientData,注释说要"在保存任何东西之前拒绝不兼容的 passkey 提供方"。它具体检查什么?为什么字段顺序会导致钱包不可用?然后关键问题:生产创建流程里,这个函数实际被谁调用?真正拦截不兼容提供方的门是哪段代码?

**追问**:
1. 顺着你对上一问的回答:现在的创建流程在哪个时点才会发现提供方不兼容?此时 pending upload 和服务器记录已经发生了什么?这算不算一个应该修的缺口?
2. UV flag 检查 (flags & 0x04) 对应 WebAuthn 里的什么?为什么 Safe 合约要求它?
3. 如果某提供方 create 格式正确但 get 字段顺序错(违反注释里的推理),现有代码会在哪一步兜住?

### D3-identity-chain-Q3 · 难度 3/5

**题干**:换新手机,用户在登录页做了一次 passkey 断言。断言响应里有公钥吗?没有的话,app 从哪里、通过什么协议拿回 P-256 公钥并重建钱包地址?这条恢复路径的实时依赖到底是什么——请精确到函数、URL 形态和默认端点。

**追问**:
1. p256-index.getvela.app 宕机时,老设备(本地有账户)和新设备(恢复)分别是什么体验?为什么一个无感一个被挡?
2. web 端在陌生域名上恢复,rpId 会变,queryRecord 会发生什么?WebAuthn 代理扩展怎么解决这个问题?
3. queryByWalletRef(按地址反查身份)和 queryRecord 用途有何不同?为什么前者超时要静默返回 null 而后者要抛错?

### D3-identity-chain-Q4 · 难度 4/5

**题干**:故障推演:创建钱包时,createRecord 请求发出后超时——响应丢了,你不知道服务器到底写没写入。走一遍代码:接下来发生什么?公钥会不会丢?会不会产生重复记录?"公钥本地永久保存 + 持续重试上传"这件事在代码里到底实现了没有——实现在哪?最后,公钥"唯一副本"只存在于这台设备的时间窗口是哪一段?

**追问**:
1. 如果 createRecord 返回 400(真 4xx,不是超时),uploadPublicKey 最终抛的是哪个错误?为什么 throw createError ?? verifyErr 这个顺序是对的?
2. pending 队列里为什么要存完整的 attestationObjectHex,而不是只存已提取的 publicKeyHex?
3. retryPendingUploads 对每条 pending 串行 try/catch 各自计数——如果索引服务器换了域名(用户改了设置),5 秒 URL 缓存会怎么影响这次重试?

### D3-identity-chain-Q5 · 难度 4/5

**题干**:encodeSetupData 里有一行 verifiers = abiEncodeUint256Hex('100')。这个 '100' 是什么?它被编码进哪个合约调用、最终影响什么?再看 attestation-parser.ts 的 derSignatureToRaw——为什么要把 s 归一化到 low-s?最后:对外(营销/文档)能不能说"Vela 的 P-256 验签靠 EIP/RIP-7212,以太坊主网和各链都支持"?依据代码,这句话哪里过满?

**追问**:
1. 如果某条已支持的链其实没有 0x100 precompile,用户在那条链上的体验是什么?失败会发生在交易生命周期的哪一步?
2. normalizeP256S 拿到 33 字节带前导零的 s 会怎么处理?derSignatureToRaw 前面哪几行处理了 DER 的符号位填充?
3. 为什么 low-s 归一化放在客户端解析签名时做,而不是让合约接受 high-s?

### D3-identity-chain-Q6 · 难度 5/5

**题干**:假设决定给 WebAuthn 签名器加一个 Solidity fallback verifier(即改 safe-address.ts 里 verifiers 的值,从裸 0x100 变成打包了 fallback 地址的 uint176)。修改影响题:(a) 直接改哪一行?(b) 这个改动会波及哪些行为——特别是老用户,分"已部署"和"未部署 counterfactual"两类说;(c) 文件头那句 "TypeScript port of SafeAddressComputer.swift" 意味着什么额外工作?这个 Swift 文件在仓库里吗?(d) 你用什么手段验证改动没把地址推导改坏?

**追问**:
1. 如果必须上这个改动,你会怎么设计版本化?版本信息放哪——本地 StoredAccount、索引服务器的 metadata 字段,还是推导时双跑新老参数比对链上 code?
2. PublicKeyRecord 里的 metadata/initialCredentialId 字段现在有没有被用起来?它们能不能承载推导参数版本?
3. SAFE_PROXY_RUNTIME_CODE 是从 PROXY_CREATION_CODE 现场切出来的——这个改动会影响它吗?为什么?

---

## D4-tx-chain — 交易构造与上链全生命周期

考察范围:UserOperation 的构造与字段、未部署账户的 initCode 部署机制、gas 定价三级优先级与 bundler 信任模型、提交/重试/回执韧性、Tempo 稳定币 gas——"一笔交易从签名到落地的每一环"。

### D4-tx-chain-Q1 · 难度 1/5

**题干**:一个从未在某条链上发过交易的 Safe 账户,第一笔交易是如何顺带完成合约部署的?部署数据由哪个文件的哪个函数构造?最终提交给 bundler 时,这段数据以什么 JSON 字段出现?MultiSend 在部署过程中到底出现在哪一层?

**追问**:
1. setupData 或 SAFE_SINGLETON 任何一个字节变了,CREATE2 地址会怎样?为什么这条性质是"跨链同址"承诺的根基?
2. 部署失败时 bundler 会报哪个 AA 错误码?钱包把它映射成什么用户文案?
3. 为什么未部署时 verificationGasLimit 要 2M 下限,而 Tempo 上要提到 6M?

### D4-tx-chain-Q2 · 难度 2/5

**题干**:本项目的 UserOperation 接口有哪些字段?你之前猜的 to/value/data 到底在哪里?提交给 bundler 时哪些字段做了 v0.7 拆分?passkey 签名到底签的是什么内容——签完之后还能改 gas 吗?

**追问**:
1. buildDummySignature 为什么必须和真签名同构等长?它伪造了哪些部分?
2. 同一次签名里,derSignatureToRaw 和 extractClientDataFields 分别解决 WebAuthn 输出的什么格式问题?
3. 如果 bundler 估出的 gas 比构造 dummy op 时的默认值高,代码流程如何保证签名仍然有效?

### D4-tx-chain-Q3 · 难度 3/5

**题干**:sendUserOp 给 maxFeePerGas 定价有一个三级优先级,分别是什么?bundler 在定价体系里除了提交交易还扮演什么角色?四个 gas 档位的倍数各是多少?什么情况下钱包会直接拒绝 bundler 的报价——拒绝之后是回退本地价还是拒发?

**追问**:
1. 为什么 Gnosis 上"钱包 RPC 否决 bundler 报价"会同时导致费率显示 "—" 和交易被拒?tipMeasured 字段解决了什么?
2. rawBundlerGasCost 为什么要把档位倍数从 totalWei 里除回去?不除会有什么用户可见的后果?
3. BUNDLER_MARGIN_NUM 旁有一条行内注释与实际值不一致——哪个是对的?这类陷阱怎么系统性防?

### D4-tx-chain-Q4 · 难度 4/5

**题干**:故障推演:某条链的 RPC 节点开始间歇性故障。分三种情况推演 sendUserOp 的行为:(a) eth_getCode 全部报错;(b) EntryPoint.getNonce 拉取失败;(c) eth_estimateUserOperationGas 失败。每种情况下,已部署/未部署账户、普通链/Tempo 链的行为有何差别?哪些情况会白白烧掉用户一次 passkey 弹窗?

**追问**:
1. 为什么"deployed + nonce 失败"必须在签名之前抛?如果放到提交后才失败,用户体验和安全上各损失什么?
2. 200 和 1024 两个字节阈值分别在保护什么?为什么不是一个阈值?
3. 如果要修 Tempo 的 nonce 不对称,你会改哪几行、如何验证不破坏未部署账户的首发路径?

### D4-tx-chain-Q5 · 难度 4/5

**题干**:用户点确认之后,依次遇到三种情况:(1) bundler 返回 'currently processing' 错误;(2) bundler 返回带 '[existingHash:0x…]' 标记的错误;(3) 提交成功但 120 秒内没有回执。钱包各做什么?乐观 nonce 缓存在哪种情况下会自增、哪种不会?重试的时候能不能顺手把 gas 价提高一点?

**追问**:
1. 为什么 existingHash 路径的轮询超时是 60 秒而主路径是 120 秒?
2. 'status unknown' 和 'not confirmed' 两个文案分别引导用户做什么?混成一个文案会引发什么用户行为?
3. waitForReceipt 的 AbortSignal 参数是给什么场景用的?中断后 op 本身会怎样?

### D4-tx-chain-Q6 · 难度 5/5

**题干**:修改影响题:假设 Tempo 官方新发了一种 TIP-20 稳定币(注意:它是 4 位小数),要求 Vela 把默认 gas fee token 从 pathUSD 换成它。你需要动哪些位置(包括本仓库之外)?哪些看不见的假设会被打破?你打算怎么验证这次修改?

**追问**:
1. 如果要新旧 token 共存(按用户余额自动选),feeToken 参数目前已贯穿哪些函数签名?还缺哪一环?
2. 为什么报销用 tempoExpectedGas(实测现实 gas)而不是 padded 的 UserOp limits 定价?用错的话用户大约多付几倍?
3. 验证阶段你如何确认 bundler 真的以新 token 收到了报销?从哪个地址、看哪笔转账?

---

## D5-D10-signing-security — 签名面板与纵深防御

考察范围:dApp 请求的分层路由与读请求限流、approval-guard 三件套与 clear signing 的边界、无限授权改写与提交咽喉兜底、交易模拟三态语义、SIWE 域名绑定——"签名面板背后每一层防线的职责与失效模式"。

### D5-D10-signing-security-Q1 · 难度 1/5

**题干**:一个已连接的 dApp 同时发来三个请求:eth_chainId、eth_getBalance、eth_sendTransaction。它们分别在哪个文件被分流、各走哪条处理路径?为什么恶意 dApp 用几千个 eth_getBalance 刷屏也不会延迟用户的签名确认弹窗?

**追问**:
1. handleReadOnlyRPC 对 eth_getCode 查询钱包自己地址时有什么特殊处理?为什么未部署时要返回 SAFE_PROXY_RUNTIME_CODE?
2. 为什么排队溢出要用 -32005 可重试错误拒绝,而不是让队列无限增长?
3. wallet_getCallsStatus 为什么要用 batchChainIds Map 记住批次提交时的链,而不是查当前链?

### D5-D10-signing-security-Q2 · 难度 2/5

**题干**:dApp 请求一笔 unlimited USDC approve。从弹窗渲染到最终上链,钱包对这笔授权做了哪几步处理?授权检测是 ERC-7730 clear signing 的一部分吗?最终上链的授权金额是 0、原值、还是别的什么?由谁决定?

**追问**:
1. ERC-721 的 approve(operator, tokenId) 和 ERC-20 approve 共用 selector 0x095ea7b3,为什么按 uint256 cap 处理仍然安全?
2. setApprovalForAll 没有金额可 cap,UI 上靠什么门控 grant?
3. rewriteApprovalParams 对 typed data 为什么要深拷贝且金额存为十进制字符串?

### D5-D10-signing-security-Q3 · 难度 3/5

**题干**:UNLIMITED_CAP_256 和 UNLIMITED_CAP_160 定义在哪、分别是多少?为什么选 2^200 和 2^152 这两个值?为什么 ERC-2612/Permit2 的 typed-data permit 被标记 editable: false,而且 enforceNoUnlimited 遇到它们直接 return 放行?放行之后这类请求靠什么防线兜底?

**追问**:
1. DAI permit 的 allowed 是布尔全额授权,它的 blockReason 和 ERC-2612 有何不同?为什么它也走 hold 门控?
2. 如果未来要支持一个金额字段是 uint96 的新协议,要在 approval-guard 里加什么?cap 选多少合适?
3. PermitBatch 里只要任意一个 detail.amount ≥ 2^152 就整体 isUnbounded,为什么不逐条展示编辑?

### D5-D10-signing-security-Q4 · 难度 4/5

**题干**:用户确认一笔 dApp swap 前,签名 sheet 里有一块"余额变动预览"。这块数据从哪来?故障推演:(a) 用户所有 RPC 节点都不支持 eth_simulateV1 时预览显示什么?(b) RPC 全部断网时呢?(c) 为什么 sim-engine-rpc 对顶层 error 返回 null 而不是 { ok: false }——如果写反了,用户会看到什么灾难性误报?

**追问**:
1. 为什么模拟 revert 时 deltas 保持为空([])而不是展示部分变动?
2. read-only replay 时预览数据从哪来?为什么不能现场重算?
3. underfundedNative 为什么在余额查询失败时选择"不警告"而不是"警告"?

### D5-D10-signing-security-Q5 · 难度 4/5

**题干**:故障推演:假设 UI 层出了 bug——ApprovalView 没渲染出来,或 confirm() 里 rewriteApprovalParams 抛异常导致 paramsOverride 为 undefined——一笔 unlimited approve 的原始 params 被原样传进 approveRequest。这笔授权会被签名上链吗?为什么?如果 dApp 改用 wallet_sendCalls 在批次第 3 个 leg 里夹带 unlimited approve 呢?这道兜底有哪些"有意放行"的形态?

**追问**:
1. 为什么 enforceNoUnlimited 放在 handleDAppRequest(共享路由层)而不是各 handler 内部或 transport 层?
2. handleSendCalls 的 per-leg 检查为什么伪装成 method='eth_sendTransaction' 来调 detectApproval?
3. 如果 dApp 把 unlimited 金额编码成 2^199(刚好低于 cap),守卫放行——这是漏洞吗?用户防线还剩什么?

### D5-D10-signing-security-Q6 · 难度 5/5

**题干**:需求变更:目前 SIWE 域名不匹配只弹红色警告,用户仍可签名。现在要求 binding === 'mismatch' 时直接禁用确认按钮。要动哪些文件哪些位置?'unknown' 状态怎么处理、为什么?有哪些边界情况会被误伤?改完怎么验证?

**追问**:
1. 为什么 siwe.ts 要拒绝 domain 里的 '@'?攻击者用 'uniswap.org@evil.com' 能骗过什么?
2. 如果 dApp 的 SIWE 消息 domain 带非默认端口(app.xyz:8443)而 origin 不带,现在的实现判 ok 还是 mismatch?为什么?
3. 升级为硬禁用后,dApp 收到什么响应更合理——用户手动 Reject,还是钱包自动回一个特定错误码?

---

## D6-D7-reads-and-state — 链上读取与本地状态

考察范围:余额/价格的 Multicall3 读取与缓存、RPC 池评分与两级封禁、AsyncStorage vela.* 键空间与清理盲区、交易记录 pending→confirmed 状态机——"数据怎么进来、状态怎么存、坏了怎么兜"。

### D6-D7-reads-and-state-Q1 · 难度 1/5

**题干**:首页的多链代币余额和价格是从哪里来的?请说出:(a) 负责的 service 文件名;(b) 它用什么合约/机制把一条链上十几个余额+价格查询压成几次 RPC;(c) 结果缓存多久、两个组件同时刷新同一地址会发几次网络请求?

**追问**:
1. includeZeroBalance 选项为什么要绕过缓存、也不写入缓存?
2. getCachedHeldTokens 为什么必须是同步、cache 冷时返回空而不触发 fetch?哪个安全功能在用它?
3. fetchTokens 抛错时缓存条目是怎么处理的?

### D6-D7-reads-and-state-Q2 · 难度 2/5

**题干**:Vela 所有本地持久化都在 AsyncStorage 的 vela.* 键空间下。请回答:(a) storage.ts 的 KEYS 表里有哪些键(说出至少 6 个)?(b) KEYS 表之外还有哪些模块自己持有 vela.* 键(说出至少 3 个,含文件)?(c) 登出时 clearAll() 会清掉哪些、漏掉哪些?

**追问**:
1. 登出后 vela.balanceCache 残留意味着什么?下一个在同设备创建钱包的用户会看到上任的余额吗?
2. 如果要把 clearAll 改成前缀清扫(getAllKeys + 前缀过滤),会误伤哪些应该跨登出保留的键?
3. vela.rpc.banned 残留对下一次冷启动是好事还是坏事?

### D6-D7-reads-and-state-Q3 · 难度 3/5

**题干**:故障推演:某轮刷新时 Polygon(137)的所有 RPC 端点都返回 HTTP 429,其他链正常。请推演:(a) rpc-pool 内部发生什么(重试?封禁?哪些集合被标记)?(b) 首页上 Polygon 的余额显示什么——清零还是保留?靠哪段代码保证?(c) 会弹"换 RPC"横幅吗?(d) 账户 USD 总额缓存会被这轮结果覆盖吗?

**追问**:
1. 同样场景换成所有端点返回 401,推演有什么不同?
2. Polygon 恢复后,哪段代码把 137 从两个集合里摘掉?
3. 怎么在开发环境手工复现这个场景?

### D6-D7-reads-and-state-Q4 · 难度 3/5

**题干**:用户在 dApp 里批准了一笔 eth_sendTransaction,链上确认要 30 秒。请回答:(a) 这条记录第一次写进 AsyncStorage 是在哪个时刻、状态是什么、由哪段代码写?(b) 确认后怎么更新——新写一条还是原地改?(c) 如果写入后 App 立刻被杀,重启后由哪些机制把它从 pending 收敛到 confirmed/failed?各自的触发点和保守规则是什么?

**追问**:
1. 为什么 flip confirmed 前必须 await pendingSave?去掉这个 await 会出什么竞态?
2. withTxLock 解决的是什么真实事故?
3. eth_getUserOperationReceipt 走的是 bundler 还是链 RPC?为什么 reconcile 要 12 秒节流?

### D6-D7-reads-and-state-Q5 · 难度 4/5

**题干**:rpc-pool 是所有链上读写的地基。请讲清:(a) endpointScore 的完整公式——来源优先级表、延迟罚分、成功加分、失败冷却各是多少?(b) 临时封禁和永久封禁的触发条件与时长(常量名+数值)?(c) 一个 JSON-RPC error 回来,代码按什么顺序把它分成四类(永久/瞬态/限流/getLogs 范围帽),各自的处置有何不同?(d) 一条链所有端点都被 ban 了怎么办?

**追问**:
1. 为什么 'exceeded' 同时出现在 isPermanentRpcError 和 isRateLimitSignal 里?一个限流错会不会被临时 ban?
2. 读操作和 bundler 操作的超时为什么不同?
3. 用户刚换掉某网络的自定义 RPC,为什么 refreshPool 还要顺手删 fastestRpcCache?

### D6-D7-reads-and-state-Q6 · 难度 5/5

**题干**:需求:把 models/types.ts 里 nftImageURL 硬编码的 ipfs.io 网关改成用户可配置(像 ethereumDataURL 一样进 ServiceEndpoints)。请回答:(a) 需要动哪些文件的哪些位置?(b) types.ts 和 storage.ts 之间现存什么地雷?你的新代码怎么写才不会踩?(c) 同步 getter 能工作的前提是什么、在哪里被满足?(d) 改完怎么验证?

**追问**:
1. 如果把 DEFAULT_SERVICE_ENDPOINTS 挪进 storage.ts 自己文件里,环就断了吗?还有哪些文件在 import 它?
2. 为什么 saveServiceEndpoints 后 rpc-pool 不会立刻看到新 bundler URL?哪个函数负责让池重读配置?
3. localePrefs 的 getter 为什么比 endpoints 多一套 listener + globalThis 锚定?

---

## D8-D9-framework-tests — 工程框架与验证基础设施

考察范围:expo-router 入口与 Hermes polyfill 加载顺序、测试体系与本地/CI 门禁、__DEV__ 与 dev_unlocked 双层门控、parallel space 测试环境、故障注入 harness——"改完代码之后,怎么证明它没坏"。

### D8-D9-framework-tests-Q1 · 难度 1/5

**题干**:你改完一段代码准备提交。在推到 main 之前,本地要跑哪几条命令?每条命令背后是多大规模的检查?另外有哪些测试默认不跑、分别用什么开关打开、为什么默认关?

**追问**:
1. 为什么 Playwright 被刻意排除在 CI 之外?什么条件下才提升进 CI?
2. RUN_NETWORK_TESTS 套件红了,你的第一判断是代码坏了还是别的?为什么?
3. npm run build:web 除了 expo export 还跑了什么(fix-cf-pages-assets.js 是干嘛的)?

### D8-D9-framework-tests-Q2 · 难度 2/5

**题干**:代码定位:这个 app 没有手写的 index.js,真正的入口在哪里定义?根组件是哪个文件?该文件第一行 import 是什么、为什么它必须排在第一行?native 和 web 分别加载哪个 polyfill 文件、各补了什么、不补会挂掉哪两个具体功能?

**追问**:
1. 如果格式化工具把 _layout.tsx 的 import 重排了,最先在哪个平台、哪个功能上暴雷?
2. polyfills.web.ts 为什么不能直接 re-export polyfills.ts?
3. Metro 是靠什么规则决定 web 加载 .web.ts 变体的?metro.config.js 里那段 nodeModulesPaths 又是为谁配的?

### D8-D9-framework-tests-Q3 · 难度 3/5

**题干**:项目里同时存在 __DEV__ 和 dev_unlocked 两种"开发者门控"。它们分别在什么阶段生效、各守着哪些东西?dev_unlocked 怎么打开?然后解释一个反直觉的设计:ParallelSpaceBadge 明明是个 dev 组件,为什么在 _layout.tsx 里必须无条件渲染、绝不能包进 __DEV__?

**追问**:
1. 怎么验证这些门控在 release 下的真实行为?
2. enterParallelSpace 为什么要顺手写 dev_unlocked='1'?不写会怎样?
3. 徽章为什么用 400ms 轮询而不订阅 listener 注册表?

### D8-D9-framework-tests-Q4 · 难度 4/5

**题干**:故障推演:你的真机上有真实钱包。你进入 parallel space 测试,测到一半 app 被杀,重启;继续测完后退出。请逐步推演 AsyncStorage 里发生了什么。三个追问:(a) 重启后 app 在真实空间还是 parallel 空间?(b) 如果退出时备份 JSON 解析失败,真实钱包会永久丢失吗?(c) 如果有人把 enterParallelSpace 里的 alreadyIn 检查删了,会造出什么事故?

**追问**:
1. exit 为什么必须清 walletpair/remoteInject 两个 session key?不清会在真实空间里看到什么?
2. 为什么备份/恢复的 key 刻意与 storage.ts 的 vela.accounts 完全一致?这让哪条签名解析路径对 fixture 零改动可用?
3. /parallel 布局的 enter effect 为什么"刻意不在 unmount 时取消"?

### D8-D9-framework-tests-Q5 · 难度 4/5

**题干**:你要修改 src/services/safe-transaction.ts 的 gas 计算逻辑(比如 calcMaxFeePerGas 或 deriveChainGasPrice)。"跑单测"为什么远远不够?请给出从改动到有信心合入的完整验证路径,并说明:要跑通真实链上验证,需要给哪两个不同的账户充值、各覆盖什么?

**追问**:
1. parallel-send.spec.ts 为什么刻意止步于 token picker?把深流程写进默认 e2e 会引入什么不稳定因素?
2. bundler gas account 与 Safe 自身余额各付什么钱?测试 skip 信息里给的两个地址分别是谁的?
3. 如果 e2e 里 gas 估算一直 pending,你会按什么顺序排查(wallet RPC 层 / bundler 报价 / 故障注入残留)?

### D8-D9-framework-tests-Q6 · 难度 5/5

**题干**:parallel-rate-limit.spec.ts 要证明"限流是冷静的、瞬态的降级:保留缓存余额、不弹 RPC 横幅"。推演:这个测试如何保证 app 的第一次余额加载就已经处于全链限流之下?为什么不能在页面加载完成后再在控制台执行 vela.rateLimitRpc('all')?整条链路上有哪几个专门为自动化留的 seam,各解决什么问题?如果有人把 fault-injection.ts 底部那个 IIFE 挪进 installFaultConsole 里,测试会怎么死?

**追问**:
1. 为什么这个 spec 把非 localhost 请求 abort 而 support/parallel.ts 的 stubWalletNetwork 却选择返回 200 空 JSON?两种策略各适合什么测试?
2. vela.rateLimitRpc 与 getRateLimitedChains 到 HomeScreen 的耦合链是什么?清掉故障后 UI 怎么自愈?
3. 如果要新增一种故障(比如 bundler 报价超时),你会在 fault-injection.ts 的哪几处各加什么?

---

## D11-D12-ops-external — 部署运维与外部依赖

考察范围:Web 构建部署链与回滚、CI 门禁与故意排除项、密钥全景与爆炸半径、跨仓库耦合(vela-bundler / getvela.app Worker)、生产排障序列——"怎么上线、坏了怎么救、钥匙在谁手里"。

### D11-D12-ops-external-Q1 · 难度 1/5

**题干**:现在要把 Web 钱包发一版到 wallet.getvela.app。请说出:(a) 完整的构建命令及它内部实际执行了什么;(b) 构建链里那个"修资产"的脚本在防什么事故,跳过它用户会看到什么;(c) About 页显示的 commit hash 是从哪来的、什么时机定下来的,能不能手改;(d) 发出去发现坏了,怎么回滚?

**追问**:
1. 如果 smoke test 时看到紫色 PARALLEL SPACE 徽章,说明发布环节哪里出了问题?对用户有什么风险?
2. expo 某次升级后字体不再输出到 assets/node_modules 了,fix 脚本会怎么表现?
3. 为什么 Web 钱包(CF Pages)和官网 API(CF Worker)的回滚手段不一样?

### D11-D12-ops-external-Q2 · 难度 2/5

**题干**:这个仓库有没有 CI?如果有:什么事件触发、有几个 job、每个 job 按顺序跑哪些检查?有哪两类测试被故意排除在 CI 外,理由分别是什么?这套 CI 现在最大的"未知数"是什么,验收标准是什么?

**追问**:
1. 为什么 lint 步骤要加 --max-warnings=10000 这种看似放水的参数?它实际把关的是什么?
2. 如果明天你 push 后 site job 红了但 app job 绿了,最可能是哪个目录的什么检查挂了?
3. 把 E2E 提进 CI 之前,08 手册定的量化标准是什么?

### D11-D12-ops-external-Q3 · 难度 3/5

**题干**:有人(可能是过去的你)说:"我们钱包是非托管的,服务端都是转发公开链上数据,所以没什么密钥要管。"请逐项反驳:这套系统实际存在哪些密钥/凭据?每一枚存放在哪里、通过什么命令进入生产、本地开发时放哪、泄漏或丢失分别是什么后果?

**追问**:
1. 如果 Alchemy key 泄漏被人刷爆,钱包用户第一时间会在哪些功能上感知到?轮换的完整四步是什么?
2. 为什么 bug-report 路由能优雅地在"无密钥"状态下运行,而 bundler 路由不能?这个设计差异说明什么?
3. 三枚 secret 里哪一枚的爆炸半径最小?为什么(权限范围角度)?

### D11-D12-ops-external-Q4 · 难度 4/5

**题干**:用户发交易时 bundler 报"gas 账户余额不足",钱包应该弹出充值 modal 而不是甩原始报错。(a) 这个错误检测逻辑在哪个文件哪个函数?(b) 它靠什么机制判断"这是 underfunded 错误"并提取充值信息?(c) 哪两条 UI 路径消费它?(d) 为什么另一个仓库里改一句错误文案能静默弄坏这里?现有防线有哪几道?

**追问**:
1. vela-bundler 明天把文案改成 'gas balance too low, top up 0x…',parse 会命中吗?逐个正则走一遍。
2. 为什么 asset 要区分 pathUSD 和 native?哪条链的 gas 不是原生币?
3. 如果你要把这个字符串耦合升级成结构化错误码,两个仓库各要改什么,过渡期怎么兼容旧版本 App?

### D11-D12-ops-external-Q5 · 难度 4/5

**题干**:凌晨两点,一键 bug report 进来一条 issue:"发不出交易,一直失败"。请给出你的诊断序列——每一步查什么、用什么工具/命令、什么现象指向什么结论。禁止"重启试试/看看日志"这类通用答案。追加:如果 getvela.app Worker 此刻整个挂了,钱包哪些功能死、哪些活?

**追问**:
1. 如果用户报的不是"发不出交易"而是"新设备恢复找不到钱包",嫌疑对象换成谁?最坏情况(D1 数据丢失)的恢复路径存在吗?
2. 为什么"余额显示正常"这条观察能帮你排除一整层嫌疑?
3. 在告警为零的现状下,你上任第一周会给哪三个信号配告警?

### D11-D12-ops-external-Q6 · 难度 5/5

**题干**:未决事项 B3:bundler/wallet/nft/transactions 四条代理路由没有任何速率限制(只有 bug-report 有)。假设你决定动手:(a) bug-report 现有的限流是怎么实现的,它有什么在注释里写明的已知缺陷?(b) 直接把这套照抄到 /api/bundler 会出什么问题?(c) 08 手册实际推荐的方案是什么,为什么不用改代码?(d) 无论选哪条路,完整的验证+发布+回滚序列是什么?

**追问**:
1. 为什么 bug-report 能容忍激进限流而 bundler 不能?从两者客户端失败路径的差异回答。
2. 如果要做 "production upgrade",KV 和 Durable Object 两种限流各有什么取舍?
3. B3 的验收里为什么要求"提供商用量告警配置截图"而不只是 CF 规则生效?

---

## 综合面试题(跨域)

规则:综合题在六个域的分域面试之后进行,允许面试官任意跨域追问。全程不得查阅代码、文档或笔记。

### 综合-Q1 · 两分钟项目介绍

**题干**:不看任何资料、不开 IDE,用两分钟向一位懂技术但从未接触过 Vela 的人完整介绍这个项目。必须覆盖三件事:① Vela 是什么、解决什么问题;② 技术上怎么工作——账户、签名、多链的主干,不超过四句话;③ 它怎么赚钱、现在处于什么阶段。超时打断,看资料即终止。

**追问**:
1. 你刚才说的某个事实(面试官任选一条)——出处是代码还是文档?具体在哪?
2. 对方追问"为什么我要信一个没有审计的新钱包",你怎么答?
3. 支持多少条链?这个数字你从哪核实的——README 可信吗?

### 综合-Q2 · 新需求改动范围:转账白名单

**题干**:产品需求:在设置里新增"转账白名单"开关——开启后,任何把资产转出钱包的操作(Send 单笔、split/sweep 批量、dApp 发起的交易)若收款方不在联系人(Contacts)里,必须被拦下:Send 流程直接禁止确认,dApp 流程降级为滑动确认并显示警告。请给出:(a) 需要动哪些文件/模块,尽量精确;(b) 三个最容易被漏掉的隐藏耦合;(c) 收款方地址到底从哪提取——对 native 转账、ERC-20 转账、批量 MultiSend、dApp 任意 calldata 分别说;(d) 开关状态存哪、登出会不会被清掉;(e) 验证方案。

**追问**:
1. ERC-20 transfer 的 tx.to 是谁?收款人在哪个字节?
2. dApp 的一笔 approve 算不算"转出资产"?你的白名单拦不拦?为什么?
3. 为什么这个校验不能只做在 UI 层?本项目哪条既有防线的分层模式可以直接参考?

### 综合-Q3 · 生产事故复盘:充值 modal 静默失效

**题干**:场景:昨天 vela-bundler 仓库合入了一个"改进错误提示措辞"的小 PR。今天 14:00 起,一键 bug report 涌入十几条"发不出交易,弹了一个看不懂的英文报错"。14:40 你确认:gas 账户不足的用户看到的是原始报错,充值 modal 不再弹出。请主持一次复盘:(a) 重建故障链——从 bundler 改文案到用户看到原始报错,中间每一环发生了什么;(b) 本应拦住它的防线有哪几道,为什么全部漏过;(c) 给出立即修复、短期加固、长期根治三层行动项;(d) 这次事故暴露的最深层问题是什么——代码问题还是流程问题?

**追问**:
1. 钱包侧的回归测试当时是绿的,为什么事故还是发生了?
2. 长期根治如果换成结构化错误码,存量旧版本 App 怎么兼容?
3. 这类事故你现在能靠什么第一时间发现?——这个答案本身说明什么问题?

### 综合-Q4 · 证明这代码是你理解的

**题干**:开放题,20 分钟。从这个代码库里挑一处你认为最能证明"我真的理解这个系统"的代码——可以是一个函数、一条跨模块耦合、一个不变量。要求:(a) 白板讲清它解决什么问题、为什么写成现在这样;(b) 回答反事实:如果删掉它或把关键判断写反,第一个坏掉的用户场景是什么;(c) 面试官会连续追问三层"为什么",并要求你现场给出 file:line 级的定位;(d) 最后必须说出一件你还不确定或没读透的相关细节——说不出任何不确定点会被扣分。

**追问**(面试官按需选用):
1. 为什么不用更简单的写法?(面试官现场提出一个看似合理的简化方案)
2. 这段代码依赖的哪个假设最可能在半年内被打破?
3. 你刚才给的行号,你上次核对是什么时候?基线 commit 之后有没有动过?

