---
title: 自行架設指南
description: "Vela 為你運行的一切、每個部分的用途，以及如何換成你自己的——中繼、公鑰索引、鏈數據、匯率和各個 App；還有你唯一無法替換的東西，以及沒有 getvela.app 時如何繼續使用。"
source: 5ae6005a9396
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# 自行架設指南

你的錢在鏈上的 Safe 合約裏，由你的鑰匙控制。Vela 運行的任何東西都無法轉移它。Vela 運行的，是讓錢包用起來
方便的那套機制：替你提交交易的中繼、幫助新裝置找到你錢包的索引、鏈數據目錄、匯率來源，以及各個 App 本身。

本頁逐一列出這些部分：沒有它會有甚麼失效，以及如何自行運行。本頁也會說明你唯一無法替換的部分——你的
通行密鑰所屬的域名——以及 getvela.app 消失時應該怎麼辦。

<Callout type="info" title="本頁適合誰">
你應該熟悉終端機、Docker 或 Cloudflare Workers，也懂得為鏈上地址注資。日常使用 Vela 完全不需要這裏的內容。
</Callout>

## 全貌

| 部分 | 用途 | Vela 的預設 | 可以替換嗎？ | 沒有它時 |
| --- | --- | --- | --- | --- |
| **中繼** | 接收你已簽署的操作，墊付 gas 並提交，收取你簽署同意的手續費 | `vela-relay-cf.getvela.app` | 可以——運行 [vela-relay](#relay)，並把錢包指向它 | 你無法轉賬 |
| **公鑰索引** | 把新錢包的鑰匙登記上鏈；回答「這把鑰匙屬於哪個錢包？」 | `p256-index-v2.getvela.app` | 可以——運行 [p256-index](#index) | 無法建立新錢包；登入改為直接讀取鏈上資料 |
| **註冊表合約** | 每個錢包鑰匙的永久公開紀錄 | Gnosis 上的 `0x94fD…1EA9` | 不需要——它沒有擁有者；錢包會直接讀取 | — |
| **鏈數據** | 網絡資料、代幣列表、標誌、清晰簽署描述檔 | `ethereum-data.getvela.app` | 可以——運行 [ethereum-data](#chain-data) | 沒有代幣列表或標誌；能解碼的交易減少；無法加入網絡 |
| **匯率** | 以你的顯示貨幣計算的法定貨幣價值 | `vela-currency.getvela.app` | 可以——運行 [vela-currency](#exchange-rates) 或任何兼容 Frankfurter 的來源 | App 會盡可能改用鏈上的 Chainlink 匯率（桌面版顯示美元） |
| **RPC 節點** | 讀取餘額、模擬交易 | 每條網絡的公共端點 | 可以——按網絡在「設定 → 網絡」中設定 | Vela 會在端點之間自動切換 |
| **App** | 錢包本身 | wallet.getvela.app、正式發佈版本 | 可以——[自行編譯](#web-app) | — |
| **getvela.app** | 你的通行密鑰所屬的域名 | — | **不可以**——見[下文](#if-getvela-app-disappears) | — |

另外還會連接幾個不屬於 Vela 的第三方服務：解碼交易時作為最後手段的公共函數選擇器資料庫（sourcify、
openchain、4byte）、用來顯示你安全密鑰型號名稱的驗證器目錄，以及你掃描 QR 碼用手機簽署時經過的 Apple
和 Google 隧道伺服器。

## 你唯一無法替換的：通行密鑰的域名

<span id="if-getvela-app-disappears"></span>

通行密鑰屬於建立它的那個網站。Vela 的鑰匙是為 `getvela.app` 建立的。瀏覽器只會把它們提供給 getvela.app
或其子域名上的頁面（或 getvela.app 聲明為相關的來源），而手機內置的通行密鑰只能在 getvela.app 認可的 App
中使用。在瀏覽器以外，規則較寬鬆：Chrome 容許獲得 getvela.app 權限的擴充功能使用它們，而你電腦上的程式
可以直接向安全密鑰或手機請求 getvela.app 簽名——自行編譯的 App 正是這樣運作的，這也是為甚麼你運行甚麼
軟件很重要。由此得出兩點。

**把網頁版錢包部署到你自己的域名上，得到的是另一個錢包。**同一份程式碼在 `wallet.example.com` 上運行時，
會為 `wallet.example.com` 建立通行密鑰——新的鑰匙，因此是新的地址。它無法為在 wallet.getvela.app 建立的
錢包簽署。這份副本仍然有用：可用於在它上面建立的錢包，或從零開始自行運行整套系統。

**對於現有的錢包，即使 getvela.app 下線或消失，以下方式仍然可用：**

| 途徑 | 可以使用的鑰匙 | 從哪裏取得 |
| --- | --- | --- |
| **Vela 瀏覽器擴充功能**（Chromium 瀏覽器：Chrome、Edge、Brave） | 瀏覽器能接觸到的任何鑰匙：本裝置的通行密鑰、USB 安全密鑰（電腦支援時也可用 NFC）、透過 QR 碼連接的手機 | [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases) 上的發佈壓縮檔，或[自行編譯](#web-app) |
| **你自行編譯的桌面版或手機 App** | 透過 QR 碼連接的手機，以及 USB 安全密鑰 | [自行編譯](#web-app) |
| **應用程式商店版和經過公證的桌面版** | 透過 QR 碼連接的手機和安全密鑰始終可用；「本裝置」通行密鑰只在作業系統仍能以 getvela.app 驗證該 App 時才可用 | GitHub 發佈頁面（稍後上架應用程式商店） |

擴充功能之所以能使用 `getvela.app` 的鑰匙，是因為 Chrome 容許獲得某網站權限的擴充功能使用該網站的通行
密鑰。瀏覽器會在本機檢查這項權限；我們實測過它可以運作，但還未在域名真正下線的情況下測試。自行編譯的
App 之所以能使用手機或安全密鑰，是因為 Vela 直接與它們溝通；而手機本身的通行密鑰（「本裝置」）需要 App
由 Vela 簽署，你編譯的版本並不是。

[簽名頁](/zh-HK/docs/clear-signing-self-host)本身並不是一條獨立的途徑：它只會為其他程式傳來的請求簽署，
而目前還沒有任何 Vela App 會發送請求給它。

<Callout type="warning" title="控制域名的人可以請求簽名">
任何由 getvela.app 或其子域名提供的頁面——或將來控制這個域名的任何人——都可以請求你的鑰匙簽署，而系統
提示顯示的是「getvela.app」，而不是交易內容。所有通行密鑰都是這樣運作的。正因如此，Vela 的網站禁止自己
的頁面使用通行密鑰。這也是擴充功能和自行編譯的 App 重要的原因：它們自帶程式碼，不過在預設情況下，它們
仍會從 getvela.app 下的服務取得描述檔，並使用那裏的服務。
</Callout>

## 把錢包指向你的服務

每個 App 在**設定 → 進階 → 服務端點**（桌面版為**設定 → 服務端點**）下都有四個欄位：「區塊鏈數據索引」、
「通行密鑰索引」、「VELA RELAY」和「法定貨幣匯率」。在你更改之前，每個欄位都顯示 Vela 的預設值；**重設為預設值**會把四個欄位
一併還原。對於中繼、索引和鏈數據，錢包會呼叫 `/api/health` 並顯示一個狀態標記，只有在端點回報正確的服務
名稱並回報 `status: "ok"` 時才會變成綠色。無論標記是甚麼顏色，你輸入的內容都會被儲存——請等它變綠。

| 服務 | `/api/health` 中的 `service` |
| --- | --- |
| 中繼 | `vela-relay` |
| 公鑰索引 | `webauthn-p256-publickey-registry` |
| 鏈數據 | `ethereum-data` |
| 匯率 | 不按名稱檢查——必須傳回以美元為基準的匯率列表 |

目前各個 App 遵從這些設定的程度：

| App | 服務端點 | 按網絡設定 RPC |
| --- | --- | --- |
| 網頁版和擴充功能 | 鏈數據、中繼和法定貨幣匯率。通行密鑰索引會用於查詢名稱，但建立錢包和登入仍然使用 Vela 的索引 | 可以 |
| 桌面版 | 全部四項；新的通行密鑰索引在你重新啟動或登出後生效 | 可以 |
| Android | 全部四項，但按地址查詢名稱時仍會詢問 Vela 的索引 | 可以 |
| iOS | **尚未支援**：頁面顯示的是預留值，也不會儲存。預設索引無法連接時，可以在登入畫面更改通行密鑰索引 | 唯讀 |

這些缺口都是程式錯誤，已記錄在案。

## 運行你自己的中繼

<span id="relay"></span>

中繼就是 [vela-relay](https://github.com/mondaylabsltd/vela-relay)（Rust，MIT 授權）。一次部署即可服務
所有鏈：錢包會呼叫 `https://your-relay/<chainId>`。它必須是 vela-relay——錢包用一個 Vela 專屬的方法索取
手續費報價，通用的 ERC-4337 打包器並沒有實作這個方法。

**你需要**

- Docker，加上你已在運行的 Redis 和 [Iggy](https://iggy.apache.org) 伺服器；或者一個使用 **Workers Paid**
  付費方案的 Cloudflare 帳戶，並在你的電腦上安裝 Node.js 和 Rust 工具鏈（包括 `wasm32-unknown-unknown`
  目標）。
- 一個 `OPERATOR_SECRET`（十六進制，至少 32 位元組）。它會衍生出一個資金庫地址和一組中繼地址，在每條鏈上
  都相同。請務必保密：它控制着中繼的資金。
- 你要服務的每條鏈上都要有 gas：把該鏈的原生幣（Tempo 上為 pathUSD）轉到你的資金庫地址。資金庫會為各個
  中繼地址補充 gas。

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# 在 .env 中設定 VELA_RELAY_IGGY_URL、VELA_RELAY_REDIS_URL、OPERATOR_SECRET，
# 如自行運行鏈數據，再設定 VELA_RELAY_CHAIN_DIRECTORY_URL，
# 並把 VELA_RELAY_IMAGE 設為你信任的發佈映像（見 docs/docker.md）
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

建議使用已發佈的映像：用 `docker compose up --build` 從原始碼建置，在目前的 Dockerfile 下可能會失敗。
不用 Docker 的話，`cargo run --release --bin vela-relay` 可以直接運行。

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# 自行運行鏈數據：在 wrangler.jsonc 的 "vars" 中加入 "VELA_RELAY_CHAIN_DIRECTORY_URL"
npx wrangler deploy
```

**檢查**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # 你在 Gnosis 上的資金庫地址，以及是否需要補充 gas
```

然後把 `https://your-relay` 填入「VELA RELAY」欄位。

**須知**

- 錢包支付的手續費會進入你的資金庫。無論你用哪個中繼，錢包計算手續費的方式都一樣
  （見[網絡與費用](/zh-HK/docs/networks-and-fees)）。
- 在更換中繼之前加入的自訂網絡，會沿用加入時的中繼地址。
- 中繼會從鏈目錄讀取每條鏈的資料和它接受的穩定幣：預設是 `ethereum-data.getvela.app`，
  把 `VELA_RELAY_CHAIN_DIRECTORY_URL` 設為[你自己的鏈數據](#chain-data)即可替換。
  這個設定於 2026 年 9 月加入；較舊的中繼版本一律讀取 Vela 的那一份。

## 運行你自己的公鑰索引

<span id="index"></span>

索引就是 [p256-index](https://github.com/mondaylabsltd/p256-index)（Rust，MIT 授權）。建立錢包時，它會檢查每把鑰匙
的證明，然後把這組鑰匙寫入 Gnosis 上的**註冊表合約**並支付 gas。請繼續使用現有的註冊表
`0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`：它沒有擁有者，任何有餘額的地址都可以寫入，而每個 Vela App
都會直接讀取它。你自己另建的註冊表，這些 App 是看不到的。

**你需要**

- Docker 加上 Redis 和 Iggy（伺服器版本），或一個 Cloudflare 帳戶（Worker 版本；它自己的 README 註明其
  鏈上寫入功能尚未經過端對端測試）。
- 一把有 xDAI 的 Gnosis 私鑰。登記一個錢包，一把鑰匙約需 110 萬 gas，七把鑰匙約需 360 萬 gas。
- 以下設定：

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

雖然伺服器的範例檔案沒有列出 `P256_INDEX_DOMAIN_REGISTRY`，但它不可或缺：缺少它，伺服器發出的挑戰會被
合約拒絕，所有登記都會失敗。

**運行與檢查**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

伺服器以普通 HTTP 監聽（預設連接埠 11256）；由於錢包只接受 `https://` 端點，請在它前面加一個 TLS 代理。
截至本文撰寫時，原始碼中的 Dockerfile 可能無法建置；用 Cargo 建置則沒有問題。

**如果完全沒有索引回應**，現有的錢包仍然可以使用：登入時，App 會透過你的 RPC 節點讀取 Gnosis（然後是
以太坊）上的註冊表合約。只有一把鑰匙的錢包，甚至可以不經註冊表，憑兩次簽名重建。建立新錢包則確實需要
索引，因為登記總要有人付費。

## 運行你自己的鏈數據

<span id="chain-data"></span>

鏈數據就是 [ethereum-data](https://github.com/atshelchin/ethereum-data)（MIT）：約 2,600 條網絡及其代幣的
靜態 JSON 和圖片，以及 Vela 用來解釋交易的 ERC-7730 描述檔。

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

它的 README 也說明了如何從原始碼建置及部署到 Cloudflare。請以 HTTPS 提供服務，並把地址填入「區塊鏈數據索引」欄位。

中繼也會讀取這些檔案，包括一個 Vela 專屬的欄位（`stables` 列表決定哪些穩定幣可以支付手續費）。
用 `VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data` 讓它改為讀取你的那一份；
每條網絡的資料，它會快取一小時。

## 運行你自己的匯率服務

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency)（MIT）轉載歐洲中央銀行每日公佈的匯率，
不需要任何 API 密鑰。

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

把 `https://your-host/v2/rates?base=USD` 填入「法定貨幣匯率」欄位。任何兼容 Frankfurter 的服務也可以。
請保留 `?base=USD`：所有換算都以此為前提。

## 自行編譯 App

<span id="web-app"></span>

所有 App 都在[同一個程式碼庫](https://github.com/mondaylabsltd/vela-wallet)中（MIT）。README 列出了每個
App 的編譯步驟；簡要版本如下：

| App | 編譯方法 | 能否為你在 getvela.app 的現有錢包簽署？ |
| --- | --- | --- |
| 瀏覽器擴充功能 | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`，然後在 `chrome://extensions` 以未封裝方式載入 `extension/dist` | 可以，任何鑰匙都行 |
| 網頁版錢包 | `cd app-web/vela-wallet && pnpm install && pnpm build`；以 Cloudflare Worker 形式部署 | 不可以——在你的域名上它是另一個錢包（見上文） |
| 桌面版 | `cd app-desktop/vela-wallet && cargo run`（打包腳本見其 README） | 可以，用透過 QR 碼連接的手機或 USB 安全密鑰 |
| Android | 先產生核心綁定（bindings），然後執行 `./gradlew :app:installDebug` | 可以，用透過 QR 碼連接的手機或 USB 安全密鑰 |
| iOS | `./rust/scripts/build-ios-xcframework.sh`，然後用你自己的開發團隊在 Xcode 中編譯 | 可以，用透過 QR 碼連接的手機或 USB-C / Lightning 接口的 YubiKey（韌體 5.8 或以上） |

自行編譯的 App 無法用「本裝置」通行密鑰為 getvela.app 錢包簽署：Apple 和 Google 只容許由 Vela 簽署的
App 使用 `getvela.app` 通行密鑰。

## 加入 Vela 沒有內置的網絡

任何具備 P-256 預編譯合約，以及 Vela 所檢查的標準合約的 EVM 鏈，都可以運行 Vela。[鏈設定](/zh-HK/chain-setup)
會告訴你某條鏈缺少甚麼，並部署任何人都能部署的部分；[網絡與費用](/zh-HK/docs/networks-and-fees)說明了具體
要求。有一個缺口：有多於一把鑰匙的錢包，還需要那條鏈上有 Safe 的通行密鑰簽署器工廠，而目前的檢查尚未
涵蓋這一點——沒有它，在那條鏈上只有第一把鑰匙能簽署。

## 全部替換後，還有甚麼指向 Vela

即使你替換了以上所有部分，仍會剩下這些：

- **用來顯示安全密鑰型號的驗證器目錄**——只影響顯示；App 會改為顯示通用名稱。
- **getvela.app 的關聯檔案**——應用程式商店版 App 使用「本裝置」通行密鑰時需要它們。手機或安全密鑰則不需要。

而以下這些不屬於 Vela：公共函數選擇器資料庫、Apple 和 Google 用於手機簽署的隧道，以及你選擇的任何 RPC
服務供應商。

下一步：[你可以自行運行的簽名頁](/zh-HK/docs/clear-signing-self-host)。
