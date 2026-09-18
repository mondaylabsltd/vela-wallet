# 05 — 部署手册 (Deployment Runbook)

> 现状(**2026-09-11,spec 039 后**):**Web 钱包 = Cloudflare Worker `vela-wallet-web`**,由 Cloudflare 从 `app-web/vela-wallet` 自建(merge 进 main 即构建),**`wallet.getvela.app` 已于 2026-09-11 由创始人迁到该 Worker**(实测:`/` 按 Accept-Language 307 到 `/en`、`/zh`;`/en/wallet` 是 SvelteKit 页面;旧的 `/onboarding`、`/pay`、`/web-request` 均 404)。旧的 Pages 项目只剩回滚用途。官网/API 与移动端仍手动发布。`.github/workflows/ci.yml` 是合并门禁。

## 部署单元一览

| 单元 | 产物 | 目标 | 命令 |
| --- | --- | --- | --- |
| Web 钱包 | `.svelte-kit/cloudflare`(Worker + 预渲染的 15 个 locale 页) | Cloudflare Worker `vela-wallet-web` = `wallet.getvela.app`(自建;自定义域名在面板配置,`wrangler.jsonc` 不记录) | merge 进 main → Cloudflare 自动执行 `pnpm build`;本地 `pnpm build && pnpm preview` 仅作验证 |
| 官网+API | `.svelte-kit/cloudflare` | Cloudflare Workers(getvela.app) | `cd app-web/getvela.app && bun run deploy` |
| iOS App | .ipa | App Store Connect(**不发 GitHub Releases**,spec 063) | Xcode Archive(`app-ios/VelaWallet/VelaWallet.xcodeproj`);EAS 已随 Expo 退役 |
| Android App | .aab | Google Play(**不发 GitHub Releases**,spec 063) | `app-android/vela-wallet` 的 gradle release 构建;**签名密钥方案待创始人定**(EAS 托管 keystore 已随 Expo 退役,见 `docs/store-submission/`) |
| 桌面 | .exe / .dmg / .rpm / .deb / Flatpak | GitHub Releases(首发渠道,**挂上去的每个包都必须下载即可用**,见下文「GitHub Releases 发布」) → 应用商店尽力而为 | `desktop-v*` tag 触发 `.github/workflows/desktop-*-packages.yml`;`.dmg` 只有在该次构建完成 Developer ID 签名+公证时才会被挂载 |
| Chrome 扩展 | `vela-wallet-extension-<version>.zip`(`app-web/vela-wallet/extension/dist` 打包) | GitHub Releases → 手动上传 Chrome Web Store | `extension-v<version>` tag 触发 `.github/workflows/web-extension-package.yml`(tag 版本必须等于 `extension/manifest.json` 的 `version`;手动 `workflow_dispatch` 只出包不发布) |

另有两个**独立仓库**的服务(不在本仓库,部署互不耦合但语义耦合):vela-relay(gas 报价与错误文案)、p256-index(公钥索引)。

## 发布前检查(每次)

```bash
npm ci --prefix scripts && npm --prefix scripts run check:expo-residue   # 工具包装得上、Expo 没回来、根目录无 npm 文件
cd app-web/vela-wallet && pnpm check && pnpm lint && pnpm test:unit -- --run && pnpm build   # Web 壳
cd app-desktop/vela-wallet && cargo fmt --all --check && cargo clippy --all-targets && cargo test
cd rust && cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures
cd app-web/getvela.app && bun run check                           # 0 errors
```

iOS/Android 的编译与单测由 CI 的 `ios`/`android` job 跑;本地跑法见 `02-local-development.md`。

发布 checklist 附加项:

- [ ] 若改过 bundler 错误文案或相关解析:与 vela-relay 仓库联合验证
- [ ] 若改过授权额度守卫 / 签名编码:在平行空间(`/[locale]/parallel`)手动过一遍清晰签名场景
- [ ] 若改过语料:`npm --prefix scripts run gen:i18n` 的产物已与语料一起提交(CI 会 diff)
- [ ] 若改过 `rust/` 下任何 `.rs` 或 `Cargo.toml`(注释也算):`npm --prefix scripts run build:wasm` 已重建并提交 `rust/pkg-web` + `assets/wasm/vela_core_bg.<hash>.wasm`(`--check` 对每个 Rust 文件取指纹,039 的第一轮 CI 就是栽在一条注释上)

## Web 钱包发布(Worker,自动)

1. 分支上开发 → PR → CI 全绿 → merge 进 main
2. Cloudflare 从 `app-web/vela-wallet` 自建:`pnpm build`(token 漂移检查 + wasm 同步 + worker 类型 + 预渲染全部 locale)。构建失败只在 CF 面板可见,**CI 绿 ≠ 已发布**
3. **Smoke Test**(Worker 的 `workers.dev` 地址;域名迁移后改用 `wallet.getvela.app`):
   - `/` 307 到某个 locale;`/en/wallet` 的 HTML 里没有 `/_expo/` 路径
   - 首屏加载、无控制台报错;创建 / 登录弹出 passkey(rpId=getvela.app)
   - 已有钱包:余额加载、收款显示地址二维码、活动渲染、联系人、设置
   - 确认**没有** PARALLEL SPACE 紫色徽章(若出现=你在 fixture 空间,立即排查)
4. 回滚:Cloudflare Workers 的版本回滚(面板或 `wrangler rollback`)

### 域名迁移(已完成,2026-09-11)

`wallet.getvela.app` 已从冻结的 Pages 部署迁到 Worker(创始人在面板操作)。迁移后的实测记在 `specs/039-retire-expo-tree/results.md`。仍要做/要知道的:

1. **官网要重新部署**(`cd app-web/getvela.app && bun run deploy`):官网六处指向 `wallet.getvela.app/onboarding[?mode=create]` 的链接在 039 分支上已改为 `https://wallet.getvela.app/`,线上官网在部署前仍是旧链接 → 404
2. `/pay` 支付链接页是**欠账**(spec 039 Part B):官网的 `/pay` 转发暂时落到钱包首页(保留 query),等钱包补上 `/pay` 路由后改回
3. 面板:若 Pages 项目仍连着 main 的自动部署,**暂停它**(否则每次 merge 都报一次失败构建);Pages 的最后一次部署留作回滚
4. 回滚:把域名重新绑回 Pages 项目的最后一次部署(静态、无状态、秒级)

**老用户注意**:Expo 版把账户/联系人/自定义代币/历史存在 localStorage,Worker 版存 IndexedDB,两者格式逐字节兼容但**不迁移**(创始人裁定 2026-09-11:客户端缓存可丢失)。钱包本身在 passkey 后面,重新登录即回。

## 官网/API 发布(getvela.app)

1. `cd app-web/getvela.app && bun run deploy`
2. 生产密钥(只需一次/轮换时):`wrangler secret put ALCHEMY_API_KEY / PIMLICO_API_KEY / GITHUB_BUG_TOKEN`
3. Smoke:`curl -s https://getvela.app/api/exchange-rate?...`;`curl -s https://getvela.app/.well-known/apple-app-site-association`
4. 回滚:`wrangler rollback` 或重发上一个 commit 的构建
5. **注意**:API 部署影响钱包壳的 bundler 代理路径——发布后立刻在钱包里做一次小额估算(不需提交)确认 `/api/bundler` 正常

## GitHub Releases 发布(桌面 + 扩展)

渠道规则是创始人 2026-09-18 的裁定([spec 063](../../specs/063-release-channels/spec.md)):

- **手机端(iOS / Android)不发 GitHub。** 渠道是应用商店(付费),或自行编译。`android-v*` / `ios-v*` tag 仍然有用:它触发 release 构建、把产物留作 workflow artifact(从 run 页面下载,本地签名后提交商店),并且是「release 配置还能不能编过」的唯一检查。
- **桌面与扩展先发 GitHub,且挂上去的包必须可用。** 「可用」= 没有开发工具的人下载后能装上、能进到钱包:
  - Linux:发行版自带工具能装。
  - Windows:能装能跑。不买代码签名证书(裁定),SmartScreen 会拦一次,release 说明里写了「更多信息 → 仍要运行」。
  - 扩展:zip 解压后「加载已解压的扩展程序」,release 说明里写了步骤。
  - macOS:**Developer ID 签名 + hardened runtime + 内嵌 Developer ID provisioning profile + 公证 + staple**,缺一不可。缺任何一样,Gatekeeper 都会把浏览器下载的 .dmg 报成「已损坏」;缺 profile 则没有平台 passkey。
- **mac 包在创始人的 Mac 上签名、手动上传**(裁定,spec 063 §3a):签名私钥不上 GitHub。tag 推上去、CI 建好 release 之后:`git checkout <tag> && cd app-desktop/vela-wallet && ./scripts/release-macos-local.sh <tag> --upload`。脚本只认 tag 对应的源码,自己找 Developer ID profile、按 profile 选证书(钥匙串里有同名证书,按 SHA-1 不按名字)、编译前先验公证凭据,已有 mac 包时拒绝覆盖。一次性准备见 [quickstart](../../specs/063-release-channels/quickstart.md) §A。
- **这条规则是机制,不是习惯。** macOS workflow 只有在该次运行真的完成签名+公证时才挂 .dmg;凭据缺失时照常构建、校验三种架构,但什么都不挂,并在 job summary 里写明缺什么。凭据放在 GitHub Environment `release` 里(仅 `desktop-v*` tag 可用),清单与生成方法见 [quickstart](../../specs/063-release-channels/quickstart.md)。

### 发版步骤

1. 四处版本号保持一致(各 workflow 的 metadata job 会断言 tag == 工程内版本):`app-desktop/vela-wallet/Cargo.toml`(+`Cargo.lock`)、`extension/manifest.json`、`app/build.gradle.kts` 的 `versionName`、`project.pbxproj` 的 `MARKETING_VERSION`;桌面另有 `metainfo.xml` 的 `<release>` 与 rpm spec 的 `%changelog`(**星期几必须与日期相符**,否则 `rpmbuild` 直接报错)。
2. 版本号只能是 `x.y.z`。`1.0.0-alpha` 这类后缀会同时打断三处:扩展 manifest(Chrome 只收点分整数,workflow 也断言 `^\d+\.\d+\.\d+$`)、rpm 的 `Version:`(不能含 `-`)、iOS 的 `CFBundleShortVersionString`。「这不是正式版」用 GitHub 的 pre-release 标记表达。
3. 合并进 main 后打 tag。

### 三个已经付过学费的坑

1. **tag 必须一个一个推。** `git push origin a-v1 b-v1 c-v1 d-v1` 一次推多个 tag,**一个 workflow 都不会触发**(0.9.0 实测:四个 tag 都到了远端,零个 run)。每个 tag 单独 `git push origin <tag>`。
2. **不要对已经有 release 的 tag 做「删除再重推」。** GitHub 会把原 release 变成草稿(对外消失,产物还在),而草稿不占用 tag,于是 workflow 的 `gh release create` 会**再建一个**同名 release——同一个 tag 下两个 release,一个草稿一个公开。要重发就升版本号;只想重跑某个平台的构建用 `workflow_dispatch`(注意它取的是所选 ref 上的 workflow 文件)。
3. **PR 全绿对打包路径没有信息量。** 所有打包/归档 job 都是 `if: github.event_name != 'pull_request'`,PR 上只跑元数据校验。0.9.0 一次暴露的三个缺陷(iOS 归档用错 Xcode、Linux 容器缺 dbus 头文件、deb control 里的 `#` 注释)全都因此潜伏了数周。**改了打包相关的东西,合并前在分支上 `gh workflow run "<workflow>" --ref <branch>` 真跑一次**——这不会建 release(附加步骤只认 tag),但会真的构建。

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
