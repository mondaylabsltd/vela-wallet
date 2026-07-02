# 02 — 本地开发 (Local Development)

## 前置

- Node(无 engines 约束;审计时使用系统 Node,npm lockfileVersion 3 ⇒ 需 npm ≥ 9)
- 原生开发:Xcode(iOS)/ Android Studio;`getvela.app` 子项目需要 **bun**
- 无需任何私有凭据即可开发 App 本体(RPC 走公共池;bundler 走内置 `getvela.app/api/bundler` 代理)

## 从零启动

```bash
npm install                 # 根项目
npm run web                 # Web 开发(expo start --web,默认 :8081)
npm run ios / npm run android   # 原生 Debug(模拟器;等价 npx expo run:ios / run:android)
```

`prestart` 钩子自动生成 `src/constants/build-info.ts`(git commit + 版本),该文件属生成产物,改动无须提交。

## 原生构建与真机运行

`/android` 与 `/ios` 是 `expo prebuild` 生成物(gitignored)。新 clone 或改过 app.json/plugins 后先重新生成:

```bash
npx expo prebuild --clean          # 重新生成 android/ + ios/(应用全部 config plugin,
                                   # 包括 with-release-signing、with-native-modules)
```

常用运行矩阵:

```bash
# ---- Debug(日常开发)----
npx expo run:ios                   # iOS 模拟器
npx expo run:ios --device          # iOS 真机(passkey 必须真机——模拟器无 Secure Enclave 凭据体验)
npx expo run:android               # Android 模拟器/已连接设备
npx expo run:android --device      # 多设备时交互选择目标真机

# ---- Release(发布前真机验证)----
npx expo run:ios --configuration Release            # iOS Release 构建(Hermes 产物、无 dev menu;
                                                    # __DEV__=false,可验证 parallel 徽章/dev_unlocked 门控)
npx expo run:ios --device --configuration Release   # Release + 真机(上架前 D 节验证用这个)
npx expo run:android --variant release              # Android Release 变体;无 android/keystore.properties
                                                    # 时回退 debug keystore 并打 WARNING(仅限本地验证,
                                                    # 该产物禁止发布,passkey DAL 校验也会因指纹不符失败)
```

注意事项:
- **passkey 测试必须真机**:iOS 需登录 iCloud(passkey 存 iCloud Keychain);Android 需 Google 账号 + 屏幕锁。rpId=`getvela.app`,AASA/assetlinks 从线上域拉取,所以真机测试无需本地服务
- **Release 构建是验证 `__DEV__` 门控的唯一方式**(fault console 不注册、/parallel 仅 `dev_unlocked` 可达、徽章仍应在 parallel 激活时出现——2026-07-02 修复项)
- Android Release + 真实签名:`cp keystore.properties.example android/keystore.properties` 填真值(见 05)
- iOS 真机需要开发者证书;Xcode 里选 Team F9W689P9NE,provisioning 需含 Associated Domains entitlement

## 常用命令与实测结果(2026-07-02)

| 命令 | 用途 | 审计实测 |
|---|---|---|
| `npx tsc --noEmit` | 类型检查 | ✅ 通过(修复前因根 tsconfig 误包含 getvela.app 子项目报 52 错,已排除) |
| `npm run lint` | ESLint | ✅ 0 error(修复前 QRScanner.tsx 有 3 个 rules-of-hooks error);~165 warning 为风格类 |
| `npm test` | Jest 单测 | ✅ 79 套件 / 1022 用例(1 个真实 RPC 集成套件默认跳过,见下) |
| `RUN_NETWORK_TESTS=1 npx jest price-query` | 真实 RPC 价格查询集成测试 | 按需运行;依赖第三方公共 RPC,可能因限流失败(非代码缺陷) |
| `npm run test:e2e` | Playwright E2E(自动拉起 dev server) | 见 04 文档实测记录 |
| `npm run build:web` | 生产 Web 构建 → `dist/` | ✅ 通过,~11MB,含 CF Pages 资产修正 |

## 测试环境:Parallel Space

- 入口:dev 下访问 `/parallel`;生产构建需先在 About 页 logo 6 连击设置 `dev_unlocked`
- 原理:**唯一差异是签名密钥** —— 用 `src/services/dev/passkey-fixture.ts` 的 3 把公开测试 P-256 私钥替代真实 passkey,其余(Safe 地址推导、链上验签、全部界面)与生产一致;进入时备份真实钱包缓存、退出恢复(`src/services/dev/parallel-space.ts`)
- **fixture 私钥是公开的,对应地址永远不能放真实资金**
- 激活时全局紫色 PARALLEL SPACE 徽章(`src/components/dev/ParallelSpaceBadge.tsx`;接管修复后在生产构建同样渲染)
- E2E 全部跑在该环境;剧本见 `docs/PARALLEL-SPACE-E2E-PLAYBOOK.md`

## 故障注入(dev only)

浏览器控制台 `vela.help()`:`vela.failRpc/rateLimitRpc/slowRpc/flakyRpc/nullPrice/clear/status`(`src/services/dev/fault-injection.ts`)。E2E 自动化种子:`globalThis.__VELA_FAULT_INIT__`。

## getvela.app 子项目(官网 + API)

```bash
cd getvela.app
bun install
cp .dev.vars.example .dev.vars   # 若无 example,手工创建;需 ALCHEMY_API_KEY / PIMLICO_API_KEY(本地才需要)
bun run dev                      # SvelteKit dev
bunx wrangler deploy             # 部署(需 Cloudflare 账号;生产密钥用 wrangler secret put)
```

注意:该子项目被根 tsconfig **排除**,类型检查用它自己的 `bun run check`(需先 `svelte-kit sync` 生成 `$types`)。

## 数据存储(本地)

全部 AsyncStorage(Web=localStorage,原生=平台实现),键前缀 `vela.*`:
`vela.accounts`(仅公开数据:credentialId/地址/公钥)、`vela.transactionHistory`、`vela.customTokens`、`vela.networkConfig`、`vela.serviceEndpoints`、`vela.walletpairSession`、`vela.language` 等。**无私钥、无助记词** —— 密钥在平台 passkey 里。

清空重置:Web 清 localStorage;原生卸载重装(passkey 仍留在系统凭据管理器,可重新登录找回)。
