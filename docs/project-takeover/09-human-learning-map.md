# 09 — 人类学习知识地图 (Human Learning Map)

> 基线 commit: `73d7aac`（2026-07-02）。本图为人类接管训练服务，与审计文档 01–08 互补：01–08 回答"系统是什么"，本文回答"按什么顺序、学到什么深度"。文档与源码冲突时以源码为准。

## 知识域划分（诊断/评分用同一套编号）

| # | 知识域 | 目标等级 | 核心证据文件 |
|---|--------|---------|-------------|
| D1 | 项目目的/商业模型/核心用户流程 | L4 | 01-system-overview.md, WHITEPAPER.md |
| D2 | 代码结构与模块边界 | L4 | src/ 布局, 01 仓库布局节 |
| D3 | 密码学身份链（passkey→P-256→Safe 地址） | L5 | src/modules/passkey/, services/attestation-parser.ts, safe-address.ts, public-key-upload.ts |
| D4 | 交易提交链（sendUserOp / 4337 / bundler） | L5 | services/safe-transaction.ts, bundler-service.ts, tempo.ts |
| D5 | dApp 连接与签名安全（SigningSheet / approval-guard / clear-signing / 模拟） | L5 | hooks/use-dapp-signing.ts, services/approval-guard.ts, clear-signing.ts, tx-simulation.ts, sim-engine-rpc.ts |
| D6 | 链上读地基（rpc-pool / wallet-api / 余额缓存） | L4 | services/rpc-pool.ts, wallet-api.ts, balance-cache.ts |
| D7 | 状态与持久化（AsyncStorage vela.* / models Context / activity） | L4 | services/storage.ts, models/, services/activity.ts |
| D8 | 框架与运行时（Expo/RN/expo-router/Hermes/Web 差异/polyfills） | L4 | src/app/_layout.tsx, polyfills*.ts, metro.config.js, app.json, plugins/ |
| D9 | 测试体系（Jest 79 套件 / Playwright / parallel-space fixture / 故障注入） | L4 | jest.config.js, e2e/, src/services/dev/ |
| D10 | 安全边界（never-unlimited / SIWE / 读方法白名单 / fixture 泄露风险 / xlsx 供应链） | L5 | approval-guard.ts, readonly-rpc-gate.ts, siwe.ts, 04 审计 P1 表 |
| D11 | 生产运维（构建/部署/CF Pages/getvela.app Workers/CI/回滚/监控） | L5 | 05-deployment-runbook.md, 06-operations-runbook.md, .github/workflows/ci.yml, scripts/ |
| D12 | 外部服务依赖与故障域（bundler / p256-index / getvela.app 代理 / WalletPair relay） | L5 | bundler-service.ts, public-key-index.ts, walletpair-transport.ts, 08-open-issues.md |
| D13 | 架构取舍（无 viem 手写 ABI / 无 .env / counterfactual 部署 / bundler 价格权威 / 单代码库跨端） | L5 | 03 末表, 13-architecture-decisions.md（待建） |

## 知识前置依赖

```
D1 ──> D2 ──> D3 ──> D4 ──> D5
              │       │      │
              └── D6 ─┴──> D7
D8 独立可并行（但 D5 的 SigningSheet 渲染依赖 D8 基础）
D9 依赖 D2；D10 依赖 D4+D5；D11 依赖 D8+D9；D12 依赖 D4+D6；D13 最后（依赖全部）
```

## 决定理解全局的关键 20%

按投入产出排序（先读完这些，剩下的可按需查）：

1. **`src/services/safe-transaction.ts`**（全文）— 整个产品的心脏：MultiSend 编码、sendUserOp 十步链、SafeOp EIP-712 hash、签名格式转换。读懂它 = 读懂 4337 + Safe + 本项目手写 ABI 风格。
2. **`src/services/safe-address.ts`** — CREATE2 counterfactual 地址推导；解释了"为什么没部署也能收款"。
3. **`src/hooks/use-dapp-signing.ts`** — dApp 方法路由 + 两道 enforceNoUnlimited 兜底（:322/:367）；安全承诺落点。
4. **`src/services/approval-guard.ts`** — detect/rewrite/enforce 三件套；所有 approval 形态。
5. **`src/services/rpc-pool.ts`** — 7 级端点优先、评分、两级封禁、错误分类；所有链上读的地基。
6. **`src/services/bundler-service.ts`** — 报价权威原则、parseBundlerUnderfunded 跨仓库耦合（:367）。
7. **`src/app/_layout.tsx`** — 初始化顺序、__DEV__ 与 dev_unlocked 门控、ParallelSpaceBadge 无条件渲染（审计修复）。
8. **`src/services/storage.ts` + `src/models/`** — vela.* 键空间、两个全局 Context、require cycle 地雷。
9. **`docs/project-takeover/05/06`** — 部署与运维 runbook（本身就是浓缩好的运维知识）。

## 推荐阅读顺序（首轮）

01-overview → 03-core-flows →（对照源码）safe-address.ts → safe-transaction.ts → bundler-service.ts → use-dapp-signing.ts + approval-guard.ts → rpc-pool.ts → wallet-api.ts → _layout.tsx + storage.ts → 05/06 runbook → 04/08（风险与债务）

## 高频修改区 / 生产事故相关（面试深挖点）

- gas 定价链（历史回归重灾区：Gnosis "gas price too low"）
- parseBundlerUnderfunded ↔ vela-relay 文案耦合
- Tempo(4217) 稳定币 gas 报销批量（垫值 380k、2× 边际是实测值）
- i18n 键深 ≤3 段（14 语言静默 fallback）
- storage.ts ↔ models/types.ts require cycle
- fixture 私钥公开 + dev_unlocked 生产可达（已修徽章，但机制必须理解）
- xlsx 用户输入路径（已升级 0.20.3，供应链警觉性）
