# 04 — 生产就绪审计 (Production Readiness Audit)

> 审计日期 2026-07-02。方法:源码/配置全面探查 + 全部检查命令实际执行 + 关键声明逐条亲自复核(含证伪一例:子代理称 `.dev.vars` 密钥"已提交入库",经 `git log --all` 验证从未提交)。

## 成熟度定性

**不是 Demo,不是原型。** 这是一个工程质量高于多数上架钱包的准生产应用:1022 个单测、63 个 E2E、幂等/重试/降级链路完整、无 mock 数据渲染给用户、无秘密入库。差距集中在**发布工程**(签名/商店流程/设备实测)而非产品代码。

## 审计发现与状态

### P0 — 无

未发现任何可致资金损失(在正常使用路径上)、密钥泄露或数据损坏的缺陷。

### P1(上线前必须解决)

| # | 问题 | 证据 | 触发条件 | 影响 | 状态 |
|---|---|---|---|---|---|
| 1 | **生产构建中 Parallel Space 无徽章 + 重载后不重武装**。`_layout.tsx` 把 `<ParallelSpaceBadge/>` 和 `applyParallelSpaceOnBoot()` 都锁在 `__DEV__` 后,但 `/parallel` 靠运行时 `dev_unlocked`(About logo 6 连击)在生产可达,fixture 私钥公开(源码+bundle 均含,已实测 `grep dist/_expo`) | `src/app/_layout.tsx:101,135`(修复前);`src/services/dev/passkey-fixture.ts` | 生产构建 → 6 连击 → /parallel → (可选)刷新 | 用户在无任何标识下持有 fixture 钱包;Receive 展示"任何人可花费"的地址 → 真实资金误入即被盗 | ✅ **已修复**:徽章无条件渲染(自我门控),boot 重武装无条件执行(真实空间下仅一次存储读) |
| 2 | **根 typecheck 失败(52 错)**:`tsconfig.json` include `**/*.ts` 吞入 getvela.app 子项目(SvelteKit 生成型 `$types` 缺失) | 修复前 `npx tsc --noEmit` exit 2 | 任何人跑类型检查 | 无可用类型门禁,真回归会被 52 个噪音淹没 | ✅ **已修复**:显式 exclude;复验 exit 0;子项目用自己的 `bun run check`(实测 0 errors) |
| 3 | **QRScanner 条件调用 Hooks(3 个 ESLint error)**:`ScanLine` 在 `Platform.OS==='web'` 早退后调用 hooks | `src/components/QRScanner.tsx:145-153`(修复前) | lint 门禁;运行时因 Platform.OS 恒定而未爆发,但属脆弱违例 | lint 无法作为门禁 | ✅ **已修复**:hooks 移入 `NativeScanLine`;复验 lint 0 errors |
| 4 | **单测套件非确定性**:`price-query.test.ts` 直连第三方公共 RPC,审计当天即因 zan.top 限流失败(1021/1022) | 首轮 `npx jest` 输出;测试文件头部自述 "real RPC" | 第三方限流/抖动 | `npm test` 结果不可信,掩盖真回归 | ✅ **已修复**:改为 `RUN_NETWORK_TESTS=1` opt-in(未删除、未弱化);默认跑 78 套件全绿 |
| 5 | **E2E 断言缺陷**:`clear-signing.spec.ts:93` 对整页 body 断言 `not.toMatch(/Unlimited/)`,被背景场景列表标题 "(Unlimited)" 误杀(App 行为正确,modal 正确显示 500 USDC 上限) | 首轮 E2E 失败截图/trace | 场景列表含 "(Unlimited)" 字样 | 假红 | ✅ **已修复**:断言改为精确警告句 + 正向断言 "500 USDC";复验通过 |
| 6 | **Android release 用 debug keystore 签名**(checklist A1):Play 重签后 assetlinks 指纹不匹配 → **passkey 在 Android 生产版直接不可用** | `android/app/build.gradle:112-115`(修复前);`getvela.app/.../assetlinks.json` 释放指纹为占位 | 上架 Google Play | Android 版核心功能(passkey)全灭 | 🟡 **代码侧已完成**:注意 `/android` 与 `/ios` 被根 .gitignore 忽略(prebuild 生成物),因此持久修复走 **config plugin** `plugins/with-release-signing.js`(已注册进 app.json,离线验证 6/6 断言 + 幂等),从 `android/keystore.properties` 读取签名配置,无文件时回退 debug 并打警告;example 在仓库根 `keystore.properties.example`。当前本地 android/ 也已同步手改。**剩余人工步骤**(见 08):生成 upload keystore、注册 Play App Signing、把两枚真实 SHA-256 写入 assetlinks.json、Statement List Tester 验证 |
| 7 | **`xlsx` 0.18.5 高危漏洞在用户输入路径**:payroll 导入把用户提供的工作簿字节交给带原型污染(GHSA-4r6h-8v6p-xvw6)+ ReDoS(GHSA-5pgg-2g8v-p4x9)的解析器;npm registry 版本停更无修复 | `src/services/recipient-table.ts:159`(`import('xlsx')` 解析 `parseWorkbook`);`npm audit` | 用户导入恶意构造的 .xlsx(如伪装的"工资表模板") | 客户端 DoS/逻辑污染(资金侧仍有 SigningSheet+passkey 把关) | ✅ **已修复**:升级到 SheetJS 官方 CDN 0.20.3(两条 advisory 均修复,API 兼容);`jest recipient-table` 16/16 通过;`npm audit` critical/high 归零 |

### P2(可延期,需有控制措施)

| # | 问题 | 证据 | 状态 |
|---|---|---|---|
| 1 | `allowBackup="true"`(钱包惯例应为 false;AsyncStorage 虽无私钥但含账户/历史元数据) | `AndroidManifest.xml:20`;checklist B2 | ✅ 已改 false(manifest + app.json 同步);若日后上 cloud-sync 需重新评估 |
| 2 | 已部署钱包 nonce 获取失败回退 `0x0` 提交 → 浪费 passkey 弹窗 + 晦涩 AA25 错误 | `safe-transaction.ts:476,486`(修复前) | ✅ 已改为签名前快速失败(可重试错误文案) |
| 3 | **无 CI**:`.github/` 仅 issue 模板,一切门禁靠手跑 | 目录实查 | ✅ 已添加 `.github/workflows/ci.yml`(app: tsc/lint/jest/build:web;site: svelte-check)。E2E 暂留本地,待跑稳后进 CI。**未验证**:workflow 需 push 后首跑确认 |
| 4 | getvela.app API 代理(bundler/wallet/nft/transactions)带服务端 Alchemy/Pimlico key,无速率限制,CORS 只防浏览器 | `getvela.app/src/hooks.server.ts`(自述"curl 可达");各 +server.ts 无 rate limit(bug-report 除外) | ⬜ 未修。控制措施:Cloudflare 侧配 WAF/rate-limit 规则(运维操作,见 06);key 消耗有提供商侧上限告警可兜底 |
| 5 | `/api/proxy` SSRF 黑名单有缺口(十进制 IP、IPv6-mapped、169.254、DNS rebinding) | `getvela.app/src/routes/api/proxy/+server.ts:5-30` | ⬜ 未修。Workers 运行时无内网/元数据服务可打,实际影响=开放代理滥用;与 P2-4 一并用 CF 规则控 |
| 6 | 公钥索引服务是跨设备恢复单点(独立仓库,本审计范围外) | `src/services/public-key-index.ts`;memory: CF Worker+D1+DO | ⬜ 文档化(见 06/08):需确认 D1 备份策略 |
| 7 | bundler 错误文案跨仓库字符串耦合 | `bundler-service.ts:367-385` ↔ vela-relay handlers.ts | ⬜ 已有单测钉住钱包侧;改文案须两仓同步(03/07 已写明) |

### P3(技术债/优化)

- `deployer-api.ts` 含 3 个 TODO 的 keccak 派生 mock —— **无生产引用**(grep 验证,仅 format-eth.ts 注释提及),属休眠代码,建议删除或接真实 API
- `wallet-api.ts` 62 处 / `walletpair-transport.ts` 15 处 console.log(带模块前缀、无敏感信息;生产可考虑日志开关)
- `wallet-api.ts`、`walletpair-transport.ts` 无直接单测(有 E2E 间接覆盖)
- Metro 警告:`storage.ts ↔ models/types.ts` require cycle;`shadow*` 样式弃用警告
- 167 个 lint warning(风格类:Array<T>、require imports、未用变量)
- `docs/test-plan.md` 数字过期(称 263 测试,实际 1022);README 数据过期(链数/费率,见 memory)
- iOS `vela-cloud-sync` 模块未接线但 entitlement 已声明(checklist B1,建议移除)
- iOS 重复注册 passkey 无 `excludeCredentials` 防护(checklist B3,产品决策)

## 分项评估

| 维度 | 结论 |
|---|---|
| 功能完整性 | ✅ 无占位页/假接口;生产路径 TODO 为零(仅休眠 deployer-api);异常/边界路径覆盖异常充分(降级/重试/幂等随处可见) |
| 安全 | ✅ 私钥不出平台认证器;无秘密入库(git 历史验证);approval never-unlimited 双保险;SIWE 钓鱼检测;SSRF 代理有基本防护(P2 缺口见上);依赖 `npm audit`:见 05 验证记录 |
| 数据可靠性 | ✅ 客户端为主(AsyncStorage,无服务端用户数据库);pending-at-submit 持久化;幂等键覆盖创建/赞助;⚠️ 跨设备恢复依赖索引服务(P2-6) |
| 稳定性 | ✅ RPC 池评分/封禁/转移;bundler 3 重试+existingHash 恢复;回执自适应退避;字体加载 3s 兜底;merge-by-chain 防清零 |
| 可观测性 | ⚠️ 薄弱项:无远程错误上报/指标(客户端 App 可接受,但 getvela.app Worker 建议开 CF analytics/logpush);一键 bug-report 通道已建 |
| 工程质量 | ✅ 1022 单测+63 E2E+类型/风格门禁(本次修通)+新增 CI;⚠️ E2E 未进 CI;文档大体优秀但个别过期 |

## 实测验证记录(最终轮,2026-07-02)

| 命令 | 结果 |
|---|---|
| `npx tsc --noEmit` | ✅ exit 0(修复前 exit 2 / 52 错) |
| `npx expo lint` | ✅ 0 errors / 167 warnings(修复前 3 errors) |
| `npx jest --ci` | ✅ 78/78 套件、1007 通过 + 15 skip(price-query 套件 opt-in;修复前 1 失败/非确定性) |
| `npx jest recipient-table`(xlsx 升级后) | ✅ 16/16 |
| `npm run build:web` | ✅ exit 0,dist ~11MB,CF Pages 资产修正 OK(依赖变更后重建复验) |
| `npx playwright test` | ✅ **62 通过 / 1 跳过 / 0 失败**(2.5 分钟;修复前 61/1/1)——在全部代码与依赖变更之后跑的最终轮 |
| `cd getvela.app && bun run check` | ✅ 0 errors / 4 CSS warnings(650 文件) |
| `npm audit` | ✅ critical/high/low 归零;剩 11 moderate 全在 @expo/* 构建工具链(修复前 1 critical + 2 high + 14 moderate + 1 low) |
| config plugin 离线验证 | ✅ `plugins/with-release-signing.js` 对原始模板 6/6 断言 + 幂等;`npx expo config --type prebuild` 解析成功 |
| 原生 iOS/Android 构建 | ❌ **本审计未执行 = 未验证**(需 Xcode/模拟器长流程+真机 passkey);依据 `docs/NATIVE-LAUNCH-CHECKLIST.md` D 节清单执行,git 历史显示 2026-07-01 有 native 修复轮 |
| 生产域 Smoke(wallet.getvela.app) | ❌ **未执行 = 未验证**(部署是不可逆的对外操作,留给发布流程;步骤见 05) |
