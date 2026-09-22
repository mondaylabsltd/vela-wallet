# ja — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), acting as native Japanese localizer · Method: single-string locale review, whole pages

Scope: `messages/ja.json` (every key the brief lists, plus the chrome drafts) and all 17 docs in
`content/docs/ja/`, `self-hosting.md` new. Every page was rewritten from the current English and
Chinese; the old Japanese was used only for terminology and phrasing whose meaning still matched
(chiefly `why-vela`, `signers`, `clear-signing-self-host`).

## Terminology and register

| Concept | ja | Note |
| --- | --- | --- |
| key (any signing credential) | 鍵, counted with 本 | the app counts keys the same way (`どれか 1 本だけでサインインできます。最大 7 本。`); the old catalog mixed つ and 本 |
| passkey | パスキー | 059 choice, kept |
| security key | セキュリティキー / ハードウェアセキュリティキー / USB セキュリティキー | the app's words (`onboarding.create.methodSecurityKeyTitle`) |
| relay | リレー; relay fee リレー手数料 | the English says relay, not relayer (C-relay-1). ERC-4337's role name given once as バンドラー; vela-relay's internal hot wallets are リレーヤー, as the relay's own docs call them |
| registry | レジストリ (公開鍵レジストリ) | the on-chain contract; the app says レジストリ |
| public-key index | 公開鍵インデックス | the service; the Settings field is named by its app label パスキーインデックス |
| self-hosting | セルフホスティング (guide: セルフホスティングガイド); 自分で動かす (verb, sidebar group) | |
| signing page | 署名ページ | |
| clear signing / blind signing | クリア署名 / ブラインド署名 | ブラインド署名 is the app's term |
| relying party | リライングパーティー | FIDO Alliance Japanese usage; English given once in parentheses |
| descriptor / treasury | ディスクリプター / トレジャリー | both as the app writes them |
| transaction | 取引 | the site's established word; UserOperation left in English |
| sign in | サインイン | the app's own screen title (`onboarding.login.header`) |

UI paths are the app's Japanese labels, not translations of the English ones: 設定 → 詳細設定 →
サービスエンドポイント, 設定 → ネットワーク, speeds 低速 / 標準 / 高速, buttons 送る / 受け取る /
ウォレットを作成, fields チェーンデータインデックス / パスキーインデックス / VELA RELAY / 法定通貨レート,
デフォルトに戻す. The clear-signing intents are the app's 送る / 承認 / 交換.

Register: です・ます throughout, as in 059; table cells and list fragments in plain form. Two
departures from the 059 text, both deliberate: the docs say トレードオフ for "trade-off" (the old 代償
reads as "a price paid", which is wrong for "choose which risk you'd rather live with"; the landing
headings 先に、代償の話を and 代償について正直でいる keep 代償, where "costs" is the meaning), and
`chrome.nav.signIn` moves from ログイン to サインイン to match the app screen it opens.

Typesetting: every Japanese paragraph and list item is on one line. The old files hard-wrapped
Japanese mid-sentence; a newline between two Japanese characters can render as a stray space.
No space after a bold run that ends in 。.

## Findings

Every High and Medium below was in the OLD text and is fixed. Where a finding is a stale fact, the
cause is that the old Japanese translated an older English; the effect on a Japanese reader is the
same as a mistranslation.

| # | File / key | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `home.meta.description`, `.ogDescription`, `.organization` | シードフレーズも、ハードウェアキーも、囲い込みもありません | mistranslation (stale) | High | says there are no hardware keys; security keys are one of the three kinds of key (C-keys-2) | パスキーかセキュリティキーで署名…改変していない Safe…サービスもすべてオープンソース |
| 2 | `home.hero.facts[3]` | Vela がサービスを止めても…／アプリもリレーもすべてのバックエンドも、自分で動かせます | mistranslation (overclaim) | High | full self-hosting with no caveat; the relay needs a code change and passkeys stay tied to getvela.app | すべてオープンソースで、自分で動かすこともできます／…まだ私たちに依存している部分 |
| 3 | `home.hero.facts[2].link` | 私たちがそれを塞ぐ方法 | mistranslation | Medium | 塞ぐ says the path is closed; the page says mitigated, not eliminated | Vela の取り組み |
| 4 | `home.why.p1` | href `account.base.app`; walls list of the old English | technical / stale | Medium | href differed from English (tag gate failed); list no longer matched | href fixed; リカバリーフレーズ…コードが公開されていない署名サービス |
| 5 | `home.tradeoffs.items[0]` | 既定では、ガスはリレーヤーが支払い…オンチェーン費用とリレーサービスの料金 | mistranslation (stale) | High | understates the fee: 3 × reserved gas at the higher price, often 10× the real cost (C-fee-1) | three paragraphs, link to `/docs/self-hosting#relay` |
| 6 | `home.tradeoffs.items[1].body` | ウォレットを作るときに、鍵を複数登録できます (no stop) | mistranslation (qualifier lost) | High | drops "up to seven" and "can't be changed afterwards" (C-keys-1) | 最大 7 本まで選び、あとから変更することはできません |
| 7 | `home.tradeoffs.items[2]` | あなたが信頼するのは監査済みの Safe コントラクトです — そして監査は保証ではありません | mistranslation (posture) | High | never says Vela's own code is unaudited with none scheduled, which this trade-off must say (C-audit-1) | コントラクトは監査済みですが、Vela 自身のコードは監査されていません…その予定もありません |
| 8 | `home.compare.rows` | 12 rows; 独立した、自分で動かすページや拡張で確認できる; アプリ、リレー、バックエンドまで自分で運用できる | technical / mistranslation | High | row count ≠ English made `home` a fragment (the page fell back to English); two cells claimed unshipped capabilities (C-signpage-1, self-hosting gaps) | 13 rows, new あとから鍵を追加 row, ソースコード row |
| 9 | `home.compare.rows` (MetaMask cells) | ガスの肩代わり：なし; オープンソースの範囲：ウォレットのクライアント | stale | Medium | MetaMask sponsors on some networks; its licence is non-commercial only | 一部のネットワークで対応; 公開。ただしライセンスは非商用利用のみ許可 |
| 10 | `home.pricing.cards` | デスクトップとモバイルのアプリ／ソースからなら無料; アプリストアから／買い切り | mistranslation (stale) | High | desktop is a free download; store apps don't exist yet (C-plat-1) | ウェブ、ブラウザ拡張、デスクトップ／無料 … 買い切り · 近日公開 |
| 11 | `home.networks` | 12 のネットワークを内蔵; link to biubiu.tools | stale / technical | High | 24 networks (C-net-1); href ≠ English | 24 のネットワークを内蔵。自分でも追加できます; `/chain-setup` |
| 12 | `home.faq.items[0].a` | Face ID か指紋で解除できる端末、または USB/NFC のセキュリティキー | mistranslation | High | omits that security keys alone need two (one unsynced key cannot create a wallet); face/fingerprint only (C-auth-1) | …セキュリティキーだけで作る場合は 2 本必要です |
| 13 | `home.faq.items[1].a` | …ウォレットを dApp に直接組み込みます／iOS・Android・デスクトップ | stale | Medium | missing that the web wallet doesn't connect; desktop is macOS/Windows only (C-dapp-1) | …ウェブウォレットは dApp に接続できません |
| 14 | `home.faq.items[4].a` | 同期されたパスキーに手が届いた人は… | mistranslation (instruction lost) | High | omits that a key can't be removed and funds must move — the one thing to do | しかも鍵は削除できません。…新しいウォレットを作り、資金を移してください |
| 15 | `home.faq.items[5].a` | ウォレットを動かせるのはあなたの鍵だけです…公開鍵、ウォレットアドレス、ウォレット名 | mistranslation (understatement) | High | hides that Vela writes the signing software, that authenticator kind and key labels are public, and that services see your IP | full answer per English |
| 16 | `home.faq.items[6].a` | …依存関係ゼロのクリア署名拡張です | mistranslation | High | the signing page can't operate a wallet alone and isn't connected (C-signpage-1); relay caveat missing | Vela のブラウザ拡張…自分でビルドしたアプリ; relay code change stated |
| 17 | `about.lede`, `about.team.bio` | 顔の見えない会社はありません; ウォレット、コントラクト、そしてこのサイト | mistranslation (stale) | Medium | there is a company (MONDAY LABS LTD); "contracts" implies Vela wrote contracts in the funds path (C-acct-1) | 英国の小さな会社 MONDAY LABS LTD…; アプリ、サービス、そしてこのサイト |
| 18 | `about.values[0..1]` | あなたの鍵、あなたのコイン — これはスローガンではなく構造です; 暗号資産が失われる最大の原因は…あなたの顔か、指紋に | cultural risk / mistranslation | Medium | slogan voice and an unsourced superlative; omits "what we control is the software"; face/fingerprint only | plain statements per English |
| 19 | `roadmap.upcoming` | …独立したセキュリティ監査; P-256 プリコンパイルのないチェーン向けの署名経路; アカウントはプラットフォームのバックアップで付いてきます | mistranslation (stale) | High | an audit listed as upcoming (never allowed); two false technical promises (C-p256-1, C-sync-1) | the 5 new items |
| 20 | `roadmap.shipped` | 7 old items | technical | High | array length ≠ English made `roadmap` a fragment | the 10 new items |
| 21 | `getStarted.fundingNote` | …同じアプリで、料金はかかりません | mistranslation | High | a self-built phone app can't use the phone's own passkey | …別のスマホかセキュリティキーで署名します |
| 22 | `getStarted.platforms.desktop.stores` | Mac App Store · Microsoft Store | stale | Medium | no Mac App Store listing | Microsoft Store |
| 23 | `getStarted.meta.description`, `.lede`, `.platforms.web.blurb` | …同じコードから作られています; スマホに…持っていってください; パスキーで本人確認 | stale | Medium | implies phone apps are available now; omits that the web wallet doesn't connect to dApps | per English |
| 24 | `chrome.nav.signIn` | ログイン | terminology | Low | the app screen it opens says サインイン | サインイン |
| 25 | `chrome.docs.titles.install` | Vela を使い始める | UI fit | Low | the page is about which app to install; now matches the page title | Vela をインストールする |
| 26 | `home.hero.facts[1].link` | 最大 7 つの鍵 | terminology | Low | keys are counted with 本 | 最大 7 本の鍵 |
| 27 | introduction, faq, networks-and-fees, whitepaper, security-audits | 12 のネットワーク | mistranslation (stale) | High | 24 (C-net-1) | 24 のネットワーク |
| 28 | networks-and-fees, faq, whitepaper | ガスアカウントの有効化デポジット（返金されない） | mistranslation (stale) | High | the mechanism no longer exists (C-fee-2) | トレジャリー補充（任意・返金なし・手数料に充てられない） |
| 29 | whitepaper (fees) | 隠れた上乗せはありません | mistranslation (framing) | High | contradicts C-fee-1 | 3 × reserved gas; often 10× or more; the relay keeps the difference |
| 30 | whitepaper (threats) | Vela のサーバー侵害 —— 署名能力は得られません。影響範囲はサービスの劣化 | mistranslation (false) | High | a compromised app delivery can present a malicious transaction; backend compromise can mislead | backend services and app delivery listed separately |
| 31 | whitepaper, recovery | プラットフォームのパスキーのバックアップが、あなたの復旧手段です | mistranslation (stale) | High | recovery is any one of up to seven keys plus the registry (C-keys-1, C-sync-1) | 鍵こそが、あなたの復旧手段です |
| 32 | account-contract | Safe 互換のインターフェースなら何でも操作できます | mistranslation (false) | High | tools can read and build; signing needs a getvela.app signature (C-safeui-1) | per English |
| 33 | install, create-wallet | web-only install page (モバイルアプリは近日公開予定); `dapps` and `what-is-public` anchors missing | technical / stale | High | pages linked from elsewhere had no target anchor (test failed); extension and desktop missing | rewritten, anchors in place |
| 34 | introduction, passkeys, create-wallet, send-and-receive | 顔や指紋で使うパスキー; 顔や指紋だけで操作できます | mistranslation | Medium | verification also by device passcode or security-key PIN (C-auth-1) | Face ID、指紋、端末のパスコード、セキュリティキーへのタッチと PIN |
| 35 | clear-signing | 無制限の承認はブロックされます…あなたが選んだ有限の額に書き換え | mistranslation (overclaim) | High | signed permits aren't capped; the wallet doesn't rewrite on its own (C-approve-1) | 「無制限」のオンチェーン承認は送信できません + what it does not stop |
| 36 | clear-signing | 検証済み (no caveat) | mistranslation | Medium | "verified" is not cryptographic (C-clear-1) | 暗号的に検証されたという意味ではありません |
| 37 | bybit-attack | 攻撃が頼った「アップグレード」という原始的な仕掛けを取り除き | mistranslation (false) | High | the owner-signed `delegatecall` exists in every Safe, Vela's included | これで*なくならない*もの… |
| 38 | bybit-attack | self-call gap absent; signing page 「公開後は任意の機能」 | stale | High | the one actionable warning (reject requests targeting your own address) was missing; status overstated | two limits paragraph; status per C-signpage-1 |
| 39 | bybit-attack | 侵害されうります | unnatural | Medium | not a Japanese form (うる/える conflated) in the page's key disclaimer | 侵害される可能性があります |
| 40 | signers | それが取引であり | mistranslation | Medium | "trade" rendered as 取引, this site's word for "transaction" | それがトレードオフであり |
| 41 | signers | 作成時に YubiKey を登録し、それで署名します; no compromise section | stale | High | a one-security-key wallet can't be created; missing "a key can't be removed — move everything" | 2 本で作成; 鍵が漏れたかもしれないとき |
| 42 | clear-signing-self-host | no status paragraph; ネットワーク通信をしません | stale | High | read as ready for real signatures; it does load token logos | status paragraph; ネットワークからデータを取得しません（ロゴのみ） |
| 43 | all docs | Japanese lines hard-wrapped mid-sentence | technical | Low | a newline between Japanese characters can render as a stray space | one line per paragraph |

Counts: **27 High, 12 Medium** fixed; 4 Low noted and fixed.

The `chrome.docs.*` drafts (`groups.keys` 鍵と復旧, `groups.selfHost` 自分で動かす,
`titles["self-hosting"]` and `footer.links.selfHosting` セルフホスティングガイド, all of `docs.ui`)
were reviewed and kept: each is what a Japanese docs site says (このページの内容, GitHub でこのページを編集,
← 前へ / 次へ →).

## Open items

- `home.hero.headline` 本当にあなたのものになるイーサリアムウォレット (and the same line in
  `chrome.footer.tagline`) — still spec 059's JA-1, Low, untouched here as instructed. になる reads a
  shade weaker than "actually own".
- Not a site defect, for whoever owns the app corpus: the app's fee and funding strings say
  `Bundler 手数料` and リレーヤー where the site now says リレー / リレー手数料 (the English says relay).
  The site follows the English; the app strings may want the same word.

## Result

reviewed — no open High or Medium findings

## Update 2026-09-22

Carried the en + zh revision of the same day (fee wording, configurable relay chain
directory, hero subtitle, facts #3 and #4) into ja. Each changed string checked on the
five single-string axes; no High or Medium left open. Docs keep one line per paragraph.

| String / section | Change | Severity of anything fixed beyond the brief | Note |
| --- | --- | --- | --- |
| `home.hero.subtitle` | 署名はあなたの端末で行われます。パスキーの秘密鍵が Vela に渡ることはありません。 | — | 秘密鍵 as the docs already say; first sentence already meant "is done" |
| `home.hero.facts[2]` | 画面で見たものが、そのまま署名されます。Vela は承認の前に、署名する取引そのものをデコードします。 / …と、Vela が署名の中身を見せるしくみ | — | Japanese has no settled term for WYSIWYS, so it is said plainly; it answers the old term 画面に見えているものが、署名されるものとは限りません |
| `home.hero.facts[3]` | term = 059 string; link Vela がなくなっても、ウォレットを使い続ける方法 | — | 059 Vela がサービスを止めても、あなたのウォレットにはアクセスできます。 **kept**: same meaning as en and zh (即使 Vela 停止服务…), natural. The 080 finding #2 against it rested on the relay code change, which no longer exists; the overclaiming 059 link is not restored |
| `home.tradeoffs.items[0].body` | paragraph 2 rewritten (one fee to the relay, Vela's unless changed; formula + `#fee` link 計算方法; relay pays the gas and keeps the rest) | — | both hrefs identical to en; paragraph 3 unchanged |
| `home.faq.items[6].a` | code-change sentence replaced by the four services you can run | — | second paragraph untouched |
| `roadmap.upcoming[1].body` | chain-data clause removed | — | |
| docs `networks-and-fees` | `<span id="fee">`; "10 倍以上" paragraph replaced; **受け取るのは誰か。** paragraph | — | |
| docs `faq` | cost bullet (relay choice, `#fee` link); shutdown answer without the code-change parenthesis | — | |
| docs `whitepaper` | intro sentence dropped; Fees bullet + new "who gets the fee" bullet; 「Vela がなくなったら」 sentence dropped | — | |
| docs `self-hosting` | intro limit dropped; `VELA_RELAY_CHAIN_DIRECTORY_URL` comment lines in both code blocks; 知っておくこと bullet; chain-data paragraph; relay line removed from the final list | — | チェーンディレクトリ / Vela のコピー as the file already said |

Nothing fixed beyond the brief.
