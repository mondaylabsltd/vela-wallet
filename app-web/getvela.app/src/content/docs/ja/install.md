---
title: Vela をインストールする
description: "ウェブ、ブラウザ拡張、デスクトップ、スマートフォン——Vela を使うすべての方法と、それぞれの費用、できること、端末に必要なもの。"
source: f88fdfac1001
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Vela をインストールする

同じウォレットがいくつかの場所で動き、どこで開いても同じ鍵で同じアドレスが開きます。必要なものを選んでください。複数を併用することもできます。ダウンロードは [Vela を入手](/ja/get-started)にあります。

| | 内容 | 費用 | 状況 |
| --- | --- | --- | --- |
| **ウェブ** | 最近のブラウザで開く [wallet.getvela.app](https://wallet.getvela.app/) | 無料 | 公開中 |
| **ブラウザ拡張** | ツールバーに置くウォレット。dApp に接続できる | 無料 | ダウンロードして手動で読み込む。Chrome ウェブストアには未掲載 |
| **デスクトップ** | macOS・Windows・Linux 向けのネイティブアプリ | 無料 | Vela を入手のページか GitHub からダウンロード |
| **iPhone、Android** | ネイティブアプリ | ストアでの買い切り | ストアには未公開。ソースからビルドできる |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">ウェブウォレットを開く →</a>

## ウェブ

インストールは不要です。[wallet.getvela.app](https://wallet.getvela.app/) を開いてウォレットを作成するかサインインすれば、それで使えます。アカウントの一覧はこのブラウザに保存されます。別の端末では、鍵のどれか 1 本でもう一度サインインするだけです。

## ブラウザ拡張

Chromium 系のブラウザ（Chrome、Edge、Brave。Chrome は 116 以降）で使えます。ウォレットをツールバーに置き、dApp から直接接続できるようにします。Chrome ウェブストアに掲載されるまでは、次の手順で読み込みます。

1. [Vela を入手](/ja/get-started)から拡張機能をダウンロードし、残しておくフォルダーに展開します。ブラウザはそのフォルダーから拡張機能を実行します。
2. `chrome://extensions` を開き、**デベロッパー モード**をオンにします。
3. **パッケージ化されていない拡張機能を読み込む**をクリックし、そのフォルダーを選びます。

中身は同じウォレットです。拡張機能とウェブウォレットは同じ `getvela.app` のパスキーを使うので、同じ鍵で同じアドレスが開きます。

## デスクトップ

ウィンドウに収めたウェブページではなく、ネイティブアプリです。対応するのは **Windows** 10 と 11（x64 と ARM）、**macOS** 11 以降、**Linux**（.deb、.rpm、Flatpak。x64 と ARM）です。

- **Windows** では、インストーラーにまだコード署名がないため「Windows によって PC が保護されました」と表示されます。**詳細情報**を選び、**実行**を押してください。
- **macOS** 版は Apple による署名と公証を別の手順で受けるため、ほかのプラットフォームより遅れることがあります。Mac のボタンに「まもなく公開」と表示されているときは、公証済みの最新の Mac 版が GitHub のリリースページにあります。
- **Linux** で USB セキュリティキーを使うには、システムがアプリにそのキーへのアクセスを許可している必要があります。.deb と .rpm のパッケージは、そのためのルールを自動でインストールします。

macOS と Windows では、デスクトップアプリに dApp 用のブラウザが内蔵されています。すべてのパッケージのチェックサムは [GitHub のリリースページ](https://github.com/mondaylabsltd/vela-wallet/releases)にあります。

## iPhone と Android

iOS 17.4 以降と Android 10 以降に対応したネイティブアプリです。App Store と Google Play で買い切りアプリとして販売する予定ですが、**まだストアには公開していません**。コードは公開されているので、自分で無料でビルドできます。ただし違いがひとつあります。自分で署名したビルドでは、getvela.app のウォレットにスマートフォン自身のパスキーを使えません。別のスマートフォンでの QR 読み取りと USB セキュリティキーは使えます。[アプリを自分でビルドする](/ja/docs/self-hosting#web-app)を参照してください。

## dApp で Vela を使う

<span id="dapps"></span>

dApp は、ほかのブラウザウォレットと同じ方法（EIP-1193 と EIP-6963）で Vela に接続します。

- パソコンのブラウザでは、**Vela のブラウザ拡張**から。
- **デスクトップアプリ**（macOS、Windows）、**iPhone アプリ**、**Android アプリ**では、内蔵のブラウザから。

wallet.getvela.app のウェブウォレットは dApp に接続できず、WalletConnect にも対応していません。dApp からのリクエストはすべて、署名の前にデコードして表示します。[クリア署名](/ja/docs/clear-signing)を参照してください。

## 端末に必要なもの

Vela は**パスキー**で署名します。ここ数年の端末なら、ほぼすべてが対応しています。

| 端末 | 対応状況 |
| --- | --- |
| iPhone、iPad、Mac | iOS / iPadOS 16 以降、macOS では最近の Safari か Chrome |
| Android | Google Play 開発者サービスが入った最近の Android、または USB セキュリティキー |
| Windows | Chrome か Edge での Windows Hello、またはセキュリティキー |
| Linux | セキュリティキー、または近くにあるスマートフォン（QR コードを読み取る） |

端末自体がパスキーを保存できない場合は、別のスマートフォンかハードウェアセキュリティキーを使ってください。各アプリがどの種類の鍵に対応しているかは、[署名鍵とセキュリティキー](/ja/docs/signers)にまとめています。

## 公式のアドレスはこれだけです

- **getvela.app** —— このサイトと、ダウンロード
- **wallet.getvela.app** —— ウェブウォレット
- **github.com/mondaylabsltd** —— コードとリリースパッケージ

<Callout type="warning" title="インストールの前に確認を">
「Vela をインストール」や「ウォレットを認証」といった名目で別の場所へ誘導されたら、そこで手を止めてください。Vela がシードフレーズを求めることはありません。そもそも Vela にはシードフレーズがないのです。
</Callout>

次は[ウォレットを作成する](/ja/docs/create-wallet)。
