 

# Vela Wallet — 100 条人工测试线索（手动测试用例指南）

> **用途.** 这是一份给"回头人工逐条过一遍"用的手动测试指南：每条线索说清**测什么、怎么测、预期结果**，并按**重要程度从高到低**排序（第 1 条最重要）。
> 内容全部来自对 vela-wallet 源码的真实通读，每条附**源码引用**便于回溯。这是 alpha 阶段产品的回归/验收清单，不是营销文案。
>
> 配套文档：营销/文案事实库见 [CONTENT-SOURCE-100-CLUES.md](CONTENT-SOURCE-100-CLUES.md)；自动化测试计划见 [test-plan.md](test-plan.md)。本指南聚焦**人工**测试。

## 测试环境与前置准备

- **平台.** 应用同源码跑在 iOS / Android / Web。能在 Web 上验的优先用 Web（`npx expo start --web`，默认 http://localhost:8081）。涉及**生物识别 / 触感 / 相机 / 云同步 / 深链**的需在**真机**上验，每条会在「平台」里标注。
- **Web 上的 passkey.** 无真机时，在 Chrome DevTools → **WebAuthn** 面板启用 virtual authenticator 即可模拟创建/使用 passkey；Mac 上 Safari/Chrome 也可用 Touch ID。
- **故障注入.** 浏览器 console 里的 `vela.*` 命令可模拟 RPC down / slow / null-price 等，用来验证失败态 UX（源码 `src/services/dev/fault-injection.ts`）。
- **清晰签名测试页.** 路由 `/clear-signing-test` 可在没有真实 dApp 的情况下逐个触发签名场景。
- **资金.** 建议在测试网或仅放极少量资金的账户上测真实交易；不可逆操作（首笔 gas 账户充值不可退款）务必先在便宜的链/小额上验证。
- **已知陷阱.** 主仓 README 已过时（写的是 8 链 / 60% markup / 蓝牙）；以本指南预期值为准：**12 链、费用约 2×、~3× 上限、WalletPair WebSocket（无蓝牙）**。

## 优先级图例


| 级别   | 含义                                                                                                   |
| -------- | -------------------------------------------------------------------------------------------------------- |
| **P0** | 资金 / 安全 / 不可逆：失败会直接导致丢资金、被钓鱼、被盗授权或账户不可恢复。**必测，每次回归都要过。** |
| **P1** | 核心功能正确性：钱包"能正常用"的主干（收发、余额、连接、gas、网络）。                                  |
| **P2** | 重要体验 / 边界 / 多链 / 本地化 / 恢复。                                                               |
| **P3** | 打磨 / 小众 / 展示性。                                                                                 |

本指南共 100 条：P0 × 25，P1 × 50，P2 × 25，P3 × 0。

---

## 测试用例（按重要程度排序）

### 1. 无限授权被拦截（ERC-20 Approve）

**`P0`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：ERC-20 unlimited approve（uint256-max）必须被拦截；钱包永不允许无限授权离开。这是资金安全的核心防线。
- **怎么测**：1) 在设置 → 清晰签名测试中点击「ERC-20 Approve」场景；2) 观察授权方为 1inch Router，金额为 uint256-max；3) 预期：确认按钮禁用，显示「无限授权已禁用」提示；4) 尝试点击确认按钮，应不响应；5) 编辑金额为有限值（如「1000」USDC），确认按钮才启用。
- **预期结果**：1) 初始状态：「Spending cap」部分显示「Unlimited」带危险红色警告；2) 确认按钮灰显且禁用；3) 输入框显示「无限授权已禁用 - 设置有限额度后重试」；4) 用户修改金额后，确认按钮变蓝可点击；5) 无其他路径绕过此拦截（包括滑动确认）。
- **边界/异常**：尝试通过粘贴 uint256-max（或 2^255、2^160-1）到自定义字段；应仍被拦截且显示「无限授权已禁用」。
- **源码参考**：`src/services/approval-guard.ts:30-45（UNLIMITED_CAP_256），src/components/signing/EditableApproveCard.tsx:68-80（validate choice）`

### 2. 编辑授权后 enforceNoUnlimited 最终守卫

**`P0`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：用户编辑授权后，提交前必须再次检查 enforceNoUnlimited，确保没有无限值离开钱包。这是不可绕过的最后防线。
- **怎么测**：1) 点击「ERC-20 Approve（无限）」；2) 在自定义框输入「999999999999999999999999」（远小于 uint256-max 但试图绕过检查）；3) 点击确认；4) 在 enforceNoUnlimited 中应仍捕获这是一个有限值并允许；5) 若某个攻击代码试图改写参数为无限值，submitting 时应 throw UnlimitedApprovalError。
- **预期结果**：1) 大数值但 < cap 应通过（e.g., 10^26 < 2^200）；2) Any value ≥ cap 应在 submit 时 throw；3) 错误信息「Unlimited approvals are disabled」；4) 钱包不签署请求，状态回滚到编辑模式；5) 无任何方式绕过此检查（no --no-verify style flag）。
- **边界/异常**：尝试通过链接数个较小的编辑（increaseAllowance 多次调用）达到无限；单个 increaseAllowance call 被拦截，但多个 call 组合的漏洞应被 wallet_sendCalls batch 中每条 call 单独检查。
- **源码参考**：`src/services/approval-guard.ts:377-396（enforceNoUnlimited），src/services/approval-guard.ts:370-375（UnlimitedApprovalError）`

### 3. 交易签名前显示无限授权防护，capped 后的参数应在批准时使用

**`P0`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：防止恶意 dApp 诱导无限授权(MAX_UINT256)。UI 在 EditableApproveCard 中自动将无限授权降级为有限额度(当前余额/2 或平衡值)，用户可再调整。批准时必须发送 rewritten 参数，不能还原为原始的无限值。
- **怎么测**：1) dApp 请求 approve(token, spender, MAX_UINT256)；2) 弹框显示 EditableApproveCard，初始值为有限额度(e.g. 1000 USDC)；3) 用户可在卡片内编辑数量；4) 用户点击"批准"；5) 签名应使用编辑后的值，不是原 MAX_UINT256。
- **预期结果**：EditableApproveCard 显示 token logo、名称、可编辑金额输入框(不显示 MAX 按钮)。批准时 approveRequest(paramsOverride: [{to, data: rewritten_calldata}])；signed request 记录 rewritten params, 不是原请求。enforceNoUnlimited(method, paramsOverride) 第二次检查应通过。
- **边界/异常**：用户强制编辑为 MAX 后应拒绝并显示错误；0 额度应允许(revoke)；小数点位数应尊重 token decimals。
- **源码参考**：`src/services/approval-guard.ts:enforceNoUnlimited, src/models/dapp-connection.tsx:493-501`

### 4. ERC-2612 Permit 离线签名显示为不可编辑

**`P0`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：ERC-2612 permit 和 Permit2 PermitSingle 是离线签名，dApp 自行在链上提交；钱包无法改写金额（会导致签名验证失败）。应禁用编辑，仅显示原始金额 + 警告。
- **怎么测**：1) 点击「EIP-712 Permit」场景（ERC-2612 permit）；2) 观察 EditableApproveCard 显示「1000 USDC」；3) 验证「自定义额度」编辑框禁用灰显；4) 显示提示「离线授权 - dApp 在链上自行提交其金额，钱包无法修改」；5) 点击「继续」签名。
- **预期结果**：1) 编辑框与「撤销」按钮均灰显，不可交互；2) 显示原始金额 + 过期时间；3) 金额旁有「unverified」标签（若 decimals 未验证）；4) 确认按钮正常可点击（无限额拦截）；5) 用户需明确认知他们正在签署的确切金额。
- **边界/异常**：Permit2 PermitBatch（多个代币）时，只要任一 amount ≥ cap160，应检测为 isUnbounded；但仍不可编辑。
- **源码参考**：`src/services/approval-guard.ts:210-274（detectTypedDataApproval），src/components/signing/EditableApproveCard.tsx:26-39（editable prop）`

### 5. 单币单人：收款人地址投毒防御（首次交互警告）

**`P0`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：地址投毒攻击通过近似地址诱骗用户错误复制。Vela 在确认屏检测「首次交互」并显示警告，强制长按确认，防用户盲目点击。
- **怎么测**：1) 进入 Send 填入一个**从未发过款**的新地址（非保存联系人）；2) 切到 confirm 屏；3) 观察地址下方是否显示「首次交互」或「First time」标签；4) 观察 Confirm CTA 从普通单击升级为长按确认（hold-to-confirm）；5) 长按并完成交易；6) 回到 Home，再次进入 Send 用同一地址，验证第二次不再显示警告和长按要求。
- **预期结果**：首次交互地址显示警告标签；confirm 按钮显示为需要长按（例如 "Hold to confirm"）；已交互过的地址移除警告和长按；长按 UI 应有视觉进度反馈。
- **边界/异常**：测试入参：已保存联系人的地址（应 NOT 警告）、contract 地址（分离显示 contract 标签）、zero address (0x00...）。
- **源码参考**：`src/services/recipient-risk.ts:67-76 (resolveRecipientRisk), src/screens/wallet/SendScreen.tsx:556-567 (recipientRisk state)`

### 6. 单币单人：Contract 地址警告（非钱包地址转账风险）

**`P0`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：向合约地址转账 ERC-20 可能导致代币永久丢失（合约可能无接收逻辑）。Vela 调用 eth_getCode 检测并警告，帮用户避免误转。
- **怎么测**：1) 进入 Send 选一个 ERC-20（如 USDC）；2) 在收款人字段粘贴一个**已知的合约地址**（例如 DEX 路由、token 合约本身）；3) 切到 confirm 屏，观察收款人地址下是否显示 「Contract」或「Smart Contract」标签；4) 继续提交（Vela 不应阻止，只是警告）；5) 用 EOA 地址重复同样的流程，验证不显示警告。
  前置：需要真实链或测试网上的已知合约地址。
- **预期结果**：contract 地址显示 "Contract" 或 "Smart Contract Account" 标签；EOA 地址不显示标签；标签仅为信息性，用户仍可继续提交。
- **边界/异常**：合约地址的 eth_getCode 调用失败（RPC down）时应 graceful 降级为 null（未知），不影响提交；zero address 的 is-contract 检测。
- **源码参考**：`src/services/recipient-risk.ts:33-48 (isContractAddress), src/services/contacts.ts:208-221 (classifyContact)`

### 7. SIWE 域名绑定检查与钓鱼防护

**`P0`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：Sign-In with Ethereum (SIWE) 消息中的 domain 必须匹配发起请求的网站 origin，否则显示大红钓鱼警告。攻击者可能显示「Uniswap」domain 但从恶意网站发起。
- **怎么测**：1) 点击「SIWE Phishing」场景；2) 观察 personal_sign 消息内容，domain=「app.uniswap.org」但 origin=「clear-signing-test」；3) 验证显示红色横幅「Domain Mismatch: Signed domain 'app.uniswap.org' ≠ Your site 'clear-signing-test' 🚨」；4) 确认按钮禁用或显示「危险」配色；5) 对比正常 SIWE（domain==origin），应显示绿色「✓ Domain verified」。
- **预期结果**：1) Mismatch 场景：大红警告，danger 风险等级；2) Match 场景：绿色「Domain verified」，normal 风险等级；3) 无 domain 的 personal_sign：显示「Plain message」，无钓鱼警告；4) 用户无法隐藏 mismatch 警告（no option to "trust anyway"）。
- **边界/异常**：Domain 带非标准端口（app.uniswap.org:8080）vs 消息中无端口：应允许匹配（hostname 比对，port 忽略）；subdomain（safe.app.uniswap.org vs app.uniswap.org）应 mismatch。
- **源码参考**：`src/services/siwe.ts:80-108（siweHost），src/services/siwe.ts:95-120（checkSiweDomainBinding）`

### 8. Blind Sign 无描述符时的警告与降级

**`P0`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：当没有 ERC-7730 描述符且 4-byte 数据库查不到函数时，钱包应显示大红「blind sign」警告，禁止用户无意识地签署不可读的交易。
- **怎么测**：1) 点击「Blind Transaction」场景（unknown selector 0x1a2b3c4d）；2) 观察 Intent 显示为不可读、风险为 danger；3) 显示红色警告横幅「无法验证此交易内容，仔细检查地址和金额」；4) 无法通过简单的「滑动确认」跳过，需明确的「我知道风险」确认；5) 确认后如常签署。
- **预期结果**：1) 大红标题 + 危险图标；2) Intent 为「Unknown Call」或「Unknown」；3) 字段为空或仅显示原始 calldata 截断；4) 无绿色/安全配色元素；5) 确认按钮明确要求用户主动确认风险。
- **边界/异常**：对比「best-effort」场景（4-byte 找到签名，但无 descriptor）：应显示「Caution」+「Not verified - from 4-byte database」标签，比 blind 更轻但仍需谨慎。
- **源码参考**：`src/services/clear-signing.ts:443-473（resolveBySelector + bestEffort），src/components/SigningRequestModal.tsx（未在摘录中，但应检查危险横幅渲染）`

### 9. 单币单人：输入金额→预期 max 包含 gas 预留

**`P0`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：原生币 max-send 必须自动预留 3× gas 估算量，防止 AA21 （无法支付 EntryPoint prefund）。这是资金安全的核心：全额转账原生币会留下 0 gas 导致失败。
- **怎么测**：1) 进入 Send，选原生币（ETH/BNB）；2) 点击 max 按钮；3) 观察输入金额 = balance - 3×gasEstimate；4) 提交交易，验证未失败；5) 链上检查交易成功。
  前置：钱包已创建，当前链有少量原生币（如 0.1 ETH），已连接真实网络或测试网。
  可在 Web 上用 vela.estimateGas() 控制台命令验证 gas 估算。
- **预期结果**：max 按钮显示的金额严格等于 balance - 3×fee.totalWei；转账完成后钱包仍保留足够的原生币用于后续交易的 gas。
- **边界/异常**：极端情况：余额 < 3×gas 估算时，max 应填 0；边界输入 0.000001 ETH 时应能识别为合法金额。
- **源码参考**：`src/screens/wallet/SendScreen.tsx:754-787, src/services/batch-send.ts:148-156 (reserveNativeGas)`

### 10. Max-send 预留 EntryPoint prefund：确保费用不会溢出

**`P0`** ｜ **分类** Gas / Bundler / 费用 / Tempo / 交易提交 ｜ **平台** All

- **测什么**：ERC-4337 发送的最大金额必须预留 gas 成本，否则 EntryPoint validation 会因余额不足而拒（AA21）。钱包应在 max-send 计算中减去估算 gas，或在确认时检查余额覆盖。
- **怎么测**：1) 有一个钱包，余额恰好 1 ETH。2) 进入 Send，选择该原生币，input 「Max」或手动输 0.99999999。3) 系统应估算 gas 并在后台计算 maxSendAmount = balance - gasCost。4) Confirm 屏显示可用金额 < 0.99999999（已预留 gas）。5) 确认发送，交易不应失败「AA21 didn't pay prefund」。
- **预期结果**：Max-send 按钮或 confirm 屏的「Available」应显 < 钱包总余额（已扣 gas reserve）。如果用户试图发送 >available，输入框应禁用或警告「Insufficient balance for gas」。交易成功后，钱包余额 = 原始 - 送出 - gas 费（三者之和正好）。
- **边界/异常**：Undeployed wallet：first tx gas cost 更高（2M prefund），max-send 应进一步减少。Tempo：max-send 预留的是 pathUSD，非原生。
- **源码参考**：`src/services/batch-send.ts 中 reserveNativeGas()`

### 11. sweep 模式（多币一人，清空钱包）：选择多个 token，gas 预留准确

**`P0`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：sweep 模式一次向一个地址转多个 token 的全部余额。原生币必须自动预留 3× gas，防 AA21。ERC-20 可全额转（gas 从 native 支付）。选择和预留逻辑必须准确，否则用户转错数量。
- **怎么测**：1) 进入 Send > token selector，选一条网络（如 Ethereum）；2) 观察 token 列表显示勾选框；3) 勾选 ETH（原生币）、USDC、DAI（都有余额）；4) 点 "Sweep" 或多选确认按钮，进入 enter-details；5) 观察 token 总结卡片：ETH 显示「Gas reserved」标签，数量 = balance - 3×fee；USDC/DAI 显示全额；6) 输入目标地址；7) 完成提交，验证链上各 token 数量与显示一致，ETH 扣除了 gas 预留。
  前置：同一链上持有多个 token 且都有余额。
- **预期结果**：多选后显示 "N tokens to recipient" 的总结；ETH/native 显示预留 gas 后的净金额，有「Gas reserved」标签；ERC-20 显示全额；总额计算正确；gas 估算改变时预留动态更新。
- **边界/异常**：多选后改网络，检测到链变应清除选择；仅选 ERC-20（无原生币）时不应显示 gas 预留行；原生币余额 < gas 时应移除该行（无法转）；同一链不同链的 token 无法混选（应在 UI 上过滤）。
- **源码参考**：`src/services/batch-send.ts:126-156 (toMultiTokenSpecs, reserveNativeGas), src/screens/wallet/SendScreen.tsx:606-647 (multiSelect helpers)`

### 12. sweep 模式：一个 MultiSend UserOp，无限授权防卫激活

**`P0`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：sweep 也用 Safe MultiSend，必须一个 UserOp 原子执行。更重要的是，Vela **永不生成无限授权**（unlimited approval），即使用户多币转账也是逐个 token 的限额转账，enforceNoUnlimited 守卫必须激活。
- **怎么测**：1) 进入 sweep，选 USDC + DAI，指定目标地址；2) 完成签名和提交；3) 查询链上 USDC/DAI 的 approval 记录，验证 allowance ≤ 转账额（不是 uint256.max）；4) 用控制台 `vela.injectSigningBug('unlimited')` 模拟生成无限授权的 bug，观察签名前的清晰签名 sheet 应该 block 并显示 "cannot use unlimited" 错误。
  前置：需要检查链上 approval event 或 allowance 状态。
- **预期结果**：sweep 提交后，每个 ERC-20 的 allowance 等于转账金额，不是 max；if 任何授权试图达到 uint256.max，enforceNoUnlimited 应拦截并错误提示；清晰签名 sheet 不显示无限授权选项。
- **边界/异常**：重复转账同一 token 时授权逻辑；token 已有某个授权时的增量更新；0x0 recipient 的边界。
- **源码参考**：`src/services/batch-send.ts (总体构建 sweeps), src/__tests__/services/dapp-signing-sendcalls.test.ts (enforceNoUnlimited 测试)`

### 13. split 模式：单个 MultiSend UserOp 批量提交，失败恢复正确

**`P0`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：split 的多笔转账必须在**一个 UserOp** 里完成（通过 Safe MultiSend），不是 N 个独立 tx。一个 op 失败，全部回滚。这是 gas 成本和原子性的保证。
- **怎么测**：1) 进入 split 模式，配置 3 个收款人各 1 token；2) 完成签名和提交；3) 观察交易状态页显示 1 个 userOpHash，不是 3 个分别的哈希；4) 查询链上或 Jiffyscan，验证只有 1 条 MultiSend 的 UserOp；5) 检查 Activity 历史，3 个发送共享同一 userOpHash，各有不同 id（hash-0/hash-1/hash-2）。
  前置：需要真实网络提交或测试网观察 UserOp。
- **预期结果**：提交后显示单个 userOpHash，不分裂成多个；Activity 每行都有分别的 id，但共享一个 userOpHash；链上验证所有转账在同一 MultiSend 执行；若一个收款人地址是黑洞，整个 MultiSend 应回滚，不部分成功。
- **边界/异常**：多个地址相同时的重复转账；地址是合约且 revert 的情况；gas 估算不足导致 MultiSend revert。
- **源码参考**：`src/services/batch-send.ts:82-87 (buildSplitCalls), src/screens/wallet/SendScreen.tsx:869-875 (split submission flow)`

### 14. 高价格拒绝保护：GasQuoteTooHighError 触发与显示

**`P0`** ｜ **分类** Gas / Bundler / 费用 / Tempo / 交易提交 ｜ **平台** All

- **测什么**：Bundler 报价 > 3× 链 gas 价时，钱包主动拒绝估算而非沉默过载。防止误配 bundler 或市场异常时用户被宰。
- **怎么测**：1) 在链上配置一个故意报高价的 bundler（或注入故障）。2) 进入 Send flow，尝试估算费用。3) 观察是否显示错误提示。4) 预期表现：估算立即失败，GasFeeCard 显「Estimate failed」（红色），可点击重试。error log 包含「3× the network rate」。
- **预期结果**：GasFeeCard 折叠态显红色「Estimate failed」+ refresh icon（可点）。开发控制台 error log："The relayer quoted an abnormally high gas price (3.5× the network rate). For your safety, Vela won't submit this transaction."。不允许用户手动 confirm/send。
- **边界/异常**：链有基础费飙升时：如果 bundler 的基础费是实时的，可能合理报高；此时应检查链的 eth_getBlockByNumber baseFee（见 getGasPrices）是否已同步更新。sendUserOp 时，若大小 calldata (>1024 bytes) 的 dApp tx 无法估算，也拒绝（见 ESTIMATION_REQUIRED_CALLDATA）。
- **源码参考**：`src/services/safe-transaction.ts:1305, src/services/safe-transaction.ts:1373`

### 15. 扫描 WalletPair QR 后显示 4 位指纹核对

**`P0`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：验证配对 URI 解析与指纹(fingerprint)生成。WalletPair 连接走 WebSocket 中继(无蓝牙)，需在 fingerprint 确认前阻止 E2E 通道打开，确保用户能跨设备人工核对。
- **怎么测**：1) 在 /wallet/connect 页面点击"扫描 QR 码"；2) 使用测试 WalletPair QR（格式 walletpair://...，含中继 URL 和 dApp 信息）；3) 扫描后等待 1-2 秒解析；4) 观察弹出的指纹卡片，显示 4 个数字、dApp 名称、绿色 E2E 锁徽章。
- **预期结果**：卡片展示「验证指纹」标题、4 个数字分别显示在 52×64 的单独方块内、monospace 字体、dApp icon 和名称、绿色 E2E 锁、"确认"和"取消"按钮各占半宽。无通道消息收发（只读状态）。
- **边界/异常**：指纹为全 0000/9999 等边界值时显示；中继 URL 缺失的格式（旧 QR）应弹出错误提示。
- **源码参考**：`src/screens/connect/ConnectScreen.tsx:49-52, src/services/walletpair-transport.ts:293-325`

### 16. 用户确认指纹后 E2E 加密通道连接成功

**`P0`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：指纹确认触发 confirmJoin，建立加密 WebSocket 通道。失败会导致 dApp 无法与钱包通信，必验证重连逻辑、超时处理、已连接状态标识。
- **怎么测**：1) 从指纹卡片点击"确认"按钮；2) 等待 confirmJoin 完成（SDK 应完成 join handshake）；3) 观察 UI 从指纹卡片切换到"已连接"状态卡片；4) 卡片应显示绿色圆点、"已连接"标题、dApp 名称、E2E 锁徽章。
- **预期结果**：已连接卡片显示：绿色活跃圆点(10×10)、"已连接"标题、dApp icon/名称、钱包地址短码(4+4)、当前链、绿色 E2E 锁徽章。WalletPair 会话持久化到 AsyncStorage(key: vela.walletpairSession)。
- **边界/异常**：confirmJoin 超过 30 秒(CONFIRM_JOIN_TIMEOUT_MS)应弹出超时错误；relay 无响应应检测。
- **源码参考**：`src/services/walletpair-transport.ts:389-404, src/models/dapp-connection.tsx:416-452`

### 17. 创建钱包-钱包地址幂等性与多链一致

**`P0`** ｜ **分类** 钱包创建与 Passkey（账户/恢复） ｜ **平台** All

- **测什么**：同一 Passkey 的同一 P256 公钥在所有12条链上必须计算出完全相同的 Safe 地址。风险：地址不一致导致用户在链A上的资金在链B上不可用。
- **怎么测**：1) 创建并成功验证一个钱包，记录成功屏幕上显示的地址（如 0x1234...）。2) 完成后进入钱包主页。3) 切换到不同的链（Settings > 选择Arbitrum/Optimism/Polygon等）。4) 进入 Wallet 标签页，查看账户地址。5) 在 Assets 标签页中再次查看。6) 返回 Settings，依次切换另外3条链，每次记录地址。7) 与第一次创建时的地址对比。
- **预期结果**：所有链上显示的账户地址完全相同，与创建时的地址一致。地址前缀为 0x，后跟40个十六进制字符。
- **边界/异常**：如果重启应用，地址仍应一致；从本地存储恢复账户后地址应幂等。
- **源码参考**：`src/screens/onboarding/CreateWalletScreen.tsx:116, src/services/safe-address.ts (computeAddress)`

### 18. 首次创建钱包-Passkey生物识别仪式

**`P0`** ｜ **分类** 钱包创建与 Passkey（账户/恢复） ｜ **平台** All

- **测什么**：验证钱包创建时成功激活生物识别（Face ID/Touch ID/PIN），无助记词存留。风险：生物识别失败或跳过会导致无法完成钱包创建。
- **怎么测**：1) 打开应用，进入欢迎屏。2) 点击「创建钱包」按钮。3) 输入账户名称（如"My Wallet"）。4) 勾选全部4个确认项（包括《条款》《隐私》）。5) 点击「创建钱包」按钮。6) 系统弹出生物识别对话框。7) 在真机上成功完成 Face ID / Touch ID 验证（Web使用虚拟验证器）。8) 验证身份后立即跳转到验证签名步骤。
- **预期结果**：生物识别对话框出现，用户完成生物验证后进入签名验证步骤；未见任何助记词、私钥或备份码在屏幕上显示；钱包地址在"成功"界面正常显示。
- **边界/异常**：如果用户在生物识别前取消（按返回/关闭对话框），需回到账户名称输入界面；多次生物识别失败应显示清晰错误提示。
- **源码参考**：`src/screens/onboarding/CreateWalletScreen.tsx:88-158, src/modules/passkey/index.ts:127-139`

### 19. 交易签名-每笔都需生物识别

**`P0`** ｜ **分类** 钱包创建与 Passkey（账户/恢复） ｜ **平台** All

- **测什么**：发送代币或与 dApp 交互时，每笔交易提交前必须重新触发生物识别。不应有"记住此设备"或无生物识别的签名快捷路径。
- **怎么测**：1) 登录钱包，进入 Wallet 或 Assets 标签。2) 发起转账：点击某代币 > "Send"。3) 填入收款地址和金额，点击确认。4) 系统弹出签名确认屏幕（Clear Signing Sheet），显示交易细节。5) 点击"Sign"按钮。6) 立即弹出生物识别对话框。7) 完成生物识别，交易提交。8) 几秒后交易进入待处理状态。9) 快速发起第二笔转账（无延迟）。10) 再次被提示生物识别。
- **预期结果**：每笔交易的签名步骤都触发生物识别，无缓存/跳过机制；生物识别失败或取消应导致交易不被签名。
- **边界/异常**：快速连续签署多笔交易时，每笔都应独立弹起生物识别（不应累积或批量）。
- **源码参考**：`src/hooks/use-dapp-signing.ts, src/modules/passkey/index.ts:144-275`

### 20. 公钥提取与地址计算-CBOR与P256验证

**`P0`** ｜ **分类** 钱包创建与 Passkey（账户/恢复） ｜ **平台** All

- **测什么**：Passkey 注册返回的 attestation object 必须正确解析以提取 P256 公钥（x, y 坐标），并计算出正确的 Safe 地址。公钥提取失败应被明确报告。
- **怎么测**：开发工具验证（Web）：1) 创建钱包 Eve。2) 在浏览器 DevTools 中监听并记录完整的 attestation object 十六进制。3) 手动运行 extractPublicKey() 来解析，验证 x, y 长度均为32字节。4) 验证地址计算：publicKey = '04' + x + y，再调用 computeAddress()，得到的地址应与屏幕显示一致。5) 对多个不同设备的 Passkey（若可用）重复验证。
- **预期结果**：Passkey 完成后，公钥正确提取（不为 null）；地址计算稳定幂等；地址格式为有效的 20 字节以太坊地址。
- **边界/异常**：如果 attestation object 畸形或不包含公钥（attest flag 未设），系统应显示"Failed to extract public key from attestation"错误。
- **源码参考**：`src/screens/onboarding/CreateWalletScreen.tsx:107-116, src/services/attestation-parser.ts:23-45`

### 21. pending→confirmed 对账及持久化

**`P0`** ｜ **分类** 历史与活动 / 交易详情 / 对账 ｜ **平台** All

- **测什么**：提交交易时立即保存为 pending（含 userOpHash），App 关闭/重启后，下次聚焦 Home 或打开 History，reconciler 自动查询 bundler 并将 pending 翻转为 confirmed（含 txHash）。这是"never lose a pending tx"保证。
- **怎么测**：1) 发送交易。2) 观察它在列表中显示为 pending，保存到本地存储（AsyncStorage）。3) 强制关闭 app（后台杀死或重启手机）。4) 重新打开 app 并进 Home 或 History。5) 等待对账逻辑运行（通常 12 秒内）。6) 观察 pending 转为 confirmed。
- **预期结果**：关闭前 pending 记录存储，重启后在 Home 或 History 加载时触发 reconcilePendingTransactions()，pending 交易在 12 秒内变为 confirmed（若链上已确认）；交易不丢失，status 和 txHash 正确更新。
- **边界/异常**：bundler 无响应：pending 维持，不被错误标记为 failed；24h+ 老 pending：停止重新轮询（保持 pending）。
- **源码参考**：`src/services/tx-reconciler.ts:47-92; src/services/storage.ts:417-445`

### 22. eth_sendTransaction 提交后立即记录 pending 状态，关闭弹框不丢失

**`P0`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：UserOp 一旦被 bundler 接受，应立即(同步)保存 pending 记录(带 userOpHash)，即使用户关闭签名弹框或重新加载页面，连接面板仍可查看、刷新状态。失败会导致用户丢失交易踪迹。
- **怎么测**：1) 发起 eth_sendTransaction；2) 点击"批准"，触发 passkey 签名；3) 签名完成后、bundler 接受 UserOp(onSubmitted 回调触发)，弹框显示"已提交，等待确认"banner；4) 立即关闭弹框或回到主页；5) 导航到 Home 的"连接"标签，应看到新交易记录，状态为"pending"。
- **预期结果**：pending 记录 id=`dapp-${nowMs}-tx`, type=dapp_tx, status=pending, userOpHash=0x..., txHash=''(未确认前为空)。关闭弹框后面板刷新应读到该记录。timestamp 为秒级精度。
- **边界/异常**：UserOp 被 bundler 拒绝(underfunded/invalid)应直接标记 failed，不经过 pending；网络慢时再次打开签名弹框应显示同 userOpHash 的 in-flight 状态而非重新签名。
- **源码参考**：`src/models/dapp-connection.tsx:538-546, src/services/dapp-history.ts:145-160`

### 23. Bundler 资金不足时的融资模态框（BundlerFundingModal）

**`P0`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：若 bundler 的 gas 账户余额不足，提交前应弹出融资模态框，引导用户向指定地址转账充值。充值后用户重新提交，流程应顺利完成。防止用户在签名后才发现资金不足导致浪费。
- **怎么测**：1) 清空测试网 bundler gas 账户（测试脚本或后端操作）；2) 进入 Send，填完所有细节点 continue 进 confirm；3) 尝试 confirm/sign，应收到 bundler error "bundler gas account insufficient"；4) 观察弹出 BundlerFundingModal，显示 deposit address + 推荐充值额；5) 按说明向 deposit address 转账（可用 Vela 或外部钱包）；6) 模态框应有「Send transaction」或「Retry」按钮，点击回到 confirm；7) 再次 confirm/sign，交易应成功；8) 查链确认 userOp 被正确执行。
  前置：需要 bundler 和 gas 账户的测试环境控制。
- **预期结果**：bundler 未资金时弹出模态框（不是错误）；显示准确的 deposit address；推荐充值额正确（threshold + 缓冲）；充值后重试成功；模态框不重复出现（防无限循环，重试计数 ≤ 3）。
- **边界/异常**：充值不足仍失败时的错误（应切换到错误提示）；多链 gas 账户独立资金检测；Tempo 链 pathUSD gas account 的融资。
- **源码参考**：`src/components/ui/BundlerFundingModal.tsx, src/screens/wallet/SendScreen.tsx:789-809 (funding modal flow)`

### 24. 自定义网络添加与完整性校验（全契约+RIP-7212）

**`P0`** ｜ **分类** 网络与 RPC（自定义链/自托管端点/容灾） ｜ **平台** All

- **测什么**：验证添加自定义链时，钱包强制检查全部 11 个必需契约（Safe、EntryPoint v0.7、Safe 4337 Module、WebAuthn Signer 等）都已部署，且 RIP-7212 P256 precompile 可用；任何一项缺失都应拒绝添加，防止用户误添不兼容的链。
- **怎么测**：1) 进入「Add Token」页面或「Add Network」tab（或通过 QR 扫描 EIP-681 链请求触发）

2) 搜索或输入一条自定义/新链的 chain ID（如 81457 Blast 或本地测试网)
3) 点击该链的建议行，APP 开始：a) 通过 chain-registry 获取该链的 RPC URLs；b) 测试所有 HTTPS RPC 的延迟，选最快的；c) 通过最快 RPC 用 eth_getCode 逐一检查 REQUIRED_CONTRACTS 中 11 个契约地址是否非空；d) 用 eth_call 发送有效的 P256 签名到 0x0100 precompile 或检查其代码
4) 等待完成提示，观察兼容性结果卡片：成功则显示绿色「Compatible」与最快 RPC 的延迟；失败则红色错误信息列出缺失的契约名称与 RIP-7212 状态
5) 若兼容，点击「Add」保存；若不兼容，「Add」按钮应禁用且显示错误原因
6) 成功添加后，新链应立即出现在网络列表中且可用于收发

- **预期结果**：检查流程完整运行，无超时；若全部契约都已部署且 P256 可用，显示「Compatible」并允许添加；若缺任何一个契约或 P256 不可用，显示具体缺失清单，禁用「Add」；新增的自定义链在后续网络选择器中可见且可选用
- **边界/异常**：测试链注册表中不存在的链 ID（应报 not-found）；所有 RPC 都超时或无效（应报 All RPC endpoints failed）；链有一个契约部署、其他缺失（应列出所有缺失，不是部分通过）；P256 precompile 存在但签名验证返回 0（应识别为不可用）
- **源码参考**：`src/services/network-checker.ts:44-112; src/services/add-network.ts:42-53; src/components/ui/AddTokenPanel.tsx:114-138`

### 25. RPC 修复 URL 验证与链 ID 检查

**`P0`** ｜ **分类** 韧性 / 故障注入 / 错误态 / Bug 上报 ｜ **平台** All

- **测什么**：验证 RPC 修复流程在保存前必须验证 URL 可达且 chainId 匹配，防止用户意外保存死 RPC 或错误链的 URL（会导致余额查询完全失败）。
- **怎么测**：1) Home banner > Fix Polygon。2) 输入一个无效 URL（如 https://invalid-rpc-url.xyz）。3) 点击 Save，等待验证。4) 应显示 "Endpoint unreachable" 错误，不保存。5) 尝试输入一个正确的 RPC 但来自不同链（如 Ethereum 的 Alchemy URL）。6) 应显示 "Chain ID mismatch: expected 137, got 1"。7) 尝试输入一个正确的 Polygon RPC URL（如从 chainlist.org 或 drpc.org）。8) 保存成功，banner 消失。
- **预期结果**：无效/不可达 URL：显示错误，不持久化。错误 chainId：显示 mismatch 错误，不保存。正确 URL：验证通过，保存到 storage，banner 消失。
- **边界/异常**：输入没有 https:// 前缀的 URL（或 http://），验证是否拒绝或自动修正；输入 localhost RPC，验证在真机上是否也能使用（可能不可达）。
- **源码参考**：`src/components/ui/RpcTroubleBanner.tsx:63-99, src/services/rpc-pool.ts:probeRpcChainId`

### 26. 账户公钥上传-成功同步到服务器

**`P1`** ｜ **分类** 钱包创建与 Passkey（账户/恢复） ｜ **平台** All

- **测什么**：创建钱包后，Passkey 公钥必须上传到 Passkey Index 服务器以支持跨设备恢复。上传失败会留下待同步队列。风险：未上传导致新设备无法恢复账户。
- **怎么测**：1) 在创建钱包成功后（显示成功屏幕）观察「验证签名」按钮上方的状态文本。2) 若显示「正在同步密钥...」，等待完成。3) 若显示「同步失败」，点击「打开设置」检查 Passkey Index URL 是否正确且可访问。4) Settings > 确认 Passkey Index 端点 URL（应为 https://p256-index.getvela.app 或自定义配置）。5) 返回创建屏幕，点击「重试上传」。6) 观察日志或状态变化确认上传完成。
- **预期结果**：创建完成后状态显示为「正在同步密钥...」然后消失（或显示成功）；未见「同步失败」错误。若失败，Settings 中重试后应成功。成功标志：无待同步队列存留。
- **边界/异常**：网络超时或服务器暂时离线不应导致钱包本地丢失；幂等上传（重复提交相同公钥）应被服务器去重。
- **源码参考**：`src/screens/onboarding/CreateWalletScreen.tsx:61-86,138-144, src/services/public-key-upload.ts:69-114`

### 27. 待确认交易显示 pending 状态徽章

**`P1`** ｜ **分类** 历史与活动 / 交易详情 / 对账 ｜ **平台** All

- **测什么**：发送的交易在 pending 状态时，点开详情显示时钟图标和「Pending...」文字。这是 P0 因为用户需要看到交易未最终确认，否则可能重复发送或误认为已到账。
- **怎么测**：1) Web 上操作：发送交易后，立即进 History 点开详情卡。2) 观察「Status」行的徽章图标和文字。3) 等待 12 秒（reconcile 间隔）；如果有模拟的 bundler 故障，pending 应维持。4) 刷新屏幕，验证状态不变。
- **预期结果**：详情卡的「Status」行显示时钟图标（color.warning.base）和「Pending...」文字；txHash 为空或灰色显示「未确认」。
- **边界/异常**：长时间 pending（超过 24h）：旧交易应保持 pending 状态不被清除；pending 期间关闭 app 再打开，状态应还原。
- **源码参考**：`src/components/ui/TxStatusBadge.tsx:12-25; src/components/ui/TransactionDetailSheet.tsx:133`

### 28. 稳定币硬锚 $1 价格展示

**`P1`** ｜ **分类** 余额与资产组合（多链/价格/到账） ｜ **平台** All

- **测什么**：验证 USDC、USDT 等稳定币在任何链上的 priceUsd 总是被固定为 1.0，而非依赖 DEX 或 Chainlink。这是资产分类的基础。
- **怎么测**：1) 进入首页「资产」tab（HoldingsList），搜索或筛选 USDC（任意链）。2) 进入该 token 详情页 (TokenDetailScreen)。3) 查看 'Price' 字段是否显示 1.0 USD。4) 在 HomeScreen 刷新并观察 USDC 行的 USD 值是否与余额 1:1 对应（如 100 USDC = $100）。
- **预期结果**：所有稳定币（category=stable）在 TokenRow 和 TokenDetailScreen 中显示价格为 $1.00；在余额计算中，稳定币总额 = 代币数量 × 1.0（不受 DEX/Chainlink 波动影响）。
- **边界/异常**：跨多链对比同一稳定币（如 Ethereum 和 Polygon 的 USDC）；验证 USDC.e、USDT 等变体也都是 $1；测试自定义稳定币标记是否也应用 priceUsd=1。
- **源码参考**：`src/services/wallet-api.ts:445-446; src/models/types.ts tokenUsdValue`

### 29. ERC-7730 Clear Signing Intent 三段设计

**`P1`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：清晰签名 UI 必须呈现 Intent（第1层）→ Substance（第2层）→ Details（第3层）的三段，用户一眼读懂要签什么。
- **怎么测**：1) 访问 /clear-signing-test 路由；2) 依次点击多个场景（如「ERC-20 Transfer」「Uniswap Swap」「Seaport NFT Buy」）；3) 对每个场景验证：a) L1：大号彩色 intent 文案（Send、Swap、Buy NFT）；b) L2：代币卡片，显示方向箭头（sent=down-left，received=up-right）；c) L3：「Advanced」折叠详情，包含合约地址、链、原始参数。
- **预期结果**：1) Intent 文案清晰，风险色调（danger=红、caution=琥珀、normal/safe=黑）；2) 代币卡片组织清晰，发送/接收分离；3) 详情部分默认折叠，点击可展开；4) 无冗长技术参数混入 L1/L2，保持可读性。
- **边界/异常**：部分解码的 ERC-7730（partial=true），应在 Intent 旁显示「⚠ 不完整，显示已验证部分」黄色横幅；blind-sign 应显示大红「⚠ 无法验证，仔细检查」。
- **源码参考**：`src/components/SigningRequestModal.tsx:70-89（riskColors, intentColor），src/services/clear-signing.ts:36-65（ClearSignResult）`

### 30. 有限授权编辑与确认

**`P1`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：用户对无限授权主动设置有限额度；编辑界面必须精确展示、提交时重新编码成新的 calldata。
- **怎么测**：1) 点击「ERC-20 Approve」场景（uint256-max）；2) 在自定义金额框输入「500」；3) 观察金额换算（USDC 6 decimals = 500,000,000 base units）；4) 点击确认；5) 验证签名内容已改为「500 USDC」而非原始无限值。
- **预期结果**：1) 编辑框自动格式化，千位逗号正常显示；2) USD 等值线显示「≈ $500.00」（对稳定币）；3) 合约调用的第二个参数（amount word）从 0xfff...f 改为 0x1dcd6500（500,000,000 十六进制）；4) 其他参数（spender 等）保持不变。
- **边界/异常**：输入小数金额（「0.5」USDC = 500,000 base units）；输入超过代币精度的小数（如「1.0000001」USDC 6-decimal）→ 应拒绝或截断；输入非数字字符应清除。
- **源码参考**：`src/services/approval-guard.ts:282-348（rewriteApprovalParams），src/components/signing/EditableApproveCard.tsx:53-87`

### 31. ERC-721 NFT setApprovalForAll 布尔授权编辑

**`P1`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：setApprovalForAll 是布尔授权（true/false），无有限金额概念。UI 应显示「Grant」或「Revoke」按钮，禁用数字输入。
- **怎么测**：1) 点击「NFT Approve All」场景（BAYC setApprovalForAll true）；2) 观察 EditableApproveCard 切换到 BooleanGrantCard；3) 显示「Grant access to all NFTs」与「Revoke」两个选项；4) 默认选中「Grant」（无限默认值）；5) 点击「Revoke」切换；6) 签名后验证 calldata 第二个参数从 0x1 变为 0x0。
- **预期结果**：1) 无数字金额输入框；2) 两个互斥选项按钮；3) 「Grant」为红色/警告色（危险），「Revoke」为绿色；4) 确认按钮只在用户明确选择后启用；5) 风险等级：Grant=danger，Revoke=safe。
- **边界/异常**：在 setApprovalForAll false（撤销）的请求上，应默认选中「Revoke」且确认按钮立即可用；不强制用户再次确认。
- **源码参考**：`src/components/signing/EditableApproveCard.tsx:44-46（isBooleanGrant），src/services/approval-guard.ts:195-204`

### 32. 撤销授权（approve-to-zero）显示为安全绿色

**`P1`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：当用户手动设置授权为 0 或用「撤销」模式时，应渲染为绿色安全状态，无风险警告。
- **怎么测**：1) 在 EditableApproveCard 中，选择「Revoke」模式；2) 观察 UI 切换为「减少授权」语义；3) 确认按钮变为绿色成功色；4) 签名后，验证 calldata 第二个参数为 0x0。
- **预期结果**：1) 卡片背景色为绿色（success.base）；2) 标签改为「Reduce by」而非「Spending cap」；3) 金额显示「0」；4) 签名前无任何警告横幅；5) 列为「正常」或「安全」风险等级。
- **边界/异常**：从有限值编辑为 0 时的过渡动画；在 ERC-721 setApprovalForAll 场景中，false 应同样显示为安全。
- **源码参考**：`src/components/signing/EditableApproveCard.tsx:84-86（isReducing），src/services/approval-guard.ts:176（isReducing: true）`

### 33. Balance Change Preview 余额模拟预览

**`P1`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：签名前，钱包对交易进行链上模拟，显示用户钱包的净资产变化（received 绿、sent 灰），帮助用户验证预期流向。
- **怎么测**：1) 打开 ERC-20 transfer 或 swap 场景；2) 观察「Balance changes」卡片，列出每个代币的 delta；3) 验证：a) Received 代币显示绿色 up-right 箭头；b) Sent 代币显示灰色 down-left 箭头；c) 金额精确显示（小数点处理）；d) 模拟失败时显示红色「Expected to fail」。
- **预期结果**：1) 成功模拟：每行显示「[Logo] +1000 USDC」或「-0.5 WETH」；2) Received token 未验证时，显示「[token] +? (unverified)」；3) 模拟若 revert，显示红色 alert「Expected to fail: insufficient balance」；4) 若模拟无法运行（RPC 挂），degraded 为静态「Expected to succeed」或「No assets move」。
- **边界/异常**：自转（recipient == sender）：显示「All assets remain in your wallet」而非「No assets move」；underfundedNative=true（模拟成功但实际不够 gas）应显示红色「Native balance insufficient」。
- **源码参考**：`src/components/signing/BalanceChangePreview.tsx:26-96，src/services/tx-simulation.ts:41-80（AssetSimResult）`

### 34. EIP-712 签名内容部分解码与风险标注

**`P1`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：EIP-712 typed data 若只解码了一部分字段（declared > decoded），应显示黄色警告「Incomplete information」，用户无法完全信任签署内容。
- **怎么测**：1) 点击「EIP-712 Unknown」场景（CustomOrder，无对应 descriptor）；2) 观察能否成功解码 maker、amount、expiry、salt 中的一些；3) 若成功解码 2/4 字段，partial=true；4) Intent 旁显示黄色横幅「⚠ Incomplete - shown verified fields only」；5) 确认按钮显示「Sign anyway」。
- **预期结果**：1) partial=true 时风险评级不低于 caution；2) 显示已解码字段，隐藏未解码字段；3) 无法 blind-sign partial（若 fields.length=0），应降级；4) 用户清楚地知道他们未看到全部信息。
- **边界/异常**：当 declared=5 但 resolved=2，判定为 partial；当 declared=5 但 resolved=0，不显示 partial 而是 blind-sign 警告。
- **源码参考**：`src/services/clear-signing.ts:673-681（resolveEip712Entry partial），src/services/clear-signing.ts:1236-1257（assessRisk partial）`

### 35. 过期时间戳字段标注与风险提升

**`P1`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：若交易包含已过期的 deadline 或 expiry（timestamp < now），应标注「(expired)」并提升风险至 caution，防止用户签署已失效的操作。
- **怎么测**：1) 点击「Expired Swap」场景（Uniswap deadline=1700000000，已过期约 2.5 年）；2) 观察 Deadline 字段显示「Nov 14, 2023 08:26 (expired)」；3) 验证字段旁有琥珀色「⚠」图标；4) 整体风险从 normal 提升至 caution；5) 确认按钮可点击但标注「继续（交易已过期）」。
- **预期结果**：1) Expired 字段显示「(expired)」文案 + 琥珀色；2) 風险評級 ≥ caution；3) 无红色 danger 配色（unless 还有其他 danger 风险）；4) 用户看到此字段时会犹豫再三。
- **边界/异常**：Deadline 为 0 或超大值（uint48-max ≈ year 2100）：不显示「expired」，而是「No deadline」；未来的 deadline 应显示正常黑色。
- **源码参考**：`src/services/clear-signing.ts:1088-1106（formatDate expired），src/services/clear-signing.ts:1236-1257（assessRisk field.expired）`

### 36. 多链场景下链 ID 与网络选择一致性

**`P1`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：SIWE 或 EIP-712 domain 中的 chainId 必须与当前钱包选中的网络匹配，否则显示警告。防止用户在以太坊钱包账户签署一个 Polygon 消息，导致 replay 攻击。
- **怎么测**：1) 当前网络设为 Ethereum (chainId=1)；2) 打开一个 SIWE 或 permit 请求，其 chain ID=137 (Polygon)；3) 观察显示「⚠ Chain ID mismatch: Message for Polygon (137) but wallet is on Ethereum (1)」；4) 确认按钮禁用或降级；5) 切换钱包网络到 Polygon，警告消失，按钮启用。
- **预期结果**：1) Mismatch 时显示黄色/琥珀警告；2) 确认按钮禁用直到网络切换匹配；3) 無 chainId 的请求（如早期 personal_sign）不显示此警告；4) EIP-712 verifyingContract 与 chainId 推断的网络无关（contract 地址可跨链一致）。
- **边界/异常**：Local/hardhat 测试网（chainId=31337）与其他网络；custom RPC 网络无标准 chainId 时的处理。
- **源码参考**：`src/services/siwe.ts:22（chainId 在 SiweFields）；EIP-712 domain 内 chainId 字段`

### 37. 未验证代币小数点标注

**`P1`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：链上代币小数点查询失败时，钱包显示金额但标注「unverified」；这防止缩放错误（如 6-decimal USDC 显示成 18-decimal）误导用户。
- **怎么测**：1) 模拟 RPC 超时（使用 fault-injection console: vela.rpc('slow', 5000)）；2) 打开 ERC-20 transfer 场景，指向一个非标准 6-decimal 代币；3) 观察金额旁显示「Unverified decimals」或小盾标记；4) 检查合约的 decimals() 调用是否在 4 秒超时内返回；5) 如超时，显示「... tokens」而非「1,000,000 XXX」。
- **预期结果**：1) unverified=true 的字段显示灰色或与 verified 字段视觉区分；2) 无金额估值（≈ $）出现；3) 仅显示方向（sent/received）但金额标注为「未验证」；4) 不会因为 unverified 导致整个请求被拒绝，仅降级为 caution 风险。
- **边界/异常**：已知代币（knownTokenDecimals 命中）不应标注 unverified，即使链上查询失败；缓存的 decimals 应标注 verified。
- **源码参考**：`src/services/clear-signing.ts:365-391（warmTokenDecimals），src/services/tx-simulation.ts:257-286（enrichDeltas, trustworthy）`

### 38. dApp 发起 eth_sendTransaction 请求显示交易估算与费用层级选择

**`P1`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：交易请求需要估算 gas、显示 Safe 部署成本、bundler 费用、最高费用上限。失败或估算错误会导致 UserOp 被拒或费用过高。必验证 EIP-1559 参数、gas 账户预检、签名前的费用确认。
- **怎么测**：1) dApp 发起 eth_sendTransaction({to: token_addr, data: approve_calldata, value: 0x0})；2) 钱包显示签名弹框，加载中动画；3) 等待 gas 估算完成(应 <3s)；4) 观察"Gas & 费用"卡片显示 3 层(Standard/Fast/Custom)、每层显示 maxFeePerGas、总费用(ETH)、估算 gas 消耗。
- **预期结果**：Gas 卡片显示所选费用层级的 maxFeePerGas(hex 格式)、总成本(wei)、gas 单位数；最低 Standard 层应基于 eth_feeHistory 的 25 百分位；最高 Custom 层可手动编辑。费用信息应清晰可读(不能 NaN/undefined)。
- **边界/异常**：RPC 返回 null 的 feeHistory 应降级为固定值(1 Gwei)；gasPrice 异常高(>200 Gwei)应显示警告。
- **源码参考**：`src/services/safe-transaction.ts:估计 fee, src/components/ui/GasFeeCard.tsx, src/models/dapp-connection.tsx:509-520`

### 39. dApp 发起 personal_sign 请求时触发签名确认弹框

**`P1`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：已连接 dApp 向钱包发送签名请求(personal_sign)，触发 SigningSheet 模态框。需验证请求解析(method 映射)、参数转换(hex 消息解码)、UI 显示清晰签名内容。
- **怎么测**：1) 已连接到 dApp；2) dApp 调用 personal_sign([0x48656c6c6f, address])；3) 钱包应弹出全屏签名模态框；4) 确认模态显示"签名消息"intent、消息文本解码后可读("Hello")、dApp 名称、地址、链选择器。
- **预期结果**：模态标题为"签名", 消息内容显示解码后的 UTF-8 文本(或 hex 若为二进制), "批准"(蓝色)和"拒绝"(灰色)按钮, isSigning/signError 都为空。消息参数长度若过大应截断(MAX_SIGNED_CONTENT=5000)。
- **边界/异常**：纯 hex 的二进制数据(\x00-\x1F)应保持 hex 显示；非 UTF-8 字节应显示原 hex；空消息应处理。
- **源码参考**：`src/hooks/use-dapp-signing.ts:170-192, src/services/dapp-history.ts:75-90, src/components/SigningRequestModal.tsx:95-150`

### 40. wallet_sendCalls (EIP-5792 批量)单签一次提交多个操作

**`P1`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：dApp 通过 EIP-5792 wallet_sendCalls 发起多个调用(e.g. approve+swap)，钱包应合并为一个 UserOp 经 MultiSend，单次签名、单个 tx hash。验证批量解析、原子性声明(atomic: true)、无限授权卡篇应逐个检查。
- **怎么测**：1) dApp 调用 wallet_sendCalls({calls: [{to: token, data: approve_cd}, {to: swap_router, data: swap_cd}], chainId: 0x89})；2) 钱包弹框显示 MultiOp 或 Batch 标题；3) 显示两个操作卡片(Approve + Swap)；4) 两个 approve 卡片都应检测无限授权、都应显示 capped 值；5) 点击"批准"，一次签名。
- **预期结果**：弹框显示"Batch"或"多操作"标题；每个操作占一个卡片，显示 intent、token、金额；两个 approve 分别显示 capped 值，都是可编辑的；批准后生成一条 dapp_tx 记录(type 仍为 dapp_tx)，单个 userOpHash，关联 chainId=0x89。
- **边界/异常**：calls 数组为空应拒绝；required capabilities 不支持应返回 EIP-5792 code 5700；单个 call 时应走 sendNative/sendContractCall，多个走 sendBatchCalls；链 ID 不支持应拒绝前检查。
- **源码参考**：`src/hooks/use-dapp-signing.ts:341-435, src/services/approval-guard.ts(逐个检查), src/components/signing/EditableApproveCard.tsx`

### 41. 单币单人：余额不足/gas 不足警告（黄色提示，提交被禁）

**`P1`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：金额输入时，实时检测：(a) token 余额不足，(b) native token 余额不足以支付 gas。警告显示且 continue 按钮禁用，防用户盲目提交会失败的交易。
- **怎么测**：1) 进入 Send，选一个 ERC-20（如 USDC，持有 5 USDC）；2) 输入 6 USDC（超额）；3) 观察红色警告 "Insufficient USDC" 显示，continue 被禁用；4) 改成 5 USDC，警告消失，continue 可用；5) 选原生币（如 ETH）；6) 计算钱包 ETH 余额（假设 0.05），输入 0.04（应该足够）；7) 若 gas 估算 > 0.01 ETH，输入金额让 amount + gas > balance，观察警告 "Insufficient gas"，continue 禁用。
- **预期结果**：余额不足时显示对应 token 名称的红色警告，continue 按钮禁用；输入足够金额时警告消失，continue 可用；gas 不足警告提及 native symbol；Tempo 链的 pathUSD gas 不足应警告 "Need pathUSD for gas"。
- **边界/异常**：ERC-20 + native gas 不足（二重检测）；Tempo 链无原生币，gas 走 pathUSD；极小金额 (0.000001) 的边界检测。
- **源码参考**：`src/screens/wallet/SendScreen.tsx:414-501 (amount warning effect)`

### 42. 单币单人：法币输入模式下金额换算准确

**`P1`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：用户选择法币输入（如 USD→CNY）时，金额必须按用户的显示币种（不是美元）÷ token 价格换算。错误会导致用户送出错误数量的代币。
- **怎么测**：1) 进入 Send，选一个有价格的 token（如 USDC）；2) 点击金额下的转换切换箭头，切到法币模式；3) 输入法币金额（如 100 USD）；4) 观察上方显示的等价 token 数量；5) 在 Settings > 显示货币 改变首选币种（如改为 EUR）；6) 再输同样的法币金额，验证换算系数已更新；7) 提交和链上验证数量准确。
- **预期结果**：法币金额 ÷ (token_priceUsd × 显示币种_汇率) = 准确的 token 数量；切换显示币种后换算系数正确更新；转账金额与显示一致。
- **边界/异常**：小数位超过 token decimals（如 USDC 6dp）时应截断而不是四舍五入；zero-decimal 币种（如日元）在法币输入时不应显示小数点。
- **源码参考**：`src/screens/wallet/SendScreen.tsx:122-128 (resolveTokenAmount), 1060-1174 (fiat conversion UI)`

### 43. split 模式（一币多人）：添加/删除收款人行，总额计算准确

**`P1`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：split 模式允许用户一次向多个地址转同一 token，每个地址不同金额。总额 = 所有行的和，必须 ≤ balance。超额或地址错误应即时警告，防用户误操作。
- **怎么测**：1) 进入 Send，选一个 token（如 USDC，持有 10）；2) 点 "Add recipient" 进入 split 模式，看到 Recipient 1 和空的 Recipient 2；3) 在 R1 输入地址 A、金额 3；4) 在 R2 输入地址 B、金额 4；5) 观察总额显示 7，绿色无警告；6) 在 R2 改成 7（总额 10），总额仍绿色；7) 在 R2 改成 8（总额 11），总额变红，"Insufficient balance" 警告显示，continue 禁用；8) 删除 R2，验证回到单人模式；9) 再次进入 split，验证状态已清除。
  前置：token 余额已知，支持至多 20 收款人。
- **预期结果**：总额正确求和（base units）；超额时显示红色总额 + 警告文案，continue 禁用；每行地址错误显示红色 "Invalid address" 提示；删除最后一行时自动退出 split 模式回单人；行数 ≤ 20，超过时 +号禁用。
- **边界/异常**：金额小数位截断测试（6 dp USDC 输 1.9999999）；地址大小写混合；删除行时保留顺序；总额 = 0 时应禁用提交。
- **源码参考**：`src/components/send/MultiRecipientEditor.tsx, src/screens/wallet/SendScreen.tsx:570-594 (split mode helpers)`

### 44. 交易模拟预览：发送前显示资产变化汇总（BalanceChangePreview）

**`P1`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：confirm 屏应显示发送后的资产变化预览（-USDC, -gas fee）。模拟失败/不可用时应 graceful 降级不显示，不阻止提交。这给用户最后检查的机会。
- **怎么测**：1) 进入 Send，填完所有细节切到 confirm；2) 观察确认前能否显示一个「Balance Change Preview」or 「You'll send」的总结行；3) 预览应显示 -amount（token symbol）、-gas fee（native symbol）；4) 若模拟失败（如 RPC down），预览应不显示，但 confirm CTA 仍可用；5) 完成交易，链上验证最终资产变化与预览一致（或若无预览就跳过）。
  前置：完整的 Send 流程到 confirm 屏。
- **预期结果**：资产变化预览正确显示受影响 token 和 amount；失败时无预览显示但不阻止；提交后实际资产变化与预览一致。
- **边界/异常**：多币 sweep 的预览（多行资产变化）；gas 估算改变时动态更新；token 价格查询失败时的 USD 预览（应降级为无 USD 显示）。
- **源码参考**：`src/components/signing/BalanceChangePreview.tsx, src/screens/wallet/SendScreen.tsx:515-554 (sim effect)`

### 45. 交易完成反馈：签名→提交→确认 UI 状态流转，userOpHash 链接

**`P1`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：交易提交后 UI 应清晰展示进度：signing → submitted (userOpHash) → confirmed (txHash)。bundler 接受 = 支付已成功（不等链确认）。用户应看到可点击的 Jiffyscan 或 explorer 链接。
- **怎么测**：1) 完成 confirm 和签名；2) 提交后观察 UI 转为 "Submitting..."，然后「submitted」状态，显示 userOpHash 和 "View on Jiffyscan" 链接；3) 点链接，应打开浏览器跳到 Jiffyscan 查询该 UserOp；4) 等待链上确认（通常数秒到数十秒），UI 转为 "Confirmed"，显示 txHash 和对应 chain explorer 链接；5) 交易总结：from/to/amount/symbol/usd/time；6) 点「Done」返回 home，观察 Activity 历史。
- **预期结果**：UI 状态清晰：Signing → Submitted → Confirmed；userOpHash/txHash 可点击链接到 explorer；用户收到振动和视觉反馈（绿勾/成功动画）；交易总结信息完整。
- **边界/异常**：链确认缓慢（数分钟）时的 pending 状态；用户在 submitted 后关闭应用，重启后应保留待确认状态；txHash 查询失败时的 graceful（stays pending，可手动点 Jiffyscan）。
- **源码参考**：`src/screens/wallet/SendScreen.tsx:811-930 (executeTransaction + receipt), src/components/ui/TransactionReceipt.tsx`

### 46. 单币单人：收款人身份解析（ENS/.bnb/.arb/Basename/Passkey Index）

**`P1`** ｜ **分类** 转账发送（含 split/sweep/联系人/收款人识别） ｜ **平台** All

- **测什么**：输入有效地址时，自动查询 ENS/Basename/.bnb/.arb 等域名服务 或 Passkey Index（Vela 用户）获取人性化名字显示。减轻地址读数难度，增强信任。
- **怎么测**：1) 进入 Send，选任意 token；2) 在收款人字段输入一个**已注册 ENS 的地址**（如 vitalik.eth 对应地址或该地址逆解析为域名的情况）；3) 等待 2-3 秒，观察地址下方显示解析的域名 + 来源（"ENS"）；4) 粘贴一个 .bnb 或 .arb 注册地址，验证来源标签变为 ".bnb" 或 ".arb"；5) 输入 Vela 的一个已注册用户地址（来自 passkey-index 后端），验证名字显示 + 来源 "👤 Vela User"。
  前置：需要实际 ENS/域名已注册的地址或 Passkey Index 里真实存在的地址。
- **预期结果**：有效地址自动解析出人名；解析来源标签准确（ENS/Basename/.bnb/.arb/Passkey）；24h 缓存工作（重复输入同地址时第二次响应快）；解析失败时优雅降级（不显示名字，但不阻止提交）。
- **边界/异常**：同一地址多个域名服务同时有注册时，遵循优先级顺序（Passkey > .bnb > .arb > Basename > ENS）；reverse resolver 不返回数据时无错误；网络超时应用缓存 or null。
- **源码参考**：`src/services/recipient-identity.ts:232-267 (resolveRecipientIdentity), src/screens/wallet/SendScreen.tsx:504-513 (identity resolution effect)`

### 47. 首笔交易触发 Gas 账户激活弹窗

**`P1`** ｜ **分类** Gas / Bundler / 费用 / Tempo / 交易提交 ｜ **平台** All

- **测什么**：新钱包在主网/L2 首笔交易前，gas 账户余额不足会弹出激活弹窗。这是 underfunded → funding modal 的关键流，失败意味着用户完全无法发起交易。
- **怎么测**：1) 创建钱包且未激活 gas 账户（首次使用）。2) 进入 Send 流选择任意链与代币。3) 输入有效金额和地址，点击「确认」进入 confirm 屏。4) 观察是否弹出「Gas 账户激活」modal，显示「免费激活」与「自己充值」两个选项。
- **预期结果**：modal 出现，标题为「激活账户」(i18n: componentsUi.funding.title)，显示当前余额 0 + 链名（如 Ethereum）。两个蓝色卡片可点击，分别为「Free Activation」(绿色，显 FREE badge) 和「Self-fund」(灰色，显推荐金额如 0.0012 ETH)。cancel 按钮可关闭。
- **边界/异常**：边界：若 checkBundlerFunding 返回 null（bundler 不可达），不弹窗，允许交易继续（graceful）。异常：自定义 bundler（非内置）永不弹窗。
- **源码参考**：`src/services/bundler-service.ts:118, src/components/ui/BundlerFundingModal.tsx:53`

### 48. Free Activation 弹窗：资格检查与请求流

**`P1`** ｜ **分类** Gas / Bundler / 费用 / Tempo / 交易提交 ｜ **平台** All

- **测什么**：用户点击「免费激活」，bundler 检查资格（nonce ≤ 3、注册 passkey、treasury 余额）并赞助 gas 账户。失败时显示拒绝原因（nonce_exceeded / treasury_depleted 等），允许用户转向自己充值。
- **怎么测**：1) 触发激活 modal（见上一测试）。2) 点击「Free Activation」按钮。3) 观察加载动画与请求过程（最多 ~5s）。4) a) 成功：余额更新为绿色，显 ✓ 与「Continue」按钮；b) 失败：显示橙色拒绝理由条，并自动切换到「Self-fund」步骤。
- **预期结果**：成功路径：modal 显示绿色余额 + 「Continue」按钮。失败路径：显示 <Text style={styles.denialRow}></text> 包含 i18n 拒绝信息（nonce_exceeded / treasury_depleted / no_passkey_registered / rate_limited 等），并显示 QR + 地址供用户自己充值。
- **边界/异常**：超时处理：bundler 超时返回 pending_unknown，弹窗仍显示自己充值选项而非硬失败。幂等性：请求携带稳定的 Idempotency-Key，重试不会双重充值（见 bundler-sponsorship.test.ts）。
- **源码参考**：`src/services/bundler-service.ts:170, src/components/ui/BundlerFundingModal.tsx:89`

### 49. 自己充值激活流：显示 QR 与检查平衡轮询

**`P1`** ｜ **分类** Gas / Bundler / 费用 / Tempo / 交易提交 ｜ **平台** All

- **测什么**：用户选择「Self-fund」或 Free Activation 被拒，弹窗显示 QR 码 + gas 账户地址 + 推荐金额。用户充值后，「检查」按钮轮询 bundler 账户余额，达到阈值时自动解锁继续。
- **怎么测**：1) 点击「Self-fund」卡片（或被拒后自动转向）。2) 观察弹窗步骤：显示「激活需要金额」（如 0.0012 ETH）、QR 码、gas 账户地址（可复制）。3) 点击地址行复制地址（应显 ✓ 图标 2s）。4) 充值该地址 ≥ 推荐金额（可在浏览器 testnet faucet 做）。5) 点击「检查」按钮，观察转圈加载，3-5s 后若余额充足应显绿色余额 + ✓ Continue。
- **预期结果**：显示白色 QR 码（可扫）。地址框显示 gas 账户地址（如 0x1234…）。点击地址复制时 Copy 图标变为 Check + 绿色。推荐金额显示为适合链的数字（Gnosis 可能 << ETH）。充值后「检查」成功时底部「Continue」按钮激活（不再灰）。
- **边界/异常**：未触发充值：轮询 10s 一次，24h 后停止（reconcile_max_age）以防 hammering bundler。余额接近但不足时，显示红色警告。
- **源码参考**：`src/components/ui/BundlerFundingModal.tsx:67, src/services/bundler-service.ts:233`

### 50. Gas 费用估算与显示：单层到多层拆分

**`P1`** ｜ **分类** Gas / Bundler / 费用 / Tempo / 交易提交 ｜ **平台** All

- **测什么**：发送交易时，估算 UserOp 总费用并展示。折叠态显示「~0.0012 POL ≈ $0.003」，展开显示网络费 + 中继费的 gwei 拆分。这是用户成本感知的核心。
- **怎么测**：1) 进入 Send flow，选代币 + 输金额。2) 自动触发费用估算（estimateTransactionFee）。3) 观察 GasFeeCard 折叠状态：显标签「Est. Fee」+ 金额 + USD 近似值。4) 点击展开箭头，观察详情面板：tier 按钮 (Slow/Standard/Fast) + Network Fee (gwei) + Relayer Fee (gwei) + Total (gwei)。
- **预期结果**：折叠：「Est. Fee ~0.0012 POL ≈ $0.003」。展开后 VelaCard 显：[Slow][Standard][Fast] 按钮，当前选中的 tier 背景为黑，文字为白。下方行显 Network Fee 与 gwei 值（右对齐，monospace 字体），Relayer Fee 同格式，You Pay 为两者之和。底部 Gas Limit 与 Wallet Deployed (Yes/No)。
- **边界/异常**：估算失败：failed=true，折叠态显红色「Estimate failed」+ refresh 图标（可重试）。Tempo 上：relayer fee = 0，fee 直接显为 pathUSD（6 dec，无中继 markup）。
- **源码参考**：`src/components/ui/GasFeeCard.tsx:72, src/services/safe-transaction.ts:321`

### 51. Gas Tier 切换：Slow/Standard/Fast 与费用实时更新

**`P1`** ｜ **分类** Gas / Bundler / 费用 / Tempo / 交易提交 ｜ **平台** All

- **测什么**：用户可在 confirm 屏切换 tier（slow ×1.1 → standard ×1.2 → fast ×2.0），费用应立即刷新。防止错误提交超高费用。
- **怎么测**：1) 进入 Send confirm 屏，GasFeeCard 展开。2) 初始选 Standard。3) 点击「Fast」按钮，观察：a) 按钮背景变黑；b) You Pay gwei 值增大（约 1.67× standard）；c) 总费用增大。4) 切回「Slow」，观察数值减小。5) 点击 refresh（转圈）重新估算，观察费用可能浮动但保持 tier 倍数关系。
- **预期结果**：Fast 按钮点击后立即（<100ms）变黑+白字，其他按钮变灰。You Pay 值从（如）4.2 Gwei → 6.8 Gwei。总费用 ~0.0012 → ~0.0019。Refresh 按钮转圈，3s 后显 refresh icon，费用可能微调但 tier 倍数不变。
- **边界/异常**：高gas链（Monad）：bundler 估算时 tier 对费用的倍数可能大于 multiplier（限制因素是 gas 用量，非 gwei）。无 bundler quote 时：本地估算 fee = gasPrice × tier × 1.5（BUNDLER_MARGIN）。
- **源码参考**：`src/components/ui/GasFeeCard.tsx:93, src/services/safe-transaction.ts:225`

### 52. RPC 故障转移与多源负载均衡

**`P1`** ｜ **分类** 网络与 RPC（自定义链/自托管端点/容灾） ｜ **平台** All

- **测什么**：验证当一个 RPC 端点失败（网络错误、超时、server 错误）时，钱包自动转向下一个端点；端点评分系统优先使用：用户配置 > provider key (Alchemy/dRPC/Ankr) > Vela 内置 > 公共 fallback；最终选择最低延迟的端点；单个请求不应超过 8-15 秒。
- **怎么测**：1) 在 Web 上开启 Chrome DevTools → Network 限流到「Offline」，尝试执行任何需要 RPC 的操作（查询余额、发送交易、估算 gas）

2) 观察控制台日志 [RPC] 的输出，看是否尝试多个端点、故障转移、最终超时后重试一次（jitter backoff）
3) 恢复网络连接，再试一遍，应该恢复正常
4) 进入「Settings → Advanced → RPC Providers」，配置一个有效的 Alchemy/dRPC API key，观察该 provider 的端点是否在日志中以 `provider:` 标记出现且排序靠前
5) 验证如果 provider RPC 和内置 RPC 都可用，provider 的被优先尝试；如果 provider 返回 401/403（无效 key），应被临时封禁 1 小时，日志显示 BANNED 和原因
6) 关闭并重启应用，模拟多个链同时进行大量 RPC 查询（可用 fault-injection 的 vela.* 命令），观察不同链的请求是否并行且不互相饥饿

- **预期结果**：在单个端点失败后 1-2 秒内自动转向下一个；同一链所有端点都失败后，显示「RPC Trouble」banner 并标出哪些链失效；端点评分日志准确反映优先级；被禁端点在冷却期间不再被使用；最终完成的请求延迟 < 15 秒（不包括意图的故障注入延迟）
- **边界/异常**：所有 RPC 端点都被永久封禁（0 成功、≥6 次失败）→ 应自动清除封禁、重新尝试；端点返回 rate-limit 错误（-32001）→ 应被临时禁而不是永久禁；transient server error（-32603）→ 应转移而不是禁；网络突然恢复 → 不应重复提示「fixing」，应自动恢复无缝
- **源码参考**：`src/services/rpc-pool.ts:640-741 (poolRpcCall); src/services/rpc-pool.ts:356-392 (endpointScore); src/services/rpc-pool.ts:397-417 (isPermanentRpcError)`

### 53. 12条默认链列表显示与快速切换

**`P1`** ｜ **分类** 网络与 RPC（自定义链/自托管端点/容灾） ｜ **平台** All

- **测什么**：验证钱包正确列出全部 12 条支持的链（Ethereum、BNB、Polygon、Arbitrum、Optimism、Base、Avalanche、Gnosis、Unichain、Tempo、Monad、Worldchain），并且可快速点击切换，每条链的图标、颜色、原生 token 符号（如 POL、xDAI、USD）准确。
- **怎么测**：1) 打开钱包首屏或资产屏的网络选择器（通常是一个链图标按钮或「All Networks」文案）

2) 点击打开「Select Chain」sheet 或 modal
3) 逐一验证所有 12 条链按照 CHAINS.ts 中的顺序显示，检查各链的：iconLabel（如 ETH/POL/xDAI）是否对应；iconColor 和 iconBg 是否与设计稿一致
4) 点击任意一条链，验证 sheet 关闭，该链被选中（如果有筛选面包屑，应显示该链的标识）
5) 再次打开选择器，验证之前选中的链有 checkmark 或高亮显示
6) 点击已选链旁的清除按钮（✕）或点击「All Networks」，验证筛选被重置

- **预期结果**：Sheet 显示全部 12 条链名称、logo、原生 token 符号准确无误；点击链后立即关闭且选择生效；checkmark 和清除按钮工作正常；不出现 UI 闪烁或数据加载延迟
- **边界/异常**：在离线或网络缓慢（DevTools 限流 Slow 3G）下验证链列表是否仍然快速展示（这些数据本地缓存）
- **源码参考**：`src/models/chains.ts:42-119; src/components/ui/NetworkFilterSheet.tsx; src/components/ui/NetworkFilterButton.tsx`

### 54. 多链余额聚合无缓存时的 $0 闪现

**`P1`** ｜ **分类** 余额与资产组合（多链/价格/到账） ｜ **平台** All

- **测什么**：验证刷新时若无缓存但多链加载在途，总余额不会闪现 $0，而是显示上一次缓存值（displayTotal 的 Math.max 逻辑）。这是防止用户惊吓的关键。
- **怎么测**：1) 清空 App（卸载或 Settings 重置缓存）。2) 进入 HomeScreen，观察 hero 余额展示。3) 即使各链还在加载（每条链 18s 超时），余额应以缓存值显示或保持昨日值，不能是 $0。4) 所有链加载完毕后，余额自动跳到实时合计。
- **预期结果**：从 App 启动到所有链加载完毕的全程，余额柱子从不降到 $0；若本次完全无价格（某链 priceUsd = null），hero 余额仍显示缓存或上次成功的聚合值，页面顶部有 ⚠️ 'Stale Balance' 警告（见 HomeScreen hero 的 balanceStaleRow）。
- **边界/异常**：测试清空缓存后的冷启动；测试多链中仅 1 条超时（18s cap）的情况下其他链数据及时加载。
- **源码参考**：`src/screens/wallet/HomeScreen.tsx:161-168; src/services/balance-cache.ts:53-74`

### 55. DEX → Chainlink → null 价格兜底链路验证

**`P1`** ｜ **分类** 余额与资产组合（多链/价格/到账） ｜ **平台** All

- **测什么**：验证原生 token 价格按优先级解析：DEX swap quote（同链）→ 同链 Chainlink feed → Ethereum Chainlink feed → null。自定义 token 则优先 token→stablecoin direct DEX path，次选 token→wrappedNative 再乘以原生价格。
- **怎么测**：1) 进入 HomeScreen，观察原生 token（ETH on Ethereum, POL on Polygon 等）的价格加载。2) 查看浏览器 console，应看到 '[Price]' 日志，例如 'BNB → $612.45 via DEX' 或 'MATIC → $0.52 via Chainlink(local)' 或 'xDAI → $0.99 via Chainlink(ETH)'。3) 导入或持有一个自定义 ERC-20（如某个小币种），观察其价格行为。4) 若 DEX 报价与 Chainlink 偏差 >50%，应采用 Chainlink；若相差 <50%，采用 DEX。
- **预期结果**：console logs 显示每条链原生 token 和自定义 token 的价格源；自定义 token 若无直接稳定币对但有 wrappedNative 对，价格应为 (token→native swap 结果) × nativePriceUsd；所有 token 的最终 priceUsd 要么是有效数字，要么是 null（无异常）。
- **边界/异常**：DEX swap 多尝试（多个 fee tiers 或 stable/volatile routes）；Chainlink 多链支持验证（Ethereum、BSC、Arbitrum 有本地 feed，Polygon 无）；自定义 token 池流动性低导致 DEX 返回 0 的降级。
- **源码参考**：`src/services/wallet-api.ts:365-416; src/services/price-service.ts:62-111`

### 56. 单链 18 秒超时降级验证

**`P1`** ｜ **分类** 余额与资产组合（多链/价格/到账） ｜ **平台** All

- **测什么**：验证若某条链的 RPC 全部故障，该链的 token 查询应在 18 秒后自动超时降级（不会卡住其他链），同时在 failedChainIds 中报告该链失败。
- **怎么测**：1) 进入 HomeScreen（活动或资产 tab），观察初始加载。2) 在浏览器控制台执行 `vela.failRpc(137)` 模拟 Polygon 全部 RPC 故障。3) 下拉刷新 (pull-to-refresh)。4) 计时：应在 18 秒内（不是 60s+），Polygon 被标记为 failed，其他链的数据继续加载；页面顶部出现 'RPC Trouble' 横幅，允许用户尝试修复。
- **预期结果**：失败链在 18 秒内被 failedChainIds 捕获，不延迟其他链加载；失败链的 token 不现身在列表（zero balance）；failedChainIds 长度 > 0 时，hero 余额顶部显示 balancePartial 状态（⚠️ 和缓存 fallback）；RpcTroubleBanner 渲染 failed chains 列表和 'Retry' 按钮。
- **边界/异常**：测试一条链超时但其他链正常；所有 12 条链都超时；部分链返回空结果（不同于 RPC 故障）。
- **源码参考**：`src/services/wallet-api.ts:191-220; src/screens/wallet/HomeScreen.tsx:304-326`

### 57. 到账实时探测与 celebration 反馈验证

**`P1`** ｜ **分类** 余额与资产组合（多链/价格/到账） ｜ **平台** All

- **测什么**：验证 HomeScreen 的 Activity 标签页中，新到账的 token 会同时触发：(1) 行高亮 glow 动画 (2) 成功 haptic (3) 顶部 toast 通知 (4) balance hero 脉冲动画。确保用户感受到真实的「钱到账」反馈。
- **怎么测**：1) 进入 HomeScreen，确保在 Activity 标签页。2) 从另一个钱包或 faucet 向该 wallet 发送一笔小额代币（需测试网）。3) 在 10 秒内（LIVE_POLL_MS），新交易应出现在列表顶部，带有高亮（setNewItemId）。4) 观察 toast 从顶部滑入，显示 '+X USDT' 绿色通知，持续 2.8s。5) 同时观察 hero balance 圆圈脉冲一次（balancePulse 动画）。6) 在 Web 上可用 console `velaSimulateReceipt(100, 'USDT')` 触发完整反馈。
- **预期结果**：到账后 ActivityRow 的 index = 0 时带 isNew={true}，使用 fadeIn 动画 + glow（balanceRing with success.base 色）；toast 从上方以弹出动画进入，显示正确的金额和币种，2.8s 后消失；hapticSuccess 触发（应感受到两次 haptic：一次成功，一次 balance 脉冲）；hero balance 有明显的 scale up/down 脉冲。
- **边界/异常**：极快连续两笔到账；到账时 Activity 标签不在前台（应保存但不 celebrate）；到账的是未定价 token；模拟界面切出再切回。
- **源码参考**：`src/screens/wallet/HomeScreen.tsx:201-216, 282-298, 651-668`

### 58. 账户切换器多账户余额聚合验证

**`P1`** ｜ **分类** 余额与资产组合（多链/价格/到账） ｜ **平台** All

- **测什么**：验证 HomeScreen 的账户切换器（AccountSwitcher modal）正确显示所有账户的缓存余额，且点击切换时实时更新活跃账户的 hero 余额。
- **怎么测**：1) 创建多个账户（Settings → Add Account，至少 2 个）。2) 进入 HomeScreen，点击顶部账户芯片（左上）打开切换器。3) 观察 modal 显示所有账户，每个账户旁显示其 cached balance（从 balance-cache 中读取）；顶部显示所有账户余额总和。4) 点击另一个账户，切换器关闭，HomeScreen 更新为新账户的数据；hero 余额瞬间展示该账户的缓存余额，然后后台刷新（若有更新则跳到新值）。5) 在切换器打开状态下，按 'Refresh All' 按钮，所有账户的余额后台更新，modal 的数字实时跳动。
- **预期结果**：AccountSwitcher 打开时立即显示所有账户的缓存余额（来自 getAccountBalances）；点击账户时 activeAccount 切换，setTokens 清空，setCachedTotal 恢复为该账户的缓存值，然后 loadData() 后台刷新；refreshAllBalances 时同步 fetchTokens(每个账户)，并更新所有的 cachedBalance map；顶部总额 = 所有账户 cached 余额之和。
- **边界/异常**：新账户无缓存（应显示加载中或 $0）；切换中途有新 tx 到账；快速切换多个账户；删除账户后切换器更新。
- **源码参考**：`src/screens/wallet/HomeScreen.tsx:401-430, 795-803; src/components/ui/HoldingsList.tsx (token rows → /token-detail)`

### 59. token 详情页 send/receive 按钮路由验证

**`P1`** ｜ **分类** 余额与资产组合（多链/价格/到账） ｜ **平台** All

- **测什么**：验证 TokenDetailScreen 的 Send/Receive 按钮正确导航到 Send 或 Receive，且 Send 时预填该 token 的 symbol 和 network。
- **怎么测**：1) 进入首页「资产」tab，点击任意 token 进入 TokenDetailScreen（如 USDC on Polygon）。2) 点击 'Send' 按钮，应导航到 /send 页面，preselectedSymbol='USDC', preselectedNetwork='polygon'；Send 页面应预选 USDC on Polygon。3) 返回 TokenDetailScreen。4) 点击 'Receive' 按钮，应导航到 /receive。5) 返回，验证 token 详情页信息仍保留（no flashing）。
- **预期结果**：Send 按钮触发 router.push(/send, {preselectedSymbol, preselectedNetwork})；Receive 按钮触发 router.push(/receive)；token 详情页未卸载（导航是 push，可返回）。
- **边界/异常**：无价格的 token；原生 token；自定义 token。
- **源码参考**：`src/screens/wallet/TokenDetailScreen.tsx:84-93`

### 60. 账户恢复-已有Passkey登录（本地）

**`P1`** ｜ **分类** 钱包创建与 Passkey（账户/恢复） ｜ **平台** All

- **测什么**：用户在创建钱包后卸载再重装应用（或清除存储），使用相同设备的已有 Passkey 重新登录。应从本地 AsyncStorage 恢复账户而无需网络。
- **怎么测**：1) 创建钱包 Alice（地址 0xABC...）并完全验证。2) 模拟应用卸载：手动删除应用存储（开发工具中清除 AsyncStorage）或在开发工具中模拟存储损坏。3) 重新打开应用，进入欢迎屏。4) 点击「我已有钱包」进行登录。5) 触发生物识别，使用相同 Passkey。6) 系统应查询本地存储找到 Alice，跳转到钱包主页。
- **预期结果**：登录成功，钱包显示 Alice 账户及其正确地址 0xABC...；无需网络即可恢复（断网状态下仍应成功）。
- **边界/异常**：如果本地存储已清空但 Passkey 仍存在（跨设备场景），则应改用服务器查询（见下条）。
- **源码参考**：`src/screens/onboarding/OnboardingScreen.tsx:78-171, src/services/storage.ts:56-59`

### 61. 账户恢复-跨设备恢复（iCloud/Google Keychain）

**`P1`** ｜ **分类** 钱包创建与 Passkey（账户/恢复） ｜ **平台** iOS / Android

- **测什么**：用户在设备 A 创建钱包，Passkey 同步到 iCloud Keychain（iOS）或 Google Password Manager（Android）。在全新设备 B 登录，Passkey 自动同步，钱包无需备份码即可恢复。
- **怎么测**：设备 A（已配置 iCloud/Google 同步）：1) 创建钱包 Bob（地址 0xDEF...）。2) 完成 Passkey 创建与上传。3) 确保 iCloud Keychain / Google Password Manager 已启用。设备 B（新/清除）：4) 安装应用，进入欢迎屏。5) 点击「我已有钱包」。6) 触发生物识别，在 Passkey 选择器中应看到 Bob 的 Passkey（通过云同步）。7) 选择并完成生物验证。8) 系统查询 Passkey Index 服务器，获得 Bob 的公钥和信息。9) 本地创建 Bob 账户，跳转钱包主页。
- **预期结果**：设备 B 上无需输入任何恢复码、备份或 seed，仅通过生物识别即可完全恢复账户及其在所有链上的地址。账户名称、公钥、地址与设备 A 一致。
- **边界/异常**：跨设备恢复依赖 Passkey Index 服务器可达；如服务器离线，设备 B 无法查询公钥（设计限制：不兼容完全离线恢复）。
- **源码参考**：`src/screens/onboarding/OnboardingScreen.tsx:126-146, src/services/public-key-index.ts:77-84`

### 62. 创建钱包-确认清单必填拦截

**`P1`** ｜ **分类** 钱包创建与 Passkey（账户/恢复） ｜ **平台** All

- **测什么**：防止用户在未勾选全部4个法律/功能确认项时创建钱包。未勾选时创建按钮应禁用，强制认知责任。
- **怎么测**：1) 进入创建钱包屏幕，输入有效账户名。2) 逐项验证：只勾选第1项，创建按钮禁用；勾选前3项，创建按钮禁用；全部勾选后，创建按钮启用。3) 勾选全部后，取消勾选其中任一项，验证按钮立即禁用。4) 重新勾选回全部4项，按钮恢复启用。
- **预期结果**：创建按钮的启用/禁用状态精确对应 checks 全部为 true 的条件；用户无法绕过任何确认项。
- **边界/异常**：法律链接（《隐私政策》《条款》）应可点击且打开正确URL（https://getvela.app/privacy 和 /terms）。
- **源码参考**：`src/screens/onboarding/CreateWalletScreen.tsx:331-359,372`

### 63. 账户公钥上传-失败与手动重试

**`P1`** ｜ **分类** 钱包创建与 Passkey（账户/恢复） ｜ **平台** All

- **测什么**：Passkey Index 服务器不可达时（网络错误、DNS 失败、超时），上传失败屏幕应出现，允许用户配置端点或重试。验证重试逻辑与故障降级。
- **怎么测**：1) Settings 中临时改为无效 URL（如 https://invalid.test）。2) 创建新钱包，完成生物识别。3) 观察上传步骤显示「同步失败」界面，包含错误详情。4) 点击「打开设置」，修正 URL 为有效地址。5) 返回创建屏幕，点击「重试上传」按钮。6) 等待 tryUpload 重试（代码中最多3次，1s 和 2s 延迟）。7) 上传成功后进入验证签名步骤。
- **预期结果**：网络失败时显示专门的失败屏幕（title="同步失败"，带错误信息）；"打开设置"链接打开 Settings 模态；重试按钮点击后显示加载状态，最终成功或再次失败；本地钱包数据不丢失。
- **边界/异常**：如果首次上传因服务器超时失败但实际已写入（写成功但响应丢失），重试时服务器的幂等性应避免重复。验证 Idempotency-Key 头部的使用。
- **源码参考**：`src/screens/onboarding/CreateWalletScreen.tsx:61-86,139-177, src/services/public-key-upload.ts:69-114`

### 64. 发送交易出现在活动列表

**`P1`** ｜ **分类** 历史与活动 / 交易详情 / 对账 ｜ **平台** All

- **测什么**：验证 Send 屏幕提交的交易在历史/活动列表中立即显示，格式正确。这是钱包基础功能，失败将导致用户无法追踪交易。
- **怎么测**：1) 创建或导入钱包（确保有测试网资金）。2) 在 Send 屏幕输入金额/地址。3) 预估 gas、签名并提交。4) 点击 History 标签进入历史屏幕。5) 观察交易是否在列表顶部，方向图标为向上箭头，金额格式为「-数额 代币符号」。
- **预期结果**：交易显示在列表最上面（新发送的排第一），显示「Sent [SYMBOL]」标签，金额为「-」开头（红/白文字），收款地址或别名显示在第二行，时间戳在下方。
- **边界/异常**：多链发送：在 Ethereum 和 Polygon 各发送一笔，切换链过滤器，验证列表只显示选中链的交易。
- **源码参考**：`src/screens/wallet/HistoryScreen.tsx:112-149; src/services/activity.ts:125-143`

### 65. 接收交易显示在活动列表（native 转账及 ERC20）

**`P1`** ｜ **分类** 历史与活动 / 交易详情 / 对账 ｜ **平台** All

- **测什么**：入站交易（原生 coin 转账或 ERC-20 转账）通过 RPC 监听被发现并持久化到本地存储，显示在历史列表中。包括 EIP-7708 原生转账。这是 P1 因为收款是基础功能。
- **怎么测**：1) 创建钱包并记录其地址。2) 从另一个地址或测试水龙头给该钱包发送一笔原生 coin（如 goerli ETH）。3) 回到 History 屏幕或 Home（Activity 列表）。4) 等待下一次同步周期（app 启动或 Home 聚焦时自动触发）。5) 观察是否出现新的「Received」行。
- **预期结果**：新交易显示在列表顶部，方向图标为向下绿色箭头（color.success.base），标签为「Received [SYMBOL]」（如「Received ETH」），金额为「+」开头的绿色数字，发送者地址或别名在第二行，状态为 confirmed。
- **边界/异常**：ERC-20 接收且元数据缺失：发送一笔未知的 ERC-20 token，如果 RPC 无法解析符号/小数位，交易应被暂时跳过（重试下次同步）；已知稳定币（USDC/USDT）应显示合理的 USD 价值。
- **源码参考**：`src/services/activity.ts:264-290; src/services/activity.ts:158-187; src/services/activity.ts:237-254`

### 66. dApp 交易及签名请求显示在历史中

**`P1`** ｜ **分类** 历史与活动 / 交易详情 / 对账 ｜ **平台** All

- **测什么**：dApp 通过 WalletPair 发起的交易（eth_sendTransaction）和签名请求（personal_sign、eth_signTypedData）被记录并显示在历史中，可在详情卡回放原始签名内容。这是核心功能。
- **怎么测**：1) Web 上访问 /clear-signing-test 路由（或使用实际 dApp 如 Uniswap）。2) 触发一笔 dApp 交易（如 swap）。3) 在签名面板签名。4) 进入 History 或 Connections 标签。5) 找到该交易，点开详情卡。
- **预期结果**：dApp 交易显示为「Swap」或「dApp Transaction」标签，带代码图标或向上箭头；签名请求显示为「Signature request」或「Typed data signature」，带文件图标；详情卡显示 Intent（如「Swap」）、操作类型、原始签名内容（calldata 或 message 文本）、dApp 来源、status。
- **边界/异常**：签名面板回放：点开已签名的 dApp 交易，应能看到原始的签名面板（read-only），包含完整的 intent、金额、recipient 等；大型 calldata 截断：若请求超过 24KB，应标记为 truncated 但仍显示前缀。
- **源码参考**：`src/services/dapp-history.ts:145-167; src/screens/wallet/HistoryScreen.tsx:156-168; src/services/activity.ts:332-348`

### 67. confirmed 交易显示成功徽章及完整哈希

**`P1`** ｜ **分类** 历史与活动 / 交易详情 / 对账 ｜ **平台** All

- **测什么**：交易确认后，历史列表的时间戳变为交易确认时间，详情卡显示绿色勾号徽章和完整的链接到区块浏览器的 txHash。验证对账逻辑正确完成。
- **怎么测**：1) 发送交易。2) 等待足够的区块确认（测试网可能秒级，主网可能分钟级）。3) 返回 Home 或 History 屏幕，观察交易状态变化。4) 点开详情卡，检查 Status 行和 Hash 行。
- **预期结果**：Status 显示绿色勾号 checkmark（color.success.base）和「Succeeded」文字；Hash 行显示缩短的 txHash，点击可打开浏览器链接；列表行显示交易确认的时间戳（不再是「failed」标签）。
- **边界/异常**：立即刷新对账：点开发送交易的待确认详情卡，快速按下拉刷新或回到 Home 再进 History，验证 pending→confirmed 转换不会导致 UI 闪烁或重排。
- **源码参考**：`src/components/ui/TxStatusBadge.tsx:18; src/services/tx-reconciler.ts:69-82; src/components/ui/TransactionDetailSheet.tsx:131-145`

### 68. 收款地址模式 - QR码生成和显示

**`P1`** ｜ **分类** 收款 / 付款请求 / 二维码 / EIP-681 ｜ **平台** All

- **测什么**：验证收款屏幕在地址模式下正确生成并显示钱包地址的QR码。QR码是收款最核心的视觉输出，若码无法生成或内容错误，用户无法被收款。
- **怎么测**：1) 创建一个钱包

2) 进入收款屏幕（应显示两个模式选项：地址 Address、请求 Request）
3) 确保地址模式已激活
4) 观察QR码区域是否显示黑白格子的二维码
5) 用手机或在线QR扫描工具扫描该码，验证解码结果是否匹配钱包的完整地址（0x开头的40位十六进制）

- **预期结果**：QR码能成功扫描，解码内容精确匹配钱包地址，不含任何前缀或格式错误。Web上选项卡切换流畅，地址/请求视觉反馈清晰。
- **边界/异常**：钱包地址为空或无效时，是否优雅降级为占位符文案（receive.noAddress）
- **源码参考**：`src/screens/wallet/ReceiveScreen.tsx:227-233, src/components/QRCode.tsx:12-52`

### 69. 发送屏幕预填 - 来自pay链接的参数注入

**`P1`** ｜ **分类** 收款 / 付款请求 / 二维码 / EIP-681 ｜ **平台** All

- **测什么**：验证用户点击/pay页面"Open in Vela Wallet"或扫描EIP-681后，发送屏幕被正确的参数预填（recipient、chainId、token、amount均locked），用户只需确认即可发送。这是支付请求的关键转换点。
- **怎么测**：1) 打开/pay链接或扫描EIP-681 URI

2) 点击"Open in Vela Wallet"按钮
3) 应导航到/send页面且带query params：prefilledRecipient、prefilledChainId、prefilledTokenAddress（若有）、prefilledAmountBase（若有）、locked=1
4) 验证收款地址字段显示预填值且禁用编辑
5) 验证网络选择器禁用或隐藏
6) 验证代币选择器禁用或隐藏（若指定了token）
7) 验证金额字段显示预填值且禁用编辑（若指定了amount）
8) 若未指定金额，金额字段应可编辑
9) 验证页面上有clear "locked mode"的视觉指示（如黄色banner或禁用图标）
10) 完成签名/提交，验证transaction确实发往预填地址

- **预期结果**：所有prefilledXxx参数正确传递；locked=1时对应字段真正禁用（不仅视觉禁用，还要阻止修改）；gas估算、签名流程正常；preview显示正确的to/token/amount。
- **边界/异常**：prefilledAmountBase是bigint格式（需fromBaseUnits转换显示）；缺少某个参数时fallback；networkId与chainId的映射；token为null vs不存在的差异
- **源码参考**：`src/screens/wallet/PayScreen.tsx:70-75, src/models/types.ts`

### 70. EIP-681 URI 解析 - 原生支付 & ERC-20 transfer

**`P1`** ｜ **分类** 收款 / 付款请求 / 二维码 / EIP-681 ｜ **平台** All

- **测什么**：验证parseEIP681函数正确解析标准EIP-681格式，区分原生币支付（ethereum:recipient@chainId）和ERC-20 transfer（ethereum:token@chainId/transfer?address=recipient&uint256=amount）。格式解析错误会导致支付失败。
- **怎么测**：可通过单元测试或手动Web端验证：

1) 构造原生支付无金额URI：ethereum:0x742d35Cc6634C0532925a3b844Bc454e4438f44e@1
2) 通过扫描或直接访问/pay?eip681=<encoded></encoded>验证解析
3) 预期：recipient=0x742d..., chainId=1, isNative=true, amountBaseUnits=undefined
4) 构造原生支付有金额：ethereum:0x742d35Cc6634C0532925a3b844Bc454e4438f44e@1?value=1000000000000000000
5) 预期：amountBaseUnits=1n*10^18
6) 构造ERC-20无金额：ethereum:0x3c499c542cEF5E3811e1192ce70d8cc03d5c3359@137/transfer?address=0x742d35Cc6634C0532925a3b844Bc454e4438f44e
7) 预期：tokenAddress=0x3c499..., recipient=0x742d..., chainId=137, isNative=false
8) 构造ERC-20有金额：上述+&uint256=5000000
9) 预期：amountBaseUnits=5000000n
10) 测试科学记数法：value=2.5e18，应正确转换为bigint
11) 测试无效输入（缺少chainId、recipient不是地址、负数金额），应返回null

- **预期结果**：所有有效格式正确解析；无效格式返回null；amountBaseUnits精确为BigInt；支持legacy pay-前缀容错；科学记数法正确转换。
- **边界/异常**：金额为0；巨大数字超过js浮点；缺少可选参数（chainId、amount）；重复的query参数；URL encode特殊字符
- **源码参考**：`src/services/eip681.ts:110-157`

### 71. 添加 ERC-20 代币 - 输入地址自动解析

**`P1`** ｜ **分类** 跨平台 / Web 差异 / 代币管理 ｜ **平台** All

- **测什么**：验证输入有效的代币合约地址后，能跨所有链正确发现并显示代币元数据（名称、符号、小数位）。这是添加代币的核心功能，失败会导致代币无法被识别。
- **怎么测**：1) 在钱包中打开「添加代币」（Add Token）屏幕；2) 在合约地址字段输入一个已知的多链部署代币地址，如 USDC（Ethereum: 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48）；3) 点击「搜索网络」(Search Networks)；4) 等待加载完成。预期：所有支持该代币的链都应在结果卡中显示（Name、Symbol、Decimals、Network），例如 Ethereum、Polygon、Arbitrum 等各显示一张卡。
- **预期结果**：代币卡片依次呈现，显示文案示例如「Name: USD Coin | Symbol: USDC | Decimals: 6 | Network: Ethereum」。每条结果包含一个可交互的「Add to Wallet」(绿色或蓝色按钮)。
- **边界/异常**：输入不是有效 0x 地址时，按钮应禁用。输入有效地址但不存在代币时，弹出提示「Token not found on any network」。
- **源码参考**：`src/screens/wallet/AddTokenScreen.tsx; src/components/ui/AddTokenPanel.tsx:159-185`

### 72. Bug 上报后台成功提交

**`P1`** ｜ **分类** 韧性 / 故障注入 / 错误态 / Bug 上报 ｜ **平台** All

- **测什么**：验证 bug 上报通过后台 getvela.app/api/bug-report endpoint 成功提交时，显示 GitHub issue URL 和号码（不丢失用户输入），且用户可点击查看 issue。
- **怎么测**：1) Settings > Feedback 打开对话框。2) 输入描述（如"Test report from wallet"）和可选步骤。3) 点击 "Send report"。4) 等待 submitting 状态完成。5) 观察弹窗：如果成功（后台可达），应显示 "Report sent!" 和 issue 号（如 #42）以及 "View issue" 按钮。6) 点击按钮验证是否跳转到 GitHub issue。7) 如果多次提交相同描述，验证是否 dedup 到同一 issue（预期 API 返回 deduped=true）。
- **预期结果**：弹窗显示成功状态，包含 GitHub issue URL（https://github.com/mondaylabsltd/vela-wallet/issues/NN）和号码；用户输入的描述仍保留在模态框内（如果用户想重新提交）。点击 "View issue" 可在浏览器打开链接。
- **边界/异常**：测试网络超时时的降级（见后续用例）；尝试提交空描述（"   "），验证 "Send" 按钮保持禁用，不会调用 API。
- **源码参考**：`src/services/bug-report.ts:110-152, src/components/ui/BugReportModal.tsx:42-49,52-68`

### 73. 账户切换或链切换后 dApp 收到 accountsChanged/chainChanged 事件

**`P1`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：用户在已连接状态下切换账户或链，钱包应推送事件通知 dApp(EIP-1193)，dApp 界面刷新。验证事件内容准确(accounts 数组、chainId hex 格式)。
- **怎么测**：1) 连接 dApp，dApp 显示账户 0x1111；2) 钱包 Home 页切换到另一个账户(或在 Settings 里新增账户)；3) dApp 页面应自动更新，显示新账户 0x2222；4) 或在钱包 Connect 页的链选择器切换链(e.g. Polygon→Ethereum)；5) dApp 应收到 chainChanged 事件，重新加载。
- **预期结果**：accountsChanged event 包含 {accounts: [{address: 0x2222, chains: [0x1, 0x89, ...]}]} 格式；chainChanged event 包含 {chain: 0x1}(hex)；dApp 的 window.ethereum.on('accountsChanged', ...) 回调应被触发；eth_accounts 查询应返回新账户。
- **边界/异常**：新增多个账户应在 accounts 数组中都出现；删除当前账户后应推送空数组；未连接状态下切换账户不应推送。
- **源码参考**：`src/services/walletpair-transport.ts:448-463, src/models/dapp-connection.tsx:236-245, 681-693`

### 74. 用户点击"断开连接"后 dApp 无法再发送请求，连接面板清空

**`P1`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：disconnectBridge 关闭 WebSocket、清理会话存储、置 status=disconnected。确保完全断开、无遗留 timer 或事件监听，避免内存泄漏。dApp 再次发请求应得到连接失败或拒绝。
- **怎么测**：1) 已连接状态下，Connect 页面的已连接卡片显示"断开连接"按钮；2) 点击"断开连接"；3) 卡片切换回"指南"和"扫描"界面；4) dApp 尝试发起请求(如 personal_sign)，应得到错误或超时；5) 立即再扫新 QR，应正常连接(无旧会话残留)。
- **预期结果**：点击"断开"后 status 变为 disconnected, connectionType=null, dappInfo=null；AsyncStorage 的 vela.walletpairSession 被清除；dApp 再发请求应得到 4001(User rejected) 或 -32603(RPC error)；下次连接使用新 URI。
- **边界/异常**：断开时有签名弹框打开应同时关闭；pending 交易应标记失败或保留供用户查看；重连前旧 listener 应全部移除。
- **源码参考**：`src/models/dapp-connection.tsx:464-473, src/services/walletpair-transport.ts:413-422`

### 75. USD 价值计算及稳定币推断

**`P1`** ｜ **分类** 历史与活动 / 交易详情 / 对账 ｜ **平台** All

- **测什么**：交易的 USD 价值在签名/提交时记录；若价格缺失但 token 是已知稳定币（USDC/USDT/DAI 等），USD ≈ 代币金额；未知 token 显示 "$0.00"。这防止稳定币转账显示为"$0.00"。
- **怎么测**：1) 发送或接收 USDC。2) 进详情卡检查 USD 值。3) 发送或接收未知 token。4) 检查 USD 显示。
- **预期结果**：USDC/USDT/DAI 等稳定币转账显示对应的 USD 值（如 100 USDC ≈ $100）；已知 token 但无价格显示 "$0.00"；detail sheet 底部「≈ $X.XX」显示格式。
- **边界/异常**：tether symbol 特殊处理："USD₮" 应被识别为稳定币（与 "USDT" 等同）；off-chain 签名（无转账金额）显示 "$0.00"。
- **源码参考**：`src/services/activity.ts:91-123; src/services/activity.ts:158-172; src/components/ui/TransactionDetailSheet.tsx:71-72`

### 76. 支付请求模式 - EIP-681 生成和编辑

**`P2`** ｜ **分类** 收款 / 付款请求 / 二维码 / EIP-681 ｜ **平台** All

- **测什么**：验证用户在请求模式可以选择代币、输入金额，系统生成有效的EIP-681 URI且能编码到QR码中。这是链接不同钱包的标准支付协议。
- **怎么测**：1) 进入收款屏幕 > 点击"Request"选项卡

2) 看到资产选择器和金额输入框
3) 点击资产选择器（默认应为ETH on Ethereum）
4) 从列表选择USDC（或其他ERC-20代币）
5) 在金额输入框输入"10.5"，观察金额验证（不应允许多个小数点、应限制小数位数）
6) 观察上方QR码和"Request Summary"文案是否更新
7) 扫描QR码，验证内容符合EIP-681格式：ethereum:<token></token>@<chainId></chainid>/transfer?address=<recipient></recipient>&uint256=<baseUnits></baseunits>

- **预期结果**：资产切换时金额精度正确重新限制；QR码实时更新；生成的EIP-681 URI有效且参数准确；USD价格换算若显示应正确对应当前价格。
- **边界/异常**：输入"0"或空金额时，QR码应编码无金额的开放请求（无uint256参数）；超大金额（>1e20 wei）；自定义链上的非标代币
- **源码参考**：`src/components/ReceiveRequestControls.tsx:44-89, src/services/eip681.ts:89-103`

### 77. 金额输入验证和精度限制

**`P2`** ｜ **分类** 收款 / 付款请求 / 二维码 / EIP-681 ｜ **平台** All

- **测什么**：验证金额输入框防止输入错误格式（多个小数点、非数字字符）且在切换代币时动态限制小数位数以符合该代币的decimals。金额格式错误会导致EIP-681无效。
- **怎么测**：1) 在请求模式下，点击金额输入框

2) 尝试输入"10.5.2"（两个小数点），观察第二个小数点是否被拒绝
3) 尝试输入"abc"或特殊符号，观察是否过滤
4) 输入"10.123456789"（多位小数）
5) 切换代币到USDC（6位decimals），观察输入是否自动截断至6位
6) 切换回ETH（18位decimals），验证小数位限制是否放宽
7) 输入"0"，再输入有效金额，验证可清空且重新输入

- **预期结果**：多小数点被拒；非数字字符过滤；切换代币后精度重新限制；sanitizeAmount函数正确处理所有边界；UI反馈流畅无卡顿。
- **边界/异常**：粘贴包含多个小数点的文本；输入非常大的数字（超过js浮点精度）；输入负数前缀
- **源码参考**：`src/components/ReceiveRequestControls.tsx:36-42, 108-116`

### 78. 不支持的网络和未知代币错误处理

**`P2`** ｜ **分类** 收款 / 付款请求 / 二维码 / EIP-681 ｜ **平台** All

- **测什么**：验证当pay链接或EIP-681包含钱包不支持的chainId或无法找到的token contract时，系统显示友好错误并给出处理选项（如添加网络）。若无此保护，用户会陷入卡死的发送流程。
- **怎么测**：前置条件：有现成的不支持链ID（如99999）和死地址（如0x000...dEaD）

1) 构造pay链接指向unsupported chain：/pay?to=0x742d...&chain=99999&sym=TEST&dec=18&net=Unknown
2) 打开链接，验证是否显示错误（"Network not supported"）
3) 若有"Add this network"按钮，点击并验证跳转到network addition flow
4) 构造/send路由参数指向不存在的token：prefilledTokenAddress=0x000...dEaD
5) 打开发送屏幕，验证是否显示"Unknown token"错误
6) 验证error state下的UI反馈：clear message、建议操作或返回按钮
7) 测试余额不足场景：prefilledAmountBase超过当前余额
8) 验证显示"You do not have enough XXX"并阻止提交

- **预期结果**：unsupported chainId显示清晰错误且建议添加网络；unknown token显示错误；insufficient balance显示红色warning；所有error state都有返回/关闭操作；不允许盲目提交失败的tx。
- **边界/异常**：网络添加失败或用户拒绝；被标记为spam的token；极端金额（>max supply）；十进制溢出的amountBase
- **源码参考**：`e2e/eip681-pay.spec.ts:71-88`

### 79. 入账实时检测和反馈

**`P2`** ｜ **分类** 收款 / 付款请求 / 二维码 / EIP-681 ｜ **平台** All

- **测什么**：验证用户在收款屏幕打开时，系统后台定期轮询余额，检测到新到账时实时显示绿色成功提示框（deposit notification）。这提升用户确认收款的安心感。
- **怎么测**：前置条件：钱包已配置、有连接到测试网RPC

1) 进入收款屏幕
2) 注意观察屏幕是否有入账检测逻辑：应有"Successfully received"类的绿色框（若有入账）
3) 在另一钱包或合约向当前钱包转账任意ERC20 token
4) 等待3-5秒（系统应在FAST_INTERVAL_MS=3000内检测一次）
5) 观察QR卡片下方是否出现新的绿框，显示：入账时间戳、代币符号、金额、网络名称、USD价值（若可用）
6) 多笔入账到达，应列为多个entry并按时间排序
7) 保持屏幕打开5分钟，验证轮询是否在TOTAL_LISTEN_MS=5分钟后停止

- **预期结果**：首次轮询3秒内检测，之后在FAST_PHASE_MS内保持3秒间隔，TOTAL_LISTEN_MS后切换60秒间隔直到5分钟停止；入账显示准确；USD换算若显示应正确；绿色提示框动画入场流畅。
- **边界/异常**：离开屏幕（isAppActive false）时应暂停检测；网络故障时graceful处理；极短时间多笔到账的排序；已持有的token新增金额vs全新token首次接收；零余额token的检测
- **源码参考**：`src/screens/wallet/ReceiveScreen.tsx:58-119`

### 80. QR码扫描 - 相机和相册上传（真机）

**`P2`** ｜ **分类** 收款 / 付款请求 / 二维码 / EIP-681 ｜ **平台** iOS / Android

- **测什么**：验证iOS/Android上的摄像头扫描和图库上传能解码QR码。在真实场景中，用户常通过扫描纸质地址或发来的截图来支付。
- **怎么测**：前置条件：已创建钱包；拥有真实iOS或Android设备

1) 在Vela发起扫描（需要额外功能如"扫描支付"或内置扫描器），或通过dApp连接触发扫描
2) 相机权限提示出现，点击"允许"
3) 相机视图打开，显示扫描框和角标
4) 将相机对准任意有效EIP-681或地址QR码
5) 系统应在2秒内识别并解析
6) 点击"图片"按钮打开相册，选择包含QR码的截图
7) 系统解码，应自动返回扫描结果

- **预期结果**：相机实时扫描无延迟；QR解码成功率>95%；图库上传支持JPEG和PNG；解码后自动关闭扫描器且传递正确的URI/地址到调用方。
- **边界/异常**：低光线条件下扫描；QR码被部分遮挡；极小或极大的QR码；损坏或打印质量差的码；反向相机（front-facing）扫描；拒绝相机权限
- **源码参考**：`src/components/QRScanner.tsx:217-249, 286-345`

### 81. 合约部署（CREATE2）的平静展示

**`P2`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：当交易是合约部署（to=null 或 CREATE2 factory）时，不显示红色「Unknown」警告，而是平静的「Deploy contract」intent + 预测地址。
- **怎么测**：1) 构造一个 CREATE2 deploy 请求（to=0x4e59b44847b379578588920ca78fbf26c0b4956c，data=salt+initCode）；2) 观察 Intent=「Deploy contract」，风险=normal；3) Fields 显示「New contract: 0x1234...5678」；4) 无红色警告，无 blind-sign 标记；5) 估算 gas 时将其视为 transfer（no contract call 模拟）。
- **预期结果**：1) Intent 黑色、neutral 字体；2) 显示 predicted address（如可计算）；3) 风险=normal，不是 caution/danger；4) Fields 最小化，仅显示新合约地址；5) UI 传达「this is safe, it's just creating a new contract」。
- **边界/异常**：unknown deployer（非 0x4e59...）的 CREATE2 应降级为 blind-sign；raw create（to=null）应判定为 deploy；create3-style wrapped deploy 可能识别为 unknown。
- **源码参考**：`src/services/clear-signing.ts:118-160（buildDeployResult）`

### 82. ERC-165 代币类型自动检测（ERC-20 vs ERC-721）

**`P2`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：approve/transferFrom 选择器在 ERC-20 与 ERC-721 间共享。钱包通过 ERC-165 supportsInterface 在链上检测代币类型，确保金额显示的是「amount」还是「token ID」。
- **怎么测**：1) 打开「NFT Transfer」场景（BAYC transferFrom）；2) 观察第三个参数（token ID = 0x1981 = 6529）显示为「#6529」（NFT format）而非「25481 tokens」；3) 打开「ERC-20 TransferFrom」（USDT）；4) 观察金额显示为「100 USDT」而非「tokenId #100」；5) 检查背后的 RPC call：supportsInterface(0x80ac58cd) for ERC-721。
- **预期结果**：1) ERC-721 detected：tokenId 显示为「#XXXX」，无小数点；2) ERC-20 detected：amount 显示为「XXXX.YYYY TOKEN」；3) ERC-1155 detected：showsAmount + id 分离；4) 检测失败/超时时降级为 ERC-20（金额显示）；5) 缓存的检测结果在会话内复用，避免重复 RPC。
- **边界/异常**：同一合约 0x0 address（burn token）应作为 ERC-20；双协议实现（同时是 ERC-20 和 ERC-721）的代币；RPC unreachable 时的缓存策略（prefer cached over guessing）。
- **源码参考**：`src/services/clear-signing.ts:186-235（callSupportsInterface, detectTokenStandard），src/services/clear-signing.ts:263-283（resolveTokenStandard）`

### 83. 清晰签名风险配色准确映射

**`P2`** ｜ **分类** 清晰签名与授权安全（approval/模拟/SIWE） ｜ **平台** All

- **测什么**：四个风险等级（safe/normal/caution/danger）必须映射到准确的颜色，帮助用户快速识别交易风险。
- **怎么测**：1) 在 clear-signing-test 中打开多个场景，逐一验证风险色：a) safe（绿）：declare contract（Safe audit）、claim reward；b) normal（黑/中立）：send、ETH transfer；c) caution（琥珀）：approve（有限值）、unverified decimals、过期时间戳；d) danger（红）：unlimited approval、blind-sign、phishing SIWE。
- **预期结果**：1) safe intent=绿色粗体；2) normal intent=黑色；3) caution intent=琥珀色，field warning 也琥珀；4) danger intent=红色粗体，field warning=红色；5) 整个卡片背景色与文案配合，不出现色调不匹配。
- **边界/异常**：某个字段 unverified=true 但 intent 是 safe/normal，应将整体风险提升至 caution；已过期的 deadline 字段应同时显示「(expired)」文案 + 琥珀警告。
- **源码参考**：`src/services/clear-signing.ts:1236-1257（assessRisk），src/components/SigningRequestModal.tsx:70-89（riskColors）`

### 84. 60 秒内自动重连未成功时，UI 显示"立即重连"手动恢复按钮

**`P2`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：如果 relay 长期离线，不应让 UI 永久卡在"重新连接中"。60 秒(RECONNECT_MAX_MS)后发出 error 事件，改为"连接困难"状态并提示用户点击"立即重连"或"取消"。
- **怎么测**：1) 已连接状态；2) 模拟 relay 长期离线(故障注入或 unplugged relay)；3) WebSocket 断开；4) UI 短暂显示"重新连接中"(黄色)；5) 等待 60+ 秒；6) 应显示"连接困难"或类似错误卡片，提示"网络已断开，检查连接或手动重连"；7) 点击"立即重连"应再次尝试。
- **预期结果**：reconnectStuck=true 后，Connect 页面显示错误提示而非无限"重新连接中"spinner；按钮列表包含"立即重连"(accent)和"取消"(secondary)；tap "立即重连"应重新开始 confirmJoin/reconnect 流程。
- **边界/异常**：如果 60 秒内恢复(reconnect 成功)应立即清除 stuck 状态并恢复"已连接"；manual "立即重连" tap 应重置 timer。
- **源码参考**：`src/services/walletpair-transport.ts:535-546, src/models/dapp-connection.tsx:212-217, 476-484`

### 85. 切后台再回前台时 WalletPair 自动重连而无需手动操作

**`P2`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** iOS, Android

- **测什么**：移动端 React Native 在后台暂停 JS 计时器，WebSocket 在 ~30 秒被 relay 关闭。回到前台时(AppState='active')应自动强制重连(reconnect())而不等待 SDK 的指数退避，确保用户无感知。
- **怎么测**：1) 钱包已连接 dApp；2) 按 Home 键将钱包放入后台(5-30 秒)；3) 点击钱包图标回到前台；4) 观察连接状态卡片(应短暂闪过黄色"重新连接中"或直接恢复绿色)；5) 立即尝试签名请求，应成功。
- **预期结果**：回到前台后 <1 秒内恢复连接(无需用户点击"立即重连")。如果后台时间 >=20s(STALE_AFTER_MS)，应直接 reconnect()；<20s 应 ping()。连接状态卡片短暂显示黄色点和"重新连接中"后恢复绿色。
- **边界/异常**：后台 1 秒内回前台应不中断现有连接；后台期间网络断开应在回前台时重连；多次快速 blur/focus 应去重(不连续 reconnect)。
- **源码参考**：`src/services/walletpair-transport.ts:563-593, src/models/dapp-connection.tsx:337-348`

### 86. 同一时刻只允许一个 dApp 会话，新连接自动断开旧连接

**`P2`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：多个 dApp 同时尝试连接时，新 URI 应自动 disconnectCurrent()，断开旧会话再建立新会话。防止多头连接的状态冲突。
- **怎么测**：1) 已连接到 dApp-A；2) 扫描 dApp-B 的 QR 码或粘贴 URI；3) 钱包应自动断开 dApp-A，开始连接 dApp-B；4) 观察 Connect 页面从"已连接 dApp-A"切换为"连接中..."再到"已连接 dApp-B"；5) dApp-A 应收到 disconnect 事件或连接错误。
- **预期结果**：新连接前调用 disconnectCurrent()，清理旧 transport、停止 heartbeat、清除 timer；新会话成功后 dappInfo 和 status 都对应新 dApp；旧 dApp 的任何签名请求应失败(无 transport)。
- **边界/异常**：新连接失败时旧连接也已断开(不回滚)；两个连接同时触发应去重。
- **源码参考**：`src/models/dapp-connection.tsx:360-368, 396-413`

### 87. 链上无法检索 Safe 合约代码时，dApp eth_getCode 返回 runtime bytecode 伪装 EOA 为智能钱包

**`P2`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：未部署的 Safe 账户调用 eth_getCode 应返回 Safe proxy runtime bytecode(非空)，让 dApp 检测到 EIP-1271 合约签名能力。dApp 才能请求 EIP-1271 风格的签名而非 ECDSA。
- **怎么测**：1) 新创建钱包，Safe 未在链上部署(如新增 Scroll 后)；2) dApp 调用 eth_getCode(walletAddress)；3) 钱包应返回 Safe proxy runtime code("0x608060..." 开头)，非"0x"；4) dApp 识别为合约并请求签名；5) 用户签名后 Safe 自动在 bundler 交易中部署。
- **预期结果**：eth_getCode(wallet_addr) 返回 SAFE_PROXY_RUNTIME_CODE(非空、非"0x")；缓存在 deployedSelfCode Map 中以避免重复查询；已部署的钱包返回真实链上代码。
- **边界/异常**：其他地址的 eth_getCode 应正常转发到 RPC；查询出错应 fallback 到 runtime code；同一地址/链多次查询应使用缓存。
- **源码参考**：`src/hooks/use-dapp-signing.ts:492-508, src/services/safe-address.ts(SAFE_PROXY_RUNTIME_CODE)`

### 88. 连接历史中的已确认交易点击后显示只读重放签名界面

**`P2`** ｜ **分类** dApp 连接（WalletPair/批量/历史回放） ｜ **平台** All

- **测什么**：SigningReplaySheet 从本地存储重新渲染原始签名面板(readOnly=true)，展示用户当时看到的内容(intent、资产变化、参数)但无 approve/reject 按钮。用于审计"我当时批准了什么"。
- **怎么测**：1) 在 Home 连接面板找到已确认的交易记录(状态="已确认")；2) 点击该记录；3) 应弹出模态框，完整重现签名时的布局：操作 intent、gas 卡片、"签名消息"内容等；4) 没有"批准""拒绝"按钮，只有"关闭"或"返回"；5) 关闭后回到面板。
- **预期结果**：模态显示原始请求的 method、params(truncated 若过大)、assetChanges 预览(资产变化)、gas 估算(重放时不刷新，显示当时值)；标题为该交易的操作类型；readOnly badge 应提示"已确认"状态；无 passkey 弹框。
- **边界/异常**：记录无 signedRequest 的旧数据应 fallback 到 ConnectionEventDetailSheet(仅显示摘要)；pending 交易重放应显示"等待确认" banner；assetChanges 丢失时应显示占位符。
- **源码参考**：`src/components/ui/SigningReplaySheet.tsx, src/services/dapp-history.ts:47-73`

### 89. Tempo Gas 模型：pathUSD 支付与费用显示

**`P2`** ｜ **分类** Gas / Bundler / 费用 / Tempo / 交易提交 ｜ **平台** All

- **测什么**：Tempo 无原生币，只能用 pathUSD（TIP-20 stablecoin）支付 gas。发送任意资产时自动以 pathUSD 结算费用，显示为「≈ $X」而非原生币。
- **怎么测**：1) 配置 app 连接到 Tempo 网络（chainId 4217 或 42431）。2) 创建钱包并获得少量 pathUSD (6 decimals)。3) 进入 Send，选任意代币（native / ERC-20 / 其他）。4) 输金额，观察费用估算：GasFeeCard 应显「~0.0012 pathUSD」（非 native coin）。5) 确认发送。6) 交易提交时，bundler 应以 0x76 在 Tempo 内支付 gas，UserOp 中 maxFeePerGas=0。7) 观察 tx 成功，钱包 pathUSD 余额减少（支付了费用）。
- **预期结果**：Fee estimate 显「~0.0012 pathUSD」（GasFeeCard title = pathUSD symbol）。Bundler 回复 OK，userOpHash 返回。没有 AA21「didn't pay prefund」错（因为 maxFeePerGas=0）。pathUSD balance 递减。
- **边界/异常**：pathUSD 不足：bundler 拒绝提交（reimbursement > Safe.pathUSD.balance）。gas account info 查询（bundler REST）应返回 pathUSD balance（从 TEMPO_DEFAULT_FEE_TOKEN 查询并 scale 到 18 dec），而非 eth_getBalance sentinel。
- **源码参考**：`src/services/safe-transaction.ts:626, src/services/tempo.ts:33`

### 90. Tempo 链的特殊 gasModel 与 bundler 集成

**`P2`** ｜ **分类** 网络与 RPC（自定义链/自托管端点/容灾） ｜ **平台** All

- **测什么**：验证 Tempo 链（4217）因无原生 gas coin 而使用 gasModel='tempo'，RPC 和 bundler 路由无差异，但在发送交易时流程会走 services/tempo.ts 的特殊处理（gas 以 USD stablecoin 结算）；Tempo 的 bundler URL 和端点管理与其他链相同。
- **怎么测**：1) 进入设置或链配置，验证 Tempo 链的信息：a) nativeSymbol 为 'USD'；b) gasModel 字段为 'tempo'（如果暴露）

2) 在 Tempo 上执行一笔发送，观察 gas 估算和扣款是否涉及 stablecoin（不是 native coin）
3) 验证 Tempo 的 RPC 端点正常工作（pool 初始化、failover 等）
4) 验证 Tempo 的 bundler URL 被正确设置（vela-relay.getvela.app/4217）

- **预期结果**：Tempo 链显示，nativeSymbol 为 USD；发送交易走 tempo.ts 流程（可查阅代码或日志）；RPC/bundler 端点管理与其他链无差异；不因特殊 gasModel 而故障
- **边界/异常**：Tempo RPC 返回 200 但链不可用 → 应被识别（合约检查时缺失）；Tempo bundler 故障 → 应 fallback 到内置而非以 native gas 重试
- **源码参考**：`src/models/chains.ts:98-106; src/services/rpc-pool.ts:201-221 (initPool applies to all chains uniformly)`

### 91. 第三方 RPC Provider 密钥集成（Alchemy/dRPC/Ankr）

**`P2`** ｜ **分类** 网络与 RPC（自定义链/自托管端点/容灾） ｜ **平台** All

- **测什么**：验证用户配置的单个全局 API 密钥（Alchemy/dRPC/Ankr）能自动为所有该 provider 支持的链生成 RPC URL，钱包在池中以 provider tier 处理，被优先级排序；无效 key 被禁；能指引用户获取密钥。
- **怎么测**：1) 进入「Settings → Advanced → RPC Providers」

2) 对每个 provider（Alchemy、dRPC、Ankr）输入一个有效的 API key（可从各服务免费获取），保存
3) 观察「Test」按钮自动触发，逐一 ping 该 provider 支持的所有链的 eth_chainId，显示延迟和 ✓/✗ 状态
4) 验证兼容链数量准确（Alchemy 12 条、dRPC 12 条、Ankr 8 条，Unichain/World/Monad/Tempo 无 Ankr 支持）
5) 输入一个无效的 key（如故意删减字符），保存，观察测试重新运行、所有链返回 ✗，显示「API key invalid」或类似
6) 清除 key，验证提示重新变为「No key」，该 tier 不再参与评分
7) 配置有效 key 后，进行任何需要 RPC 的操作，观察日志 [RPC] 包含 `provider:` 标记且排序高于 `default:` 和 `public:`

- **预期结果**：输入和保存 key 后 < 5 秒完成测试；支持链列表准确（数量、名称）；invalid key 立即被识别并禁（不浪费后续请求）；provider RPC 在日志中排优先级；能点击链接获取 key（外部浏览器打开对应 provider 的仪表板）
- **边界/异常**：key 中含特殊字符或空格 → 应 trim 后处理；中途更改 key 时前一个 key 的测试结果应清除；没有网络时输入 key → 保存应成功，测试延迟到网络恢复
- **源码参考**：`src/screens/settings/RpcProvidersModal.tsx; src/services/rpc-providers.ts:74-137; src/services/rpc-pool.ts:286-293`

### 92. RPC 端点临时封禁与永久封禁恢复机制

**`P2`** ｜ **分类** 网络与 RPC（自定义链/自托管端点/容灾） ｜ **平台** All

- **测什么**：验证 RPC 端点的两层封禁系统：1) 临时禁（1 小时，用于 rate-limit/401/403），冷却后自动解禁；2) 永久禁（0 成功且 ≥6 次失败），24 小时后过期解禁以允许恢复；通过故障注入验证行为。
- **怎么测**：1) 在 Web 上通过浏览器 console 的 fault-injection 注入 vela.banEndpoint(url) 或类似（若 API 存在），或者让一个特定 URL 的 RPC 返回 HTTP 401 多次

2) 观察日志显示 「BANNED: unauthorized」和该端点在 SOURCE_PRIORITY 中的降分（消失在候选列表）
3) 等待 1 小时或模拟时间流逝，观察该端点是否重新进入候选（临时禁解除）
4) 让同一端点故意失败 6+ 次无任何成功，观察是否被标记为永久禁、日志显示「PERMA-BANNED: 0 success in N attempts」
5) 关闭再打开应用，验证永久禁列表从本地存储恢复
6) 等待 24 小时或刷新缓存，观察永久禁是否自动解除

- **预期结果**：临时禁的端点在 1 小时冷却后、或应用重启后自动重试；永久禁的端点被持久化存储，跨应用重启保留；24 小时后永久禁自动清除；日志清晰标出禁类型和恢复条件；不会因单次超时而永久禁
- **边界/异常**：同一 URL 被多个并发请求同时失败 → 只禁一次（不重复）；URL 列表中有重复 → 应被去重，不多次禁；清除缓存后，禁列表应清空（不污染新会话）
- **源码参考**：`src/services/rpc-pool.ts:68-116 (ban system); src/services/rpc-pool.ts:147-154 (maybePermaBan)`

### 93. RPC 故障 Banner 与修复流程

**`P2`** ｜ **分类** 韧性 / 故障注入 / 错误态 / Bug 上报 ｜ **平台** All

- **测什么**：验证当一条或多条链的 RPC 全部失败时，Home/Assets 屏幕显示黄色警告 banner，用户可点击链名打开修复对话框，输入新 RPC URL 后验证并保存，之后 banner 消失。
- **怎么测**：1) 打开钱包 Home 屏幕。2) 注入故障：vela.failRpc(137)（Polygon）。3) 点击 Refresh 或等待自动检测，观察顶部是否出现 "RPC unavailable" 黄色 banner。4) Banner 应列出故障链，如 Polygon 卡片带 "Fix" 链接。5) 点击 Polygon "Fix"。6) 在弹窗里粘贴一个有效的 Polygon RPC URL（如 https://polygon-rpc.com）。7) 点击 "Save RPC"，等待验证（应验证 chainId 匹配和可达性）。8) 成功后 banner 应消失，钱包恢复正常。9) 输入 vela.clear() 清除故障。
- **预期结果**：故障时 banner 可见、清晰标识失败链、点击 Fix 打开对话框。验证成功后 banner 消失，后续 Home 刷新正常工作。如果输入错误 URL（如错误的 chainId），应显示"Chain ID mismatch"错误提示，不保存。
- **边界/异常**：同时故障多个链（failRpc('all')），验证 banner 显示"Multiple networks unavailable (12)"和所有链的卡片；尝试输入 websocket RPC URL（wss://），验证是否支持或提示不支持。
- **源码参考**：`src/components/ui/RpcTroubleBanner.tsx:34-100, src/services/rpc-pool.ts:getFailedRpcChains`

### 94. vela.failRpc 单链注入

**`P2`** ｜ **分类** 韧性 / 故障注入 / 错误态 / Bug 上报 ｜ **平台** Web

- **测什么**：验证故障注入控制台 vela.failRpc(137) 能强制 Polygon RPC 失败，确保发送/查询路径检测到失败状态并正确降级（不是悄悄上链）。
- **怎么测**：1) Web 上打开钱包，进入 Home 屏幕。2) 打开浏览器控制台（F12），输入 vela.failRpc(137) 并回车。3) 验证 vela.status() 显示 'RPC: failing on Polygon (137)'。4) 尝试在 Polygon 上发送一笔交易（Send 屏幕选 Polygon）。5) 观察是否在 gas 估算或提交阶段失败，并显示错误提示（不是假报告成功）。6) 输入 vela.clear() 清除故障。
- **预期结果**：gas 估算失败并显示"RPC unavailable"或类似网络错误提示；sendUserOp 不会被提交到链上。如果有 RPC banner，Polygon 会显示在故障链列表中。
- **边界/异常**：尝试切换到其他链（Ethereum）并验证它们仍能正常工作；然后再次 failRpc(137) 后尝试 failRpc('all') 验证全局失败。
- **源码参考**：`src/services/dev/fault-injection.ts:60-65,134, src/services/rpc-pool.ts:18`

### 95. Bug 上报后台不可用降级

**`P2`** ｜ **分类** 韧性 / 故障注入 / 错误态 / Bug 上报 ｜ **平台** Web

- **测什么**：验证后台无法访问（503/timeout/网络错误）时，对话框自动降级为预填 GitHub issue URL 的浏览器链接，用户可直接跳转，不会丢失输入数据。
- **怎么测**：1) Web 上打开 DevTools 网络面板，拦截 getvela.app/api/bug-report 的请求（设置为 offline 或返回 503）。2) Settings > Feedback 打开对话框。3) 输入描述和步骤。4) 点击 "Send report"。5) 等待提交，应显示"Sorry, backend unavailable"或类似，并弹出 "Open GitHub" 按钮（fallbackUrl）。6) 原输入应保留在弹窗内（未被清除）。7) 点击 "Open GitHub" 跳转到预填表单（environment 字段预填）。8) 在 GitHub issue 表单里验证描述和步骤已预填。
- **预期结果**：提交失败后显示fallback对话框，包含"Open GitHub"按钮；点击后在新标签页打开 GitHub issue 模板，environment 字段显示 app 版本、平台、失败链等；用户仍可在原对话框看到他输入的文本（通过 Cancel 返回编辑）。
- **边界/异常**：测试网络不通（navigator.onLine=false）的行为；设置 RPC failRpc('all') 后提交 bug 上报，验证 environment 块包含所有失败链信息。
- **源码参考**：`src/services/bug-report.ts:110-152, src/components/ui/BugReportModal.tsx:70-82`

### 96. 一键 Bug 上报 - 预览内容脱敏

**`P2`** ｜ **分类** 韧性 / 故障注入 / 错误态 / Bug 上报 ｜ **平台** All

- **测什么**：验证设置里 Feedback 按钮打开 Bug 上报对话框，预览显示正确的脱敏诊断信息（app 版本、平台、失败链列表），从不泄露私钥、地址、签名或 API 密钥。
- **怎么测**：1) iOS/Android/Web 上打开 Settings 屏幕（齿轮图标）。2) 向下滚动找到 "Feedback" 行（MessageSquare 图标）。3) 点击 Feedback。4) 在对话框里输入测试描述（如"screen froze"）。5) 点击 "Show preview" 或向下看预览区域。6) 验证预览包含：app 版本号、平台名、语言、RPC 失败链列表（如果有）。7) 验证预览 NOT 包含：钱包地址、私钥（不应含 0x 开头的长十六进制）、任何 http 查询参数（api key 应脱敏为 ***）。
- **预期结果**：预览文本显示格式化的诊断块："App version: X.X.X", "Platform: iOS 18.0", "Language: 中文", "RPC unreachable: Polygon (137)", "Diagnostics:" 和 metrics 摘要。长十六进制或 token 应显示为 "0x…"。
- **边界/异常**：注入一个 RPC 失败（vela.failRpc(56)）后再打开 preview，验证失败链列表包含 BNB；记录 100+ 字节的测试描述，验证不被截断或显示不完整。
- **源码参考**：`src/services/bug-report.ts:92-103,52-61, src/components/ui/BugReportModal.tsx:111-121`

### 97. 字体加载 3 秒超时兜底

**`P2`** ｜ **分类** 韧性 / 故障注入 / 错误态 / Bug 上报 ｜ **平台** iOS / Android

- **测什么**：验证应用启动时加载 Plus Jakarta Sans 字体，如果 3 秒内未完成则使用系统字体兜底，不会因字体加载慢而阻塞启动（白屏）。
- **怎么测**：1) 在真机或模拟器上，使用 Charles/Mitmproxy 限制 CDN 带宽（如 fonts.gstatic.com 限速到 1KB/s）或拦截字体请求延迟 5 秒。2) 杀掉应用完整重启（冷启动）。3) 观察：启动画面应在 3 秒内消失，Home 屏幕应展示（使用系统字体或降级字体）。4) 等待完整加载，观察字体是否最终正确应用（Plus Jakarta Sans）。5) 验证 UI 文字清晰可读，无乱码或特别扭曲。6) 在 Settings 查看应用版本，验证版本号、字体样式都正确。
- **预期结果**：启动不阻塞（不超过 3 秒的主屏幕加载时间）；如果字体延迟，初始显示系统字体但应用可用；最终字体加载完成后样式更新；无乱码或渲染错误。
- **边界/异常**：测试在低速网络（2G/3G 模拟）或离线状态启动，验证 3 秒兜底是否生效；观察多个屏幕（Settings、Assets）的字体一致性。
- **源码参考**：`src/app/_layout.tsx (useFonts hook), src/app/_layout.tsx:75-100 (font fallback logic)`

### 98. 应用从后台回到前台时自动刷新余额

**`P2`** ｜ **分类** 跨平台 / Web 差异 / 代币管理 ｜ **平台** All

- **测什么**：当用户在其他应用停留后返回钱包，应重新查询最新的余额、代币列表和价格数据，防止显示陈旧数据导致用户误判。
- **怎么测**：1) 打开钱包，记录当前显示的某一资产余额（如 USDC: 100）；2) 切换到另一个应用（如浏览器），停留几秒；3) 在外部应用中发送一笔交易到钱包地址，或在区块浏览器确认有新交易；4) 返回钱包应用（iOS: 切换 App、Android: 长按任务、Web: 重新聚焦标签页）；5) 观察余额是否更新，代币列表是否刷新。
- **预期结果**：余额应更新到最新值。如果在后台期间有新交易，应立即可见（无需手动拉刷新）。
- **边界/异常**：若网络连接在后台中断，返回前台时应尝试重连而非显示旧数据。
- **源码参考**：`src/services/platform.ts:isAppActive (检查应用是否在前台)；需找到 AppState 或 visibilitychange 监听逻辑`

### 99. 添加代币 - 保存并在列表中显示已添加代币

**`P2`** ｜ **分类** 跨平台 / Web 差异 / 代币管理 ｜ **平台** All

- **测什么**：点击「Add to Wallet」后代币应被保存到本地存储，并在「已添加代币」(Added Tokens) 部分重新出现，确保持久化和重复添加防护。
- **怎么测**：1) 按上述步骤添加一个多链代币（如 USDC）；2) 在结果卡上点击「Add to Wallet」；3) 观察该卡变为「Token Added」(带绿勾)状态；4) 滚动到底部，在「已添加代币」部分找到刚添加的代币；5) 关闭 Add Token 屏幕并重新打开，重复第 4 步。
- **预期结果**：第 3 步：按钮转变为禁用状态且文案改为「✓ Token Added」或类似。第 4 步：代币以「USDC · USD Coin · Ethereum」格式显示在列表中。第 5 步：重启后代币仍在列表（验证 AsyncStorage 持久化）。
- **边界/异常**：同一个代币在不同链上添加时应生成不同的 token ID (chainId_address)，允许在多个链上跟踪同名代币。
- **源码参考**：`src/components/ui/AddTokenPanel.tsx:187-218; src/services/storage.ts:saveCustomToken`

### 100. 多链余额显示 - 同名代币在不同链的区分

**`P2`** ｜ **分类** 跨平台 / Web 差异 / 代币管理 ｜ **平台** All

- **测什么**：当用户在多条链上都持有同一代币（如 USDT 在 Ethereum、Polygon、Arbitrum），列表应清晰区分每条链的余额，使用链 logo 徽章或链名标签，避免混淆。
- **怎么测**：1) 创建或导入一个多链钱包，在不同链上各持有一些 USDC（或手动添加跨链 USDC）；2) 打开主资产列表；3) 查看 USDC 代币行，观察是否显示链徽章或明确的链标识；4) 点击 USDC 进入详情页，观察是否显示各链上的余额分解。
- **预期结果**：资产列表中，USDC 的 logo 右下角有小的链 logo 徽章（如蓝色 E for Ethereum）。点击进详情后，显示类似「Ethereum: 50 USDC | Polygon: 30 USDC | Arbitrum: 20 USDC」的分解。
- **边界/异常**：自定义代币 ID 包含 chainId，系统应能正确区分跨链部署。
- **源码参考**：`src/components/TokenLogo.tsx:20-100 (chain badge); src/models/network.ts`

---

## 附录 A — P0 冒烟清单（每次发版至少过这些）

- [ ]  #1 无限授权被拦截（ERC-20 Approve）
- [ ]  #2 编辑授权后 enforceNoUnlimited 最终守卫
- [ ]  #3 交易签名前显示无限授权防护，capped 后的参数应在批准时使用
- [ ]  #4 ERC-2612 Permit 离线签名显示为不可编辑
- [ ]  #5 单币单人：收款人地址投毒防御（首次交互警告）
- [ ]  #6 单币单人：Contract 地址警告（非钱包地址转账风险）
- [ ]  #7 SIWE 域名绑定检查与钓鱼防护
- [ ]  #8 Blind Sign 无描述符时的警告与降级
- [ ]  #9 单币单人：输入金额→预期 max 包含 gas 预留
- [ ]  #10 Max-send 预留 EntryPoint prefund：确保费用不会溢出
- [ ]  #11 sweep 模式（多币一人，清空钱包）：选择多个 token，gas 预留准确
- [ ]  #12 sweep 模式：一个 MultiSend UserOp，无限授权防卫激活
- [ ]  #13 split 模式：单个 MultiSend UserOp 批量提交，失败恢复正确
- [ ]  #14 高价格拒绝保护：GasQuoteTooHighError 触发与显示
- [ ]  #15 扫描 WalletPair QR 后显示 4 位指纹核对
- [ ]  #16 用户确认指纹后 E2E 加密通道连接成功
- [ ]  #17 创建钱包-钱包地址幂等性与多链一致
- [ ]  #18 首次创建钱包-Passkey生物识别仪式
- [ ]  #19 交易签名-每笔都需生物识别
- [ ]  #20 公钥提取与地址计算-CBOR与P256验证
- [ ]  #21 pending→confirmed 对账及持久化
- [ ]  #22 eth_sendTransaction 提交后立即记录 pending 状态，关闭弹框不丢失
- [ ]  #23 Bundler 资金不足时的融资模态框（BundlerFundingModal）
- [ ]  #24 自定义网络添加与完整性校验（全契约+RIP-7212）
- [ ]  #25 RPC 修复 URL 验证与链 ID 检查

## 附录 B — 按分类索引

**清晰签名与授权安全（approval/模拟/SIWE）**

- #1（P0）无限授权被拦截（ERC-20 Approve）
- #2（P0）编辑授权后 enforceNoUnlimited 最终守卫
- #4（P0）ERC-2612 Permit 离线签名显示为不可编辑
- #7（P0）SIWE 域名绑定检查与钓鱼防护
- #8（P0）Blind Sign 无描述符时的警告与降级
- #29（P1）ERC-7730 Clear Signing Intent 三段设计
- #30（P1）有限授权编辑与确认
- #31（P1）ERC-721 NFT setApprovalForAll 布尔授权编辑
- #32（P1）撤销授权（approve-to-zero）显示为安全绿色
- #33（P1）Balance Change Preview 余额模拟预览
- #34（P1）EIP-712 签名内容部分解码与风险标注
- #35（P1）过期时间戳字段标注与风险提升
- #36（P1）多链场景下链 ID 与网络选择一致性
- #37（P1）未验证代币小数点标注
- #81（P2）合约部署（CREATE2）的平静展示
- #82（P2）ERC-165 代币类型自动检测（ERC-20 vs ERC-721）
- #83（P2）清晰签名风险配色准确映射

**dApp 连接（WalletPair/批量/历史回放）**

- #3（P0）交易签名前显示无限授权防护，capped 后的参数应在批准时使用
- #15（P0）扫描 WalletPair QR 后显示 4 位指纹核对
- #16（P0）用户确认指纹后 E2E 加密通道连接成功
- #22（P0）eth_sendTransaction 提交后立即记录 pending 状态，关闭弹框不丢失
- #38（P1）dApp 发起 eth_sendTransaction 请求显示交易估算与费用层级选择
- #39（P1）dApp 发起 personal_sign 请求时触发签名确认弹框
- #40（P1）wallet_sendCalls (EIP-5792 批量)单签一次提交多个操作
- #73（P1）账户切换或链切换后 dApp 收到 accountsChanged/chainChanged 事件
- #74（P1）用户点击"断开连接"后 dApp 无法再发送请求，连接面板清空
- #84（P2）60 秒内自动重连未成功时，UI 显示"立即重连"手动恢复按钮
- #85（P2）切后台再回前台时 WalletPair 自动重连而无需手动操作
- #86（P2）同一时刻只允许一个 dApp 会话，新连接自动断开旧连接
- #87（P2）链上无法检索 Safe 合约代码时，dApp eth_getCode 返回 runtime bytecode 伪装 EOA 为智能钱包
- #88（P2）连接历史中的已确认交易点击后显示只读重放签名界面

**转账发送（含 split/sweep/联系人/收款人识别）**

- #5（P0）单币单人：收款人地址投毒防御（首次交互警告）
- #6（P0）单币单人：Contract 地址警告（非钱包地址转账风险）
- #9（P0）单币单人：输入金额→预期 max 包含 gas 预留
- #11（P0）sweep 模式（多币一人，清空钱包）：选择多个 token，gas 预留准确
- #12（P0）sweep 模式：一个 MultiSend UserOp，无限授权防卫激活
- #13（P0）split 模式：单个 MultiSend UserOp 批量提交，失败恢复正确
- #23（P0）Bundler 资金不足时的融资模态框（BundlerFundingModal）
- #41（P1）单币单人：余额不足/gas 不足警告（黄色提示，提交被禁）
- #42（P1）单币单人：法币输入模式下金额换算准确
- #43（P1）split 模式（一币多人）：添加/删除收款人行，总额计算准确
- #44（P1）交易模拟预览：发送前显示资产变化汇总（BalanceChangePreview）
- #45（P1）交易完成反馈：签名→提交→确认 UI 状态流转，userOpHash 链接
- #46（P1）单币单人：收款人身份解析（ENS/.bnb/.arb/Basename/Passkey Index）

**Gas / Bundler / 费用 / Tempo / 交易提交**

- #10（P0）Max-send 预留 EntryPoint prefund：确保费用不会溢出
- #14（P0）高价格拒绝保护：GasQuoteTooHighError 触发与显示
- #47（P1）首笔交易触发 Gas 账户激活弹窗
- #48（P1）Free Activation 弹窗：资格检查与请求流
- #49（P1）自己充值激活流：显示 QR 与检查平衡轮询
- #50（P1）Gas 费用估算与显示：单层到多层拆分
- #51（P1）Gas Tier 切换：Slow/Standard/Fast 与费用实时更新
- #89（P2）Tempo Gas 模型：pathUSD 支付与费用显示

**钱包创建与 Passkey（账户/恢复）**

- #17（P0）创建钱包-钱包地址幂等性与多链一致
- #18（P0）首次创建钱包-Passkey生物识别仪式
- #19（P0）交易签名-每笔都需生物识别
- #20（P0）公钥提取与地址计算-CBOR与P256验证
- #26（P1）账户公钥上传-成功同步到服务器
- #60（P1）账户恢复-已有Passkey登录（本地）
- #61（P1）账户恢复-跨设备恢复（iCloud/Google Keychain）
- #62（P1）创建钱包-确认清单必填拦截
- #63（P1）账户公钥上传-失败与手动重试

**历史与活动 / 交易详情 / 对账**

- #21（P0）pending→confirmed 对账及持久化
- #27（P1）待确认交易显示 pending 状态徽章
- #64（P1）发送交易出现在活动列表
- #65（P1）接收交易显示在活动列表（native 转账及 ERC20）
- #66（P1）dApp 交易及签名请求显示在历史中
- #67（P1）confirmed 交易显示成功徽章及完整哈希
- #75（P1）USD 价值计算及稳定币推断

**网络与 RPC（自定义链/自托管端点/容灾）**

- #24（P0）自定义网络添加与完整性校验（全契约+RIP-7212）
- #52（P1）RPC 故障转移与多源负载均衡
- #53（P1）12条默认链列表显示与快速切换
- #90（P2）Tempo 链的特殊 gasModel 与 bundler 集成
- #91（P2）第三方 RPC Provider 密钥集成（Alchemy/dRPC/Ankr）
- #92（P2）RPC 端点临时封禁与永久封禁恢复机制

**韧性 / 故障注入 / 错误态 / Bug 上报**

- #25（P0）RPC 修复 URL 验证与链 ID 检查
- #72（P1）Bug 上报后台成功提交
- #93（P2）RPC 故障 Banner 与修复流程
- #94（P2）vela.failRpc 单链注入
- #95（P2）Bug 上报后台不可用降级
- #96（P2）一键 Bug 上报 - 预览内容脱敏
- #97（P2）字体加载 3 秒超时兜底

**余额与资产组合（多链/价格/到账）**

- #28（P1）稳定币硬锚 $1 价格展示
- #54（P1）多链余额聚合无缓存时的 $0 闪现
- #55（P1）DEX → Chainlink → null 价格兜底链路验证
- #56（P1）单链 18 秒超时降级验证
- #57（P1）到账实时探测与 celebration 反馈验证
- #58（P1）账户切换器多账户余额聚合验证
- #59（P1）token 详情页 send/receive 按钮路由验证

**收款 / 付款请求 / 二维码 / EIP-681**

- #68（P1）收款地址模式 - QR码生成和显示
- #69（P1）发送屏幕预填 - 来自pay链接的参数注入
- #70（P1）EIP-681 URI 解析 - 原生支付 & ERC-20 transfer
- #76（P2）支付请求模式 - EIP-681 生成和编辑
- #77（P2）金额输入验证和精度限制
- #78（P2）不支持的网络和未知代币错误处理
- #79（P2）入账实时检测和反馈
- #80（P2）QR码扫描 - 相机和相册上传（真机）

**跨平台 / Web 差异 / 代币管理**

- #71（P1）添加 ERC-20 代币 - 输入地址自动解析
- #98（P2）应用从后台回到前台时自动刷新余额
- #99（P2）添加代币 - 保存并在列表中显示已添加代币
- #100（P2）多链余额显示 - 同名代币在不同链的区分

---

*本指南由对 vela-wallet 源码的全量通读生成；每条用例附源码引用。源码演进后，先核对引用文件再据此测试。*
