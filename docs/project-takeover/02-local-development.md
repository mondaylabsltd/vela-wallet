# 02 — 本地开发 (Local Development)

> 2026-09-11 按 spec 039 重写:Expo / React Native 应用已退役,本文只描述现存的四个壳、共享核心与 `scripts/` 里的工具包(根目录不放任何 npm 文件)。

## 前置

- **Node 22 + npm ≥ 9**(根目录工具包);**pnpm 10**(`app-web/vela-wallet`,版本在其 `packageManager` 字段);**bun**(`app-web/getvela.app`)
- **Rust 1.97.1**(`rustup`),加 `wasm32-unknown-unknown` target 与 `wasm-pack 0.15`(仅在改动核心后需要重建 wasm)
- iOS:Xcode(需能编译 xcframework);Android:JDK 17 + Android Studio/SDK;桌面:gpui 的系统依赖(见 `app-desktop/vela-wallet/README.md`)
- 无需任何私有凭据即可开发钱包本体(RPC 走公共池;bundler 走内置 `getvela.app/api/bundler` 代理)

## 从零启动

```bash
# 工具包在 scripts/(4 个依赖),根目录不放 npm 文件;不构建任何 App
npm ci --prefix scripts

# Web 壳(生产 Web 钱包)
cd app-web/vela-wallet && pnpm install && pnpm dev        # http://localhost:5173
# 新 clone 上先跑一次 pnpm dev / pnpm build / pnpm sync:wasm 再跑 pnpm check:
# static/ 里的 wasm 副本不入库,由 sync-wasm 生成,`check` 只校验不生成

# 桌面壳
cd app-desktop/vela-wallet && cargo run

# iOS 壳:先出 xcframework(gitignored),再开工程
./rust/scripts/build-ios-xcframework.sh
open app-ios/VelaWallet/VelaWallet.xcodeproj               # ⌘R

# Android 壳:先生成 Kotlin 绑定(gitignored),再装机
cd rust && cargo build --release -p vela-core-uniffi && \
  cargo run --release -p vela-core-uniffi --bin uniffi-bindgen -- generate \
    --library target/release/libvela_core_uniffi.dylib --language kotlin \
    --out-dir bindings/kotlin --no-format
cd ../app-android/vela-wallet && ./gradlew :app:installDebug
```

新 clone 建不出 iOS/Android 的原因与解法都在 `.github/workflows/ci.yml` 的 `ios`/`android` job 注释里(绑定产物不入库)。

## 常用命令与门禁

工具包(`scripts/package.json`;下表命令在根目录写作 `npm --prefix scripts run <name>`,或 `cd scripts` 后 `npm run <name>`;每条脚本都先切回仓库根再执行):

| 命令 | 用途 |
|---|---|
| `gen:i18n` | 语料 → Rust 目录 + `public/i18n`(改语料必须一起提交产物;加键要改脚本里的路径计数) |
| `lint:i18n` / `verify:i18n` | 语料缺陷登记 / Rust i18n 与 `i18next` 的 parity |
| `gen:identicon-features` / `verify:identicon` | 头像图形表再生成 / 与 `identicons-esm` 的 parity |
| `dump:vectors` | 从 npm 预言机重导 identicon/i18n 语料向量(CI 要求零 diff) |
| `gen:core-types` | Rust 枚举 → `app-web/vela-wallet/src/lib/*/generated`(CI 要求零 diff) |
| `build:wasm` / `verify:wasm` | 重建 `rust/pkg-web` + `public/vela_core_bg.<hash>.wasm`(**改过 `rust/` 下任何 `.rs`/`Cargo.toml`,连注释都算,推送前必须重建**)/ 用发布产物回放语料 |
| `lint:lottie` / `check:native-reachability` | 启动动画资产合法性 / 每个原生壳的页面都能从导航根到达 |
| `check:expo-residue` | Expo 残留检查(删掉的路径、根目录 npm 文件、依赖、CI 步骤、文档里的死命令) |

各壳的门禁(提交前跑自己改过的壳):

| 壳 | 命令 |
|---|---|
| app-web | `pnpm check && pnpm lint && pnpm test:unit -- --run && pnpm build`;e2e:`pnpm test:e2e`(起真实 worker,截图落在 `e2e/__screenshots__/`) |
| app-desktop | `cargo fmt --all --check && cargo clippy --all-targets && cargo test` |
| rust(核心) | `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures`(少了 feature 会红,见 ci.yml 注释) |
| app-ios | `xcodebuild test -project VelaWallet.xcodeproj -scheme VelaWallet -destination 'id=<模拟器 udid>' CODE_SIGNING_ALLOWED=NO`(Swift Testing 的结果在第二行,"Executed 0 tests" 不是失败) |
| app-android | `./gradlew :app:assembleDebug :app:testDebugUnitTest -PvelaSkipRustBuild` |

## 测试环境:Parallel Space

- Web:`/[locale]/parallel`。**唯一差异是签名密钥** —— 用固定的公开测试 P-256 密钥集替代真实 passkey,其余(地址推导、链上验签、全部界面)与生产一致(`app-web/vela-wallet/src/lib/dev/parallel-space.ts`);激活时全局紫色 PARALLEL SPACE 徽章(`ParallelSpaceBadge.svelte`)。平行空间里 gas 赞助按设计关闭。
- 桌面:`vela-core` 的 `dev-fixtures` feature 提供同一密钥集的软件签名器(`cargo test`/本地开发用;发布包不编译)。
- **fixture 私钥是公开的,对应地址永远不能放真实资金。** 金标测试 Safe 见 `docs/PARALLEL-SPACE.md`。
- 真网只读巡检与真发钱的步骤:`docs/PARALLEL-SPACE.md`。

## 故障注入(Web)

浏览器控制台 `vela.help()`:`vela.failRpc/rateLimitRpc/slowRpc/flakyRpc/nullPrice/clear/status`(`app-web/vela-wallet/src/lib/services/fault-injection.ts`);控制台由 `localStorage['vela.dev.console']` 门控(`src/lib/services/dev-console.ts`)。

## getvela.app 子项目(官网 + API)

```bash
cd app-web/getvela.app
bun install
cp .dev.vars.example .dev.vars   # 若无 example,手工创建;需 ALCHEMY_API_KEY / PIMLICO_API_KEY(本地才需要)
bun run dev                      # SvelteKit dev
bunx wrangler deploy             # 部署(需 Cloudflare 账号;生产密钥用 wrangler secret put)
```

类型检查用它自己的 `bun run check`(需先 `svelte-kit sync` 生成 `$types`)。

## 数据存储(本地)

- Web:钱包记录在 IndexedDB(库 `vela`、store `kv`,`app-web/vela-wallet/src/lib/services/storage.ts`),键前缀 `vela.*`:`vela.accounts`(仅公开数据:credentialId/地址/公钥)、`vela.transactionHistory`、`vela.customTokens`、`vela.networkConfig`、`vela.contacts*` 等;onboarding 的四个键与 `vela.serviceEndpoints` 留在 localStorage。清空重置:设置 → 存储,或浏览器清站点数据。
- 桌面:单个 JSON 文件,tmp+rename 写入(`app-desktop/vela-wallet/src/executor/storage.rs`)。
- iOS/Android:见各自 README。
- **无私钥、无助记词** —— 密钥在平台 passkey 或安全钥匙里;清掉本地数据后凭任意一把创始钥匙重新登录即可找回钱包。
