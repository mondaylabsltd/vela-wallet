# 01 — 系统全景 (System Overview)

> 接管审计日期:2026-07-02。所有结论基于源码/配置/实际运行结果,证据以 `文件:行号` 标注。

## 一句话介绍

Vela Wallet 是一个 **passkey(P-256/WebAuthn)签名的 ERC-4337 智能合约钱包**(Safe v1.4.1 + EntryPoint v0.7),无助记词、无浏览器插件依赖,跨 Web / iOS / Android(Expo React Native 单代码库),支持 12+ EVM 链,通过自营 bundler 收取 relayer 费获利。

## 用户与商业模型

- 目标用户:多链活跃转账者(见 `docs/marketing/100-marketing-leads.md`)
- 收入:**Web 版免费**;**iOS/Android 商店版付费下载(定价未定,创始人决策中)**;叠加 bundler relayer 费(约 2×/3× gas 上限加价)
- 团队:单人创始人(开发/运维/发布同一人)

## 技术栈

| 层 | 技术 | 版本 | 证据 |
|---|---|---|---|
| App 框架 | Expo + React Native + expo-router | expo ~55.0.17, RN 0.83.6, React 19.2.0 | `package.json` |
| 语言 | TypeScript strict | ~5.9.2 | `tsconfig.json` |
| Web 部署 | 静态导出 → Cloudflare Pages | `expo export --platform web` | `package.json` scripts, `scripts/fix-cf-pages-assets.js` |
| 站点/API 后端 | SvelteKit on Cloudflare Workers(仓库内子项目) | SvelteKit ^2.50, wrangler 4.x, **bun** 管理 | `getvela.app/wrangler.jsonc` |
| 密码学 | 平台 passkey(Secure Enclave / Credential Manager / navigator.credentials),私钥永不进 JS | — | `src/modules/passkey/index.ts` |
| 链上账户 | Safe v1.4.1 + Safe4337Module + WebAuthn Signer + EntryPoint v0.7 | 合约地址硬编码(全链统一 CREATE2) | `src/services/safe-address.ts:19-28` |
| 单元测试 | Jest + ts-jest(node env) | 79 套件 / 1022 用例 | `jest.config.js` |
| E2E | Playwright(chromium,parallel-space fixture) | 15 spec | `playwright.config.ts`, `e2e/` |

**注意:没有 viem/ethers 依赖** —— ABI 编码、EIP-712、RPC、Multicall 全部手写(`src/services/abi.ts` 等)。改动这些底层时务必跑全量测试。

## 仓库布局(顶层)

```
src/                 钱包 App 本体(唯一 App 代码根)
  app/               expo-router 路由(12 个生产路由 + 4 个测试路由)
  screens/ components/ hooks/  UI 层
  services/          业务核心:safe-transaction, rpc-pool, bundler-service,
                     approval-guard, tx-simulation, clear-signing, wallet-api…
  modules/passkey/   跨平台 passkey JS 接口
  models/            wallet-state / dapp-connection 两个全局 Context
  i18n/              14 语言全覆盖
  __tests__/         79 个测试文件
getvela.app/         独立 SvelteKit 子项目:官网 + API 代理(bundler/bug-report/
                     exchange-rate/nft/og/proxy/transactions/wallet)+ .well-known
modules/vela-passkey/  原生模块源码(Swift/Kotlin),经 plugins/with-native-modules.js 注入
modules/vela-cloud-sync/ iOS iCloud KV(未接线,见 08-open-issues)
android/ ios/        原生工程(**不入库**,.gitignore:42-43 忽略;prebuild 生成物,持久化改动必须走 config plugin,见 plugins/)
chrome-ext-webauthn-proxy/  独立开发工具(passkey rpId 代理插件),不参与 App 构建
e2e/                 Playwright 测试
docs/                设计/需求/测试/上架文档(较全,见各文件)
```

## 关键外部依赖(运行时)

| 服务 | 用途 | 位置 |
|---|---|---|
| vela-relay(**独立仓库**) | 自营 4337 bundler,gas 报价权威 | `src/services/bundler-service.ts`;错误文案字符串耦合见 `parseBundlerUnderfunded` |
| p256-index.getvela.app(**独立仓库** biubiu-projects) | 公钥索引(跨设备恢复),CF Worker + D1 + DO 队列 | `src/services/public-key-index.ts` |
| getvela.app/api/* | Alchemy/Pimlico 代理、bug-report GitHub 代理 | `getvela.app/src/routes/api/` |
| 公共 RPC 池 | 每链多端点评分/封禁/故障转移 | `src/services/rpc-pool.ts` |
| WalletPair relay | dApp 连接 WebSocket 中继 | `src/services/walletpair-transport.ts` |
| Chainlink / DEX quoter | 价格 | `src/services/wallet-api.ts` |

## 关键数据流

1. **创建钱包**:passkey 注册 → 解析 attestation 提取 P-256 公钥 → CREATE2 推导 Safe 地址(未部署,首笔交易时部署)→ 公钥上传索引服务(create→verify→pending 重试队列,`src/services/public-key-upload.ts`)。
2. **发送交易**:构建 callData(单笔/MultiSend 批量)→ `sendUserOp`(`src/services/safe-transaction.ts:459`):verifyChainReady → 并行取 deployed/nonce/gas → bundler 报价优先定价 → bundler 估 gas(大 calldata 估算失败则拒绝提交)→ EIP-712 SafeOp hash → passkey 签名 → 提交 bundler(3 次重试,`[existingHash:]` 恢复)→ 乐观递增 nonce 缓存 → 轮询回执(自适应退避,120s)。
3. **dApp 签名**:WalletPair/SSE 收请求 → `use-dapp-signing.ts` 路由方法 → 交易模拟(eth_simulateV1)+ ERC-7730 clear signing + `enforceNoUnlimited`(`src/hooks/use-dapp-signing.ts:322,367`)→ 单一 `<SigningSheet>` 渲染 → 批准后走同一 sendUserOp 路径。
4. **状态持久化**:AsyncStorage,键均为 `vela.*`;账户仅存 credentialId/地址/公钥(无私密材料),交易历史 pending-at-submit 持久化。

## 配置体系

- **没有 .env / EXPO_PUBLIC_* 体系**:App 端全部配置为代码内常量 + AsyncStorage 用户覆盖(RPC/bundler/服务端点均可在设置中覆盖,`vela.serviceEndpoints`)。
- 构建元信息(版本+git commit):`app.config.js` 构建时求值注入 `extra.gitCommit`(CI 优先读 `CF_PAGES_COMMIT_SHA`/`GITHUB_SHA`,本地回退 `git rev-parse`),`src/constants/build-info.ts` 是从 `expo-constants` 读取的静态源码。(2026-07-02 前为 `scripts/generate-build-info.js` 生成物,旧机制每次构建弄脏工作区,已废除。)
- getvela.app 子项目:本地密钥在 `.dev.vars`(已 gitignore,从未入库——`git log --all` 验证),生产密钥走 `wrangler secret put`(GITHUB_BUG_TOKEN / ALCHEMY_API_KEY / PIMLICO_API_KEY)。

## 环境差异

| 环境 | 差异 |
|---|---|
| `__DEV__`(expo start) | 注册 /parallel 路由组、ParallelSpaceBadge、`vela.*` 故障注入控制台、passkey fixture 覆盖种子 |
| 生产 Web(expo export) | 上述 `__DEV__` 分支不激活;但测试路由文件仍在 bundle 中,靠运行时 `dev_unlocked` 门控(About 页 logo 6 连击解锁) |
| 原生 iOS/Android | passkey 走原生模块;rpId 固定 `getvela.app`;依赖 AASA / assetlinks.json 域名关联 |

## CI/CD

**不存在。** `.github/` 只有 issue 模板。构建、测试、部署全部手动。详见 `04-production-readiness.md` 与 `08-open-issues.md`。
