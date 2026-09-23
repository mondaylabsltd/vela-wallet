# 交接：清晰签名页

> ## ⚠️ 2026-09-23：跨设备通道已全部撤回
>
> 隧道和 BLE 都砍了，Web 钱包也不再支持清晰签名器。只剩「同机 App ↔ 本页的回环
> WebSocket」一条通道。下面凡是提到隧道、BLE、六位对码、`secure.js` 的，都是历史
> 记录 —— 对应的文件已经删除，对应的命令跑不起来。理由见
> `specs/075-clear-signer-channel/spec.md`「What the owner cut」。

给下一个接手的人（或下一次对话）。读完这一页就能继续干活。

---

## 这是什么

一个**零构建、零依赖**的纯静态页面，同一个文件夹既是网页也是 Chrome MV3 扩展。
它接收一份**签名意图**，渲染成看得懂的签名卡，然后用**真实的 WebAuthn passkey**
签名并把结果送回请求方。

075 起它还是**一条 passkey 通道**，和「这台设备 / 手机或平板 / USB 安全密钥」平级
（创始人，2026-09-22）：钱包的创建、登录、证明流程也能走它 —— 四种**钥匙仪式**
（`vela_createPasskey` / `vela_signIn` / `vela_proof` / `vela_memberProof`），各有自己的卡。
一个会话可以承载多个请求；跨设备走隧道或 BLE。

一句话目标：**所见即所签，不容有沙子。** 屏幕上每一样东西都要能说清来源，
凡是请求方能改又被当成事实展示的，一律拿掉或明确标注。

## 先跑一遍（约两分钟）

```sh
cd app-web/clearsigning
export CHROME_BIN="$HOME/Library/Caches/ms-playwright/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
export SB=<任意可写目录>   # 需要 tls-serve.py / cert.pem / key.pem，见下

node samples/safeop-test.mjs      #  9/9  摘要对拍 vela-core 的 wasm
node samples/identicon-test.mjs   #  9/9  头像与 vela-core 逐字节一致
node samples/secure-vectors.mjs   # 82/82 secure.js 对拍 vela-core 的会话向量（Node + Chrome）
node samples/ble-loopback.mjs     # 10/10 BLE 分帧/握手/加密/防重放
node samples/mock-tunnel-test.mjs  # 14/14 模拟隧道守 tunnel.md §1 的房间规则
node samples/hostile-test.mjs     # 39/39 敌意上下文 + 敌意仪式（自带挑战码、网站要建钥匙、冒充钱包、撒谎的注册表）
node samples/channels-test.mjs    # 36/36 各通道 + 真 WebAuthn + 交易 + 篡改拒签
node samples/ceremony-test.mjs    # 55/55 四种仪式 + 多请求会话 + 隧道与比对码
node samples/extension-surface-test.mjs   # 扩展页里的表现（33 张卡，0 失败）
node samples/desktop-demo.mjs --auto      #  8/8 桌面应用全流程 + 自验签
```

**`$SB` 里需要三个文件**（几个测试要把页面架在真实的 `getvela.app` 源上，
因为 passkey 的 rpId 准入检查在浏览器进程里，绕不过）：

```sh
openssl req -x509 -newkey rsa:2048 -keyout $SB/key.pem -out $SB/cert.pem \
  -days 30 -nodes -subj "/CN=getvela.app" -addext "subjectAltName=DNS:getvela.app"
cat > $SB/tls-serve.py <<'EOF'
import http.server, functools, ssl, sys
directory, port, cert, key = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=directory)
httpd = http.server.ThreadingHTTPServer(('127.0.0.1', port), handler)
ctx = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
ctx.load_cert_chain(cert, key)
httpd.socket = ctx.wrap_socket(httpd.socket, server_side=True)
httpd.serve_forever()
EOF
```

看界面：

```sh
python3 -m http.server 8099        # → http://localhost:8099/gallery.html
node samples/desktop-demo.mjs      # 不带 --auto：真按一次 Touch ID
```

## 代码怎么分层

```
intake.js（会话）→ 请求 → resolve.js → 视图模型 → render.js → DOM
   签名意图：digest.js/safeop.js → 摘要   → signer.js    → 断言 → 回传
   钥匙仪式：ceremony.js → 本页的挑战码   → ceremony.js  → 注册/断言 → 回传
   → 回到「等待钱包」→ 下一个请求 … 直到 bye / 关闭 / 空闲 5 分钟
```

| 文件 | 职责 | 不变量 |
| --- | --- | --- |
| `lib/resolve.js` | 全部裁决（降级阶梯、安全闸门、费用、身份；仪式的准入与拒签） | **不产生任何人话**，只出 i18n key |
| `lib/render.js` | 视图模型 → DOM（签名卡、仪式卡、等待卡、比对码） | **不做任何判断**，也没有任何能编辑意图的控件 |
| `lib/digest.js` `lib/safeop.js` | 摘要 | 只签**自己算出来的**；算不出就拒签 |
| `lib/signer.js` | 签名意图的 passkey 断言 | **没有创建能力**：签名意图走到的只有 `sign()` |
| `lib/ceremony.js` | 钥匙仪式：挑战码推导、成员挑战码的获取与核对、创建、断言 | 只签**自己生成或自己核对过**的挑战码；创建只由 resolve 放行的 `vela_createPasskey` 调用 |
| `lib/transport/secure.js` | BLE 与隧道共用的 ECDH/HKDF/AES-GCM 会话 | 与 vela-core 的 `clear_signer::secure` 逐字节一致（向量） |
| `lib/transport/ble.js` `tunnel.js` | BLE 分帧 / 隧道房间 | 隧道：`rk` 对不上就拒绝并离开；比对码画在每张卡上 |
| `lib/identicon*.js` | 头像 | 与 vela-core 逐字节一致 |
| `lib/fee.js` | 费用 | 从 calldata 里的**费用腿**读，不接受成品字符串 |
| `lib/intake.js` | 六条通道归一成**会话** | 只有浏览器背书的通道才 `originVerified: true`；通道事实覆盖请求方 context 里的同名字段 |
| `lib/logos.js` | 远端 logo | 纯装饰，只进 `<img>`，失败静默退回 |

规范：[PROTOCOL.md](PROTOCOL.md)（通道线格式、摘要、**第 9 节的来源对照表**）。
设计稿：`docs/design/clearsigning/`（33 张，Penpot 导出；CS19 未导出）。

## 已经完成

- 33 个场景全部由外部意图渲染（`samples/intents.json`，页面零内嵌数据）
- 六级降级阶梯、never-unlimited 拒签、烧毁拦截、SIWE 域名比对、嵌套 calldata 拆解
- i18n（zh/en）、深浅色跟随系统
- 摘要：EIP-191 / EIP-712 / **SafeOp / SafeMessage**，全部对拍通过
- 交易绑定检查：站点请求的调用必须真在被签的操作里，否则拒签
- 通道：postMessage、扩展端口、URL 片段 + 回环回调 —— 三条端到端跑通真 WebAuthn
- BLE 协议 + 中心端实现（`lib/transport/ble.js`），参考外设 `samples/ble-peripheral.mjs`
- 桌面应用演示 `samples/desktop-demo.mjs`（自带回环回调 + 自验签）
- **075**：四种钥匙仪式（各有卡、中英文）；创建从 `lib/enrol.js` 挪进 `lib/ceremony.js`，
  只对 Vela 钱包放行；成员挑战码由本页自己取、自己算、对上才签（与线上注册表对过）；
  多请求会话（回环 WebSocket、postMessage、隧道、BLE），空闲 5 分钟结束；
  `lib/transport/secure.js`（BLE 与隧道共用，对拍 Rust 向量）；隧道通道 + `rk` 核对 + 比对码；
  模拟隧道 `samples/mock-tunnel.mjs`、模拟注册表 `samples/mock-registry.mjs`、
  两个替身钱包（`samples/test-kit.mjs`）、网页钱包替身 `samples/wallet-sim.html`

## 还没做

1. **BLE 真机射频**。协议与中心端就绪，`samples/ble-peripheral.mjs` 是参考实现。
   Chrome 的 `BluetoothEmulation` 是实验域，走到设备选择器就停（不发
   `gattOperationReceived`），**没有替代验证手段**，必须 Mac + 手机各一台。
2. **三个原生外设端**：桌面 Rust / iOS `CBPeripheralManager` / Android
   `BluetoothGattServer`，各自实现 PROTOCOL.md 第 1–4 节即可。
3. **远端 ERC-7730 描述符**。logo 已接（`lib/logos.js`），但描述符还只有页面自带的
   本地表。接远端时**注意来源**：远端描述符给的名字是第三方自述，按第 9 节的规矩
   应当标注，而不是当事实 —— 这一条需要创始人再点一次头。
4. **部署到 `getvela.app`**。目前只在 localhost / 假 TLS 下验证过。
5. **真机 Touch ID 的取消/超时路径**。虚拟认证器永远说"用户已验证"。
6. **真隧道**。页面只对过 `samples/mock-tunnel.mjs`（照 tunnel.md §1 写的）；Rust 的
   `vela-relay`（Docker / Worker）上线后要拿真隧道跑一遍 `ceremony-test` 的 D 段。
   隧道的 120 秒空闲断开靠它自己的 ping/pong 维持，页面不发心跳 —— 人在钱包上核对比对码
   超过两分钟时要确认 ping 真的在跑。
7. **BLE 上的多请求会话**只在代码层面接好了（`fromBle` 用同一个会话），没有测试覆盖；
   BLE 本身的真机射频见第 1 条。
8. **钱包端核对**。答复形状按主力钉的格式写（PROTOCOL.md 第 10.3 节），但 vela-core 的
   `verify_registration` / `verify_ceremony` 写好之前，只由本页的测试自己核对过。

## 创始人已经拍板的规矩（别自作主张改）

1. **意图到达即定死** —— 只能签或不签。没有费币选择器，也没有额度编辑器
   （编辑器会重写 calldata，正是本页要防的病）。无限额授权 = 拒签并指路。
2. **rpId**：web 用 hostname（`*.getvela.app` 折成 `getvela.app`），扩展硬编码。
3. ~~**签名页永远不创建 passkey**~~ —— **已被创始人推翻（2026-09-22）**：钱包的创建流程
   请求时，本页创建 passkey —— 作为**它自己的请求**（`vela_createPasskey`，自己的卡），
   **永远不在签名里面**，且只对 Vela 钱包放行（App 通道，或浏览器背书的 `*.getvela.app` /
   本机回环 origin）。签名意图走到的 `signer.sign()` 仍然没有创建能力。
3a. **只签自己推导的**，钥匙仪式同样适用（PROTOCOL.md 第 10.1 节）：登录签本页生成的
   `vela-signin-<ms>-<16 hex>`；证明签 `vela-verify-<ms>` / `vela-recover-<ms>`；成员证明签
   本页自己向注册表要、并自己算出一致的挑战码；创建用本页的 32 个随机字节。
   请求方想自带挑战码 → 直接拒签，否则「登录」可能是一笔乔装的交易。
4. **收款方的名字不显示**（请求方可投毒）；**账户名字显示**（它指向该用哪把 passkey，
   改不了钱的去向，且地址与 identicon 就在旁边）。
5. **费用从 calldata 的费用腿读出**，法币只接受**汇率**、本地相乘并显示汇率。
   Vela 走带内费用，4337 的 gas 字段通常全是零。
6. 桌面 App 与同机浏览器**不要用 BLE**（控制器扫不到自己）；同机走回环 WebSocket
   （桌面正从 URL + 回环回调迁过来）。
7. ~~没有服务器，没有隧道，跨设备走 BLE~~ —— **已被创始人推翻（2026-09-22）**：跨设备走
   **隧道**（WebSocket，Rust 写，Docker 与 Cloudflare Worker）或 **BLE**。隧道是瞎的，只转发
   两端加密好的字节；页面凭 `rk` 拒绝冒充的钱包，人凭六位码拒绝冒充的页面。

## 环境坑（踩过的，别重踩）

- **Chrome 152 stable 静默无视 `--load-extension`**。用 ms-playwright 缓存里的
  Chrome for Testing。
- `fetch('/json/new?<url>')` 会**丢掉 `#` 之后的一切** —— 要 `encodeURIComponent`。
- **CDP 虚拟认证器只属于添加它的 target**（同一个标签页导航后还在）。签名意图的测试用
  `WebAuthn.addCredential` **预先注入**钥匙；`ceremony-test` 则让页面自己建一把，后面的登录、
  证明、隧道都用这一把 —— 所以它们必须在同一个标签页里。
- 只改片段的导航是同文档导航，页面不会重载：测试换通道时要改查询串（`&s=2`）。
- 测试缩短空闲时钟：导航前 `Page.addScriptToEvaluateOnNewDocument('window.__velaIdleMs = 1500')`。
- 系统 socks5 代理会绕开 `--host-resolver-rules`，测试会静默打到线上真站 →
  加 `--no-proxy-server`。
- 本仓库多会话共用工作树：**提交前先看 `git status`，只提交自己的路径**。

## 生成物

- `samples/intents.json` 由 `node samples/make-intents.mjs` 生成（含时间戳，
  重跑会有噪声 diff，正常）。
- `samples/hostile-intents.json` 由 `hostile-test.mjs` 每次重写。
- `lib/identicon-features.js` 从 vela-core 的生成表提取；**改动它等于改变
  所有用户的头像**，是发布阻断级事件，不是杂活。
