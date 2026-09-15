---
title: 監査と既知の問題
description: Vela が依存するすべてのオンチェーンのコントラクト、誰が監査したのか、監査された版と実際に配置されている版が一致するのか、そして何が監査されていないのか。
---

「監査済み」とは、特定のコードの特定のバージョンについての主張です。だからこのページ
はその言葉を振り回すのではなく、報告書そのもの、配置アドレスそのもの、そして監査版と
配置版の差分を示します。あわせて、何が**監査されていない**のかも挙げます。後者の
リストも、前者と同じだけ重みを持つからです。

最終確認：2026 年 8 月。誤りを見つけたら教えてください。直します。

## 資金の経路

あなたのお金に触れうるコントラクトは 4 層。4 層とも、公開された監査のある第三者の
コントラクトであり、いずれも配置アドレスは公式の正規デプロイです。

### Safe v1.4.1 —— アカウント本体

あなたのウォレットは [Safe](https://github.com/safe-global/safe-smart-account) の
プロキシです。SafeL2 のシングルトン、プロキシファクトリ、互換フォールバックハンドラ、
そしてバッチ用の MultiSend。

[Ackee Blockchain が Safe v1.4.0 を監査](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
（最終報告 2023 年 3 月）：指摘 11 件、クリティカルおよび高はゼロ。私たちが配置する
v1.4.1 と監査された v1.4.0 の差は、ERC-4337 互換のための 1 行の修正だけです
（[PR #572](https://github.com/safe-global/safe-smart-account/pull/572)）。
MultiSend のロジックは
[G0 Group が監査した v1.3.0](https://github.com/safe-global/safe-smart-account/tree/main/docs)
以来変わっていません。すべてのアドレスは
[safe-deployments](https://github.com/safe-global/safe-deployments) の正規デプロイ
と一致し、これらのコントラクトは
[Safe Foundation のバグバウンティ](https://docs.safefoundation.org/security/bug-bounty)
の対象です（クリティカルで最大 100 万ドル）。

監査が扱わないものがひとつあります。2025 年の Bybit の事件です。あの攻撃が侵害した
のは Safe 公式ウェブフロントエンドのビルドパイプラインであって、コントラクトでは
ありません。[公式のフォレンジックの結論](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
は、Safe のスマートコントラクトに脆弱性はなかったとしています。私たちはこれを
ウェブと運用の層についての教訓として読んでいます。その層こそ、あなたが私たちを
厳しく見るべき場所でもあります。

### Safe4337Module v0.3.0 —— ERC-4337 のアダプタ

`0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` に配置。v0.3.0 の正規アドレスです
（Sourcify で完全一致——オンチェーンのバイトコードが監査されたコードそのもの）。
[Ackee Blockchain が監査](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
（最終報告 2024 年 3 月）し、情報レベルを超える未解決の指摘はありません。私たちが
使う v0.3.0 + EntryPoint v0.7 + Safe 1.4.1 以上という組み合わせは、監査とリリース
ノートが記述しているとおりの構成です。

このモジュールの履歴には、公表された問題がひとつあります。v0.1.0（2023 年）は
`initCode` と `paymasterAndData` に署名しておらず、ガスのグリーフィング経路が
ありました。これは
[v0.2.0 で修正](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)
され、v0.1.0 がテストネットの外へ出ることはありませんでした。私たちが使うのは
v0.3.0 で、この修正を引き継いでいます。

### SafeWebAuthnSharedSigner v0.2.1 —— パスキーの署名器

`0x94a4F6affBd8975951142c3999aEAB7ecee555c2` に配置。v0.2.1 の正規アドレスです
（Safe のシングルトンファクトリにより、どのチェーンでも同じ）。

「shared（共有）」の意味と、意味しないこと：共有されているのは*コントラクトの配置*
であって、Safe のシングルトンが共有されているのと同じです。あなたの鍵は共有されま
せん。各 Safe が delegatecall で `configure()` を呼び、自分の P-256 公開鍵を自分の
ストレージに保存します。ひとつの署名器のインスタンスが表すのは、Safe ごとにちょうど
ひとつのパスキーであり、他人の Safe があなたのものを使うことはできません。

ここではバージョンが効きます。v0.2.0 の監査は、共有署名器が対象外であると
[明記しています](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
——当時そのコントラクトはまだ存在しませんでした。私たちが配置しているものを対象と
するのは v0.2.1 の監査です。
[Hats Finance の監査コンペ](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
（2024 年 6〜7 月：高ゼロ、中ゼロ、低 3 件——すべて修正済み）に加えて、
[Certora によるリリースコミットのレビュー](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)
でも新たな指摘はありませんでした。リリース以降、コントラクトレベルの脆弱性は公表
されていません。パスキー関連のコントラクトも Safe Foundation のバウンティ対象です。

Safe 自身のドキュメントは、ひとつのクレデンシャルをアカウント唯一の鍵として扱うので
はなく、パスキーの所有に復旧経路を組み合わせることを勧めています。Vela がこれをどう
扱うかは[復旧とサインイン](/ja/docs/recovery)にあります。

オンチェーンの P-256 検証は RIP-7212 のプリコンパイルを直接使い、Solidity の
フォールバック検証器はありません。ネットワークを有効化する前に、アプリは実際の署名で
プリコンパイルを試し、検証に失敗すればそのネットワークを断ります。正直な注意が 2 つ。
当初の RIP-7212 仕様には端のケースの欠陥があり、
[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) がそれを直すために書かれました
（正しい形の WebAuthn 署名には影響しません）。そして 1 回の試行では、あるチェーンの
実装が珍しい実行文脈でどう食い違いうるかを、すべて捕まえることはできません。

### EntryPoint v0.7 —— ERC-4337 のエントリポイント

`0x0000000071727De22E5E9d8BAf0edAc6f37da032` に配置。
[v0.7.0 の正規デプロイ](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)です。
[OpenZeppelin が監査](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
（イーサリアム財団の委託、2024 年 1 月）：クリティカル ゼロ、高 ゼロ、中 5 件、
すべて解決済み——そして監査されたコミットが、配置されているリリースです。
EntryPoint v0.7.0 はイーサリアム財団の
[ERC-4337 バグバウンティ](https://docs.erc4337.io/community/bug-bounty)の対象です
（最大 25 万ドル）。

## 私たちが見張っている既知の問題

### EntryPoint のグリーフィング経路

2026 年 2 月、Trust Security のセキュリティ研究者が、v0.9 より前のすべての
EntryPoint——私たちが使う v0.7 を含みます——に影響するグリーフィングと検閲の経路を
[公表しました](https://erc4337.substack.com/p/improving-useroperation-execution)。
署名済みの UserOperation がマイニングされる前に横取りできる攻撃者は、それを自分が
制御するコールフレームの中で実行し、内側の実行を失敗させられます。操作は失敗します
が、ガスは請求されます。イーサリアム財団はこの発見に 5 万ドルの報奨金を支払い、
資金窃取ではなく検閲・グリーフィングの経路と分類しました。実際に悪用されたことは
ありません。

できること：手数料を無駄にし、取引を遅らせること。できないこと：資金を盗むこと、
署名を偽造すること。Vela の露出は狭く、UserOperation は公開のメモリプールを通らず
リレーへ直接送られるので横取りの機会がほとんどなく、最悪でもあなたが同意済みの手数料
の範囲に収まります。修正は EntryPoint v0.9（2025 年 11 月）にしかなく、v0.7 自体に
パッチは当てられません。周辺のスタック——とくに Safe の 4337 モジュール系——が v0.9
に対応するのに合わせて移行する見込みで、そのときはここに書きます。

## 監査されていないもの

- **Vela 自身のコントラクト。** 私たちが書いた小さなコントラクトが 2 つ、Gnosis に
  配置されています。
  [パスキー公開鍵インデックス](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  （あなたの端末が公開鍵を見つけるための、追記のみのレジストリ）と、そのバッチ補助
  です。これらは監査されていません。構造上、資金を保持せず、所有者もおらず、
  アップグレードもできません。発見のための層であって、認可の層ではありません。支払い
  の権限は、つねにあなたの Safe の中に設定されたパスキーから来ます。現実的な最悪の
  事態はグリーフィング（誰かがインデックスの項目を先取りする）で、復旧が不便になる
  ことはあっても、お金は動かせません。以前の手数料設計にあったガス精算用のスプリッタ
  は、もう取引の経路にありません。
- **Multicall3。** その README 自身が
  [はっきり書いています](https://github.com/mds1/multicall3)：「このコントラクトは
  監査されていません。」私たちは、作者が安全だと説明しているとおりの使い方——残高、
  トークンのメタデータ、価格を読むためのバッチ読み取り——をしています。Vela がそこに
  承認を与えることはなく、資金を持たせることもありません。バグの最悪の結果は、読み
  取り値が正しくないことです。
- **CREATE2 のデプロイヤー。**
  [Arachnid の決定論的デプロイプロキシ](https://github.com/Arachnid/deterministic-deployment-proxy)
  はエコシステム標準のステートレスなデプロイヤーで、正式な監査はありません。あるチェーン
  で欠けていたり改変されていたりすれば、私たちのネットワーク検査は安全側に倒れて失敗
  します。
- **Tempo と pathUSD。** 12 の組み込みネットワークのひとつである Tempo にはネイティブ
  コインがなく、ガスは pathUSD ステーブルコインで決済されます。2026 年 8 月時点で、
  Tempo のコアプロトコルにも pathUSD にも公開されたセキュリティ監査はなく、バグ
  バウンティもありません。独立した
  [DefiLlama の担保評価](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  （2026 年 4 月）は pathUSD を高リスクと評価しました。これはチェーン側のリスクで、
  どのウォレットにも緩和できません。Tempo に置いた資金も、そこでのガス決済も、それを
  引き継ぎます。Tempo はこのリストで最も新しく、最も実績の少ないチェーンとして扱い、
  残高の大きさもそれに合わせてください。監査が公開されればこの節を更新します。
- **Vela そのもの。** 私たちのアプリとバックエンドサービスは第三者の監査を受けて
  いません。これがこのページで最大の留保であり、サイトのヘッダーにも明記しています。
  正直な詳細は[Vela はアルファです](/blog/vela-is-in-alpha)に。少額から始めてください。
  コードを読んでください。

## 自分で確かめる

上のアドレスはすべて公開された正規デプロイで、公式のレジストリと突き合わせられます
——[safe-deployments](https://github.com/safe-global/safe-deployments)、
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)、
そして [EntryPoint のリリースノート](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)：

| コントラクト | アドレス |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| パスキー公開鍵インデックス（Gnosis） | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
