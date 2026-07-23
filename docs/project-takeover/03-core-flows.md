# 03 — 核心用户流程 (Core Flows)

> 每条流程列出:入口 → 关键代码 → 领域规则。行号为 2026-07-02 审计时状态。

## 1. 创建钱包(Onboarding)

入口 `/onboarding` → `src/screens/onboarding/CreateWalletScreen.tsx`

1. `Passkey.isSupported()` → `Passkey.register(name)`(`src/modules/passkey/index.ts`,平台分派:iOS ASAuthorization / Android Credential Manager / Web navigator.credentials)
2. `validateCreateClientData()`(`src/services/public-key-upload.ts:36`)——设计意图是**在保存任何东西之前**拒绝 clientDataJSON 字段顺序不合 Safe 合约要求的 passkey 提供方。**⚠️ 2026-07-02 核实:该函数生产代码零调用方(仅导出),门未接线**——见 08-open-issues C7,决策待定(接线或删除)
3. 解析 attestation 提取 P-256 公钥 → `computeSafeAddress()` CREATE2 推导(`src/services/safe-address.ts`),Safe 此时**未部署**(counterfactual),首笔交易时通过 initCode 部署
4. 公钥上传索引服务:3 次重试(1s/2s 退避)→ create→**verify 为准**→失败进 pending 队列,App 启动时 `retryPendingUploads()` 静默重试(幂等,Idempotency-Key = `rpId:credentialId`)
5. 账户写入 AsyncStorage `vela.accounts`(只有 credentialId/地址/公钥,无私密材料)

**隐含约束**:passkey 私钥在平台认证器内,**没有导出/备份路径**;跨设备恢复依赖 (a) 平台 passkey 同步(iCloud Keychain / Google Password Manager)+ (b) 公钥索引服务把 credentialId→地址映射找回来。索引服务挂了 = 新设备找不回地址(资金安全,但入口丢失)→ 这是单点,见 08。

## 2. 发送(Send / Split / Sweep / Payroll)

入口 `/send`(modal)→ `src/screens/wallet/SendScreen.tsx`

- 单笔:`sendNative()` / `sendERC20()`;批量(split 1→N、sweep N→1、payroll 法币换算批发):全部编译成一个 MultiSend `sendBatchCalls()`(`src/services/safe-transaction.ts:104-197`)
- 核心提交链 `sendUserOp()`(`safe-transaction.ts:459`):
  1. `verifyChainReady()` 校验 EntryPoint 存在(每链缓存)
  2. 并行取 deployed/nonce/gasPrices;**已部署且 nonce 取不到 → 签名前快速失败**(接管审计修复)
  3. 定价优先级:确认屏报价 override(带 bigint 运行时类型防御)> bundler 自身报价 `pimlico_getUserOperationGasPrice` > 本地估算;**bundler 是价格权威,钱包 RPC 永不否决它**(Gnosis "gas price too low" 惨案的教训,`isQuoteAbusive` 只拒绝 >3× bundler 自报网络价的滥价)
  4. bundler 估 gas;**大 calldata(>1024B)估算失败直接拒绝**(否则会静默 2 分钟超时)
  5. EIP-712 SafeOp hash → passkey 签名 → DER→raw → 合约签名格式
  6. 提交:3 次重试(仅 transient);`[existingHash:0x…]` → 改为轮询已有 op(防重复提交)
  7. 成功后**乐观递增 nonce 缓存**(并发发送不撞 nonce);回执轮询 1s→3s 自适应退避,120s 上限
- gas 档位:slow 1.1× / standard 1.2× / rapid 1.5× / fast 2.0×(`GAS_TIER_MULTIPLIERS`)
- bundler 余额不足 → `parseBundlerUnderfunded()`(`bundler-service.ts:367`)字符串匹配错误文案弹充值 modal —— **与 vela-relay 仓库 handlers.ts 文案强耦合,改任何一边必须同步**

## 3. Tempo 链(4217)特殊规则

无原生币,gas 用 TIP-20 稳定币(`src/services/tempo.ts`):UserOp 以 maxFee=0 签名,MultiSend 里内嵌 `feeToken.transfer(bundlerEOA, fee)` 报销;fee = 实际成本×2,callGasLimit 每子调用垫到 380k(TIP-20 转账 308k+,估算器会低报)。**改批量逻辑时勿动报销 transfer 的位置。**

## 4. dApp 连接与签名

入口 `/(tabs)/connect` 扫码/粘贴 → `src/models/dapp-connection.tsx`(Context)
两条传输:RemoteInjectTransport(SSE+POST,浏览器插件桥)与 WalletPairTransport(WS relay,本地 WalletPair v1 协议实现)。会话持久化 `vela.walletpairSession`/`vela.remoteInjectSession`,App 恢复时自动重连(60s reconnecting 宽限)。

签名请求 → `src/hooks/use-dapp-signing.ts` 方法路由(personal_sign / eth_signTypedData_v4 / eth_sendTransaction / wallet_sendCalls EIP-5792)→ 单一 `<SigningSheet>`(`src/components/SigningRequestModal.tsx`,生产与测试 harness 同一渲染路径):

- **交易模拟**:`eth_simulateV1`(`sim-engine-rpc.ts`;`null`=引擎不可用降级,`{ok:false}`=真实 revert);Tevm 引擎留了缝但禁用
- **Clear Signing**:ERC-7730 描述符(`clear-signing.ts`),无描述符 → blind-sign 硬警告
- **Never-unlimited 强制**:`detectApproval` → 用户可改上限/撤销 → `rewriteApprovalParams` 重写 calldata(其余字节保持)→ 出门前 `enforceNoUnlimited`(`use-dapp-signing.ts:322,367`)兜底 throw。上限:uint256 1<<200,uint160(Permit2) 1<<152。**任何新签名路径必须过这两道。**
- SIWE 域名绑定校验(钓鱼检测)、EIP-1271 智能合约签名、只读 RPC 方法白名单直通
- dApp 交易与 Send 同一条 sendUserOp 估算/资金检查路径(禁止盲提交)

## 5. 活动/历史

`src/services/activity.ts`:本地交易 **pending-at-submit 即持久化**(关页面/刷新不丢),回执后更新;链上收款靠 RPC 转账监听合并。回放:历史签名记录用只读 SigningSheet 重现。

## 6. 余额/价格

`src/services/wallet-api.ts`:Multicall3 单次 eth_call 批量余额+DEX 价格;5 分钟缓存 + in-flight 去重;失败链**回退缓存值,绝不清零健康链余额**(merge-by-chain 有专门回归测试)。Chainlink 兜底;法币汇率走可配置端点。429 限流 = 瞬态:显示缓存、不弹"换 RPC"横幅(`getRateLimitedChains()` 贯穿 rpc-pool↔HomeScreen↔故障注入)。

## 7. RPC 池(所有链上读的地基)

`src/services/rpc-pool.ts`:端点来源 7 级优先(用户覆盖 > 提供商 key > 内置 > 公共池 > ethereum-data 索引);评分 = 来源基线 − 延迟惩罚 + 成功加成 − 失败惩罚;两级封禁(临时 1h:限流/401;永久 24h:从未成功且 ≥6 失败,持久化 AsyncStorage)。错误分类:permanent(鉴权/付费)立即封禁,transient(5xx/超时)只故障转移。

## 8. 测试环境 Parallel Space(理解它才能安全测试)

见 `02-local-development.md`。关键不变量:**唯一差异是签名密钥**;真实钱包缓存进入时备份、退出恢复;激活时全局紫色徽章(生产构建也渲染——接管审计修复 `src/app/_layout.tsx`)。

## 最难改/最易回归的区域(改前必读)

| 区域 | 为什么危险 |
|---|---|
| `safe-transaction.ts` 签名/编码(SafeOp hash、签名格式、MultiSend 编码) | 错一个字节 = 链上验签失败或资金操作错误;全部手写无 viem 兜底 |
| gas 定价链(bundler 报价权威原则) | 历史上反复出 "gas price too low"/"—" 费率回归,已有专门检验;改动须过 `bundler-service.test.ts` |
| `parseBundlerUnderfunded` 文案匹配 | 跨仓库耦合 vela-relay |
| approval-guard 检测/重写 | 安全承诺"never unlimited";所有 approval 形态(ERC-20/2612/DAI permit/Permit2/setApprovalForAll)都要覆盖 |
| Tempo 报销批量 | 估算器低报,垫值和 2× 边际是实测值 |
| i18n 键深 ≤3 段 | 超深会静默 fallback 成键名(14 语言) |
| `storage.ts ↔ models/types.ts` require cycle | Metro 警告已存在,重构时留意初始化顺序 |
