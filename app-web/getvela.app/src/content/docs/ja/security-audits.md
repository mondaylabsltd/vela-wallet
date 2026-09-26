---
title: 監査と既知の問題
description: "Vela が依存するすべてのコントラクト、どのバージョンを誰が監査したのか、監査されたバージョンがデプロイされているものと同じか、私たちが注視している未解決の指摘、そしてまったく監査されていないもの。"
source: a9c5e58e7ed3
---

「監査済み」とは、特定のバージョンの特定のコードについての主張です。そのため、このページでは監査報告書、コミット、デプロイ先のアドレスを示します。そして、それと同じくらい重要な、**監査されていない**ものも挙げます。

最終確認日：2026 年 9 月 22 日。誤りを見つけたら知らせてください。修正します。

## 資金の経路

資金に触れうるコントラクトは、すべて第三者のコードの正規のデプロイメントで、レビューが公開されています。

### Safe v1.4.1 —— アカウント本体

あなたのウォレットは、SafeL2 シングルトンと SafeProxyFactory を使う [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1) のプロキシです。バッチ処理は MultiSend を通ります。

[Ackee Blockchain が Safe v1.4.0 を監査しました](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)（最終報告 2023 年 3 月 16 日、修正レビュー 3 月 28 日）。指摘は 11 件で、Critical や High はありません。Medium の 2 件は、変更されずに認識済みとされました。対象範囲は SafeL2、SafeProxyFactory、CompatibilityFallbackHandler、MultiSendCallOnly、SignMessageLib です。v1.4.1 と v1.4.0 の機能上の違いは 1 行だけで、モジュールのセットアップにおける ERC-4337 互換性の修正です（[PR #572](https://github.com/safe-global/safe-smart-account/pull/572)）。Safe は Ackee に相談し、再監査は不要と結論づけました。MultiSend のロジックは v1.3.0 から変わっておらず、v1.3.0 は [G0 Group が監査しています](https://github.com/safe-global/safe-smart-account/tree/main/docs)。すべてのアドレスは [safe-deployments](https://github.com/safe-global/safe-deployments) と一致します。コアコントラクトは [Safe Foundation のバグバウンティ](https://docs.safefoundation.org/security/bug-bounty)の対象で、最上位の報奨金は最大 100 万ドルです。

2025 年の Bybit の事件は、コントラクトに関する指摘ではありません。攻撃者が改ざんしたのは Safe のウェブインターフェースに配信される JavaScript であり、Safe の[調査声明](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)はコントラクトに脆弱性はなかったとしています。[事件についてのページ](/ja/docs/bybit-attack)では、同じ種類の攻撃が、私たちのものも含めてあらゆるウォレットのインターフェースに関わる理由を説明しています。

### Safe4337Module v0.3.0 —— ERC-4337 アダプター

`0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` にデプロイされ（Sourcify で完全一致）、あなたの Safe のフォールバックハンドラーにも設定されています。レビューは 3 回行われています。[報告書はこちら](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)。

- **Ackee Blockchain**、最終報告 2024 年 3 月：警告 1 件（コンパイラのオプティマイザーの使用）が認識済み。それより重い未解決の指摘はありません。
- **Certora**、2026 年 8 月：**Medium** の指摘が 1 件あり、認識済みですが v0.3.0 では**修正されていません**。*認可の変更が、同じバンドル内ですでに検証された後続の UserOperation を無効にしない*というものです。後述の「既知の問題」を参照してください。
- **Nethermind**、2026 年 8 月：指摘なし。

ウォレットのデプロイ時にこのモジュールを有効にする SafeModuleSetup v0.3.0（`0x2dd6…5b47`）は、Certora と Nethermind のレビューの対象に含まれています。

このモジュールで公表された問題は 1 件です。v0.1.0 は `initCode` と `paymasterAndData` に署名しておらず、ガスを浪費させる攻撃の余地がありましたが、[v0.2.0 で修正されました](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)。Safe によれば、v0.1.0 はテストネット以外では使われていません。Vela が使っているのは v0.3.0 で、EntryPoint v0.7 と Safe 1.4.1 との組み合わせは、このモジュールのリリースが示している構成です。

### Safe パスキーモジュール v0.2.1 —— 署名器

最初の鍵は、`0x94a4F6affBd8975951142c3999aEAB7ecee555c2` にある **SafeWebAuthnSharedSigner** が検証します。「Shared（共有）」とは、Safe のシングルトンと同じように、コントラクトのデプロイメントが共有されているという意味です。鍵が共有されるわけではありません。各 Safe は、自分の P-256 公開鍵を自分のストレージに保存しています。

追加の鍵にはそれぞれ専用の署名器コントラクトがあり、`0x1d31F259eE307358a26dFb23EB365939E8641195` の **SafeWebAuthnSignerFactory** が、`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` の **SafeWebAuthnSigner シングルトン**へのプロキシとして作成します。

v0.2.1 のこれらのコントラクトを対象とするレビューは次のとおりです（[報告書](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)）。

- [Hats Finance の監査コンペティション](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)（2024 年 6〜7 月）：High と Medium の指摘はなし。Low が 3 件あり、すべて修正済み。
- **Certora** によるリリースコミットのレビュー：新たな指摘はなし。（それ以前の v0.2.0 の監査には、共有署名器はまだ監査されていないと記されています。共有署名器はその監査のあとに追加されたものです。）
- **Nethermind**、2026 年 8 月：指摘なし。

リリース以降、コントラクトレベルの脆弱性は公表されておらず、パスキーのコントラクトも Safe Foundation のバウンティの対象です。

パスキーの署名はチェーンの **EIP-7951 / RIP-7212** プリコンパイルで検証され、代わりの検証器はありません。アプリは、ネットワークを有効にする前に、実際の署名でプリコンパイルを確認します。注意点が 2 つあります。当初の RIP-7212 の仕様には境界ケースの欠陥があり、[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) で修正されています（影響するのはもともと失敗すべき入力だけで、正しい形式の WebAuthn 署名には影響しません）。また、一度の確認では、チェーンの実装がずれうるすべてのケースを見つけることはできません。

### EntryPoint v0.7 —— 操作を実行する

`0x0000000071727De22E5E9d8BAf0edAc6f37da032` にデプロイされた、[正規の v0.7.0 リリース](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)です。Ethereum Foundation の依頼で [OpenZeppelin が監査しました](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)（2024 年 1 月）。Critical と High の指摘はなく、Medium が 5 件、24 件の指摘すべてが解決済みで、修正レビューのコミットはリリースと一致します。Ethereum Foundation の [ERC-4337 バグバウンティ](https://docs.erc4337.io/community/bug-bounty)（最大 25 万ドル）の対象です。

## 注視している既知の問題

### 同じバンドル内での認可の変更（Safe4337Module、Certora M-01）

EntryPoint は、バンドル内のすべての操作を検証してから、それぞれを実行します。そのため、ある操作が所有者を削除しても、同じバンドルの後ろのほうにある、その所有者が署名した操作は検証を通過して実行されます。Safe はこれを認識したうえで、v0.3.0 を変更しませんでした。

Vela のアプリは所有者を変更する操作を一切組み立てず、dApp がそれを求めても即座に拒否されるので、Vela 自身がこれを引き起こすことはありません。それでも、ほかの Safe ツールで漏れた鍵を削除する人には関係があります。同じバンドルの中でその鍵が確実に締め出されるとは期待できないからです。

### 署名済みの操作の横取り（v0.9 より前の EntryPoint）

2026 年 2 月、研究者たちが、v0.7 を含む v0.9 より前のすべての EntryPoint に影響する、妨害と検閲の手口を[公表しました](https://erc4337.substack.com/p/improving-useroperation-execution)。署名済みの操作をマイニング前に手に入れた人は、それを自分が管理する呼び出しの中で実行し、内部の実行を強制的にリバートさせられます。操作は失敗し、署名し直す必要があります。（Vela のインバンド手数料では、手数料の送金も一緒にリバートするので、ガス代を負担するのはあなたではなくリレーです。）影響を受けるのは、リエントランシー保護のあるコントラクトを呼び出す操作や、一時的な状態によってリバートさせられる操作で、単純な送金は影響を受けません。出金のフローに対して繰り返し使われれば、しばらくのあいだ資金を使えない状態が続くおそれがあります。署名を偽造することも、資金の行き先を変えることもできません。

Vela のリレーは共有のメンプールを通さず操作を直接送信しますが、保留中の `handleOps` トランザクションは公開のメンプールで見えるため、これはリスクを狭めるだけで、なくすわけではありません。修正は EntryPoint v0.9（2025 年 11 月）にしかなく、v0.7 にパッチを当てることはできません。移行は Safe の 4337 モジュールが v0.9 に対応するかどうかにかかっており、移行したらこのページでお知らせします。

### Vela 自身の防御に欠けているもの

コントラクトの指摘ではありませんが、ウォレットがあなたを守る範囲が、想像よりも狭いかもしれない点です。意図的なトレードオフと明記したものを除き、いずれも修正に向けて追跡しています。

- **無制限の承認は、そのままにすれば送信されます。**これは意図的なトレードオフです。上限を設けた承認では、Permit2 やバッチ化されたスワップが動かなくなるからです。「無制限」の承認（2^200 以上。Permit2 では 2^152 以上）は赤で表示され、上限を設けないかぎり dApp が求めたとおりに送信されます。ウェブとデスクトップでは、バッチの中の承認にはまだ上限を設けられません。署名による許可には、どこでも上限を設けられません。
- **独立した署名ページは、まだどのアプリとも連携していません。**
- **ウェブサイトは、パスキーと同じドメインで第三者のアナリティクスのスクリプトを読み込んでいます。**サイトは自分のページがパスキーを使うことを（Permissions-Policy ヘッダーで）禁止しており、鍵を保持するページにはこのスクリプトを載せていません。

## 監査されていないもの

- **Vela 自身のコントラクト。**`0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`（Gnosis。Ethereum と Base でも同じアドレス）にある[公開鍵レジストリ](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)、`0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf` にある最初のレジストリのデプロイメント（そのアドレスは、すべての登録の署名ドメインに含まれています）、そしてそれらが置き換えた以前のインデックス（`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`。読み取り専用の履歴）。これらは監査されていません。資金を持たず、所有者もおらず、アップグレードもできません。認可の層ではなく、ウォレットを見つけるための層です。資金を動かす権限は、あなたの Safe に設定された鍵からしか生まれません。現実的に起こりうる最悪の事態は、新しい端末でウォレットが見つけにくくなることであり、資金が動くことではありません。
- **Multicall3。**その README には「This contract is unaudited.」と[書かれています](https://github.com/mds1/multicall3)。Vela はこれを、残高、トークンの詳細、価格の見積もりといったまとめての読み取りにしか使わず、承認や資金には一切使いません。
- **決定論的デプロイヤー**（Arachnid の CREATE2 プロキシと Safe のシングルトンファクトリー）。エコシステムの標準でステートレスですが、正式な監査は受けていません。これらがなければ、Vela のネットワークの確認は安全側に倒れて失敗します。確認するのはそのアドレスにコードがあることで、1 バイトずつ一致するかではありません。
- **Tempo。**24 の内蔵ネットワークのひとつで、ネイティブコインがなく、Vela はそこでのガス代を pathUSD というステーブルコインで払います。2026 年 9 月時点で、Tempo の[セキュリティポリシー](https://github.com/tempoxyz/.github/blob/main/SECURITY.md)には、プロトコルはまだ監査中で、有効なバグバウンティはないと書かれています。Tempo で保有する資金と、そこで払うガス代には、このチェーンレベルのリスクが伴います。一覧の中で最も新しく、最も実績の少ないチェーンとして扱ってください。
- **Vela そのもの。**アプリ、バックエンドのサービス、そして上に挙げたコントラクトは、第三者による監査を受けておらず、その予定もありません。これがこのページで最も大きな注意点です。詳しくは [Vela is in alpha](/blog/vela-is-in-alpha) に書いています。少額から始め、コードを読んでください。

## 自分で確かめる

以下のアドレスは、どれも正規の公開デプロイメントです。[safe-deployments](https://github.com/safe-global/safe-deployments)、[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)、[EntryPoint のリリース](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0)と照らし合わせて確認してください。

| コントラクト                              | アドレス                                     |
| ----------------------------------------- | -------------------------------------------- |
| SafeL2 シングルトン v1.4.1                | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                   | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                          | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹     | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                    | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                     | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1           | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1          | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| SafeWebAuthnSigner シングルトン v0.2.1    | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                           | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| 公開鍵レジストリ（Vela、未監査）          | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ ネットワークを追加するときに確認されます。あなたの Safe は、代わりに 4337 モジュールをフォールバックハンドラーとして使っています。
