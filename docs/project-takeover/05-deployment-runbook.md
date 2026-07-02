# 05 — 部署手册 (Deployment Runbook)

> 现状:全手动部署,无 CI/CD 历史(本次接管新增 `.github/workflows/ci.yml` 作为门禁,尚未接部署)。三个可部署单元互相独立。

## 部署单元一览


| 单元        | 产物                     | 目标                                 | 命令                                                                 |
| ------------- | -------------------------- | -------------------------------------- | ---------------------------------------------------------------------- |
| Web 钱包    | `dist/`(静态)            | Cloudflare Pages(wallet.getvela.app) | `npm run build:web` → CF Pages 上传/`wrangler pages deploy dist`    |
| 官网+API    | `.svelte-kit/cloudflare` | Cloudflare Workers(getvela.app)      | `cd getvela.app && bun run deploy`                                   |
| iOS App     | .ipa                     | App Store Connect                    | Xcode/`expo run:ios --configuration Release` + 上传                  |
| Android App | .aab                     | Google Play                          | `cd android && ./gradlew bundleRelease`(需 keystore.properties,见下) |

另有两个**独立仓库**的服务(不在本仓库,部署互不耦合但语义耦合):vela-bundler(gas 报价与错误文案)、p256-index(公钥索引)。

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

- [ ]  `git status` 干净、在 main 上(commit hash 由 `app.config.js` 构建时注入——脏工作区构建会给"未提交的代码"打上"最近 commit"的标签,线上无法溯源)
- [ ]  若改过 bundler 错误文案或 `parseBundlerUnderfunded`:与 vela-bundler 仓库联合验证
- [ ]  若改过 approval-guard / 签名编码:在 parallel space 手动过一遍 clear-signing 场景页
- [ ]  若动过依赖:重跑全量 E2E

## Web 钱包发布

1. `npm run build:web`(内含 fix-cf-pages-assets,勿跳过——CF Pages 会丢 `node_modules` 路径资产)
2. 部署 `dist/` 到 CF Pages
3. **Smoke Test(生产域)**:
   - 打开 wallet.getvela.app → 首屏加载、无控制台报错
   - 已有钱包:余额加载、Receive 显示地址、Activity 渲染
   - passkey 登录弹窗出现(rpId=getvela.app)
   - 确认**没有** PARALLEL SPACE 紫色徽章(若出现=你在 fixture 空间,立即排查)
4. 回滚:CF Pages 控制台一键回滚到上一个 deployment(静态产物,无状态,秒级)

## 官网/API 发布(getvela.app)

1. `cd getvela.app && bun run deploy`
2. 生产密钥(只需一次/轮换时):`wrangler secret put ALCHEMY_API_KEY / PIMLICO_API_KEY / GITHUB_BUG_TOKEN`
3. Smoke:`curl -s https://getvela.app/api/exchange-rate?...`;`curl -s https://getvela.app/.well-known/apple-app-site-association`
4. 回滚:`wrangler rollback` 或重发上一个 commit 的构建
5. **注意**:API 部署影响钱包 App 的 bundler 代理路径——发布后立刻在钱包里做一次小额估算(不需提交)确认 `/api/bundler` 正常

## Android 发布(当前被阻塞,见 08)

前置(一次性,人工)。注意 `/android` 与 `/ios` 目录是 `expo prebuild` 生成物、**不入库**;release 签名逻辑由 `plugins/with-release-signing.js`(app.json 已注册)在每次 prebuild 时注入,手改 android/ 会被 `expo prebuild --clean` 抹掉:

1. `keytool -genkeypair … -keystore upload-keystore.jks -alias vela-upload`(**离库保存+备份**)
2. `cp keystore.properties.example android/keystore.properties` 并填真值(example 在仓库根)
3. Play Console 注册 + Play App Signing 开启
4. 从 Play Console 取 **app signing cert** 与 **upload cert** 两枚 SHA-256,写入 `getvela.app/src/routes/.well-known/assetlinks.json/+server.ts`,部署官网
5. 用 Google Statement List Tester 验证 assetlinks
6. 真机验证 passkey 创建/登录(DAL 生效需要签名匹配)

之后每次:`cd android && ./gradlew bundleRelease` → Play Console 上传 → 分阶段发布(建议 10%→50%→100%)。
**警告**:没有 keystore.properties 时构建会用 debug keystore 并打印 WARNING——该产物**禁止上传**。

## iOS 发布

- Team ID F9W689P9NE 已配好 AASA(`webcredentials:getvela.app`);确认 provisioning profile 含 Associated Domains entitlement
- Xcode Archive → App Store Connect → TestFlight 先行
- 真机 Smoke:passkey 创建、QR 扫描、一笔小额估算
- 回滚:App Store 无真回滚,靠"暂停分阶段发布"+加急审核,谨慎推 100%

## 数据迁移

无服务端用户数据库,无迁移流程。客户端 AsyncStorage 结构变更时必须在代码内做向后兼容读取(现有惯例,如 accounts 校验 `computeAddress` 自愈,`wallet-state.ts:118-120`)。
