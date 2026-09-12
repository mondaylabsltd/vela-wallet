# Vela 签名通道协议 v1

一份签名意图怎么从请求方到达签名页，签名怎么回去。四种通道，**一种意图格式**。

没有服务器。没有中继。跨设备走 BLE。

---

## 0. 通道选择

| 请求方 | 签名方 | 通道 | 来源可验证 |
| --- | --- | --- | --- |
| 同浏览器的网页 dApp | 本页 | `window.postMessage` | ✅ 浏览器背书 |
| 同浏览器的网页 dApp | Chrome 扩展 | `externally_connectable` | ✅ 浏览器背书 |
| 同机桌面 App | 本页 / 扩展 | URL 片段 + 回环回调 | ❌ 自述 |
| **跨设备的 App（手机或另一台电脑）** | **Chrome（扩展页或签名页）** | **BLE GATT** | ❌ 自述，但**邻近可证** |

同机的桌面 App 不要用 BLE：蓝牙控制器一般扫不到本机自己发出的广播，而回环回调更快也更可靠。

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

没有服务器就没有 TLS。BLE 自身的链路加密在各家实现上参差不齐，所以**我们自己加密**，
用的全是浏览器内建的 WebCrypto（P-256 ECDH + HKDF + AES-GCM），零依赖。

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
   key    = HKDF-SHA256(shared, salt, "vela-ble/1 key",  32)
   code   = HKDF-SHA256(shared, salt, "vela-ble/1 code",  4)
   ```

4. **比对码** = `code` 前 4 字节按 big-endian 取模 `1000000`，补足 6 位十进制。
   两端同时显示，用户确认一致后才继续。

   这一步不是仪式感：它是中间人攻击唯一的拦截点。攻击者能各自和两端完成 ECDH，
   但两端算出的比对码不会相同。**没有比对码的 BLE 通道等于明文广播里的一次转账。**

5. 之后所有帧 `flags=1`，载荷是 `12 字节 IV ‖ AES-GCM 密文`：

   ```
   IV  = 4 字节方向标签 ‖ 8 字节计数器（big-endian，每方向各自递增，绝不重用）
   AAD = "vela-ble/1|" ‖ 方向("c2p"/"p2c") ‖ "|" ‖ msgId
   ```

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
`unsupported`（方法不认识）、`refused`（本页规则拒签，例如无限额授权）。

**任意方向：心跳/挂断**

```json
{"v":1,"t":"bye","n":9,"reason":"done"}
```

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
| Chrome 扩展页 / 签名页 | central | `lib/transport/ble.js` —— 已实现，`samples/ble-loopback.mjs` 10/10 通过 |
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

### 7.2 URL 片段 + 回环回调（同机原生 App，无服务器）

App 用系统能力打开浏览器：

```
https://sign.getvela.app/sign.html?ch=url#i=<base64url(JSON)>&cb=<base64url(回调URL)>&t=<一次性token>&z=1
```

`i` 是 `{"intent":{…},"context":{…}}`；`z=1` 表示先经过 `deflate-raw`
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

