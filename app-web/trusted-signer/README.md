# clearsigning

One folder, one deployment: a static page. Pure HTML/CSS/JS — no build step, no
dependencies, no bundler, nothing to install.

It is reached by the **Android, iOS and desktop wallets**, which open it in this
device's own browser — and, since the owner made it usable everywhere
(2026-10-30), by the **web wallet and the Chrome extension**, which open it with
`window.open` and hand it the request over `postMessage`. (The page itself was a
Chrome MV3 extension once; that form went on 2026-09-23 and stays gone — today's
extension is the wallet, `app-web/vela-wallet/extension`, and opens this page
like the web wallet does.)

## Run as a web page

Any static host works; every path in the app is relative, so a subdirectory is
fine too.

```sh
# from this folder
python3 -m http.server 8080   # → http://localhost:8080
```

Opening `index.html` straight off disk (`file://`) also works.

## 通道

App 用的只有**一条**通道，一种意图格式，**没有服务器**。规范见 [PROTOCOL.md](PROTOCOL.md)。

| 请求方 | 通道 | 状态 |
| --- | --- | --- |
| 桌面 App（macOS / Windows / Linux）、iPhone、Android | 请求放在 URL 片段里（`sign.html?ch=url#i=…`），回答走 `velawallet://sign-result` 自定义 scheme 交回 App | ✅ Android、Windows 11 有端到端记录；macOS / iOS / Linux 接线相同、尚无完整跑通记录 |
| 网页钱包、Chrome 扩展（同浏览器） | `postMessage`（`?ch=post`），答复只交给浏览器报告的打开者 origin **恰好是** `https://wallet.getvela.app`、`https://getvela.app`、`chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl` | ✅ 2026-10-30 起：交易、批量、钱包自己的发送、`personal_sign`、typed data、多签批准（PROTOCOL.md 第 13 节）；其他打开者一律拒签（规格 102 R7） |
| Safari 扩展（iOS，2026-10-31） | 没有 `window.opener`（实测），走同窗中继：Vela 的内容脚本在本页窗口里证明扩展此刻的 origin（`safari-web-extension://<UUID>`，Safari 每次启动都换）并转交消息；只答**证明过的**那一个（PROTOCOL.md 第 13.5 节） | ✅ 本机回环 + iOS 模拟器跑通；官方页上线后生效 |
| 同机 App | 回环 WebSocket（`?ch=ws`，代码还在，没有 App 使用） | 仅测试用：发布页的 CSP 连不出去 |

已砍掉：跨设备的 BLE 与 WebSocket 隧道（075，「只留回环这个，这样更加安全」）；
扩展端口（2026-09-23 随扩展形态一起砍掉）；App 不再用回环 WebSocket（076 改走自定义
scheme —— 发布页的 `default-src 'none'` 就在被哈希的字节里，页面里的 WebSocket 连不出去）。
~~网页版钱包不用签名页（075）~~ —— 2026-10-30 创始人推翻：签名页在网页钱包和扩展上处处可用（上表第二行）。

**答复只交给 `velawallet://sign-result`**（规格 102 R7）：签名、以及 url 通道上的每一种
仪式，答复地址不是这一串就拒签，不弹 passkey，也不往别的地址发任何东西；页面不在
框架里运行（托管方另发 `frame-ancestors 'none'` / `X-Frame-Options: DENY`）。理由与
测试例外见 [PROTOCOL.md](PROTOCOL.md) 第 0 节「答复只交给钱包」。

**签名页处处可用（2026-10-30）**：账户的「签名方式」选了签名页，网页钱包与 Chrome 扩展里的每一种签名都交到这一页 ——
走哪里由 vela-core 的一条规则决定（`signing_venue::signing_route`），答复只按核心的方式接受（`trusted_signer::accept_signature`：
核心从它自己造的请求算出的字节、账户的钥匙）。本页只答 Vela 自己的钱包，按浏览器报告的打开者 origin 逐字比对；
钱包转来的站点写成「钱包的说法」（`warn.relayedOrigin`），不当作已核实。Chrome 应用商店分配给商店包的 id 尚未加入
（`resolve.js` 的 `EXTENSIONS`）。见 [PROTOCOL.md](PROTOCOL.md) 第 13 节。

**多签的执行（第七轮，r7-1）**：执行一笔多签交易是执行者自己的操作（钱包自己的 `wallet_sendCalls`：先部署还没上链的多签和
批准成员的 Vela 账户，再 `execTransaction`，最后费用腿）。本页不再把它画成「批量 —— 无法预知」：`execTransaction` 里的 SafeTx
按多签批准的同一套规则读（拒签也一样），safeTxHash 按钱包给的序号（`context.multisig`，`execTransaction` 不带序号）重算、
与钱包的不符就拒；部署多签的腿必须正好是 Vela 建多签的 setup（重新编码逐字节比对）且 CREATE2 推出的就是被执行的多签，
部署成员账户的腿必须正好是那个 Vela 账户的 setup、推出的是一位批准了的成员；别的腿一律拒。卡上：「执行多签交易 #N：」、
多签发出的金额与收款方、批准数（`2 / 2`，门槛不是本次部署读出的就标「App 的说法」）、批准人、三个哈希、顺带部署了什么。
见 [PROTOCOL.md](PROTOCOL.md) 第 12.7 节。

**多签成员的批准（2026-10-29，`vela_multisigApprove`）**：网页钱包的成员选了签名页时，批准交到这一页。
本页从原始 SafeTx 字段自己算 safeTxHash（以及域哈希、消息哈希），App 给的哈希不一致就拒签；按 vela-core
`review` 的同一套规则读出转账 / 成员变更 / 批量的每一腿 / 拒绝 / 改名，拒绝 MultiSendCallOnly 以外的
delegatecall、改模块 / guard / handler / singleton 的自调用和 gas 退款；成员的 passkey 签
`SafeMessage(safeTxHash)`（成员自己的域）。答复经 postMessage 只交给浏览器报告的网页钱包 origin
（`https://wallet.getvela.app`、`https://getvela.app`），且只限这一种请求。见 [PROTOCOL.md](PROTOCOL.md) 第 12 节。

```sh
node samples/multisig-test.mjs  # 320 项：多签批准 —— 哈希、批准字节、读法与发布的 vela-core wasm 逐例对拍；钱包的唯一审阅（multisigReview）拒的本页一律拒，含状态差异守卫的 Bybit 向量；哈希不符、看不见的字符、超大载荷、非钱包打开者；第 3 段：多签的执行（执行者自己的操作：execTransaction 里的 SafeTx 按批准的规则读、部署腿从 setup 核实、其余一律拒；36 个 SafeTx 逐个执行，核心拒的本页必拒）；Arc 的 USDC 按单位读；十五种语言
bun samples/serve-dist.mjs      # 本机按托管方的样子提供 dist/（含 _headers 的规则与 /b/<hash>/sign），部署前先试
bun samples/answer-test.mjs     # 55 项：答复只交给钱包 —— 钓鱼站的 cb、postMessage、框架、回环，在发布出去的字节上；G 段：网页钱包的多签批准与消息被答、子域被拒；H 段：真的未打包扩展 —— 已发布 id 的 personal_sign / typed data / 交易被答，别的 id 被拒
node samples/hostile-test.mjs   # 35 项：请求方能控制的字段全填毒串，断言一个都不当事实显示
node samples/channels-test.mjs  # 41 项：URL 片段 → velawallet:// + 真 WebAuthn + 交易 + 篡改拒签；postMessage 上的签名被拒；坏链接、读不了的钥匙 id 都有一句话
node samples/claims-test.mjs    # 62 项：谁的说法 —— 「本账户」、网络费的币种、本页不认识的链上 App 给的币名（标「币名由 App 提供」、显示链 ID）、法币与币种、看不见的字符、读不了的钥匙 id
node samples/provenance-test.mjs # 96 项：意图的读法从哪来 —— 同一函数形状打在陌生合约上、换一条链、App 说的金库资产、App 的模拟、去向不是本账户、App 说的现有额度；五种来源行十五种语言
node samples/safeop-test.mjs    #  9 项：SafeOp / SafeMessage 对拍 vela-core
node samples/identicon-test.mjs #  9 项：identicon 与 vela-core 逐字节一致（含 1000 随机地址）
node samples/takeover-test.mjs  # 18 项：自调用 / delegatecall / SafeTx 拒签 —— 与 vela-core self_call_guard 同一规则
node samples/unlimited-line-test.mjs # 11 项：「无限额」从哪里开始 —— 与 vela-core approval_guard 同一条线
```

每样东西的来源见 [PROTOCOL.md](PROTOCOL.md) 第 9 节。**意图下面一行写明它的读法从哪来**
（2026-10-28，与网页钱包同一设计）：✓ 本页从交易读出 / △ 某份描述的说法（描述不是保证）/
△ 函数名来自公共数据库 / △ 看起来像某种调用 —— 只看形状 / ○ 请求方的说法。协议的描述符
只绑在它写给的合约（与链）上；同样形状的函数打在别的合约上就是「合约调用」，按钮是「签名」。

### 在自己机器上点一遍

```sh
node samples/desktop-demo.mjs            # 一笔 1,000 USDC 转账
node samples/desktop-demo.mjs --message  # 一条 SIWE 登录
node samples/desktop-demo.mjs --tamper   # 组装时换掉收款方 → 应当拒签
```

它扮演一个本机桌面应用：起回环端口 → 拉起默认浏览器 → 等签名回来 → **自己验签**
（签名对公钥验证、challenge 就是那份摘要、摘要等于 vela-core 算的 SafeOp 哈希、UV 位、
rpIdHash）。第一次会让你真的创建一把 passkey —— `localhost` 是合法 rpId 且是安全上下文，
**不用先部署到 getvela.app**。加 `--auto` 用无头 Chrome + 虚拟认证器跑，适合放进门禁。

**签名页只签自己算出来的摘要**（`lib/digest.js` + `lib/safeop.js`）：
EIP-191 / EIP-712 对上权威向量，SafeOp 与 SafeMessage 与 **vela-core 的 wasm
逐字节一致**。交易还要过一道绑定检查 —— 站点请求的那一笔调用必须真的在被签的操作里，
否则拒签。`eth_sign` 永远拒签。

**会交出账户的请求一律拒签**（2026-09-27，与 vela-core 的 `self_call_guard.rs` 同一规则）：
账户对自己调用 `addOwnerWithThreshold` / `removeOwner` / `swapOwner` / `changeThreshold` /
`enableModule` / `disableModule` / `setGuard` / `setModuleGuard` / `setFallbackHandler` /
`setup` / `execTransaction` / `execTransactionFromModule`（穿透 `multiSend` 与
`execTransaction` 最多 4 层）；任何 delegatecall —— 包括操作外层 delegatecall 到 MultiSend
以外的地址、MultiSend 里标了 operation 1 的腿；以及请求方要签的 EIP-712 `SafeTx`。
核心在请求到达本页之前就拦了；本页再拦一次，因为它存在的意义就是不信任组装操作的 App ——
检查的是**操作里的每一条腿**，不只是站点要的那几条。

```sh
node samples/safeop-test.mjs    # 9 项：SafeOp / SafeMessage 对拍 vela-core + calldata 解码
```

## 三条不可动摇的规矩

1. **这一页不能改签名意图。** 意图到达即定死，只有「签」和「不签」两种出路。
   因此没有费币选择器，也**没有授权额度编辑器** —— 编辑器会重写 calldata，
   那正是本页要防的「所见非所签」。无限额的授权与 permit（门槛与 App 同一条线：uint256 为 2^200，Permit2 的 uint160 为 2^152）**标红、可原样签**
   （2026-09-26 创始人裁决：Permit2 与批量交易依赖原样额度）；要设上限，是在请求到达这里之前、
   在钱包自己的授权界面上设。对整个 NFT 合集的授权仍然拒签。
2. **逻辑不产生任何人话。** `resolve.js` 只输出 i18n key + 参数，措辞全在
   `lib/locales/`。缺 key 会原样显示 key —— 签名提示上的窟窿必须刺眼。
3. **数据全部来自外部。** 页面不内嵌任何场景；画廊 fetch `samples/intents.json`
   （或 `?src=` 指定），真实入口则从通道接收意图。

## 渲染样例画廊

`gallery.html` 把设计稿 `docs/design/clearsigning/` 的 33 个场景全部渲染一遍。
每一张都走真实管线 —— **意图 → resolve → render** ——，没有一张是写死的 HTML：

```sh
python3 -m http.server 8099   # → http://localhost:8099/gallery.html
```

`?lang=zh|en` 切语言，右上角可切；深浅色默认跟随系统，按钮可覆盖。
样例数据由 `node samples/make-intents.mjs` 生成（一次性 fixture 生成器，
不是构建步骤 —— 页面只 fetch 生成好的 JSON）。

页脚有两条自检：keccak256 对已知向量、以及每个描述符的选择器是否等于
`keccak256(函数签名)[0:4]`。后者是这页存在的理由 —— 一份被投毒的描述符若能
给任意 selector 挂上 `transfer(...)` 的名字，渲染就会如实地骗人。

### 六级降级阶梯

| 级 | 依据 | 口吻 |
| --- | --- | --- |
| 1 | 代币标准（ERC-20 / 721），或写给**这个合约、这条链**的描述符 | 说人话的意图 + 来源行 |
| 2 | 源码已验证，ABI 全解码（只有画廊的样例：`demo/fixtures.js`） | 只报事实，不解释含义 |
| 3 | 协议函数的形状打在别的合约上；或仅 4byte 数据库命中函数名 | 「合约调用」/ 名字无人担保 + caution |
| 4 | 解不开，只有请求方的模拟 | 模拟画成请求方的说法；风险**不降**（仍是 danger） |
| 5 | 模拟发现未声明的流出 | danger + 建议拒绝 |
| 6 | 解码与模拟双失败 | 最强警告「无法预知」 |

`eth_sign` 不走阶梯，直接 hard-danger。

### 管线

| 文件 | 职责 |
| --- | --- |
| `lib/i18n.js` + `lib/locales/*` | 唯一出现人话的地方；十五种语言，和各 App 一样（规格 102） |
| `lib/keccak.js` | keccak256 + 选择器推导（信任的锚点） |
| `lib/abi.js` | ABI 解码（静态类型、动态 bytes/string、数组、元组） |
| `lib/encode.js` | ABI 编码 —— 样例的 calldata 由签名现算，不手抄十六进制 |
| `lib/catalog.js` | 内置网络与代币，由 `samples/gen-catalog.mjs` 从 vela-core 的表生成（按链） |
| `lib/registry.js` | 代币 / 合约（按链）/ ERC-7730 描述符（协议的绑在合约与链上）—— 只有真实的；画廊的虚构样例在 `demo/fixtures.js`，发布页不加载 |
| `lib/resolve.js` | 意图 → 视图模型：降级阶梯与所有安全闸门都在这里 |
| `lib/render.js` | 视图模型 → DOM，不做任何判断 |
| `samples/intents.json` | 33 个场景的意图（外部数据，页面不内嵌） |
| `lib/intake.js` | 通道归一成一个形状（App 用 URL 片段，另有 `postMessage`），并标注来源是否可验证 |
| `lib/digest.js` | 算得出就签，算不出就拒 |
| `lib/safeop.js` | SafeOp / SafeMessage 摘要 + 读回操作 calldata |
| `lib/multisig.js` | 多签批准：严格读 SafeTx、三个哈希、成员的批准字节、按 vela-core `review` 读法与拒签 |
| `lib/signer.js` | 签名的 passkey 断言；钥匙路线（只问那一把钥匙，带 transports 与 hints，规格 102 R5） |
| `lib/ceremony.js` | 钥匙仪式：本页的挑战码、创建与断言（按钥匙所在位置） |
| `sign.html` + `sign.js` | 真正的签名入口：点按确认（WebAuthn 在点按里同步发起）、拒签即关闭、页面版本那一行 |
| `samples/desktop-demo.mjs` | 本机桌面请求方演示 + 回环回调那一端的参考实现 |

判断全部集中在 `resolve.js`，渲染层不做决定 —— 这样「所见即所签」是**读一个文件**
就能核对的，而不是散落在整个应用里。

## Signing is a real passkey ceremony

Creating a key calls `navigator.credentials.create`, signing calls
`navigator.credentials.get`. There is no fixture key and no fake prompt on
either surface — the authenticator prompt you see is the browser's.

On the self-check page (`demo/index.html`) the assertion is then verified with
WebCrypto alone, the same four things `vela-core` checks. The signing page
itself does not verify its own answer — the app that asked does, in
`vela-core`'s `trusted_signer::verify`:

- the ECDSA P-256 signature covers `authenticatorData ‖ sha256(clientDataJSON)`
- `authenticatorData[0..32]` equals `sha256(rpId)`
- the UV flag is set (user actually verified, not just present)
- `clientData.challenge` is byte-for-byte the challenge we sent

The challenge is `sha256` of the exact text shown in the box above the buttons.
What you read is what gets hashed — that is the whole point of clear signing.

### Which relying party

| Surface | rpId |
| --- | --- |
| `https://getvela.app`, `https://*.getvela.app` | `getvela.app` (subdomains fold up) |
| any other host, incl. `localhost` | that hostname |
| `file://` | none — passkeys are disabled, with a message saying so |

The trade in the second row is deliberate: a deploy on a non-getvela.app host
mints a *different* passkey, because a different rpId is a different relying
party.

There used to be a `chrome-extension://` row with `getvela.app` hardcoded,
because under an extension `location.hostname` is the extension id: using it
does not throw, it mints a valid passkey no other surface recognises, and the
person lands in a different, empty wallet. The extension is gone and so is that
hazard.

### Re-running the check without touching a fingerprint reader

The page is driven headlessly with a CDP virtual authenticator. Three things
that are easy to lose a day to:

- The rpId check happens in the browser process, so the page must really be on
  the origin under test: `--host-resolver-rules="MAP getvela.app:443 127.0.0.1:8443"`
  plus a local TLS server and `--ignore-certificate-errors`.
- Add `--no-proxy-server`, or a system SOCKS proxy resolves the name itself and
  the mapping is bypassed — you end up testing the live site.
- The virtual authenticator only exists for the target that added it; add it per
  page, not once per browser.

## Rules that keep the page working everywhere

- **No absolute paths** (`/app.js`) — they break under any web subdirectory.
  Relative only.
- **Classic script, not `type="module"`** — modules are blocked over `file://`.
- **No CDN, no wasm** — nothing is fetched that is not in this folder.
- **`file://` cannot do passkeys** — no origin, no rpId. Double-clicking
  `index.html` still renders; the signing buttons explain why they are off.

The "no inline `<script>`" rule is gone with the extension that required it
(MV3 rejects inline script). Spec 076 relies on that: the signing page is
published as ONE file whose every executable byte is covered by one hash.

## Files

The folder has a boundary now, and it is structural rather than a convention:
**`src/` is what the artefact is made of, `dist/` is what is deployed, and
nothing else in here is served.** Before there was a build step, the whole
folder was published as it stood; now "which files are served" is no longer
"all of them", and a layout that still implied otherwise would invite someone
to deploy the root — a multi-file page whose hash nobody verified.

| Path | Role |
| --- | --- |
| `src/` | The signing page: `sign.html`, `sign.js`, `sheet.css` (the app's tokens, copied), `fonts.css` (the app's face, generated by `samples/gen-fonts.mjs`), `lib/`. This, and only this, is what the artefact contains. |
| `dist/` | **What is deployed** — an index and every published version at `b/<sha256>/sign.html`. In git, so a published path cannot vanish by accident. Built by `bun samples/build-single.mjs`. |
| `demo/` | The self-check page (`index.html`), the fixture board (`gallery.html`) and the icons. Development surfaces; not published with the signer. |
| `samples/` | Harnesses — Node/Bun only, for the maintainer. |

`src/sign.html` names no subresource but its own scripts and stylesheet: it
carries no favicon, because the artefact carries none, and a source page that
referenced something the shipped page does not would be a difference between
what is tested and what is sent.

`icons/icon.svg` is a copy of the canonical mark at `docs/design/icon/app-icon.svg`.
Regenerate the PNGs after changing it:

```sh
for s in 16 32 48 128; do
  magick -background none -density 600 icons/icon.svg -resize ${s}x${s} PNG32:icons/$s.png
done
```
