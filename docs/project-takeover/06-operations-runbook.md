# 06 — 运维手册 (Operations Runbook)

## 可观测性现状(如实)

- **客户端无远程遥测/崩溃上报**(设计取向:隐私+单人团队)。诊断依赖:用户一键 bug-report(→ GitHub issues,含脱敏诊断信息)+ 控制台日志(带 `[Module]` 前缀)
- **getvela.app Worker**:Cloudflare 自带请求分析;建议开启 Workers Logs / Logpush(未配置,TODO)
- **告警:不存在**。建议最低配置:CF 上对 `/api/bundler` 5xx 率、Alchemy/Pimlico 用量阈值、p256-index 健康做告警

## 依赖服务与故障影响矩阵

| 服务挂了 | 用户看到什么 | App 内建降级 | 你要做什么 |
|---|---|---|---|
| 某链 RPC | 该链余额停更 | 缓存余额+多端点转移+封禁(`rpc-pool.ts`);429 只静默用缓存 | 通常自愈;必要时在设置里换 RPC 或推荐用户自配 |
| 所有内置 RPC(某链) | "网络异常"横幅 | `rpcFailedChains` 驱动 UI;缓存兜底 | 检查公共端点池;更新内置列表发版 |
| vela-bundler | 发送失败(明确报错)、估算失败拒绝提交 | 3 重试+existingHash 恢复;大 calldata 直接拒绝 | 查 bundler 仓库/宿主;**gas 报价与错误文案都以它为权威** |
| getvela.app/api | bundler 代理断 → 同上;汇率/NFT 缺失 | 服务端点可在 App 设置覆盖(`vela.serviceEndpoints`) | `wrangler tail` 看 Worker 日志;`wrangler rollback` |
| p256-index | 新钱包创建时"同步失败"(可重试);**新设备恢复找不到钱包** | 创建时 3 重试+pending 队列自动补传 | 独立仓库排障;确认 D1/DO 状态。**资金不受影响**,可安抚用户 |
| WalletPair relay | dApp 连接断 | 60s reconnecting 宽限+会话持久化自动重连 | 检查 relay 服务 |
| ERC-7730 registry | 清晰签名退化为 blind-sign 硬警告 | 本地描述符缓存 | 无需紧急处理(安全默认) |

## 常见故障排障

**"gas price too low" / 费率显示 "—"(Gnosis 高发)**
历史惨案区。原则:bundler 报价是权威,钱包 RPC 永不否决。查 `getBundlerGasQuote`(`safe-transaction.ts:1464`)与 bundler 侧 `pimlico_getUserOperationGasPrice`。回归测试在 `bundler-service.test.ts`。

**充值 modal 不弹 / 弹错**
`parseBundlerUnderfunded`(`bundler-service.ts:367`)字符串匹配 vela-bundler 错误文案。两仓文案是否同步?

**用户报"余额清零"**
某链 RPC 失败绝不应清零(merge-by-chain 回归测试保护)。若真发生,查 `wallet-api.ts` 缓存合并逻辑。

**passkey 创建失败(特定设备)**
`validateCreateClientData` 会拒绝字段顺序不合规的提供方(**故意的**,Safe 合约要求)——不是 bug,让用户换 passkey 提供方。iOS CANCELLED 态 vs 无凭据态见 checklist C3。

**Android passkey 全体失败**
99% 是签名指纹 ≠ assetlinks.json(DAL)。核对 Play app-signing cert SHA-256 与 `getvela.app/.well-known/assetlinks.json`。

**用户界面出现紫色 PARALLEL SPACE 徽章**
用户进入了测试空间(fixture 密钥,地址公开可花费)。指导:点徽章 → /parallel → 退出(恢复真实钱包缓存)。**告诫用户切勿向该空间地址收款。**

**汇率/价格为空**
`vela.nullPrice` 故障注入的生产等价物是价格源挂了:查 `fiat-rates.ts` 端点与 DEX quoter/Chainlink 回退。总额会少算——UI 有"价格不可用"处理。

## 模拟故障(开发环境复现)

浏览器控制台:`vela.failRpc(1)` / `vela.rateLimitRpc('all')` / `vela.slowRpc(3000)` / `vela.flakyRpc(0.5)` / `vela.nullPrice('all')` / `vela.clear()`。E2E 种子:`__VELA_FAULT_INIT__`。

## 密钥与凭据清单

| 凭据 | 位置 | 轮换 |
|---|---|---|
| ALCHEMY_API_KEY / PIMLICO_API_KEY | CF Worker secrets(生产);`getvela.app/.dev.vars`(本地,已 gitignore,从未入库) | 提供商控制台生成新 key → `wrangler secret put` → 验证 → 废旧 |
| GITHUB_BUG_TOKEN | CF Worker secret(fine-grained PAT,只有 issues 权限) | GitHub 设置轮换 |
| Android upload keystore | 本地/密码管理器(**绝不入库**,gitignore 已覆盖 `*.jks`/keystore.properties) | 丢失 = 走 Play 重置流程(有 Play App Signing 所以可恢复) |
| Apple 分发证书 | 开发者账号(Team F9W689P9NE) | Xcode 管理 |
| p256-index 服务端签名 key | 独立仓库/CF(本仓库不含) | 见该仓库 |

## 灾难恢复

- **用户资金**:非托管,链上 Safe + 用户 passkey;任何服务全灭都不影响资金所有权
- **最坏场景**:p256-index D1 数据丢失 → 已有设备不受影响(本地缓存地址);新设备恢复受阻 → 需从链上事件重建索引(Safe 部署事件含公钥)——**该重建脚本尚不存在,列入 08**
- Web/API 均为无状态静态/Worker,重部署即恢复
