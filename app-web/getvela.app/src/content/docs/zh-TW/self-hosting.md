---
title: 自架指南
description: "Vela 替你執行的每一樣東西、各自的用途，以及怎麼換成你自己的——中繼、公鑰索引、鏈資料、匯率和各個應用程式；還有唯一無法取代的那一樣，以及沒有 getvela.app 時該怎麼辦。"
source: 5ae6005a9396
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# 自架指南

你的錢在鏈上的 Safe 合約裡，由你的金鑰控制。Vela 執行的任何東西都動不了它。
Vela 執行的，是讓錢包用起來方便的那套機制：替你送出交易的中繼、幫新裝置找回錢包的
索引、鏈資料目錄、匯率來源，以及各個應用程式本身。

這一頁把這些元件逐一列出：每一樣在做什麼、少了它會怎樣、怎麼換成你自己的。
也會說明唯一無法取代的那一樣——你的密碼金鑰所屬的網域——以及 getvela.app
不在了該怎麼辦。

<Callout type="info" title="這一頁寫給誰">
你需要會用終端機、Docker 或 Cloudflare Workers，也會替鏈上的位址儲值。
日常使用 Vela 完全用不到這裡的內容。
</Callout>

## 全貌

| 元件 | 做什麼 | Vela 的預設 | 能換掉嗎 | 少了它會怎樣 |
| --- | --- | --- | --- | --- |
| **中繼** | 接收你簽好的操作，墊付 gas、送上鏈，收取你簽署同意的手續費 | `vela-relay-cf.getvela.app` | 能——架設 [vela-relay](#relay)，並讓錢包指向它 | 無法發送交易 |
| **公鑰索引** | 把新錢包的金鑰註冊上鏈；回答「這把金鑰屬於哪個錢包？」 | `p256-index-v2.getvela.app` | 能——架設 [p256-index](#index) | 無法建立新錢包；登入時改為直接讀鏈 |
| **註冊表合約** | 每個錢包金鑰的永久公開紀錄 | Gnosis 上的 `0x94fD…1EA9` | 不需要換——它沒有擁有者，錢包會直接讀取 | — |
| **鏈資料** | 網路資訊、代幣清單、圖示、清晰簽署描述檔 | `ethereum-data.getvela.app` | 能——架設 [ethereum-data](#chain-data) | 沒有代幣清單和圖示；能解碼的交易變少；無法新增網路 |
| **匯率** | 以你的顯示幣別呈現法幣價值 | `vela-currency.getvela.app` | 能——架設 [vela-currency](#exchange-rates)，或任何相容 Frankfurter 的來源 | 應用程式會盡量改用鏈上的 Chainlink 匯率（桌面版會顯示美元） |
| **RPC 節點** | 讀取餘額、模擬交易 | 各網路的公共節點 | 能——在「設定 → 網路」裡逐一設定 | Vela 會在節點之間自動切換 |
| **應用程式** | 錢包本身 | wallet.getvela.app、正式發布版本 | 能——[自行建置](#web-app) | — |
| **getvela.app** | 你的密碼金鑰所屬的網域 | — | **不能**——請看[下文](#if-getvela-app-disappears) | — |

另外還會連到幾個不屬於 Vela 的第三方服務：解碼交易時最後才會用到的公開函式選擇器
資料庫（sourcify、openchain、4byte）、用來顯示安全金鑰型號名稱的驗證器目錄，
以及用手機掃描 QR 碼簽署時會經過的 Apple 和 Google 通道伺服器。

## 唯一無法取代的：密碼金鑰的網域

<span id="if-getvela-app-disappears"></span>

密碼金鑰屬於建立它的那個網站。Vela 的金鑰是為 `getvela.app` 建立的。瀏覽器只會把它們提供給
getvela.app 和它的子網域上的頁面（或 getvela.app 宣告為相關的來源），手機內建的密碼金鑰也只能在
getvela.app 認可的應用程式裡使用。在瀏覽器之外，規則寬鬆一些：Chrome 允許取得 getvela.app 權限的
擴充功能使用它們，而你電腦上的程式可以直接向安全金鑰或手機要一個給 getvela.app 的簽章——自行建置的
應用程式就是這樣運作的，這也是為什麼你執行什麼軟體很重要。由此可以得出兩點。

**把網頁錢包放到你自己的網域上，得到的是另一個錢包。** 同一份程式碼放在
`wallet.example.com` 上，建立的就是屬於 `wallet.example.com` 的密碼金鑰——新的金鑰，
所以也是新的位址。它無法替在 wallet.getvela.app 建立的錢包簽署。這份副本還是有用的：
可以用在你在它上面新建的錢包，或是從零開始自己跑整套服務。

**對已經存在的錢包來說，即使 getvela.app 下線或消失，下面這些方式仍然能用：**

| 方式 | 能用哪些金鑰 | 從哪裡取得 |
| --- | --- | --- |
| **Vela 瀏覽器擴充功能**（Chromium 系列瀏覽器：Chrome、Edge、Brave） | 瀏覽器用得到的任何金鑰：這台裝置的密碼金鑰、USB 安全金鑰（電腦支援時也可以用 NFC）、掃描 QR 碼連線的手機 | [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases) 上的發布壓縮檔，或[自行建置](#web-app) |
| **你自己建置的桌面版或手機應用程式** | 掃描 QR 碼連線的手機，以及 USB 安全金鑰 | [自行建置](#web-app) |
| **商店版和通過公證的桌面版** | 掃描 QR 碼的手機和安全金鑰一定能用；「這台裝置」的密碼金鑰，只有在作業系統還能拿 getvela.app 驗證這個應用程式時才能用 | GitHub 發布頁面（之後會上架商店） |

擴充功能能用 `getvela.app` 的金鑰，是因為 Chrome 允許取得某個網站權限的擴充功能使用該網站
的密碼金鑰。這項權限由瀏覽器在本機檢查；我們實測過它能用，但還沒有在網域真正下線的情況下測過。
自行建置的應用程式能用手機或安全金鑰，是因為 Vela 直接和它們溝通；手機本身的密碼金鑰（「這台裝置」）
則要求應用程式由 Vela 簽署，而你建置的版本不是。

[簽署頁](/zh-TW/docs/clear-signing-self-host)本身不是一條獨立的路：它只替其他程式送來的
請求簽署，而目前還沒有任何 Vela 應用程式會把請求送給它。

<Callout type="warning" title="控制網域的人就能要求簽章">
getvela.app 或它任何子網域上的頁面——或將來控制這個網域的人——都能要求你的金鑰簽署，而系統跳出的提示
顯示的是「getvela.app」，不是交易內容。所有地方的密碼金鑰都是這樣運作的。正因如此，Vela 的網站禁止自己的
頁面使用密碼金鑰。這也是擴充功能和自行建置的應用程式之所以重要的原因：它們帶著自己的程式碼，不過預設仍會從
getvela.app 底下的服務取得描述檔、使用那裡的服務。
</Callout>

## 讓錢包指向你的服務

每個應用程式在「設定 → 進階 → 服務端點」（桌面版是「設定 → 服務端點」）下都有四個欄位：
鏈資料、密碼金鑰索引（畫面上顯示為「通行密鑰索引」）、Vela 中繼和法幣匯率。在你修改之前，每個欄位顯示的是
Vela 的預設值；「重設為預設值」會把四個一起還原。對中繼、索引和鏈資料，錢包會呼叫 `/api/health` 並顯示一個
狀態標記，只有端點回報了正確的服務名稱、而且 `status` 是 `"ok"` 時才會是綠色。不論標記是什麼顏色，
你輸入的內容都會被儲存——請等它變綠。

| 服務 | `/api/health` 裡的 `service` |
| --- | --- |
| 中繼 | `vela-relay` |
| 公鑰索引 | `webauthn-p256-publickey-registry` |
| 鏈資料 | `ethereum-data` |
| 匯率 | 不檢查名稱——必須回傳以美元為基準的匯率清單 |

目前各個應用程式對這些設定的支援程度：

| 應用程式 | 服務端點 | 各網路的 RPC |
| --- | --- | --- |
| 網頁版和擴充功能 | 鏈資料、中繼和法幣匯率。密碼金鑰索引會用來查名字，但建立錢包和登入仍然使用 Vela 的索引 | 支援 |
| 桌面版 | 四個都生效；新的密碼金鑰索引要在重新啟動或登出之後才會生效 | 支援 |
| Android | 四個都生效，只有「替位址查名字」仍然詢問 Vela 的索引 | 支援 |
| iOS | **還不行**：頁面顯示的是預留值，也不會儲存。預設索引連不上時，可以在登入畫面更改密碼金鑰索引 | 唯讀 |

這些缺口都是錯誤（bug），已經列入追蹤。

## 架設你自己的中繼

<span id="relay"></span>

中繼就是 [vela-relay](https://github.com/mondaylabsltd/vela-relay)（Rust，MIT 授權）。
一個部署就服務所有鏈：錢包會呼叫 `https://your-relay/<chainId>`。必須是 vela-relay——
錢包是用一個 Vela 專屬的方法取得手續費報價，通用的 ERC-4337 bundler 並沒有實作它。

**你需要**

- Docker，加上你已經在執行的 Redis 和 [Iggy](https://iggy.apache.org) 伺服器；或是一個
  **Workers Paid** 付費方案的 Cloudflare 帳號，並在你的電腦上裝好 Node.js 和 Rust 工具鏈
  （含 `wasm32-unknown-unknown` 目標）。
- 一組 `OPERATOR_SECRET`（十六進位，至少 32 位元組）。它會衍生出一個金庫位址和一組
  中繼位址，在每條鏈上都一樣。請務必保密：它控制著中繼的資金。
- 你要服務的每條鏈上都要有 gas：把該鏈的原生幣（Tempo 上是 pathUSD）轉到你的
  金庫位址，金庫會替各個中繼位址補充。

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# 在 .env 裡填入 VELA_RELAY_IGGY_URL、VELA_RELAY_REDIS_URL、OPERATOR_SECRET，
# 如果你自己架設鏈資料，再填 VELA_RELAY_CHAIN_DIRECTORY_URL，
# 並把 VELA_RELAY_IMAGE 設成你信任的發布映像檔（見 docs/docker.md）
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

建議使用已發布的映像檔：用 `docker compose up --build` 從原始碼建置，在目前的 Dockerfile
下可能會失敗。不用 Docker 的話，可以用 `cargo run --release --bin vela-relay` 直接執行。

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# 自己的鏈資料：在 wrangler.jsonc 的 "vars" 裡加上 "VELA_RELAY_CHAIN_DIRECTORY_URL"
npx wrangler deploy
```

**檢查**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # 你在 Gnosis 上的金庫位址，以及是否需要補 gas
```

接著把 `https://your-relay` 填進「VELA RELAY」欄位。

**你該知道的**

- 錢包支付的手續費會進到你的金庫。不論用哪個中繼，錢包計算手續費的方式都一樣
  （見[網路與費用](/zh-TW/docs/networks-and-fees)）。
- 在更換中繼之前新增的自訂網路，會沿用新增時記下的中繼位址。
- 中繼會從鏈目錄讀取每條鏈的資訊和它接受的穩定幣：預設是 `ethereum-data.getvela.app`，
  把 `VELA_RELAY_CHAIN_DIRECTORY_URL` 設成[你自己的鏈資料](#chain-data)就能換掉。
  這個設定是 2026 年 9 月加入的；更早的中繼版本一律讀取 Vela 的那一份。

## 架設你自己的公鑰索引

<span id="index"></span>

索引就是 [p256-index](https://github.com/mondaylabsltd/p256-index)（Rust，MIT 授權）。建立錢包時，
它會逐一檢查每把金鑰的證明，再把這組金鑰寫進 Gnosis 上的**註冊表合約**，並支付
gas。請繼續使用現有的註冊表 `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`：它沒有
擁有者，任何有餘額的位址都能寫入，而每個 Vela 應用程式都會直接讀取它。你另外建立的註冊表，
它們是看不到的。

**你需要**

- Docker 加上 Redis 和 Iggy（伺服器版本），或是一個 Cloudflare 帳號（Worker 版本；它自己的
  README 註明鏈上寫入還沒有做過端對端測試）。
- 一把有 xDAI 的 Gnosis 私鑰。註冊一個錢包，一把金鑰大約要 1.1M gas，七把大約 3.6M。
- 以下設定：

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

雖然伺服器的範例設定檔裡沒有 `P256_INDEX_DOMAIN_REGISTRY`，但它不可或缺：少了它，
伺服器發出的挑戰會被合約拒絕，每一筆註冊都會失敗。

**執行與檢查**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

伺服器監聽的是一般的 HTTP（預設連接埠 11256）；錢包只接受 `https://` 端點，所以要在前面加一層
TLS 代理。截至撰寫本文時，原始碼裡的 Dockerfile 可能建置不起來；用 Cargo 建置沒有問題。

**如果完全沒有索引可以回應**，已經存在的錢包照樣能用：登入時，應用程式會透過你的 RPC 節點直接讀取
Gnosis（接著是以太坊）上的註冊表合約。只有一把金鑰的錢包，甚至可以不經過註冊表，
憑兩次簽章重建出來。但建立新錢包確實需要索引，因為註冊總得有人付費。

## 架設你自己的鏈資料

<span id="chain-data"></span>

鏈資料就是 [ethereum-data](https://github.com/atshelchin/ethereum-data)（MIT）：大約 2,600
條網路和它們代幣的靜態 JSON 與圖片，再加上 Vela 用來解釋交易的 ERC-7730 描述檔。

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

它的 README 也說明了怎麼從原始碼建置、部署到 Cloudflare。請用 HTTPS 對外提供服務，再把位址
填進「鏈資料索引」欄位。

中繼也會讀取這些檔案，包括一個 Vela 專屬的欄位（`stables` 清單決定哪些穩定幣能付手續費）。
用 `VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data` 讓它改讀你的那一份；
每條網路的資料，它會快取一小時。

## 架設你自己的匯率服務

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency)（MIT）轉發歐洲中央銀行每天
公布的匯率，不需要任何金鑰。

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

把 `https://your-host/v2/rates?base=USD` 填進「法幣匯率」欄位。任何相容 Frankfurter 的
服務也可以。一定要保留 `?base=USD`：所有換算都以它為前提。

## 自行建置應用程式

<span id="web-app"></span>

所有應用程式都在[同一個儲存庫](https://github.com/mondaylabsltd/vela-wallet)裡（MIT）。
README 列出了每個應用程式的建置步驟，這裡是精簡版：

| 應用程式 | 建置 | 能替你在 getvela.app 已有的錢包簽署嗎？ |
| --- | --- | --- |
| 瀏覽器擴充功能 | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`，再到 `chrome://extensions` 用「載入未封裝項目」載入 `extension/dist` | 可以，任何金鑰都行 |
| 網頁錢包 | `cd app-web/vela-wallet && pnpm install && pnpm build`；以 Cloudflare Worker 部署 | 不行——放在你的網域上，它就是另一個錢包（見上文） |
| 桌面版 | `cd app-desktop/vela-wallet && cargo run`（打包腳本見它的 README） | 可以，用掃描 QR 碼的手機或 USB 安全金鑰 |
| Android | 先產生核心綁定，再執行 `./gradlew :app:installDebug` | 可以，用掃描 QR 碼的手機或 USB 安全金鑰 |
| iOS | `./rust/scripts/build-ios-xcframework.sh`，再用你自己的開發團隊在 Xcode 裡建置 | 可以，用掃描 QR 碼的手機，或 USB-C / Lightning 接頭的 YubiKey（韌體 5.8 以上） |

自行建置的應用程式，無法用「這台裝置」的密碼金鑰替 getvela.app 錢包簽署：Apple 和 Google 只允許
由 Vela 簽署的應用程式使用 `getvela.app` 的密碼金鑰。

## 新增 Vela 沒有內建的網路

任何具備 P-256 預編譯合約、也部署了它會檢查的那些標準合約的 EVM 鏈，都能執行 Vela。
[鏈設定](/zh-TW/chain-setup)會告訴你一條鏈缺什麼，並部署任何人都能部署的那些；
[網路與費用](/zh-TW/docs/networks-and-fees)說明了具體條件。有一個缺口：有多把金鑰的錢包，還需要那條鏈上有
Safe 的密碼金鑰簽署器工廠合約，而目前的檢查還不會看它——少了它，在那條鏈上只有第一把金鑰能簽署。

## 全部換掉之後，還有哪些指向 Vela

把上面的都換成你自己的之後，還會剩下這些：

- **顯示安全金鑰型號的驗證器目錄**——只影響顯示；應用程式會改顯示通用名稱。
- **getvela.app 上的關聯檔案**——商店版應用程式使用「這台裝置」的密碼金鑰時需要它們。手機或
  安全金鑰用不到。

以下這些則不屬於 Vela：公開的函式選擇器資料庫、Apple 和 Google 的手機登入通道，以及你自己
選擇的 RPC 服務商。

下一步：[可以自己架設的簽署頁](/zh-TW/docs/clear-signing-self-host)。
