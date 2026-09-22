---
title: セルフホスティングガイド
description: "Vela があなたのために動かしているもの、それぞれの役割、そして自分のものに置き換える方法（リレー、公開鍵インデックス、チェーンデータ、為替レート、アプリ）。置き換えられない唯一のものと、getvela.app なしで使い続ける方法も。"
source: 5ae6005a9396
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# セルフホスティングガイド

あなたの資金はオンチェーンの Safe コントラクトにあり、あなたの鍵で管理されています。Vela が動かしているものは、どれもそれを動かせません。Vela が動かしているのは、ウォレットを便利にするための仕組みです。取引を送信するリレー、新しい端末がウォレットを見つけるためのインデックス、チェーンデータのディレクトリ、為替レートの配信、そしてアプリそのものです。

このページでは、そのひとつひとつについて、何をしているのか、それがないと何が困るのか、自分で動かすにはどうするのかを説明します。置き換えられない唯一のもの、つまりパスキーが属するドメインと、getvela.app がなくなったときにどうすればいいかも扱います。

<Callout type="info" title="このページの対象読者">
ターミナル、Docker または Cloudflare Workers の扱いと、チェーン上のアドレスへの入金に慣れていることを前提にしています。日常的に Vela を使うだけなら、ここにあるものは何も必要ありません。
</Callout>

## 全体像

| 構成要素 | 役割 | Vela の既定 | 置き換えられるか | ないとどうなるか |
| --- | --- | --- | --- | --- |
| **リレー** | 署名済みの操作を受け取り、ガス代を払って送信し、あなたが署名した手数料を受け取る | `vela-relay-cf.getvela.app` | できる。[vela-relay](#relay) を動かし、ウォレットの接続先にする | 送金できない |
| **公開鍵インデックス** | 新しいウォレットの鍵をオンチェーンに登録し、「この鍵はどのウォレットのものか」に答える | `p256-index-v2.getvela.app` | できる。[p256-index](#index) を動かす | 新しいウォレットを作成できない。サインインはチェーンの直接読み取りに切り替わる |
| **レジストリコントラクト** | 各ウォレットの鍵の、永続的な公開記録 | Gnosis 上の `0x94fD…1EA9` | 不要。所有者がおらず、ウォレットが直接読み取る | — |
| **チェーンデータ** | ネットワークの情報、トークンの一覧、ロゴ、クリア署名のディスクリプター | `ethereum-data.getvela.app` | できる。[ethereum-data](#chain-data) を動かす | トークンの一覧やロゴがない。デコードできる取引が減る。ネットワークを追加できない |
| **為替レート** | 表示通貨での法定通貨換算額 | `vela-currency.getvela.app` | できる。[vela-currency](#exchange-rates) か、Frankfurter 互換の任意のソースを動かす | アプリは可能な範囲でオンチェーンの Chainlink のレートに切り替える（デスクトップは米ドル表示） |
| **RPC ノード** | 残高の読み取り、取引のシミュレーション | ネットワークごとの公開エンドポイント | できる。「設定 → ネットワーク」でネットワークごとに | Vela がエンドポイントを自動で切り替える |
| **アプリ** | ウォレットそのもの | wallet.getvela.app、リリースビルド | できる。[自分でビルドする](#web-app) | — |
| **getvela.app** | パスキーが属するドメイン | — | **できない**。[後述](#if-getvela-app-disappears) | — |

Vela のものではない、第三者のサービスにもいくつか接続します。取引をデコードする最後の手段として使う公開の関数セレクターデータベース（sourcify、openchain、4byte）、セキュリティキーの機種名を表示するための認証器ディレクトリ、そして QR コードを読み取ってスマートフォンで署名するときに通る Apple と Google のトンネルサーバーです。

## 置き換えられない唯一のもの：パスキーのドメイン

<span id="if-getvela-app-disappears"></span>

パスキーは、作成されたウェブサイトに属します。Vela の鍵は `getvela.app` のために作られています。ブラウザがそれを提示するのは getvela.app とそのサブドメイン上のページ（または getvela.app が関連オリジンとして宣言したオリジン）だけで、スマートフォンに内蔵されたパスキーは、getvela.app が認めたアプリでしか使えません。ブラウザの外では、ルールはもう少しゆるやかです。Chrome は getvela.app へのアクセス権を持つ拡張機能にそれを使わせますし、パソコン上のプログラムは、セキュリティキーやスマートフォンに getvela.app 向けの署名を直接求められます。自分でビルドしたアプリはこの仕組みで動いており、どのソフトウェアを動かすかが重要になるのもこのためです。ここから 2 つのことが言えます。

**自分のドメインで動かすウェブウォレットのコピーは、別のウォレットです。**`wallet.example.com` から配信すれば、同じコードでも `wallet.example.com` 用のパスキーを作ります。新しい鍵、つまり新しいアドレスです。wallet.getvela.app で作ったウォレットのために署名することはできません。それでもこのコピーには使い道があります。そこで新しく作るウォレットのため、あるいはすべてを最初から自分で動かすためです。

**既存のウォレットについては、getvela.app がオフラインになっても、なくなっても、次の方法が使えます。**

| 使い方 | 使える鍵 | 入手先 |
| --- | --- | --- |
| **Vela のブラウザ拡張**（Chromium 系のブラウザ：Chrome、Edge、Brave） | ブラウザから使えるあらゆる鍵：この端末のパスキー、USB セキュリティキー（パソコンが対応していれば NFC も）、QR コードでつなぐスマートフォン | [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases) のリリース zip、または[自分でビルドする](#web-app) |
| **自分でビルドしたデスクトップアプリやスマートフォンアプリ** | QR コードでつなぐスマートフォン、USB セキュリティキー | [自分でビルドする](#web-app) |
| **ストア版と公証済みのデスクトップアプリ** | QR コードでつなぐスマートフォンとセキュリティキーは常に使える。「この端末」のパスキーは、OS がアプリを getvela.app と照合できるあいだだけ | GitHub のリリース（ストアは今後） |

拡張機能で `getvela.app` の鍵を使えるのは、Chrome が、あるサイトへのアクセス権を持つ拡張機能に、そのサイトのパスキーを使わせるからです。このアクセス権はブラウザがローカルで確認します。動作することは実測で確かめていますが、ドメインが実際にオフラインの状態ではまだ試していません。自分でビルドしたアプリでスマートフォンやセキュリティキーを使えるのは、Vela がそれらと直接やり取りするからです。スマートフォン自身のパスキー（「この端末」）を使うには、アプリが Vela によって署名されている必要があり、あなたがビルドしたものはそうではありません。

[署名ページ](/ja/docs/clear-signing-self-host)は、それだけでは使い方のひとつになりません。ほかのプログラムから送られてきたリクエストに署名するものですが、まだどの Vela のアプリもリクエストを送らないからです。

<Callout type="warning" title="ドメインを握る者は、署名を求められる">
getvela.app やそのサブドメインから配信されるページはどれも（将来このドメインを握る者のページも）、あなたの鍵に署名を求めることができ、システムの確認画面に表示されるのは取引ではなく「getvela.app」です。これはどこでも同じ、パスキーのしくみです。そのため Vela のウェブサイトは、自分のページがパスキーを使うことを禁止しています。拡張機能や自分でビルドしたアプリが重要なのもこのためです。これらは自分のコードを内蔵しています。ただし既定では、ディスクリプターの取得やサービスの利用には、いまも getvela.app 配下のものを使います。
</Callout>

## ウォレットの接続先を自分のサービスにする

各アプリの **設定 → 詳細設定 → サービスエンドポイント**（デスクトップでは **設定 → サービスエンドポイント**）に、4 つの欄があります。**チェーンデータインデックス**、**パスキーインデックス**、**VELA RELAY**、**法定通貨レート**です。変更するまで、各欄には Vela の既定値が表示されます。**デフォルトに戻す**を押すと、4 つすべてが元に戻ります。リレー、インデックス、チェーンデータについては、ウォレットが `/api/health` を呼び出してバッジを表示します。バッジが緑になるのは、エンドポイントが正しいサービス名を返し、`status: "ok"` を報告したときだけです。入力した内容は、どちらの場合も保存されます。緑になるのを待ってください。

| サービス | `/api/health` の `service` |
| --- | --- |
| リレー | `vela-relay` |
| 公開鍵インデックス | `webauthn-p256-publickey-registry` |
| チェーンデータ | `ethereum-data` |
| 為替レート | 名前では確認しない。米ドル基準のレート一覧を返す必要がある |

現在、各アプリがこれらの設定にどこまで従うかは、次のとおりです。

| アプリ | サービスエンドポイント | ネットワークごとの RPC |
| --- | --- | --- |
| ウェブと拡張機能 | チェーンデータ、リレー、法定通貨レートは有効。パスキーインデックスは名前の検索には使われるが、ウォレットの作成とサインインには引き続き Vela のインデックスを使う | 対応 |
| デスクトップ | 4 つすべて有効。新しいパスキーインデックスは、再起動かサインアウトのあとに反映される | 対応 |
| Android | 4 つすべて有効。ただし、アドレスの名前の検索は引き続き Vela のインデックスに問い合わせる | 対応 |
| iOS | **未対応**：画面には仮の値が表示され、保存もされない。既定のインデックスに接続できないときは、サインイン画面でパスキーインデックスを変更できる | 読み取り専用 |

これらの不足はバグで、追跡しています。

## 自分のリレーを動かす

<span id="relay"></span>

リレーは [vela-relay](https://github.com/mondaylabsltd/vela-relay)（Rust、MIT）です。ひとつのデプロイメントですべてのチェーンを扱い、ウォレットは `https://your-relay/<chainId>` を呼び出します。vela-relay である必要があります。ウォレットは Vela 独自のメソッドで手数料の見積もりを求めますが、汎用の ERC-4337 バンドラーはこのメソッドを実装していないからです。

**必要なもの**

- Docker と、すでに動かしている Redis と [Iggy](https://iggy.apache.org) のサーバー。または **Workers Paid** プランの Cloudflare アカウントと、手元のマシンの Node.js、Rust ツールチェーン（`wasm32-unknown-unknown` ターゲット付き）。
- `OPERATOR_SECRET`（16 進数、32 バイト以上）。ここからトレジャリーのアドレスが 1 つと、リレーヤーのアドレス群が導出され、どのチェーンでも同じになります。秘密にしてください。リレーの資金を管理する値です。
- 扱いたいすべてのチェーンでのガス代。チェーンのコイン（Tempo では pathUSD）をトレジャリーのアドレスに送ってください。トレジャリーがリレーヤーに補充します。

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# .env に VELA_RELAY_IGGY_URL、VELA_RELAY_REDIS_URL、OPERATOR_SECRET を設定し、
# 自分のチェーンデータを動かすなら VELA_RELAY_CHAIN_DIRECTORY_URL も加え、
# VELA_RELAY_IMAGE を信頼できるリリースイメージにする（docs/docker.md を参照）
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

公開されているイメージを使ってください。`docker compose up --build` でソースからビルドすると、現在の Dockerfile では失敗することがあります。Docker を使わない場合は、`cargo run --release --bin vela-relay` で直接動かせます。

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# 自分のチェーンデータを使うなら、wrangler.jsonc の "vars" に "VELA_RELAY_CHAIN_DIRECTORY_URL" を追加する
npx wrangler deploy
```

**確認する**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # Gnosis でのトレジャリーのアドレスと、ガスの補充が必要かどうか
```

そのあと、`https://your-relay` を **VELA RELAY** 欄に入力します。

**知っておくこと**

- ウォレットが払う手数料は、あなたのトレジャリーに入ります。どのリレーを使っても、ウォレットは同じ方法で手数料を計算します（[ネットワークと手数料](/ja/docs/networks-and-fees)を参照）。
- リレーを変更する前に追加したカスタムネットワークは、追加したときのリレーのアドレスを使い続けます。
- リレーは、各チェーンの情報と、受け付けるステーブルコインの一覧を、チェーンディレクトリから読みます。`VELA_RELAY_CHAIN_DIRECTORY_URL` に[自分のもの](#chain-data)を設定しないかぎり、読み先は `ethereum-data.getvela.app` です。この設定は 2026 年 9 月に追加されました。それより古いリレーのビルドは、常に Vela のコピーを読みます。

## 自分の公開鍵インデックスを動かす

<span id="index"></span>

インデックスは [p256-index](https://github.com/mondaylabsltd/p256-index)（Rust、MIT）です。ウォレットが作成されると、インデックスは各鍵の証明を確認し、鍵の組を Gnosis の**レジストリコントラクト**に書き込んで、ガス代を払います。既存のレジストリ `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` をそのまま使ってください。所有者がおらず、資金のあるアドレスなら誰でも書き込めて、すべての Vela のアプリがそれを直接読み取ります。自分で別のレジストリを作っても、アプリからは見えません。

**必要なもの**

- Docker と Redis、Iggy（サーバー版）、または Cloudflare アカウント（Worker 版。その README には、オンチェーンへの書き込みはまだ端から端までテストされていないと書かれています）。
- xDAI の入った Gnosis の秘密鍵。ウォレット 1 つの登録には、鍵 1 本で約 110 万ガス、7 本で約 360 万ガスかかります。
- 次の設定。

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

サーバーのサンプル設定ファイルには `P256_INDEX_DOMAIN_REGISTRY` がありませんが、これは欠かせません。これがないと、サーバーはコントラクトが拒否するチャレンジを発行し、すべての登録が失敗します。

**動かして確認する**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

サーバーは暗号化なしの HTTP（既定のポートは 11256）で待ち受けます。ウォレットは `https://` のエンドポイントしか受け付けないので、前段に TLS プロキシを置いてください。この文書の執筆時点では、ソースの Dockerfile はビルドできないことがあります。Cargo でのビルドは問題ありません。

**どのインデックスもまったく応答しない場合**でも、既存のウォレットは使えます。サインインのとき、アプリはあなたの RPC ノードを通じて Gnosis（次に Ethereum）のレジストリコントラクトを読みます。鍵が 1 本のウォレットなら、レジストリを使わずに、署名 2 つから再構築することさえできます。新しいウォレットの作成には、インデックスが必要です。登録の費用を誰かが払わなければならないからです。

## 自分のチェーンデータを動かす

<span id="chain-data"></span>

チェーンデータは [ethereum-data](https://github.com/atshelchin/ethereum-data)（MIT）です。約 2,600 のネットワークとそのトークンについての静的な JSON と画像、そして Vela が取引を説明するのに使う ERC-7730 のディスクリプターです。

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

ソースからのビルドと Cloudflare へのデプロイについても、README で説明しています。HTTPS で配信し、そのアドレスを **チェーンデータインデックス** 欄に入力してください。

リレーもこれらのファイルを読みます。その中には Vela 独自のフィールドもあります（手数料の支払いに使えるステーブルコインは `stables` の一覧で決まります）。`VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data` で、リレーの読み先を自分のコピーにしてください。リレーは各ネットワークのエントリを 1 時間キャッシュします。

## 自分の為替レートを動かす

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency)（MIT）は、欧州中央銀行が毎日公表するレートを再配信します。キーは必要ありません。

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

`https://your-host/v2/rates?base=USD` を **法定通貨レート** 欄に入力してください。Frankfurter 互換のサービスなら何でも使えます。`?base=USD` は外さないでください。すべての換算がこれを前提にしています。

## アプリを自分でビルドする

<span id="web-app"></span>

すべてのアプリは[ひとつのリポジトリ](https://github.com/mondaylabsltd/vela-wallet)（MIT）にあります。各アプリのビルド手順は README にあります。要点は次のとおりです。

| アプリ | ビルド | 既存の getvela.app のウォレットのために署名できるか |
| --- | --- | --- |
| ブラウザ拡張 | `cd app-web/vela-wallet && pnpm install && pnpm build:extension` のあと、`chrome://extensions` で `extension/dist` をパッケージ化せずに読み込む | できる。どの鍵でも |
| ウェブウォレット | `cd app-web/vela-wallet && pnpm install && pnpm build`。Cloudflare Worker としてデプロイする | できない。自分のドメインでは別のウォレットになる（前述） |
| デスクトップ | `cd app-desktop/vela-wallet && cargo run`（パッケージ化のスクリプトは README に） | できる。QR コードでつなぐスマートフォンか USB セキュリティキーで |
| Android | コアのバインディングを生成してから `./gradlew :app:installDebug` | できる。QR コードでつなぐスマートフォンか USB セキュリティキーで |
| iOS | `./rust/scripts/build-ios-xcframework.sh` のあと、自分のチームで Xcode からビルドする | できる。QR コードでつなぐスマートフォンか、USB-C / Lightning の YubiKey（ファームウェア 5.8 以降）で |

自分でビルドしたアプリでは、getvela.app のウォレットに「この端末」のパスキーは使えません。Apple と Google は、Vela が署名したアプリにしか `getvela.app` のパスキーを使わせないからです。

## Vela が内蔵していないネットワークを追加する

Vela は、P-256 プリコンパイルと、Vela が確認する標準コントラクトがそろった EVM チェーンなら、どこでも動きます。[チェーンのセットアップ](/ja/chain-setup)で、そのチェーンに何が足りないかがわかり、誰でもデプロイできるものはそこからデプロイできます。要件は[ネットワークと手数料](/ja/docs/networks-and-fees)で説明しています。抜けがひとつあります。鍵が 2 本以上のウォレットには、そのチェーン上に Safe のパスキー署名器ファクトリーも必要ですが、確認ではまだこれを調べていません。ファクトリーがなければ、そのチェーンで署名できるのは最初の鍵だけです。

## すべて置き換えても、Vela を向いたまま残るもの

ここまでのものをすべて置き換えても、次のものは残ります。

- **セキュリティキーの機種名を表示する認証器ディレクトリ** —— 見た目だけの問題で、アプリは汎用の名前で代用します。
- **getvela.app の関連付けファイル** —— ストア版のアプリが「この端末」のパスキーを使うのに必要です。スマートフォンやセキュリティキーには必要ありません。

そして、次のものは Vela のものではありません。公開のセレクターデータベース、スマートフォンでのサインインに使う Apple と Google のトンネル、そしてあなたが選ぶ RPC プロバイダーです。

次は[自分で動かせる署名ページ](/ja/docs/clear-signing-self-host)。
