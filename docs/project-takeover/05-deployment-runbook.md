# 05 — 部署手册 (Deployment Runbook)

> 现状(**2026-09-11,spec 039 后**):**Web 钱包的生产构建 = Cloudflare Worker `vela-wallet-web`**,由 Cloudflare 从 `app-web/vela-wallet` 自建(merge 进 main 即构建)。**但 `wallet.getvela.app` 这个域名今天仍指向旧的 Cloudflare Pages 项目**——它冻结在 Pages 最后一次成功构建的 Expo 包(main @ 936f1b3f),因为 Pages 的构建命令(根目录的 `build:web` 脚本)已随 Expo 应用删除。域名迁到 Worker 是创始人在面板里做的一次操作,清单见下文「域名迁移」。官网/API 与移动端仍手动发布。`.github/workflows/ci.yml` 是合并门禁。

## 部署单元一览

| 单元 | 产物 | 目标 | 命令 |
| --- | --- | --- | --- |
| Web 钱包 | `.svelte-kit/cloudflare`(Worker + 预渲染的 15 个 locale 页) | Cloudflare Worker `vela-wallet-web`(自建;`wrangler.jsonc` 里 `workers_dev: true`,自定义域名在面板配置) | merge 进 main → Cloudflare 自动执行 `pnpm build`;本地 `pnpm build && pnpm preview` 仅作验证 |
| 官网+API | `.svelte-kit/cloudflare` | Cloudflare Workers(getvela.app) | `cd app-web/getvela.app && bun run deploy` |
| iOS App | .ipa | App Store Connect | Xcode Archive(`app-ios/VelaWallet/VelaWallet.xcodeproj`);EAS 已随 Expo 退役 |
| Android App | .aab | Google Play | `app-android/vela-wallet` 的 gradle release 构建;**签名密钥方案待创始人定**(EAS 托管 keystore 已随 Expo 退役,见 `docs/store-submission/`) |
| 桌面 | .exe / .app / .rpm / .deb / Flatpak | GitHub Releases | `desktop-v*` tag 触发 `.github/workflows/desktop-*-packages.yml` |

另有两个**独立仓库**的服务(不在本仓库,部署互不耦合但语义耦合):vela-relay(gas 报价与错误文案)、p256-index(公钥索引)。

## 发布前检查(每次)

```bash
npm ci && npm run check:expo-residue                              # 根目录工具包装得上、Expo 没回来
cd app-web/vela-wallet && pnpm check && pnpm lint && pnpm test:unit -- --run && pnpm build   # Web 壳
cd app-desktop/vela-wallet && cargo fmt --all --check && cargo clippy --all-targets && cargo test
cd rust && cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures
cd app-web/getvela.app && bun run check                           # 0 errors
```

iOS/Android 的编译与单测由 CI 的 `ios`/`android` job 跑;本地跑法见 `02-local-development.md`。

发布 checklist 附加项:

- [ ] 若改过 bundler 错误文案或相关解析:与 vela-relay 仓库联合验证
- [ ] 若改过授权额度守卫 / 签名编码:在平行空间(`/[locale]/parallel`)手动过一遍清晰签名场景
- [ ] 若改过语料:`npm run gen:i18n` 的产物已与语料一起提交(CI 会 diff)
- [ ] 若改过核心 Rust:`npm run build:wasm` 已重建并提交 `rust/pkg-web` + `public/vela_core_bg.<hash>.wasm`(`--check` 对每个 Rust 文件取指纹)

## Web 钱包发布(Worker,自动)

1. 分支上开发 → PR → CI 全绿 → merge 进 main
2. Cloudflare 从 `app-web/vela-wallet` 自建:`pnpm build`(token 漂移检查 + wasm 同步 + worker 类型 + 预渲染全部 locale)。构建失败只在 CF 面板可见,**CI 绿 ≠ 已发布**
3. **Smoke Test**(Worker 的 `workers.dev` 地址;域名迁移后改用 `wallet.getvela.app`):
   - `/` 307 到某个 locale;`/en/wallet` 的 HTML 里没有 `/_expo/` 路径
   - 首屏加载、无控制台报错;创建 / 登录弹出 passkey(rpId=getvela.app)
   - 已有钱包:余额加载、收款显示地址二维码、活动渲染、联系人、设置
   - 确认**没有** PARALLEL SPACE 紫色徽章(若出现=你在 fixture 空间,立即排查)
4. 回滚:Cloudflare Workers 的版本回滚(面板或 `wrangler rollback`)

### 域名迁移(创始人一次性操作,时间自定)

`wallet.getvela.app` 从冻结的 Pages 部署迁到 Worker:

1. 仓库里我们自己指向旧路径的链接已改为 `https://wallet.getvela.app/`(官网六处);SDK 默认的 `/web-request` 与 `/pay` 是**欠账**(spec 039 Part B),迁移前补齐或接受 404
2. 面板:给 Worker `vela-wallet-web` 加自定义域名 `wallet.getvela.app`;把 Pages 项目从该域名解绑,并**暂停 Pages 的自动部署**(否则每次 merge 都报一次失败构建)
3. 迁移前后各做一次上面的 Smoke Test,结果记入 `specs/039-retire-expo-tree/results.md`
4. 回滚:把域名重新绑回 Pages 项目的最后一次部署(静态、无状态、秒级)
5. 当天改本文与 README 的「域名现状」一句

**老用户注意**:Expo 版把账户/联系人/自定义代币/历史存在 localStorage,Worker 版存 IndexedDB,两者格式逐字节兼容但**不迁移**(创始人裁定 2026-09-11:客户端缓存可丢失)。钱包本身在 passkey 后面,重新登录即回。

## 官网/API 发布(getvela.app)

1. `cd app-web/getvela.app && bun run deploy`
2. 生产密钥(只需一次/轮换时):`wrangler secret put ALCHEMY_API_KEY / PIMLICO_API_KEY / GITHUB_BUG_TOKEN`
3. Smoke:`curl -s https://getvela.app/api/exchange-rate?...`;`curl -s https://getvela.app/.well-known/apple-app-site-association`
4. 回滚:`wrangler rollback` 或重发上一个 commit 的构建
5. **注意**:API 部署影响钱包壳的 bundler 代理路径——发布后立刻在钱包里做一次小额估算(不需提交)确认 `/api/bundler` 正常

## Android 发布

EAS 的云构建与其托管的 keystore 已随 Expo 应用退役(2026-09-11)。现行路径:

1. 签名:创始人自持 upload keystore(方案与保管见 `docs/store-submission/`;**尚未定案**,这是上架前的阻塞项)
2. 构建:`app-android/vela-wallet` 的 gradle release 变体(先生成 Kotlin 绑定,见 02);三 ABI 的 NDK 交叉编译约 6 分钟
3. Play Console:上传首个 AAB + Play App Signing;取**两枚 SHA-256**(app signing cert 与 upload cert)写入 `app-web/getvela.app/src/routes/.well-known/assetlinks.json/+server.ts` 并部署官网;Google Statement List Tester 验证
4. 真机验证 passkey 创建/登录(DAL 生效需要签名匹配;无 GMS 机型走 USB/caBLE 路径,见 spec 019)
5. 分阶段发布(建议 10%→50%→100%)

## iOS 发布

- Team ID F9W689P9NE 已配好 AASA(`webcredentials:getvela.app`);确认 provisioning profile 含 Associated Domains entitlement
- 先 `./rust/scripts/build-ios-xcframework.sh`,再 Xcode Archive → App Store Connect → TestFlight 先行
- 真机 Smoke:passkey 创建、QR 扫描、一笔小额估算
- 回滚:App Store 无真回滚,靠"暂停分阶段发布"+加急审核,谨慎推 100%

## 数据迁移

无服务端用户数据库,无迁移流程。客户端存储结构变更时必须在代码内做向后兼容读取(Web 的 `src/lib/services/storage.ts` 头注释记录了逐字节兼容契约;桌面 `executor/storage.rs`)。
