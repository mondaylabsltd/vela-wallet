# 033 · Desktop Send Parity — results

分支 `033-desktop-send-parity`,叠在 `032-desktop-money-wiring` 上(032 的 44–47
四刀先修掉了刷新、隐藏余额、网络筛选和网络设置四组)。

**闸门**(与 032 相同,原样跑):

```bash
cd app-desktop/vela-wallet
cargo fmt --all --check && cargo test --features dev-fixtures && cargo test \
  && scripts/sweep-gallery.sh && scripts/check-windows.sh
```

---

## Phase 1 — 归集(sweep):一次把几种币发给同一个人

对照清单里最大的一件:`ToggleMultiToken` / `ToggleAllMultiTokens` /
`SetMultiNetwork` / `ConfirmMultiSelection` 四个事件,和 `multi_select_mode` /
`multi_selected_ids` / `multi_valuable_ids` / `multi_chain_id` / `multi_specs`
五个视图字段,桌面**一个都没读过**。而 DSD1L 那句"发送多种代币"从 spec 021 起就画在
列表底下,是一行居中的灰字 —— **没有监听器**。

### 一个必须留在壳里的标志

核心的 `multi_select_mode` 只在选择被**确认**时才翻,所以"勾选框正在显示吗"在视图里
根本没有对应字段。web 的移植记了同一件事(`live-send.ts` 的 `sweepPicking`),手机
也一样(`TokenSelector.tsx` 的 `sweepActive`)。所以 `send_sweeping` 是页面的:
**哪些币能选、什么算"有价值"、一次归集搬多少,全都还是核心的。**

### 第一次点,决定这是哪条链

一笔批量只能在一条链上。手机用筛选器钉,画稿(SD1b)用**第一次点**钉:第一下命名网络,
之后核心拒绝其他链的每一行。所以行的监听器在 `multi_chain_id` 还是 `None` 时先发
`SetMultiNetwork`,再发 `ToggleMultiToken`。清空选择会解钉,于是"重来一次"不用离开这一屏。

### 别的链的行变灰,不消失

这条是 SD1b 自己的注释,也是这一刀唯一值得单独写测试的规则:**那些币还是这个人的**。
列表在有人勾了一下之后悄悄变短,读起来就是"我的钱不见了"。所以灰而不删,而且灰掉的行
不再可点 —— 核心反正会拒,画成可点是一个它不会兑现的邀请。

"全选有价值的"发的是 `ToggleAllMultiTokens { visible_ids }`:**范围是屏幕上正在显示的**,
而这些里面哪些算有价值,仍然是核心答。

### 零新键

`send.multiSendTitle` / `multiSendChainNotice` / `selectAllValuable` /
`multiSendContinue` 全都在语料里,十五种语言齐全 —— 手机先画的这一屏。

### 测试与实机

一条纯映射测试(勾了什么、灰了哪些、药丸说哪条链、CTA 数几个),视图从真机器取一份再替换
sweep 字段(`wallet::live` 的老办法:手写一个几十个字段的视图等于对它们的不变量瞎猜)。

实机(parallel space 金标 Safe,只有一条链有钱):按"发送多种代币"→ 勾选框和"全选有价值的"
出现;点 xDAI → **Gnosis 的圆标 + "Gnosis selected — a multi-token send stays on one
network, so the others are greyed out"**,行染上强调色,底部变成橙色的 `Send 1 · Gnosis →`。

desktop **342 / 338**(+1 测试),fmt clean,画廊全渲染。

---

## Phase 2 — 中继金库那一屏,终于有出口了

清单上原本三件,复核之后只剩一件半:

- **报价过期(`Requote` / `FeeView.stale`)撤下**。web 的 `FeeQuote` 类**有** `requote()`
  这个方法,而**没有任何组件调用它**;`FeeView.stale` 也没有任何 live 模型读。
  两端都没有 ⇒ 不是对齐,是新功能。记在 spec 里,不在这一刀做。
- **中继金库**不是"缺一张图":桌面从 032 phase 31 起就把它画成了内联的 notice
  ——地址、缺口金额、"现在检查"的重试,全都对。**缺的是离开它的那一下。**

### 一个从写下来就没人发过的事件

`DismissTreasurySheet` 在 `send` 机器里一直有,这只壳一次也没发过。于是"这条链上中继
付不起"这个停顿,唯一的出路是**把整趟发送关掉** —— 而这个停顿是会自己解除的
(有人给金库充了值,或者重试成功)。现在 notice 卡上并排两个出口:核心给的重试在前
(那是人来按的那一个),"暂不"在后。用的是手机自己的词 `componentsUi.funding.cancel`,零新键。

一条测试:停顿在,两个出口都在,而且**该说的事实一个没少** —— 往哪儿充、还差多少 ——
一个把事实也带走的"关闭"是更糟的屏幕,不是更体贴的。

desktop **343 / 339**(+1 测试),fmt clean,画廊全渲染。

---

## Phase 3 — 分账的行,终于能改能删了

`RecipientsChanged` 是 web 那边点了四处的事件(改一行的金额、删一行、加一行、
为某一行开通讯录),桌面**一次也没发过**。收款人卡上那个 X 从 spec 021 起就画着,
**没有监听器**;每行的金额是一行静态文字。也就是说:桌面能进分账模式、能从通讯录挑人,
**但没有任何办法给每个人填金额** —— 一屏填得完、永远发不出去。

### 一个事件,三种改法

核心只给了一个事件,而且它带的是**整份名单**。所以三种改法都是"名单,改一处":
`split_amount_edited` / `split_row_removed` / `split_row_appended`。抽成函数而不是写在
页面的闭包里,因为出错的地方是看不见的:**重建时丢掉一行的 `id` 或 `name`**,
会让通讯录选择器(按 `picker_target` 找行)指向一个不存在的行,会把"Alice"悄悄变回一串地址,
而下游不会有任何东西抱怨。测试就钉这一条:改第 1 行,第 0 行和第 2 行**逐字节不变**;
删中间一行,剩下两行的 id 和名字都在;新加的行 **id 为空** —— 由核心去铸
(它自己的 `rcpt_{n}`),壳再造一套命名就是第二种命名法。

### 每行一个焦点句柄

焦点是"键盘敲进哪个框",共享句柄会把每一次击键送进最后画的那一行。所以句柄按行存在页面上,
按需生长 —— 和设置面板里那套一样。

### 实机与诚实的边界

**删验过**:点第二行的 X,行没了,而且屏幕**收回单人模式** —— 这正是核心自己的规矩
(`rows.len() <= 1` 时收回,并把剩下那一行的地址和金额带回单人字段)。
**改和加没能在实机上点到**:今天这台机器上的合成点击在这一片区域不落点(032 交接里
已经记过这条坑),两条改法由抽出来的函数和它的测试守着。

desktop **344 / 340**(+1 测试),fmt clean,画廊全渲染。
