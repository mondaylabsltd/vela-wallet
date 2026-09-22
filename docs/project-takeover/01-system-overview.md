# 01 — 系统全景 (System Overview)

> 接管审计日期:2026-07-02;**2026-09-11 按 spec 039 重写**(Expo / React Native 应用已退役并删除)。所有结论基于源码/配置/实际运行结果,证据以 `文件:行号` 标注。

## 一句话介绍

Vela Wallet 是一个 **passkey(P-256/WebAuthn)签名的 ERC-4337 智能合约钱包**(Safe v1.4.1 + EntryPoint v0.7),无助记词、无浏览器插件依赖,由**一个共享 Rust 核心 + 四个原生壳**组成(iOS SwiftUI / Android Compose / Web SvelteKit / 桌面 gpui),支持 12+ EVM 链,通过自营 bundler 收取 relayer 费获利。

## 用户与商业模型

- 目标用户:多链活跃转账者(见 `docs/marketing/100-marketing-leads.md`)
- 收入:**Web 版免费**;**iOS/Android 商店版付费下载($39.99 买断,见 `docs/marketing/`)**;叠加中继手续费(预留 gas × 3 × 所选速度价格,签名前显示;2026-09-22 按 spec 080 更正,原"约 2×"不对)
- 团队:单人创始人(开发/运维/发布同一人)

## 技术栈

| 层 | 技术 | 版本 | 证据 |
|---|---|---|---|
| 共享核心 | Rust:`vela-core`(crux 状态机 + 纯计算)、`vela-core-wasm`(wasm-bindgen)、`vela-core-uniffi`(Swift/Kotlin 绑定) | Rust 1.97.1 | `rust/Cargo.toml`, `.github/workflows/ci.yml` `rust` job |
| Web 壳 | SvelteKit 2 / Svelte 5(runes)/ TypeScript strict / Cloudflare Workers;同一份代码另构建 Chrome MV3 扩展 | pnpm 10 | `app-web/vela-wallet/package.json`, `wrangler.jsonc`, `extension/build.mjs` |
| 桌面壳 | Rust + gpui(Zed 的 UI 框架),直接依赖 `vela-core` crate | — | `app-desktop/vela-wallet/Cargo.toml` |
| iOS 壳 | SwiftUI + VelaCoreKit(SPM 包,包裹 uniffi 生成的 xcframework) | Xcode 16 | `app-ios/VelaWallet/VelaWallet.xcodeproj`, `rust/scripts/build-ios-xcframework.sh` |
| Android 壳 | Kotlin + Jetpack Compose + uniffi Kotlin 绑定(`rust/bindings/kotlin`,gitignored,构建前生成) | JDK 17, minSdk 29 | `app-android/vela-wallet/app/build.gradle.kts` |
| 站点/API 后端 | SvelteKit on Cloudflare Workers(仓库内子项目) | bun 管理 | `app-web/getvela.app/wrangler.jsonc` |
| 密码学 | 平台 passkey(Secure Enclave / Credential Manager / navigator.credentials / 桌面 hidapi 安全钥匙 + caBLE),私钥永不进壳 | — | `rust/crates/vela-core/src/webauthn/`, 各壳的 passkey 执行器 |
| 链上账户 | Safe v1.4.1 + Safe4337Module + WebAuthn Signer + EntryPoint v0.7 | 合约地址硬编码(全链统一 CREATE2) | `rust/crates/vela-core/src/safe/` |
| 工具包(`scripts/`) | npm:语料/头像生成器、npm 预言机(`i18next`、`identicons-esm`)、闸门脚本;根目录不放任何 npm 状态 | Node 22 | `scripts/package.json`(不构建任何 App;`npm --prefix scripts run <name>`) |
| 测试 | `cargo test`(核心 + 桌面)、vitest + Playwright(web)、Swift Testing / XCTest(iOS)、gradle 单测(Android)、`scripts/` 闸门脚本 | — | `.github/workflows/ci.yml` |

**注意:没有 viem/ethers 依赖** —— ABI 编码、EIP-712、RPC、Multicall 在 Rust 核心里(alloy-core 等固定版本 crate)。改动这些底层时务必跑核心的 conformance 语料(`cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures`)。

## 仓库布局(顶层)

```
rust/                共享核心:crates/vela-core(状态机、原语、i18n 语料与目录)、
                     vela-core-wasm、vela-core-uniffi;scripts/(wasm 构建、类型生成);
                     pkg-web/(提交的 wasm 产物)
app-web/vela-wallet/ Web 壳(SvelteKit,生产 Web 钱包;extension/ 为 Chrome 扩展构建)
app-web/getvela.app/ 官网 + API 代理(bundler/bug-report/exchange-rate/…)+ .well-known
app-web/clearsigning/ 零构建的清晰签名静态页(见其 HANDOVER.md)
app-desktop/vela-wallet/  桌面壳(gpui)
app-ios/             iOS 壳(VelaWallet 工程 + VelaCoreKit SPM 包)
app-android/         Android 壳
assets/i18n/         gen-i18n 生成的 15 份语言目录,iOS/Android/Web 与两道闸门都从此路径读
assets/wasm/vela_core_bg.<hash>.wasm  build:wasm 产物,app-web 的 sync-wasm 从此复制
assets/fonts/        Plus Jakarta Sans TTF(桌面 include_bytes!)
design/              图标 SVG 源、Lottie 启动动画、插画
scripts/             工具包(package.json 在此,根目录无 npm 文件):gen-i18n、gen-identicon-features、
                     verify-*-parity、lint-lottie-assets、check-native-reachability、check-expo-residue、
                     gen-app-icons.sh;onchain/ 为链上 e2e 脚本(bun)
docs/                设计/需求/测试/上架/接管文档
specs/               按落地顺序编号的功能规格(spec/plan/tasks/results)
```

`src/`、`e2e/`、`modules/`、`plugins/`、`targets/`、`android/`、`ios/` 已不存在(spec 039)。

## 关键外部依赖(运行时)

| 服务 | 用途 | Web 壳位置 | 桌面壳位置 |
|---|---|---|---|
| vela-relay(**独立仓库**) | 自营 4337 bundler,gas 报价权威 | `app-web/vela-wallet/src/lib/services/dapp-submit.ts`、`safe-transaction.ts` | `app-desktop/vela-wallet/src/executor/relay.rs` |
| p256-index.getvela.app(**独立仓库** biubiu-projects) | 公钥索引(跨设备恢复),CF Worker + D1 + DO 队列 | `src/lib/onboarding/core/` | `executor/` |
| getvela.app/api/* | bug-report GitHub 代理、汇率(`/api/exchange-rate`)、下载镜像(`/api/downloads`)、OG 图(`/api/og`) | `app-web/getvela.app/src/routes/api/` | — |
| 公共 RPC 池 | 每链多端点评分/封禁/故障转移(规则在核心 `rpc_pool.rs`) | `src/lib/services/rpc-pool-endpoints.ts` | `executor/` |
| Chainlink / DEX quoter | 价格 | `src/lib/services/wallet-api.ts` | `executor/balance_dashboard.rs` |

WalletPair 中继与 remote-inject 桥已随 Expo 应用退役(创始人在 spec 027 裁定不做);Web 的 dApp 通路是 Chrome 扩展注入的 EIP-1193/6963 provider。

> **勘误(2026-09-22,spec 081 FR-015)**:官网曾有五条 `api/{wallet,transactions,nft,bundler,proxy}` 代理路由(Alchemy/Pimlico 转发 + 一个开放 fetch-and-pipe),经全仓 + vela-relay + p256-index 复核**零调用方**,已删除。**钱包从来不走 `getvela.app/api/bundler`**:bundler 的真实默认是 `https://vela-relay-cf.getvela.app`(核心 `rust/crates/vela-core/src/app/network_admin.rs:154` 的 `DEFAULT_BUNDLER_SERVICE_URL`,用户可在设置里覆盖)。官网现存的 API 只有 `og` / `downloads` / `bug-report` / `exchange-rate` 四条。

## 关键数据流

1. **创建钱包**:核心 `create_wallet` 状态机 → 壳执行 passkey 注册(可选平台/跨设备/安全钥匙)→ 核心解析 attestation 提取 P-256 公钥 → 地址 = f(全部创始钥匙)(CREATE2,未部署,首笔交易时部署)→ 公钥上传索引服务(重试队列由核心驱动)。
2. **登录/恢复**:核心 `login` 机:任意一把钥匙签两次 → 从签名恢复公钥 → 索引查其余钥匙 → 重建地址(索引是缓存,不是单点)。
3. **发送交易**:核心 `send` / `fee_policy` 机:构建 callData(单笔/split/sweep = MultiSend)→ 壳取 deployed/nonce/gas → bundler 报价优先 → EIP-712 SafeOp hash → passkey 签名 → 提交 bundler → `track` 机轮询回执;历史记录 pending-at-submit 持久化。
4. **dApp 签名**(Web 扩展):`dapp_permissions` 决定谁能问 → 请求在侧栏/独立窗口(`/[locale]/request`)呈现 → 交易模拟 + ERC-7730 清晰签名 + 授权额度守卫(永不 unlimited)→ 同一 send 路径提交。
5. **状态持久化**:键均为 `vela.*`,格式与旧 Expo 应用逐字节兼容;Web 存 IndexedDB(`src/lib/services/storage.ts`),桌面存单个 JSON 文件(`executor/storage.rs`,tmp+rename)。账户仅存 credentialId/地址/公钥。

## 配置体系

- **没有 .env / EXPO_PUBLIC_* 体系**:壳内全部配置为代码内常量 + 用户在设置里的覆盖(RPC/bundler/服务端点,键 `vela.serviceEndpoints`)。
- getvela.app 子项目:本地密钥在 `.dev.vars`(已 gitignore),生产密钥走 `wrangler secret put`。spec 081 删掉五条代理路由后,**唯一还被读的 secret 是 `GITHUB_BUG_TOKEN`**(可选 `GITHUB_BUG_REPO`);`ALCHEMY_API_KEY` / `PIMLICO_API_KEY` / `BUNDLER_PROVIDER` 已无代码引用,可在 Cloudflare 面板删除。
- 设计 token 唯一来源 `docs/design-tokens.json`(Penpot 导出),各壳生成自己的 token 文件并有漂移闸门。

## 环境差异

| 环境 | 差异 |
|---|---|
| Web 开发/测试 | `/[locale]/parallel`(固定密钥集的平行空间,紫色徽章)、`/[locale]/gallery` 与 `/dev/gallery`(fixture 画廊)、`vela.*` 故障注入控制台(需 `localStorage['vela.dev.console']`,见 `src/lib/services/dev-console.ts`) |
| Web 生产(Worker) | 以上入口仍在代码中,由运行时门控;wasm 不进 Worker(每个 locale 页预渲染) |
| 桌面 | `dev-fixtures` feature 提供平行空间的软件签名器(仅 `cargo test`/本地,发布包不编译) |
| iOS/Android | passkey 走平台 API;rpId 固定 `getvela.app`;依赖 AASA / assetlinks.json 域名关联 |

## CI/CD

`.github/workflows/ci.yml`,PR 与 main 推送触发。八个 job:`app`(`scripts/` 工具闸门:语料/头像预言机再生成与 diff、Lottie 与可达性 lint、两道 parity、Expo 残留检查)、`web`(app-web `pnpm build`)、`site`(getvela.app `bun run check`)、`rust`(fmt/clippy/测试/wasm 金丝雀/web 产物一致/onboarding 类型一致/Swift 生成/Kotlin 语料)、`rust-macos`(Swift 语料)、`desktop`(fmt/clippy/test)、`android`(生成 Kotlin 绑定 + assembleDebug + 单测)、`ios`(xcframework + xcodebuild test)。

部署:Web 钱包由 Cloudflare 从 `app-web/vela-wallet` 自建 Worker `vela-wallet-web`(与 CI 无耦合,**CI 绿 ≠ 已发布**);官网手动 `bun run deploy`;iOS/Android 见 `05-deployment-runbook.md`。
