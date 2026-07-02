<!-- ⚠️⚠️⚠️ 面试官专用文件。模拟面试期间不得向受训者展示本文件。 -->
<!-- 受训者在完成全部模拟面试并出分之前,阅读本文件即视为泄题,该轮面试作废。 -->

# 11 — 接管面试答案库(面试官专用)

> **警告:模拟面试期间不得向受训者展示本文件。** 受训者只能看 `10-interview-question-bank.md`。

- **基线 commit**:`73d7aac`(2026-07-02)。所有 file:line 以该版本为准;若代码已演进,以"语义等价位置"判分,行号偏移不扣分。
- 每题结构:难度 / 考察目标 / 题干 / 标准答案要点 / 代码证据(file:line)/ 常见错误 / 追问 / 真懂 vs 背诵判据 / 评分规则(满分 5)。
- "常见错误"中标注【受训者原错】的条目来自基线诊断,是该题存在的直接原因——答错这些条目视为未纠正原错,不得高于 2 分。

---

## D3-identity-chain — 身份与地址链条

### D3-identity-chain-Q1 · 难度 1/5

**考察目标**:代码定位:能指出 passkey 注册/签名的平台分派发生在哪个文件、分派顺序是什么,破除"概念强但零代码引用"的习惯。

**题干**:用户点击签名后,Vela 调用 passkey 做 WebAuthn 断言。这个"到底走浏览器 WebAuthn 还是走原生模块"的分派逻辑在哪个文件?sign() 里的判断顺序是什么(共三层)?iOS 和 Android 在 JS 层有没有分开的分支?

**标准答案要点**:
1. 文件是 src/modules/passkey/index.ts,统一 API 覆盖 iOS/Android/Web。
2. sign() 的分派顺序:先查 `__DEV__ && __override`(parallel space 固定密钥签名器)→ 再查 isWeb 走 webSign(navigator.credentials.get)→ 否则 assertNativeAvailable() 后调 VelaPasskey.sign。
3. iOS 和 Android 在 JS 层没有分开分支——两者都走同一个 VelaPasskey 原生模块(NativeModules.VelaPasskey),平台差异(ASAuthorization vs Credential Manager)在原生代码里。
4. isWeb 由 Platform.OS === 'web' 决定;原生模块缺失时抛 PasskeyError(PASSKEY_NOT_AVAILABLE)。
5. rpId 也按平台分派:原生固定 getvela.app;web 端从 hostname 提取可注册域(wallet.getvela.app → getvela.app),localhost/IP 原样用,还有 WebAuthn 代理扩展的 `__VELA_WEBAUTHN_PROXY_RPID__` 全局覆盖优先。
6. web 端 sign 有 AbortController:新请求会先 abort 上一个 pending 请求,避免 'a request is already pending'。

**代码证据**:
- `src/modules/passkey/index.ts:16-17` — const { VelaPasskey } = NativeModules; isWeb = Platform.OS === 'web'
- `src/modules/passkey/index.ts:178-187` — sign() 三层分派:__DEV__ override → isWeb→webSign → assertNativeAvailable+VelaPasskey.sign
- `src/modules/passkey/index.ts:159-173` — register()/authenticate() 同样的三层分派结构
- `src/modules/passkey/index.ts:33-54` — getRelyingPartyId():代理扩展全局变量优先、localhost/IP 原样、*.getvela.app 归一为 getvela.app
- `src/modules/passkey/index.ts:126-138` — PasskeyOverride/__setPasskeyOverride,仅 __DEV__ 生效(parallel space 唯一功能差异)
- `src/modules/passkey/index.ts:274-277` — webSign 先 abort 上一个 _signAbortController
- `src/modules/passkey/index.ts:354-361` — assertNativeAvailable 抛 PASSKEY_NOT_AVAILABLE

**常见错误**:
- 只讲 WebAuthn 概念(challenge、断言),说不出文件名和函数名(受训者诊断中的零代码引用问题)。
- 以为 iOS 和 Android 在 JS 里各有一条 if 分支(实际共用一个 VelaPasskey 模块)。
- 漏掉第一层 dev override,或以为它在生产也可能被触发(实际 __setPasskeyOverride 非 __DEV__ 直接 return)。
- 以为 rpId 永远是 getvela.app(web 端在非 getvela.app 域名下会用 hostname)。

**追问**:
1. parallel space 的固定密钥签名器为什么装在这一层而不是更上层的业务代码?生产包里这段代码去哪了?
2. 在 pages.dev 预览部署上,rpId 会变成什么?这对"同一个 passkey 跨环境用"意味着什么?
3. PasskeyErrorCode.CANCELLED 和 FAILED 的区分对上层 UI 有什么用?哪个错误名会被 web 端归为 CANCELLED?

**真懂 vs 背诵**:真懂的人能按 sign() 的实际代码顺序说出三层分派并解释为什么 override 必须在最前;背诵的人只会说"web 用 navigator.credentials、原生用系统 API"这种任何钱包都成立的话。

**评分规则**:3 分线:说对文件 + isWeb/原生两层分派 + JS 层不分 iOS/Android;5 分线:再答出 dev override 第一层、rpId 按平台/域名的推导规则、AbortController 细节中至少两项。

### D3-identity-chain-Q2 · 难度 2/5

**考察目标**:考察为什么要在保存前拒绝不兼容 passkey 提供方,以及"设计意图 vs 实际接线"的差距——validateCreateClientData 在生产里其实没有调用点。

**题干**:public-key-upload.ts 里有个 validateCreateClientData,注释说要"在保存任何东西之前拒绝不兼容的 passkey 提供方"。它具体检查什么?为什么字段顺序会导致钱包不可用?然后关键问题:生产创建流程里,这个函数实际被谁调用?真正拦截不兼容提供方的门是哪段代码?

**标准答案要点**:
1. validateCreateClientData 把 clientDataJSONHex 解码后检查:必须以精确前缀 `{"type":"webauthn.create","challenge":"` 开头、以 `}` 结尾,否则抛 PasskeyIncompatibleError(明确标注不可重试)。
2. 原理:Safe 链上 WebAuthn verifier 按固定字段顺序重建/校验 clientDataJSON;注释的推理是"提供方在 create 时字段顺序错,get(签名)时也会错",所以能在注册阶段提前拒绝。
3. 如果不拦截,会创建一个链上永远验不过签的钱包——地址能推导、能收钱,但花不出去。
4. 签名数学不在本地检查:webauthn-verify.ts 注释明确说安全区产生的签名一定数学正确,真正的兼容性问题只在 clientDataJSON 字段顺序,已知例子是 Xiaomi Password Manager。
5. 关键事实:validateCreateClientData 在生产代码里没有任何调用点,只有单元测试 import 它(public-key-upload.test.ts)。
6. 生产里实际的门是 verifySafeWebAuthn(检查 webauthn.get 前缀、结尾 }、authData≥33 字节、UV flag 0x04):创建流程在 CreateWalletScreen 的 handleSignIn 里用一次真实测试断言(challenge 为 'vela-verify-'+Date.now())过这道门;登录/恢复在 OnboardingScreen 断言后也过这道门。

**代码证据**:
- `src/services/public-key-upload.ts:36-59` — validateCreateClientData:requiredPrefix 检查 + endsWith('}') 检查,抛 PasskeyIncompatibleError
- `src/services/public-key-upload.ts:17-26` — PasskeyIncompatibleError 注释:NOT retryable,设备/提供方不可用
- `src/services/public-key-upload.ts:31-34` — 注释:create 字段顺序错 ⇒ get 也会错,可在保存前拒绝
- `src/services/webauthn-verify.ts:10-13` — P256 签名数学不检查;真实兼容问题是字段顺序(e.g. Xiaomi Password Manager)
- `src/services/webauthn-verify.ts:18,37-62` — REQUIRED_PREFIX = {"type":"webauthn.get",... + UV flag (flags & 0x04) 检查在 authData[32]
- `src/screens/onboarding/CreateWalletScreen.tsx:196-208` — handleSignIn:测试 challenge 真实断言 + verifySafeWebAuthn 不通过则弹 incompatible 警告
- `src/screens/onboarding/OnboardingScreen.tsx:97-104` — 登录断言后 verifySafeWebAuthn 门
- grep 全仓库确认:validateCreateClientData 仅 src/__tests__/services/public-key-upload.test.ts 调用,无生产调用点

**常见错误**:
- 说它在验证签名的密码学正确性(webauthn-verify.ts 注释明确排除了这点)。
- 说它是防钓鱼/防重放的安全检查(实际是 Safe 合约的 clientDataJSON 字段顺序兼容性)。
- 想当然认为它已经接在注册流程里(受训者零代码引用的典型翻车点——设计文档说的和代码接线不一致)。
- 把 create 前缀和 get 前缀混为一谈,说不出生产门用的是 webauthn.get 检查 + UV flag。

**追问**:
1. 既然 validateCreateClientData 没被调用,现在的创建流程在哪个时点才发现提供方不兼容?此时 pending upload 和服务器记录已经发生了什么?这算不算一个应该修的缺口?
2. UV flag 检查 (flags & 0x04) 对应 WebAuthn 里的什么?为什么 Safe 合约要求它?
3. 如果某提供方 create 格式正确但 get 字段顺序错(违反注释里的推理),现有代码会在哪一步兜住?

**真懂 vs 背诵**:真懂的人会主动指出"这个函数其实没接线,真正的门是 handleSignIn 的测试断言 + verifySafeWebAuthn"并能评估时序缺口;背诵的人复述注释里的设计意图,默认它已经在跑。

**评分规则**:3 分线:说清检查内容 + 字段顺序为什么致命 + 不可重试;5 分线:额外指出无生产调用点、说出实际的 verifySafeWebAuthn 门及其检查项和调用时点。

### D3-identity-chain-Q3 · 难度 3/5

**考察目标**:针对受训者的真实错误(把恢复实时依赖说成 Gnosis RPC):考察新设备恢复时公钥的完整回取路径与真正的实时依赖。

**题干**:换新手机,用户在登录页做了一次 passkey 断言。断言响应里有公钥吗?没有的话,app 从哪里、通过什么协议拿回 P-256 公钥并重建钱包地址?这条恢复路径的实时依赖到底是什么——请精确到函数、URL 形态和默认端点。

**标准答案要点**:
1. 断言响应(PasskeyAssertionResult)只有 credentialId/signatureHex/authenticatorDataHex/clientDataJSONHex/可选 userIdHex,没有公钥字段;公钥只在注册时的 attestationObject 里能提取(extractPublicKey 走 CBOR → authData → COSE key 的 -2/-3 取 x/y)。
2. 恢复顺序:Passkey.authenticate() → verifySafeWebAuthn 兼容门 → 先查本地 loadAccounts 按 credentialId 匹配 → 未命中才查索引服务器。
3. 实时依赖是 HTTP API,不是 Gnosis RPC:PublicKeyIndex.queryRecord(rpId, credentialId) 发 GET `${baseUrl}/api/query?rpId=...&credentialId=...`;服务器把公钥存在 Gnosis 链上,但那是服务端实现细节,客户端从不直接读链。
4. 默认端点是 https://p256-index.getvela.app(DEFAULT_SERVICE_ENDPOINTS.passkeyIndexURL),用户可在设置改;getBaseUrl 有 5 秒 TTL 缓存,读 storage 失败回退 FALLBACK_URL。
5. 拿到 record.publicKey 后调 computeAddress(record.publicKey) 在本地重新推导出同一个 Safe 地址,然后 saveAccount 落地。
6. 失败分支:404 → "未找到"提示;网络错误 → 标记 endpoint unreachable 并打开设置面板;查询超时 8 秒(NET_TIMEOUTS.keyIndexRead)。

**代码证据**:
- `src/modules/passkey/index.ts:92-98` — PasskeyAssertionResult 字段定义,无公钥
- `src/services/attestation-parser.ts:23-45` — extractPublicKey 只从 attestationObject 提取 x/y(注册时唯一机会)
- `src/screens/onboarding/OnboardingScreen.tsx:108-131` — 本地优先,未命中调 PublicKeyIndex.queryRecord(rpId, assertion.credentialId)
- `src/services/public-key-index.ts:78-84` — queryRecord:HTTP GET /api/query?rpId=&credentialId=,fetchWithTimeout
- `src/services/public-key-index.ts:2-6` — 头注释:服务器把公钥存 Gnosis Chain,客户端无签名/挑战、纯 HTTP
- `src/models/types.ts:309` — passkeyIndexURL 默认 'https://p256-index.getvela.app'
- `src/services/public-key-index.ts:14-30` — getBaseUrl 5s TTL 缓存 + FALLBACK_URL 回退
- `src/services/net.ts:41` — keyIndexRead: 8_000
- `src/screens/onboarding/OnboardingScreen.tsx:134` — computeAddress(record.publicKey) 本地重推导地址
- `src/screens/onboarding/OnboardingScreen.tsx:151-160` — 404 → not found 弹窗;网络错误 → endpointUnreachable + 打开设置

**常见错误**:
- 【受训者原错】说"客户端通过 Gnosis RPC 读链上合约拿公钥"(链只是服务端存储层,客户端依赖是 HTTP API)。
- 以为断言响应里带公钥,或以为公钥能从签名反推。
- 以为恢复靠 iCloud/云备份或本地缓存,说不出索引服务器这一环。
- 说不出查询键是 (rpId, credentialId),或不知道 rpId 参与查询(rpId 不一致会查不到)。

**追问**:
1. p256-index.getvela.app 宕机时,老设备(本地有账户)和新设备(恢复)分别是什么体验?为什么一个无感一个被挡?
2. web 端在陌生域名上恢复,rpId 会变,queryRecord 会发生什么?WebAuthn 代理扩展怎么解决这个问题?
3. queryByWalletRef(按地址反查身份)和 queryRecord 用途有何不同?为什么前者超时要静默返回 null 而后者要抛错?

**真懂 vs 背诵**:真懂的人能一口说出"断言不含公钥→HTTP GET /api/query→computeAddress 重推导"这条链并强调客户端从不碰 Gnosis RPC;背诵的人在"链上存储"和"客户端依赖"之间含糊,又滑回 RPC 的说法。

**评分规则**:3 分线:断言无公钥 + HTTP API 查询(非链上 RPC)+ computeAddress 重推导;5 分线:再给出本地优先顺序、查询键 (rpId,credentialId)、默认端点/超时/失败分支中至少两项。

### D3-identity-chain-Q4 · 难度 4/5

**考察目标**:故障推演 + 针对受训者"不知道本地保存与持续重试是否已实现"的盲区:createRecord 超时后的完整行为、幂等机制与公钥唯一副本窗口。

**题干**:故障推演:创建钱包时,createRecord 请求发出后超时——响应丢了,你不知道服务器到底写没写入。走一遍代码:接下来发生什么?公钥会不会丢?会不会产生重复记录?"公钥本地永久保存 + 持续重试上传"这件事在代码里到底实现了没有——实现在哪?最后,公钥"唯一副本"只存在于这台设备的时间窗口是哪一段?

**标准答案要点**:
1. uploadPublicKey 把 create 的失败先记在 createError 而不立即抛,随后 queryRecord 验证——服务器记录才是 source of truth:超时但实际写入成功的场景会被验证确认,视为成功。
2. 验证通过且 record.publicKey 匹配 → removePendingUpload;验证也失败 → throw createError ?? verifyErr,注释明确"未确认的结果绝不移除 pending、绝不伪造成功"。
3. 不会产生重复记录:createRecord 带 Idempotency-Key: `${rpId}:${credentialId}`,且 (rpId, credentialId) 是自然键,后端即使忽略该头,同一 passkey 也不会有第二条。
4. 本地保存已实现:CreateWalletScreen 在上传之前就 savePendingUpload(含 publicKeyHex 和完整 attestationObjectHex)进 AsyncStorage;且刻意不 saveAccount——账户只有在服务器确认后才落地,防止"本机能用、异地永远恢复不了"的静默缺口。
5. 持续重试已实现,两层:创建界面内 tryUpload 自动重试 3 次(1s/2s 退避)+ 失败后的手动 Retry 按钮;每次 app 启动 _layout.tsx 检查 hasPendingUploads 后静默调 retryPendingUploads(无需生物识别)。
6. 唯一副本窗口:从 passkey 注册成功(extractPublicKey 得到公钥)到服务器验证确认之间,公钥唯一副本在本设备 pending 队列里——这期间设备丢失/清数据 = 该钱包无法在任何其他设备恢复;产品缓解是地址只在成功页才展示,未同步的钱包不会被打钱,所以没有资金风险。

**代码证据**:
- `src/services/public-key-upload.ts:84-91` — create 失败只记 createError 并 warn,'verify before deciding'
- `src/services/public-key-upload.ts:96-105` — queryRecord 为 source of truth;throw createError ?? verifyErr;注释:never remove pending on unconfirmed, never fake success
- `src/services/public-key-upload.ts:107-113` — publicKey 不匹配抛错;确认后才 removePendingUpload
- `src/services/public-key-index.ts:58-64` — Idempotency-Key: rpId:credentialId,注释解释超时重试折叠为一条记录
- `src/screens/onboarding/CreateWalletScreen.tsx:127-141` — 先 savePendingUpload(含 attestationObjectHex),注释解释为何不先 saveAccount(异地不可恢复的静默缺口 + 地址只在成功页展示所以无资金风险)
- `src/screens/onboarding/CreateWalletScreen.tsx:63-84` — tryUpload:maxAttempts=3,1s/2s 退避
- `src/app/_layout.tsx:171-177` — 启动时 hasPendingUploads() → retryPendingUploads().catch(() => {})
- `src/services/public-key-upload.ts:118-126` — retryPendingUploads 遍历 loadPendingUploads,注释:无需生物识别、可静默跑
- `src/services/storage.ts:85-104` — savePendingUpload/loadPendingUploads/removePendingUpload:AsyncStorage 队列,按 id 去重
- `src/services/net.ts:42-43` — keyIndexWrite 15s(后面压着一笔链上写入)

**常见错误**:
- 【受训者原错】回答"不确定重试实现了没有"(正确答案必须指到 _layout.tsx 的启动钩子和 storage.ts 的 pending 队列)。
- 以为 create 失败就直接报错终止,漏掉"先验证再决定"的核心设计。
- 以为超时重试会在服务器产生两条记录(漏掉 Idempotency-Key 和自然键双保险)。
- 以为账户创建后立即 saveAccount(实际刻意延后到服务器确认,这是防静默恢复缺口的关键决策)。
- 把唯一副本窗口说成"永远存在"或"不存在"(实际是注册成功到服务器确认之间的有限窗口,且有"未同步不展示地址"的资金缓解)。

**追问**:
1. 如果 createRecord 返回 400(真 4xx,不是超时),uploadPublicKey 最终抛的是哪个错误?为什么 throw createError ?? verifyErr 这个顺序是对的?
2. pending 队列里为什么要存完整的 attestationObjectHex,而不是只存已提取的 publicKeyHex?
3. retryPendingUploads 对每条 pending 串行 try/catch 各自计数——如果索引服务器换了域名(用户改了设置),5 秒 URL 缓存会怎么影响这次重试?

**真懂 vs 背诵**:真懂的人能画出"先写 pending → create → verify 兜底 → 确认才清 pending/存账户"的状态机并指出两层重试的确切位置;背诵的人只会说"有重试机制、有幂等"但指不出 _layout.tsx 和 verify-as-source-of-truth 这两个关键点。

**评分规则**:3 分线:说清 verify 兜底(超时≠失败)+ pending 不清除会重试 + 幂等键防重复;5 分线:再答出启动钩子位置、"确认前不 saveAccount"的理由、唯一副本窗口及其资金缓解。

### D3-identity-chain-Q5 · 难度 4/5

**考察目标**:针对受训者对 EIP/RIP-7212 落地状态说得过满的问题:考察 verifier 常量 0x100 的真实含义、low-s 归一化的项目内理由、以及对外表述的边界。

**题干**:encodeSetupData 里有一行 verifiers = abiEncodeUint256Hex('100')。这个 '100' 是什么?它被编码进哪个合约调用、最终影响什么?再看 attestation-parser.ts 的 derSignatureToRaw——为什么要把 s 归一化到 low-s?最后:对外(营销/文档)能不能说"Vela 的 P-256 验签靠 EIP/RIP-7212,以太坊主网和各链都支持"?依据代码,这句话哪里过满?

**标准答案要点**:
1. '100' 是十六进制 0x100 = 256,即 RIP-7212 P-256 precompile 的约定地址;它作为 WebAuthnSigner 的 configure((uint256,uint256,uint176)) 第三个参数(verifiers),和公钥 x/y 一起在 Safe.setup 的 MultiSend delegatecall 里配置进签名器。
2. uint176 的 verifiers 里只填了 precompile 地址,没有配任何 Solidity fallback verifier——链上没有这个 precompile 时没有兜底。
3. 这个配置在 setupData 里,而 salt = keccak256(keccak256(setupData) || saltNonce) → CREATE2 地址;所以 verifier 配置是地址推导的输入之一:它必须在所有链取同一值,才能保证"同一公钥、所有链同一地址"。
4. low-s 归一化:derSignatureToRaw 把 DER 转 raw r||s 后,若 s > n/2 则 s = n − s(P256_N 常量硬编码);代码注释给出的理由是 RIP-7212 precompile 拒绝 s > n/2 的签名。
5. 对外表述必须收敛:代码只是把 verifier 硬编码为 0x100 且没有任何按链分支——precompile 是否真的部署取决于每条链自身;"各链/主网都支持 7212"不是这份代码能背书的话,只能说"签名器被配置为调用 RIP-7212 约定地址,支持该 precompile 的链上可验"。
6. 文件头注释写明这些合约地址对"所有 EVM 链"相同(all EVM chains)——一致性是刻意设计,不是巧合。

**代码证据**:
- `src/services/safe-address.ts:117-126` — configure((uint256,uint256,uint176)) selector + verifiers = abiEncodeUint256Hex('100'),注释 'RIP-7212 P256 precompile'
- `src/services/safe-address.ts:128-131` — configure 作为 delegatecall 打进 MultiSend(tx2, WEBAUTHN_SIGNER)
- `src/services/safe-address.ts:202-210` — salt = keccak256(abi.encode(keccak256(setupData), nonce)) → create2Address:verifier 配置进入地址推导
- `src/services/attestation-parser.ts:89-92` — 注释:normalize s to low-s for RIP-7212 P256 precompile compatibility
- `src/services/attestation-parser.ts:101-122` — P256_N/P256_HALF_N 常量 + normalizeP256S(s > n/2 时 s = n − s)
- `src/services/safe-address.ts:19-28` — 'Contract Addresses (all EVM chains)':工厂/单例/签名器等地址全链统一
- grep 确认:safe-address.ts / attestation-parser.ts 无任何 per-chain 分支或 fallback verifier 配置

**常见错误**:
- 【受训者原错】说"EIP-7212 已在以太坊主网上线,所以所有链都能验"(代码层面只有一个硬编码假设,没有逐链事实)。
- 把 low-s 解释成"防签名可延展性的通用最佳实践/像比特币那样"(项目内的直接理由是注释写明的 precompile 拒收 s > n/2)。
- 认为 verifier 配置和地址无关、以后随便换(实际它在 setupData → salt → CREATE2 链条里)。
- 看不出 uint176 里没有 fallback verifier,以为链上有软件验签兜底。

**追问**:
1. 如果某条已支持的链其实没有 0x100 precompile,用户在那条链上的体验是什么?失败会发生在交易生命周期的哪一步?
2. normalizeP256S 拿到 33 字节带前导零的 s 会怎么处理?derSignatureToRaw 前面哪几行处理了 DER 的符号位填充?
3. 为什么 low-s 归一化放在客户端解析签名时做,而不是让合约接受 high-s?

**真懂 vs 背诵**:真懂的人能说出 0x100→configure→setupData→CREATE2 这条因果链,并主动把"7212 支持"限定为"代码假设,逐链待验证";背诵的人把 7212 当作已成事实的行业标准来讲,且说不出 verifiers 参数没有 fallback。

**评分规则**:3 分线:0x100 = RIP-7212 precompile 地址 + low-s 因 precompile 拒收 + verifier 进入地址推导;5 分线:再答出 uint176 无 fallback、全链必须同值才保地址一致、对外表述的准确边界。

### D3-identity-chain-Q6 · 难度 5/5

**考察目标**:修改影响分析:改动 verifier 配置(或任何 setup 参数)的爆炸半径——老用户地址漂移、双实现同步疑点、验证手段;综合考察 D3 全链条的因果理解。

**题干**:假设决定给 WebAuthn 签名器加一个 Solidity fallback verifier(即改 safe-address.ts 里 verifiers 的值,从裸 0x100 变成打包了 fallback 地址的 uint176)。修改影响题:(a) 直接改哪一行?(b) 这个改动会波及哪些行为——特别是老用户,分"已部署"和"未部署 counterfactual"两类说;(c) 文件头那句 "TypeScript port of SafeAddressComputer.swift" 意味着什么额外工作?这个 Swift 文件在仓库里吗?(d) 你用什么手段验证改动没把地址推导改坏?

**标准答案要点**:
1. (a) 直接改点是 safe-address.ts encodeSetupData 里的 verifiers = abiEncodeUint256Hex('100') 一行;但这一行的值流入 setupData → keccak256(setupData) → CREATE2 salt → computeAddress,等于改了地址推导函数本身。
2. (b) 已部署的老 Safe:verifier 配置已写在链上存储,客户端常量改动不影响它们的验签;但客户端 computeAddress 是恢复路径的地址来源——OnboardingScreen 恢复时用 computeAddress(record.publicKey) 重推导,新参数会推导出一个不同的地址,老用户恢复后看到的是空钱包,资金在旧地址上"消失"。
3. (b 续) 未部署的 counterfactual 账户同样漂移:同一公钥推导出新地址,首次部署会部署到新地址;所以这类改动必须版本化(老公钥用老参数推导,新钱包才用新参数),不能一刀切换常量。
4. (c) 文件头声明 TS 实现必须与 SafeAddressComputer.swift 产出完全一致;但仓库里没有这个 Swift 文件(现存 .swift 只有 VelaPasskeyModule/VelaCloudSyncModule/AppDelegate)——改之前必须先确认第二实现是否还存在于别处,注释可能已过时;若确有 Swift 双胞胎,必须同步修改否则 iOS 原生路径推导出不同地址。
5. (d) 验证手段:src/__tests__/services/safe-address.test.ts 已有 computeAddress(TEST_PUBLIC_KEY) 的黄金地址断言,参数一改这些测试必须按预期失败/更新;用 parallel space 的固定 keyset(passkey-fixture 的 attestation 专门构造成 extractPublicKey 可提取固定公钥)跑真实的创建+恢复往返;对至少一个已部署真实账户对照链上地址,确认老路径推导不变。
6. 隐含清单:凡是进 setupData 的常量(WEBAUTHN_SIGNER、SAFE_4337_MODULE、SAFE_MODULE_SETUP、MULTI_SEND、threshold 等)都有同样的爆炸半径;SAFE_SINGLETON/PROXY_CREATION_CODE/SAFE_PROXY_FACTORY 虽不在 setupData 里,也经 deploymentCode→initCodeHash/CREATE2 deployer 进入地址推导——这道题的结论对整组常量成立。

**代码证据**:
- `src/services/safe-address.ts:120` — verifiers = abiEncodeUint256Hex('100'),唯一直接改点
- `src/services/safe-address.ts:220-225` — computeAddress:parsePublicKey → calculateSaltNonce → encodeSetupData → calculateProxyAddress 全链条
- `src/services/safe-address.ts:202-210` — setupData 的 keccak 进 CREATE2 salt,证明 setup 参数都是地址输入
- `src/services/safe-address.ts:1-4` — 头注释:'TypeScript port of SafeAddressComputer.swift — must produce identical results'
- find 全仓库确认:不存在 SafeAddressComputer.swift(仅 VelaPasskeyModule.swift、VelaCloudSyncModule.swift、AppDelegate.swift)
- `src/screens/onboarding/OnboardingScreen.tsx:134` — 恢复路径用 computeAddress(record.publicKey) 重推导:推导规则一变,恢复地址即漂移
- `src/screens/onboarding/CreateWalletScreen.tsx:118` — 创建路径同一函数推导地址
- `src/__tests__/services/safe-address.test.ts:75-94` — computeAddress 黄金地址测试(TEST_PUBLIC_KEY 断言)
- `src/services/dev/passkey-fixture.ts:169,193` — 固定公钥的 fixture attestation,专为 extractPublicKey 可解析而构造(parallel space 验证工具)

**常见错误**:
- 认为"改一个常量、跑通编译就行"(没看到 setupData 在 CREATE2 salt 里,地址整体漂移)。
- 认为"地址只由公钥决定,verifier 变了地址不变"(salt 的两个输入:setupData 哈希和公钥派生的 saltNonce,前者被忽略)。
- 认为已部署老用户也会坏(方向搞反:链上存储不受客户端常量影响,坏的是恢复/counterfactual 推导)。
- 不加验证地复述"要同步改 Swift"(仓库里根本没有这个 Swift 文件——照抄注释而不查证,正是零代码引用习惯的反面教材)。
- 验证方案只说"多测测",指不出黄金地址测试和 parallel space 固定 keyset 这两个现成工具。

**追问**:
1. 如果必须上这个改动,你会怎么设计版本化?版本信息放哪——本地 StoredAccount、索引服务器的 metadata 字段,还是推导时双跑新老参数比对链上 code?
2. PublicKeyRecord 里的 metadata/initialCredentialId 字段现在有没有被用起来?它们能不能承载推导参数版本?
3. SAFE_PROXY_RUNTIME_CODE 是从 PROXY_CREATION_CODE 现场切出来的——这个改动会影响它吗?为什么?

**真懂 vs 背诵**:真懂的人先画出常量→setupData→salt→地址的因果链,再分"链上已固化 vs 客户端推导"两个世界讨论老用户,并对"Swift 双实现"先查证再下结论;背诵的人直接跳到"改常量+改 Swift+多测测",既不分用户群也不查 Swift 是否存在。

**评分规则**:3 分线:说出地址会漂移的因果链 + 老用户恢复被破坏 + 需要版本化;5 分线:再分清已部署/counterfactual 的不同命运、指出 Swift 文件不在仓库需先查证、给出黄金地址测试 + parallel space 固定 keyset 的具体验证方案。

---

## D4-tx-chain — 交易构造与上链全生命周期

### D4-tx-chain-Q1 · 难度 1/5

**考察目标**:纠正受训者基线诊断中的真实错误:澄清未部署 Safe 的部署机制是 initCode/factory,而不是 MultiSend 第一笔子调用;并能定位部署数据的构造代码。

**题干**:一个从未在某条链上发过交易的 Safe 账户,第一笔交易是如何顺带完成合约部署的?部署数据由哪个文件的哪个函数构造?最终提交给 bundler 时,这段数据以什么 JSON 字段出现?MultiSend 在部署过程中到底出现在哪一层?

**标准答案要点**:
1. 部署靠 UserOp 的 initCode,不是 MultiSend 的第一笔子调用:sendUserOp 在 deployed=false 时调用 buildInitCode(publicKeyHex) 填充 initCode,deployed=true 时 initCode 为空字节。
2. buildInitCode(src/services/safe-transaction.ts)构造 SAFE_PROXY_FACTORY 地址前缀 + createProxyWithNonce(SAFE_SINGLETON, setupData, saltNonce) 的 calldata。
3. setupData 由 safe-address.ts 的 encodeSetupData 构造:Safe.setup(owner=[WEBAUTHN_SIGNER], threshold=1),其内部通过 MultiSend delegatecall 执行 enableModules(Safe4337Module)+configure(P256 公钥)——MultiSend 只出现在 setup 初始化器内部,这正是"MultiSend 参与部署"错觉的来源,它不是 UserOp callData 里的子调用。
4. saltNonce = keccak256(abi.encode(x, y)),由 calculateSaltNonce 从 P-256 公钥坐标推导,保证同一 passkey 跨链同址(CREATE2 无 chainId 输入)。
5. v0.7 序列化时 initCode 被 userOpToDict 拆成 factory + factoryData 两个字段上送。
6. 未部署账户 nonce 固定 '0x0',verificationGasLimit 用 2M 下限(VERIFICATION_GAS_UNDEPLOYED),Tempo 上是 6M(TEMPO_VERIFICATION_GAS_UNDEPLOYED,因为 Safe 部署在 Tempo 实测约 3.9M gas)。

**代码证据**:
- `src/services/safe-transaction.ts:481-483` — sendUserOp 中 `deployed ? new Uint8Array(0) : buildInitCode(publicKeyHex)`,部署与否的唯一分叉点
- `src/services/safe-transaction.ts:856-884` — buildInitCode:createProxyWithNonce selector + singleton + saltNonce + setupData,末尾把 factory 地址前置拼接
- `src/services/safe-transaction.ts:1696-1699` — userOpToDict 把 initCode 拆成 factory(前 20 字节)+ factoryData(其余)
- `src/services/safe-address.ts:75-78` — calculateSaltNonce = keccak256(abi.encode(x, y))
- `src/services/safe-address.ts:106-188` — encodeSetupData:setup 参数编码,tx1/tx2 为对 SAFE_MODULE_SETUP 和 WEBAUTHN_SIGNER 的 MultiSend delegatecall(129-131 行)
- `src/services/safe-transaction.ts:49,492` — VERIFICATION_GAS_UNDEPLOYED=2M;未部署 nonce 固定 '0x0'
- `src/services/tempo.ts:87` — TEMPO_VERIFICATION_GAS_UNDEPLOYED=6M
- `src/services/safe-transaction.ts:1790-1791` — 部署失败(AA13/initCode failed)映射为 'Wallet deployment failed…' 文案

**常见错误**:
- 【受训者原错】认为未部署账户由 MultiSend 批量调用中的第一笔子调用完成部署——实际是 initCode/factory 机制;MultiSend 只出现在 setup 初始化数据内部,层级完全不同。
- 认为部署是先发一笔独立交易再发用户交易——实际同一个 UserOp 里 EntryPoint 先执行 initCode 再执行 callData,一次完成。
- 以为 v0.7 仍以单一 initCode 字段上送——实际已拆成 factory/factoryData。

**追问**:
1. setupData 或 SAFE_SINGLETON 任何一个字节变了,CREATE2 地址会怎样?为什么这条性质是"跨链同址"承诺的根基?
2. 部署失败时 bundler 会报哪个 AA 错误码?钱包把它映射成什么用户文案?
3. 为什么未部署时 verificationGasLimit 要 2M 下限,而 Tempo 上要提到 6M?

**真懂 vs 背诵**:真懂的人能画出层级:initCode(factory+factoryData)→ createProxyWithNonce → setupData → setup 内部才是 MultiSend delegatecall;背诵的人只会说"用 initCode 部署",解释不了 MultiSend 到底出现在哪一层、为什么容易被误认成子调用。

**评分规则**:3 分线=说出 initCode/buildInitCode 并明确否定"MultiSend 子调用部署";5 分线=还能讲清 setupData 内部的 MultiSend delegatecall 层级、v0.7 factory/factoryData 拆分和 saltNonce=keccak(x,y) 的跨链同址推导。

### D4-tx-chain-Q2 · 难度 2/5

**考察目标**:纠正受训者"UserOp 字段记不得、猜 to/data/value"的真实错误:掌握本项目 UserOperation 的 11 个字段、to/value/data 的真实去处、v0.7 序列化拆分,以及 SafeOp 签名覆盖范围。

**题干**:本项目的 UserOperation 接口有哪些字段?你之前猜的 to/value/data 到底在哪里?提交给 bundler 时哪些字段做了 v0.7 拆分?passkey 签名到底签的是什么内容——签完之后还能改 gas 吗?

**标准答案要点**:
1. 11 个字段:sender、nonce、initCode、callData、verificationGasLimit、callGasLimit、preVerificationGas、maxFeePerGas、maxPriorityFeePerGas、paymasterAndData、signature(interface UserOperation)。
2. to/value/data 不是 UserOp 顶层字段——它们被 buildExecuteCallData 编码进 callData:executeUserOp(address to, uint256 value, bytes data, uint8 operation),单笔时 operation=0(CALL)。
3. 批量时 callData 是 executeUserOp(MULTI_SEND, 0, multiSend(packed), 1) 即 DELEGATECALL 到 MultiSend;packed 内每个子调用的 operation byte 为 0(CALL)。
4. v0.7 序列化(userOpToDict):initCode → factory+factoryData;paymasterAndData → paymaster+paymasterData(外加两个 0x0 的 paymaster gas 字段);Tempo 的 feeToken 是 Vela 扩展字段经 extra 合并,bundler 读取后剥离。
5. passkey 签的是 SafeOp 的 EIP-712 hash:typeHash 覆盖 safe/nonce/initCode/callData/全部 gas 字段/maxFee/paymasterAndData/validAfter/validUntil(恒 0)/entryPoint;domain = chainId + SAFE_4337_MODULE(不是 Safe 本身)。
6. 因为 gas 字段在签名 hash 内,估气必须发生在签名之前;签完不能再改任何 gas 值,否则签名失效。

**代码证据**:
- `src/services/safe-transaction.ts:62-74` — UserOperation interface,11 个字段,没有 to/value/data
- `src/services/safe-transaction.ts:765-791` — buildExecuteCallData:executeUserOp(address,uint256,bytes,uint8) 编码,operation=0
- `src/services/safe-transaction.ts:808-841` — buildMultiSendExecuteCallData:833 行传 MULTI_SEND 地址,836 行 abiEncodeUint256(1n) 即 DELEGATECALL,812 行子调用 operation byte=0
- `src/services/safe-transaction.ts:1679-1714` — userOpToDict:1696-1699 factory 拆分,1702-1707 paymaster 拆分,1709-1711 Vela 扩展字段(feeToken)
- `src/services/safe-transaction.ts:890-937` — calculateSafeOpHash:896-900 行 SafeOp typeHash 全字段清单,915-916 行 validAfter/validUntil=0,925-931 行 domain=chainId+SAFE_4337_MODULE
- `src/services/safe-transaction.ts:534-579` — 流程顺序:先 estimateGas 更新 gas 字段(541-558),后算 SafeOp hash(579)再签名

**常见错误**:
- 【受训者原错】把 to/data/value 猜成 UserOp 顶层字段——那是 Safe 内层调用参数,藏在 callData 的 executeUserOp 编码里。
- 以为签名只覆盖 callData——实际全部 gas 字段都在 SafeOp hash 内,这决定了"先估气后签名"的流程顺序。
- 以为 EIP-712 domain 的 verifyingContract 是 Safe 地址——实际是 Safe4337Module(EIP-1271 消息签名那条路径才用 Safe 地址,见 computeSafeMessageHash)。

**追问**:
1. buildDummySignature 为什么必须和真签名同构等长?它伪造了哪些部分?
2. 同一次签名里,derSignatureToRaw 和 extractClientDataFields 分别解决 WebAuthn 输出的什么格式问题?
3. 如果 bundler 估出的 gas 比构造 dummy op 时的默认值高,代码流程如何保证签名仍然有效?

**真懂 vs 背诵**:真懂的人能立即回答"to/value/data 在 callData 的 executeUserOp 编码里",并自己推出"gas 在签名 hash 内 → 必须先估气后签名"的因果链;背诵的人只能干背字段清单,一追问签名范围就露馅。

**评分规则**:3 分线=报全 11 个字段且说清 to/value/data 在 callData 内;5 分线=还能讲清 v0.7 两处拆分、feeToken 扩展字段机制,以及 SafeOp hash 覆盖 gas 字段导致的流程顺序约束。

### D4-tx-chain-Q3 · 难度 3/5

**考察目标**:纠正受训者两个真实错误:不知道 bundler 除发交易外还是 gas 报价权威、不知道档位倍数。考察定价三级优先级、GAS_TIER_MULTIPLIERS、isQuoteAbusive 信任模型与拒发语义。

**题干**:sendUserOp 给 maxFeePerGas 定价有一个三级优先级,分别是什么?bundler 在定价体系里除了提交交易还扮演什么角色?四个 gas 档位的倍数各是多少?什么情况下钱包会直接拒绝 bundler 的报价——拒绝之后是回退本地价还是拒发?

**标准答案要点**:
1. 优先级 1:maxFeeOverride(确认屏已展示的 bundler 报价原样透传),并用 typeof maxFeeOverride === 'bigint' 做运行时类型校验——防止误接 onPress 的 gesture event 序列化成 '0x[object Object]' 炸掉估算和 SafeOp hash。
2. 优先级 2:getBundlerGasQuote → pimlico_getUserOperationGasPrice。bundler 是价格的唯一权威(single source of truth):既是提交/接受方,也是报价方,钱包展示它的报价、从不自行加价。
3. 优先级 3:本地兜底 calcMaxFeePerGas = gasPrice × 档位倍数 × BUNDLER_MARGIN,仅当 bundler 不支持报价方法(quote 返回 null)时使用。
4. 档位倍数(GAS_TIER_MULTIPLIERS):slow ×1.1、standard ×1.2、rapid ×1.5、fast ×2.0。
5. 本地兜底的 BUNDLER_MARGIN_PERCENT=100,即 2× 加成(BUNDLER_MARGIN_NUM 实为 200;行内 '// 150' 注释是过时的),必须与 bundler 侧 WALLET_GAS_MARGIN_PERCENT 保持一致,否则兜底 op 会被拒。
6. 拒绝条件(isQuoteAbusive):首选用 bundler 自报的 networkFeePerGas 判断——报价 > 3×(MAX_QUOTE_VS_CHAIN_MULTIPLE)其自报网络成本即抛 GasQuoteTooHighError;这是拒发(refusal),不是回退,防止恶意/错配 bundler 静默超收。
7. 只有当 bundler 没报 networkFee(通用 bundler)时才用钱包自测链价交叉验证,且仅在 tipMeasured 为真时——不可靠的本链 RPC 永远不能否决 bundler 报价(fail-open),这是 Gnosis 费率显示 '—' 加 'gas price too low' 慢性病的教训。

**代码证据**:
- `src/services/safe-transaction.ts:504-510` — 三级优先级本体:505 行 typeof bigint 校验,508 行 getBundlerGasQuote,509 行 calcMaxFeePerGas 兜底
- `src/services/safe-transaction.ts:231-236` — GAS_TIER_MULTIPLIERS:11/10、12/10、15/10、20/10
- `src/services/safe-transaction.ts:245-247` — BUNDLER_MARGIN_PERCENT=100 → NUM=BigInt(200),行内 '// 150' 注释过时(陷阱点)
- `src/services/safe-transaction.ts:1376-1382` — calcMaxFeePerGas = gasPrice × tier × margin
- `src/services/safe-transaction.ts:1390,1393-1402` — MAX_QUOTE_VS_CHAIN_MULTIPLE=3n 与 GasQuoteTooHighError
- `src/services/safe-transaction.ts:1433-1451` — isQuoteAbusive:1440-1442 主判据用 reportedNetworkFeePerGas×3,1445 行 !tipMeasured 时 fail-open 返回 false
- `src/services/safe-transaction.ts:1470-1518` — getBundlerGasQuote:null=回退本地,1509-1515 行超限抛错=拒发
- `src/services/safe-transaction.ts:1413-1415` — BUNDLER_QUOTE_TIP_PERCENT 按档位缩放 tip 才能同档位公平比较

**常见错误**:
- 【受训者原错】以为 bundler 只负责提交交易、价格由钱包自己从链上 RPC 算——实际 bundler 是报价权威,钱包 RPC 只做受限交叉验证且 fail-open。
- 【受训者原错】不知道档位倍数,瞎猜 1×/2×/3×——实际是 1.1/1.2/1.5/2.0。
- 以为报价超过 3× 上限会回退到本地价继续发——实际是抛 GasQuoteTooHighError 拒发。
- 把 3× 上限理解成"对钱包自测链价的比较"——主路径比较的是 bundler 自报的 networkFeePerGas(它自己的 markup)。

**追问**:
1. 为什么 Gnosis 上"钱包 RPC 否决 bundler 报价"会同时导致费率显示 "—" 和交易被拒?tipMeasured 字段解决了什么?
2. rawBundlerGasCost 为什么要把档位倍数从 totalWei 里除回去?不除会有什么用户可见的后果?
3. BUNDLER_MARGIN_NUM 旁边那个 '// 150' 注释和实际值 200 哪个对?这类陷阱怎么系统性防?(答:代码值 200 为准,注释过时;系统性防法=常量与注释同处修改、回归测试锁值。)

**真懂 vs 背诵**:真懂的人能讲出信任模型的"不对称":bundler 自报 markup 是主判据,钱包本链 RPC 只在 tipMeasured 时兜底且失败即放行,并说出 Gnosis 事故背景;背诵的人只会背 3× 和四个倍数,分不清"拒发"和"回退"。

**评分规则**:3 分线=说出三级优先级 + bundler 是报价权威 + 四个档位倍数;5 分线=还能讲清 isQuoteAbusive 的双判据顺序、tipMeasured fail-open 的设计原因和"拒发≠回退"的语义。

### D4-tx-chain-Q4 · 难度 4/5

**考察目标**:故障推演能力:RPC 间歇故障时 sendUserOp 各环节的降级方向(fail-safe/fail-fast/静态兜底/拒绝)各不相同,重点考察是否会浪费 passkey 弹窗,以及 Tempo 路径的 nonce 降级不对称。

**题干**:故障推演:某条链的 RPC 节点开始间歇性故障。分三种情况推演 sendUserOp 的行为:(a) eth_getCode 全部报错;(b) EntryPoint.getNonce 拉取失败;(c) eth_estimateUserOperationGas 失败。每种情况下,已部署/未部署账户、普通链/Tempo 链的行为有何差别?哪些情况会白白烧掉用户一次 passkey 弹窗?

**标准答案要点**:
1. (a) isDeployed 在 RPC error 和抛异常时都返回 true——fail-safe 方向是"当作已部署":已部署账户不受影响;真未部署账户会被误判 → initCode 为空 → bundler 验证阶段拒绝,不会误发也不会双部署。
2. (b) 普通链:并行取数时 getNonce().catch(() => null);若 deployed 且 nonce 为 null,在签名之前直接抛 'Could not fetch the account nonce…'——fail-fast,明确为了不烧 passkey 弹窗(否则提交 0x0 必被 AA25 拒);未部署账户 nonce 本来就是 0,用 '0x0' 正常继续。
3. (b-Tempo)sendUserOpTempo 的 catch 返回 '0x0' 而不是 null,没有 fail-fast 分支——已部署 Tempo 账户在 nonce 拉取失败时会带着 0x0 完成签名并提交,烧掉一次 passkey 弹窗后被 AA25 拒绝(映射为 'Transaction nonce mismatch')。这是与普通链不对称的已知薄弱点。
4. (c) 估算失败:callData ≤ 1024 字节(ESTIMATION_REQUIRED_CALLDATA)→ 静态默认 gas 继续(简单转账足够准);> 1024 字节 → 直接抛 'Could not estimate gas…' 拒绝——否则会提交一个 bundler 接受但落不了地的 op,用户静默等 2 分钟超时。
5. 补充:needsEstimation = !deployed || callData.length > 200,已部署 + 小 calldata 的简单转账根本不走估算,天然免疫 (c)。
6. 最前置的闸门 verifyChainReady 检查 EntryPoint 有无代码,首次成功后按链缓存;EntryPoint 缺失则第 0 步就抛 'network not ready'。

**代码证据**:
- `src/services/safe-transaction.ts:1189-1211` — isDeployed:1198-1200 行 RPC error 返回 true,1207-1210 行异常也返回 true(fail-safe 朝已部署)
- `src/services/safe-transaction.ts:473-478` — 清 gas 缓存后 Promise.all 并行取 deployed/nonce/gasPrices,nonce 带 .catch(() => null)
- `src/services/safe-transaction.ts:485-492` — deployed && nonceResult===null → 签名前 fail-fast 抛错,485-488 行注释明说避免烧 passkey 弹窗于必被 AA25 拒的 op
- `src/services/safe-transaction.ts:666-673` — Tempo 路径:668 行 catch 返回 '0x0',673 行直接采用——已部署 + 拉取失败会签名后才死于 AA25
- `src/services/safe-transaction.ts:56,538,559-569` — ESTIMATION_REQUIRED_CALLDATA=1024;needsEstimation 阈值 200;565-567 行大 calldata 估算失败即拒绝
- `src/services/safe-transaction.ts:1758-1773` — verifyChainReady:检查 EntryPoint 代码,按链缓存
- `src/services/safe-transaction.ts:1794-1795` — AA25/invalid account nonce 映射为 'Transaction nonce mismatch. Please try again.'

**常见错误**:
- 以为 isDeployed 失败会当作"未部署"从而带上 initCode 重复部署——实际 fail-safe 方向相反,返回 true。
- 以为所有 RPC 失败都统一"报错重试"——实际四个环节各有不同降级策略:fail-safe(getCode)、fail-fast(nonce+deployed)、静态兜底(小 calldata 估算)、拒绝(大 calldata 估算)。
- 没意识到 Tempo 路径的 nonce 降级(catch→'0x0')与普通链(catch→null→抛错)不一致,会多烧一次 passkey 弹窗。
- 以为大 calldata 估算失败也会用默认 gas 硬发——实际明确拒绝,因为静态默认盖不住真实 callGasLimit/preVerificationGas。

**追问**:
1. 为什么"deployed + nonce 失败"必须在签名之前抛?如果放到提交后才失败,用户体验和安全上各损失什么?
2. 200 和 1024 两个字节阈值分别在保护什么?为什么不是一个阈值?
3. 如果要修 Tempo 的 nonce 不对称,你会改哪几行、如何验证不破坏未部署账户的首发路径?

**真懂 vs 背诵**:真懂的人能按调用时序逐点说出每个失败点的降级方向和"为什么朝这个方向",并主动指出 Tempo 不对称;背诵的人只会笼统说"失败就报错/重试",说不出 fail-safe 与 fail-fast 的取向差异。

**评分规则**:3 分线=答对 (a) 返回 true、(b) 普通链签名前抛错、(c) 大小 calldata 分流三个主干;5 分线=额外指出 Tempo catch '0x0' 的不对称及其烧 passkey 的后果,并能解释每个降级方向背后的取舍。

### D4-tx-chain-Q5 · 难度 4/5

**考察目标**:考察提交与回执阶段的韧性链路:3 次重试的适用范围、existingHash 恢复路径、乐观 nonce 自增的精确时机、waitForReceipt 退避与两种超时文案的语义区分。

**题干**:用户点确认之后,依次遇到三种情况:(1) bundler 返回 'currently processing' 错误;(2) bundler 返回带 '[existingHash:0x…]' 标记的错误;(3) 提交成功但 120 秒内没有回执。钱包各做什么?乐观 nonce 缓存在哪种情况下会自增、哪种不会?重试的时候能不能顺手把 gas 价提高一点?

**标准答案要点**:
1. (1) submitUserOp 只对 'currently processing' / 'Retry later' 两类瞬时错误重试,MAX_RETRIES=3、每次固定间隔 3 秒;重发的是同一个已签名 dict——SafeOp 签名覆盖全部 gas 字段,重试不可能换价,换价必须重签(再弹一次 passkey)。
2. (2) sendUserOp 捕获提交错误后用 parseExistingUserOpHash 提取 '[existingHash:0x…]' 中的在途 hash,直接转为轮询那个旧 op 的回执(60 秒超时),对调用方视同提交成功而非失败;普通链和 Tempo 路径共用这个解析器。
3. (3) waitForReceipt 默认 120 秒,自适应退避 1s 起每轮 +500ms、3s 封顶;轮询中的 RPC error 不放弃(op 可能仍会落地)。
4. 超时结案分两种文案:从未收到干净响应(sawCleanResponse=false 且 rpcFailures>0)→ 'status is unknown, check the explorer'(命运未知,先查再重试);bundler 应答过但没落地 → 'submitted but not confirmed, may still land'(不能暗示失败诱导用户重发)。
5. 回执 result.success===false → 抛 'dropped from the network',这是终态,轮询 catch 里专门识别并直接上抛。
6. 乐观 nonce(incrementNonceCache)只在 submitUserOp 正常返回新 hash 后执行(防并发发送撞 nonce);existingHash 恢复路径在 catch 分支内直接 return,不自增。

**代码证据**:
- `src/services/safe-transaction.ts:1569-1594` — MAX_RETRIES=3、RETRY_DELAY=3000,1584 行仅 'currently processing'/'Retry later' 可重试,重发同一 dict
- `src/services/safe-transaction.ts:604-620` — 提交失败 catch → parseExistingUserOpHash → 返回 existingHash 并以 60_000ms 超时轮询,不算失败
- `src/services/safe-transaction.ts:1725-1730` — parseExistingUserOpHash 的 /\[existingHash:(0x…)\]/ 正则,注释说明普通链与 Tempo 共用
- `src/services/safe-transaction.ts:622-623,1247-1254` — incrementNonceCache 位于 try/catch 之后,仅成功提交路径可达;实现为缓存 nonce+1
- `src/services/safe-transaction.ts:1596-1669` — waitForReceipt:1599 行默认 120s,1648-1650 行 1s→3s 退避,1633-1636 行 success===false 抛 dropped,1653-1668 行两种超时文案分流
- `src/services/safe-transaction.ts:890-919` — SafeOp hash 包含 maxFeePerGas 等全部 gas 字段——"重试不能换价"的密码学依据

**常见错误**:
- 以为重试会自动提高 gas 价(类似 EOA 的 replacement fee bump)——签名覆盖 gas 字段,重试只能原样重发,换价=重签=再弹 passkey。
- 以为 existingHash 是失败,应该报错给用户——实际是恢复路径,转为轮询在途 op,用户无感。
- 以为 120 秒超时=交易失败,可以直接让用户重发——两种超时文案都明确 op 可能仍落地,贸然重发才是事故源。
- 以为任何提交尝试后乐观 nonce 都会自增——existingHash 路径不自增。

**追问**:
1. 为什么 existingHash 路径的轮询超时是 60 秒而主路径是 120 秒?
2. 'status unknown' 和 'not confirmed' 两个文案分别引导用户做什么?混成一个文案会引发什么用户行为?
3. waitForReceipt 的 AbortSignal 参数是给什么场景用的?中断后 op 本身会怎样?

**真懂 vs 背诵**:真懂的人能从"gas 字段在签名内"推出"重试不能换价"并精确指出 nonce 自增只在拿到新 hash 后;背诵的人把三种情况混成"重试三次然后报错",分不清两种超时文案的语义差别。

**评分规则**:3 分线=说清 3 次重试的适用错误、existingHash 转轮询、120s 超时不等于失败;5 分线=额外答出"重试不能换价"的签名学依据、nonce 自增的精确位置和两种超时文案的设计意图。

### D4-tx-chain-Q6 · 难度 5/5

**考察目标**:修改影响分析:换 Tempo 默认 fee token 牵动的全部位置(含跨仓库 vela-bundler)、会破的隐藏假设(小数、×10^12 缩放、per-subcall gas 标定、bundler float),以及验证方案。同时深考 Tempo 稳定币 gas 报销机制本身。

**题干**:修改影响题:假设 Tempo 官方新发了一种 TIP-20 稳定币(注意:它是 4 位小数),要求 Vela 把默认 gas fee token 从 pathUSD 换成它。你需要动哪些位置(包括本仓库之外)?哪些看不见的假设会被打破?你打算怎么验证这次修改?

**标准答案要点**:
1. 常量本体:tempo.ts 的 TEMPO_DEFAULT_FEE_TOKEN;safe-transaction.ts 四个发送入口(sendNative/sendERC20/sendContractCall/sendBatchCalls)都是引用该常量传入 sendUserOpTempo,改常量即全覆盖。
2. 小数假设会破:TEMPO_FEE_TOKEN_DECIMALS=6 是"每个 TIP-20 USD 都 6 位小数"的全局假设;4 位小数意味着 tempoReimbursement/attoToTokenUnits/estimateTempoFee 的换算,以及 bundler-service.ts 里 gas 账户余额的 ×10^12(6→18 位)缩放全部要改,否则报销金额和余额显示错 100 倍。
3. gas 账户显示:bundler-service.ts 的 fetchBundlerAccountInfo 在 Tempo 分支用 TEMPO_DEFAULT_FEE_TOKEN 做 balanceOf 且把 nativeSym 硬编码为 'pathUSD',token 地址和符号都要换。
4. token picker 归类:isTempoFeeToken 决定哪个 token 被归入 'Gas' 类,跟着常量走但要确认 UI 文案。
5. 跨仓库:feeToken 作为 Vela 扩展字段随 eth_sendUserOperation 上送(submitUserOp 第三参 → userOpToDict extra),vela-bundler 端用它给外层 0x76 付 gas 并校验批内报销转账覆盖成本——bundler 必须认可新 token 且持有其浮动资金,sponsorship 也是给 gas 账户打 fee-token float;bundler 不认就会以 reimbursed=0 语义拒单,钱包侧改动全白费。
6. gas 标定假设可能破:TEMPO_CALL_GAS_PER_SUBCALL=380k 与 TEMPO_PER_SUBCALL_GAS_EST=95k 是按 0x20c0 TIP-20 transfer(实测 ~308k)标定的;新 token 的 transfer 计量不同就要重新实测,否则要么批量 OOG 回滚、要么报销定价失真。
7. 机制不变量要守住:Tempo UserOp 恒以 maxFeePerGas=0 签名(避免 AA21),用户成本全在批尾的 feeToken.transfer(bundlerEOA, reimbursement),定价用 tempoExpectedGas(现实 gas)而非 padded limits——这些不随 token 更换而变。
8. 验证:在 Moderato 测试网(42431,在 TEMPO_CHAIN_IDS 内)/并行空间测试环境走完整发送:核对确认屏报价 == 链上实扣报销(estimateTempoFee 与 sendUserOpTempo 共用 tempoExpectedGas 公式,天然可对账);验证资助弹窗余额与符号;验证未部署账户首发(6M verificationGas、部署实测 ~4.1M)不回归;验证 bundler 侧确实以新 token 收到报销。

**代码证据**:
- `src/services/tempo.ts:55,57-58` — TEMPO_DEFAULT_FEE_TOKEN 常量与 TEMPO_FEE_TOKEN_DECIMALS=6("每个 TIP-20 USD 都 6 位小数"的假设注释)
- `src/services/safe-transaction.ts:116,145,165,192` — 四个发送入口全部传 TEMPO_DEFAULT_FEE_TOKEN 进 sendUserOpTempo
- `src/services/safe-transaction.ts:677-681,717-720,740` — 批尾追加 feeToken.transfer(feeCollector, reimbursement);报销按 tempoExpectedGas 定价;740 行 submitUserOp(userOp, chainId, { feeToken }) 上送扩展字段
- `src/services/safe-transaction.ts:697-698,1709-1711` — maxFeePerGas=0 避免 AA21;extra 字段合并进 dict,bundler 读取后剥离
- `src/services/bundler-service.ts:264-277` — Tempo gas 账户余额:对 TEMPO_DEFAULT_FEE_TOKEN 做 balanceOf,274 行 ×10^12 缩放,276 行 nativeSym 硬编码 'pathUSD'
- `src/services/bundler-service.ts:150-153,243-247` — sponsorship 在 Tempo 是打 pathUSD float;从错误的 bundler 读 depositAddress 会导致提交方以 reimbursed=0 拒单
- `src/services/tempo.ts:64-80,96-113,148-157` — TEMPO_OUTER_OVERHEAD_GAS/TEMPO_CALL_GAS_PER_SUBCALL=380k 标定、2× 报销 margin、tempoExpectedGas 与 tempoReimbursement 公式
- `src/services/tempo.ts:30,87` — TEMPO_CHAIN_IDS 含 Moderato 42431(验证环境);TEMPO_VERIFICATION_GAS_UNDEPLOYED=6M(须 ≤ bundler 8M cap)

**常见错误**:
- 只改 tempo.ts 一个常量就收工——漏掉小数换算、bundler-service ×10^12 缩放、nativeSym 硬编码、380k/95k gas 标定四处隐藏假设。
- 忘了这是跨仓库变更:vela-bundler 不认新 feeToken、不持有其 float,钱包侧改动全部白费。
- 忘了 sponsorship 和 gas 账户余额显示也以 fee token 计价,只测发送路径不测资助路径。
- 以为 Tempo 的 UserOp maxFeePerGas 要按新 token 重新定价——它恒为 0,全部定价都在批尾报销转账里。

**追问**:
1. 如果要新旧 token 共存(按用户余额自动选),feeToken 参数目前已贯穿哪些函数签名?还缺哪一环?
2. 为什么报销用 tempoExpectedGas(实测现实 gas)而不是 padded 的 UserOp limits 定价?用错的话用户大约多付几倍?
3. 验证阶段你如何确认 bundler 真的以新 token 收到了报销?从哪个地址、看哪笔转账?

**真懂 vs 背诵**:真懂的人会先列"隐藏假设清单"(6 位小数、×10^12、380k 标定、bundler float、跨仓库校验)再谈改哪行代码,并给出报价==实扣的对账式验证;背诵的人改一个常量就宣布完成,直到报销金额错 100 倍才发现。

**评分规则**:3 分线=找到常量本体 + 小数换算 + bundler-service 余额显示三处并知道要在测试网验证;5 分线=完整覆盖跨仓库 bundler 认可/float、gas 标定重测、sponsorship 路径,并说出"maxFee 恒 0、定价全在报销"的机制不变量。

---

## D5-D10-signing-security — 签名面板与纵深防御

### D5-D10-signing-security-Q1 · 难度 1/5

**考察目标**:考察 dApp 请求进入钱包后的分层路由:签名方法、即时本地只读方法、网络只读方法各走哪条路径,以及为什么读请求洪水堵不死签名确认(代码定位题)。

**题干**:一个已连接的 dApp 同时发来三个请求:eth_chainId、eth_getBalance、eth_sendTransaction。它们分别在哪个文件被分流、各走哪条处理路径?为什么恶意 dApp 用几千个 eth_getBalance 刷屏也不会延迟用户的签名确认弹窗?

**标准答案要点**:
1. 统一入口在 src/models/dapp-connection.tsx:isSigningMethod(method) 为真(eth_sendTransaction/wallet_sendCalls/personal_sign/eth_sign/signTypedData)时直接 setIncomingRequest 弹出签名 sheet,完全不经过读请求的任何 gate。
2. eth_chainId 属于 use-dapp-signing.ts 的 INSTANT_READONLY_METHODS 白名单(本地状态即时应答,如 eth_accounts/eth_chainId/net_version),绕过并发门直通 handleReadOnlyRPC。
3. eth_getBalance 走 gateReadOnly(readOnlyKey(...), () => handleReadOnlyRPC(...)):readonly-rpc-gate.ts 里 MAX_CONCURRENT_READS=6 并发上限 + MAX_QUEUED_READS=512 排队上限,溢出时以 code -32005 可重试错误拒绝。
4. 相同 in-flight 读请求按 key(chainId|account|method|params)去重共享一次底层调用;只合并并发重复,从不跨时间缓存结果(不会返回旧数据)。
5. 签名请求不进 gate 是设计不变量(readonly-rpc-gate.ts 文件头注释明说),所以读洪水最多耗尽读并发槽位,签名确认永远不排队。

**代码证据**:
- `src/models/dapp-connection.tsx:252` — isSigningMethod(method) 分支,签名方法直接进弹窗流程(270 行 setIncomingRequest)
- `src/models/dapp-connection.tsx:301-303` — INSTANT_READONLY_METHODS 直通 vs gateReadOnly 包裹 handleReadOnlyRPC 的分流点
- `src/hooks/use-dapp-signing.ts:440-446` — isSigningMethod 定义
- `src/hooks/use-dapp-signing.ts:453-461` — INSTANT_READONLY_METHODS 白名单定义
- `src/services/readonly-rpc-gate.ts:19` — MAX_CONCURRENT_READS = 6
- `src/services/readonly-rpc-gate.ts:26-29` — MAX_QUEUED_READS = 512 与 RATE_LIMITED_CODE = -32005
- `src/services/readonly-rpc-gate.ts:71-87` — gateReadOnly 的去重 + 并发实现
- `src/services/readonly-rpc-gate.ts:1-17` — 文件头注释:签名请求不经过此 gate、去重从不跨时间缓存

**常见错误**:
- 以为所有 dApp 请求都排同一个队,读多了签名就会变慢(实际签名根本不进 gate)。
- 以为 gate 有结果缓存会返回旧余额(实际只合并并发重复,结果从不跨时间缓存)。
- 以为 eth_chainId 也要走网络 RPC(实际是本地即时应答白名单)。

**追问**:
1. handleReadOnlyRPC 对 eth_getCode 查询钱包自己地址时有什么特殊处理?为什么未部署时要返回 SAFE_PROXY_RUNTIME_CODE?(参考 use-dapp-signing.ts:492-507)
2. 为什么排队溢出要用 -32005 可重试错误拒绝,而不是让队列无限增长?
3. wallet_getCallsStatus 为什么要用 batchChainIds Map 记住批次提交时的链,而不是查当前链?

**真懂 vs 背诵**:真懂的人能画出 dapp-connection.tsx 里三条互不干扰的分流路径并说出"签名不进 gate"这个不变量;背诵的人只会说"有个限流器"而说不清签名请求和读请求的隔离关系。

**评分规则**:3 分线 = 说出签名与只读两条路径分离且签名不被读流量阻塞;5 分线 = 准确说出三条路径(签名直通/INSTANT 白名单/gateReadOnly)、6 并发 512 队列 -32005 常量,以及去重只合并并发不缓存的语义。

### D5-D10-signing-security-Q2 · 难度 2/5

**考察目标**:考察 approval-guard 三件套(detectApproval/rewriteApprovalParams/enforceNoUnlimited)与 ERC-7730 clear signing 是两套独立机制,以及 unlimited 授权最终被改写成什么(直接针对受训者"以为改写成 0"和"把授权检测归给 clear signing"两个真实错误)。

**题干**:dApp 请求一笔 unlimited USDC approve。从弹窗渲染到最终上链,钱包对这笔授权做了哪几步处理?授权检测是 ERC-7730 clear signing 的一部分吗?最终上链的授权金额是 0、原值、还是别的什么?由谁决定?

**标准答案要点**:
1. 授权检测走 src/services/approval-guard.ts 的 detectApproval,直接从原始 calldata/typed-data 识别 8 种授权形态,文件头注释明确 INDEPENDENT of ERC-7730 descriptors——因为 descriptor 查询恰恰在陌生/恶意合约上会失败,防线不能依赖它。
2. clear signing(src/services/clear-signing.ts)是另一套只负责"展示"的机制:descriptor 的 threshold 命中时也只是把字段渲染成 'Unlimited' + warning(进而 risk=danger),它不改写、不拦截请求。
3. SigningSheet 里 approval = useMemo(() => detectApproval(...)) 与 descriptor 解析完全并行;approval.editable 时渲染 ApprovalView 让用户自己选一个有限额度或 revoke——不是自动改成 0。
4. 确认时 confirm() 调 rewriteApprovalParams 把 calldata 中的金额 word 重编码为用户选的值;chosenAmount 对 ≥ cap 的选择直接 throw('Unlimited approvals are disabled');assertOnlyWordChanged 保证除金额那 32 字节外一个字节都没变。
5. 用户没做选择前无法确认:confirmDisabled 包含 (!!approval?.editable && !approveChoice)。
6. 结论:最终上链金额 = 用户选择的有限金额(user-chosen finite);只有用户主动选 revoke 才是 0。改写后的 paramsOverride 经 approveRequest 替换原始 params 签名提交('never the original unbounded request')。

**代码证据**:
- `src/services/approval-guard.ts:1-23` — 文件头:三件套职责 + 明确独立于 ERC-7730 descriptor 的设计理由
- `src/services/approval-guard.ts:116-137` — detectApproval 直接从 (method, params) 原始数据检测
- `src/services/approval-guard.ts:310-323` — chosenAmount:revoke→0、grant→1(仅布尔)、amount 必须有限否则 throw
- `src/services/approval-guard.ts:433-441` — assertOnlyWordChanged round-trip 安全断言
- `src/components/SigningRequestModal.tsx:176-179` — approval 检测独立于 descriptor 的 useMemo
- `src/components/SigningRequestModal.tsx:369-381` — editable 授权渲染 ApprovalView 花费上限编辑器
- `src/components/SigningRequestModal.tsx:481-484` — confirm() 里 rewriteApprovalParams 生成 paramsOverride
- `src/components/SigningRequestModal.tsx:516-519` — confirmDisabled:未选择额度不能确认
- `src/models/dapp-connection.tsx:492-494` — approveRequest 用 paramsOverride 替换原始 params('never the original unbounded request')
- `src/services/clear-signing.ts:957-967` — clear signing 的 threshold→'Unlimited'+warning 只是展示层
- `src/services/clear-signing.ts:1242-1243` — warning 字段驱动 risk='danger'(展示,不拦截)

**常见错误**:
- 【受训者原错】以为 unlimited 授权会被自动改写成 0(实际改写目标是用户在 UI 选择的有限金额,revoke 才是 0)。
- 【受训者原错】把授权检测归给 clear signing / ERC-7730(实际是 approval-guard 独立机制,descriptor 缺失时照样工作)。
- 以为 descriptor 查不到就无法识别授权(恰恰相反,防线特意不依赖 descriptor)。

**追问**:
1. ERC-721 的 approve(operator, tokenId) 和 ERC-20 approve 共用 selector 0x095ea7b3,为什么按 uint256 cap 处理仍然安全?(参考 approval-guard.ts:148-153 注释)
2. setApprovalForAll 没有金额可 cap,UI 上靠什么门控 grant?(答:显式 grant 选择 + requiresHold 滑动确认。)
3. rewriteApprovalParams 对 typed data 为什么要深拷贝且金额存为十进制字符串?

**真懂 vs 背诵**:真懂的人能说清"检测/改写/兜底"三步各在哪一层、改写目标由用户选择决定、clear signing 只管展示;背诵的人会把两套机制混成一个"防钓鱼功能"并臆断改写成 0。

**评分规则**:3 分线 = 说出授权检测独立于 clear signing 且改写为用户选的有限值而非 0;5 分线 = 完整讲出三件套分工、confirmDisabled 强制选择、paramsOverride 替换原始请求、clear signing threshold 仅是展示层这四件事。

### D5-D10-signing-security-Q3 · 难度 3/5

**考察目标**:考察上限常量 UNLIMITED_CAP_256 = 1n<<200n / UNLIMITED_CAP_160 = 1n<<152n 的取值原理,以及 off-chain permit 签名不可改写的边界:为什么 enforceNoUnlimited 对 typed-path 主动放行,放行后靠什么兜底。

**题干**:UNLIMITED_CAP_256 和 UNLIMITED_CAP_160 定义在哪、分别是多少?为什么选 2^200 和 2^152 这两个值?为什么 ERC-2612/Permit2 的 typed-data permit 被标记 editable: false,而且 enforceNoUnlimited 遇到它们直接 return 放行?放行之后这类请求靠什么防线兜底?

**标准答案要点**:
1. src/services/approval-guard.ts:32/34:UNLIMITED_CAP_256 = 1n << 200n(uint256 金额字段),UNLIMITED_CAP_160 = 1n << 152n(Permit2 的 uint160 金额字段)。
2. 取值逻辑:cap 必须落在"最大合理真实授权"(total_supply × 10^decimals ≈ 2^128)之上、"unlimited 哨兵"(uint256-max、2^255、uint160-max)之下,干净分隔"用户选的大有限数"与"无限";uint160 字段哨兵是 2^160-1,所以要低一档的 2^152。capForBits/isUnboundedAmount 按 amountBits 选 cap。
3. off-chain permit(ERC-2612 Permit / Permit2 PermitSingle / PermitBatch / DAI permit)由 dApp 拿签名后提交它自己的 struct 上链——钱包只出签名,控制不了 dApp 提交的字节。改写签名里的金额只会让签名与链上 struct 失配,dApp 的兑现交易 revert(注释里的 "signed the Permit2, but Uniswap's swap fails" 经典 bug),所以 editable:false + blockReason=PERMIT_SIG_BLOCK。
4. enforceNoUnlimited 只管辖钱包自己提交的交易:detected.locus.type === 'typed-path' 时提前 return(approval-guard.ts:385)——这是职责边界,不是漏洞。
5. 放行后的兜底防线在 UI 层:PermitSignView 呈现真实风险且不给 cap 编辑器(SigningRequestModal.tsx:364-366);permitGrantsBroad(typed-path 且 isUnbounded 且非 reducing)触发 requiresHold,把一键确认换成 SlideToConfirmButton 滑动确认(462-475、624)。
6. 对照:Permit2 的链上 approve(address,address,uint160,uint48)(selector 0x87517c45)是钱包自己提交的 calldata,token 在第一个参数、金额是 word 2 的 uint160——仍然 editable、仍受 cap 管辖(approval-guard.ts:181-193)。

**代码证据**:
- `src/services/approval-guard.ts:32-34` — 两个 cap 常量定义
- `src/services/approval-guard.ts:18-23` — cap 取值原理注释(≈2^128 合理上界 vs unlimited 哨兵)
- `src/services/approval-guard.ts:38-45` — capForBits / isUnboundedAmount 按位宽选 cap
- `src/services/approval-guard.ts:216-224` — off-chain permit 不可改写的设计注释 + PERMIT_SIG_BLOCK 文案
- `src/services/approval-guard.ts:239-272` — erc2612-permit/permit2-single/permit2-batch 全部 editable:false
- `src/services/approval-guard.ts:381-385` — enforceNoUnlimited 对 typed-path 的提前 return 及注释
- `src/services/approval-guard.ts:181-193` — 链上 Permit2 approve:uint160、word 2、editable:true
- `src/components/SigningRequestModal.tsx:362-366` — typed-path 授权渲染 PermitSignView(不给 cap 编辑器)
- `src/components/SigningRequestModal.tsx:462-475` — permitGrantsBroad → requiresHold 滑动确认门控
- `src/components/SigningRequestModal.tsx:619-631` — requiresHold 时渲染 SlideToConfirmButton

**常见错误**:
- 以为 cap 就是 uint256-max / uint160-max 本身(实际特意选在哨兵之下、真实金额之上的分隔点)。
- 以为 typed-data permit 也会被改写 cap(实际钱包改不了 dApp 上链的 struct,强改只会造成签名失配 revert)。
- 把 typed-path 放行当成 enforceNoUnlimited 的漏洞(实际是职责边界,兜底移交给了滑动确认 + 风险呈现)。
- 分不清链上 Permit2 approve(可改写)与 off-chain PermitSingle 签名(不可改写)。

**追问**:
1. DAI permit 的 allowed 是布尔全额授权,它的 blockReason 和 ERC-2612 有何不同?为什么它也走 hold 门控?
2. 如果未来要支持一个金额字段是 uint96 的新协议,要在 approval-guard 里加什么?cap 选多少合适?
3. PermitBatch 里只要任意一个 detail.amount ≥ 2^152 就整体 isUnbounded,为什么不逐条展示编辑?

**真懂 vs 背诵**:真懂的人能推导 cap 为什么卡在 2^128 与哨兵之间、并用"钱包控制不了 dApp 提交的字节"解释 typed-path 放行;背诵的人只会复述两个数字,把放行说成"漏了一块"或以为 permit 也能 cap。

**评分规则**:3 分线 = 说出两个常量位置数值 + off-chain permit 因签名失配不可改写;5 分线 = 完整讲出 cap 取值的上下界推理、typed-path 在 enforceNoUnlimited:385 的提前 return 是职责边界、兜底是 PermitSignView+滑动确认,并能区分链上/链下两种 Permit2。

### D5-D10-signing-security-Q4 · 难度 4/5

**考察目标**:考察交易模拟防线的存在与降级语义(受训者此前完全不知道这条防线):eth_simulateV1 引擎级联、null(不知道)vs ok:false(真 revert)vs changes:[](跑了没动)三态区分,以及日志可伪造带来的非对称信任模型(故障推演题)。

**题干**:用户确认一笔 dApp swap 前,签名 sheet 里有一块"余额变动预览"。这块数据从哪来?故障推演:(a) 用户所有 RPC 节点都不支持 eth_simulateV1 时预览显示什么?(b) RPC 全部断网时呢?(c) 为什么 sim-engine-rpc 对顶层 error 返回 null 而不是 { ok: false }——如果写反了,用户会看到什么灾难性误报?

**标准答案要点**:
1. SigningSheet 对 eth_sendTransaction 和 wallet_sendCalls 都会调 simulateAssetChanges(单笔 252-258 行,批次全部 legs 307-312 行),结果渲染进 BalanceChangePreview(556 行,与 Send 确认步共用同一组件)。
2. 引擎级联(tx-simulation.ts:169-185):① rpcSimulate(eth_simulateV1,traceTransfers:true 把原生转账合成 Transfer 日志、validation:false 不要求 Safe 持币)→ ② tevmSimulate(默认关闭的本地 fork seam)→ ③ 降级到 simulateCall(单次 eth_call 纯 revert 预检)→ ④ 连预检都不可达才返回 null。
3. 三态语义:null = "不知道"(方法不支持/断网),绝不能当失败;{ok:false, revertReason} = 真实执行 revert;changes:[] = "跑了但什么都没动"(如 approve),与 changes:null(没引擎算不出资产变动)是不同信息。
4. (a) 不支持 eth_simulateV1:rpcSimulate 对顶层 error/非数组结果返回 null,降级到 simulateCall,用户仍能看到 revert 预检(engine:'none', changes:null),只是没有资产变动明细;(b) 全断网:simulateCall 也 null,simulateAssetChanges 整体返回 null,预览缺席但不阻止签名(sim 不在 confirmDisabled 条件里)。
5. (c) 顶层 error 是"节点拒绝了方法/参数",不是执行 revert;若误报 ok:false,则所有不支持 simulateV1 的 RPC 上,每一笔健康交易都会显示"预计失败"——sim-engine-rpc.ts:14-18 注释明确此区分,零填充 value 被 go-ethereum 拒绝就是真实案例。
6. 加分:validation:false 让模拟能花 Safe 没有的钱,所以原生流出还要对照真实余额做 underfundedNative 交叉检查(191、206-216 行);模拟日志不带鉴权,恶意合约可伪造 Transfer(_, you, big) 假装给你打钱——所以"收到"的 token 金额必须过 trusted set(链上稳定币/wrapped/已持有/curated)才敢渲染数值,否则 unverified;"流出"无法被这种方式低估,可直接渲染(248-286 行非对称信任模型)。

**代码证据**:
- `src/components/SigningRequestModal.tsx:252-258` — eth_sendTransaction 触发 simulateAssetChanges(带 cancelled 竞态守卫)
- `src/components/SigningRequestModal.tsx:307-312` — wallet_sendCalls 全批次一起模拟(顺序执行共享状态)
- `src/components/SigningRequestModal.tsx:556` — BalanceChangePreview 渲染,与 Send 共用一条渲染路径
- `src/services/tx-simulation.ts:169-185` — rpc → tevm → simulateCall 降级级联,全失败才 null
- `src/services/tx-simulation.ts:69-74` — changes:null(没引擎)与 changes:[](跑了没动)的语义区分注释
- `src/services/tx-simulation.ts:126-149` — simulateCall:res.error=真 revert,网络异常=null('no info, never false will-fail')
- `src/services/sim-engine-rpc.ts:57-71` — eth_simulateV1 payload:traceTransfers:true + validation:false
- `src/services/sim-engine-rpc.ts:80-84` — 顶层 error/非数组 → return null(degrade),注释:不是执行 revert
- `src/services/sim-engine-rpc.ts:27-32` — valueParam 注释:零填充 value 会被节点拒绝并曾表现为假 'Expected to fail'
- `src/services/tx-simulation.ts:191` — underfundedNative 交叉检查(validation:false 的补偿)
- `src/services/tx-simulation.ts:248-286` — enrichDeltas 非对称信任:流出可信、流入需过 trusted set 否则 unverified

**常见错误**:
- 【受训者原错】完全不知道存在交易模拟防线,以为签名 sheet 只有 clear signing 展示。
- 把 null 和 ok:false 混为一谈,认为"模拟失败就是交易会失败"(写反的后果是 RPC 不支持时全部健康交易误报"预计失败")。
- 以为模拟结果可以无条件信任(实际日志可伪造,流入金额必须过信任门控)。
- 以为模拟失败会禁用确认按钮(实际预览缺席不阻止签名,阻止签名的是 gas 估算失败)。

**追问**:
1. 为什么模拟 revert 时 deltas 保持为空([])而不是展示部分变动?(答:EVM 丢弃全部效果、没有诚实日志可展示,见 sim-engine-rpc.ts:107-109。)
2. read-only replay 时预览数据从哪来?为什么不能现场重算?(答:persisted assetSim / replaySim——链上状态已变,现场重算结果失真。)
3. underfundedNative 为什么在余额查询失败时选择"不警告"而不是"警告"?

**真懂 vs 背诵**:真懂的人能按"引擎级联 + 三态语义 + 非对称信任"三层复述并推演出"写反 null/ok:false 会造成全网误报"这个反事实;背诵的人只知道"有个模拟功能",说不出降级顺序和 null 的确切含义。

**评分规则**:3 分线 = 知道模拟防线存在、能说出 eth_simulateV1 主引擎和 null≠失败;5 分线 = 完整推演三个故障场景 + changes:[] vs null 区分 + validation:false/underfunded 与流入信任门控的原因。

### D5-D10-signing-security-Q5 · 难度 4/5

**考察目标**:考察 enforceNoUnlimited 作为提交咽喉处的最后兜底(受训者此前不知道 use-dapp-signing.ts:322/367 这道防线):UI 层全部失效时系统为何仍 fails closed,以及批次夹带与三类有意放行的边界(故障推演题)。

**题干**:故障推演:假设 UI 层出了 bug——ApprovalView 没渲染出来,或 confirm() 里 rewriteApprovalParams 抛异常导致 paramsOverride 为 undefined——一笔 unlimited approve 的原始 params 被原样传进 approveRequest。这笔授权会被签名上链吗?为什么?如果 dApp 改用 wallet_sendCalls 在批次第 3 个 leg 里夹带 unlimited approve 呢?这道兜底有哪些"有意放行"的形态?

**标准答案要点**:
1. 不会上链。handleDAppRequest 在路由到任何 handler 之前第一步就是 enforceNoUnlimited(method, request.params)(use-dapp-signing.ts:322),注释明确这是 'descriptor-independent safety net',专门接住绕过 UI cap 的请求。
2. detectApproval 重扫最终出站参数,uint256/uint160 金额 ≥ 对应 cap 即 throw UnlimitedApprovalError(approval-guard.ts:391-395),此时 Passkey.sign 尚未被调用——签名根本不会发生,系统 fails closed;confirm() 里对 rewrite 失败静默置 undefined 的 catch 正是依赖这个咽喉守卫兜底(SigningRequestModal.tsx:478-484 注释)。
3. 抛出的错误在 approveRequest 的 catch 中被接住并作为 signError 显示在 sheet 上(dapp-connection.tsx:579 catch 起,635 setSignError),请求不会以 unlimited 形式离开钱包。
4. 批次夹带也不行:handleSendCalls 对每个 leg 以 enforceNoUnlimited('eth_sendTransaction', [{to, data, value}]) 当作独立交易重查(use-dapp-signing.ts:364-368),注释明确 'batch must not smuggle an unbounded approval past the per-tx guard'。
5. 三类有意放行(approval-guard.ts:380-390):① typed-path off-chain permit——钱包改不了 dApp 上链的 struct,由 PermitSignView + 滑动确认门控;② isBooleanGrant(setApprovalForAll true)——没有金额可 cap,由显式 grant 选择 + hold 门控;③ isReducing(revoke/decreaseAllowance)——不授予任何权限,直接放行。
6. 这体现纵深防御分层:UI cap 编辑器是第一层,per-leg 批次守卫是第二层,提交咽喉的 enforceNoUnlimited 是最后一层,三层互相独立。

**代码证据**:
- `src/hooks/use-dapp-signing.ts:319-322` — handleDAppRequest 开头的 enforceNoUnlimited 兜底 + 'safety net' 注释
- `src/hooks/use-dapp-signing.ts:364-368` — handleSendCalls 对批次每个 leg 的独立重查
- `src/services/approval-guard.ts:370-375` — UnlimitedApprovalError 定义
- `src/services/approval-guard.ts:377-396` — enforceNoUnlimited 全逻辑:typed-path/booleanGrant/isReducing 三类放行 + 超 cap 抛错
- `src/components/SigningRequestModal.tsx:478-484` — confirm() 注释:'rewrite failure fails closed (never unbounded)',rewrite 抛错时 paramsOverride 置 undefined
- `src/components/SigningRequestModal.tsx:485-503` — 批次 leg 的 rewrite 失败同样依赖 per-leg 守卫 fails closed
- `src/models/dapp-connection.tsx:533` — handleDAppRequest 在 try 块中被调用
- `src/models/dapp-connection.tsx:579-586, 635-636` — catch 捕获错误(586 提取 msg),635 setSignError 显示在 sheet,636 以 -32603 回复 dApp

**常见错误**:
- 【受训者原错】不知道 enforceNoUnlimited 兜底存在,以为 UI 层的 cap 编辑器是唯一防线,UI bug = unlimited 直接放行。
- 以为批次里的 legs 不会被逐个检查,wallet_sendCalls 可以夹带绕过。
- 把三类有意放行(typed-path/boolean/reducing)当成守卫的漏洞,而不是"各自有专属门控"的职责划分。
- 以为 rewrite 抛异常会把请求整体拒绝(实际是静默降级为原始 params,由咽喉守卫兜底拒绝)。

**追问**:
1. 为什么 enforceNoUnlimited 放在 handleDAppRequest(共享路由层)而不是各 handler 内部或 transport 层?
2. handleSendCalls 的 per-leg 检查为什么伪装成 method='eth_sendTransaction' 来调 detectApproval?
3. 如果 dApp 把 unlimited 金额编码成 2^199(刚好低于 cap),守卫放行——这是漏洞吗?用户防线还剩什么?

**真懂 vs 背诵**:真懂的人能沿"UI 编辑器 → per-leg 批次守卫 → 提交咽喉"三层把故障推演到 UnlimitedApprovalError 抛出点,并说清三类放行各自的替代门控;背诵的人只会说"有校验",定位不到 322/367 两处也讲不出 fails closed 的因果链。

**评分规则**:3 分线 = 知道提交前有独立兜底会抛错、批次逐 leg 检查;5 分线 = 准确定位两处调用点、说出 fails closed 设计(rewrite 失败靠咽喉兜底)、完整列出三类有意放行及其替代门控。

### D5-D10-signing-security-Q6 · 难度 5/5

**考察目标**:考察修改影响面分析能力:把 SIWE 域名绑定从"警告"升级为"mismatch 禁止确认",需要理解 siwe.ts 三态语义、SigningSheet 单一渲染路径、状态所在层级与验证手段(修改影响题)。

**题干**:需求变更:目前 SIWE 域名不匹配只弹红色警告,用户仍可签名。现在要求 binding === 'mismatch' 时直接禁用确认按钮。要动哪些文件哪些位置?'unknown' 状态怎么处理、为什么?有哪些边界情况会被误伤?改完怎么验证?

**标准答案要点**:
1. 现状定位:SIWE 解析与绑定检查在 MessageSignView 叶子组件内(SigningRequestModal.tsx:1129-1133,parseSiwe + checkSiweDomainBinding,origin 取 dappInfo?.url ?? incomingRequest.origin,见 415-417);mismatch 目前只渲染 danger WarningBanner(1158-1163),不影响按钮。
2. 核心改动:confirmDisabled 在 SigningSheet 顶层(516-522),而 binding 算在叶子组件里——必须把 personal_sign 的 SIWE 解析/绑定提升到 SigningSheet 层(或经回调上抛),再把 binding==='mismatch' 加入 confirmDisabled;或者退一档改为 mismatch 时 requiresHold(469-475)强制滑动确认。
3. unknown 处理:checkSiweDomainBinding 是三态('ok'/'mismatch'/'unknown',siwe.ts:95-113),unknown = 请求 origin 缺失或不可解析——不能阻断,否则没有 origin 的合法通道(如原生 WalletPair 场景)的所有 SIWE 登录全部被杀;只有确证的 mismatch 才禁用。
4. 不能误伤的边界:① parseSiwe 是保守解析(首行锚定 + 拒绝 domain 中的 userinfo/path/scheme,siwe.ts:38/45),非 SIWE 消息返回 null 走普通消息视图,新逻辑不得影响;② CRLF 规范化(siwe.ts:33-36)保证 web payload 的 \r\n 不会让解析失败而静默绕过新禁用逻辑;③ siweHost 用 hostname 丢弃端口、剥尾点(80-93),避免非默认端口造成假 mismatch 反而把合法登录禁死;④ readOnly replay 路径没有确认按钮,确认新状态不污染历史回放。
5. 单一渲染路径红利:SigningSheet 是唯一签名渲染面(95-103 注释,prod modal 与 Clear-Signing 测试 harness 共用),只改这一处,两个宿主同时生效——不允许在 harness 里另写一份逻辑。
6. 验证:siwe.ts 纯函数无依赖可直接单测(mismatch/ok/unknown/非 SIWE/CRLF/userinfo 注入各一例);UI 层在 parallel space 测试环境的 test-dApp 构造 domain≠origin 的 personal_sign 验证按钮禁用,再验证 ok 与非 SIWE 消息不受影响;回归确认 harness 中同样生效。

**代码证据**:
- `src/components/SigningRequestModal.tsx:1129-1133` — parseSiwe + checkSiweDomainBinding 目前在 MessageSignView 叶子内计算
- `src/components/SigningRequestModal.tsx:1158-1163` — mismatch 目前仅渲染 danger WarningBanner,不禁用按钮
- `src/components/SigningRequestModal.tsx:415-417` — requestOrigin 来源:dappInfo?.url ?? incomingRequest.origin
- `src/components/SigningRequestModal.tsx:516-522` — confirmDisabled 所在层级(SigningSheet 顶层),新条件的落点
- `src/components/SigningRequestModal.tsx:469-475` — requiresHold 备选方案落点
- `src/services/siwe.ts:95-113` — SiweBinding 三态定义与 unknown 语义(不知道 ≠ 钓鱼)
- `src/services/siwe.ts:38-45` — 首行锚定 + 拒绝 domain 携带 userinfo/path/scheme(防 'uniswap.org@evil.com' 前缀伪装)
- `src/services/siwe.ts:33-36` — CRLF 规范化注释:否则尾部 \r 破坏首行锚、静默禁用钓鱼检测
- `src/services/siwe.ts:80-93` — siweHost:hostname 丢端口、剥尾点、解析失败返回 null(fail safe)
- `src/components/SigningRequestModal.tsx:95-103` — SigningSheet 单一渲染路径注释(prod + harness 共用)
- `src/services/siwe.ts:1-11` — 文件头:personal_sign 是第一钓鱼面、域名绑定是唯一防御

**常见错误**:
- 只在 MessageSignView 里加个本地 disabled,没意识到确认按钮的 confirmDisabled 在 SigningSheet 顶层,状态层级不通。
- 把 unknown 也一并禁用,杀掉所有无 origin 通道的合法 SIWE 登录(混淆"不知道"与"钓鱼")。
- 忽略 parseSiwe 返回 null 的非 SIWE 消息路径,把普通 personal_sign 一起禁了。
- 在测试 harness 里复制一份新逻辑,违反单一渲染路径 mandate(安全 UI 不允许两份实现)。
- 忘记端口/尾点/CRLF 这些会制造假 mismatch 或假 null 的规范化细节。

**追问**:
1. 为什么 siwe.ts 要拒绝 domain 里的 '@'?攻击者用 'uniswap.org@evil.com' 能骗过什么?
2. 如果 dApp 的 SIWE 消息 domain 带非默认端口(app.xyz:8443)而 origin 不带,现在的实现判 ok 还是 mismatch?为什么?
3. 升级为硬禁用后,dApp 收到什么响应更合理——用户手动 Reject,还是钱包自动回一个特定错误码?

**真懂 vs 背诵**:真懂的人会先指出状态在叶子组件而按钮门在顶层这个结构性障碍,并坚持 unknown≠mismatch 的三态语义;背诵的人直接说"在警告旁边把按钮 disable 掉",暴露没读过组件层级和 fail-safe 设计。

**评分规则**:3 分线 = 找对两处代码位置并知道 unknown 不能阻断;5 分线 = 完整给出状态提升方案、四类误伤边界(非 SIWE/CRLF/端口/replay)、单一渲染路径约束和纯函数单测+parallel space 实测的验证计划。

---

## D6-D7-reads-and-state — 链上读取与本地状态

### D6-D7-reads-and-state-Q1 · 难度 1/5

**考察目标**:代码定位 + 基本架构:能说出余额读取住在哪个 service 文件、用什么机制、一条链发几次 RPC(直击基线诊断中"说不出任何 service 文件名"的短板)。

**题干**:首页的多链代币余额和价格是从哪里来的?请说出:(a) 负责的 service 文件名;(b) 它用什么合约/机制把一条链上十几个余额+价格查询压成几次 RPC;(c) 结果缓存多久、两个组件同时刷新同一地址会发几次网络请求?

**标准答案要点**:
1. 文件是 src/services/wallet-api.ts(入口函数 fetchTokens),没有任何后端 indexer——全部是客户端直连 RPC。
2. 机制是 Multicall3:每条链把 native 余额(getEthBalance)+ 各 ERC-20 balanceOf/decimals + DEX 报价 + Chainlink feed 打包成一个 aggregate3,通过 poolRpcCall 发一次 eth_call;各链之间用 Promise.allSettled 并行。
3. 缓存 TTL 是 TOKEN_CACHE_TTL_MS = 5 分钟,按小写地址为 key 存内存 Map(tokenCache)。
4. in-flight 去重:并发调用命中 cached.inFlight 时直接 await 同一个 Promise,所以两个组件同时刷新只发一轮请求。
5. 每条链还有 PER_CHAIN_TIMEOUT_MS = 18 秒的 Promise.race 兜底,慢链超时后本轮贡献空数组、下轮重试。

**代码证据**:
- `src/services/wallet-api.ts:4-9` — 文件头注释:每个网络一次 eth_call、Multicall3 批量余额+价格
- `src/services/wallet-api.ts:55` — TOKEN_CACHE_TTL_MS = 5 * 60 * 1000
- `src/services/wallet-api.ts:98-100` — cached.inFlight 命中即复用同一 Promise(in-flight 去重)
- `src/services/wallet-api.ts:191-203` — PER_CHAIN_TIMEOUT_MS = 18_000 + Promise.race 单链超时兜底
- `src/services/wallet-api.ts:350-352` — encAggregate3(calls) 后 ethCall(chainId, MULTICALL3, encoded)
- `src/services/wallet-api.ts:592-597` — ethCall 底层走 poolRpcCall('eth_call', ...)

**常见错误**:
- 说不出文件名,只能说"大概有个 API 层"(基线诊断的真实表现)。
- 以为接了 Alchemy/Moralis 之类的余额 indexer 后端,其实 fetchTokens 全是链上 eth_call。
- 以为每个 token 一次 RPC,不知道 Multicall3 把整条链压成一次 eth_call。
- 把 balance-cache.ts(账户 USD 总额缓存,24h)和 wallet-api 的 5 分钟 token 缓存混为一谈。

**追问**:
1. includeZeroBalance 选项为什么要绕过缓存、也不写入缓存?(参考 wallet-api.ts:97-112)
2. getCachedHeldTokens 为什么必须是同步、cache 冷时返回空而不触发 fetch?哪个安全功能在用它?
3. fetchTokens 抛错时缓存条目是怎么处理的?(答:有旧数据则回写保留、没有就删除,wallet-api.ts:120-127。)

**真懂 vs 背诵**:真懂的人能顺着 fetchTokens → fetchAllChainTokens → queryChainAssets → poolRpcCall 讲出调用链并解释 in-flight 去重解决什么并发问题;背诵的人只会复述"有个 5 分钟缓存"。

**评分规则**:3 分线 = 说出 wallet-api.ts + Multicall3 一链一次 eth_call + 5 分钟缓存;5 分线 = 再讲清 in-flight 去重的并发语义和 18 秒单链超时兜底。

### D6-D7-reads-and-state-Q2 · 难度 2/5

**考察目标**:AsyncStorage vela.* 键空间清单意识:知道 storage.ts 的 KEYS 表只是键空间的一部分,以及 clearAll() 的清理盲区(直击基线诊断"不知道 vela.* 键空间")。

**题干**:Vela 所有本地持久化都在 AsyncStorage 的 vela.* 键空间下。请回答:(a) storage.ts 的 KEYS 表里有哪些键(说出至少 6 个)?(b) KEYS 表之外还有哪些模块自己持有 vela.* 键(说出至少 3 个,含文件)?(c) 登出时 clearAll() 会清掉哪些、漏掉哪些?

**标准答案要点**:
1. KEYS 表 11 个键:vela.accounts、vela.activeAccountIndex、vela.pendingUploads、vela.customTokens、vela.networkConfig、vela.serviceEndpoints、vela.transactionHistory、vela.priceSource、vela.customNetworks、vela.localePrefs、vela.rpcProviders。
2. 表外自持键(本域内):vela.rpc.banned(rpc-pool 封禁表)、vela.balanceCache(账户 USD 总额缓存)、vela.remoteInjectSession(dApp 会话)、vela.lastScan.{chainId}.{address}(transfer-monitor 动态 key——现行扫描已改为无 checkpoint 的滑动窗口,该键不再被写入,只在 resetTransferCheckpoints 里被清理的遗留键)。
3. 表外还有 vela.contacts / vela.displayCurrency / vela.language / vela.walletpairSession / vela.tokenMeta.* 等(能说出任 3 个即可)。
4. clearAll() 只遍历 Object.values(KEYS) 逐个 removeItem——表外的键全部残留:登出后 RPC 封禁表、余额缓存、dApp 会话、扫描位点都不会被清。
5. 动态 key(vela.lastScan.*、vela.tokenMeta.*)不在任何清理表里;transfer-monitor 提供 resetTransferCheckpoints(address, chainIds) 按链逐个 removeItem(其注释写明用于切换账户/登出),但当前代码库没有任何调用方——动态键实际从不被清理。

**代码证据**:
- `src/services/storage.ts:13-25` — KEYS 表全 11 个键
- `src/services/storage.ts:560-564` — clearAll() 只清 Object.values(KEYS)
- `src/services/rpc-pool.ts:68` — BANNED_STORAGE_KEY = 'vela.rpc.banned'
- `src/services/balance-cache.ts:10` — STORAGE_KEY = 'vela.balanceCache'
- `src/models/dapp-connection.tsx:49` — STORAGE_KEY = 'vela.remoteInjectSession'
- `src/services/transfer-monitor.ts:73-75` — lastScanKey() 生成 vela.lastScan.{chainId}.{address}
- `src/services/transfer-monitor.ts:242-250` — resetTransferCheckpoints 还顺带清早期 vela.nativeBal.* / vela.nativeBlk.* 遗留键
- `src/services/contacts.ts:61-63` — vela.contacts / vela.contacts.dismissed / vela.contactGroups

**常见错误**:
- 完全不知道有 vela.* 前缀约定(基线诊断的真实表现)。
- 以为 clearAll() 会按前缀清掉所有 vela.* 键——实际它只认 KEYS 表,余额缓存/封禁表/会话全残留。
- 不知道存在动态拼接的键(vela.lastScan.{chain}.{addr}),以为键都是静态常量。
- 把 vela.transactionHistory(本地交易记录)和 vela.balanceCache(USD 总额)混淆。

**追问**:
1. 登出后 vela.balanceCache 残留意味着什么?下一个在同设备创建钱包的用户会看到上任的余额吗?(答:cache 按地址 key,新地址查不到——但数据仍在盘上。)
2. 如果要把 clearAll 改成前缀清扫(getAllKeys + 前缀过滤),会误伤哪些应该跨登出保留的键?(如 vela.language、vela.colorScheme。)
3. vela.rpc.banned 残留对下一次冷启动是好事还是坏事?(答:好事——免重踩坑;坏事——换网络环境后旧 ban 还压 1-24 小时。)

**真懂 vs 背诵**:真懂的人知道键空间分"KEYS 表 + 各模块自持 + 动态拼接"三层,并能推出 clearAll 的盲区;背诵的人最多背出 KEYS 表几个名字。

**评分规则**:3 分线 = 说出 6 个 KEYS 键 + 知道 clearAll 只清 KEYS 表;5 分线 = 再说出 3 个表外键(含 vela.rpc.banned 或 vela.lastScan.* 动态键)并讲清残留后果。

### D6-D7-reads-and-state-Q3 · 难度 3/5

**考察目标**:故障推演 + 余额铁律:429 全灭时首页余额、横幅、last-known-good 总额各自的行为(直击基线诊断"余额铁律答反、以为失败要引导用户换 RPC")。

**题干**:故障推演:某轮刷新时 Polygon(137)的所有 RPC 端点都返回 HTTP 429,其他链正常。请推演:(a) rpc-pool 内部发生什么(重试?封禁?哪些集合被标记)?(b) 首页上 Polygon 的余额显示什么——清零还是保留?靠哪段代码保证?(c) 会弹"换 RPC"横幅吗?(d) 账户 USD 总额缓存会被这轮结果覆盖吗?

**标准答案要点**:
1. rpc-pool:429 抛 RateLimitError,只 recordFailure 进评分冷却、绝不 tempBan/硬封;全端点失败后带 jitter 重试一轮,仍失败则 throw,同时 rpcFailedChains.add(137) 且因 sawRateLimit=true 把 137 也加入 rpcRateLimitedChains。
2. wallet-api:queryChainAssets 对 137 catch 后返回 [],该链本轮"贡献空"而不是"贡献零"——merge 单位是链。
3. HomeScreen merge-by-chain:onProgress 里 kept = prev.filter(t => !fresh.has(tokenChainId(t))),Polygon 不在 fresh 集合里,旧 token 全部保留——这就是余额铁律:失败链回退上一次的值,绝不清零健康链,也绝不显示 $0。
4. 不弹横幅:RpcIssue 横幅的 chainIds 是 failedChainIds.filter(id => !rateLimitedChainIds.includes(id)),429 属瞬态自愈,UI 保持安静、继续用缓存值——引导用户换 RPC 是错误答案。
5. 总额缓存不被污染:只有 failed.length === 0 且所有持仓都有价格时才 setAccountBalance 写入 last-known-good,这轮 137 失败所以不写。
6. 回归测试锚点:wallet-api-merge.test.ts(失败链不清零健康链)和 rpc-pool-ratelimit.test.ts(429 归类瞬态、不进换 RPC 横幅)。

**代码证据**:
- `src/services/rpc-pool.ts:560-563` — RateLimitError:'fail over now … never a hard ban'
- `src/services/rpc-pool.ts:607-610` — HTTP 429 → throw RateLimitError
- `src/services/rpc-pool.ts:788-792` — RateLimitError 分支:sawRateLimit=true,只 recordFailure
- `src/services/rpc-pool.ts:804-818` — 全端点失败后 jitter 重试一次;终局 rpcFailedChains.add + sawRateLimit 时 rpcRateLimitedChains.add
- `src/services/rpc-pool.ts:164-176` — getRateLimitedChains 注释明说:UI 不得因此提示用户换自己的 RPC
- `src/services/wallet-api.ts:194-203` — queryChainAssets(...).catch(() => []) — 失败链贡献空,不影响其他链
- `src/screens/wallet/HomeScreen.tsx:331-343` — merge-by-chain:fresh 集合外的 prev token 全保留
- `src/screens/wallet/HomeScreen.tsx:348-358` — rateLimitedChainIds 快照 + failed.length===0 才写 setAccountBalance
- `src/screens/wallet/HomeScreen.tsx:665` — 横幅 chainIds 过滤掉 rateLimited 链
- `src/__tests__/services/wallet-api-merge.test.ts:51-63` — 回归测试:失败链不清零健康链
- `src/__tests__/services/rpc-pool-ratelimit.test.ts:41-73` — 回归测试:429 → failed+rateLimited 双标记,成功后双清除

**常见错误**:
- 【受训者原错】铁律答反:"Polygon 余额会显示 0/报错,应该弹横幅引导用户换 RPC"(429 恰恰是唯一明确不弹换 RPC 提示的失败类型)。
- 以为 429 会把端点 ban 1 小时(429 只进评分冷却,tempBan 留给 401/403/404 和 auth 类 JSON-RPC 错)。
- 以为保留旧值靠的是 AsyncStorage 的 vela.balanceCache(那是账户级 USD 总额,链级保留靠的是 HomeScreen 内存里的 merge-by-chain)。
- 以为失败这轮也会更新总额缓存,导致 last-known-good 被部分总额污染。

**追问**:
1. 同样场景换成所有端点返回 401,推演有什么不同?(答:HTTP 401 走 HttpBanError → tempBan+maybePermaBan,rpcRateLimitedChains 不标记,横幅会弹。)
2. Polygon 恢复后,哪段代码把 137 从两个集合里摘掉?(答:rpc-pool.ts:777-778 recordSuccess 路径。)
3. 怎么在开发环境手工复现这个场景?(答:fault-injection 的 vela.rateLimitRpc,rpc-pool.ts:707-713。)

**真懂 vs 背诵**:真懂的人能把"铁律"落到具体代码——merge 单位是链、fresh/kept 集合运算、横幅的减法过滤;背诵的人只会说"失败保留缓存",一追问 429 和 401 的区别就露馅。

**评分规则**:3 分线 = 铁律方向答对(保留不清零)+ 知道 429 不弹换 RPC 横幅;5 分线 = 完整讲出双集合标记、merge-by-chain 代码位置、总额缓存写入条件三件事。

### D6-D7-reads-and-state-Q4 · 难度 3/5

**考察目标**:pending-at-submit 持久化时机 + 重启收敛:一笔 dApp 交易从批准到确认的存储生命周期,以及 App 被杀后谁负责收敛(直击基线诊断"pending-at-submit 时机未说出")。

**题干**:用户在 dApp 里批准了一笔 eth_sendTransaction,链上确认要 30 秒。请回答:(a) 这条记录第一次写进 AsyncStorage 是在哪个时刻、状态是什么、由哪段代码写?(b) 确认后怎么更新——新写一条还是原地改?(c) 如果写入后 App 立刻被杀,重启后由哪些机制把它从 pending 收敛到 confirmed/failed?各自的触发点和保守规则是什么?

**标准答案要点**:
1. 第一次持久化在 bundler 接受、拿到 userOpHash 的那一刻(onSubmitted 回调),不是确认后:dapp-connection.tsx approveRequest 里 (hash) => buildSigningRecord({status:'pending', userOpHash:hash}) + saveTransaction——所以关掉签名面板/刷新页面都不会丢。
2. 确认后原地改:先 await pendingSave 保证 pending 写入已落盘,再 updateTransaction(同 id) flip 成 confirmed+txHash——'never a second record, never a lost pending→confirmed race';终态失败则 flip 成 failed。
3. 重启收敛机制一:dapp-connection 的 resume effect——启动时 loadTransactions 过滤 status==='pending' && type==='dapp_tx' && 有 userOpHash && 24h 内,对每条 waitForReceipt 后 flip confirmed。
4. 重启收敛机制二:tx-reconciler.reconcilePendingTransactions——Home 聚焦/定时触发,向 bundler 发 eth_getUserOperationReceipt,受 12 秒节流(MIN_INTERVAL_MS)和 _running 互斥保护。
5. 保守规则:null/超时/RPC 错误一律留 pending 下轮重试(timeout 永不当失败);只有 receipt.success === false 才标 failed;超过 RECONCILE_MAX_AGE_MS(24h)停止轮询,记录停在 pending = 诚实的"未知",绝不猜 failed。
6. reconciler 只碰 txHash === ''(尚未确认)且属于当前地址的记录;Send 流程同样在提交时写 pending,reconciler 是两条路径共用的恢复半边。

**代码证据**:
- `src/models/dapp-connection.tsx:525-546` — 注释 'recorded pending the moment the bundler accepts it (in onSubmitted) — BEFORE the long on-chain receipt wait' + saveTransaction(pending)
- `src/models/dapp-connection.tsx:554-561` — await pendingSave 后 updateTransaction 同 id flip confirmed
- `src/models/dapp-connection.tsx:628-634` — 提交后终态失败 → flip 'failed'
- `src/models/dapp-connection.tsx:750-766` — resume effect:重启后捞 24h 内 pending dapp_tx,waitForReceipt 收敛
- `src/services/tx-reconciler.ts:11-14` — 'SendScreen and the dApp connection persist the pending record at submit time … this reads those back and converges them'
- `src/services/tx-reconciler.ts:29-31` — RECONCILE_MAX_AGE_MS = 24h,MIN_INTERVAL_MS = 12s
- `src/services/tx-reconciler.ts:86-93` — 过滤条件:pending + userOpHash + txHash==='' + 当前地址 + 24h 内
- `src/services/tx-reconciler.ts:99-108` — transient 留 pending;success===false → failed;否则 confirmed+txHash
- `src/screens/wallet/HomeScreen.tsx:292-308` — Home 聚焦时调 reconcilePendingTransactions,有收敛才重读 feed
- `src/services/storage.ts:432-451` — withTxLock 串行化 + saveTransaction 按 id 去重(重提交同 userOpHash 不产生重复行)

**常见错误**:
- 【受训者原错】以为记录在链上确认后才第一次写盘——那样关面板/杀 App 就永远丢了这笔交易的踪迹(基线诊断中说不出提交时机)。
- 以为确认时是追加一条新记录(实际是同 id 原地 patch,否则 Activity 会出现 pending+confirmed 两行)。
- 以为 reconciler 超时/查不到就标 failed(设计恰好相反:timeout 永不当失败,24h 后停在诚实的 pending)。
- 不知道存在两条收敛路径(dapp-connection resume 只管 dapp_tx,tx-reconciler 管所有带 userOpHash 的 pending)。

**追问**:
1. 为什么 flip confirmed 前必须 await pendingSave?去掉这个 await 会出什么竞态?(答:updateTransaction 找不到 id 变 no-op,记录永远 pending。)
2. withTxLock 解决的是什么真实事故?(答:批量发送 N 条 sibling 并发读改写互相覆盖,批次在 Activity 里塌成一行——storage.ts:424-431 注释。)
3. eth_getUserOperationReceipt 走的是 bundler 还是链 RPC?为什么 reconcile 要 12 秒节流?

**真懂 vs 背诵**:真懂的人能画出"submit 写 pending → await 后同 id flip → 重启双路径收敛"的完整状态机并解释每条保守规则防的事故;背诵的人只会说"有个 pending 状态"。

**评分规则**:3 分线 = 说对"bundler 接受即写 pending"+ 知道重启后有 reconciler 收敛;5 分线 = 再讲出同 id flip 的竞态防护、24h/12s 两个常量语义和"timeout 永不当失败"的保守原则。

### D6-D7-reads-and-state-Q5 · 难度 4/5

**考察目标**:RPC 池核心机制精读:端点评分公式、两级封禁的准入条件与时长、四类错误分类(permanent/transient/rate-limit/range-cap)各走什么处置。

**题干**:rpc-pool 是所有链上读写的地基。请讲清:(a) endpointScore 的完整公式——来源优先级表、延迟罚分、成功加分、失败冷却各是多少?(b) 临时封禁和永久封禁的触发条件与时长(常量名+数值)?(c) 一个 JSON-RPC error 回来,代码按什么顺序把它分成四类(永久/瞬态/限流/getLogs 范围帽),各自的处置有何不同?(d) 一条链所有端点都被 ban 了怎么办?

**标准答案要点**:
1. SOURCE_PRIORITY 六档:user=10000 > provider=9000 > default=1000 > public=500 > builtin=100 > fallback=10;banned 直接 -Infinity。
2. 延迟罚分:avgLatencyMs(EMA,0.7 旧+0.3 新)超 200ms 部分每 10ms 扣 1 分、封顶扣 200;成功加分 = min(成功次数, 50);失败冷却:冷却期内(30s×2^(n-1)、封顶 5 分钟)直接 -50000 等效禁用,冷却期外每次连败扣 200。
3. 临时 ban:isPermanentRpcError(unauthorized/api key/forbidden/payment required/exceeded/subscription/specify an address 等)或 HTTP 401/403/404 → tempBan,TEMP_BAN_TTL_MS = 1 小时,写入 vela.rpc.banned 持久化。
4. 永久 ban 条件:maybePermaBan——该端点从未成功过(successes===0)且 totalFailures >= PERMA_BAN_MIN_FAILURES(6);"永久"其实也有 PERMA_BAN_TTL_MS = 24 小时的过期,允许从瞬时故障恢复。
5. 分类顺序(poolRpcCall 内):先查 eth_getLogs 的 getLogsRangeCap(必须最先——range 错误常带 'exceed'/-32000,晚查会被误 ban),命中记 success 并原样返回让调用方劈半;再查 isPermanentRpcError → ban+failover;再查 isTransientServerError(internal error/-32603/-32000..-32099 等)→ 只 recordFailure 换下一个端点,不 ban;HTTP 429/isRateLimitSignal → sawRateLimit 标记,只冷却绝不 ban。
6. 全部被 ban:poolRpcCall 发现排序后端点为 0,清空该链 banMap + banned 标记 + consecutiveFailures 重建池再试——保证永远有路可走。

**代码证据**:
- `src/services/rpc-pool.ts:379-386` — SOURCE_PRIORITY 六档数值
- `src/services/rpc-pool.ts:388-415` — endpointScore:banned=-Infinity、延迟罚分、min(successes,50)、冷却 -50000/指数退避
- `src/services/rpc-pool.ts:522-531` — recordSuccess:EMA 0.7/0.3、consecutiveFailures 清零
- `src/services/rpc-pool.ts:63-71` — 两级 ban 注释 + TEMP_BAN_TTL_MS=1h、PERMA_BAN_TTL_MS=24h、PERMA_BAN_MIN_FAILURES=6
- `src/services/rpc-pool.ts:147-154` — maybePermaBan:successes===0 && totalFailures>=6
- `src/services/rpc-pool.ts:422-440` — isPermanentRpcError 消息清单(含 publicnode BSC 'specify an address' 特判)
- `src/services/rpc-pool.ts:447-462` — isTransientServerError:-32603/-32000..-32099 + 消息匹配,revert/gas/execution 豁免
- `src/services/rpc-pool.ts:476-511` — getLogsRangeCap:返回端点声明的块跨度上限/0(劈半)/null(非范围错)
- `src/services/rpc-pool.ts:743-754` — poolRpcCall 中 range-cap 检查 'must come before the permanent/transient checks'
- `src/services/rpc-pool.ts:604-610` — HTTP 401/403/404 → HttpBanError;429 → RateLimitError
- `src/services/rpc-pool.ts:719-730` — 全 ban 时清 banMap 重建池
- `src/services/transfer-monitor.ts:103-113` — range-cap 的消费方:scanRecentTransfers 收到 cap 后只再发一次收窄的 getLogs

**常见错误**:
- 以为 429 也走 tempBan 一小时(429 只进评分冷却;把公共端点因限流 ban 掉一小时会白白烧掉整条链的容量)。
- 以为永久 ban 真的永久(有 24h TTL,注释明说是为了从瞬时故障恢复)。
- 说不出分类的先后顺序,不知道 getLogs range 错误必须最先判——它常含 'exceed' 字样,晚判会把健康端点误永 ban。
- 以为评分只看延迟,说不出 user 覆盖 > provider key > 内置的来源优先级压倒延迟的设计。

**追问**:
1. 为什么 'exceeded' 同时出现在 isPermanentRpcError 和 isRateLimitSignal 里?一个限流错会不会被临时 ban?(答:会 tempBan 1h,但 sawRateLimit 仍把链标成瞬态——ban 决策和链级分类是解耦的,rpc-pool.ts:565-567 注释。)
2. 读操作和 bundler 操作的超时为什么不同?(答:RPC_READ_TIMEOUT_MS 8s vs REQUEST_TIMEOUT_MS 15s,rpc-pool.ts:544-552。)
3. 用户刚换掉某网络的自定义 RPC,为什么 refreshPool 还要顺手删 fastestRpcCache?(答:X-Rpc-Url 会把旧端点递给 bundler 最长 1 小时,rpc-pool.ts:994-999。)

**真懂 vs 背诵**:真懂的人能解释每个数值背后的事故(为什么 range-cap 要最先判、为什么 429 不 ban、为什么永 ban 留 24h 后门);背诵的人只能报出几个常量数字,顺序和例外一问就乱。

**评分规则**:3 分线 = 说对优先级表大小关系 + 两级 ban 的条件和时长 + 429 不 ban;5 分线 = 完整公式(冷却指数/封顶值)+ 四类错误的判定顺序及 range-cap 前置的原因 + 全 ban 自愈路径。

### D6-D7-reads-and-state-Q6 · 难度 5/5

**考察目标**:修改影响题 + require cycle 地雷:沿现有 ServiceEndpoints 模式加一个可配置端点并在 models/types.ts 里消费,识别 storage.ts↔types.ts 的模块环及安全改法、验证手段。

**题干**:需求:把 models/types.ts 里 nftImageURL 硬编码的 ipfs.io 网关改成用户可配置(像 ethereumDataURL 一样进 ServiceEndpoints)。请回答:(a) 需要动哪些文件的哪些位置?(b) types.ts 和 storage.ts 之间现存什么地雷?你的新代码怎么写才不会踩?(c) 同步 getter 能工作的前提是什么、在哪里被满足?(d) 改完怎么验证?

**标准答案要点**:
1. 改动位置:types.ts 的 ServiceEndpoints 接口 + DEFAULT_SERVICE_ENDPOINTS 加字段;storage.ts 仿照 getEthereumDataURL 加同步 getter(读 _endpointsCache,空则回退默认);types.ts 的 nftImageURL 改调新 getter;SettingsScreen 的端点编辑弹层加输入项。
2. 地雷是双向模块环:types.ts:7 import getEthereumDataURL from '@/services/storage',而 storage.ts:9 反向 value-import DEFAULT_SERVICE_ENDPOINTS/DEFAULT_LOCALE_PREFS from '@/models/types'(storage.ts:8 只是 import type、编译期擦除)——两条边都是值导入,构成 require cycle。
3. 环现在没炸的原因:types.ts 对 storage 的使用全部在函数体内(调用时求值,如 tokenLogoURLs),而 storage.ts 在模块初始化时就展开 DEFAULT_SERVICE_ENDPOINTS(第 171 行 _endpointsCache = {...DEFAULT_SERVICE_ENDPOINTS})——加载顺序不利时后者拿到的是部分初始化的导出;安全改法 = 新代码保持"跨环引用只在调用时、绝不在模块初始化时求值",或把共享常量下沉到无依赖的叶子模块。
4. 同步 getter 的前提:_endpointsCache 必须在启动时被 loadServiceEndpoints() 灌好——app/_layout.tsx:156 在启动序列里调用;rpc-pool 的 initPool 也会再保险一次(rpc-pool.ts:227);新 getter 若在灌注前被调用,拿到的是默认值而非用户配置,必须能接受这种降级。
5. 验证:yarn typecheck + jest 全量(尤其 wallet-api-* 系列,它们 mock 了 storage);盯 Metro console 的 'Require cycle' 警告没有新增;跑真机验证 nftImageURL 在冷启动首帧(缓存未灌)和改配置后(灌注+notify)两种时序下都正确;沿用 SettingsScreen 现有 endpoints 编辑路径手测保存/重启生效。
6. 附带影响检查:clearAll 会清 vela.serviceEndpoints(在 KEYS 表内),登出后回默认——符合预期,无需额外清理。

**代码证据**:
- `src/models/types.ts:174-180` — nftImageURL 硬编码 https://ipfs.io/ipfs/ 网关
- `src/models/types.ts:266-282` — ServiceEndpoints 接口(加字段处)
- `src/models/types.ts:307-314` — DEFAULT_SERVICE_ENDPOINTS(加默认值处)
- `src/models/types.ts:7` — 环的一边:types.ts import getEthereumDataURL from '@/services/storage'
- `src/services/storage.ts:9` — 环的另一边:storage.ts value-import DEFAULT_SERVICE_ENDPOINTS from '@/models/types'(第 8 行是 import type,不构成运行时边)
- `src/services/storage.ts:171` — 模块初始化时展开 DEFAULT_SERVICE_ENDPOINTS(环上最脆的一环)
- `src/services/storage.ts:194-212` — getEthereumDataURL/getBundlerServiceURL/getFiatRatesURL 同步 getter 模板(空则回退默认)
- `src/models/types.ts:119-130` — tokenLogoURLs 展示"调用时求值"的安全消费姿势
- `src/app/_layout.tsx:156` — 启动序列里 loadServiceEndpoints() 灌注缓存
- `src/services/rpc-pool.ts:226-227` — initPool 再次 await loadServiceEndpoints() 兜底
- `src/screens/settings/SettingsScreen.tsx:423` — 端点编辑弹层 loadServiceEndpoints/保存入口

**常见错误**:
- 只在 types.ts 里把字符串换成另一个硬编码,没进 ServiceEndpoints 体系(用户配不了、Settings 里看不见)。
- 完全不知道 types.ts↔storage.ts 存在 require cycle,在 types.ts 顶层(模块初始化时)调用 storage 的函数或在 storage.ts 顶层消费更多 types 导出,把环踩实。
- 在 nftImageURL(同步、渲染期调用)里用 await AsyncStorage.getItem 直接读盘,不懂"启动灌注 + 同步 getter"的既有模式。
- 验证只跑 typecheck——环问题 TS 编译完全无感,只有运行时(Metro 警告/首帧行为)能暴露。

**追问**:
1. 如果把 DEFAULT_SERVICE_ENDPOINTS 挪进 storage.ts 自己文件里,环就断了吗?还有哪些文件在 import 它?(需 grep 确认所有消费方。)
2. 为什么 saveServiceEndpoints 后 rpc-pool 不会立刻看到新 bundler URL?哪个函数负责让池重读配置?(答:invalidateAllPools/refreshPool,rpc-pool.ts:994-1005。)
3. localePrefs 的 getter 为什么比 endpoints 多一套 listener + globalThis 锚定?(答:渲染期同步读 + Fast Refresh 会换掉模块级 Set,storage.ts:221-233。)

**真懂 vs 背诵**:真懂的人能指出环的两条边各在哪一行、说清"调用时求值 vs 模块初始化求值"为什么决定生死,并给出运行时验证手段;背诵的人只会说"加个字段加个 getter",对环和灌注时序毫无警觉。

**评分规则**:3 分线 = 找全四处改动位置 + 知道同步 getter 依赖启动灌注;5 分线 = 准确指出双向环的两行 import、解释安全消费姿势(仅调用时求值),并给出含 Metro require-cycle 警告和冷启动首帧在内的验证清单。

---

## D8-D9-framework-tests — 工程框架与验证基础设施

### D8-D9-framework-tests-Q1 · 难度 1/5

**考察目标**:补上基线诊断中最致命的空白:受训者说不出任何测试/构建命令、不知道测试规模。考察本地提交前门禁四件套 + 两个 opt-in 测试开关 + 项目测试规模数字。

**题干**:你改完一段代码准备提交。在推到 main 之前,本地要跑哪几条命令?每条命令背后是多大规模的检查?另外有哪些测试默认不跑、分别用什么开关打开、为什么默认关?

**标准答案要点**:
1. 四件套与 CI 完全同款:npx tsc --noEmit(类型检查)、npm run lint(expo lint)、npm test(Jest)、npm run build:web(生产 Web 构建,~11MB 输出到 dist/)——.github/workflows/ci.yml 的 app job 就是这四步。
2. Jest 规模:79 个套件 / 1022 个用例,jest.config.js 只收 src 下 __tests__/**/*.test.ts,testEnvironment 是 node(纯逻辑测试,不渲染 RN 组件)。
3. Playwright E2E:npm run test:e2e,e2e/ 下 15 个 .spec.ts;刻意不进 CI(要拉起 Metro dev server + fixture relay,稳定性未在 runner 上验证),本地跑。
4. opt-in 开关一:RUN_NETWORK_TESTS=1 npx jest price-query —— price-query.test.ts 用 describeNetwork = RUN_NETWORK_TESTS==='1' ? describe : describe.skip 门控;默认关是因为它打真实第三方公共 RPC,失败反映的是网络/限流,不是代码。
5. opt-in 开关二:RUN_ONCHAIN=1 npx playwright test parallel-onchain —— 真实 Gnosis 链上结算冒烟,花真 xDAI,且 fixture Safe 没余额会自跳过。
6. 站点子项目 getvela.app 被根 tsconfig 排除,CI 里单独用 bun run check 检查。

**代码证据**:
- `package.json:16-19` — "test": "jest"、"test:e2e": "playwright test" 等脚本定义
- `.github/workflows/ci.yml:28-35` — CI app job 依次跑 npx tsc --noEmit / npx expo lint / npx jest --ci / npm run build:web
- `.github/workflows/ci.yml:4-9` — 注释明确 Playwright 与 price-query 真实 RPC 套件刻意不进 CI 及原因
- `src/__tests__/price-query.test.ts:159` — const describeNetwork = process.env.RUN_NETWORK_TESTS === '1' ? describe : describe.skip
- `e2e/parallel-onchain.spec.ts:29` — test.skip(!process.env.RUN_ONCHAIN, ...) 链上测试开关
- `docs/project-takeover/02-local-development.md:56-61` — 实测命令表:79 套件/1022 用例、build:web ~11MB
- `jest.config.js:4-7` — testEnvironment node、roots src、testMatch __tests__/**/*.test.ts

**常见错误**:
- 【受训者原错】一条命令都说不出来,或报出通用命令如 npm run test:unit / yarn test 这类本项目不存在的脚本。
- 以为 npm test 会连带跑 E2E,或以为 CI 里有 Playwright。
- 看到 79 套件以为全部会打真实网络——其实唯一的真实 RPC 套件默认 describe.skip。
- 不知道 build:web 也是门禁的一部分(类型过了但 web 导出可能挂)。

**追问**:
1. 为什么 Playwright 被刻意排除在 CI 之外?什么条件下才提升进 CI?
2. RUN_NETWORK_TESTS 套件红了,你的第一判断是代码坏了还是别的?为什么?(答:先怀疑第三方 RPC 可用性/限流,不是代码。)
3. npm run build:web 除了 expo export 还跑了什么(fix-cf-pages-assets.js 是干嘛的)?

**真懂 vs 背诵**:真懂的人能把每条命令对应到 ci.yml 的具体 step 并解释两个 opt-in 开关为什么默认关;背诵的人只会报菜名式地念 "lint、test、build"。

**评分规则**:3 分线:说出 tsc/lint/jest/build:web 四件套且知道 e2e 单独本地跑;5 分线:再准确给出 79 套件/1022 用例与 15 spec 的规模、两个 opt-in 开关名和默认关的理由。

### D8-D9-framework-tests-Q2 · 难度 2/5

**考察目标**:代码定位题。针对基线"不知道 expo-router、不知道 Hermes 踩过的坑":考察入口机制、polyfill 加载顺序为何是硬约束、平台分叉文件的作用。

**题干**:代码定位:这个 app 没有手写的 index.js,真正的入口在哪里定义?根组件是哪个文件?该文件第一行 import 是什么、为什么它必须排在第一行?native 和 web 分别加载哪个 polyfill 文件、各补了什么、不补会挂掉哪两个具体功能?

**标准答案要点**:
1. 入口是 package.json 的 "main": "expo-router/entry" —— expo-router 按 src/app/ 目录文件系统生成路由,根组件是 src/app/_layout.tsx(RootLayout)。
2. _layout.tsx 第 1 行是 import '@/polyfills',注释写明 MUST be first:必须在任何依赖模块被求值之前,把 crypto/btoa/atob/Buffer 装到 Hermes 全局上。
3. Hermes(iOS/Android)缺三组浏览器全局:crypto.getRandomValues(walletpair-sdk + @noble/* 用于 X25519 keygen 和 nonce)、btoa/atob(walletpair join payload 的 base64url)、Buffer(services/image-decode 的相册 QR 解码 fallback)。
4. polyfills.ts 用 react-native-get-random-values 补 crypto,base-64 补 btoa/atob,buffer 包补 Buffer,且都是 typeof 检查后才赋值。
5. web 由 Metro 平台扩展解析自动选 polyfills.web.ts,它是刻意的 no-op(export {}):浏览器本来就有这些全局,同时避免把 native-only 的 react-native-get-random-values 打进 web bundle。
6. 不补的后果:WalletPair(Vela Connect)第一次扫码配对直接抛错、相册导入 QR 静默失败——且两者在 web 构建里完全复现不出来(web-only 测试曾掩盖过这类 native bug)。

**代码证据**:
- `package.json:3` — "main": "expo-router/entry"
- `src/app/_layout.tsx:1` — import '@/polyfills'; // MUST be first: installs crypto/btoa/atob/Buffer on Hermes before any dep loads
- `src/polyfills.ts:8-13` — 逐条列出 walletpair-sdk+@noble→getRandomValues、btoa/atob、Buffer 的依赖方,及"WalletPair 首扫抛错 / 相册 QR 静默失败,web 均不复现"
- `src/polyfills.ts:20` — import 'react-native-get-random-values' 装 crypto.getRandomValues
- `src/polyfills.ts:33-35` — btoa/atob/Buffer 的条件赋值
- `src/polyfills.web.ts:1-7` — web 变体是 no-op,注释解释避免把 native-only 包拉进 web bundle

**常见错误**:
- 【受训者原错】找不到入口,猜 App.tsx / index.js(不知道 expo-router 的文件系统路由)。
- 以为 Hermes 是"类浏览器环境"自带 crypto/btoa——正是这个项目踩过的坑:web 全有、native 全无。
- 以为 import 顺序无所谓、"打包器会处理"——按字母序重排 import 就会把 polyfill 排到依赖之后。
- 以为 web 也需要同一份 polyfill,或以为 polyfills.web.ts 是遗漏没写完的文件。

**追问**:
1. 如果格式化工具把 _layout.tsx 的 import 重排了,最先在哪个平台、哪个功能上暴雷?
2. polyfills.web.ts 为什么不能直接 re-export polyfills.ts?
3. Metro 是靠什么规则决定 web 加载 .web.ts 变体的?metro.config.js 里那段 nodeModulesPaths 又是为谁配的?

**真懂 vs 背诵**:真懂的人能把三组缺失全局各自映射到具体依赖方和具体故障症状,并说清"web 复现不出来"这个陷阱;背诵的人只会泛泛说 "RN 需要 polyfill"。

**评分规则**:3 分线:定位到 expo-router/entry + _layout.tsx 首行 polyfill + Hermes 缺 crypto;5 分线:三组全局、两条真实故障链、web no-op 的打包动机全部答对。

### D8-D9-framework-tests-Q3 · 难度 3/5

**考察目标**:门控辨析题。考察 __DEV__(编译期)与 dev_unlocked(运行期)两种门控的本质区别,以及 ParallelSpaceBadge 为什么必须无条件渲染——这是 2026-07-02 接管审计的真实修复项,背后是 fixture 私钥公开的安全威胁链。

**题干**:项目里同时存在 __DEV__ 和 dev_unlocked 两种"开发者门控"。它们分别在什么阶段生效、各守着哪些东西?dev_unlocked 怎么打开?然后解释一个反直觉的设计:ParallelSpaceBadge 明明是个 dev 组件,为什么在 _layout.tsx 里必须无条件渲染、绝不能包进 __DEV__?

**标准答案要点**:
1. __DEV__ 是编译期常量,release 打包时整段代码被剔除:fault/metrics/parallel 三个 console 只在 __DEV__ 安装(_layout.tsx:135);/parallel 的 Stack.Screen 只在 __DEV__ 注册(_layout.tsx:98);最关键的 __setPasskeyOverride 在非 __DEV__ 下直接 return——mock passkey 在生产是编译期 no-op。
2. dev_unlocked 是运行期 AsyncStorage 标志:About 页 logo 3 秒内连点 6 次写入 '1'(AboutScreen handleLogoTap),生产构建也能开;/parallel 布局在非 __DEV__ 下检查它决定放行还是 Redirect 回 wallet。
3. 因此生产构建可以"处于 parallel 模式":fixture 账户被写进 vela.accounts、flag 置位(enterParallelSpace 还顺手写 dev_unlocked='1'),但 fixture 签名装不上(override no-op)。
4. 威胁链:passkey-fixture.ts 的 SEED 三把 P-256 私钥是故意提交到仓库的公开测试钥。若徽章藏在 __DEV__ 后,生产构建重启后 storage 里是 fixture 账户却没有任何标记——fixture 钱包冒充真钱包,用户可能往私钥人尽皆知的地址存真钱。
5. 修复:ParallelSpaceBadge 无条件渲染、自门控(非激活 return null);applyParallelSpaceOnBoot 也无条件在 boot 时跑,flag 在就重新武装 signer + 亮徽章,真实空间下只是一次 AsyncStorage 读。
6. 徽章读 globalThis.__VELA_PARALLEL__ 而不是模块局部变量:Metro 可能把 parallel-space 模块打包两份(app 代码与 expo-router 路由树各解析一次),模块局部 flag 会让安装方和徽章看到不同的值。

**代码证据**:
- `src/app/_layout.tsx:98` — {__DEV__ && <Stack.Screen name="parallel" />} 路由注册的编译期门控
- `src/app/_layout.tsx:101-105` — 徽章处注释:'Must NOT be behind __DEV__: a production build can still enter the parallel space via dev_unlocked... its keys are public'
- `src/app/_layout.tsx:135` — if (__DEV__) { installFaultConsole(); installMetricsConsole(); installParallelConsole(); }
- `src/app/_layout.tsx:136-143` — applyParallelSpaceOnBoot 在 boot Promise.all 中无条件执行,注释写明 'Runs in prod too'
- `src/modules/passkey/index.ts:135-137` — __setPasskeyOverride: if (!__DEV__) return; 生产编译期 no-op
- `src/screens/settings/AboutScreen.tsx:22-33` — logo 6 连击(3 秒窗口)写入 dev_unlocked='1'
- `src/app/parallel/_layout.tsx:20-28` — __DEV__ 直接 allow,否则读 dev_unlocked 决定 allow/deny
- `src/services/dev/passkey-fixture.ts:45-49` — SEED 三把私钥明文提交;16-19 行注释声明 throwaway 测试钥
- `src/services/dev/parallel-space.ts:154` — enterParallelSpace 的 multiSet 里写入 ['dev_unlocked','1']
- `src/components/dev/ParallelSpaceBadge.tsx:21,36` — 读 globalThis.__VELA_PARALLEL__,非激活 return null;18-21 行注释解释 Metro 双打包问题

**常见错误**:
- 把两种门控混为一谈,"都是 dev 开关"(基线只知道 parallel space 的名字,不知道门控层次)。
- "dev 组件当然该包 __DEV__"——这正是被修复前的 bug 方向,答这个等于复现事故。
- 以为生产构建进了 parallel 就能用 fixture 签名(其实 override 是编译期 no-op,只有账户缓存被换)。
- 以为徽章靠 React context/listener 驱动,不知道 globalThis 单一事实源是为了 Metro 双打包。

**追问**:
1. 怎么验证这些门控在 release 下的真实行为?(答:只有 Release 构建能验——02-local-development.md 明确 "Release 构建是验证 __DEV__ 门控的唯一方式"。)
2. enterParallelSpace 为什么要顺手写 dev_unlocked='1'?不写会怎样?
3. 徽章为什么用 400ms 轮询而不订阅 listener 注册表?

**真懂 vs 背诵**:真懂的人能推出完整威胁链:生产可进 parallel + override no-op + 私钥公开 → 无标记的 fixture 钱包会骗用户存真钱;背诵的人只会说"徽章要一直显示以防混淆"。

**评分规则**:3 分线:分清编译期 vs 运行期两种门控、知道 6 连击入口、知道徽章必须无条件渲染;5 分线:完整讲出威胁链 + override 生产 no-op + globalThis 双打包细节。

### D8-D9-framework-tests-Q4 · 难度 4/5

**考察目标**:故障推演题。针对基线"parallel space 只知道名字":考察核心不变量全文(唯一差异是签名密钥)+进/出的缓存备份恢复协议,以及"缓存丢失 ≠ 密钥丢失"这一关键心智模型。

**题干**:故障推演:你的真机上有真实钱包。你进入 parallel space 测试,测到一半 app 被杀,重启;继续测完后退出。请逐步推演 AsyncStorage 里发生了什么。三个追问:(a) 重启后 app 在真实空间还是 parallel 空间?(b) 如果退出时备份 JSON 解析失败,真实钱包会永久丢失吗?(c) 如果有人把 enterParallelSpace 里的 alreadyIn 检查删了,会造出什么事故?

**标准答案要点**:
1. 核心不变量:parallel space 与真实 app 的唯一差异是签名密钥——固定 P-256 fixture(passkey-fixture.ts 三把公开测试钥)替代设备 passkey;链、bundler、存储、transport、UI 全部是真的,fixture 地址永远不能放真实资金。
2. enter:先 installMockPasskey(置 __VELA_PARALLEL__,徽章亮);读 K_FLAG(vela.parallelSpace)判断是否已在 parallel 中——只在首次进入时把 vela.accounts + vela.activeAccountIndex 备份到 vela.parallelSpace.realWalletBackup;然后 multiSet fixture 账户、activeIndex=0、flag='1'、dev_unlocked='1'。
3. (a) 重启后仍在 parallel:_layout.tsx boot 的 Promise.all 里 applyParallelSpaceOnBoot 读到 flag='1' 就重新 installMockPasskey——fixture 已在存储里,signer 重新武装,徽章重新亮。
4. exit:uninstallMockPasskey;恢复备份(备份里 accounts/idx 为 null 的项走 multiRemove);最后清 K_BACKUP、K_FLAG,以及 parallel 期间创建的连接会话 K_REMOTE_SESSION(vela.remoteInjectSession)和 K_WALLETPAIR_SESSION——防止测试会话泄漏进真实空间。
5. (b) 不会永久丢:JSON 解析失败的 fallback 是 multiRemove 掉 accounts+activeIndex,app 回到无钱包/onboarding 态;但存储里只是账户缓存(地址+公钥),真私钥在设备 passkey(Secure Enclave/Credential Manager)里,enterParallelSpace 的注释原话是 'the true keys live in the device passkey, never here'——重新 Sign In 即可恢复。
6. (c) 删掉 alreadyIn 检查后,二次进入(或 /parallel 布局重复触发 enter)会在存储里已是 fixture 账户时再执行备份——用 fixture 覆盖真实钱包的备份,退出时"恢复"出来的还是 fixture,真实账户缓存被冲掉(资金仍安全,但缓存需重新登录找回)。
7. 补充:live wallet context 不会自动反映 swap——/parallel 布局 enter 后 dispatch SET_WALLET 装入 fixtureAccounts,或整页 reload 也行。

**代码证据**:
- `src/services/dev/parallel-space.ts:4-22` — 模块头注释:唯一差异是 passkey 签名,Boundary 清单,全部 __DEV__ 门控
- `src/services/dev/parallel-space.ts:141-147` — alreadyIn = flag==='1' 检查,仅首次进入备份 accounts+idx 到 K_BACKUP
- `src/services/dev/parallel-space.ts:150-155` — multiSet fixture 账户/activeIndex/flag/dev_unlocked
- `src/services/dev/parallel-space.ts:165-183` — exit:恢复备份、JSON 损坏 catch 里 multiRemove、清 K_BACKUP/K_FLAG/K_REMOTE_SESSION/K_WALLETPAIR_SESSION
- `src/services/dev/parallel-space.ts:192-198` — applyParallelSpaceOnBoot:flag='1' 则重新 installMockPasskey
- `src/app/_layout.tsx:143` — boot Promise.all 首项调用 applyParallelSpaceOnBoot(在钱包挂载前)
- `src/services/dev/parallel-space.ts:129-137` — enter 的 docstring:'the true keys live in the device passkey, never here, so this swap is safe and fully reversible'
- `src/app/parallel/_layout.tsx:33-39` — enterParallelSpace 后 dispatch({type:'SET_WALLET', accounts: fixtureAccounts(), activeIndex: 0})

**常见错误**:
- "备份坏了真实钱包就没了"——混淆账户缓存与签名密钥;这个架构里 AsyncStorage 从不保存真私钥。
- "重启就自动回到真实钱包了"——flag + boot re-arm 恰恰保证留在 parallel(否则会出现无徽章的 fixture 钱包)。
- "每次 enter 都重新备份更保险"——恰恰相反,二次备份会用 fixture 覆盖真备份,alreadyIn 就是防这个。
- 【受训者原错】只答得出"parallel space 是测试环境"一句话,说不出不变量全文和备份恢复协议。

**追问**:
1. exit 为什么必须清 walletpair/remoteInject 两个 session key?不清会在真实空间里看到什么?
2. 为什么备份/恢复的 key 刻意与 storage.ts 的 vela.accounts 完全一致?这让哪条签名解析路径对 fixture 零改动可用?
3. /parallel 布局的 enter effect 为什么"刻意不在 unmount 时取消"?

**真懂 vs 背诵**:真懂的人张口就是"唯一差异是签名密钥"并能推演 alreadyIn 删除后的备份覆盖事故;背诵的人只会说"它会备份再恢复",答不出损坏 fallback 的语义和资金安全边界。

**评分规则**:3 分线:讲清 enter 备份→exit 恢复→boot re-arm 的闭环 + 不变量一句话;5 分线:三个追问全对——留在 parallel、缓存丢≠钥匙丢、alreadyIn 防备份覆盖,并知道要清的 session keys。

### D8-D9-framework-tests-Q5 · 难度 4/5

**考察目标**:修改影响题。直接打击基线"改 safe-transaction.ts 只会答跑单测":考察单测的明确覆盖边界、完整门禁、parallel space 实测路径、RUN_ONCHAIN 链上冒烟的双账户充值模型。

**题干**:你要修改 src/services/safe-transaction.ts 的 gas 计算逻辑(比如 calcMaxFeePerGas 或 deriveChainGasPrice)。"跑单测"为什么远远不够?请给出从改动到有信心合入的完整验证路径,并说明:要跑通真实链上验证,需要给哪两个不同的账户充值、各覆盖什么?

**标准答案要点**:
1. 单测有明确的覆盖边界:src/__tests__/services/safe-transaction.test.ts 文件头写明只测纯函数(calldata 构造、hash、格式化、quote-abuse 判定的 golden vector),'RPC-dependent functions are not tested here'——gas 逻辑恰恰大量走 RPC/bundler,单测绿不代表行为对。
2. 第一层:本地门禁四件套 npx tsc --noEmit / npm run lint / npx jest / npm run build:web(与 ci.yml 同款)。
3. 第二层:parallel space 实测——dev 下进 /parallel 用 fixture 钱包走真实 Send/dApp 流程;注意 parallel-send.spec.ts 是 hermetic 的,只覆盖到 token picker 入口、刻意不落链,所以它测不到 gas 路径。
4. 第三层:RUN_ONCHAIN=1 npx playwright test parallel-onchain —— 唯一证明全栈协同的测试:fixture passkey 签真实 ERC-4337 UserOp → 真 vela-bundler 提交 → RIP-7212 P256 预编译 + EntryPoint + Safe4337Module 在 Gnosis(chain 100)真结算,dApp 拿到真 tx hash。
5. 双账户充值模型:① fixture Safe「Parallel One」(0xD400866e00B055B20752a826CD5C89b811de130b)要有少量 xDAI 覆盖转账金额;② vela-bundler 的 per-Safe gas 存款地址(bundlerGasAccount 查询)要另充——bundler 从存款地址付 gas,不是从 Safe 余额;两者任一不足测试都会自跳过。
6. gas 改动的额外雷区:gas 报价与 vela-bundler 跨仓耦合(bundler 是价格权威,wallet 侧不能否决其报价),改这一带要连同 bundler 的行为一起验证,不能只看 wallet 单侧。
7. 降级路径验证:用故障注入 vela.slowRpc / vela.failRpc 检查 gas 估算失败/超时时的 UI 表现。

**代码证据**:
- `src/__tests__/services/safe-transaction.test.ts:1-6` — 文件头:'Tests the pure functions... RPC-dependent functions are not tested here (require network mocking)'
- `src/__tests__/services/safe-transaction.test.ts:14-20` — 被测导出清单:calcMaxFeePerGas、deriveChainGasPrice、isQuoteAbusive、buildExecuteCallData 等
- `.github/workflows/ci.yml:28-35` — 门禁四件套的 CI 版本
- `e2e/parallel-send.spec.ts:3-11` — 注释:只测 Send 入口,'no funds move and no UserOp is sent',深流程是 backlog
- `e2e/parallel-onchain.spec.ts:2-13` — '@onchain' 真实 Gnosis 结算说明 + RUN_ONCHAIN=1 + 先给 Parallel One 充 xDAI
- `e2e/parallel-onchain.spec.ts:42-49` — bundler 从 per-Safe deposit address 付 gas(不是 Safe 余额),gas 账户不足则 skip 并打印充值地址
- `e2e/support/parallel.ts:131-140` — bundlerGasAccount():查询 vela-bundler.getvela.app 的 depositAddress 与 spendableWei

**常见错误**:
- 【受训者原错】"跑单测就行"——没意识到该测试文件自我声明不覆盖 RPC 路径,gas 改动的主要风险面根本不在单测里。
- 不知道存在 RUN_ONCHAIN 链上冒烟,或以为普通 e2e(parallel-send)已经覆盖了发送落链。
- 只给 Safe 充值就去跑 onchain 测试,不知道 bundler gas 存款地址是独立的第二个要充值的账户。
- 忽略跨仓耦合,只在 wallet 仓里自测 gas 逻辑。

**追问**:
1. parallel-send.spec.ts 为什么刻意止步于 token picker?把深流程写进默认 e2e 会引入什么不稳定因素?
2. bundler gas account 与 Safe 自身余额各付什么钱?测试 skip 信息里给的两个地址分别是谁的?
3. 如果 e2e 里 gas 估算一直 pending,你会按什么顺序排查(wallet RPC 层 / bundler 报价 / 故障注入残留)?

**真懂 vs 背诵**:真懂的人先指出单测文件自我声明的覆盖边界,再给出分层验证直到真链结算,并主动提双账户充值;背诵的人在"跑单测+跑 e2e"就停了。

**评分规则**:3 分线:说出单测只覆盖纯函数 + 四件套 + 要进 parallel space 实测;5 分线:完整给出 RUN_ONCHAIN 冒烟链路、双账户充值模型、以及 bundler 报价跨仓耦合意识。

### D8-D9-framework-tests-Q6 · 难度 5/5

**考察目标**:harness 深水故障推演。考察故障注入 vela.* 的时序设计:__VELA_FAULT_INIT__ 预埋 seam 为什么必须在模块加载期生效、e2e 为什么依赖 DEV build、限流 vs 硬失败两条分类路径及断言 seam 的取舍。

**题干**:parallel-rate-limit.spec.ts 要证明"限流是冷静的、瞬态的降级:保留缓存余额、不弹 RPC 横幅"。推演:这个测试如何保证 app 的第一次余额加载就已经处于全链限流之下?为什么不能在页面加载完成后再在控制台执行 vela.rateLimitRpc('all')?整条链路上有哪几个专门为自动化留的 seam,各解决什么问题?如果有人把 fault-injection.ts 底部那个 IIFE 挪进 installFaultConsole 里,测试会怎么死?

**标准答案要点**:
1. 时序核心:Playwright 用 page.addInitScript 在任何 app 代码执行前把 window.__VELA_FAULT_INIT__ = [['rateLimitRpc','all']] 写进页面;fault-injection.ts 模块底部的 IIFE applyPreArmedFaults 在模块被加载的那一刻读取它并直接改 faults + recompute——注释原话:'Applied at MODULE LOAD — earlier than any React effect',所以第一次余额加载天然带故障。
2. 页面加载后再调 vela.rateLimitRpc 有竞态:首轮余额刷新很可能已经用真实网络跑完,测到的是"刷新后限流"而不是"首载即限流",退化成 refresh-timing games。
3. DEV build 依赖:vela.* 控制台只在 __DEV__ 下由 _layout.tsx 安装;playwright.config.ts 注释明确 parallel space 需要 DEV build——fixed-passkey override 在生产是编译期 no-op。__VELA_FAULT_INIT__ 本身不被 __DEV__ 包裹,但只在该全局被显式设置时才生效(注释:never in prod use)。
4. 语义区分是被测标的:rpcShouldRateLimit 与 rpcShouldFail 是两条独立分类——限流=瞬态(缓存余额、无 'RPC unavailable' 横幅),硬失败=持久(会走横幅路径);spec 两条用例分别断言 rateLimited.length === failed.length 和 === 0。
5. 断言 seam:测试读 dev-only 的 window.__velaRpcState(rpc-pool 的实时分类集合,正是 HomeScreen 过滤横幅用的信号),而不是等 UI 横幅——spec 注释说明:完整 token fetch 的 ethereum-data metadata gate 在沙箱里不可达,读分类信号更稳;单测覆盖在 rpc-pool-ratelimit.test.ts。
6. 辅助手段:route 把非 localhost 的请求全部 abort,让失败快速且确定;faults 的 hooks 走单个 active 布尔短路,热路径零成本。
7. 把 IIFE 挪进 installFaultConsole 的后果:installFaultConsole 在 React useEffect 里才被调用(_layout.tsx:135),晚于首轮模块加载与首次数据请求——预埋故障生效太迟,"首载即限流"的断言变成 flaky/假绿,测试失去其存在意义。

**代码证据**:
- `e2e/parallel-rate-limit.spec.ts:32-38` — bootParallel:addInitScript 写 __VELA_FAULT_INIT__ + route abort 非 localhost
- `src/services/dev/fault-injection.ts:201-207` — automation seam 注释:Playwright addInitScript、'Applied at MODULE LOAD — earlier than any React effect'、no-op unless set
- `src/services/dev/fault-injection.ts:208-220` — applyPreArmedFaults IIFE:解析 [method,arg] 步骤直接改 faults 并 recompute
- `src/app/_layout.tsx:135` — installFaultConsole 只在 __DEV__、且在 useEffect 中安装(晚于模块加载)
- `playwright.config.ts:4-6` — 注释:parallel space 需要 DEV build,fixed-passkey override 在生产是 compile-time no-op
- `src/services/dev/fault-injection.ts:73-79` — rpcShouldRateLimit 与 rpcShouldFail 的语义区分(transient/self-healing vs 持久失败)
- `e2e/parallel-rate-limit.spec.ts:10-13` — 注释:用 __velaRpcState seam 断言而非驱动横幅,metadata gate 在沙箱不可达;单测在 rpc-pool-ratelimit.test.ts
- `e2e/parallel-rate-limit.spec.ts:56-71` — 两条用例:rateLimited===failed(冷静无横幅)与 rateLimited===0(硬失败持久路径)
- `src/services/dev/fault-injection.ts:46-59` — active 快路径布尔,recompute 在每次变更时重算

**常见错误**:
- "goto 之后 evaluate 一句 vela.rateLimitRpc 就行"——首轮加载已经跑完,恰恰是这个 seam 要消灭的竞态。
- 【受训者原错】根本不知道 vela.* 控制台和 __VELA_FAULT_INIT__ 的存在,或以为它是生产功能/安全隐患而不知道其双重保护(global 未设置即 no-op + console 仅 __DEV__)。
- 分不清 failRpc 和 rateLimitRpc——以为都是 "RPC 挂了",答不出瞬态(缓存+无横幅)vs 持久(横幅)两条 UI 路径。
- 以为断言应该等横幅 UI 出现/消失,不理解为什么读 __velaRpcState 分类信号更稳。

**追问**:
1. 为什么这个 spec 把非 localhost 请求 abort 而 support/parallel.ts 的 stubWalletNetwork 却选择返回 200 空 JSON?两种策略各适合什么测试?
2. vela.rateLimitRpc 与 getRateLimitedChains 到 HomeScreen 的耦合链是什么?清掉故障后 UI 怎么自愈?
3. 如果要新增一种故障(比如 bundler 报价超时),你会在 fault-injection.ts 的哪几处各加什么?

**真懂 vs 背诵**:真懂的人用"模块加载 vs React effect"的时序差解释 IIFE 的位置是设计而非巧合,并能推演挪动后的 flaky 死法;背诵的人只会说"测试前先注入故障"。

**评分规则**:3 分线:讲清 addInitScript→__VELA_FAULT_INIT__→模块加载期生效的时序理由 + DEV build 依赖;5 分线:再答出限流/硬失败双路径断言、__velaRpcState seam 的取舍原因、以及 IIFE 挪位后的具体失效模式。

---

## D11-D12-ops-external — 部署运维与外部依赖

### D11-D12-ops-external-Q1 · 难度 1/5

**考察目标**:能独立说出 Web 钱包从代码到 wallet.getvela.app 上线的完整命令链、fix-cf-pages-assets 存在的原因、build-info 自动生成机制、以及 CF Pages 回滚方式(直接针对基线错误:构建部署命令说不出)。

**题干**:现在要把 Web 钱包发一版到 wallet.getvela.app。请说出:(a) 完整的构建命令及它内部实际执行了什么;(b) 构建链里那个"修资产"的脚本在防什么事故,跳过它用户会看到什么;(c) src/constants/build-info.ts 里的 commit hash 是谁写进去的,能不能手改;(d) 发出去发现坏了,怎么回滚?

**标准答案要点**:
1. 构建命令是 npm run build:web,它实际执行 npx expo export --platform web && node ./scripts/fix-cf-pages-assets.js;之后把 dist/ 部署到 Cloudflare Pages(wrangler pages deploy dist 或控制台上传)。
2. npm 的 pre 钩子 prebuild:web 会先跑 prebuild-info(scripts/generate-build-info.js),用 git rev-parse --short HEAD 生成 src/constants/build-info.ts;文件头写明 do not edit,手改会在下次构建被覆盖,且 bug-report 的 environment 行依赖它标注版本。
3. fix-cf-pages-assets.js 防的是:CF Pages 在 wrangler pages deploy 时会丢弃任何名为 node_modules 的目录,而 expo 把 Plus Jakarta 字体输出在 dist/assets/node_modules/ 下;跳过则字体请求回落到 index.html,浏览器报 OTS parsing error,useFonts() 永久挂起,应用白屏转圈。
4. 该脚本把 assets/node_modules 移到 assets/vendor 并重写 .js/.html/.css/.json 中的所有引用,最后自检仍有残留引用就 exit 1 使构建失败——所以它是构建必经步骤不是可选优化。
5. 上线后 smoke test:首屏无控制台报错、余额/Receive/Activity 正常、passkey 弹窗 rpId=getvela.app、且确认没有紫色 PARALLEL SPACE 徽章(出现=发了 fixture 空间)。
6. 回滚:CF Pages 控制台一键回滚到上一个 deployment,静态产物无状态,秒级完成;这与 getvela.app Worker 的回滚(wrangler rollback)是两个不同的部署单元。

**代码证据**:
- `package.json:13-14` — prebuild:web 钩子接 prebuild-info;build:web = expo export + fix-cf-pages-assets.js
- `package.json:6` — prebuild-info 脚本指向 scripts/generate-build-info.js
- `scripts/generate-build-info.js:10-17` — git rev-parse --short HEAD 写入 build-info.ts,带 'do not edit' 头
- `src/constants/build-info.ts:1-3` — 生成物实体:APP_VERSION + GIT_COMMIT
- `scripts/fix-cf-pages-assets.js:3-13` — 头注释:CF Pages 丢 node_modules 目录 → 字体变 HTML → OTS parsing error → useFonts 挂起
- `scripts/fix-cf-pages-assets.js:66-78` — 部署前自检,残留引用则 fail(exit 1)
- `docs/project-takeover/05-deployment-runbook.md:36-43` — Web 发布四步:build(勿跳过 fix 脚本)→ 部署 → smoke(含无紫徽章)→ CF Pages 一键回滚

**常见错误**:
- 【受训者原错】完全说不出命令,只能说"AI 之前都是自动弄的"。
- 以为 git push 到 main 就自动部署了(本项目 CI 只是门禁,部署全手动,ci.yml 头注释和 05 手册都写明)。
- 把 fix-cf-pages-assets 当成可跳过的优化脚本,不知道跳过=生产白屏。
- 以为 build-info.ts 是手工维护的版本文件,发版前去手改它。

**追问**:
1. 如果 smoke test 时看到紫色 PARALLEL SPACE 徽章,说明发布环节哪里出了问题?对用户有什么风险?
2. expo 某次升级后字体不再输出到 assets/node_modules 了,fix 脚本会怎么表现?(答:打印 'no assets/node_modules dir' 后照常自检,幂等设计。)
3. 为什么 Web 钱包(CF Pages)和官网 API(CF Worker)的回滚手段不一样?

**真懂 vs 背诵**:真懂的人能讲出 fix 脚本防的具体事故链(node_modules 被丢→字体 404 变 HTML→useFonts 挂起)并知道它失败会中断构建;背诵的人只会念"运行 npm run build:web 然后部署"。

**评分规则**:3 分线=说出 build:web 完整命令链+CF Pages 一键回滚;5 分线=额外讲清 fix 脚本的事故机理、prebuild 钩子生成 build-info 不可手改、smoke 项含无紫徽章。

### D11-D12-ops-external-Q2 · 难度 2/5

**考察目标**:确认知道仓库存在 CI、其两个 job 各查什么、哪些检查被故意排除及理由、以及"CI 从未被 push 验证过"这一未决状态(直接针对基线错误:不知道仓库有 CI)。

**题干**:这个仓库有没有 CI?如果有:什么事件触发、有几个 job、每个 job 按顺序跑哪些检查?有哪两类测试被故意排除在 CI 外,理由分别是什么?这套 CI 现在最大的"未知数"是什么,验收标准是什么?

**标准答案要点**:
1. 有:.github/workflows/ci.yml,push 到 main 和所有 pull_request 触发;是本次接管新增的门禁,不做任何部署。
2. app job(Node 22 + npm ci):npx tsc --noEmit → npx expo lint(--max-warnings=10000)→ npx jest --ci → npm run build:web 四道关。
3. site job:working-directory 为 getvela.app,bun install --frozen-lockfile → bun run check(svelte-check),因为官网是独立的 bun/SvelteKit 项目。
4. 故意排除一:Playwright E2E——需要起 Metro dev server + fixture relay,等 runner 上有稳定记录再提升进 CI(本地 npm run test:e2e 跑)。
5. 故意排除二:price-query 真实 RPC 套件——RUN_NETWORK_TESTS=1 手动开启,因为它依赖第三方公共 RPC 可用性而非我们的代码。
6. 最大未知数是 08 号未决事项 C1:ci.yml 已提交但从未经真实 push 验证(审计只在本地等价执行了全部步骤);验收=push 后 app+site 两个 job 全绿,之后才开 branch protection。

**代码证据**:
- `.github/workflows/ci.yml:13-16` — 触发条件:push main + pull_request
- `.github/workflows/ci.yml:27-35` — app job 四步:npm ci、tsc --noEmit、expo lint、jest --ci、build:web
- `.github/workflows/ci.yml:37-47` — site job:getvela.app 目录下 bun install --frozen-lockfile + bun run check
- `.github/workflows/ci.yml:4-10` — 头注释明确列出 deliberately NOT included:Playwright E2E、price-query 真实 RPC 套件及各自理由
- `docs/project-takeover/08-open-issues.md:46-48` — C1:CI 未经 push 验证,验收=两 job 全绿后开 branch protection
- `docs/project-takeover/08-open-issues.md:50-52` — C2:E2E 进 CI 的条件是连续 10 次无 flake
- `docs/project-takeover/05-deployment-runbook.md:3` — 全手动部署,CI 仅门禁不接部署

**常见错误**:
- 【受训者原错】"这个仓库没有 CI"(ci.yml 就在 .github/workflows/ 且是最近的提交之一)。
- 以为 CI 绿了就等于自动发布了新版本(它不部署任何东西)。
- 以为 E2E 在 CI 里跑,坏了 CI 会拦住(实际 E2E 只在本地)。
- 不知道 CI 本身从未被验证过,直接开 branch protection 可能把自己锁死在一个坏配置上。

**追问**:
1. 为什么 lint 步骤要加 --max-warnings=10000 这种看似放水的参数?它实际把关的是什么?(答:把关 errors 而非 warnings。)
2. 如果明天你 push 后 site job 红了但 app job 绿了,最可能是哪个目录的什么检查挂了?
3. 把 E2E 提进 CI 之前,08 手册定的量化标准是什么?(答:连续 10 次无 flake。)

**真懂 vs 背诵**:真懂的人能说出"哪些故意不在 CI 里+为什么+C1 未验证"这三层;背诵的人只会背 "有 typecheck lint test build" 四个词。

**评分规则**:3 分线=知道 ci.yml 存在、触发条件和两 job 大致内容;5 分线=能完整说出排除项的理由和 C1 未验证状态及其验收标准。

### D11-D12-ops-external-Q3 · 难度 3/5

**考察目标**:彻底纠正"服务端都是公开的不需要密钥"这一最危险基线错误:建立密钥全景清单——每枚密钥是什么、存哪、怎么进生产、泄漏/丢失的后果。

**题干**:有人(可能是过去的你)说:"我们钱包是非托管的,服务端都是转发公开链上数据,所以没什么密钥要管。"请逐项反驳:这套系统实际存在哪些密钥/凭据?每一枚存放在哪里、通过什么命令进入生产、本地开发时放哪、泄漏或丢失分别是什么后果?

**标准答案要点**:
1. getvela.app Worker 有三枚 secret:ALCHEMY_API_KEY、PIMLICO_API_KEY、GITHUB_BUG_TOKEN;生产通过 wrangler secret put 写入,本地放 getvela.app/.dev.vars(已 gitignore、从未入库)。
2. ALCHEMY/PIMLICO key 被 bundler 代理直接拼进上游 URL(api.pimlico.io/v2/{chainId}/rpc?apikey=… 和 {slug}.g.alchemy.com/v2/{key}),这正是钱包 App 不直连提供商、必须走 /api/bundler 代理的原因——key 泄漏=配额被烧/被封,钱包估算与发送直接受损。
3. GITHUB_BUG_TOKEN 是 fine-grained PAT、仅 issues 权限,永不到达客户端;未配置时 bug-report 路由返回 503 {error:'not_configured'},客户端自动降级为预填 GitHub URL——这是唯一可"无密钥运行"的路由,且是刻意设计。
4. vela-bundler(独立仓库)持有 gas account EOA 私钥——这是唯一直接控制真金白银的密钥,泄漏=资金被直接盗走,严重度与前三枚不同量级。
5. 未来的 Android upload keystore:绝不入库(gitignore 已覆盖 *.jks 和 keystore.properties),存本地+密码管理器;丢失可走 Play 重置流程(因为开了 Play App Signing);仓库只有 keystore.properties.example。
6. p256-index 服务端签名 key 在独立仓库/CF,本仓库不含。
7. 轮换流程(以 API key 为例):提供商控制台生成新 key → wrangler secret put → 验证 → 废旧。
8. "非托管所以没密钥"错在混淆两件事:用户资金确实不依赖任何服务端密钥(passkey+链上 Safe),但服务可用性和运营账户(bundler EOA、API 配额、issue 通道)全都依赖密钥。

**代码证据**:
- `docs/project-takeover/06-operations-runbook.md:48-56` — 密钥与凭据清单表:三枚 Worker secret、Android keystore、Apple 证书、p256-index key,含位置与轮换列
- `docs/project-takeover/05-deployment-runbook.md:48` — 生产密钥命令:wrangler secret put ALCHEMY_API_KEY / PIMLICO_API_KEY / GITHUB_BUG_TOKEN
- `getvela.app/src/routes/api/bundler/+server.ts:14-16` — 从 env 读 ALCHEMY_API_KEY / PIMLICO_API_KEY / BUNDLER_PROVIDER
- `getvela.app/src/routes/api/bundler/+server.ts:55-58` — Pimlico URL 直接拼 ?apikey=,key 缺失时 buildUrl 返回 null
- `getvela.app/src/routes/api/bug-report/+server.ts:5-11` — 注释:PAT 永不到客户端,只存于 CF secret;未配置返回 503 not_configured
- `getvela.app/src/routes/api/bug-report/+server.ts:78-81` — 无 token 时的 503 分支实现
- `src/services/bug-report.ts:142-148` — 客户端拿到 503 not_configured 后静默走 fallbackUrl
- `docs/project-takeover/14-human-progress.md:9` — 全系统凭据盘点明文:getvela.app 3 枚 secret + bundler EOA 私钥(bundler EOA 属独立仓库)
- `docs/project-takeover/06-operations-runbook.md:60` — 灾难恢复:用户资金非托管,服务全灭不影响资金所有权(反驳的"半真"部分)

**常见错误**:
- 【受训者原错】"服务端都是公开的不需要密钥"(本域最危险的一条——它会导致把 .dev.vars 提交入库或把 key 写进客户端代码)。
- 以为 GITHUB_BUG_TOKEN 在 App 里、客户端直连 GitHub(实际 token 只在 Worker,客户端只知道 https://getvela.app/api/bug-report)。
- 把 bundler EOA 私钥和 API key 混为一谈,意识不到前者泄漏=直接资金损失。
- 以为 keystore 丢了 App 就永久发不了新版(有 Play App Signing 可重置 upload key)。

**追问**:
1. 如果 Alchemy key 泄漏被人刷爆,钱包用户第一时间会在哪些功能上感知到?轮换的完整四步是什么?
2. 为什么 bug-report 路由能优雅地在"无密钥"状态下运行,而 bundler 路由不能?这个设计差异说明什么?
3. 三枚 secret 里哪一枚的爆炸半径最小?为什么(权限范围角度)?(答:GITHUB_BUG_TOKEN——fine-grained PAT 仅 issues 权限。)

**真懂 vs 背诵**:真懂的人能按爆炸半径给密钥排序(bundler EOA 私钥 > API keys > 仅 issues 权限的 PAT)并说出每枚的降级行为;背诵的人只能报出三个名字。

**评分规则**:3 分线=说出三枚 Worker secret + wrangler secret put/.dev.vars 两个存放位置 + bundler EOA 私钥在独立仓库;5 分线=完整覆盖 keystore/轮换流程/503 降级路径,并能准确说出"非托管"论断哪半对哪半错。

### D11-D12-ops-external-Q4 · 难度 4/5

**考察目标**:代码定位题:高频故障"gas 账户余额不足→充值 modal"的检测逻辑在哪、靠什么机制识别、为什么与 vela-bundler 仓库存在脆弱的文案耦合、防线是什么(针对基线错误:不知道 underfunded 弹窗这一高频故障模式)。

**题干**:用户发交易时 bundler 报"gas 账户余额不足",钱包应该弹出充值 modal 而不是甩原始报错。(a) 这个错误检测逻辑在哪个文件哪个函数?(b) 它靠什么机制判断"这是 underfunded 错误"并提取充值信息?(c) 哪两条 UI 路径消费它?(d) 为什么另一个仓库里改一句错误文案能静默弄坏这里?现有防线有哪几道?

**标准答案要点**:
1. 位置:src/services/bundler-service.ts 的 parseBundlerUnderfunded(第 367 行),返回 BundlerUnderfunded | null。
2. 机制:对错误消息做字符串/正则匹配——命中 /dedicated bundler (gas account|EOA)/i,或同时命中 'Deposit to: 0x' 与 'required:';兼容 bundler 历史上的两代文案(legacy 'dedicated bundler EOA' 和现行 'Insufficient native balance…Deposit to: 0x…')。
3. 再从消息里正则提取 Spendable(spendableWei)、required(requiredWei)、Deposit to 后的 40 位 hex 地址;asset 字段按消息含 pathUSD 判定为 'pathUSD'(Tempo)否则 'native'——这样即使后续账户查询失败也能直接开充值 modal。
4. 两条消费路径:SendScreen.tsx:984(Send 流程)和 models/dapp-connection.tsx:593(dApp 签名流程),两处都在收到发送错误后调它决定是否弹 funding modal。
5. 耦合原因:错误来自独立仓库 vela-bundler 的 handlers.ts 文案,钱包侧靠字符串匹配识别;对方改措辞不会产生任何编译/类型错误,只会让 parse 返回 null,用户看到原始报错——静默降级。
6. 防线一:匹配设计上抓"稳定信号"(dedicated bundler / Deposit to+required)而非整句精确匹配,小改动能扛住;防线二:bundler-service.test.ts 里 parseBundlerUnderfunded 的回归测试固定了两代文案样本;防线三:发布 checklist 明文规定"若改过 bundler 错误文案或 parseBundlerUnderfunded,与 vela-bundler 仓库联合验证"。
7. 另有主动路径:checkBundlerFunding(bundler-service.ts:118)在发送前查 spendableBalance ≥ threshold,不足则用 recommendedFundingWei(第 106 行,含 FUNDING_BUFFER_BPS 缓冲)先弹 modal——parseBundlerUnderfunded 是这条主动检查漏掉时的兜底。

**代码证据**:
- `src/services/bundler-service.ts:367-372` — parseBundlerUnderfunded 函数与两组匹配正则
- `src/services/bundler-service.ts:354-366` — 文档注释:bundler 两代文案、'match on the stable signal rather than one exact phrase'
- `src/services/bundler-service.ts:374-384` — 提取 Spendable/required/Deposit 地址与 pathUSD 判定
- `src/screens/wallet/SendScreen.tsx:984` — Send 路径消费:parseBundlerUnderfunded(error?.message)
- `src/models/dapp-connection.tsx:593` — dApp 路径消费:parseBundlerUnderfunded(msg)
- `src/__tests__/services/bundler-service.test.ts:54-91` — 两代文案 + 负例的回归测试
- `src/services/bundler-service.ts:106-110,118-140` — recommendedFundingWei 与 checkBundlerFunding 主动检查
- `docs/project-takeover/05-deployment-runbook.md:30` — 发布 checklist:改文案必须与 vela-bundler 联合验证
- `docs/project-takeover/06-operations-runbook.md:26-27` — 排障手册"充值 modal 不弹/弹错"条目直指两仓文案同步

**常见错误**:
- 以为错误识别走结构化错误码/类型化 API(实际是裸字符串正则,这正是耦合脆弱的根源)。
- 【受训者原错】根本不知道存在"underfunded→充值 modal"这个高频故障模式,排障时只会说通用套话。
- 以为只有 Send 一条路径,漏掉 dapp-connection 的第二个消费点——修文案匹配只验 Send 就上线。
- 以为两仓库部署耦合(实际部署独立,耦合的是语义/文案)。

**追问**:
1. vela-bundler 明天把文案改成 'gas balance too low, top up 0x…',parse 会命中吗?逐个正则走一遍。(答:不命中——无 'dedicated bundler'、无 'Deposit to: 0x'、无 'required:',返回 null,用户看原始报错。)
2. 为什么 asset 要区分 pathUSD 和 native?哪条链的 gas 不是原生币?(答:Tempo/4217,TIP-20 稳定币计价。)
3. 如果你要把这个字符串耦合升级成结构化错误码,两个仓库各要改什么,过渡期怎么兼容旧版本 App?

**真懂 vs 背诵**:真懂的人能空手复述两组匹配正则的"稳定信号"思路并数出两个消费点+三道防线;背诵的人只会说"有个函数解析错误消息"。

**评分规则**:3 分线=定位到 bundler-service.ts:367 且说清字符串匹配机制和跨仓库文案耦合;5 分线=完整给出两代文案兼容、两个 UI 消费点、回归测试+发布 checklist 防线,以及 checkBundlerFunding 主动检查与兜底的关系。

### D11-D12-ops-external-Q5 · 难度 4/5

**考察目标**:故障推演题:半夜收到"发不出交易",能给出项目特异的诊断序列(按故障影响矩阵分层定位:underfunded → Gnosis gas price → getvela.app 代理 → vela-bundler → RPC),并知道每层的具体工具和命令(针对基线错误:排障序列无项目特异性)。

**题干**:凌晨两点,一键 bug report 进来一条 issue:"发不出交易,一直失败"。请给出你的诊断序列——每一步查什么、用什么工具/命令、什么现象指向什么结论。禁止"重启试试/看看日志"这类通用答案。追加:如果 getvela.app Worker 此刻整个挂了,钱包哪些功能死、哪些活?

**标准答案要点**:
1. 第 0 步读 issue 本身:一键 bug report 自带脱敏 environment(App 版本+commit、平台、RPC unreachable 链列表)和 diagnostics(metrics 的 recentFailures,含 service/outcome/status)——先看最近失败的是哪个 service、什么状态码,直接缩小范围。
2. 第一嫌疑(高频):bundler gas account underfunded——正常应弹充值 modal;若用户看到的是原始报错文案,查 parseBundlerUnderfunded 与 vela-bundler 当前文案是否还匹配(两仓文案同步问题,06 手册专门条目)。
3. 第二嫌疑(历史惨案区):Gnosis 'gas price too low'/费率显示 '—'——原则是 bundler 报价是权威、钱包 RPC 永不否决;查 getBundlerGasQuote(safe-transaction.ts:1470)与 bundler 侧 pimlico_getUserOperationGasPrice;回归测试在 bundler-service.test.ts。
4. 第三层:getvela.app /api/bundler 代理——wrangler tail 看 Worker 实时日志;若是刚发过版,wrangler rollback;修复后 smoke=在钱包里做一次小额估算(不需提交)确认代理通。
5. 第四层:vela-bundler 服务本身(独立仓库/宿主)——App 侧已有 3 重试+existingHash 恢复、大 calldata 直接拒绝;gas 报价与错误文案都以它为权威。
6. 第五层:链 RPC——单链 RPC 挂只影响余额刷新(缓存兜底+多端点转移+封禁,rpc-pool);429 只静默用缓存;发送路径走 bundler 不走这条,所以"余额正常但发不出"反而排除 RPC。
7. 复现工具:开发环境浏览器控制台 vela.failRpc(1)/vela.rateLimitRpc('all')/vela.slowRpc(3000)/vela.flakyRpc(0.5)/vela.nullPrice('all'),E2E 种子 __VELA_FAULT_INIT__。
8. 追加题答案——getvela.app Worker 全挂:死=bundler 代理(内置 bundler 用户估算/发送失败但有明确报错、估算失败拒绝提交)、汇率、NFT、transactions、bug-report 后端(自动降级 GitHub URL);活=资金(非托管)、余额(RPC pool 直连+缓存)、passkey 登录、Web 钱包本体(CF Pages 是独立部署单元);临时缓解=App 设置里 vela.serviceEndpoints 可覆盖服务端点。
9. 注意告警现状:客户端无遥测、CF 无告警配置——半夜你能收到的只有用户 bug report,这本身是 06 手册记录的短板(建议对 /api/bundler 5xx 率配告警)。

**代码证据**:
- `docs/project-takeover/06-operations-runbook.md:23-27` — 排障条目:gas price too low(bundler 是权威)与充值 modal 不弹(两仓文案同步)
- `docs/project-takeover/06-operations-runbook.md:15-16` — 故障影响矩阵:vela-bundler 行(3 重试+existingHash)与 getvela.app/api 行(wrangler tail / wrangler rollback / vela.serviceEndpoints 覆盖)
- `docs/project-takeover/06-operations-runbook.md:44-46` — 模拟故障命令全家桶 vela.* 与 __VELA_FAULT_INIT__
- `docs/project-takeover/06-operations-runbook.md:5-7` — 可观测性现状:无遥测无告警,诊断靠 bug report + 控制台日志
- `src/services/bug-report.ts:53-75` — environment 行(版本/平台/RPC unreachable)与 diagnostics(recentFailures 的 service/outcome/status)构成 issue 内容
- `src/services/safe-transaction.ts:1470` — getBundlerGasQuote 定义(bundler 报价权威的落点;06 手册写 1464 已微过期)
- `src/services/bundler-service.ts:367` — parseBundlerUnderfunded(第一嫌疑的检测点)
- `docs/project-takeover/05-deployment-runbook.md:51` — API 发布后必须在钱包做一次小额估算确认 /api/bundler
- `getvela.app/src/routes/api/` — 8 条路由(bundler/wallet/nft/transactions/exchange-rate/bug-report/og/proxy)同住一个 Worker,Worker 挂=全灭

**常见错误**:
- 【受训者原错】通用排障套话:"先看日志、重启服务、检查网络"——没有一步是本项目特异的。
- 【受训者原错】不知道 underfunded 充值 modal 是第一嫌疑的高频故障。
- 把 getvela.app 挂和 vela-bundler 挂混为一谈(一个是代理 Worker,一个是独立仓库的 bundler 服务,诊断命令完全不同)。
- 以为官网 Worker 挂了 Web 钱包也一起挂(CF Pages 与 CF Worker 是独立部署单元)。
- 以为某条链 RPC 挂会导致发不出交易(发送走 bundler;RPC 挂主要影响余额展示)。

**追问**:
1. 如果用户报的不是"发不出交易"而是"新设备恢复找不到钱包",嫌疑对象换成谁?最坏情况(D1 数据丢失)的恢复路径存在吗?(答:p256-index 单点;重建脚本尚不存在,08 手册 C3。)
2. 为什么"余额显示正常"这条观察能帮你排除一整层嫌疑?(答:余额走 RPC pool、发送走 bundler,余额正常=RPC 层大概率无恙。)
3. 在告警为零的现状下,你上任第一周会给哪三个信号配告警?(答:06 手册建议——/api/bundler 5xx、Alchemy/Pimlico 用量、p256-index 健康。)

**真懂 vs 背诵**:真懂的人按"客户端证据→高频已知故障→代理层→bundler 服务→RPC"分层排除并给出每层的具体命令(wrangler tail/rollback、vela.* 注入、小额估算 smoke);背诵的人给通用 SRE 流程。

**评分规则**:3 分线=能从 bug report 的 diagnostics 出发,说出 underfunded 和 gas-price 两大高频嫌疑及 wrangler tail/rollback;5 分线=完整分层序列+死活功能矩阵+vela.serviceEndpoints 缓解+知道告警缺失的现状。

### D11-D12-ops-external-Q6 · 难度 5/5

**考察目标**:修改影响题:落实 08-B3(代理滥用防护)——判断"照抄 bug-report 限流"方案的适用边界与已知缺陷,选对"不改代码"的推荐路径,并给出完整的验证与发布序列。

**题干**:未决事项 B3:bundler/wallet/nft/transactions 四条代理路由没有任何速率限制(只有 bug-report 有)。假设你决定动手:(a) bug-report 现有的限流是怎么实现的,它有什么在注释里写明的已知缺陷?(b) 直接把这套照抄到 /api/bundler 会出什么问题?(c) 08 手册实际推荐的方案是什么,为什么不用改代码?(d) 无论选哪条路,完整的验证+发布+回滚序列是什么?

**标准答案要点**:
1. bug-report 现状:isolate 内存 Map<ip, timestamps[]>,RATE_LIMIT=5 次/RATE_WINDOW_MS=10 分钟,getClientAddress() 取 IP(可 throw 则降级共享桶),超限返回 429 {error:'rate_limited'};另有 MAX_BODY_CHARS=16000 的体积上限。
2. 写明的已知缺陷:Cloudflare isolate 短命且互不共享,这只是 best-effort、只能挡单个 isolate 内的突发;注释明说 'durable KV/DO limiter is the production upgrade'——它不是全局限流。
3. 照抄到 bundler 的问题一:阈值完全不适用——bug report 一人 5 次/10 分钟合理,但钱包正常使用(估算、gas 报价、发送、重试)对 /api/bundler 的调用频率远高于此,照抄=把正常用户限死在发送流程里。
4. 照抄的问题二:客户端行为不同——bug-report 客户端对任何失败都有 fallbackUrl 优雅降级,而 bundler 调用失败直接表现为估算/发送失败,429 会被用户感知为"发不出交易"(正好制造 Q5 那种半夜工单)。
5. 08 手册推荐:Cloudflare WAF rate-limiting rules(纯运维配置,不改代码)+ Alchemy/Pimlico 提供商用量告警;B3 验收=CF 规则生效+压测确认限流+提供商告警配置截图。
6. 若仍改代码,触点:getvela.app/src/routes/api/bundler/+server.ts(及 wallet/nft/transactions 三条),阈值必须基于钱包真实调用频率实测而非照抄;客户端侧要确认发送/估算路径对 429 的呈现是可理解的错误而非静默失败。
7. 验证序列:cd getvela.app && bun run check(0 errors,这也是 CI site job 的门禁)→ 本地 .dev.vars 起 dev 环境,用钱包的 vela.serviceEndpoints 指向本地/预发部署,完整走一遍估算+发送确认不误伤 → 压测确认限流生效。
8. 发布与回滚:cd getvela.app && bun run deploy;发布后立刻在钱包做一次小额估算(不需提交)确认 /api/bundler 正常(05 手册硬性要求);坏了 wrangler rollback。
9. CORS 不是防护:hooks.server.ts 只对浏览器生效,curl 不受限——注释原文 "the real protection is each route's own rate-limit/token",所以 B3 才成立。

**代码证据**:
- `docs/project-takeover/08-open-issues.md:37-39` — B3 原文:四代理无限流(bug-report 有);建议 CF WAF rules+用量告警;验收含压测与截图
- `getvela.app/src/routes/api/bug-report/+server.ts:26-29` — MAX_BODY_CHARS=16000、RATE_LIMIT=5、RATE_WINDOW_MS=10 分钟
- `getvela.app/src/routes/api/bug-report/+server.ts:40-55` — in-memory limiter 实现与 "isolate 不共享,KV/DO 才是 production upgrade" 注释
- `getvela.app/src/routes/api/bug-report/+server.ts:83-91` — getClientAddress 降级与 429 rate_limited 返回
- `src/services/bug-report.ts:142-151` — 客户端把任何非 2xx(含 429)转为 ok:false+fallbackUrl,天然优雅降级(bundler 调用没有这层)
- `getvela.app/src/hooks.server.ts:9-13` — 注释明确:CORS 挡不住 curl,真正防护是各路由自己的 rate-limit/token
- `docs/project-takeover/05-deployment-runbook.md:47-51` — getvela.app 发布/回滚序列与"发布后小额估算确认 /api/bundler"的硬性 smoke
- `docs/project-takeover/06-operations-runbook.md:7` — 告警建议:/api/bundler 5xx 率、Alchemy/Pimlico 用量阈值
- `.github/workflows/ci.yml:46-47` — bun run check 是 site job 门禁,改动必须过它

**常见错误**:
- 【受训者原错的变体】"这些代理转发的都是公开数据,不需要防护"——烧的是你的 Alchemy/Pimlico 配额和账单,配额被封=全体用户估算/发送失败。
- 以为 in-memory Map 在 Cloudflare 上是全局共享的可靠限流(isolate-local,注释里明确否认)。
- 直接照抄 5 次/10 分钟阈值到 bundler,把正常发送流程限死。
- 只想到改代码,不知道 08 推荐的是 WAF 规则+用量告警这条不改代码的路径。
- 以为 CORS 白名单(hooks.server.ts)已经算防护了。

**追问**:
1. 为什么 bug-report 能容忍激进限流而 bundler 不能?从两者客户端失败路径的差异回答。
2. 如果要做 "production upgrade",KV 和 Durable Object 两种限流各有什么取舍?
3. B3 的验收里为什么要求"提供商用量告警配置截图"而不只是 CF 规则生效?(答:两层防线——限流挡滥用,告警兜底发现漏网。)

**真懂 vs 背诵**:真懂的人会先质疑"照抄"前提(阈值语境、isolate 局限、客户端降级差异),然后指出 08 推荐的免代码路径;背诵的人直接复制 rateLimited 函数交差。

**评分规则**:3 分线=说清 bug-report 限流机制+isolate-local 缺陷+发布/回滚命令;5 分线=额外指出阈值不可照抄的两个理由、08 的 WAF+告警推荐路径、以及"发布后小额估算 smoke"这一硬性验证。

---

## 综合面试题(跨域)— 答案要点与评分维度

### 综合-Q1 · 两分钟项目介绍

**考察目标**:脱稿输出完整、准确、含商业模型的项目叙事——检验知识是否内化成"自己的话",以及事实纪律(不复读陈旧 README、不虚构审计状态)。

**评分维度(总分 5)**:
- **准确性(0–2 分)**:每条事实可对上代码或 docs/CONTENT-SOURCE-100-CLUES.md(全项目事实库)。**红线错误(任一命中即本维度 0 分)**:① 说"审计已完成/已排期"——实情是无第三方审计且未排期,现有审查=开源+社区+AI 辅助,不等于专业审计;② 照抄 README 陈旧数据(README 已过时:实际约 12 条链而非 8、费率是约 2×/3×-cap 加成而非 60%、WalletPair 走 WebSocket 而非 BLE);③ 把产品说成托管/助记词钱包。
- **无资料(0–1.5 分)**:全程不翻文档不开 IDE;两分钟内讲完;卡壳超过 10 秒或折返修正核心事实各扣 0.5。
- **覆盖商业模型(0–1.5 分)**:必须讲到:钱包免费,收入=内置 bundler/relayer 的 gas 加成(本地兜底 2× margin、报价档位 1.1–2.0×);"活跃多链交易者"是收入核心用户而非获客入口;当前阶段=接管审计 CONDITIONAL GO、正备战 App Store/Play 上架、上架后 90 天门槛决定全职或搁置。

**标准内容骨架(参考,不要求逐句)**:Vela 是非托管智能合约钱包:passkey(P-256/WebAuthn)即账户,无助记词;链上是 Safe + ERC-4337(Safe4337Module + WebAuthn signer,RIP-7212 precompile 验签);地址由公钥经 CREATE2 推导,跨链同址;覆盖约 12 条 EVM 链,Tempo 链用 TIP-20 稳定币付 gas;换机恢复靠 p256-index 公钥索引服务(HTTP API,数据存 Gnosis 链)。赚钱方式:免费下载,内置 bundler 对 gas 加成收费。现状:单人创始团队,无第三方审计(如实告知),准备上架。

**追问参考口径**:
1. "出处是哪"——合格答案能区分代码事实(file:line)与文档事实(CONTENT-SOURCE-100-CLUES.md/接管手册),并知道 README 是不可信来源。
2. "没审计凭什么信"——诚实路线:开源可查+社区/AI 审查+非托管架构(服务全挂资金无损),绝不许诺"审计快来了"。
3. "多少条链"——正确动作是引用代码里的链配置/事实库而非 README。

**真懂 vs 背诵**:真懂的人叙事有因果("因为 passkey 是唯一密钥,所以恢复必须有公钥索引服务;因为免费,所以收入只能在 relayer margin"),数字全部对得上源头;背诵的人罗列 buzzword,商业模型一句带过或数字来自陈旧 README。

### 综合-Q2 · 新需求改动范围:转账白名单

**考察目标**:跨域改动范围分析(D4 交易构造 + D5 防线分层 + D6-D7 存储 + D8-D9 验证):能否识别"收款方提取"的真实难度、复用既有纵深防御模式、避开存储与 i18n 的已知地雷。

**标准答案要点**:
1. **(a) 改动面**:设置 UI(SettingsScreen 新开关)+ 新 AsyncStorage 键(遵守 vela.* 前缀;决定进不进 storage.ts KEYS 表——进则 clearAll 登出会重置,安全开关是否跨登出保留是要明说的产品决策);联系人数据源 src/services/contacts.ts(vela.contacts,注意它在 KEYS 表外);Send 路径(SendScreen 确认禁用)+ split/sweep 批量路径(sendBatchCalls 的每个子调用逐一校验);dApp 路径(SigningRequestModal/SigningSheet 显示警告 + requiresHold 滑动确认,复用既有 SlideToConfirmButton 模式);提交咽喉兜底(仿 enforceNoUnlimited 在 use-dapp-signing.ts:322 的模式,handleDAppRequest 路由层 + handleSendCalls per-leg 重查 364-368 的双点);i18n 文案(14 个 locale 全覆盖,键深 ≤3 段的已知 gotcha)。
2. **(b) 三个隐藏耦合**:① 收款方 ≠ tx.to:ERC-20 转账的 tx.to 是 token 合约,真实收款人在 calldata(transfer selector 后第一个 address word);② UI 校验可被 rewrite 异常/paramsOverride=undefined 类故障绕过——必须像 approval-guard 一样三层纵深(UI 编辑器 → per-leg → 咽喉 fails-closed),只做 UI 层就是重蹈"以为 UI 是唯一防线"的原错;③ SigningSheet 是单一渲染路径(prod + harness 共用,SigningRequestModal.tsx:95-103)——不许在 harness 另写一份;④(任选其三)开关的同步读取需要"启动灌注+同步 getter"模式(参照 endpoints 缓存),不能在渲染期 await AsyncStorage。
3. **(c) 收款方提取**:native 转账=executeUserOp 的 to;ERC-20=decode calldata transfer(to,amount) 的 word1(transferFrom 则是 word2);批量 MultiSend=解包 packed 子调用逐条提取(参照 buildMultiSendExecuteCallData 的编码逆过程);dApp 任意 calldata=无法穷举,只能识别已知转出形态(做法对标 detectApproval 的 8 形态清单)——**识别不了的合约调用不能硬拦也不能放行,走"警告+滑动确认"降级**(类比 SIWE 的 unknown≠mismatch 三态纪律:只有确证的"转给陌生人"才禁止)。
4. **(d) 存储**:新键建议进 KEYS 表并接受 clearAll 语义;若要跨登出保留则必须在文档里标注 clearAll 盲区(与 vela.rpc.banned 同类)。
5. **(e) 验证**:地址提取器做纯函数 jest 单测(native/ERC-20/transferFrom/MultiSend/无法解析 各 golden vector);四件套门禁;parallel space 走 Send/split/sweep/test-dApp 真实流程(含白名单命中与不命中);确认 harness 里 SigningSheet 行为同步生效;read-only replay 无确认按钮,确认不受影响。

**评分规则**:3 分线=覆盖 Send+批量+dApp 三条路径、知道 ERC-20 收款人在 calldata、给出可行验证;5 分线=提出提交咽喉 fails-closed 兜底+per-leg 批量校验(引用 enforceNoUnlimited 模式)、"无法解析→降级滑动确认"的三态纪律、存储 clearAll 语义与单一渲染路径约束。

**常见错误**:只在 SendScreen 加个 if(漏批量与 dApp);拿 tx.to 当收款人(ERC-20 全部漏判);无法解析的 calldata 一律硬拦(杀死全部合约交互)或一律放行(白名单形同虚设);在 harness 复制一份逻辑。

### 综合-Q3 · 生产事故复盘:充值 modal 静默失效

**考察目标**:复盘方法论 + 跨仓库耦合的系统性理解:能否把"一句文案改动"追到完整故障链,区分触发因素与根因,并识别"防线存在但为什么没起作用"。

**标准答案要点**:
1. **(a) 故障链**:vela-bundler handlers.ts 文案改动 → 钱包 parseBundlerUnderfunded(bundler-service.ts:367)的两组稳定信号正则(/dedicated bundler (gas account|EOA)/i;'Deposit to: 0x'+'required:')全部落空 → 返回 null → 两个消费点 SendScreen.tsx:984 与 dapp-connection.tsx:593 都不弹 funding modal → 原始英文报错透出。前置的主动防线 checkBundlerFunding(bundler-service.ts:118)只在发送前查询余额,查询失败或余额在阈值边缘时会漏,parse 正是它的兜底——兜底断了故障才可见。
2. **(b) 防线为何全部漏过**:① 回归测试(bundler-service.test.ts:54-91)固定的是**钱包仓内**的两代文案样本——bundler 仓的 PR 不会触发钱包的测试,单侧测试护不住双边契约;② 发布 checklist(05-deployment-runbook.md:30)明文要求"改 bundler 错误文案须与钱包联合验证"——流程存在但没被执行,这是流程执行失败而非流程缺失;③ 无遥测无告警(06-operations-runbook.md:5-7),发现渠道只有用户 bug report,40 分钟延迟是观测缺失的直接代价。
3. **(c) 三层行动项**:立即=钱包侧 hotfix 扩正则兼容新文案 + 把新文案样本追加进回归测试;短期=vela-bundler 仓 PR 模板/review checklist 增加"动过错误文案?→ 与钱包联验"强制项,双仓文案变更互相 tag review;长期=结构化错误码(机器可读字段,如 code:'BUNDLER_UNDERFUNDED'+结构化 deposit/required 字段),过渡期 bundler 双发(新字段+旧文案并存)兼容存量旧版 App,同时给 /api/bundler 5xx 率或 parse-miss 配告警。
4. **(d) 最深层问题**:跨仓库隐式契约以自然语言文案为载体(memory 中的已知架构债:parseBundlerUnderfunded 必须与 vela-bundler handlers.ts 措辞同步)+ 零观测——这是架构与流程问题,不是某一行代码的 bug;单纯"把正则改对"没有消除事故类别。

**评分维度(总分 5)**:
- 故障链完整性(0–1.5):从文案改动追到两个消费点与用户可见现象,并说清 checkBundlerFunding 与 parse 的主动/兜底关系。
- 防线分析(0–1.5):三道防线(单侧回归测试/未执行的 checklist/零告警)各自"为什么没拦住"——尤其能说出"测试是绿的因为它只护钱包侧"。
- 行动项质量(0–1):三层齐全、可验收(有 owner/验收标准意识),长期方案含旧版本兼容。
- 复盘素养(0–1):blameless、区分触发因素(那个 PR)与根因(字符串契约+无观测)、承认"下次换个措辞还会炸"除非根治。

**常见错误**:把根因归结为"bundler 同事不小心"(人因归因,复盘失格);行动项只有"改回文案";不知道有两个消费点,只修 Send 路径;长期方案切换错误码却没有旧 App 过渡期方案。

### 综合-Q4 · 证明这代码是你理解的

**考察目标**:开放性深度检验:选题品味、因果推理深度、反事实推演、代码锚定与诚实度——区分"内化的理解"与"高质量复述"的终极题。

**评分维度(每项 0–1,总分 5)**:
1. **选题含金量**:选了有真实设计张力的点。强选题示例(不限于):isQuoteAbusive 的非对称信任+tipMeasured fail-open;enforceNoUnlimited 提交咽喉 fails-closed 三层纵深;HomeScreen merge-by-chain 余额铁律;initCode→setupData→CREATE2 地址推导因果链;pending-at-submit 状态机与双收敛路径;_layout.tsx 首行 polyfill 顺序;ParallelSpaceBadge 无条件渲染的威胁链;sim-engine null≠ok:false 三态。弱选题:纯 UI 样式、通用工具函数、任何钱包都成立的泛泛逻辑。
2. **因果深度**:讲的是"为什么非这样不可"而非"它做了什么";能把设计追到它防的具体事故/用户损失(如"写反 null/ok:false → 所有不支持 simulateV1 的 RPC 上健康交易全部误报预计失败")。
3. **反事实质量**:删掉/写反后的第一个受害场景具体到链、用户操作、可见症状;能回应面试官现场提出的"更简单写法"并指出该简化丢掉的那个失效模式。
4. **代码锚定**:file:line 与基线 73d7aac 对得上(允许小幅行号偏移);被追问"行号还准吗"时能说明核对时点而不是硬撑。
5. **诚实度**:主动给出一个真实的不确定点/没读透的细节(例如"Tempo nonce 不对称我知道现象但没验证过修法"),且该不确定点与选题相邻——说不出任何不确定点按 0 分计(背诵者的典型特征是全知幻觉)。

**面试官操作指引**:三层"为什么"示例——第一层问机制(它怎么工作),第二层问取舍(为什么不用 X),第三层问边界(什么假设破了它就错了)。红旗信号:只讲 what;被简化方案问倒后开始编造不存在的约束;行号张口就来但抽查全错;把输入 JSON/手册里的措辞逐句复读而无法换角度重述。

**5 分样例特征**:候选人自选 enforceNoUnlimited,白板画出 UI 编辑器→per-leg→咽喉三层,反事实推到"rewrite 抛异常静默置 undefined 正是靠咽喉兜底,删掉 322 行后这条 catch 就成了 unlimited 直放通道",引用 approval-guard.ts:377-396 的三类放行并解释各自替代门控,最后主动说"PermitBatch 逐条金额的 UI 呈现我还没读透"。

---

**统计**:全库共 40 题 = 6 域 × 6 题(D3/D4/D5-D10/D6-D7/D8-D9/D11-D12)+ 4 道综合题。难度分布(域内):难度1×6、难度2×6、难度3×8、难度4×10、难度5×6。





