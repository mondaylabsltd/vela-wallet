# 多签账户：方案

2026-10-27 · 第二版。§5.2、§5.3.2 于 2026-11-09 与代码核对（提交 `fa6083c87`，见各节开头的说明）。2026-10-28 起在 web 端实现（分支 `feat/multisig-account`），见下方"实现状态"。
配套的链上合约和服务在独立仓库 `safe-genesis-registry`（本地 `/Volumes/data/production3/safe-genesis-registry`），
合约已部署，服务尚未上线（见第 8 节）。

## 实现状态（web，2026-10-28）

**已完成**，并在 Gnosis 分叉上用测试 passkey 端到端跑通（`scripts/onchain/multisig-fork`，`e2e/onchain`）：

| 部分 | 内容 |
| --- | --- |
| vela-core `multisig` | genesis（Safe L2 v1.4.1、CompatibilityFallbackHandler、无 hook、owner 按给定顺序）、safeTxHash、批准哈希 `SafeMessage(safeTxHash)`、passkey assertion → 批准、批准核对、签名拼装（按 owner 排序、嵌套 1271、执行者 `v = 1`）、执行调用（缺代码的多签和批准 owner 一并部署）、交易解读与安全形状。对外只有一个 JSON 入口 `multisigCall` |
| genesis 服务 | 凭 owner passkey 证明代付未部署 Safe 的记录（独立通道和单 owner 限额）；提案信箱（`/api/proposals`，批准按链上规则核对、并发安全合并）；证明过的 genesis 在记录落链前就能查到（`pending`） |
| web | 账户切换器里的多签一栏；创建（不部署）、按地址添加、邀请；首页多签面板（规则、成员、待批准）；多签有名字：创建第一步填写，随记录写入注册表 v4（`record` 必带名字，0xF412…945d），地址到名字的映射，不参与地址推导、可以重名，多签自己可以在链上改名（见 3.3）；任何钱包里添加过的网络都能发起提案（信箱收任何链的提案；服务不读的链上信箱按 genesis 判定成员，钱包自己读链上成员和 nonce，见 3.4）；创建时可选本人任何有 Safe v1.4.1 合约的网络，记录一律写到索引链（3.4）；发起转账走和 Vela 账户完全相同的发送流程（发送、扫码、通讯录、代币详情；选币、金额、收款人、分账、确认页都一样），只有两处不同：手续费那一行换成「几人中几人批准；手续费由执行成员支付」，确认按钮是「发起并批准」，确认后打开提案详情；提案详情（批准 / 批准并执行 / 执行、邀请链接、拒绝 = 同 nonce 作废）；执行后写活动记录，其他设备执行的从链上 `ExecutionSuccess` 找回；15 种语言 |
| 导入与外部 owner（§11） | 从 Safe{Wallet} 导出文件导入（core 解析；已部署的由服务从交易服务还原 genesis，未部署的在本机由 props 推导，都由 core 核对地址；地址簿名字作本机备注）；Safe v1.3.0 / v1.4.1 / v1.5.0；外部钱包签名走 PeerConnect + WalletPair（Vela 的 `/connect` 页面是钱包端，EIP-6963 发现钱包，签 SafeTx 的 EIP-712），签名由 core ecrecover 核对后进信箱；服务接受 EOA 批准；没有 owner 在本机时，发送确认直接走外部签名；签名凑齐后任何本机 Vela 账户都可以执行（非 owner 时不占签名位）；分叉测试覆盖：EOA 成员 + passkey 成员混签执行，导入两个 Safe 后两个 EOA 先后签名、非 owner 的 Vela 账户执行并部署 |

**体验统一（2026-10-28，owner 反馈后重做）**：多签用起来只比 Vela 账户多一点点东西。

- 头部：名字独占一行（不被网络徽章挤压），地址同一行后面是"2/3"规则小标签和网络徽章；点规则进入设置。
- 首页：和 Vela 账户完全一样，去掉了顶部多签面板；待批准交易作为 Activity 最上面一组"等待批准"的行（标题、金额、"轮到你批准 · 1/2 · 网络"），点开是提案详情。
- 账户切换器：Vela 账户和多签在同一个列表里（多签行带"2/3"），底部只有一个"添加账户"，打开后四个选项：新建账户、登录已有账户、创建多签、添加多签（地址或从 Safe{Wallet} 导入）。
- 设置 → 账户：多签也在账户列表里；当前是多签时，"密钥"一块换成多签自己的：成员（标出本设备上的）、规则、邀请成员、"把多签配置保存到 Ethereum"（对应 Vela 账户的"保存公钥到 Ethereum"，由本设备的 Vela 账户发 v4 的 `record` 交易）、注册表里的名字（"在链上改名"发起提案）、从本设备移除；服务没有它的记录（旧版注册表里的）时标"尚未共享"并提供"重新共享"。
- 创建：第一步就填名字。
- 扩展连接 dApp：当前是多签时，dApp 看到的就是多签地址；`eth_sendTransaction` 变成提案（本机成员 passkey 批准，或外部成员钱包签名），请求一直挂着，执行后用执行交易的哈希回答 dApp；被同 nonce 作废则回答拒绝。多签暂不签消息（`personal_sign`、typed data 回 4200）。核心的交易解读新增"合约调用"，仍拒绝对多签自身的未知调用和 delegatecall。

**和设计稿的出入**：

- 提案信箱直接放进了第一阶段（5.3.4 原计划如此），推送、链上批量批准、提议人都还没做。
- **成员管理**（2026-10-29，第 7 节）：设置里"更改成员"——添加成员（地址 / ENS 名（core 正向解析，地址全文显示）/ 联系人或本机账户；名字和类型只记在本机）、
  每位成员的"替换"（swapOwner，丢了 passkey 的恢复路径）和"移除"、阈值步进器。一次操作 = 每条**已部署**链各一个提案，由 core
  （`member_change_tx`）按那条链**当前**的成员（`getOwners` 的链表顺序，prevOwner 由此算出）和阈值构造；做不到的（阈值大于成员数、
  移除后不够阈值、已是成员、多签自己、零地址 / 哨兵、超过 20 人、那条链上已有一个成员变更在等）按链用一句话说明，不签。
  未部署的链只说"会以初始成员开始"，不提供任何会部署的操作。等待中的成员变更合成一项，显示各链进度（"Arc 1/2 · Base 0/2"）。
  每条链一个 passkey（各链的 safeTxHash 不同）。创建时的阈值步骤和设置里各有一段 passkey 原生恢复说明（2/3 丢一把，另两位替换即可）；
  带延时的恢复模块留作后续设计。
- 首页面板列出待批准交易，"拒绝"即发起同 nonce 的作废交易；反对票、软过期尚未实现。
- ~~多签账户暂不连接 dApp：web 本身没有内置浏览器，扩展仍以 session 的 Vela 账户应答。~~（已过时，R23 核对：多签可以连接 dApp，站点请求变成提案，见 `multisig_dapp` 与 `docs/multisig-machines.md` §31、§35。）
- 执行手续费由执行者账户按现有方式在交易内支付。~~执行前不单独展示金额（按 relay 当时报价签名）。~~（已过时，R23 核对：执行前显示手续费，只按显示的那笔签，web `execution-fee.ts` 的 `ExecutionQuote`、core `Event::Execute.shown_fee`。）
- 支持 Safe v1.3.0 – v1.5.0 的已知 factory 和 singleton；更早的版本（EIP-712 域里没有 chainId）导入时提示"暂不支持这个版本"。
- Vela passkey 成员在 v1.5.0 Safe 里的批准挑战不同（合约签名走 `isValidSignature(bytes32,bytes)`），暂未适配；EOA 成员不受影响。
- 外部签名默认中继 `wss://relay.peerconnect.dev`（可用 `vela.peerconnectRelay` 改）。公共中继要求每端带 token：由 getvela.app 的 `POST /api/peerconnect-token` 用 App Secret（Worker secret `PEERCONNECT_APP_SECRET`）按设备签发（一年有效，设备本地缓存）；该接口不可用时退回 PeerConnect 公开试用 token（两小时）。
- 外部钱包自己发送 `execTransaction`（本机没有任何 Vela 账户时）还没接到界面上；Vela 总有登录账户，所以目前都由 Vela 账户执行。

**上线前还需要**：

- 部署 `genesis-cf`（含信箱，`wrangler deploy`、`safe-genesis.getvela.app` 域名、代付私钥）；
- 部署信箱的新版本（任何签名者、跨链列表 `GET /api/proposals/{safe}`、按 safe / 签名者限流；分支 `mailbox-any-signer`）——
  钱包的跨链发现在旧服务上退回逐链读，新成员在服务不读的链上的批准要等新服务上线才会被收下；
- 在主网用真实 passkey 做一次小额全流程。

**与第一版的区别**（owner，2026-10-27）：多签不再是 Vela 里的内置应用，而是**和 Vela 账户平级的账户**。
"多签账户本质也是账户，只是在 Vela 里创建的多签账户的 owner 是 Vela 账户而已。"
Vela 账户有的功能（资产、活动、Explore、收款、转账……），多签账户也要有。

## 1. 结论

- **Vela 账户保持 1-of-N**，不做原生 M-of-N。
- **多签账户**是一个完全标准的 Safe（M-of-N），出现在账户列表里，和 Vela 账户并列，可以切换成当前账户。
- 它的 owner 通常是 Vela 账户，也可以是任意地址（硬件钱包、其他 Safe），保持和 Safe{Wallet} 兼容。
  **Vela 只用本机已登录的 Vela 账户替它签名**。
- **创建时不部署**，只把初始配置（genesis：owner 顺序、阈值等）写进链上的 `SafeGenesisRegistry`。
  地址从这一刻起在所有链上都确定；在某条链上第一次转出时才部署。
- **没有名字**：多签账户一律显示地址和 identicon，用成员列表来辨认（第 3.3 节）。
- **登录时**只输入多签地址：从 genesis 服务取配置，Vela 自己核对配置能算出这个地址，然后加载。
- **签名**：多签账户自己没有钥匙。它的"签名"就是本机的 owner（Vela 账户）各自批准，凑够阈值后由其中一个
  owner 发交易执行。所有签名入口都要认识这种账户。
- **其他成员**通过定向链接、提案信箱（同步到 Safe 的队列）和推送收到提案，在自己的设备上独立核对、批准；最后一个批准的人默认直接执行（5.3）。
- **dApp 交易一直挂着**，直到多签执行或被拒绝才回复网站（第 6 节）。
- 成员和阈值可以修改，换人不换地址。
- 同一个地址可以扩展到其他链，靠的是 registry 里的 genesis。

## 2. 为什么不在 Vela 账户本身做 M-of-N

| 问题 | 说明 |
| --- | --- |
| 阈值进地址 | 已有钱包无法升级成多签；registry 元数据也要加阈值，否则恢复时算不回地址 |
| 钥匙固定 | 丢 N−M+1 把就永久锁死 |
| 提案随 gas 过期 | 4337 下签的是 SafeOp 哈希，包含 gas 和手续费，gas 一涨就要全部重签 |
| 消息签名异步化 | ERC-1271 也要 M 份签名，dApp 登录基本不可用 |

多签账户绕开了前三条：Vela 账户不动；SafeTx 里的 gas 字段全为 0，执行时的 gas 由执行者（某个 owner）自己的
UserOp 支付，所以**提案不会因为 gas 价格变化而过期**，只会因为 nonce 被占用而失效。第四条对多签账户依然成立，
见第 6 节。

## 3. 账户模型

### 3.1 多签账户记录

```text
MultisigAccount {
  address            多签地址（身份，也是它唯一的显示名）
  genesis            factory、singleton、saltNonce、initializer、proxyCreationCodeHash（本地缓存，已核对）
  genesis_recorded   registry 是否已确认收录（未确认的进 outbox 继续补，见 4.1）
  added_at
}
```

- **不存成员和阈值**。它们每条链各不相同、随时会变，每次使用时从链上读取，本地只缓存最近一次结果，用于离线显示。
- **单独一个存储键**：`vela.multisigAccounts`，不混进 `vela.accounts`。原因有二：
  - 旧版本的 `Account` 读取器要求有 `public_key_hex`（[mod.rs:615](../rust/crates/vela-core/src/app/mod.rs#L615)），
    会拒绝没有钥匙的记录。
  - 会话迁移会用钥匙重算每个账户的地址（[session.rs:841](../rust/crates/vela-core/src/app/session.rs#L841)）。
    多签账户如果混在同一个列表里，在同源的 web 上会被旧版本读坏。
- **会话的账户行变成两类**：`AccountRow = Vela(Account) | Multisig(MultisigAccount)`，一律按地址识别。
  当前账户按地址另存一个键；旧的 `vela.activeAccountIndex` 照写，供旧版本读取。
- **移除多签账户**只删本机这一行，和移除 Vela 账户一样，重新输入地址就能回来。
- **本机没有它的 owner** 时，多签账户照样能加，状态是**只读**：能看资产和活动，不能发起或批准。
  界面提示"登录其中一个成员账户后即可签名"。如果移除的 Vela 账户是某个多签账户在本机唯一的 owner，移除前要提示。

### 3.2 各功能怎么适配

大部分功能只按地址工作，换成多签地址就能用；需要改的都和签名有关。

| 功能 | 多签账户下的行为 |
| --- | --- |
| 资产、联系人、代币管理、网络 | 不变，按地址读取 |
| 收款 | 未部署的链：重放检查为 `ready`，且没有任何链改过成员时，才展示地址，并注明"未部署，第一次转出时自动部署"。已部署的链：成员和其他已部署链一致时才展示。其他情况明确提示"转到这里的资金由初始成员控制" |
| 活动 | 收到的转账照常扫描。发出的交易记录 `from = 多签地址`，跟踪的是执行者那笔 UserOp。另加一个"待批准"分组：本账户的未执行提案 |
| 转账（单笔、一币多人、多币一人） | 同一套界面。确认后不是提交 UserOp，而是**发起提案**（第 5 节）。多笔合并时用 MultiSendCallOnly |
| Explore（dApp） | 能连接；交易变成提案；消息签名第一版不支持（第 6 节） |
| 成员管理 | 新增页面：增删换成员、改阈值，按链显示同步状态（第 7 节） |
| 扩展到新链 | 新增操作（第 7 节） |
| Agent（clientmcp） | 能代为发起提案，批准永远是本人在 Vela 签名页点的（spec 107 E3） |
| 账户切换器 | 多签行显示地址缩写、identicon 和 "2/3" 标记；只读的再标出来 |

### 3.3 名字（2026-10-28 改：名字随记录写入，地址到名字的映射；2026-10-29 注册表 v4：多签可以改名）

- **定**（owner）：多签地址是唯一的，注册表把地址映射到一个名字即可；名字可以重复，不参与地址推导。
- **合约**（registry v4，`0xF412388bF908E510066d3Be5c704bd9A327f945d`，盐 `keccak256("safe-genesis-registry/v4")`）：
  `record(factory, proxyCode, singleton, initializer, saltNonce, name)` 必须带名字（1–64 字节，否则 `EmptyName` / `NameTooLong`），
  记录已存在就不改；`rename(name)` 只能由 Safe 自己调用（`msg.sender` 是 Safe，即成员批准的 Safe 交易），可以改任意次，
  只改名字和 `renamedAt`，并发出 `GenesisRenamed`；Safe 必须已部署在该注册表所在的链上。`genesisOf` 返回
  `name`、`renamedAt`、`recorder`、`index`；`renamedAt > 0` 或 `recorder == safe` 时名字才是 Safe 自己定的，否则只是第一个写入者定的。
  每个注册表（Gnosis、Ethereum）各有各的名字。
- **钱包的做法**：
  - 发给服务的 genesis 一律带名字（`namedGenesis`）：自己的名字，没有就用服务同样的规则从地址算出的默认名（"Quiet Harbor"，
    `default_name`，与服务逐字一致）。服务会丢弃提案里不带名字的 genesis，所以这一条是必须的；导入时 Safe{Wallet} 地址簿的名字只作本机备注，不上链。
  - 显示的名字是记录在链上的名字：每次会话用本钱包自己的 RPC 读一次 `genesisOf`（core 重新推导地址），读不到时才用服务给的；
    本机备注（`label`）从不被改名影响。
  - **改名**：设置里"在链上改名"→ 一笔多签自己的交易 `{to: 注册表, data: rename(name)}`，走和转账完全一样的提案、批准、执行；
    在存这条记录的注册表链（索引链：服务的主链 Gnosis 优先，有记录、钱包有这个网络）上执行——不管多签平时用在哪些网络。
    **改名从不部署**（owner，2026-10-29）：只有多签已经部署在那条链上，设置里才提供"在链上改名"；没部署时只说
    "注册表记录只能由多签自己在 Gnosis 上改名，而它没有部署在那里"，什么都不提供（3.4）。core 把它解读为"在注册表中把这个多签改名为 …"，
    名字不合规（不可见字符、首尾空格等）或编码不规范的一律不签。
  - 签名页（clear signing）把对注册表的 `record(...)` 和 `rename(string)` 当作 Vela 自己的合约来读：网络、地址、名字三行，确定无疑。
- **规则**：非空、首尾无空格、无控制字符和 U+FFFD、不超过 64 字节。名字只是标签：谁先写记录谁定名字，不证明任何事，身份是地址和成员。
- **显示**：记录里有合规的名字就显示它，否则显示本机备注（导入时 Safe{Wallet} 地址簿的名字），再否则显示规则"多签 2/3"。
- **创建**：第一步就填名字（和 Vela 账户一样），saltNonce 仍是随机数。

### 3.4 注册表是索引，不是多签的"所在链"（owner，2026-10-29）

- **定**（owner）：Safe Genesis Registry（v4，`0xF412388bF908E510066d3Be5c704bd9A327f945d`，Gnosis 和 Ethereum 上已部署）
  只是**配置索引**，和 passkey 的 p256-index 一样。genesis（factory、singleton、initializer = owners + threshold、saltNonce、名字）
  记在 Gnosis（服务代付写入）或 Ethereum（用户自己付费写入）。多签本身和链无关：同一个 genesis 在任何有 Safe v1.4.1 合约的链上
  部署出同一个地址。生产服务 `safe-genesis.getvela.app` 只需要 Gnosis；`/api/health` 的 `chains`（带 `registry` 标记）说的是
  **服务自己读、自己索引哪些链**，不是多签能在哪些链上。提案信箱接受任何链的提案。
- **能建在哪些网络**（core `MULTISIG_CONTRACTS` / `safe_contracts`）：新多签的 genesis 只用到三个 Safe v1.4.1 合约——
  `SafeProxyFactory`、`SafeL2` singleton、`CompatibilityFallbackHandler`。钱包用自己的 RPC 对这三个地址 `eth_getCode`，
  由 core 判定 `present` / `missing` / `unknown`（没回答的不算缺）。创建页列出本人显示的所有网络，只去掉 core 判为 `missing` 的；
  服务的链表不再参与（原来的"服务不支持你的任何网络"一句已删除）。每个能加进 Vela 的网络本来就要求有 Safe L2 和 factory
  （`REQUIRED_CONTRACTS`），实际上只有 fallback handler 可能缺。
- **记录写在哪**：一律写到索引链——服务的 `homeChainId`（Gnosis；服务没说时用 100），和创建者选的网络无关，
  也不要求那个网络是服务读的链。"把多签配置保存到 Ethereum"仍是可选的、由本机账户付费。
- **多签"在"哪些网络**（core `multisig_networks`，按顺序去重）：链上已部署的链（钱包自己读）∪ 信箱里有它提案的链
  ∪ 提案链接指明的网络 / 本人在"选择网络"里选的网络 ∪ 本机记下的网络（创建时选的、在本机用过的）。
  **从不用记录所在的注册表链**。什么都说不出来时，问本人（"选择它的网络"），不猜。
- **提案在每条链上都能找到**（owner，2026-10-29，Q3）：信箱有跨链列表 `GET /api/proposals/{safe}`（不带 `chainId`，
  一次读完它在所有链上的提案，分页，和单次读同一档限流）。钱包每次刷新只读这一次（不再每个网络读一次），
  有提案的链 ⇒ 是它的网络（本机有这个网络就加进来、照常逐链读成员和 nonce）；未部署的链上的提案也因此被
  另一台设备发现，不需要链接。服务没有这条路由（旧版答 404，或缺 `chainId` 的 400）时，退回原来的逐链读。
  本机**没有**的网络（成员自己的网络）：core `missing_networks` 列出来，在"等待批准"里显示
  "在 <链索引名>（chain <id>）上——本设备没有的网络"，带"添加网络"（打开设置 → 添加网络，预填 chain id；链索引不认识就让人填 RPC）；
  从不读、不判、不计入"轮到你"——加了网络之后它和其他提案一样审阅。提案链接指向本机没有的网络也一样（加完网络回来链接再打开）。
  已有的多签每次会话还用钱包自己的 RPC 在本人的网络上查一次部署（`discoverChains`）。
- **成员是每条链各自的**（owner，2026-10-29，Q1）：genesis 的成员是初始成员；在某条链上签名、执行时，钱包读那条链
  **当前**的成员和阈值（`readMembers`，钱包自己的 RPC），状态、谁能批准、阈值、nonce 都按它算；只有那条链当前成员的、
  core 核对过签名的批准才计数，其他有效批准显示为"不计入（现在不是 <网络> 的成员）"。设置里显示"初始成员"
  和每条已部署链的"<网络> 上的当前成员"（标出新增 / 移除、阈值变化），未部署的链说"会以初始成员开始"。
  发送确认页的"几人中几人批准"也按所选网络的当前成员。
- **信箱只存不判**：服务接受任何对该 safeTxHash 签名有效的批准（EOA ecrecover；passkey 成员按其 genesis 和 WebAuthn 核对，
  拿不到工厂代码时存为 `verified: false` 的"未核对合约签名"，由钱包核对），不管签名者是谁；`signerIsGenesisOwner`、
  `signerIsOwner`（服务读得到的链上）只是信息标记。每个提案最多 32 个非已知成员的批准，每个 safe+链最多 100 个未执行提案
  （其中非已知成员发起的最多 32 个）。写入按 safe、签名者限流，另有宽松的按 IP 上限；`Retry-After` 对浏览器可见。
- **服务不读的链上的提案**：信箱照收、照存（genesis-cf `Target::Unread`），但服务在那里看不到链：它给的成员永远是 genesis 的、
  nonce 0，`deployed: false`。钱包从不用服务给的成员，都由钱包自己读。那条链上改过成员后，新成员的批准信箱照收，钱包计数。
- **信箱忙（429）**：写入（提案、批准、取消、记录）遇到 429 从不算失败：签好的批准留在本机发件箱，界面说"多签服务忙——N 秒后重新发送"，
  按 `Retry-After`（或正文里的"retry in N s"，再不行退避）重发，约 2 分钟；之后说"还没发出去，已保存在本设备"，提供"重新发送"
  （提案链接照常可分享）。从不重新签名。读遇到 429 安静地等它说的时间，列表标为可能过时。
- **改名在哪条链执行**：`rename(name)` 只能由 Safe 自己在**存这条记录的注册表所在链**上调用，所以改名交易总是在索引链
  （有记录、钱包有这个网络的注册表链，主链优先，即 Gnosis）上执行，不管多签平时用在哪些网络。**改名从不部署**
  （owner，2026-10-29，Q2）：只在多签已部署在那条链上时提供；core 的执行计划拒绝"部署 + 改名"，提案详情里未部署链上的
  改名说"不能在这里执行"。改名提案也让 Gnosis 成为它的网络之一（那里有了它的提案）。
- **新网络上的第一笔**：在未部署的网络上执行第一笔交易会按 genesis 部署它；只要别的网络上的当前成员和初始成员不同，
  审阅页就说"这个网络以初始成员开始"并列出初始成员。

### 3.5 谁能代表一个 Safe：链上状态，或反事实规则（2026-10-29，r4-e S1）

- **原则**：注册表和信箱都不能决定谁能代表一个 Safe。**已部署的链上，权威是那条链上 Safe 的实时状态；未部署时，
  只有一条设计好、核对过的反事实规则，由 core 判定**（`multisig::counterfactual_authority`、`multisig::deployed_check`）。
  web 只负责读链（`eth_getCode`、`eth_getStorageAt`、`eth_call`），判定全在 core。
- **起因**（r4-e）：恶意或被攻破的 genesis 服务给出一个 `setup` 会 delegatecall
  `SafeModuleSetup.enableModules([攻击者])` 的 genesis。地址照样推导得出（CREATE2 只认参数），owner 和阈值照样读得出，
  钱包却把它当成普通多签加进来——部署出来以后，攻击者的模块不经任何成员就能转走资金（Arc Testnet 上已实测部署）。
- **反事实规则**：`counterfactual_authority(genesis, safe, chain_facts) → Verified { owners, threshold } | Refused { reasons } | Unknown { unread }`。
  `chain_facts` 是目标链上 `eth_getCode` 的结果，读哪些地址由 core 说（`counterfactual_reads`）。全部满足才是 Verified：
  1. genesis 推导出所说的地址（原有的 CREATE2 核对）；
  2. factory、singleton、fallback handler（以及下面允许的 hook 和它切换到的 L2 singleton）都是钱包支持的 Safe 官方部署，
     **并且**它们在目标链上的运行时代码哈希等于固定的官方代码哈希。支持的部署（`multisig::CANONICAL`，18 个）：
     factory 1.3.0（canonical、eip155）/ 1.4.1 / 1.5.0；singleton 1.3.0（L1、L2，canonical、eip155）/ 1.4.1（L1、L2）/ 1.5.0（L1、L2）；
     CompatibilityFallbackHandler 1.3.0（两个地址）/ 1.4.1 / 1.5.0；SafeToL2Setup 1.4.1 / 1.5.0。
     哈希来源：safe-deployments @ b45417dd（genesis 服务 `genesis-rules/data/known-contracts.json`）；2026-10-29 在 Gnosis (100)
     和 Arc Testnet (5042002) 上用 `keccak256(eth_getCode)` 逐个重算，两条链完全一致。v1.4.1 的五份代码存为测试向量
     （`rust/crates/vela-core/tests/vectors/safe-code/`），分叉测试每次在 fork 上重算全部 18 个。
     没有代码、代码不对 → Refused（`missingCode` / `wrongCode`）；链没回答 → Unknown（"暂时无法核对"），**绝不算 Verified**。
  3. `setup` 的 `to`/`data` 为空，或者**恰好**是允许的那一个 hook：`SafeToL2Setup.setupToL2(<同版本的官方 L2 singleton>)`
     （Safe{Wallet} 1.4.1 的部署就是这样：L1 singleton + 这个 hook，非以太坊主网上切到 L2 singleton，别的什么都不做），
     core 完整解码（36 字节，地址字之外不许有任何东西，hook、L2 singleton、genesis singleton 必须同一版本）。
     其他一律 Refused 并给出解码后的原因：`SafeModuleSetup.enableModules` → `module {address}`（"启用一个不经成员就能转走资金的模块"）；
     任何别的合约 → `setupCode {address}`（"在部署时执行 <地址> 上的代码"）；只有 data 没有 to → `notSetup`。
  4. `fallbackHandler` 为零，或者是该版本的官方 CompatibilityFallbackHandler（`handler {address}`）。
  5. `paymentToken`、`payment`、`paymentReceiver` 全为零（`payment {token, amount, receiver}`）。
  另外：initializer 必须是 Safe 自己的标准 ABI 编码（core 解码后重新编码，逐字节相同，偏移、填充、尾部都藏不了东西，否则 `notSetup`）；
  成员必须是 Safe 的 `setup` 会接受的（非空、无重复、无零地址和哨兵、不含 Safe 自己、1 ≤ 阈值 ≤ 人数，否则 `badMembers`）。
- **已部署的 Safe**：权威是链上状态。除了 `getOwners`/`getThreshold`，core 还读（`deployed_reads` 给出调用和槽位）：
  `getModulesPaginated(SENTINEL, 10)`（页的 `next` 本身也是模块）、guard（`GUARD_STORAGE_SLOT`）、v1.5.0 的 module guard、
  fallback handler（`FALLBACK_HANDLER_STORAGE_SLOT`）、singleton（槽 0）以及 handler 和 singleton 的代码哈希（`deployed_check` 第二轮）。
  有模块、有 guard、handler 不是任一版本的官方 CompatibilityFallbackHandler（零可以）、singleton 不是官方版本 → 高风险，
  逐条说清楚："在 Gnosis 上，这个多签有一个不经成员就能转走资金的模块：<地址>"。读不到 → 不加（`unavailable`），不当作干净。
- **用在哪里**（每个 genesis 进入或部署发生的地方）：
  - 按地址添加（`accounts.ts` `findMultisig` / `addMultisig`）、提案/邀请链接（`linked.svelte.ts` 走 `findMultisig`，失败时
    打开添加页由它说明）、Safe{Wallet} 导入（`importExported`：服务给的、`safeWalletPropsGenesis` 推导的、交易服务还原的 genesis
    一视同仁）、服务给的 genesis 答案都经过 `vetAuthority`：多签在它的网络上**哪里都没部署** → 每个网络都要 Verified，
    Refused 不加，Unknown 说"暂时无法核对"；**部署了** → 以链上状态为准，有风险就返回 `highRisk`。
  - 执行（`execute.ts` `planCalls`）：要部署多签时，**就在拼部署调用之前**重新读一次代码（不缓存），core 不是 Verified
    就抛 `GenesisRefusedError`（在 passkey 之前），提案页说明原因。core 自己也守着：`ExecutionPlan.deploy_facts` 缺失或
    判定不是 Verified，`execution_calls` 拒绝拼出部署调用。
  - 创建页的"能建在哪些网络"（`safe_contracts`）也改为核对代码哈希：地址上的代码不是 Safe 的，视同缺失。
- **界面**：Refused → 添加页（或导入行）红色说明框列出每一条原因，"添加"不可点，也没有"仍然添加"——不部署、不签名、不保存；
  Unknown → 一句"暂时无法核对 <网络> 上的 Safe 合约"。高风险 → 同样的说明框列出每个模块/guard/handler/singleton，
  只有点"我已了解，仍然添加"（危险样式，不是强调色）才加入；加入后 `findings` 存在账户上，设置页一直显示这份说明，
  账户切换器那一行标"高风险"，从不隐藏。
- **本机已有的多签重新核对**（2026-10-29，`recheck.ts`）：在此规则之前加入的多签从没核对过，已部署的多签也可能事后多了模块。
  每次会话一次、以及它多了一个网络（或本机多了这个网络）时，本机每个多签在它的每个网络上都按添加时的同一规则重新判定：
  已部署 → `deployed_check`；未部署 → `counterfactual_authority`。结果替换该网络在账户上的 `findings`（链是权威：
  风险消失了也就不再标）。新发现的风险 → 设置页"高风险"说明框、切换器"高风险"标记；genesis 在某网络被拒 →
  该网络**只读**，设置页红框写明"在 <网络> 上只读"和每条原因，切换器标"已拒绝"。链没回答、或 core 说暂时无法核对 →
  不算核对过，什么都不改，下次再问。从不静默。
- **签名闸门**（`gate.svelte.ts` `authorizeSigning`，在任何 passkey 之前；批准、提案（转账、改名、dApp 调用、取消）、执行、
  外部钱包批准都经过它）：先确保该网络本次会话核对过；被拒的网络 → 不签（`SigningRefusedError`），提案页只显示说明、
  不提供任何签名按钮，发送流程失败时由多签层弹出原因；高风险网络 → **每一次签名**都弹出确认（`RiskConfirmSheet`），
  逐条列出那条链上找到的模块 / guard / handler / singleton 及其地址，只有点"我了解风险，签名"（危险样式）才继续，
  "不签名"或关掉 = 取消，从不记住；没有界面能问时视同拒绝。核对不了 → 暂不签名（`SigningUncheckedError`，"请稍后再试"）。
  执行时的部署闸门照旧。

### 3.6 签名对等与状态差异守卫（2026-10-29，round 5）

- **原则**（owner，binding）：多签的每个签名者，得到的清晰签名必须和 Vela 账户**一样、一样好**；
  不能因为"别的成员会看"就放松。Bybit（2025，约 15 亿美元）：签名者信任了被篡改的 Safe{Wallet} 前端，
  盲签了一笔 delegatecall，把 Safe 的实现（槽 0）换掉。签名前，尽一切可能让签名者明白自己在签什么。
- **一份审阅，每个签名者**（`multisig_review::multisig_review`，wasm `multisigCall('multisigReview')`）。
  提案页（ProposalSheet）在**每个成员的设备上、每一种操作**都画同一份审阅：发起（发送流程、改名、成员变更
  现在都先打开审阅，"发起并批准"之后才要 passkey，`review-first.svelte.ts`；dApp 调用原本就先审阅）、批准、
  执行、拒绝、批量。块和签名页一样（同一个 `BlockList`、`requestBlocks`、`landedVerdict`、`TechDetails`/`RawData`、
  `AgentAdviceCard`），由 core 决定：
  - 解读与出处：转账、调用、批量现在都走签名页的 clear-signing 机器（`reviewedCalls`），成员变更、改名、作废是多签自己的解读；
  - **资产变动**：内层调用按 Safe 身份模拟（`eth_simulateV1`，from = Safe，和签名页给 Vela 账户的同一条路径，
    `rpcSimulateReply` → `sim_outcome::verdict`）；网络不提供时，用下面状态探针读到的 Safe 执行前后余额
    （原生币 + 交易涉及的代币，标明来源"由多签执行前后的余额读出"）；都没有就如实说"无法核对"；
  - **收款方风险**：每条腿的每个地址（收款、被调用的合约、新成员）——首次、是合约、**相似地址**
    （开头 4 位和结尾 4 位相同但不是同一地址，`looks_like`，对照联系人、本机账户、多签、成员）；
  - **AI 意见**：`adviceRequest` 是一份 `SignFacts`（id `multisig:<chain>:<safeTxHash>`，params 含 safeTx、
    safeTxHash、解读、状态守卫），`agent_review` 照常读、照常绑定摘要；standing 规则与签名页相同（当前 / 早期版本）；
  - **技术细节**：SafeTx 十个字段原样、EIP-712 typed data（`RawData` 高亮 JSON、原样复制）、
    **domain hash / message hash / safeTxHash** 各自复制——Vela 成员也有（像硬件钱包那样核对）；
  - **签名规则**（`SignRule`）：`allowed=false` → 不给任何签名按钮，没有"仍然"；`anyway` → 和签名页一样的摩擦：
    调用读不懂（`ClearSigningView.sign_anyway`）或有危险（执行会失败、相似地址）时，按钮变成危险样式的
    "仍然批准 / 仍然批准并执行 / 仍然执行 / 仍然发起并批准"；审阅没到之前按钮保持不可点（同签名页等模拟结果）。
- **状态差异守卫**（core，独立于任何解码）：在 Safe 当前状态上执行这笔交易，比较决定"谁控制多签"的状态——
  槽 0（singleton）、模块链表、guard 槽、fallback handler 槽、v1.5.0 module guard 槽、owner 链表与阈值、
  owner 数——**只允许**解码出的成员变更所声明的那一种变化（及其规则）。任何未声明的关键写入 →
  红色"这会改变谁控制这个多签"，列出每一条写入，**不签名、没有覆盖**（`Concern::StateChange`，`Tone::Refused`）。两个来源：
  1. **trace**：`debug_traceCall` + `prestateTracer`（`diffMode`）跟踪**真实的** `execTransaction`；签名用状态覆盖放进
     `approvedHashes[owner][safeTxHash]`（前 threshold 个成员按地址排序，`v=1`），nonce 覆盖为提案自己的编号
     （排队中的提案按它将来运行的样子跟踪）。能看到对 Safe 存储的**每一次**写入：命名不了的槽 → `Storage{slot}`，也是红色。
     跟踪到的调用若回滚，diff 是空的——**空 diff 绝不等于"没有变化"**：只有 Safe 至少被写了一次（nonce）才采信，否则听探针的。
  2. **probe**：`eth_call` 调 Safe 自己的 `simulateAndRevert(probe, …)`（StorageAccessible，v1.3.0–v1.5.0），
     `SafeStateProbe`（`rust/crates/vela-core/contracts/SafeStateProbe.sol`，`contracts/build.sh` 生成运行时代码）
     的代码由状态覆盖放到一个空地址——**不部署**。它在 Safe 的存储里读一次关键状态、像 `execTransaction`
     （`safeTxGas=0`）那样执行内层调用（CALL 或 DELEGATECALL）、再读一次，随回滚带回。它看到命名的状态，
     以及交易里出现的每个地址（任何偏移处的地址形字）的 `owners[a]` / `modules[a]` 项——不在链表里的签名权
     （`OwnerEntry`）也能抓到；它看不到任意槽，这点写在旁边。
  未部署的链：core 按已核对的 genesis（`counterfactual_start`）放置 setup 之后的状态（singleton 代码取自链上并核对代码哈希、
  owner 链表、阈值、模块哨兵、handler）。两个来源都没有 → 说"无法在 <网络> 上核对"，只剩解码规则（§5.2），标明如此。
- **各网络的来源**（2026-10-29 实测）：Arc Testnet（`rpc.testnet.arc.network`，`arc/v1`）：没有 `debug_traceCall`、
  `trace_call`、`eth_simulateV1`；`eth_call` 的状态覆盖可用 → **probe**（资产变动也由 probe 的余额读出；Vela 账户在 Arc 上
  的签名页只能说"无法核对"）。Gnosis 分叉（anvil 1.5.1）：`debug_traceCall` 的 prestate diff 和 `stateOverrides`、
  `eth_simulateV1` 都有 → **trace** + 模拟。web 两个都问（并行），core 选。
- **签名闸门移到 core**（`multisig_review::signing_gate`，`multisigCall('signingGate')`）：顺序是 读不到 → `unchecked`；
  genesis 被拒 → `refused`；这笔交易不签 → `blocked`（新：`SigningBlockedError`）；有风险 → `risk`（每次签名都问）；否则 `open`。
  `authorizeSigning(account, chainId, tx)` 在**每一次**签名前**重新读链**（`checkNow`，不用会话缓存）：谁能代表它
  （deployed check / 反事实规则）、它在该链现在的成员、以及这笔交易的状态差异——会话中途被加上的模块，下一次 passkey 之前就会被拦下。
- **整合（2026-10-30，round 5 三分支合并）**：
  - 审阅的每一次读（`eth_getCode`、trace、probe、`eth_simulateV1`、收款方风险）和预览一样有界（`REVIEW_READ_MS` 8 s，
    `bounded.ts`）：读不到就按"没有回答"交给 core（状态未核对、资产未模拟），审阅不会一直挂着按钮，闸门（同一条读路径）
    也不会把 passkey 挂住。
  - 执行：预览已经说明 passkey 之前就停（会失败、没有报价、**超时未核对**（`unchecked`）、执行者付不起）的，在闸门之前就停
    （`throwIfStopped`）——"批准并执行"退回只批准时，那次批准自己过一次闸门，风险确认不会问两次；签名页上批准的执行者
    （`approval_route` = 页面）在执行前先批准，闸门只问一次（`approveAuthorized`）。
  - 签名页（`app-web/trusted-signer`）的读法规则与 `multisig_review` 对拍：核心审阅拒的（`sign.allowed=false`，含 Bybit 状态差异向量），
    页面从字节上就拒（`samples/multisig-test.mjs`）；页面读不了链，链上才看得出的未声明写入由钱包在开页**之前**拦下。
- **已知限制**：模拟与真实执行可能不同（恶意代码可以识别 `eth_call`）；解码规则（delegatecall 只给 MultiSendCallOnly、
  不签对多签自身的未知调用）仍然是第一道门。作废（拒绝）不单独弹审阅：它是 0 值、空 data 的自调用，闸门照样核对其状态差异。
- **测试**：core `multisig_review_tests.rs`（Bybit 读起来是转账却写槽 0、嵌套调用里未声明的 addOwner、声明的成员变更通过、
  guard / 模块、回滚的 trace 不算干净、排队提案按自己的 nonce 跟踪、闸门顺序），`tests/multisig_review_vectors.rs`
  （分叉上 anvil 的真实回答：trace 和 probe 两种来源）；web `review-blocks.test.ts`（块、文字、对等：同一笔转账 / approve / 调用，
  Vela 账户的签名页和多签审阅画出同样的块种类）、`gate.test.ts`（core 规则、每次签名前重读）、`ProposalSheet.svelte.test.ts`
  （每种操作都画出守卫、资产、三个哈希、技术细节；Bybit 无按钮；"仍然"）。

## 4. 创建与登录

### 4.1 创建（不部署）

流程参考 Safe{Wallet} 的创建流程，只是最后一步从"部署"换成"写 genesis"。

1. **选成员**：本机 Vela 账户、联系人、任意地址。
   - **顺序就是用户排的顺序**，不排序。owner 顺序进 initializer，也就决定了地址；之后 `getOwners()` 也按这个顺序返回。
   - 拒绝重复地址、零地址和 `0x…01`（Safe 链表的哨兵值）。
2. **设阈值**：1 ≤ M ≤ N，推荐 M ≥ 2，并提示只有 M < N 才能容忍丢失成员。
3. **Vela 生成 genesis**，和 Safe{Wallet} 现在的默认一致：
   - Safe v1.4.1 factory，singleton 用 `Safe`；
   - setup hook 用 `SafeToL2Setup.setupToL2(SafeL2)`：在 L2 上自动切到 L2 版，同一套参数在所有链上得到同一个地址；
   - fallback handler 用 `CompatibilityFallbackHandler` 1.4.1；
   - `payment = 0`；`saltNonce` 随机；
   - **不启用模块**，多签账户不走 4337。

   这些都是 genesis 服务重放检查里判为 `ready` 的形状。
4. **确认页**：成员表（按顺序）、阈值、算出来的地址。创建者用 passkey 确认一次，这次签名同时用来申请代付（见下）。
5. **写入 registry**：调用 genesis 服务，由服务代付，把 `record(...)` 写到 Gnosis（索引链，服务的 `homeChainId`），
   不管创建者在确认页选的是哪个网络（那个网络只是它在本机的第一个网络，可以是服务不读的链，见 3.4）。
   Gnosis 是服务查询时首先读的地方，那里的 gas 每条记录不到 0.001 xDAI。
6. **确认收录之前**，genesis 留在本地 outbox，下次启动继续补交，做法同 issue 409 的公钥 outbox。
   确认之后，别的设备只凭地址就能加载它。
7. **生成邀请链接或二维码**，发给其他成员。

**服务的代付规则**（已在 safe-genesis-registry 实现，未提交）：未部署的 Safe，凭 owner 的 passkey 证明代付。

- **请求**：`POST /api/import`，带上：
  - `genesis`：多签账户的配置；
  - `ownerProof`：创建者的 Vela 账户地址、它自己的 genesis，以及一次 passkey 签名（WebAuthn assertion）。
- **签的内容**：`keccak256("safe-genesis-registry/sponsor/v1" ‖ 多签地址)`，作为 WebAuthn challenge。
  - 创建确认页上的那一次 passkey 就是这次签名，不额外弹窗。
  - 这个值不可能和 SafeOp、SafeMessage 的哈希撞上，所以签了它不会授权任何别的东西。
- **服务核对**：
  - 创建者在多签的 owner 列表里；
  - 创建者账户的 genesis 能算出它的地址，并且是只由 passkey 控制的钱包：重放检查读得懂、阈值 1、passkey 签名者在 owner 里；
  - 签名出自其中一把 passkey。

  核对完全自证，不查 p256-index，也不读链上状态。原先设想的"公钥必须在 p256-index 里"去掉了：
  注册 p256-index 同样免费，加上它挡不住刷量，反而多了一个依赖。
- **两条通道分开计数**：
  - 已部署的 Safe：每分钟 20 条，每天 500 条；
  - 凭证明的未部署 Safe：每分钟 10 条，每天 300 条，每个 owner 每天 5 条。

  生成 passkey 不要钱，所以证明本身挡不住批量造号；它的作用是把这类写入隔离出来，刷满了也不影响已部署 Safe 的导入。
  超出额度时，服务返回 `recordCall`，由创建者自己发；也可以由 outbox 第二天自动重试。
- **已验证**：
  - genesis-rules 测试：181 个真实 Vela 钱包都能取出 passkey 公钥；伪造签名、换签名用的钥匙、换 challenge 的拒绝路径都有覆盖。
  - Gnosis 分叉上，两个外壳都跑通了端到端：
    - 带证明的 5 条被代付写入，第 6、7 条被单 owner 限额挡下，陌生钥匙签名、没有证明、地址算不对的请求都被拒绝；
    - 写入后，Safe 仍未部署；按地址查回的 owner 顺序和阈值不变；
    - 用 `deployCallOf` 部署，得到同一地址、同样的成员。

**未部署的风险**：

- 只有选定参数的那个人，才有机会让两套配置撞到同一个地址，而且要做约 2^80 次计算的生日攻击。
  在 Vela 里这个人就是创建者自己。Safe{Wallet} 的"先算地址、后部署"也是同样的性质。
- 任何人都可以拿着公开的 genesis 去部署它，但部署出来的就是这套配置、这些 owner，没有危害。

### 4.2 第一次在某条链上转出（部署）

- 执行者的 UserOp 用一笔 MultiSend 完成两件事：`factory.createProxyWithNonce(...)`（即 `deployCallOf`），
  然后 `execTransaction(...)`。
- 部署前先看这条链的重放检查，不是 `ready` 就不执行。
- 未部署的 Safe 的 nonce 是 0，safeTxHash 照样可以先算出来，所以部署前就能收集批准。
- Safe Transaction Service 不接受未部署 Safe 的提案，所以未部署的多签账户只用 Vela 自己的提案信箱和定向链接（5.3.2 ②）。部署之后，信箱会把提案同步到 Safe 的队列。

### 4.3 登录（输入地址）

入口在添加账户里，与"登录 Vela 账户"并列，叫"添加多签账户"。地址可以手输、粘贴、扫码、从联系人选，或者打开邀请链接。

1. `GET /api/genesis/{safe}`：取 genesis 和记录它的链。Vela 创建的多签账户一般还没部署，有这条记录就够了。
2. **不信任服务**。core 通过 Vela 自己的 RPC 发一次 `eth_call registry.genesisOf(safe)`，核对返回内容和服务给的一致；
   再按 CREATE2 公式自己算一遍地址。registry 上的记录本身就是合约算过地址才写入的。
3. `GET /api/safe/{safe}/chains`：各链是否已部署；已部署的链给出当前成员、阈值和相对 genesis 的漂移，未部署的链给出重放结论。
   凡是要在上面签名的链，core 都会自己再读一遍 `getOwners` 和 `getThreshold`。
4. 用成员（未部署时就是 genesis 里的 owner）和本机 Vela 账户求交集：
   - 有交集：显示"本机成员：A、B"，账户可以签名；
   - 没有交集：账户只读。
5. 存成一行 `MultisigAccount`，切换为当前账户。它的网络按 3.4 的规则：已部署的链、信箱里有它提案的链、链接或本人选的网络；
   记录所在的注册表链不算。什么都说不出来时，问本人选一个网络。

**registry 里没有**（例如在 Safe{Wallet} 创建的 Safe）时：

- core 在已配置的链上用 `getCode` 找出它部署在哪条链，然后 `POST /api/import {chainId, safe}`（优先选服务读的那条；
  只部署在服务不读的链上时，服务还原不了，提示"未找到"）。
- 服务从交易服务或创建交易里还原 genesis。只有能算回这个地址的参数才会被接受，之后代付写入。
- Vela 轮询任务，完成后回到第 1 步。
- 结果是 `noCreate2`（老式 `createProxy`）时，可以加，但标"无法扩展到其他链"。
- 哪条链上都没有、registry 里也没有：无法加载，提示"未找到这个多签的配置"。
  Safe{Wallet} 的未部署 Safe 可以靠导入它的导出文件（`safeWalletProps`），放到后续版本。

## 5. 签名

### 5.1 一份批准是什么

- **每个 owner 签什么**：自己 Vela 账户域下的 `SafeMessage`，内容是多签账户的 safeTxHash。
  - 1.4.1 版多签账户验签调用旧接口 `isValidSignature(bytes, bytes)`。handler 对 66 字节的
    `0x1901‖domain‖structHash` 求 keccak，结果就是 safeTxHash，所以要签的是 `SafeMessage(safeTxHash)`。
  - **不能**复用 `compute_safe_message_hash` 现有的 `keccak(abi.encode(hash))` 写法
    （[user_op.rs:428](../rust/crates/vela-core/src/user_op.rs#L428)），复用会得到 GS024。
  - 这个差别同时也是一道隔离：dApp 让 Vela 账户签的任何东西，都不可能被当成批准。
- **新的签名入口只在 core 内部**。这份签名正是 `self_call_guard` 拒绝 dApp 索取的那种
  （[self_call_guard.rs:339](../rust/crates/vela-core/src/app/self_call_guard.rs#L339)），那条拒绝原样保留。
  批准只能从多签账户自己的流程进入，并且由 core 自己重算 safeTxHash、自己解码 SafeTx。
- **签名类型**：`SignatureKind` 新增 `SafeTxApproval`（[signing_venue.rs:401](../rust/crates/vela-core/src/signing_venue.rs#L401)）。
  它走的是**签名 owner 自己的**签名地点：Vela 签名页或 Trusted Signer 页。Trusted Signer 页要新增这种模式。
- **拼装**：
  - 静态区按 owner 地址严格升序，每段 65 字节。
  - 1271 签名的格式是 `r = owner 地址, s = 偏移, v = 0`；动态区放 `len ‖ build_eip1271_signature 的输出`。
  - 执行者填 `v = 1`（它就是 `msg.sender`），所以凑够 M 只需要 M−1 份批准，加上执行者执行时的那一次确认。
  - EOA 成员的签名是 65 字节普通 ECDSA。
- **owner 账户必须已在该链部署**，否则 1271 校验报 GS024。执行者可以在同一笔里替它部署
  （setup 数据能从 p256-index registry 里的公钥重建），也可以让该成员改用 `approveHash`。

### 5.2 签名确认页

> **与代码核对（R23，2026-11-09，提交 `fa6083c87`）。以代码为准。** 本节是设计稿；下面几处与代码不符，
> 安全上的准确说法见 [`docs/SECURITY-MODEL.md`](SECURITY-MODEL.md) §9。
>
> - **"安全形状"规则（operation、gas/refund 字段、对多签自身只允许成员管理和作废）照此实现**：
>   `multisig::review`、`read_batch`、`read_owner_management`；delegatecall 只认 v1.4.1 的
>   MultiSendCallOnly（`MULTI_SEND_CALL_ONLY`），v1.3.0 的会被拒绝。另外允许一种本节没写的形状：
>   改名（`Action::Rename`，调用注册表合约，不是自调用）。
> - **"账户体检……就停止在这条链上签名"与代码不符。** 读到模块、guard、未知 singleton 或未知 fallback
>   handler 时，`signing_gate` 给出 `Gate::Risk`，`authorize_signing` 在**每一次**签名前弹出风险确认
>   （`MultisigOperation::AskRisk`），成员确认后仍然可以签。只有这条链读不到（`Unchecked`）、或未部署的多签
>   genesis 被拒（`Gate::Refused`）时才停止签名。它列在 SECURITY-MODEL §7 的"… anyway"清单里。
> - **genesis 服务的 `authorityChanged` 提示没有实现**；体检完全由 core 自己读链（`deployed_check`，R22 起
>   两个独立来源核对）。
> - **`KeyRoute` 限定 `allowCredentials`**：只用在 Trusted Signer 页这条路上；Vela 自己的签名页把该账户的
>   全部凭据交给 passkey，再由 core 核对签出的凭据属于这个 owner（`approval_from_assertion`）。
> - **执行前会显示手续费**，并且只按显示的那笔签（web `execution-fee.ts` 的 `ExecutionQuote`；core `Event::Execute.shown_fee`）。
>   上面"实现状态"里"执行前不单独展示金额"一句已过时；"多签账户暂不连接 dApp"一句也已过时
>   （`multisig_dapp`，见 [`docs/multisig-machines.md`](multisig-machines.md) §31、§35）。

当前账户是多签账户时，转账确认页、dApp 交易页、成员管理页都会出现同一块"批准"区域：

```text
多签 · 2/3                         已批准 0 / 需要 2
以 [头像] 张三的钱包 批准           ← 本机 owner；有多个时可选
[批准]        或     [批准并执行]   ← 本机能凑够阈值时直接执行
```

- **用哪个账户签**：本机 owner 中还没批准的那些。只有一个就直接用它；有多个就默认第一个，可以改选。
  `KeyRoute` 把 `allowCredentials` 限定在这个账户的凭据上，机制上签不错账户。
- **本机凑得够阈值**（例如 2/3 中有两个 owner 都在本机）：可以依次批准，最后一个点"批准并执行"。
  每个 owner 各弹一次 passkey。
- **执行**：执行者的 Vela 账户发一笔普通 UserOp 调用 `execTransaction(...)`。手续费由执行者按现有方式付；
  交易跟踪的 sender 是执行者，记录的 `from` 是多签账户。
  `safeTxGas = 0` 且 `gasPrice = 0` 时，内层调用失败会让整笔交易回滚，所以失败会像普通交易失败一样显示出来。
- **只签安全形状**，适用于所有入口：
  - `operation = 0`，唯一的例外是 delegatecall 到已知的 MultiSendCallOnly，并且其中每条腿都是 `operation = 0`；
  - `safeTxGas = baseGas = gasPrice = 0`，`gasToken = refundReceiver = 0`。`gasPrice > 0` 会按 `refundReceiver`
    从多签里"退 gas"，是一条掏空多签的旁路；
  - 发给多签自己的调用，只允许成员管理的四个函数，以及作废（0 值、空 data）。
    `enableModule`、`setGuard`、`setFallbackHandler` 一律不签。
  - 别处（例如 Safe{Wallet}）提出、不在这个范围内的交易：照常解码显示，标明"Vela 不签这类交易"，既不签也不执行。
- **账户体检**：某条链上出现模块、guard、未知的 singleton 或未知的 fallback handler，就停止在这条链上签名，
  并告诉用户原因（模块可以绕过所有 owner）。genesis 服务的 drift 字段 `authorityChanged` 给出提示，core 自己读链确认。
- **独立核对**：passkey 弹窗只显示域名，多签的安全来自每个成员各自核对内容。

### 5.3 让其他成员签名、凑齐阈值

目标：让"凑齐签名"像"把一条消息转给对方，对方点一下"一样简单，同时不降低安全性。先看 Safe 官方怎么做，再说 Vela 改进什么。

#### 5.3.1 Safe 官方的做法（2026-10 查证）

| 环节 | Safe 的做法 | 问题 |
| --- | --- | --- |
| 提案存放 | Safe Transaction Service：中心化的链下队列，按 Safe 地址列出待签交易。官方托管的 API 现在要 API key，不带 key 限每秒 2 次、每月 5000 次 | 第三方钱包直接调用要内置 API key；不支持尚未部署的 Safe；只覆盖它部署了的链 |
| 发现待签 | 在 Safe{Wallet} 的交易队列里看。网页推送需要对每个 Safe 签名证明 owner 身份，并打开浏览器和系统的通知权限。Safe{Mobile}（2025-10）有推送，靠 owner 签名授权的一把"delegate key" | 不开 App 就不知道。推送设置繁琐，每个 Safe、每台设备都要设一次。没有"请某个人签这一笔"的直达链接 |
| 签名 | 连接钱包，在队列里点 Confirm，签 safeTxHash（EIP-712）。硬件钱包常常只能看到哈希，等于盲签 | 2025 年 2 月 Bybit 事件：Safe{Wallet} 的前端被篡改，签名人在硬件钱包上盲签了一笔改 singleton 的 delegatecall。Safe 之后建议签名人用独立工具核对哈希 |
| 执行 | 凑够后任一 owner 点 Execute，自己付原生币 gas。最后一个签名的人可以勾选"签名并执行"。部分链有 Gelato 代付，每天 5 笔 | 执行者手上要有这条链的原生币 |
| 拒绝 | 在同一 nonce 上发一笔空交易，同样要凑够阈值，还要付 gas。签名不能撤回。发起人可以把提案从链下队列删掉，但这只是隐藏 | 拒绝和批准一样费事。签名一旦给出，只有占掉 nonce 才算真正作废 |
| 提议人 | Proposers / delegates：非 owner 可以发起交易，但不能签名 | — |
| 排队 | nonce 严格递增；同 nonce 的交易互相冲突，先执行的生效 | 一笔卡住的提案会堵住后面所有交易 |

来源：[交易服务 API key](https://docs.safe.global/sdk/api-kit)、[网页推送](https://help.safe.global/articles/9750082418-how-to-start-receiving-web-push-notifications-in-the-web-wallet)、
[delegate key](https://help.safe.global/en/articles/40799-what-is-a-delegate-key)、[Safe{Mobile}](https://safe.global/blog/secure-signing-now-seamlessly-mobile-safe-labs-introduces-an-all-new-safe-mobile-app)、
[拒绝与删除](https://help.safe.global/articles/3608612277-reject-and-delete-transactions)、[同 nonce 冲突](https://help.safe.global/en/articles/40839-why-are-transactions-with-the-same-nonce-conflicting-with-each-other)、
[Proposers](https://help.safe.global/en/articles/235770-proposers)、[代付](https://help.safe.global/articles/6461820093-gas-fee-sponsorship-on-safe-transactions)。

#### 5.3.2 Vela 的改进

> **与代码核对（R23，2026-11-09，提交 `fa6083c87`）。以代码为准。** 本节是设计稿，下面的①–⑨只有一部分实现了：
>
> | 条目 | 代码里的实际情况 |
> | --- | --- |
> | ① 链接"自带全部内容" | **不符。** 链接 `#ms=<chainId>.<多签>.<safeTxHash>` 只指明是哪一笔（`multisig_link::proposal_link`），内容从信箱取，再由 core 重算核对。每笔一个链接，没有按人生成的二维码；外部成员拿到的是 connect 页链接，不是 Safe{Wallet} 链接。 |
> | ① 打开链接自动添加多签 | 已实现（`multisig_accounts_link`）；本机没有成员时只读预览，不添加。 |
> | ② "没有任何有效 owner 批准的提案不收" | **不符。** 信箱收任何人的提案（`mailbox-any-signer`，服务端 R22 另加 `nonceTooFar` 上限）。钱包侧：没有成员批准的提案标为 `from_non_member`，不进默认列表、不进任何计数、不参与 `next_nonce_of`，只在"显示非成员的提案"之后带警告出现（`docs/security/hardening.md` §10）。 |
> | ② 同步到 Safe 交易服务 | **没有实现。** |
> | ② 信箱不是信任来源 | 已实现：`read_proposal` 重算 safeTxHash、重新解读、逐份核对批准、只计当前成员。 |
> | ② 公开性要在界面写明 | **没有实现**（只有非成员提案的警告）。信箱无需认证、CORS `*`，见 SECURITY-MODEL §11。 |
> | ③ 订阅与推送 | **没有实现**；App 打开时每 60 秒拉取信箱（`multisig_presence`）。 |
> | ④ "不存在盲签" | **不符。** 多签审阅有"… anyway"：状态差异或模拟有一项读不到、或交易本身是盲签，确认就变成"… anyway"，需按住 1.5 秒（`HOLD_TO_CONFIRM_MS`）；只有打开"拒绝盲签"（默认关闭）才会拒绝。成立的是：审阅还没读出来（加载中 / 失败）时一律不能签（§12 的 review door，`door_with`）。 |
> | ④ 显示 safeTxHash 缩写 | 已实现，完整哈希在详情里。 |
> | ⑤ 一次批准多笔（批量 `approveHash`） | **没有实现**；只有外部钱包单笔上链批准。 |
> | ⑥ 批准并执行、用代币付费 | 已实现。 |
> | ⑦ 反对票、"已无法通过"、建议立即作废、软过期 | **都没有实现**。"拒绝" = 发起同 nonce 的作废提案。 |
> | ⑦ "下一笔提案自动用被放弃的 nonce" | **不符。** `next_nonce_of` 把新提案排在所有有成员批准的未完成提案之后，被放弃的提案继续占着它的 nonce，要靠作废提案释放。 |
> | ⑧ 按 nonce 排队 | 已实现；"替换它"没有实现（新草稿总是取下一个空闲 nonce）。 |
> | ⑨ 提议人 | **没有实现。** |

**① 定向的签名请求：一条链接直达"请李四批准"**

- 发起后，确认页列出还需要谁，按人生成请求：

  ```text
  还差 1 份批准（李四、王五任意一位即可）
  李四   [分享链接]  [二维码]
  王五   [分享链接]  [二维码]
  0x9f…（外部成员）  [Safe{Wallet} 链接]
  ```

- **链接自带全部内容**（SafeTx 原文和已收集的批准），放在 `#` 后面，不经过服务器。链接里还带了"请谁签"，
  但这只是提示：打开后，core 自己用 owner 和本机账户求交集，选出用哪个账户签，不信链接里写的人名。
- **打开链接直接到批准页**：本机还没加这个多签账户，就自动走 4.3（地址就在链接里），加好以后进入批准页；
  当前账户不是这个多签也没关系，不需要先切换。
- **当面**：二维码给对方扫一下就行。
- Safe 的做法是让对方自己去队列里找；Vela 让对方点开就能签。

**② 提案信箱：自己的服务做队列，同步到 Safe 的队列**

- **为什么不直接用 Safe Transaction Service**：
  - 官方 API 要 API key，客户端内置 key 不合适；
  - 它不接受尚未部署的 Safe，而 Vela 的多签账户默认就是未部署的（4.1）；
  - 它只覆盖它部署了的链。
- **建议**：在 genesis 服务里加一个**提案信箱**，和 registry 共用部署、链访问和告警。
  - **内容**：按 `(chainId, 多签地址, safeTxHash)` 存 SafeTx 原文和已收集的批准；按多签地址读取。
  - **写入要核对**：
    - SafeTx 确实算出这个 safeTxHash；
    - 每份批准确实出自一个 owner：已部署的多签用 `eth_call checkNSignatures` 核对；
      未部署的多签，owner 是 passkey 钱包时，服务直接验 WebAuthn 签名，复用 4.1 已经实现的 owner 证明逻辑。
    - 没有任何有效 owner 批准的提案不收，防止刷垃圾。
  - **同步到 Safe**：多签已部署、并且这条链有交易服务时，信箱用服务端自己的 API key 把提案和批准同步过去，
    Safe{Wallet} 里的外部成员能看到同一笔；同时也把那边的新批准读回来。
- **信箱不是信任来源**：设备从信箱读到的一切，都由 core 重新计算哈希、重新解码、重新核对签名。
  信箱被篡改，最多导致提案丢失或者无法显示，不会让人签错。
- **公开性**：和 Safe 的队列一样，知道多签地址的人都能读到它的待签提案。界面里要写明这一点。

**③ 通知：学 Safe 的"先证明是 owner"，但只做一次**

- **订阅**：设备订阅某个多签账户的通知时，用本机 owner 账户的 passkey 签一次订阅请求。
  信箱用 4.1 的 owner 证明逻辑核对，确认这台设备背后确实是 owner。
  - 加多签账户时顺带完成，不需要单独设置；
  - 这台设备上的多个 owner 账户共用一次订阅。
- **通道**：App 用 APNs / FCM，网页和扩展用 Web Push。
- **通知内容只有提示**："多签 0x1234…abcd 有 1 笔待你批准"，加上提案的标识。
  交易内容不放进推送，打开后由设备自己拉取、解码。
- **事件**：
  - 有新提案、需要你批准；
  - 有人批准了（可以关掉这一项）；
  - 凑够阈值、可以执行；
  - 已执行；
  - 已被拒绝或取代。
- **还没有推送时**（第一阶段）：App 打开时拉取信箱，账户切换器显示"2 笔待批准"，主要靠 ① 的分享链接。

**④ 每个成员的设备独立核对内容（吸取 Bybit 的教训）**

- 批准页只显示 core 自己解码出的内容和模拟结果，不显示链接或信箱附带的任何说明。
- 只签安全形状（5.2）：delegatecall 一律不签（MultiSendCallOnly 除外），改 singleton、模块、guard、fallback handler 的调用一律不签。
  Bybit 那一笔，在 Vela 里根本到不了"批准"按钮。
- 不存在盲签：passkey 弹窗不显示内容，所以内容一定在 Vela 的签名页、或独立的 Trusted Signer 页上看完再签。
  页面上显示 safeTxHash 的缩写，方便成员之间互相核对。
- 建议成员分散使用 Vela 签名页和 Trusted Signer 页，这样一个界面被攻破，不会让所有人一起签错。

**⑤ 一次确认，批准多笔（可选，链上批准）**

- 默认是链下批准：签 `SafeMessage(safeTxHash)`，不花钱。
- 当一个成员同时有好几笔待批准，或者他的 Vela 账户在这条链上还没部署（这时链下的嵌套 1271 签名会报 GS024，见 5.1），
  就提供"一次批准全部"：用一笔 UserOp 对每一笔调用多签的 `approveHash(safeTxHash)`。
  - 只要一次 passkey；
  - 需要时顺带部署他的账户；
  - 手续费照 Vela 现有方式付，可以用代币付。
- 链上批准还有一个好处：别的设备不靠信箱也能看到（`ApproveHash` 事件）。
- 限制：只适用于已部署的多签账户。

**⑥ 执行不卡在 gas**

- 最后一个批准的人，默认就是"批准并执行"。Safe 是让人勾选，Vela 把它设成默认。
- 执行者用自己的 Vela 账户发 UserOp。Vela 账户本来就支持用代币付手续费，不需要这条链的原生币。
  Safe 的做法要求原生币，或者走每天 5 笔的代付。
- 凑够了但最后一个人当时没有执行，任何成员打开时都会看到"可以执行"，点一下就行。

**⑦ 拒绝更轻，作废尽量不额外花钱**

- **反对票**：成员可以在信箱里投"反对"，不花钱，也可以改主意。页面显示"批准 1/2 · 反对 1"。
- **自动判定"已无法通过"**：反对的人数多于 N − M 时，剩下的人都批准也凑不够 M，提案标为"已无法通过"。
  但已经收集到的签名在 nonce 被占用之前仍然有效，界面要写清楚这一点。
- **作废时机**：
  - 不需要专门作废：下一笔提案自动用这个被放弃的 nonce，它执行时，旧提案就同时失效了，省下一笔作废交易；
  - 需要马上作废：被放弃的提案已经凑够阈值，任何人随时都能执行它。这时 Vela 提示"建议立即作废"，
    发起同 nonce 的作废交易，作废交易本身也要凑够 M 份批准。
- **软过期**：提案带一个 Vela 侧的有效期，默认 7 天。过期后 Vela 不再签、不再执行它，并建议用新提案替换。
  这只约束 Vela，拿着足够签名的人照样能在链上执行，所以界面上明确叫"Vela 内有效期"。

**⑧ 排队一目了然**

- 待批准列表按 nonce 排列：第一笔是"当前"，后面的是"排队中"。
- 前面一笔卡住时，给出两个选项："替换它"（同 nonce 发新提案）或"作废它"。

**⑨ 提议人（第二阶段）**

- 和 Safe 的 Proposers 一样：允许非成员发起、不能批准。
  适用场景：财务同事、clientmcp 代理（spec 107 E3：代理只发起，批准永远是本人点的）。
- 提议人由一个 owner 授权（passkey 签名），授权记录在信箱里；信箱只收已授权提议人的提案，或者带有效 owner 批准的提案。

#### 5.3.3 一次完整的协作（2/3：张三、李四、王五）

1. 张三在 Explore 里向某个协议存 500 USDC。签名页解码、模拟、检查都通过后，张三批准，这是第 1 票。
   网站的请求挂着（第 6 节）。
2. 签名页显示"还差 1 份（李四或王五）"。张三把链接发到群里。信箱收下提案，并给李四和王五的设备推送通知。
3. 李四在手机上点开通知或链接，直接进入批准页（他的手机上已经加过这个多签账户）。
   批准页显示 core 解码的内容、模拟结果，以及"张三已批准（核对通过）"。
4. 李四点"批准并执行"：一次 passkey，他的 Vela 账户发 UserOp 执行，手续费用 USDC 付。
5. 张三的设备从链上看到执行结果，把交易哈希回复给网站；王五的通知变成"已执行"；所有人的活动里多出一条已完成记录。

#### 5.3.4 分阶段

| 阶段 | 内容 |
| --- | --- |
| 一 | ① 定向链接与二维码；② 提案信箱（含同步到 Safe 的队列）；③ 中的"打开 App 拉取"；④ 独立核对；⑥ 默认批准并执行；⑦ 反对票、"已无法通过"、自动沿用被放弃的 nonce、软过期；⑧ 排队 |
| 二 | ③ 推送；⑤ 一次批准多笔；⑨ 提议人 |

信箱放进第一阶段的原因：没有它，未部署的多签账户只能靠链接传递；而直接调用 Safe 的交易服务，客户端又要内置 API key。

## 6. Explore（dApp）

多签账户可以像 Vela 账户一样连接网站。网站看到的是多签地址。可用的链是：已部署且可签名的链，
加上重放结论为 `ready`、第一笔交易会顺带部署的未部署链。

**决定**（owner，2026-10-27）：网站发来的交易**一直挂着**，直到多签执行完、或者被拒绝，才回复网站。

| 请求 | 行为 |
| --- | --- |
| `eth_accounts`、读链 | 照常 |
| `eth_sendTransaction`、`wallet_sendCalls` | 变成提案，请求挂着，直到有结果（见下）。先过同一套安全形状检查，不通过就当场拒绝 |
| `personal_sign`、`eth_signTypedData` | 第一版拒绝并说明原因。以后可以用同一套"挂着、收集批准"的机制做 Safe 的离线消息签名 |
| 要求签 `SafeTx` 的类型数据 | 拒绝，规则不变 |

**一笔 dApp 交易的生命周期**：

1. 网站发来交易 → Vela 的签名页解码、模拟、检查安全形状。
2. 发起人用本机 owner 批准。本机就能凑够阈值时，直接执行，和普通账户一样拿到交易哈希后回复网站。
3. 凑不够时，签名页变成等待状态：

   ```text
   等待其他成员批准 · 1 / 2          [请其他成员批准]  [收起]
   这个网站会一直等，直到交易执行或被拒绝。
   ```

   - "请其他成员批准"打开 5.3.2 ① 的定向请求卡片。
   - "收起"把签名页收成一个常驻的"等待中"提示条。网站的请求仍然挂着，用户可以继续浏览、切换账户，
     也可以处理别的请求。同一网站再发来的交易排在下一个 nonce。
4. 其他成员在自己的设备上批准、执行（5.3）。
5. **发起人的设备看到结果，回复网站**：
   - 已执行：回复执行那笔交易的哈希（和普通账户一样，是真实上链的交易哈希，不是 safeTxHash）。
     执行者可以是任何一个成员，发起人的设备从链上的 `ExecutionSuccess` 找到它。
   - 已拒绝（作废交易先执行了）：回复 EIP-1193 的 4001（用户拒绝）。
   - 执行了但失败（回滚）：按现有的"已上链但失败"回复。

**为什么能挂着**：EIP-1193 本身没有超时，网站也一直显示"请在钱包中确认"。限制来自 Vela 自己的
传输通道，多签请求要分别处理：

- **Vela 内置浏览器**：标签页还在，请求就一直挂着。现有的等待上限（`DAPP_TX_ANSWER_WINDOW_MS` 120 秒、
  `PAGE_WAIT_CAP_MS` 10 分钟）对多签提案不适用，要单独放开。
- **浏览器扩展**：扩展页自己有 300 秒的上限，也要为多签请求放开，或者改成扩展保留请求、到时回复。
- **远程连接**（WalletPair / remote inject）：传输层自己可能有过期时间；过期了，请求就答不上了，但提案不受影响。

**网站等不到的情况**：标签页关了、App 重启了、扩展或远程连接断了。这时提案照常进行，执行后照常生效。
网站等不到回复，下次读链时会看到结果。签名页的等待状态和"待批准"记录里要写清楚这一点，避免用户以为交易没发出去而重复发起。

## 7. 初始配置、跨链与成员变更

| | 初始配置（genesis） | 当前配置 |
| --- | --- | --- |
| 内容 | 首批 owners、阈值、saltNonce、singleton、factory、setup 的 to/data、fallback handler | 某条链上此刻的 owners 和阈值 |
| 决定什么 | **地址**（CREATE2 只认它） | **谁能动钱** |
| 会不会变 | 永远不变 | 每条链各自变化 |
| 存在哪 | `SafeGenesisRegistry`（链上） | 各条链上的 Safe 本身 |

- **还没有任何链改过成员**：所有链都等价，在任何一条链上第一次转出时顺带部署（4.2）。不需要单独的"扩展"操作。
- **某条链上改过成员之后**：其他未部署的链，部署出来的控制权仍属于**初始成员**。所以在这些链上：
  - 先看 `GET /api/replay/{safe}?chainId=` 的结论，不是 `ready` 就不执行；
  - 部署之后紧接着同步成当前成员，同步交易要由初始成员在这条链上凑够阈值来签；
  - 同步完成前，不展示收款地址。
- **成员变更**：成员变更页面并排显示变更前后的成员表，对每条已部署的链分别发起，并显示同步进度。
  Safe 没有跨链签名，每条链要单独签一笔。
- **成员和其他已部署链不一致的链**：只读，并标出差异（倾向如此，待确认）。

## 8. SafeGenesisRegistry 的现状

- **合约 v4** `0xF412388bF908E510066d3Be5c704bd9A327f945d`（2026-10-29）：经确定性部署器 CREATE2 部署，所有链地址相同；
  每条记录带名字，可以按写入顺序枚举（`totalRecords`、`safeAt`、`genesisPage`），Safe 可以自己改名（`rename`）。
  - 已部署在 Gnosis（服务代付写入）和 Ethereum（用户自己付费写入），另有 Arc Testnet (5042002)。它只是索引（3.4）：
    多签能在哪些链上，和注册表、服务读哪些链都无关。
  - 服务只读 v4（按运行时代码哈希核对，读到别的版本回 502，不会说"没有"）。v1（Gnosis 和 Arc Testnet，`0xdA88…fb89`，0 条记录）、
    v2（Arc Testnet，`0x70b3…4802`）、v3（Arc Testnet，`0x535f…801E`）都已退役：旧版里的多签服务不再认识，钱包标"尚未共享"并可"重新共享"
    （本机的 genesis 不受影响，地址照样推导、照样能用；只是别的设备按地址找不到，直到重新共享）。这些都只在测试网和本地分叉上出现过。
- **以前的合约** `0xdA88357C2eB85e7e0515B1122a28befd6Dc8fb89`（v1）：
  - 曾部署在 Gnosis (100) 和 Arc Testnet (5042002)。
  - Gnosis 上已在 gnosisscan（Blockscout）和 Sourcify 验证源码。
  - 接口：`record(factory, proxyCreationCode, singleton, initializer, saltNonce)`、`genesisOf(safe)`、`deployCallOf(safe)`。
    每个 Safe 一条记录，写一次，谁都能写；没有 owner、管理员和升级入口。
- **服务**：`genesis-cf`（Cloudflare Worker）和 `genesis-server`（docker）两个外壳，API 相同。
  提供跨链查 genesis、各链状态与漂移、重放检查、代付导入、缓存和告警。**尚未上线**：需要 `wrangler login` 和一把由我们自己生成的代付私钥。
- **已验证**：
  - 数百个真实 Safe（v1.1.1–v1.5.0）、184 个 Vela 钱包、Lido、Bybit、ENS、SafeDAO 等知名多签，都能正确算出地址；
  - 在另一条链的分叉上，只用 `genesisOf` 的结果就部署出同一地址、同样成员的 Safe，约 700 次重放，没有意外失败；
  - 三方对抗审查的发现已全部修复（`docs/review-2026-10-27.md`）。
- **建议 Vela 把自己账户的 genesis 也写进 registry**。Vela 账户本身就是 Safe，它作为多签 owner 时，
  有 genesis 绑定的 owner 在重放检查里直接可信；否则扩展到新链时会被判为 `unverified`。
- 真实数据里见到过成员漂移：Lido 的一个 3/6 多签自创建以来换了 3 个成员。按初始配置在新链重放，
  控制权会回到那 3 个旧成员手里。这就是第 7 节要求"扩展时同时同步成员"的原因。

## 9. Vela 侧改动面

- **vela-core**：
  - 账户模型：`MultisigAccount`；会话里 `AccountRow` 的两类行；当前账户按地址记录；独立的存储键和对应的 outbox。
  - 创建（不部署，只写 genesis）与登录两个新流程；首次转出时部署加执行的 MultiSend。genesis 服务客户端，沿用 `registry_lookup` 的 step 模式（`IndexGet` 加 `EthCall`）。
    `NetServiceEndpoints` 增加 genesis 服务地址。
  - 签名：SafeTx 的 EIP-712 哈希；`SafeMessage(safeTxHash)`（旧接口路径）；签名拼装（排序、偏移、嵌套 1271、
    执行者 `v = 1`）；安全形状检查；账户体检；`SignatureKind::SafeTxApproval`。
  - 发送：`send` 在多签账户下的分支：发起提案、收集批准、执行。执行用的 UserOp 调用 `execTransaction`，
    可选替成员部署账户。tracker 的 sender 改为执行者。
  - 活动："待批准"分组、批准与反对两个进度、排队状态、Vela 内有效期；信箱客户端；`checkNSignatures` 核对；定向链接的编解码；本机提案存储；链上 `approveHash` 批量批准（第二阶段）。
  - dApp：`dapp_rpc` 和 `sign_request` 在多签账户下的分支（第 6 节）。
- **genesis 服务**（safe-genesis-registry 仓库）：未部署的 Safe 凭 owner 的 passkey 证明代付，两条通道和单 owner 限额（4.1，已实现，未提交）。
- **提案信箱与通知**（genesis 服务）：存放与核对提案和批准、同步到 Safe 的交易服务、反对票、推送订阅（5.3.2 ②③）。
- **dApp 等待**：多签提案的 dApp 请求不受 `DAPP_TX_ANSWER_WINDOW_MS`、`PAGE_WAIT_CAP_MS` 和扩展页 300 秒上限约束；签名页的等待状态与收起后的提示条（第 6 节）。
- **四端外壳**：存储键；账户切换器与添加多签账户入口；签名页的批准区域；成员管理页；Trusted Signer 页的批准模式。
- **不改**：Vela 账户本身、p256-index registry、恢复流程、`self_call_guard` 现有的拒绝规则。

## 10. 未决与待验证

- [ ] 分叉链上验证嵌套 1271 全链路：多签 → Vela 账户 fallback（Safe4337Module）→ passkey 验签；
      把 `SafeMessage(safeTxHash)` 钉成测试向量。
- [ ] Safe Transaction Service 是否接受嵌套 1271 的确认（信箱同步时需要），以及我们服务端用的 API key。
- [ ] 提案信箱放进 genesis 服务，还是单独一个服务（倾向前者）。
- [ ] 推送：APNs / FCM / Web Push 的证书和发送端放在哪；通知事件的默认开关。
- [ ] Vela 内有效期的默认值（暂定 7 天）。
- [ ] 成员和其他已部署链不一致的链：只读，还是允许操作但加警告（倾向只读）。
- [ ] 每个成员的嵌套签名验证要多花多少 gas，需要实测。
- [ ] genesis 服务上线：Worker 部署和代付私钥。
- [ ] 扩展页 300 秒上限和远程连接的过期时间，对挂着的多签请求实际怎么处理，需要在各端实测。
- [ ] Vela 侧生成 `ownerProof`：创建确认页的 passkey 签 4.1 的 challenge，并带上创建者账户自己的 genesis。
- [ ] Vela 账户创建时是否顺带把自己的 genesis 写进 registry（第 8 节）。
- [x] 服务不读的链上改过成员的多签：owner 决定（2026-10-29，Q1）信箱只存不判，接受任何有效签名，钱包只计当前成员（3.4）。
- [x] 改名会让多签部署到 Gnosis：owner 决定（Q2）改名从不部署，只在已部署的注册表链上提供（3.3、3.4）。
- [x] 未部署多签由谁控制：genesis 只核对了地址，`setup` 里的 hook、模块、fallback handler、付款都没人看，
      恶意服务给出的 genesis 能把多签交给模块（r4-e S1）。已由 core 的反事实规则和链上状态核对解决（3.5），
      添加、链接、导入、执行四处都经过它。

## 11. 导入 Safe{Wallet} 多签与外部 owner 签名（2026-10-28 设计）

**目的**：让现有 Safe 用户有第二个多签应用和界面可选。他们的 owner 大多是 MetaMask、Rabby 等外部钱包（EOA），不是 Vela account。

### 11.1 导入

- **入口**：添加多签 →「从 Safe{Wallet} 导入」，选择 Safe 官网导出的 JSON（设置 → 数据 → 导出）。
- **文件格式**（version 3.0）：
  - `data.addedSafes[chainId][address] = {owners:[{value}], threshold}`：已部署的 Safe；
  - `data.undeployedSafes[chainId][address].props`：未部署的 Safe 的完整创建参数
    （`factoryAddress, masterCopy, safeAccountConfig, saltNonce, safeVersion`）；
  - `data.addressBook[chainId][address] = name`：用户起的名字。
- **解析在 core**：按地址合并各链，列出每个 Safe 的链、阈值、成员、版本线索、地址簿名字。文件只在本机解析，不上传。
- **用户勾选**要导入的 Safe，然后逐个确认 genesis：
  - genesis 服务已有记录 → 直接用；
  - 没有 → `POST /api/import`：已部署的给 `chainId`（服务从交易服务或创建交易里还原），未部署的附 `safeWalletProps`。
    服务按地址校验后用 owner proof 车道之外的「已部署」车道代付记录；
  - 还原不出来（例如 v1.1.1 以前的 `createProxy`，地址不能跨链）→ 仍可导入，但只在已部署的链上可用，标明原因。
- **导入后**：和 Vela 创建的多签一样出现在切换器，有资产、活动、发送、提案。名字取地址簿（本机）。
- **支持的版本**：Safe v1.3.0、v1.4.1、v1.5.0（L1 和 L2）。更早的版本 EIP-712 域里没有 chainId，签名格式不同，先不支持，导入时说明。

### 11.2 外部 owner 签名（PeerConnect + WalletPair）

参考 sub3 的实现：Vela 是 WalletPair 的「dApp 端」，外部签名页是「钱包端」。

- **通道**：PeerConnect 房间（Noise IKpsk1 端到端加密，中继看不到内容也不存内容）。中继默认 `wss://relay.peerconnect.dev`，可配置。
- **中继 token**：公共中继只接受带 token 的连接，两端各带自己的（token 不在邀请里）。设备第一次需要时，用本地生成的随机设备 ID 向 `getvela.app/api/peerconnect-token` 申请；站点用 App Secret 调 PeerConnect 的 `POST /api/apps/<app_id>/tokens`（用户 ID 为 `web:<设备 ID>`），只回传 token、过期时间和中继地址。token 只能用来连中继（房间内容端到端加密），所以接口不要求登录；每个新设备 ID 占用 App 的用户名额（默认 10,000），接口按 IP 限速。中继以 4403 拒绝时，客户端丢弃缓存的 token 重新申请；站点不可用时退回公开试用 token。自建中继不需要 token。
- **签名页**：Vela 自己的 `/[locale]/connect`（同源、严格 CSP）。用户在装了钱包扩展的浏览器（或手机钱包的 dApp 浏览器）里打开链接或扫码，
  页面用 EIP-6963 发现钱包（没有就用 `window.ethereum`），用户确认连接。
- **签什么**：`eth_signTypedData_v4`，内容是 SafeTx 的 EIP-712 数据（域 `{chainId, verifyingContract: safe}`）。
  签名页先切到 Safe 所在的链（`wallet_switchEthereumChain`），钱包里显示的是可读的 SafeTx。
- **Vela 端核对**：回来的签名先在 core 里 ecrecover，必须等于该 owner 地址、且对应这笔 safeTxHash，才提交给邮箱。
  外部钱包连上来的账户不是该 Safe 的 owner 时，直接说明，不发签名请求。
- **邮箱**：approval 增加一种 `ecdsa {owner, signature}`。服务同样按链上规则验证（ecrecover 后必须是成员），和 passkey 批准合并在同一提案里。
- **执行**：签名凑齐后，
  - 本机有 Vela 账户 → 由它执行（UserOp 里调 `execTransaction`，手续费从它付），和现在一样；
  - 本机没有 Vela 账户 → 通过同一个通道请外部钱包 `eth_sendTransaction` 执行，手续费由该外部账户付。
- **签名的组装**：按 owner 地址升序；EOA 是 65 字节 `r‖s‖v`（v=27/28），Vela 成员是合约签名（v=0），可以混合。
- **体验**：
  - 提案详情里每个外部成员一行「用外部钱包签名」，点开显示二维码和链接，状态依次是等待打开、已连接（显示账户）、请在钱包里确认、已签名；
  - 配对会保存在本机，下次同一个外部钱包签名不用再扫码；
  - 任何一步都可以取消，取消会通知签名页（`walletpair_cancel`）；
  - 账户不符、链不符、用户拒绝、断开，都有明确的提示和重试。
- **安全**：
  - 邀请链接是一次性凭证，配对后失效（PeerConnect 换到只有双方能算出的 pair ID）；
  - 签名页只转发签名和发送交易，拒绝 `eth_sign`；
  - Vela 不信签名页：签名、owner、safeTxHash 全部在 core 里重新校验；
  - 签名页显示请求方是 Vela 和这笔交易的摘要，钱包里显示完整的 EIP-712 内容。
- **测试**：本地中继 + fork 上用 EOA owner 部署的 Safe v1.3.0/v1.4.1，第二个浏览器上下文注入一个用测试私钥签名的 EIP-1193 提供者，端到端跑导入、签名、执行。

### 11.3 第四轮之后的外部 owner 流程（2026-10-29 更新）

11.2 的「提案详情里点开、实时配对」仍然可用，但不再是唯一的路：外部 owner 可以稍后、单独、只凭链接处理一笔交易，不需要 Vela 那一端开着。

- **稍后批准的链接**：提案详情里给还没批准的外部成员一个链接 `/<locale>/connect#for=<成员地址,…>&ms=<chainId>.<safe>.<safeTxHash>`（不带邀请，不是一次性凭证；`for=` 是 Vela 期望的成员）。
  签名页从邮箱读这笔交易，**什么都不信**：core 重新计算 SafeTx 的哈希，必须等于链接里的 safeTxHash，否则拒绝并说明；
  邮箱里的每个批准都重新校验，只有它的 owner 此刻是该链上的成员才算数（加上链上 `approvedHashes` 里的批准）。谁能操作，以链上状态为准，不以邮箱为准。
  页面从第一帧起就说「正在读取交易…」（`app.html` 在任何模块运行前读 fragment），不会说「正在连接 Vela」。
- **签名前的审阅**：签名页画出和 Vela 自己的提案详情同一份审阅：哪个多签、哪条链、这笔交易做什么（core 解读，转账/调用/成员变更）、交易编号，
  以及硬件钱包会显示的三个哈希——safeTxHash、域哈希（domain hash）、消息哈希（message hash），每个都可复制，用来和钱包屏幕逐位核对。
  §3.5 的判断在这里同样执行：core 拒绝的 genesis 在该链上只读，什么都不批准、不执行、不拒绝；有成员以外的东西能操作时先说明并再确认一次；还读不出来时不做任何操作。
- **批准的三种方式**：
  - 首选 `eth_signTypedData_v4`（EIP-712 的 SafeTx）；
  - 钱包不支持签结构化数据时，用 Safe 的 `eth_sign` 形式（对 safeTxHash 做 `personal_sign`，签名的 `v` 加 4，即 31/32）——只接受 EIP-712 的邮箱会拒收这种形式，页面说明这一点，并提供链上批准；
  - 链上 `approveHash(safeTxHash)`：由该 owner 的钱包发一笔交易，手续费由它付；这种批准不进邮箱，执行时从链上读回（签名里 `v = 1`）。
  无论哪种，签名回来先在 core 里核对（ecrecover 必须是这个 owner、对应这笔 safeTxHash），再交给邮箱；邮箱一时收不下的会保留并自动重发，页面显示倒计时和「重新发送」。
- **执行**：批准数达到阈值后，外部 owner 可以直接在签名页执行：钱包 `eth_sendTransaction` 调 `execTransaction`，手续费由该钱包付，发送前先估算并显示；
  落链后说明成功或失败（失败也付了手续费）。
- **拒绝**：外部 owner 可以拒绝：提出同一编号的取消交易（发给多签自己、不转任何东西），并附上自己的批准；取消交易执行后，原交易作废。
  在签名页里不能发起新的转账提案：那需要 Vela 的发送流程（代币、金额、收款人、下一个空闲编号）。
- **结果**：每个请求在页面上都有结局——已批准 ✓（可以关闭页面）、已签名 ✓、被拒绝、被撤回、失败（原因用人话说）；钱包长时间不回应时提示去钱包窗口看看，并可以「再问一次」，永远不会无止境地「请在钱包里确认」。
- **实时配对仍在**：Vela 开着提案详情时，签名页的请求**先出现在页面上**（审阅），再到钱包里确认；配对保存在本机，下次同一个钱包不用再扫码，也可以「换一个钱包」或解除配对。

