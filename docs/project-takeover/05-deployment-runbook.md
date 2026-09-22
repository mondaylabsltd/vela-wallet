# 05 — 部署手册 (Deployment Runbook)

> 现状(**2026-09-11,spec 039 后**):**Web 钱包 = Cloudflare Worker `vela-wallet-web`**,由 Cloudflare 从 `app-web/vela-wallet` 自建(merge 进 main 即构建),**`wallet.getvela.app` 已于 2026-09-11 由创始人迁到该 Worker**(实测:`/` 按 Accept-Language 307 到 `/en`、`/zh`;`/en/wallet` 是 SvelteKit 页面;旧的 `/onboarding`、`/pay`、`/web-request` 均 404)。旧的 Pages 项目只剩回滚用途。官网/API 与移动端仍手动发布。`.github/workflows/ci.yml` 是合并门禁。

## 部署单元一览

| 单元 | 产物 | 目标 | 命令 |
| --- | --- | --- | --- |
| Web 钱包 | `.svelte-kit/cloudflare`(Worker + 预渲染的 15 个 locale 页) | Cloudflare Worker `vela-wallet-web` = `wallet.getvela.app`(自建;自定义域名在面板配置,`wrangler.jsonc` 不记录) | merge 进 main → Cloudflare 自动执行 `pnpm build`;本地 `pnpm build && pnpm preview` 仅作验证 |
| 官网+API | `.svelte-kit/cloudflare` | Cloudflare Workers(getvela.app) | `cd app-web/getvela.app && bun run deploy` |
| iOS App | .ipa | App Store Connect(**不发 GitHub Releases**,spec 063) | Xcode Archive(`app-ios/VelaWallet/VelaWallet.xcodeproj`);EAS 已随 Expo 退役 |
| Android App | .aab | Google Play(**不发 GitHub Releases**,spec 063) | `app-android/vela-wallet` 的 gradle release 构建;**签名密钥方案待创始人定**(EAS 托管 keystore 已随 Expo 退役,见 `docs/store-submission/`) |
| 桌面 | .exe / .dmg / .rpm / .deb / Flatpak | GitHub Releases(首发渠道,**挂上去的每个包都必须下载即可用**,见下文「GitHub Releases 发布」) → 应用商店尽力而为 | 推 `release/vX.Y.Z` 分支触发 `.github/workflows/release.yml`(spec 064);`.dmg` 只有在该次构建完成 Developer ID 签名+公证时才会被挂载 |
| Chrome 扩展 | `vela-wallet-extension-<version>.zip`(`app-web/vela-wallet/extension/dist` 打包) | GitHub Releases → 手动上传 Chrome Web Store | 随 `release/vX.Y.Z` 一起发(spec 064);手动 `workflow_dispatch` 只出包不发布 |

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
2. 生产密钥(只需一次/轮换时):`wrangler secret put GITHUB_BUG_TOKEN`
3. Smoke:`curl -s "https://getvela.app/api/exchange-rate?currency=CNY"`;`curl -s https://getvela.app/.well-known/apple-app-site-association`
4. 回滚:`wrangler rollback` 或重发上一个 commit 的构建
5. **注意**:官网发布**不影响钱包发交易**。现存 API 只有 `og` / `downloads` / `bug-report` / `exchange-rate` 四条,全部是官网自用或反馈通道。

> **勘误(2026-09-22,spec 081 FR-015)**:本节原写「生产密钥 `ALCHEMY_API_KEY` / `PIMLICO_API_KEY`」与「发布后立刻在钱包里做一次小额估算确认 `/api/bundler` 正常」——两条都作废。`api/{wallet,transactions,nft,bundler,proxy}` 五条路由零调用方,已删除,那两枚 key 也不再被任何代码读取。钱包的 bundler 一直是自营中继 `https://vela-relay-cf.getvela.app`(核心 `rust/crates/vela-core/src/app/network_admin.rs:154`),它在**另一个仓库**(vela-relay),与官网发布无关。

## GitHub Releases 发布(桌面 + 扩展)

渠道规则是创始人 2026-09-18 的裁定([spec 063](../../specs/063-release-channels/spec.md)):

- **手机端(iOS / Android)不发 GitHub。** 渠道是应用商店(付费),或自行编译。它们仍然随每次发版构建(`release.yml` 调用),产物留作 workflow artifact(从 run 页面下载,本地签名后提交商店),并且是「release 配置还能不能编过」的唯一检查。
- **桌面与扩展先发 GitHub,且挂上去的包必须可用。** 「可用」= 没有开发工具的人下载后能装上、能进到钱包:
  - Linux:发行版自带工具能装。
  - Windows:能装能跑。不买代码签名证书(裁定),SmartScreen 会拦一次,release 说明里写了「更多信息 → 仍要运行」。
  - 扩展:zip 解压后「加载已解压的扩展程序」,release 说明里写了步骤。
  - macOS:**Developer ID 签名 + hardened runtime + 内嵌 Developer ID provisioning profile + 公证 + staple**,缺一不可。缺任何一样,Gatekeeper 都会把浏览器下载的 .dmg 报成「已损坏」;缺 profile 则没有平台 passkey。
- **mac 包在创始人的 Mac 上签名、手动上传**(裁定,spec 063 §3a):签名私钥不上 GitHub。`release.yml` 建好 release 之后:`git fetch --tags && git checkout vX.Y.Z && cd app-desktop/vela-wallet && ./scripts/release-macos-local.sh vX.Y.Z --upload`。脚本只认 tag 对应的源码,自己找 Developer ID profile、按 profile 选证书(钥匙串里有同名证书,按 SHA-1 不按名字)、编译前先验公证凭据,已有 mac 包时拒绝覆盖。一次性准备见 [quickstart](../../specs/063-release-channels/quickstart.md) §A。
- **这条规则是机制,不是习惯。** macOS workflow 只有在该次运行真的完成签名+公证时才挂 .dmg;凭据缺失时照常构建、校验三种架构,但什么都不挂,并在 job summary 里写明缺什么。凭据放在 GitHub Environment `release` 里(若启用,仅 `release/v*` 分支可用;按裁定目前留空——mac 包在本机签),清单与生成方法见 [quickstart](../../specs/063-release-channels/quickstart.md)。

### 版本号 — 日历版本 `YY.M.REVISION`(spec 066,2026-09-19 起)

三个数字是**年 · 月 · 当月第几次修订**,不是 major/minor/patch:`26.9.0` = 2026 年 9 月第一个版本,`26.9.1` 第二个,`26.10.0` = 10 月第一个。

- **月份是「发版那一刻」的月份**,不是被修的那条代码线的月份:10 月里给 26.9.2 出的修复就是 `26.10.0`。没有维护分支。
- **不补零**:`26.09.0` 会被 cargo 直接拒绝(`invalid leading zero`)、被 Chrome manifest 禁止,而 rpm/dpkg 会把它当成和 `26.9.0` 相等——一个版本两种写法。门禁直接拒绝,不做规范化。
- 修订号从 `.0` 起、逐个加一,不能跳、不能回头。
- 这些都由 `scripts/check-release-version.sh` 在门禁里执行(「现在」按全球任一时区算,所以 10 月 1 日早上在国内发 `26.10.0` 不会因为 UTC 还在 9 月 30 日被拒);规则的每一句在 `scripts/check-release-version.test.sh` 里都有用例,门禁先跑测试再信脚本。
- **两个容易漏的后果**:① 上了商店之后这是单行道——版本只能增大,`26.x` 之后回不到 `1.0.0`;② `release.yml` 里「`0.*` 自动标 pre-release」从此不再命中,`26.9.0` 会作为正式版(GitHub 的 Latest)发布。
- 只有**各端 App** 用日历版本;`vela-core` 等 Rust crate 是有 API 契约的库,继续用 SemVer。

下文的 `vX.Y.Z` 都读作 `vYY.M.R`。

### 发版步骤 — 推 `release/vX.Y.Z` 就是发版

创始人 2026-09-19 的裁定([spec 064](../../specs/064-release-from-branch/spec.md)):不再在 main 上打 tag。

```bash
git switch -c release/v26.9.0 main    # 在这个分支上改四处版本号,提交
git push origin release/v26.9.0       # ← 这一步就是发版
```

`.github/workflows/release.yml` 依次做四件事:

1. **门禁**(几秒,编译之前):分支名必须是 `release/vX.Y.Z`(纯数字——`-alpha` 这类后缀会同时打断扩展 manifest、rpm 的 `Version:` 和 iOS 的 `CFBundleShortVersionString`;「不是正式版」用 pre-release 标记表达,0.x 自动带上);四处声明的版本必须都等于它——`app-desktop/vela-wallet/Cargo.toml`(+`Cargo.lock`)、`extension/manifest.json`、`app/build.gradle.kts` 的 `versionName`、`project.pbxproj` 的 `MARKETING_VERSION`;**该版本不能已经从另一个 commit 发布过**——已发布的包不在下载过的人手底下被替换,要修就升版本号(同一个 commit 重跑没问题,会重新挂载)。
2. **构建**:六个打包 workflow 被**调用**(`workflow_call`),全部从同一个 commit 构建。每个构建 job 自带一步断言:包里写的 commit 就是检出的那个 commit。
3. **发布**:一个 tag `vX.Y.Z`、一个 GitHub Release,建在**发布分支的那个 commit 上**(`gh release create --target`)。挂桌面(Windows、Linux)和扩展;手机包只留作 workflow artifact(spec 063)。
4. **推进 `released` 分支**到该 commit(只快进)。Cloudflare 的 `vela-wallet-web` 以 `released` 为生产分支,所以 Web 钱包和其它所有包是同一个 commit。(指针分支不能叫 `release`:git 把分支名当路径存,它无法与 `release/v*` 共存。)

然后在创始人的 Mac 上补 mac 包(spec 063 §3a):`git fetch --tags && git checkout v0.9.3 && cd app-desktop/vela-wallet && ./scripts/release-macos-local.sh v0.9.3 --upload`。

**每个壳的「关于」都显示 `X.Y.Z (commit 前 7 位)`**,来源是同一条规则:构建设置的 `VELA_GIT_COMMIT`(CI 取自检出的 commit;Cloudflare 构建取自 `WORKERS_CI_COMMIT_SHA`),否则 `git rev-parse`,否则写 `unknown`——绝不是一个看起来像 commit 的常量。(spec 064 之前,桌面端每一个发出去的版本都显示设计稿的 `6ab8f`,Web 和扩展显示 `v1.0.0 (6ab8f)`。)

**副作用,需知情**:合并进 main 不再让 Web 钱包上线——它跟着发版走。官网(`getvela`)仍从 main 部署。

桌面另有 `metainfo.xml` 的 `<release>` 与 rpm spec 的 `%changelog` 要随版本更新(**星期几必须与日期相符**,否则 `rpmbuild` 直接报错)。

### 付过学费的坑

1. ~~tag 必须一个一个推~~ —— **已退役(spec 064)**:不再手推 tag,`release.yml` 在发布 commit 上自己建。(原始教训留档:0.9.0 时一条 `git push` 推四个 tag,零个 workflow 被触发。)
2. ~~不要对已有 release 的 tag 做「删除再重推」~~ —— **已退役为门禁规则**:同版本换 commit 会被 `release.yml` 直接拒绝。(原始教训留档:GitHub 会把原 release 变成草稿,workflow 再建一个同名的,同一个 tag 下两个 release。)
3. **PR 全绿对打包路径没有信息量——仍然成立。** 所有打包/归档 job 在 PR 上都跳过,PR 只跑元数据校验。0.9.0 一次暴露的三个缺陷全都因此潜伏了数周。**改了打包相关的东西,合并前在分支上 `gh workflow run "<workflow>" --ref <branch>` 真跑一次。**

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
