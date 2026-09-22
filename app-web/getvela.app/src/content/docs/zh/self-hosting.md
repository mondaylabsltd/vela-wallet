---
title: 自托管指南
description: "Vela 替你运行的每一样东西、各自的用途，以及如何换成你自己的——中继、公钥索引、链数据、汇率和各个 App；还有唯一无法替换的那一样，以及没有 getvela.app 时怎么办。"
source: de484cb33065
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# 自托管指南

你的钱在链上的 Safe 合约里，由你的钥匙控制。Vela 运行的任何东西都动不了它。
Vela 运行的，是让钱包用起来方便的那套机器：替你提交交易的中继、帮新设备找回钱包的
索引、链数据目录、汇率源，以及各个 App 本身。

这一页把这些部件逐一列出来：每样是做什么的、没有它会怎样、怎么换成你自己的。
也讲清楚唯一无法替换的那一样——你的通行密钥所属的域名——以及 getvela.app
不在了该怎么办。

<Callout type="info" title="这一页写给谁">
你需要会用终端、会用 Docker 或 Cloudflare Workers，也会给链上地址充值。
日常使用 Vela 完全用不到这里的内容。
</Callout>

## 全景

| 部件 | 做什么 | Vela 的默认实例 | 能换吗 | 没有它会怎样 |
| --- | --- | --- | --- | --- |
| **中继** | 接收你签好的操作，垫付 gas、提交上链，收取你签名同意的手续费 | `vela-relay-cf.getvela.app` | 能——运行 [vela-relay](#relay) 并让钱包指向它 | 无法发送交易 |
| **公钥索引** | 把新钱包的钥匙登记上链；回答“这把钥匙属于哪个钱包” | `p256-index-v2.getvela.app` | 能——运行 [p256-index](#index) | 无法创建新钱包；登录时改为直接读链 |
| **注册表合约** | 每个钱包钥匙的永久公开记录 | Gnosis 上的 `0x94fD…1EA9` | 不需要换——它没有所有者，钱包直接读取 | — |
| **链数据** | 网络信息、代币列表、图标、清晰签名描述文件 | `ethereum-data.getvela.app` | 能——运行 [ethereum-data](#chain-data) | 没有代币列表和图标；能解码的交易变少；无法添加网络 |
| **汇率** | 按你选择的法币显示金额 | `vela-currency.getvela.app` | 能——运行 [vela-currency](#exchange-rates) 或任何兼容 Frankfurter 的服务 | App 会尽量改用链上的 Chainlink 汇率（桌面版显示美元） |
| **RPC 节点** | 读取余额、模拟交易 | 各网络的公共节点 | 能——在“设置 → 网络”里按网络设置 | Vela 会在节点之间自动切换 |
| **App** | 钱包本身 | wallet.getvela.app、发布版安装包 | 能——[自己编译](#web-app) | — |
| **getvela.app** | 你的通行密钥所属的域名 | — | **不能**——见[下文](#if-getvela-app-disappears) | — |

另外还会连接几个不属于 Vela 的第三方服务：解码交易时最后才用到的公共函数选择器
数据库（sourcify、openchain、4byte）、用来显示安全密钥型号名称的认证器目录，
以及用手机扫码签名时经过的 Apple 和 Google 隧道服务器。

## 唯一无法替换的：通行密钥的域名

<span id="if-getvela-app-disappears"></span>

通行密钥属于创建它的那个网站。Vela 的钥匙是为 `getvela.app` 创建的。浏览器只会把它们提供给
getvela.app 及其子域名上的页面（或 getvela.app 声明为相关的来源），手机自带的通行密钥也只在
getvela.app 认可的 App 里可用。在浏览器之外，规则要宽松一些：Chrome 允许获得 getvela.app 权限的
扩展使用它们，而你电脑上的程序可以直接向安全密钥或手机请求一个 getvela.app 签名——自编译 App 正是
这样工作的，这也是为什么你运行什么软件很重要。由此有两个结论。

**把网页钱包部署到你自己的域名上，得到的是另一个钱包。**同一份代码放在
`wallet.example.com` 上，创建的是属于 `wallet.example.com` 的通行密钥——新的钥匙，
因此是新的地址。它无法给在 wallet.getvela.app 创建的钱包签名。这份副本仍然有用：
用于在它上面新建的钱包，或者从零开始完整运行你自己的一套。

**对于已有的钱包，即使 getvela.app 下线或消失，下面这些方式依然可用：**

| 方式 | 能用哪些钥匙 | 从哪里获得 |
| --- | --- | --- |
| **Vela 浏览器扩展**（Chromium 系浏览器：Chrome、Edge、Brave） | 浏览器能用到的任何钥匙：本设备的通行密钥、USB 安全密钥（电脑支持时也可用 NFC）、扫码连接的手机 | [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases) 上的发布包，或[自己编译](#web-app) |
| **你自己编译的桌面或手机 App** | 扫码连接的手机、USB 安全密钥 | [自己编译](#web-app) |
| **商店版和经过公证的桌面版** | 扫码手机和安全密钥始终可用；“本设备”通行密钥只在操作系统还能把 App 与 getvela.app 对上时可用 | GitHub 发布页（之后上架商店） |

扩展能用 `getvela.app` 的钥匙，是因为 Chrome 允许获得某网站权限的扩展使用该网站
的通行密钥。这项权限由浏览器在本地检查；我们实测过它可用，但还没有在域名真正下线的情况下测试过。自己编译的 App 能用手机和安全密钥，是因为 Vela 直接与它们通信；而手机
自带的通行密钥（“本设备”）要求 App 由 Vela 签名，你编译的版本不是。

[签名页](/zh/docs/clear-signing-self-host)本身不是一条独立的路：它只给别的程序发来的
请求签名，而目前还没有任何 Vela App 会向它发送请求。

<Callout type="warning" title="控制域名的人就能请求签名">
getvela.app 或其任何子域名上的页面——或将来控制这个域名的人——都可以请求你的钥匙签名，而系统弹窗
显示的是“getvela.app”，不是交易内容。所有通行密钥都是这样工作的。正因如此，Vela 的网站禁止自己的
页面使用通行密钥。这也是扩展和自编译 App 重要的原因：它们自带代码，不过默认情况下仍会从
getvela.app 下的服务获取描述文件、使用那里的服务。
</Callout>

## 让钱包指向你的服务

每个 App 在 **设置 → 高级 → 服务端点**（桌面版为 **设置 → 服务端点**）下有四个字段：链数据、通行密钥索引、
Vela 中继、法币汇率。在你修改之前，字段显示的是 Vela 的默认值；**恢复默认** 会把
四个一起还原。对中继、索引和链数据，钱包会请求 `/api/health` 并显示一个状态标记，
只有返回了正确的服务名且 `status` 为 `"ok"` 时才是绿色。无论标记是什么颜色，你输入的
内容都会被保存——请等它变绿。

| 服务 | `/api/health` 中的 `service` |
| --- | --- |
| 中继 | `vela-relay` |
| 公钥索引 | `webauthn-p256-publickey-registry` |
| 链数据 | `ethereum-data` |
| 汇率 | 不按名称校验——必须返回以美元为基准的汇率列表 |

目前各 App 对这些设置的支持情况：

| App | 服务端点 | 按网络设置 RPC |
| --- | --- | --- |
| 网页版和扩展 | 链数据、中继和法币汇率生效。通行密钥索引用于按地址查名字，但创建钱包和登录仍然使用 Vela 的索引 | 支持 |
| 桌面版 | 四个都生效；新的通行密钥索引要重启或退出登录后才生效 | 支持 |
| Android | 四个都生效，但“按地址查名字”仍然询问 Vela 的索引 | 支持 |
| iOS | **暂不可用**：页面显示的是占位数据，也不会保存。默认索引连不上时，可以在登录界面修改通行密钥索引 | 只读 |

这些不足都是缺陷，已经记录在案。

## 运行你自己的中继

<span id="relay"></span>

中继就是 [vela-relay](https://github.com/mondaylabsltd/vela-relay)（Rust，MIT 许可）。
一个部署服务所有链：钱包调用 `https://your-relay/<chainId>`。必须是 vela-relay——
钱包用一个 Vela 专有的方法获取手续费报价，通用的 ERC-4337 打包器没有实现它。

**你需要**

- Docker，外加你已经在运行的 Redis 和 [Iggy](https://iggy.apache.org) 服务；或者一个
  **Workers Paid** 付费套餐的 Cloudflare 账号，并在本机装好 Node.js 和带
  `wasm32-unknown-unknown` 目标的 Rust 工具链。
- 一个 `OPERATOR_SECRET`（十六进制，至少 32 字节）。它派生出一个金库地址和一组
  中继地址，在每条链上都相同。务必保密：它控制着中继的资金。
- 你要服务的每条链上都要有 gas：把该链的原生币（Tempo 上是 pathUSD）转到你的
  金库地址，金库会自动给各个中继地址补充。

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# 在 .env 中填写 VELA_RELAY_IGGY_URL、VELA_RELAY_REDIS_URL、OPERATOR_SECRET，
# 自己运行链数据的话再填 VELA_RELAY_CHAIN_DIRECTORY_URL，
# 并把 VELA_RELAY_IMAGE 设为你信任的发布镜像（见 docs/docker.md）
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

建议用已发布的镜像：用 `docker compose up --build` 从源码构建，在当前的 Dockerfile
下可能会失败。不用 Docker 的话，`cargo run --release --bin vela-relay` 可以直接运行。

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# 自己的链数据：在 wrangler.jsonc 的 "vars" 里加上 "VELA_RELAY_CHAIN_DIRECTORY_URL"
npx wrangler deploy
```

**检查**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # 你在 Gnosis 上的金库地址，以及是否需要补 gas
```

然后把 `https://your-relay` 填进 **Vela 中继** 字段。

**需要知道的**

- 钱包支付的手续费进入你的金库。无论用哪个中继，钱包计算手续费的方式都一样
  （见[网络与手续费](/zh/docs/networks-and-fees)）。
- 在更换中继之前添加的自定义网络，会继续使用添加时记录的中继地址。
- 中继从链目录读取每条链的信息和它接受的稳定币：默认是 `ethereum-data.getvela.app`，
  把 `VELA_RELAY_CHAIN_DIRECTORY_URL` 设为[你自己的链数据](#chain-data)即可替换。这个设置在
  2026 年 9 月加入；更早的中继版本只会读 Vela 的那一份。

## 运行你自己的公钥索引

<span id="index"></span>

索引就是 [p256-index](https://github.com/mondaylabsltd/p256-index)（Rust，MIT 许可）。创建钱包时，
它逐一检查每把钥匙的证明，然后把这组钥匙写进 Gnosis 上的**注册表合约**，并支付
gas。请继续使用现有的注册表 `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`：它没有
所有者，任何有余额的地址都能写入，而每个 Vela App 都会直接读取它。你另建的注册表，
它们是看不到的。

**你需要**

- Docker 加 Redis 和 Iggy（服务端版本），或者一个 Cloudflare 账号（Worker 版本；它自己的
  README 说明链上写入尚未做过端到端测试）。
- 一个有 xDAI 的 Gnosis 私钥。登记一个钱包，一把钥匙约需 110 万 gas，七把约 360 万。
- 以下配置：

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

虽然服务端的示例配置文件里没有 `P256_INDEX_DOMAIN_REGISTRY`，但它必不可少：缺了它，
服务端发出的挑战会被合约拒绝，所有登记都会失败。

**运行与检查**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

服务端监听的是普通 HTTP（默认端口 11256）；钱包只接受 `https://` 端点，所以要在前面加一个
TLS 代理。截至本文撰写时，源码里的 Dockerfile 可能无法构建；用 Cargo 构建没有问题。

**如果完全没有索引可用**，已有的钱包照样能用：登录时 App 会通过你的 RPC 节点直接读取
Gnosis（然后是以太坊）上的注册表合约。只有一把钥匙的钱包，甚至可以不经过注册表，
凭两次签名重建出来。创建新钱包则确实需要索引，因为登记总得有人付费。

## 运行你自己的链数据

<span id="chain-data"></span>

链数据就是 [ethereum-data](https://github.com/atshelchin/ethereum-data)（MIT）：约 2600
条网络及其代币的静态 JSON 和图片，外加 Vela 用来解释交易的 ERC-7730 描述文件。

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

它的 README 也讲了如何从源码构建和部署到 Cloudflare。用 HTTPS 对外提供服务，然后把地址
填进 **链数据** 字段。

中继也读取这些文件，包括一个 Vela 专有的字段（`stables` 列表决定哪些稳定币能付手续费）。
用 `VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data` 让它读你的这一份；每条网络的信息
它会缓存一小时。

## 运行你自己的汇率服务

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency)（MIT）转发欧洲央行每天
公布的汇率，不需要任何密钥。

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

把 `https://your-host/v2/rates?base=USD` 填进 **法币汇率** 字段。任何兼容 Frankfurter 的
服务也可以。一定要保留 `?base=USD`：所有换算都以它为前提。

## 自己编译 App

<span id="web-app"></span>

所有 App 都在[同一个仓库](https://github.com/mondaylabsltd/vela-wallet)里（MIT）。
README 里有每个 App 的编译步骤，这里是简版：

| App | 编译 | 能给你在 getvela.app 已有的钱包签名吗 |
| --- | --- | --- |
| 浏览器扩展 | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`，然后在 `chrome://extensions` 以“加载已解压的扩展程序”加载 `extension/dist` | 能，任何钥匙都行 |
| 网页钱包 | `cd app-web/vela-wallet && pnpm install && pnpm build`；以 Cloudflare Worker 部署 | 不能——在你的域名上它是另一个钱包（见上文） |
| 桌面版 | `cd app-desktop/vela-wallet && cargo run`（打包脚本见其 README） | 能，用扫码手机或 USB 安全密钥 |
| Android | 先生成核心绑定，再 `./gradlew :app:installDebug` | 能，用扫码手机或 USB 安全密钥 |
| iOS | `./rust/scripts/build-ios-xcframework.sh`，然后用你自己的开发者团队在 Xcode 中编译 | 能，用扫码手机或 USB-C / Lightning 接口的 YubiKey（固件 5.8 及以上） |

自己编译的 App 无法用“本设备”的通行密钥给 getvela.app 钱包签名：Apple 和 Google 只允许
由 Vela 签名的 App 使用 `getvela.app` 的通行密钥。

## 添加 Vela 没有内置的网络

任何支持 P-256 预编译、并部署了它所检查的那些标准合约的 EVM 链都能运行 Vela。
[链设置](/zh/chain-setup)会告诉你一条链缺什么，并部署任何人都能部署的那部分；
[网络与手续费](/zh/docs/networks-and-fees)解释了具体要求。有一个缺口：多把钥匙的钱包还需要那条链上有
Safe 的通行密钥签名器工厂，而目前的检查还不会查它——没有它，在那条链上只有第一把钥匙能签名。

## 全部换掉之后，还剩哪些指向 Vela

把上面的都换成你自己的，还会剩下这些：

- **显示安全密钥型号的认证器目录**——只影响显示；连不上时 App 会显示通用名称。
- **getvela.app 上的关联文件**——商店版 App 使用“本设备”通行密钥时需要它们。扫码手机和
  安全密钥用不到。

以下这些不属于 Vela：公共函数选择器数据库、Apple 和 Google 的手机扫码隧道，以及你自己
选择的 RPC 服务商。

下一步：[可以自己运行的签名页](/zh/docs/clear-signing-self-host)。
