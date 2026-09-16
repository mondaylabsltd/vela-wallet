---
title: Vela を使い始める
description: Vela はブラウザで動きます——インストールもアプリストアも不要です。ウェブウォレットを開くか、パスキーに必要な端末の条件をまず確認してください。
---

# Vela を使い始める

Vela は**ブラウザの中で**動きます。ダウンロードするものはなく、アプリストアを通す
必要もありません。ウェブウォレットを開けば、1 分かからずにウォレットを作成、または
復旧できます。

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">ウェブウォレットを開く →</a>

同じコードから作られた同じウォレットは、iOS と Android でも動きます。**ネイティブの
モバイルアプリは近日公開予定**です。公開されても、アカウントはオンチェーンにあって
特定のアプリの中にあるわけではないので、パスキーもウォレットもそのまま引き継がれます。

## 端末に必要なもの

Vela は**パスキー**（WebAuthn）で署名するので、それに対応した端末とブラウザが必要
です。ここ数年の環境なら、ほぼ対応しています。

| プラットフォーム | パスキー対応 | 同期するもの |
| -------- | --------------- | --------- |
| iPhone / iPad / Mac | iOS/iPadOS 16 以降、最近の Safari | iCloud キーチェーン |
| Android | Android 9 以降、現行の Chrome | Google パスワードマネージャー |
| デスクトップ | 現行の Chrome、Edge、Safari、Firefox | お使いのプラットフォームのパスキー基盤 |

ウォレットを新しい端末へ引き継ぐには、プラットフォームのパスキー同期をオンのままに
しておいてください（Apple なら iCloud キーチェーン、Android/Chrome なら Google
パスワードマネージャー）。その仕組みは[復旧とサインイン](/ja/docs/recovery)にあります。

## 公式の URL はこの 2 つだけ

Vela はオープンソースで、それ自体が要点なのですが、同時に「本物を開いているか」を
自分で確かめる必要があるということでもあります。公式のアドレスは次の 2 つだけです。

- **getvela.app** —— このサイト
- **wallet.getvela.app** —— ウォレット

「Vela をインストールする」と言って別の場所へ誘導されたら、いったん止まって、この
2 つと突き合わせてください。コードは
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet)
で公開しています。

次は[ウォレットを作成する](/ja/docs/create-wallet)。
