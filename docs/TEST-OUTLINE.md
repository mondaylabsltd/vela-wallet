# Vela Wallet — 测试大纲（以用户故事为中心）

> **本文档定位.** 这是 Vela Wallet 的**测试总纲**：先把系统拆成「**核心** vs **依赖**」，核心再拆成模块；然后从**真实用户流程**出发、以**用户故事（User Story）**为主轴，把每个故事映射到「命中哪些核心模块、依赖哪些外部系统、该在哪一层测、现有覆盖到没到」。
>
> 它是**索引 / 骨架**，不重复细节。配套：
> - 自动化用例细节 → [test-plan.md](test-plan.md)
> - 人工逐条回归清单（100 条，按优先级）→ [MANUAL-TEST-100-CLUES.md](MANUAL-TEST-100-CLUES.md)
> - 故障注入命令 → `src/services/dev/fault-injection.ts`（浏览器 console 的 `vela.*`）
>
> 全部结论来自对 `src/` 的真实通读（65 个 service/model、`src/screens/**` 六大旅程、`src/__tests__/**` 现有 60+ 单测 + `e2e/` 3 个 Playwright）。README 已过时，以本文与源码为准：**12 链、ERC-4337（EntryPoint + Safe + passkey）、Tempo(4217) gas 用稳定币结算、WalletPair 走 WebSocket（无蓝牙）**。

---

## 0. 测试哲学（为什么这样拆）

1. **核心可离线、确定、快** —— 核心是纯逻辑（密钥推导、交易编码、授权拦截、模拟、格式化）。这些用 `jest` 单测覆盖，**零网络、毫秒级、可穷举边界**。这是资金安全的护城河，必须最密。
2. **依赖只在边界处 mock** —— 所有外部系统（RPC / bundler / Vela 后端 / 行情 / dApp relay / 原生模块）只在**注入点**打桩，一处 mock 覆盖多条流程。绝不为了测 UI 去连真网。
3. **用户故事驱动集成/e2e** —— 集成与 e2e 不按文件测，按**"用户想干成一件事"**测：一个故事跨多个模块 + 依赖，验的是"这条路能不能走通、走不通时 UX 对不对"。
4. **失败态是一等公民** —— 钱包的价值一半在 happy path，一半在"网挂了 / gas 不够 / 同步失败 / 二维码是脏的"时**不丢钱、不误导、可恢复**。每个故事都必须列失败态用例，用 `vela.*` 故障注入验。

**测试金字塔（本项目实际形态）：**

```
        ┌─────────────────────┐
        │  手动真机 (P0 资金/生物识别/相机)   │  ← MANUAL-TEST-100-CLUES.md
        ├─────────────────────┤
        │  E2E Playwright (Web, 关键旅程)     │  ← e2e/*.spec.ts + /clear-signing-test
        ├─────────────────────┤
        │  组件/交互测试 (待建, 见 §6 gap)      │
        ├─────────────────────┤
        │  服务集成 (mock 依赖, 跨模块)         │
        ├─────────────────────┤
        │  单元 (纯核心逻辑, 已有 60+)  ← 最厚    │
        └─────────────────────┘
```

---

## 1. 第一层拆分：核心 vs 依赖

### 1.1 核心（Core）— 我们自己的确定性逻辑

见 §2 的 10 个模块。判据：**给定输入 → 确定输出，不碰网络/磁盘/设备**（或 IO 可在一个函数边界注入）。

### 1.2 依赖（Dependencies）— 外部系统，测试时必须 mock/stub

| ID | 依赖 | 具体端点/协议 | 注入点（在哪 mock） | 已有 test double? |
|----|------|--------------|---------------------|-------------------|
| **D1** | 区块链 RPC 节点（每链，用户/Alchemy/dRPC/Ankr/Vela/公共/ethereum-data 分级） | JSON-RPC 2.0 over HTTP；带打分、封禁、故障转移 | mock `fetch` | ✅ `rpc-pool.test.ts`（failover/封禁/getLogs cap）+ 签名/模拟测试里 mock |
| **D2** | Bundler（ERC-4337），`vela-relay.getvela.app` | JSON-RPC（`eth_sendUserOperation` 等）+ REST（`/v1/account`、`/v1/sponsor`，带 Idempotency-Key） | mock `fetch` | ✅ `bundler-service.test.ts`（含 `parseBundlerUnderfunded`） |
| **D3** | Vela 后端：`p256-index.getvela.app`（CF Worker+D1+DO）、`getvela.app/api/bug-report`、`ethereum-data`（缓存 token 列表 + 7730 descriptor） | REST（`/api/query`、`/api/create`；bug-report 代理） | mock `fetch` | ✅ `bug-report.test.ts`；⚠️ public-key-index 仅集成 |
| **D4** | 行情 / 汇率：Chainlink（链上，经 RPC）、可配 FX endpoint（Frankfurter / er-api / 自定义） | Chainlink 走 RPC `eth_call`；FX 走 HTTP GET | mock RPC + mock `fetch`；或用缓存快照 | ⚠️ 有缓存+回退，多数用快照 |
| **D5** | dApp relay：remote-inject（SSE + POST）、WalletPair（WebSocket，spec §7/§9.6） | `EventSource` / `WebSocket` | mock `EventSource` / `WebSocket` | ⚠️ 签名逻辑经 `dapp-signing*.test.ts`；transport 内部无独立桩 |
| **D6** | 原生/设备：VelaPasskey（iOS/Android）/ `navigator.credentials`（web）、相机、媒体库、haptics、剪贴板 | WebAuthn API、Expo 原生模块 | mock `NativeModules.VelaPasskey` / `navigator.credentials`；相机用 fixture 图 | ❌ 需补（web 可用 DevTools WebAuthn 虚拟认证器） |
| **D7** | 4-byte / descriptor 服务：Sourcify、OpenChain、4byte.directory、ethereum-data(7730) | 并行 HTTP 查询 + 缓存 | mock `fetch` | ✅ `selector-registry.test.ts`、`clear-signing.test.ts` |
| **D8** | Deployer service（`foundry-contract-deployer-server`） | RPC 轮询（当前实现为 mock/TODO 真 API） | 已是 mock | — |

**离线跑全流程必须 mock 的最小集：** D1、D2、D3、D5、D6（D4/D7 有缓存回退可用快照，D8 已 mock）。

---

## 2. 核心模块拆分（Core Modules）

> 标注：**[纯]** = 纯计算，可穷举边界；**[IO:x]** = 依赖外部系统 x（测试需注入）。"覆盖" 引用 `src/__tests__/`。

### M1 — 密钥与账户（Key & Account）
钱包身份的根，**P0 资金安全**。
| 文件 | 职责 | 类型 | 覆盖 |
|------|------|------|------|
| `modules/passkey/index.ts` | 统一 WebAuthn（iOS/Android 原生 + web credentials） | [IO:D6] | ⚠️ |
| `services/attestation-parser.ts` | 解 CBOR attestation，抽 P-256 (x,y)，DER→r‖s | [纯] | ✅ |
| `services/webauthn-verify.ts` | Safe WebAuthn 断言兼容性检查（clientDataJSON、UV flag） | [纯] | ✅（新增 webauthn-verify.test.ts） |
| `services/safe-address.ts` | 由 P-256 公钥确定性推导 Safe 地址、CREATE2 salt、MultiSend 编码 | [纯] | ✅ |
| `services/eth-crypto.ts` | keccak-256、ABI 编码、CREATE2、EIP-55 checksum | [纯] | ✅ |
| `services/accounts.ts` | 账户按余额排序、总额聚合 | [纯] | ✅ |
| `services/public-key-upload.ts` | 上传编排（校验[纯] + 建记录[IO] + 验证为准） | [纯]+[IO:D3] | ✅（新增 public-key-upload.test.ts 四象限） |
| `services/public-key-index.ts` | p256-index 客户端（跨设备恢复） | [IO:D3] | ⚠️ 集成 |

### M2 — 交易构建与签名（Tx Building & Signing）
| 文件 | 职责 | 类型 | 覆盖 |
|------|------|------|------|
| `services/safe-transaction.ts` | 构建/签名 ERC-4337 UserOp、gas 估算、Tempo 稳定币结算 | [纯]+[IO:D1/D2] | ✅ |
| `services/batch-send.ts` | split（1→N）/ multiSelect（N→1）编码，MultiSend 批量 | [纯] | ✅ |
| `services/bundler-service.ts` | gas 账户查询、赞助资格、充值建议、underfunded 解析 | [IO:D2] | ✅ |
| `services/eip712.ts` | EIP-712 typed-data 哈希 | [纯] | ✅ |
| `services/siwe.ts` | EIP-191 SIWE 签名 | [纯] | ✅ |

### M3 — 交易安全与可读化（Clear Signing / Safety）
**P0 防钓鱼护城河**，Vela 核心卖点。
| 文件 | 职责 | 类型 | 覆盖 |
|------|------|------|------|
| `services/approval-guard.ts` | 检测无限授权（approve/increaseAllowance/setApprovalForAll/permit/Permit2）并改写额度 | [纯] | ✅ |
| `services/clear-signing.ts` | ERC-7730 descriptor 匹配、字段解码、风险分级 | [纯]+[IO:D3/D7] | ✅ |
| `services/tx-simulation.ts` | 模拟编排：eth_call 回滚预检 + 资产变化引擎选择 | [IO:D1] | ✅ |
| `services/sim-assets.ts` | 从 log 算资产净变化、解析 revert 原因 | [纯] | ✅ |
| `services/sim-engine-rpc.ts` | `eth_simulateV1` 后端模拟 | [IO:D1] | ✅ |
| `services/sim-engine-tevm.ts` | Tevm 本地 fork 回退（默认禁用，显式 loader 注入） | [IO:tevm] | ⛔ 刻意可选 |
| `services/abi.ts` / `abi-decode.ts` | ABI 编解码、calldata 解码 | [纯] | ✅ |
| `services/selector-registry.ts` | 4-byte 选择器查询（并行多源）+ 缓存 | [IO:D7] | ✅ |

### M4 — 资产与行情（Assets & Pricing）
| 文件 | 职责 | 类型 | 覆盖 |
|------|------|------|------|
| `services/wallet-api.ts` | 多链资产查询（Multicall3 组合 ERC20 余额+DEX 报价+Chainlink） | [IO:D1] | ⚠️ |
| `services/token-reads.ts` / `token-metadata.ts` / `tokens.ts` | allowance 读、token 元数据解析、已知 token 表 | [纯]/[IO:D1] | ✅/✅/✅（新增 tokens.test.ts） |
| `services/balance-cache.ts` / `balance-history.ts` | 余额缓存（AsyncStorage）、历史余额（archive RPC） | [IO:D6]/[IO:D1] | ✅/— |
| `services/price-service.ts` | Chainlink 主网喂价（Multicall3），缓存 3min | [IO:D4] | ✅（新增 price-service.test.ts：别名+回退） |
| `services/fiat-rates.ts` / `fiat-fx.ts` | 法币汇率（Chainlink via ENS / 可配 FX endpoint） | [IO:D4] | ✅/✅ |
| `services/currency.ts` / `currency-catalog.ts` | 货币换算、货币目录 | [纯] | ✅ |
| `services/format-eth.ts` / `locale-format.ts` | wei 格式化、本地化数字/日期格式（显式预设，非 Intl） | [纯] | ✅ |

### M5 — 网络与 RPC（Network & RPC）
| 文件 | 职责 | 类型 | 覆盖 |
|------|------|------|------|
| `models/chains.ts` / `models/network.ts` | 12 链权威表、explorer/bundler URL 构建、自定义网络 | [纯] | ✅ |
| `services/rpc-pool.ts` | 分级端点、打分、封禁（临时/永久）、10min 刷新、故障转移 | [IO:D1] | ✅（打分/封禁/failover/getLogsRangeCap） |
| `services/rpc-providers.ts` / `rpc-adapter.ts` | provider slug→URL、method 路由（bundler vs rpc） | [纯] | ✅ |
| `services/net.ts` | 超时配置、Timeout/Abort 错误类 | [纯] | ✅ |
| `services/readonly-rpc-gate.ts` | dApp 只读请求限流门（6 并发/512 排队） | [纯] | ✅ |
| `services/network-checker.ts` / `add-network.ts` | 端点健康（eth_chainId/getCode/P256 precompile）、加网络 | [IO:D1] | ✅ |
| `services/chain-registry.ts` | 链注册/查询 | [纯] | ✅ |

### M6 — 活动与对账（Activity & Reconciliation）
| 文件 | 职责 | 类型 | 覆盖 |
|------|------|------|------|
| `services/activity.ts` | 活动条目本地存取、批量折叠 | [纯]+[IO:D6] | ✅ |
| `services/transfer-monitor.ts` | 增量扫 log 检测到账 | [IO:D1] | ✅（+decode 反伪造/spam/native） |
| `services/tx-reconciler.ts` | 轮询 bundler receipt 对账 pending（超时不判失败） | [IO:D2] | ✅ |
| `services/dapp-history.ts` | dApp 签名事件历史（pending→confirmed/failed，可只读重放） | [纯]+[IO:D6] | ✅ |

### M7 — 收付款与地址解析（Pay/Receive & Identity）
| 文件 | 职责 | 类型 | 覆盖 |
|------|------|------|------|
| `services/eip681.ts` | EIP-681 支付 URI 解析/构建 | [纯] | ✅ |
| `services/recipient-identity.ts` | ENS / .bnb / Vela 自有账户名解析（best-effort） | [IO:D1/D3] | ✅ |
| `services/recipient-risk.ts` | 首次转账 / 合约地址风险标记 | [纯]+[IO:D1] | ✅ |
| `services/contacts.ts` | 联系人本地 CRUD | [纯]+[IO:D6] | ✅ |
| `services/qrcode.ts` / `image-decode.ts` | QR 生成 / JPEG·PNG 解码 | [纯] | ✅ QR |

### M8 — dApp 连接（dApp Transport）
| 文件 | 职责 | 类型 | 覆盖 |
|------|------|------|------|
| `services/dapp-transport.ts` | remote-inject relay（SSE + POST，nonce/secret 鉴权） | [IO:D5] | ✅（parseRemoteInjectURL + connect SSE 状态机） |
| `services/walletpair-transport.ts` | WalletPair（WebSocket，能力协商 + Tier-2 只读 RPC 转发） | [IO:D5] | ⚠️ |
| `models/dapp-connection.tsx` | RemoteInjectTransport 会话上下文、请求队列、连接状态 | [纯]+[IO:D5/D6] | ⚠️ |

### M9 — 本地状态与存储（State & Storage）
| 文件 | 职责 | 类型 | 覆盖 |
|------|------|------|------|
| `models/wallet-state.ts` | WalletState reducer（SET_WALLET/ADD_ACCOUNT/SWITCH_ACCOUNT/LOGOUT） | [纯] | ✅（新增 wallet-state.test.ts） |
| `models/types.ts` | 核心域类型 + token 辅助（余额换算、USD、logo） | [纯] | ✅ |
| `services/storage.ts` | AsyncStorage 存取（解析错误静默回退） | [IO:D6] | ✅ |

### M10 — 平台与遥测（Platform & Telemetry）
| 文件 | 职责 | 类型 | 覆盖 |
|------|------|------|------|
| `services/platform.ts` | haptics/剪贴板/浏览器/Alert（动态 import + web 回退） | [IO:D6] | — |
| `services/share-card.ts` | 收款卡截图/保存/分享 | [IO:D6] | — |
| `services/bug-report.ts` / `feedback.ts` | 一键报错代理 + GitHub 回退 | [IO:D3] | ✅ bug-report |
| `services/metrics.ts` | 本地遥测快照（喂给 bug-report 诊断） | [纯] | ✅ |
| `services/local-descriptors.ts` | 本地 7730 descriptor | [纯] | — |

---

## 3. 以用户故事为中心的测试矩阵（主体）

> 6 个 Epic = 6 大真实旅程。每个 Epic 下拆用户故事（US），每个故事给：**验收主线 → 关键失败/边界态 → 命中核心模块 → 依赖(需 mock) → 建议测试层 → 现状**。
> 测试层图例：**U**=单元 / **SI**=服务集成(mock 依赖) / **C**=组件交互 / **E2E**=Playwright / **M**=手动真机。

### Epic 1 — 新用户拥有一个钱包（Onboarding）
> 入口 `/onboarding` → `WelcomeScreen` → `CreateWalletScreen`。核心不变量：**公钥同步成功前，账户绝不落地本地**（见 memory：onboarding 不提前持久化）。

| US | 用户故事 | 验收主线 | 关键失败/边界态 | 核心模块 | 依赖 | 层 | 现状 |
|----|---------|---------|----------------|---------|------|----|----|
| 1.1 | 作为新用户，我用生物识别创建钱包，拿到一个地址 | 输入名→勾选 4 项→passkey 注册→本地算出 Safe 地址→显示可复制地址 | passkey 不支持 / 用户取消 / 公钥抽取失败 | M1(passkey,attestation,safe-address,eth-crypto) | D6 | U + M | U✅ M待 |
| 1.2 | 作为新用户，我的公钥被同步以便跨设备恢复 | pending 上传落盘→`uploadPublicKey`→**3 次自动重试**→verify 为准→**之后**才 dispatch 账户 | index 不可达(3×→自动开设置) / 5xx / 超时 / create 成功但 verify 失败→保持 pending 下次重试 | M1(public-key-upload/index) | D3 | SI + M | SI⚠️ |
| 1.3 | 作为新用户，同步失败时我能看懂并重试/报错 | "Sync Failed" 态：错误详情 + 重试按钮 + 一键 bug 报告 | 反复失败仍可手动重试；不误导为"已成功" | M1,M10(bug-report) | D3 | C/E2E | 待 |
| 1.4 | 作为新用户，我签一笔验证挑战确认 passkey 可用 | 用同一 passkey 签 challenge→WebAuthn 兼容校验通过→进主页 | Safe 版本/字段序不兼容→人类可读错误 | M1(webauthn-verify) | D6 | U + M | **U❌ 缺** |

**Epic 1 失败态清单（须 `vela.*` 或断网验）：** passkey 取消、index 3× 不可达自动开设置、同步 5xx 重试、字段序不兼容、公钥抽取失败。

---

### Epic 2 — 查看资产与活动（Home）
> 入口 `/(tabs)/wallet` → `HomeScreen`。核心不变量：**总额永不因单链失败而错误地显示更小的数**（`max(live, cached)` + `balancePartial` 警示）。

| US | 用户故事 | 验收主线 | 关键失败/边界态 | 核心模块 | 依赖 | 层 | 现状 |
|----|---------|---------|----------------|---------|------|----|----|
| 2.1 | 作为持有者，我打开 app 立刻看到余额和活动 | 先用本地缓存瞬时绘制→后台多链流式拉取→按链合并 | 冷启动无缓存空态；账户切换中途(addressRef 守卫) | M4(wallet-api,balance-cache),M6(activity) | D1 | SI + E2E | SI⚠️ |
| 2.2 | 作为持有者，某条链 RPC 挂了我仍看到可信的总额 | 单链失败→加入 `failedChainIds`→总额取 `max(live,cached)`；**持久故障**才弹换-RPC banner，**限流(transient)只留缓存、不弹 banner** | 全部链失败；token 无喂价→partial；限流 vs 硬故障区分 | M4,M5(rpc-pool `getRateLimitedChains`) | D1,D4 | U+E2E | ✅ rpc-pool-ratelimit + fault-injection 单测 + parallel-rate-limit e2e（平行空间，经 `__velaRpcState` 断言分类区分） |
| 2.3 | 作为持有者，我下拉刷新 / app 自动刷新 | VelaRefresh 手势(spinner 保持 650ms)；前台每 10min 自动刷 | 刷新中断网→保留旧值不清零 | M4 | D1 | C/M | 待 |
| 2.4 | 作为持有者，我查看某 token 明细与历史 | TokenDetail：余额、喂价、历史；Activity 列表 ENS/vela 名解析 | 无喂价 token；名解析失败→显示裸地址 | M4,M6,M7(recipient-identity) | D1,D3,D4 | SI + E2E | ⚠️ |
| 2.5 | 作为持有者，pending 的发送在我打开 app 时被对账 | Home 加载时轮询 bundler receipt→pending→confirmed/failed | receipt 超时→保持 pending 不误判失败 | M6(tx-reconciler) | D2 | SI | ✅ |

**Epic 2 失败态：** 单链 RPC down、全链 down、token 无价、账户切换竞态、reconcile 超时、活动名解析失败。（`vela.failRpc('all')`、`vela.slowRpc(ms)`、`vela.nullPrice('all')`）

---

### Epic 3 — 发送资金（Send，含 split / sweep）
> 入口 `/send` → `SendScreen` + `components/send/*` + `components/contacts/*`。三步：选 token → 填详情 → 确认。核心不变量：**估真实交易再签（绝不盲发未估算的大额 op）**（见 memory：dApp/Send UX parity）；**滑动/长按确认防误触**。

| US | 用户故事 | 验收主线 | 关键失败/边界态 | 核心模块 | 依赖 | 层 | 现状 |
|----|---------|---------|----------------|---------|------|----|----|
| 3.1 | 作为用户，我给一个地址/联系人发单笔 token | 选 token→填收款人(地址/ENS/联系人/QR)+金额→确认(余额变化模拟+风险标签)→passkey 签→回执带 explorer 链接 | 余额不足；名解析失败→裸地址；QR 脏数据 | M2(safe-tx),M3(tx-simulation),M7 | D1,D2,D6 | SI+E2E+M | SI✅ |
| 3.2 | 作为用户，ERC-20 gas 不够时我被引导先充值 gas 账户 | bundler underfunded→FundingModal 充 EntryPoint→自动重试 | underfunded 文案跨仓匹配(见 memory：bundler 耦合)；充值不可逆 | M2(bundler-service) | D2 | SI + M | ✅ parse |
| 3.3 | 作为用户，在 Tempo(4217) 上我用稳定币付 gas | 无原生币；gas 以 TIP-20 稳定币结算；余额检查针对 fee token | fee token 不足；地址/EntryPoint 一致性 | M2(safe-tx tempo) | D2 | U + M | U✅ |
| 3.4 | 作为用户，我把 1 个 token 拆发给 N 个人（split） | MultiRecipientEditor 多行→1 个 MultiSend UserOp→批量回执分行 | 某行地址/金额非法；行增删 | M2(batch-send) | D2 | U + E2E | U✅ |
| 3.5 | 作为用户，我把某链上的多个 token 全额扫给 1 个地址（sweep/multiSelect） | 从资产 sheet 预选→1 个 MultiSend N 笔 ERC20→全额 | 与 split 互斥；全额取整 | M2(batch-send) | D2 | U + E2E | U✅ |
| 3.6 | 作为用户，我扫/粘一个 EIP-681 支付请求发款 | 解析 URI→锁定收款人/金额→缺网络走"加网络"、缺 token 造零余额占位 | 网络未知、token 未知、金额/地址非法 | M7(eip681),M5(add-network) | D1 | U + E2E | U✅ E2E✅ |
| 3.7 | 作为用户，收款人有风险时我被要求更谨慎地确认 | 首次/合约地址→风险标签(First time/Contract)→**滑动**确认(SlideToConfirm，danger 变红) | 首次转账警告、合约 vs EOA | M3,M7(recipient-risk) | D1 | E2E + M | ✅ E2E(send-high-risk) |

**Epic 3 失败态：** 余额不足、原生 gas 不足、Tempo fee token 不足、bundler underfunded、gas 估算失败(best-effort)、模拟失败(无预览)、passkey 取消、签后 bundler 报错(回执转轮询)、EIP-681 三类非法。（RPC 类用 `vela.failRpc/slowRpc`；bundler underfunded 需手动断网/指向不可达端点）

---

### Epic 4 — 收款 / 付款链接（Receive & Pay）
> `/receive`（地址 QR + 请求构建器）、`/pay`（公开支付链接桥）。核心不变量：**地址中毒防御**——收款地址 QR 前有"确认地址正确"遮罩须手动消除。

| US | 用户故事 | 验收主线 | 关键失败/边界态 | 核心模块 | 依赖 | 层 | 现状 |
|----|---------|---------|----------------|---------|------|----|----|
| 4.1 | 作为收款人，我出示地址二维码收款 | 200×200 地址 QR + 支持网络网格 + 复制/存图；防中毒遮罩 | 无地址(未建钱包)→"No address"；存图权限被拒→提示 | M7(qrcode),M10(share-card) | D6 | U + M | QR U✅ |
| 4.2 | 作为收款人，二维码打开时到账被实时探测 | 3s×1min 快轮询→60s×4min 慢轮询→停；diff baseline→haptic+行内到账；decode 只认收款人=本钱包的日志(反伪造) | 探测 RPC 失败→静默跳过本轮；5min 后停；RPC 返回他人日志→拒绝 | M4(fetchTokens),M6(transfer-monitor decode) | D1 | U + M | ✅ U(decode 反伪造/spam/native) |
| 4.3 | 作为收款人，我生成一个带金额的 EIP-681 请求 | 构建器：链/token/金额→生成 URI→QR 编码→人类可读摘要 | 参数非法 | M7(eip681) | — | U | ✅ |
| 4.4 | 作为付款人，我打开一个 Vela 支付链接付款 | Pay 页显示 token/金额/收款人/链；三路径：Open in Vela(锁定 Send)/另一钱包 QR/复制字段 | 无效链接→"Invalid payment link"；网络未知→Open in Vela 禁用 | M7(eip681) | D1 | U + E2E | eip681 E2E✅ |

**Epic 4 失败态：** 未建钱包无地址、存图权限拒、到账探测 RPC 失败静默、探测 5min 超时停、无效支付链接、支付链接网络未知。

---

### Epic 5 — 连接 dApp 并签名（Connect & Sign）
> `/(tabs)/connect` → `ConnectScreen` + `components/signing/*`（SigningSheet 单一渲染路径，prod 与 harness 共用）。核心不变量（P0）：**无限授权永不放行**（approval-guard）；**签名请求提交即持久化，关闭/重载不丢**（见 memory：connection-activity replay）。

| US | 用户故事 | 验收主线 | 关键失败/边界态 | 核心模块 | 依赖 | 层 | 现状 |
|----|---------|---------|----------------|---------|------|----|----|
| 5.1 | 作为用户，我扫码/粘链接连上一个 dApp | 扫 QR/粘链接→指纹交换→连上显示 dApp 卡+账户+链+E2E 徽章(WalletPair) | 无效配对链接；连接失败→重试；重连卡住→banner+手动重连 | M8(dapp-transport/walletpair) | D5 | U + M | ✅ U(parseURL + connect SSE 状态机) |
| 5.2 | 作为用户，dApp 请求交易时我看到人类可读意图（clear signing） | descriptor 命中→意图布局(动作词+token 卡+箭头+折叠详情)；无 descriptor→盲签回退(合约+calldata) | descriptor 解析失败→盲签；未知选择器 | M3(clear-signing,abi-decode,selector) | D3,D7 | U + E2E | U✅ E2E✅ |
| 5.3 | 作为用户，任何无限授权都被拦下、必须我设有限额度 | approve/increaseAllowance/setApprovalForAll/permit/Permit2 皆检出→额度可编辑→**无限时确认禁用**，无旁路(含滑动) | 粘贴 uint256-max / 2^255 / 2^160-1 仍被拦 | M3(approval-guard) | — | U + M | U✅(P0) |
| 5.4 | 作为用户，签名前我看到链上会发生什么（余额变化预览） | eth_call 回滚预检 + 资产净变化预览 + 收款人风险标签 | 模拟失败→无预览(best-effort)；received token 只 gate 不信 log(见 memory：非对称信任) | M3(tx-simulation,sim-assets) | D1 | SI + M | ✅ |
| 5.5 | 作为用户，签名/交易被记录，我能事后只读重放 | 签名落 dApp 历史(全量 request+response)→Connections 面板事件(pending→confirmed/failed)→点开只读重放 SigningSheet | 关闭/重载不丢 pending；签后 bundler 超时→回执转轮询 | M6(dapp-history) | D6,D2 | SI + M | ✅ |
| 5.6 | 作为用户，读取类请求不被 dApp 打爆（只读限流） | readonly-rpc-gate：6 并发/512 排队；溢出→-32005 可重试 | 队列溢出 | M5(readonly-rpc-gate) | D1 | U | ✅ |

**Epic 5 失败态：** 无效配对链接、连接失败、重连卡住、descriptor 解析失败盲签、模拟失败无预览、passkey 取消→请求自动拒、签名失败弹错、bundler 超时回执粘滞、只读队列溢出。

---

### Epic 6 — 设置 / 本地化 / 网络 / 反馈（Settings）
> `/settings` → `SettingsScreen`、`RpcProvidersModal`、`AboutScreen`。

| US | 用户故事 | 验收主线 | 关键失败/边界态 | 核心模块 | 依赖 | 层 | 现状 |
|----|---------|---------|----------------|---------|------|----|----|
| 6.1 | 作为用户，我切换语言/货币/数字·日期格式 | 14 语种全量 i18n；货币经 Chainlink+FX；显式格式预设(US 1,234.56 / EU 1.234,56) | 缺翻译回退；zh-HK 口语粤语；无 ICU | M4(currency,fiat-*,locale-format) | D4 | U + C | ✅ format |
| 6.2 | 作为用户，我查看/切换/加自定义网络并看端点健康 | 12 默认+自定义；WS 用 eth_chainId、HTTPS 用 JSON-RPC POST 探健康+延迟 | 端点不可达红点；URL 非法校验；在用网络不可删 | M5(network-checker,add-network,rpc-pool) | D1 | SI + M | ✅ checker |
| 6.3 | 作为用户，我改 Vela 服务端点(passkey index/data API/bundler) | 端点可配 + 健康指示 + 重置默认 | 健康检查超时(10s)→离线 | M1,M4,M2 | D2,D3 | SI + M | ⚠️ |
| 6.4 | 作为用户，我切换账户 / 登出 / 清数据 | 账户切换器(带余额)；登出清本地账户重置状态；清数据(不可逆) | 清数据不可逆二次确认 | M9(wallet-state,storage) | D6 | U + M | **reducer 建补** |
| 6.5 | 作为用户，我一键反馈 bug（不吓人的"Feedback"入口） | 安静的 Settings "Feedback" 行→代理 `getvela.app/api/bug-report`(PAT 服务端)→失败回退预填 GitHub bug.yml | 503 not_configured→静默回退 URL；诊断已脱敏 | M10(bug-report,feedback,metrics) | D3 | U + M | ✅ |

**Epic 6 失败态：** 端点不可达、健康检查超时、自定义网络 URL 非法、在用网络删除、清数据不可逆、bug-report 后端未配置回退。

---

## 4. 覆盖矩阵与缺口（Gap Analysis）

**已很好覆盖（单元）：** 密钥推导/attestation/Safe 地址/eth-crypto、EIP-712/SIWE、approval-guard、clear-signing、tx-simulation/sim-assets、abi/abi-decode、selector-registry、batch-send、bundler-service、eip681、fiat-rates/fx、currency/locale-format、network、readonly-rpc-gate、storage、activity、tx-reconciler、contacts、recipient-identity、siwe 等 60+。

**明确缺口（按优先级）：**

| 优先级 | 缺口 | 归属 | 建议 |
|-------|------|------|------|
| ~~P0~~ ✅ | ~~`webauthn-verify.ts` 无测试~~ → 已补 `webauthn-verify.test.ts`（prefix/字段序/UV flag/长度/检查顺序，US 1.4） | M1 | 完成 |
| ~~P0~~ ✅ | ~~`wallet-state.ts` reducer 无测试~~ → 已补 `models/wallet-state.test.ts`（SET_WALLET/ADD/SWITCH/LOGOUT 资金隔离/不可变性，US 6.4）；reducer/INITIAL_STATE/WalletAction 已追加导出 | M9 | 完成 |
| ~~P1~~ ✅ | 组件/旅程交互层——**3/3 条 P0 旅程已建并实机验证**：✅ `onboarding-sync.spec.ts`(US 1.3)、✅ `approval-guard.spec.ts`(US 5.3)、✅ `send-high-risk.spec.ts`(US 3.7 高风险确认，mock 链上余额+合约) | 全 | 真实模式（非 `/test-*` 假路由）：真路由 + localStorage 播种(`vela.accounts`) + `page.route` 阻断/构造 JSON-RPC + **CDP 虚拟 WebAuthn 认证器** + **手编 aggregate3 余额 fixture**（用真实 `decAggregate3` 验证过） |
| ~~P1~~ ✅ | dApp transport（US 5.1）：✅ `parseRemoteInjectURL`（无效链接→null）+ ✅ `RemoteInjectTransport.connect` SSE 状态机（ready 解析/onerror 未ready拒绝/openTimer 超时/断连 emit）——`dapp-transport.test.ts` 13 tests | M8 | 完成（walletpair 重连边界可后续补） |
| ~~P1~~ ✅ | `wallet-api.ts`（US 2.1/2.2）：✅ 缓存/in-flight 去重/clearTokenCache（`wallet-api-cache.test.ts`）+ ✅ merge-by-chain（`wallet-api-merge.test.ts`：某链失败不清零健康链余额，aggregate3 fixture） | M4 | 完成 |
| ⬜ P1 | `walletpair-transport.ts` 重连边界（30s/60s，US 5.1）——深度耦合 walletpair-sdk（WebSocketTransport/session） | M8 | 需 walletpair-sdk mock harness；宜与 transport 测试(`remote-inject-transport.test.ts`)一并建，避免重复 |
| ⬜ P2 | `share-card.ts` 收款卡截图/保存/分享（view-shot/canvas，纯 IO） | M10 | 快照式测试 |
| ~~P1~~ ✅ | `transfer-monitor.ts`（US 4.2）：已补 decode 反日志伪造（topics[2]≠本钱包→拒绝）+ <3topics/零值 spam 过滤 + from/value/token 提取 + EIP-7708 native 分类（9 tests） | M6 | 完成 |
| ~~P2~~ ✅ | `public-key-upload`（US 1.2「verify 为准」四象限 + validateCreateClientData + retry）→ 已补 `public-key-upload.test.ts` | M1 | 完成 |
| 🟡 P2 | ✅ `tokens.ts`、✅ `price-service.ts`(别名+回退)、✅ `platform.ts`(web 回退：showAlert/copyToClipboard，`platform.test.ts`)；⬜ 剩 `share-card.ts`(view-shot/canvas，IO 快照) | M4/M10 | 剩 share-card 用快照 |
| **P3** | `sim-engine-tevm.ts` 刻意禁用 | M3 | 保持可选，不强测 |

---

## 5. 测试分层策略与工具映射

| 层 | 工具 | 跑什么 | mock 什么 | 命令 |
|----|------|--------|----------|------|
| 单元(U) | Jest + ts-jest (`testEnvironment: node`) | §2 所有 [纯] 逻辑 + IO 边界注入 | fetch/AsyncStorage/native | `npm test` / `npm run test:watch` |
| 服务集成(SI) | Jest | 一个用户故事跨多模块(mock 依赖) | D1-D8 在注入点 | `npm test` |
| 组件(C) | *(待建)* Playwright + `/test-*` dev 路由 | 真组件 + mock 数据经 props/context 注入，绕过 passkey/网络 | 全部依赖 | — |
| E2E | Playwright (Web) | 关键旅程端到端；`/clear-signing-test` 逐场景 | 视需要 | `npm run test:e2e` / `:headed` |
| 手动(M) | 真机 iOS/Android | P0 资金/生物识别/相机/深链/云同步 | — | MANUAL-TEST-100-CLUES.md |

**Web 上测 passkey：** Chrome DevTools → WebAuthn 面板启用 virtual authenticator（无真机时）。

---

## 6. 故障注入专项（失败态验证）

失败态是钱包一等公民。用 `src/services/dev/fault-injection.ts` 的 `vela.*` console 命令覆盖每个 Epic 的失败清单：

以下为 `fault-injection.ts` 的**真实** console 命令（`vela.help()` 可打印）：

| 场景 | 命令 | 覆盖故事 |
|------|------|---------|
| 单链 / 全链 RPC 失败（同时停活动，同一 RPC 层） | `vela.failRpc(chainId \| 'all')` | US 2.2, 3.1, 4.2 |
| RPC 限流(429)——**临时**：余额留缓存、**不弹**换-RPC banner | `vela.rateLimitRpc(chainId \| 'all')` | US 2.2 |
| RPC 加延迟 | `vela.slowRpc(ms)` | US 2.3, 5.4 |
| 随机丢包 | `vela.flakyRpc(rate 0..1)` | US 2.1, 5.1 |
| token 有余额但无喂价（总额少算） | `vela.nullPrice(chainId \| 'all')` | US 2.2, 2.4 |
| 复位所有故障 / 看当前故障 | `vela.clear()` / `vela.status()` | 全 |

> bundler underfunded（US 3.2）与后端同步失败（US 1.2/1.3/6.5）当前 **不在** `vela.*` 覆盖内——需手动断网或指向不可达端点验证，或按需扩展 `fault-injection.ts`。

---

## 7. 执行路线图（建议顺序）

1. **补 P0 单测缺口**（低成本高价值）：`webauthn-verify`、`wallet-state` reducer。→ 补齐资金安全护城河的确定性验证。
2. **补 P1 核心地基**：`rpc-pool` fixture 测（可信总额）、dApp transport 桩（连接稳定性）。
3. **建组件/E2E 层**（✅ 完成）：三条 P0 旅程全部建成并实机验证 —— ✅ Onboarding 同步失败→账户不落地(US 1.3, `e2e/onboarding-sync.spec.ts`)、✅ Sign 无限授权门控(US 5.3, `e2e/approval-guard.spec.ts`)、✅ Send 高风险确认(US 3.7, `e2e/send-high-risk.spec.ts`)。US 3.7 通过 mock JSON-RPC 实现：`page.route` 构造 aggregate3 余额响应（让 token picker 有可花的原生币）+ `eth_getCode` 返回字节码（判定合约）+ 阻断 bundler（`checkBundlerFunding` 返回 null 放行到确认步）。此 RPC-mock fixture 可复用给 Epic 2/3 更多余额相关旅程。
4. **失败态回归**：把 §6 每条 `vela.*` 场景固化成 e2e 断言，纳入 CI。
5. **手动真机**：每次发版按 MANUAL-TEST-100-CLUES.md 过 P0×25。

---

*本大纲随源码演进；改动核心模块或用户旅程时同步更新对应 Epic/矩阵行。*
