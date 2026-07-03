# 08 — 未决事项 (Open Issues)

> 每项含:负责条件(谁/什么权限才能做)、优先级、验收标准。状态日期 2026-07-02。

## A. 上线阻塞(Android 商店路径)

### A1. Android 发布签名(P1,阻塞 Play 上架)——路径已定:EAS 托管(2026-07-03)
- **现状**:项目已接 EAS Build(eas.json + projectId + 双平台生产工作流)。签名主路径=EAS 云端生成并托管 upload keystore,**本地 keystore 保管责任消失**(建议 `eas credentials` 下载一份冷备)。`plugins/with-release-signing.js` 保留作本地 gradlew 后备路径,与 EAS 构建互不干扰(EAS 注入 `eas-build.gradle` 覆盖项目签名配置,插件的 debug-回退 WARNING 在 EAS 日志中属预期噪音)。
- **剩余步骤**:首次 `eas build -p android --profile production` → Play Console 注册 + Play App Signing → assetlinks 双指纹(app signing cert 取自 Play Console,upload cert 取自 `eas credentials`)→ Statement List Tester → 真机 passkey 验证(详见 05 Android 节)
- **负责条件**:创始人(Google Play 账号 + Expo 账号)
- **验收**:Play 内部测试轨道安装的构建,passkey 创建+登录成功

### A2. 真机验证清单 D 节(P1,两平台上架前)
- **现状**:`docs/NATIVE-LAUNCH-CHECKLIST.md` D 节列出的 iOS/Android 设备级验证未记录完成;2026-07-01 有一轮 native 修复(Hermes polyfills/passkey/QR/键盘)
- **负责条件**:真机(iOS 16+ / Android 14+)
- **验收**:D 节逐项打勾,尤其 passkey sheet、tx 签名、QR Back 键、键盘避让

### A3. 商店材料 + 付费下载配置(P1,提交前)
- 截图/描述/隐私表单:`docs/store-submission/` 已备好文案;Play 账户类型/封闭测试 12 人要求(个人账户)待确认
- **商业模型:Web 免费,商店版付费下载(定价未定)** → 额外前置:
  - 定价决策(创始人;注意两店价格档位机制不同,且 Apple/Google 抽成 15%(小商家)适用)
  - Apple:签署 Paid Applications Agreement + 银行/税务信息(App Store Connect → Agreements)
  - Google:Payments profile + 商家账号 + 税务表单
  - 付费应用退款政策文案;商店描述与 getvela.app 官网话术需一致(勿写"free app")
- **验收**:定价落定;两店付费协议/银行税务生效;提交表单全部填完并通过初审

## B. 产品/安全决策(需创始人拍板)

### B1. iOS iCloud KV entitlement(P2)
`modules/vela-cloud-sync` 未接线但 entitlement 已声明。**建议移除**(减少审核问题+攻击面);若近期要上 cloud-sync 则保留并在 App ID 开能力。
**验收**:二选一落地;entitlement 与实际能力一致。

### B2. iOS 重复 passkey(P2)
`register()` 无 `excludeCredentials` → 重复"创建钱包"会铸第二个 passkey/地址。决策:JS 层单设备单账户门,或允许多账户(当前多账户 UI 已存在)。
**验收**:决策文档化 + 若需要则实现门控。

### B3. getvela.app API 滥用防护(P2)
bundler/wallet/nft/transactions 代理无速率限制(bug-report 有)。**建议**:Cloudflare WAF rate-limiting rules(运维配置,不用改代码)+ Alchemy/Pimlico 用量告警。
**验收**:CF 规则生效 + 压测确认限流;提供商用量告警配置截图。

### B4. `/api/proxy` SSRF 加固(P3)
补 169.254.0.0/16、IPv6-mapped、数字 IP 形态;考虑目的地白名单。Workers 环境实际风险低。

## C. 工程债

### C1. CI 首跑验证(P2)✅ 已解决(2026-07-02)
首个试水 PR(main 自动部署切换,即引入本次修改的 PR)触发 app+site 两 job 真实运行——该 PR 合并即两 job 全绿的证明。后续动作:开 branch protection(required checks = app+site,勾 Do not allow bypassing)。

### C2. E2E 进 CI(P2)
本地 2.5 分钟全绿(62/63,1 skip)。需要 runner 上装 Chromium + 评估稳定性。
**验收**:CI 连续 10 次无 flake 后设为必过。

### C3. p256-index 灾备重建脚本(P2)
索引 D1 若丢失,新设备恢复受阻。Safe 部署事件含公钥,可链上重建。脚本尚不存在(属 biubiu-projects 仓库范畴)。
**验收**:演练:从链上事件重建一条索引记录并被 App 查询命中。

### C4. deployer-api.ts 休眠代码(P3)
3 个 TODO 的 mock 派生,无生产引用。删除或接真实 API。

### C5. 文档过期修正(P3)
`docs/test-plan.md`(263→1022 测试)、README(链数/费率/WalletPair 传输方式,见 `docs/CONTENT-SOURCE-100-CLUES.md` 为准)。

### C6. expo 工具链 moderate 漏洞(P3)
11 个 moderate 全在 @expo/* 家族,随下次 expo SDK 升级消除。基线记录在 04。

### C7. validateCreateClientData 未接线(P2,2026-07-02 训练题库构建时发现)
`src/services/public-key-upload.ts:36` 导出但生产代码零调用方——设计意图(创建时拒绝 clientDataJSON 字段顺序不兼容的 passkey 提供方)未落地,03 号文档原陈述已修正。若某提供方 create 兼容但签名时字段顺序不合 Safe 合约要求,用户可能在**收款后**才发现无法签名。
**决策**:接线到创建流程(推荐)或删除并依赖后置门。**验收**:二选一落地 + 单测覆盖不兼容 clientDataJSON 被拒场景。

## D. 已知行为限制(非缺陷,记录在案)

- passkey 不可导出:恢复 = 平台同步(iCloud/Google)+ p256-index;两者都丢则地址找不回(资金仍在链上,但无签名能力=永久锁定)。**面向用户的恢复说明文档尚未写**(建议列入上架前材料)
- 无第三方审计且未排期(memory: 定位话术是"开源+社区+AI 审查",**永远不说 audit is planned**)
- E2E 不做真实滑动确认发送(实链时序);链上真实发送靠 parallel space 手动+`parallel-onchain.spec` 的有限覆盖
