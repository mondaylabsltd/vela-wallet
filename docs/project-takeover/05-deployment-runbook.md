# 05 — 部署手册 (Deployment Runbook)

> 现状(2026-07-02 起):**Web 钱包 = merge 进 main 自动部署**(CF Pages git-connected,production branch = main;PR 每次 push 自动生成预览 URL)。官网/API 与移动端仍手动。`.github/workflows/ci.yml` 是合并门禁(branch protection 要求 app+site 两 job 绿才能 merge)。三个可部署单元互相独立。

## 部署单元一览


| 单元        | 产物                     | 目标                                 | 命令                                                                 |
| ------------- | -------------------------- | -------------------------------------- | ---------------------------------------------------------------------- |
| Web 钱包    | `dist/`(静态)            | Cloudflare Pages(wallet.getvela.app,git-connected) | merge 进 main → CF Pages 自动执行 `npm run build:web`(本地跑它仅作验证,无手动上传路径) |
| 官网+API    | `.svelte-kit/cloudflare` | Cloudflare Workers(getvela.app)      | `cd getvela.app && bun run deploy`                                   |
| iOS App     | .ipa                     | App Store Connect                    | `eas build -p ios --profile production`(EAS 托管证书;Xcode Archive 为后备) |
| Android App | .aab                     | Google Play                          | `eas build -p android --profile production`(EAS 托管 keystore;本地 gradlew 为后备,见下) |

另有两个**独立仓库**的服务(不在本仓库,部署互不耦合但语义耦合):vela-relay(gas 报价与错误文案)、p256-index(公钥索引)。

## 发布前检查(每次)

```bash
npx tsc --noEmit          # 必须 exit 0
npx expo lint             # 必须 0 errors
npx jest --ci             # 必须全绿(78 套件;网络套件默认 skip)
npm run build:web         # 必须 exit 0
npx playwright test       # 必须全绿(需本机 Chrome;~2.5 分钟)
npm audit                 # critical/high 必须为 0(当前基线:11 moderate,全在 expo 工具链)
cd getvela.app && bun run check   # 0 errors
```

发布 checklist 附加项:

- [ ]  (手动部署单元:官网/移动端)`git status` 干净、在 main 上——commit hash 由 `app.config.js` 构建时注入,脏工作区构建会给"未提交的代码"打上"最近 commit"的标签。Web 钱包对此已免疫:CF Pages 只从已合并的 main commit 构建
- [ ]  若改过 bundler 错误文案或 `parseBundlerUnderfunded`:与 vela-relay 仓库联合验证
- [ ]  若改过 approval-guard / 签名编码:在 parallel space 手动过一遍 clear-signing 场景页
- [ ]  若动过依赖:重跑全量 E2E

## Web 钱包发布(自动,2026-07-02 起)

1. 分支上开发 → PR → CI(app+site)绿 → merge 进 main
2. CF Pages 自动构建部署:执行 `npm run build:web`(内含 fix-cf-pages-assets,git 构建同样必需——CF Pages 会丢 `node_modules` 路径资产);About 页 commit 由 `CF_PAGES_COMMIT_SHA` 经 `app.config.js` 注入,永远等于所合并的 main commit
3. 合并前可用 PR 的 CF 预览 URL 做预检(每次 push 自动生成)
4. **Smoke Test(生产域)**:
   - 打开 wallet.getvela.app → 首屏加载、无控制台报错
   - 已有钱包:余额加载、Receive 显示地址、Activity 渲染
   - passkey 登录弹窗出现(rpId=getvela.app)
   - 确认**没有** PARALLEL SPACE 紫色徽章(若出现=你在 fixture 空间,立即排查)
   - About 页 commit = 刚合并的 main commit
5. 回滚:CF Pages 控制台一键回滚到上一个 deployment(静态产物,无状态,秒级)
6. 故障排查:**CI 绿 ≠ 已发布**——部署由 CF Pages 独立构建,若线上没更新,去 CF Dashboard 看构建日志(Retry deployment);GitHub Actions 里看不到 CF 的失败

## 官网/API 发布(getvela.app)

1. `cd getvela.app && bun run deploy`
2. 生产密钥(只需一次/轮换时):`wrangler secret put ALCHEMY_API_KEY / PIMLICO_API_KEY / GITHUB_BUG_TOKEN`
3. Smoke:`curl -s https://getvela.app/api/exchange-rate?...`;`curl -s https://getvela.app/.well-known/apple-app-site-association`
4. 回滚:`wrangler rollback` 或重发上一个 commit 的构建
5. **注意**:API 部署影响钱包 App 的 bundler 代理路径——发布后立刻在钱包里做一次小额估算(不需提交)确认 `/api/bundler` 正常

## Android 发布(主路径:EAS Build,2026-07-03 起)

签名主路径已切换为 **EAS 托管凭据**:首次 `eas build -p android --profile production`(或 `eas credentials`)时,EAS 生成 upload keystore 并存于 Expo 云端——创始人不再承担本地 keystore 保管/丢失风险(可用 `eas credentials` 下载备份一份离库冷存)。

前置(一次性):

1. `eas build -p android --profile production` → 选择让 EAS 生成 keystore(项目已接 EAS,projectId 在 app.json)
2. Play Console 注册 + 上传首个 AAB + Play App Signing 开启(EAS keystore 自动成为 upload key)
3. 取**两枚 SHA-256** 写入 `getvela.app/src/routes/.well-known/assetlinks.json/+server.ts` 并部署官网:
   - app signing cert:Play Console → App Signing 页(Google 重签名后的正式证书)
   - upload cert:`eas credentials`(Android → keystore → 显示指纹;内部测试轨道装的是这把签的)
4. Google Statement List Tester 验证 assetlinks
5. 真机验证 passkey 创建/登录(DAL 生效需要签名匹配)

之后每次:`eas build -p android --profile production`(eas.json production profile 带 autoIncrement,版本号源=remote)→ Play Console 上传(或 `eas submit`)→ 分阶段发布(建议 10%→50%→100%)。`.eas/workflows/create-production-builds.yml` 可一次出双平台生产包。

**与 `plugins/with-release-signing.js` 的关系**(经 EAS 构建流程文档核实):EAS 构建在 prebuild 之后向 `android/app/build.gradle` 末尾 `apply from` 注入 `eas-build.gradle`,用云端 keystore 的 release signingConfig **覆盖**项目内配置——所以 EAS 构建时本插件的 debug 回退分支虽会执行并打 WARNING,但随后被 EAS 覆盖,产物签名正确(该 WARNING 在 EAS 日志里属预期噪音,可忽略)。插件保留,服务本地 `expo run:android` release 变体的老路径(keytool + keystore.properties,见 keystore.properties.example);两条路径互不干扰。

## iOS 发布

- Team ID F9W689P9NE 已配好 AASA(`webcredentials:getvela.app`);确认 provisioning profile 含 Associated Domains entitlement
- Xcode Archive → App Store Connect → TestFlight 先行
- 真机 Smoke:passkey 创建、QR 扫描、一笔小额估算
- 回滚:App Store 无真回滚,靠"暂停分阶段发布"+加急审核,谨慎推 100%

## 数据迁移

无服务端用户数据库,无迁移流程。客户端 AsyncStorage 结构变更时必须在代码内做向后兼容读取(现有惯例,如 accounts 校验 `computeAddress` 自愈,`wallet-state.ts:118-120`)。
