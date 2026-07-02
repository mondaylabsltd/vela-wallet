# 07 — 维护指南 (Maintenance Guide)

## 修改任何东西之前

1. `git pull` + 全绿基线:`tsc --noEmit` / `expo lint`(0 errors)/ `jest --ci` / `playwright test`
2. 判断改动落在哪个风险区(下表),按对应验证清单执行
3. 小步提交;money-path 改动必须带回归测试(仓库惯例如此,保持)

## 风险分区与验证要求

### 🔴 红区(错一字节 = 资金/安全事故)

| 模块 | 改动前必读 | 必须验证 |
|---|---|---|
| `safe-transaction.ts`(SafeOp hash/签名编码/MultiSend/initCode) | 03 文档 §2;全部手写无 viem 兜底 | `jest safe-transaction` + parallel space 真实链上小额发送 |
| `approval-guard.ts` | "never unlimited" 是产品承诺;上限 1<<200 / 1<<152 | `jest approval-guard` + e2e approval-guard.spec + clear-signing 场景页 |
| `use-dapp-signing.ts` | 所有新签名路径必须过 `enforceNoUnlimited`(:322,:367)与模拟 | `jest dapp-signing*`(4 个套件)+ e2e parallel-dapp |
| `modules/passkey/` + `modules/vela-passkey/`(原生) | attestation/clientDataJSON 字段顺序是 Safe 合约级约束 | 真机 iOS+Android 创建/签名;`validateCreateClientData` 不许放宽 |
| gas 定价链(`getBundlerGasQuote`/`isQuoteAbusive`/tier) | **bundler 是权威**,钱包 RPC 永不否决报价;Gnosis 是回归高发链 | `jest bundler-service` + Gnosis 真实估算 |
| `tempo.ts` + Tempo 分支 | 报销 transfer 位置/380k 垫值/2× 边际是实测值 | Tempo 测试网真实发送 |

### 🟡 黄区(坏了核心体验)

- `rpc-pool.ts`(评分/封禁):跑 `jest rpc-pool` 两个套件;`vela.rateLimitRpc` 手测 429 静默
- `wallet-api.ts`(余额合并):**失败链绝不清零** —— `jest` merge-by-chain 用例是底线
- `walletpair-transport.ts` / `dapp-connection.tsx`:e2e parallel-dapp + 真实 dApp 连接
- `activity.ts`:pending-at-submit 持久化语义不能破坏(关页不丢交易)
- i18n:新 key ≤3 段深;14 语言全补(缺失会渲染键名);`resources.ts` 记得接 namespace

### 🟢 绿区(UI/文案/文档)

设计语言约束仍在:de-container、hairline 分割、开放式 hero、弱化 $(`docs/DESIGN-LANGUAGE.md`)。动画/手感参考 `feedback_premium_custom_ux`(自定义反馈优于原生默认)。

## 跨仓库耦合(改之前想到另一边)

| 本仓库 | 对端 | 耦合物 |
|---|---|---|
| `parseBundlerUnderfunded`(bundler-service.ts:367) | vela-bundler handlers.ts | **错误文案字符串** |
| gas 报价消费逻辑 | vela-bundler 报价接口 | `pimlico_getUserOperationGasPrice` 语义/markup |
| `public-key-index.ts` | p256-index(biubiu-projects 仓库) | API 契约 + Idempotency-Key 格式 |
| `.well-known` 两个路由(getvela.app) | iOS entitlements / Android 签名指纹 | rpId=getvela.app、Team ID、SHA-256 指纹 |
| walletpair-sdk 版本 | WalletPair relay | 协议版本 |

## 测试策略

- **单测**(`src/__tests__/`,79 套件):纯逻辑,node env,`__DEV__=true`(fixture seam 需要);新增 money-path 代码 → 必须同 PR 带测试
- **网络集成**:`RUN_NETWORK_TESTS=1 npx jest price-query` —— 只在怀疑链上接口变更时手跑
- **E2E**(Playwright,parallel space):新用户流程 → 新增 spec;断言**必须限定在语义文本/元素**,不要对整页 body 做否定匹配(2026-07-02 的教训,见 04 §P1-5)
- **真机**:passkey/相机/键盘类改动无法在 Web 验证,按 `docs/NATIVE-LAUNCH-CHECKLIST.md` D 节执行

## 依赖管理

- 锁文件必须提交;`npm audit` critical/high 保持 0(当前基线 11 moderate = expo 工具链,升 expo 大版本时一并消)
- `xlsx` 用的是 SheetJS 官方 CDN tarball(0.20.3,npm registry 版本停更在 0.18.5 有漏洞)——升级时继续用 `https://cdn.sheetjs.com/xlsx-<ver>/xlsx-<ver>.tgz`
- expo SDK 升级是大动作:单独分支,全量真机回归

## 生成物与禁改清单

- `src/constants/build-info.ts`:脚本生成,别手改
- `android/` `ios/`:部分由 `expo prebuild` + `plugins/with-native-modules.js` 管理;手改原生文件前确认它不会被 prebuild 覆盖(签名配置、Manifest 我们已手改,重跑 prebuild 时需比对 app.json 的 `allowBackup: false` 是否保留了语义)
- `dist/`:构建产物
