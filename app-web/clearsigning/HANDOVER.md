# 交接：清晰签名页

给下一个接手的人（或下一次对话）。读完这一页就能继续干活。

---

## 这是什么

一个**零构建、零依赖**的纯静态页面，同一个文件夹既是网页也是 Chrome MV3 扩展。
它接收一份**签名意图**，渲染成看得懂的签名卡，然后用**真实的 WebAuthn passkey**
签名并把结果送回请求方。

一句话目标：**所见即所签，不容有沙子。** 屏幕上每一样东西都要能说清来源，
凡是请求方能改又被当成事实展示的，一律拿掉或明确标注。

## 先跑一遍（约两分钟）

```sh
cd app-web/clearsigning
export CHROME_BIN="$HOME/Library/Caches/ms-playwright/chromium-1234/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"
export SB=<任意可写目录>   # 需要 tls-serve.py / cert.pem / key.pem，见下

node samples/safeop-test.mjs      #  9/9  摘要对拍 vela-core 的 wasm
node samples/identicon-test.mjs   #  9/9  头像与 vela-core 逐字节一致
node samples/ble-loopback.mjs     # 10/10 BLE 分帧/握手/加密/防重放
node samples/hostile-test.mjs     # 15/15 敌意上下文：毒串一个都不许当事实显示
node samples/channels-test.mjs    # 24/24 三条通道 + 真 WebAuthn + 交易 + 篡改拒签
node samples/extension-surface-test.mjs   # 扩展页里的表现
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
意图 → resolve.js → 视图模型 → render.js → DOM
                 ↘ digest.js/safeop.js → 摘要 → signer.js → 签名 → intake.js 回传
```

| 文件 | 职责 | 不变量 |
| --- | --- | --- |
| `lib/resolve.js` | 全部裁决（降级阶梯、安全闸门、费用、身份） | **不产生任何人话**，只出 i18n key |
| `lib/render.js` | 视图模型 → DOM | **不做任何判断**，也没有任何能编辑意图的控件 |
| `lib/digest.js` `lib/safeop.js` | 摘要 | 只签**自己算出来的**；算不出就拒签 |
| `lib/signer.js` | passkey 断言 | **没有创建能力**（在 `lib/enrol.js`，签名页不加载） |
| `lib/identicon*.js` | 头像 | 与 vela-core 逐字节一致 |
| `lib/fee.js` | 费用 | 从 calldata 里的**费用腿**读，不接受成品字符串 |
| `lib/intake.js` | 四条通道归一 | 只有浏览器背书的通道才 `originVerified: true` |
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

## 创始人已经拍板的规矩（别自作主张改）

1. **意图到达即定死** —— 只能签或不签。没有费币选择器，也没有额度编辑器
   （编辑器会重写 calldata，正是本页要防的病）。无限额授权 = 拒签并指路。
2. **rpId**：web 用 hostname（`*.getvela.app` 折成 `getvela.app`），扩展硬编码。
3. **签名页永远不创建 passkey** —— 创建是新账户。做成了结构保证。
4. **收款方的名字不显示**（请求方可投毒）；**账户名字显示**（它指向该用哪把 passkey，
   改不了钱的去向，且地址与 identicon 就在旁边）。
5. **费用从 calldata 的费用腿读出**，法币只接受**汇率**、本地相乘并显示汇率。
   Vela 走带内费用，4337 的 gas 字段通常全是零。
6. 桌面 App 与同机浏览器**不要用 BLE**（控制器扫不到自己），走 URL + 回环回调。

## 环境坑（踩过的，别重踩）

- **Chrome 152 stable 静默无视 `--load-extension`**。用 ms-playwright 缓存里的
  Chrome for Testing。
- `fetch('/json/new?<url>')` 会**丢掉 `#` 之后的一切** —— 要 `encodeURIComponent`。
- **CDP 虚拟认证器只属于添加它的 target**，而且签名页不会自己造钥匙，
  所以测试要用 `WebAuthn.addCredential` **预先注入**一把常驻钥匙。
- 系统 socks5 代理会绕开 `--host-resolver-rules`，测试会静默打到线上真站 →
  加 `--no-proxy-server`。
- 本仓库多会话共用工作树：**提交前先看 `git status`，只提交自己的路径**。

## 生成物

- `samples/intents.json` 由 `node samples/make-intents.mjs` 生成（含时间戳，
  重跑会有噪声 diff，正常）。
- `samples/hostile-intents.json` 由 `hostile-test.mjs` 每次重写。
- `lib/identicon-features.js` 从 vela-core 的生成表提取；**改动它等于改变
  所有用户的头像**，是发布阻断级事件，不是杂活。
