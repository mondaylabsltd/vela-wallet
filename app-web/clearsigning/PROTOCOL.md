# Vela 签名通道协议 v1

一份请求怎么从请求方到达签名页，答复怎么回去。六种通道，**一种请求格式**。

请求有两类：**签名意图**（dApp 的 EIP-1193 请求、钱包自己的转账，第 4、8 节）和
**钥匙仪式**（创建、登录、证明，第 10 节）。清晰签名器是和「这台设备 / 手机或平板 /
USB 安全密钥」平级的一条 passkey 通道（创始人，2026-09-22）。

~~没有服务器。没有中继。跨设备走 BLE。~~ —— **已被创始人推翻（2026-09-22）**：跨设备走
**中继**（WebSocket，第 7.5 节）或 **BLE**。中继是瞎的：它只转发两端加密好的字节
（`specs/075-clear-signer-channel/contracts/relay.md`）。

---

## 0. 通道选择

| 请求方 | 签名方 | 通道 | 来源可验证 | 一个会话几个请求 |
| --- | --- | --- | --- | --- |
| 同浏览器的网页钱包 / dApp | 本页 | `window.postMessage`（7.1） | ✅ 浏览器背书 | 多个 |
| 同浏览器的网页 dApp | Chrome 扩展 | `externally_connectable`（7.3） | ✅ 浏览器背书 | 一个 |
| 同机的 Android / iOS / 桌面 App | 本页 | 回环 WebSocket（7.4） | ❌ 自述；一次性 token 证明「是打开本页的那个 App」 | 多个 |
| 同机桌面 App（旧） | 本页 / 扩展 | URL 片段 + 回环回调（7.2） | ❌ 自述 | 一个 |
| **任何一端，跨设备** | **另一台设备上的本页** | **中继**（7.5） | ❌ 自述；`rk` + 六位码证明「是那个钱包」 | 多个 |
| 原生 App，跨设备 | 附近的 Chrome | **BLE GATT**（1–4） | ❌ 自述，但**邻近可证** | 多个 |

同机的 App 不要用 BLE：蓝牙控制器一般扫不到本机自己发出的广播。桌面 App 正从 URL 片段
+ 回调迁到回环 WebSocket（和手机同一套 Rust 会话）。

浏览器只能当 **central**，而且 Web Bluetooth 只在文档里可用（service worker 里没有）。
所以 BLE 通道成立的前提永远是：**一端是我们的原生 App（peripheral），另一端是打开着的 Chrome 页面（central）**。

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

---

## 3. 握手：ECDH + 数字比对

BLE 自身的链路加密在各家实现上参差不齐，中继又不该看见任何东西，所以**我们自己加密**，
用的全是浏览器内建的 WebCrypto（P-256 ECDH + HKDF + AES-GCM），零依赖。

BLE 和中继跑**同一份会话代码**：页面这边是 `lib/transport/secure.js`，钱包那边是
vela-core 的 `clear_signer::secure`（Rust）。**标签是参数**：BLE 用 `vela-ble/1`，
中继用 `vela-relay/1`（下文写作 `<label>`）。两边由
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
         尾巴：BLE 是这条消息帧的 msgId；中继是 IV 里的计数器（十进制）
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

`code` 取值：`user_rejected`（用户没滑）、`expired`（超过 `expires`）、
`unsupported`（方法不认识）、`refused`（本页规则拒签，例如无限额授权、自带挑战码）、
`unavailable`（本页需要的服务没应答，例如成员证明时的注册表）。

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
- 攻击者必须在射频范围内。相比二维码 + 中继，截屏钓鱼这条路被切断了。

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

**一个会话多个请求**（第 11 节）：第一条 intent 必须来自 `window.opener`，之后只听这个窗口、
这个 origin；答完一个继续等下一个，直到对方发 `{vela:'bye'}`、关窗或空闲 5 分钟。
参考 `samples/wallet-sim.js`。

### 7.2 URL 片段 + 回环回调（同机原生 App，无服务器）

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

错误码分两种，对请求方意义完全不同：`refused`（**签名页的规则**拒绝了它，例如无限额
授权、操作与请求不符）与 `user_rejected`（**人**没有滑动确认）。

可运行的参考实现：`samples/desktop-demo.mjs`。

### 7.3 扩展端口（桌面首选）

我们自己的页面（`externally_connectable` 白名单内）连一个端口：

```js
const port = chrome.runtime.connect(EXTENSION_ID, { name: 'vela-sign' });
port.postMessage({ vela: 'intent', intent, context });
port.onMessage.addListener(m => { /* { vela:'result', result } | { vela:'error', code } */ });
```

用端口而不是一次性消息：端口在人读签名卡的这段时间里让 service worker 保持存活，
万一还是被回收，dApp 会收到 `onDisconnect` 而不是无限等待。
`sender.origin` 由浏览器填写，因此**这条通道的来源是可验证的**。

> Chrome 不允许 `externally_connectable.matches` 写全通配，所以第三方 dApp
> 不能直接连扩展 —— 它先到我们的 https 页面（7.1），再由那一页转交。

钥匙仪式的答复经扩展转回时，端口上收到的是 `{vela:'result', …字段}`（第 10.3 节）。

### 7.4 回环 WebSocket（同机 App，071 起；075 起一个会话多个请求）

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

### 7.5 中继（跨设备，075）

钱包生成房间号和一把静态 P-256 密钥，显示二维码 + 可复制的链接：

```
https://sign.getvela.app/sign.html?ch=relay#relay=<wss URL，percent 编码>&room=<22 位 base64url>&rk=<22 位 base64url>&v=1
```

- `rk = b64url(SHA-256(钱包公钥)[0..16])`。
- 页面从片段读出（片段不发给任何服务器，读完立刻抹掉），连 `<relay>/v1/rooms/<room>?role=signer`。
- 中继 URL 只接受 `wss:`，或本机回环上的 `ws:`。
- 中继发来 `{"v":1,"relay":"joined"}` 后，页面发明文 hello（文本帧），钱包回 hello；
  **钱包公钥哈希对不上 `rk` → 页面拒绝**：不算比对码、不发任何加密帧、离开房间
  （这挡住中继或猜到房间号的人冒充钱包）。
- 对上了 → 双方显示同一个**六位码**，人在钱包上确认后钱包才发请求（这挡住冒充页面的人，
  对「创建钥匙」尤其要紧：冒充的页面会塞给钱包一把别人的钥匙）。
- 之后每条消息是一个二进制帧 `IV(12) ‖ AES-GCM`（第 3 节，`vela-relay/1`，AAD 尾巴是计数器），
  没有 BLE 分帧。明文 JSON 与第 4 节相同。
- 钱包掉线（`{"relay":"left"}`）：屏幕上那个请求作废（不再能确认，也不会答复），页面等它回来；
  再次 `joined` 时从头握手（新密钥、新 nonce、新比对码，`rk` 再查一遍）。
- 比对码画在等待卡和会话里的**每一张卡**上。
- 中继关闭码：4408 过期/空闲、4409 角色已占、4400/1009 请求不对 —— 页面各有说法。

参考：`lib/transport/relay.js`；房间规则的模拟实现 `samples/mock-relay.mjs`（真中继是 Rust，
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
| 网络费（金额/代币/收款方） | 从 calldata 里的**费用腿**读出 | 否 |
| 「这一笔是网络费」 | 请求方的标注 | 是，界面明说 |
| 法币金额 | 本地 = 数量 × **请求方给的汇率**，汇率同时显示 | 只能改汇率，且看得见 |
| 签名摘要 | 本地推导（EIP-191 / EIP-712 / SafeOp） | 否 |
| 链名 | 由 **chainId** 本地查表；未知链才用自述并标「自述」 | 否（已知链） |
| identicon | 本地由地址算出，与 vela-core 逐字节一致 | 否 |
| 账户地址 | 交易时即 `userOp.sender`，在摘要内 | 否 |
| **账户名字** | 请求方（或本机注册记录） | 是 —— 它的作用是**指向该用哪把 passkey**，改不了钱的去向 |
| 站点名 / 图标 / origin | 请求方 | 是，且始终挂「站点身份由请求方自述」 |
| 模拟结果 | 请求方 | 是，标注「模拟由请求方提供」 |
| **收款方的名字** | —— | **不显示**：这是最容易被投毒的一项 |
| 预渲染的费用字符串 | —— | **不使用** |
| 请求方给的摘要 | —— | **不使用** |

---

## 10. 钥匙仪式：创建、登录、证明（075）

请求仍是 `{id, intent:{method, params, origin:""}, context}`，所以通道不需要新信封。
`context.walletName` 是钱包的名字（显示在卡上，它是人认出自己钱包的方式；不可读或超长则不显示）。

### 10.1 四种请求，以及每种怎么守住「只签自己推导的」

| `intent.method` | `params[0]` | 本页签的挑战码 |
| --- | --- | --- |
| `vela_createPasskey` | `{name, excludeCredentialIds}` | 本页生成的 32 个随机字节。注册本来就不证明任何挑战码，别人的字节没有理由进来 |
| `vela_signIn` | `{}` | UTF-8 `vela-signin-<毫秒>-<16 位十六进制>`：本页的时钟 + 8 个随机字节 |
| `vela_proof` | `{credentialId?, purpose}`，`purpose` = `verify` / `recover_first` / `recover_second` | UTF-8 `vela-verify-<毫秒>` / `vela-recover-<毫秒>`（各端原生流程的同一格式） |
| `vela_memberProof` | `{credentialId, publicKey, attestation, groupPublicKey, registry}` | 本页**自己**请求注册表，并且**自己算出**同一个挑战码：`keccak256(abi.encode(chainId, registry, rpId, publicKey, keccak256(abi.encode(groupPublicKey, attestation))))`，`chainId` 与 `registry`（`domainRegistry`）来自注册表的 `GET /api/health`；`POST {registry}/api/challenge {rpId, groupPublicKey, publicKey, attestation?}` 答的必须正是这 32 字节（以及同一个 `binding`），否则拒签 |

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

- App 通道：回环 WebSocket（一次性 token）、中继（`rk` + 比对码）、BLE（邻近 + 比对码）；
- 或浏览器背书的 origin（postMessage、扩展端口）是 `https://getvela.app` / `https://*.getvela.app`，
  或本机回环（开发与测试时网页钱包跑在这里）。

URL 片段不算：任何网页都能用它打开本页。请求方在 `context` 里写的 `channel` / `requester` /
`originVerified` 一律被通道自己的事实覆盖。登录和证明对其他请求方也显示，但挂一条
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

## 11. 会话：一个通道，多个请求（075）

回环 WebSocket、postMessage、中继、BLE 上，一个会话按顺序承载多个请求（创建 → 成员证明；
恢复第 1 步 → 第 2 步），钱包每个流程只打开本页一次：

- 答完一个请求，页面回到平静的「等待钱包」，继续等下一个 `intent`；
- 结束于：钱包的 `bye`、通道关闭、或空闲 5 分钟（页面发 `bye`，`reason:"idle"`）。
  卡还在屏幕上、等人看的时候不算空闲；
- 会话里被本页规则拒掉的请求**立刻**答 `refused`，理由留在屏幕上直到下一个请求；
  一次性通道（URL 片段、扩展）仍是「关掉页面才发拒绝」；
- 关掉页面时屏幕上若有未答的请求，照旧答 `user_rejected`（或 `refused`）。

