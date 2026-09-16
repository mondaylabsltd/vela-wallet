# Approved copy — verbatim, per locale

The founder-approved strings for 059. **Use verbatim** (A02 FR-1). Until
`src/lib/i18n/messages/` exists (plan stage C), this file is where an approved
translation lives so it cannot be lost or paraphrased; stage C moves each row
into the message store and this file becomes the record of who approved what.

A locale not listed for a key has no approved wording yet — it gets a draft in
stage D, marked `drafted`, and may keep the English line until reviewed (FR-006).

## `home.tagline` — the headline

| locale | string | approved |
|---|---|---|
| `en` | `An Ethereum wallet you actually own` | founder, 2026-09-15 |
| `zh` | `真正属于你的以太坊钱包` | founder, 2026-09-15 |
| `ja` | *draft* `本当にあなたのものになるイーサリアムウォレット` | pending native review |
| `pt-BR` | *draft* `Uma carteira Ethereum realmente sua` | pending native review |
| 11 others | — | stage D |

## `home.subtitle` — the line under the headline

| locale | string | approved |
|---|---|---|
| `en` | `Sign with a passkey Vela never sees. Open source and self-hostable on most EVM chains — so it keeps working even if we disappear.` | founder, 2026-09-15 |
| `zh` | `用通行密钥签名，Vela 永远看不到你的密钥。开源、可自部署、支持大多数 EVM 链——即使 Vela 消失，钱包照常可用。` | founder, 2026-09-15 |

**Superseded the same day.** The founder cut the subtitle to its first sentence
(`Sign with a passkey Vela never sees.` / `用通行密钥签名，Vela 永远看不到你的密钥。`)
and moved the rest into the hero's fact column, where it became the hook
"If Vela disappears, your wallet does not." The notes below still govern both
halves wherever they now appear.

**Superseded again, 2026-09-16.** The current line, founder-approved and
verbatim:

| locale | string | approved |
|---|---|---|
| `en` | `Signing happens on your device. Vela never receives your passkey.` | founder, 2026-09-16 |
| `zh` | `签名发生在你的设备上，Vela 拿不到你的通行密钥。` | founder, 2026-09-16 |

*Never receives* replaces *never sees*, and the device is now stated rather than
implied. The distinction is the point: "we never look" is a promise about our
conduct, "it never arrives" is a fact about where the key lives — only the
second one survives A02 FR-1. A locale must not translate it back into the
first. The other thirteen were realigned the same day
([reviews/single-string.md](reviews/single-string.md)).
| others | — | stage D |

### Notes for translators (R7 axes)

- **"a passkey Vela never sees"** is the honesty claim, not a feature boast — it
  restates *"we architecturally can't"* (A02 FR-1). It must not weaken into "we
  don't store your keys" or "your keys are safe with us" in any locale.
- **"most EVM chains"** is deliberate and must not become "all". The page's own
  fact column says 12 built in, plus any chain with the RIP-7212 precompile and
  Vela's contracts deployed — not every EVM chain qualifies.
- **"even if we disappear"** must keep the conditional. It is a statement about
  self-custody surviving the company, not a prediction, and not a promise that
  the company will last.
- The Simplified Chinese line uses `通行密钥` for passkey; every other Chinese
  locale must pick its own market's term rather than copying this one
  (`zh-TW`/`zh-HK` conventions differ), and `zh-HK` is spoken Cantonese in this
  project's corpus.


## `home.facts.lede` — the caption over the four claims

| locale | string | approved |
|---|---|---|
| `en` | `Before you put assets in Vela` | founder, 2026-09-16 |
| `zh` | `把资产放进 Vela 之前` | founder, 2026-09-16 |

It used to read `Four things worth checking before a wallet holds your money.`
Three things changed and each one is a rule for the other thirteen:

- **No count.** The `<ol>` already numbers the rows; a line that says "four" has
  to be edited the day a fifth claim is added, and says nothing meanwhile.
- **`Vela`, not "a wallet".** The four claims are about this wallet. The hedge
  read as though the page were handing out general advice.
- **`assets`, not "money".** ETH, tokens, whatever is in there. Every locale
  already had its term for this in `home.faq.items[2].a` — use that one.
- **No full stop.** It is a caption and a fragment, in every language. A locale
  that completes it into a sentence has changed what it is.

The thirteen were realigned the same day
([reviews/single-string.md](reviews/single-string.md)).

## `home.hero.facts[*].term` — the four claims

| # | `en` | `zh` | approved |
|---|---|---|---|
| 0 | `Your account is an unmodified Safe v1.4.1.` | `账户合约是未经改动的 Safe v1.4.1。` | founder, 2026-09-16 |
| 1 | `Your wallet can have more than one signing key.` | `一个钱包可以有不止一把钥匙。` | founder, 2026-09-16 |
| 2 | `What you see on screen isn’t necessarily what gets signed.` | `你在屏幕上看到的，不一定是你实际签下的。` | founder, 2026-09-16 |
| 3 | `Access to your wallet doesn’t depend on Vela staying online.` | `即使 Vela 停止服务，你仍然可以访问自己的钱包。` | founder, 2026-09-16 |

All four were rewritten from the 2026-09-15 set, which had drifted into four
different voices — a noun phrase, a piece of advice, two fragments, a
conditional. Rules that follow from the new set:

- **They are statements, not slogans.** Each one is a claim the reader is invited
  to go and check; a locale that turns one back into an imperative ("Don't bet
  the wallet on one device") or a headline fragment has changed the register of
  the whole column.
- **#2 keeps its hedge.** `isn't necessarily` / `不一定` is the true form of the
  claim — the decoded screen is *usually* right, and the row is about the case
  where it is not. A locale that writes "is not" has made a false statement, and
  a stronger-sounding one, which is how this particular error survives review.
- **#3 is about access, not survival.** The old line said the wallet outlives the
  company; the new one says reaching it does not depend on us being up. Do not
  translate it back into "Vela will always be there" or "your wallet never
  disappears".
- **#1 says a wallet CAN have several signing keys** — a capability, not a
  recommendation and not a claim that it already does.
- The `link` strings underneath are unchanged and were out of scope.

The thirteen were realigned the same day
([reviews/single-string.md](reviews/single-string.md)).

## `home.why.p1` / `home.why.p2` — why we built it

| key | `en` | approved |
|---|---|---|
| `p1` | unchanged — `We used <a …>Base Account</a> every day and liked it. Then we hit the walls: …` | founder, 2026-09-15 |
| `p2` | `Vela is the version we were willing to keep money in. We kept passkey signing, but made sure the wallet doesn’t depend on us.` | founder, 2026-09-16 |

| key | `zh` | approved |
|---|---|---|
| `p1` | `我们每天都在用 <a …>Base Account</a>，也确实喜欢它。但用久了，有几个问题绕不过去：恢复密钥由浏览器生成，你只能选择相信；不能加自定义网络；服务也不能自己部署——哪天服务没了，钱包也就跟着没了。` | founder, 2026-09-16 |
| `p2` | `Vela 就是我们自己愿意往里放钱的那个版本。我们保留了通行密钥，但不想让钱包依赖 Vela。` | founder, 2026-09-16 |

- **`p2` stopped being a spec line.** "Same passkey sign-in. None of the lock-in."
  claimed a result; the new sentence names the work — we kept the sign-in, and
  then made sure the wallet does not need us. It also lines up with
  `hero.facts[3]`, which now says access does not depend on Vela being online.
  A locale must not turn it back into two fragments, and must not promise
  independence as a property we were given rather than one we built.
- **`p1`'s English did not change**, so the eleven non-Chinese locales keep its
  detail — including *you just have to trust* and *nothing you can host
  yourself*, which are the reasons the three complaints are complaints.
- **The Chinese `p1` did change**: the "hit the walls" metaphor is gone, and the
  three complaints are three short clauses. `zh-TW` and `zh-HK` follow it in
  their own idiom.
- **`你只能选择相信` was restored** at the founder's request, same day. The first
  draft of the new Chinese had compressed it away, leaving `恢复密钥由浏览器生成` —
  which states a mechanism and not a complaint. The whole objection is that you
  cannot check what the browser generated; you can only believe it. All fifteen
  locales now carry the clause, and the asymmetry that was open here is closed.
  `zh-HK` says it as Cantonese does: `你唯有信佢`.
- **"Then we hit the walls" is an English idiom**, and seven locales had
  translated it word for word. Repaired
  ([reviews/single-string.md](reviews/single-string.md)).

## `home.tradeoffs.items[0].body` — the relay fee trade-off

Four paragraphs became three (founder, 2026-09-16). `en`:

> Vela transactions verify passkey signatures on-chain and execute through
> ERC-4337, so they generally use more gas than ordinary transfers.
>
> By default, the relayer pays the gas and submits the transaction. The fee
> includes the on-chain cost and a relay service fee, and is fixed when you sign.
>
> You can switch relayers in settings or `<a …>run your own</a>`.

`zh`:

> Vela 的交易需要在链上验证通行密钥签名，并通过 ERC-4337 执行，因此通常会比普通账户直接转账消耗更多 gas。
>
> 默认由中继器支付 gas 并提交交易。费用包括链上成本和中继服务费，并在你签名时确定。
>
> 你可以在设置中更换中继器，也可以`<a …>自己运行一个</a>`。

What went was the whole third paragraph — "The full fee is shown before you sign
and is part of the transaction you actually sign. What you signed is what you
pay; it does not change afterwards." **`is fixed when you sign` now carries it.**
Rules for the other thirteen:

- **Exactly three paragraphs.** `messages.test.ts` enforces it, and a locale that
  keeps four is a locale still making the old argument.
- **Nothing that costs the reader money may be dropped**: the gas is higher, we
  take a service fee *on top of* the on-chain cost, and you can leave. A locale
  that loses the second one has turned a trade-off into a feature.
- **`fixed when you sign` is a claim about the moment**, not about disclosure.
  Do not restore "shown before you sign" in a locale to make the sentence feel
  complete — the source deliberately stopped saying it.
- The `title` above this body was not in scope and is unchanged.

Realigned in all thirteen the same day
([reviews/single-string.md](reviews/single-string.md)).

## `home.tradeoffs.items[1].body` — the many-keys trade-off

Six paragraphs became two (founder, 2026-09-16). `en`:

> You can add multiple keys when creating your wallet
>
> Vela is 1-of-n: any authorized key can spend from the wallet on its own.
> Adding another key gives you another way to recover access — and another entry
> point you need to protect.

`zh`:

> 创建钱包时可以设置多把钥匙
>
> Vela 是 1-of-n：任何一把已授权的钥匙都能独立花掉钱包里的钱。多一把钥匙意味着多一种恢复方式，也意味着多一个需要保护的入口。

Gone: how passkeys sync, what that makes you trust, and the two "worried
about…" recipes. What is left is the trade-off itself, which is what this
section is for — the recipes belong in the docs.

- **Two paragraphs, and the first has no full stop.** It is a lead-in, not a
  sentence; the gate enforces the count and the locales follow the punctuation.
- **`1-of-n` stays untranslated** in all fifteen. It is the property name, and
  every locale already wrote it that way.
- **Both halves of the second sentence must survive.** Another key is another
  recovery path AND another spend path. A locale that keeps only the first has
  written marketing; the sentence exists for the second.
- **Open for the founder:** the item's `title` still reads *Wherever your keys
  live, that place becomes part of your wallet's security* — written for the body
  that has just been cut, and the new body does not mention where keys live.
  Title was not in scope, so it is recorded rather than changed.

Realigned in all thirteen the same day
([reviews/single-string.md](reviews/single-string.md)).

## `home.tradeoffs.items[1].title` — the many-keys title

| locale | string | approved |
|---|---|---|
| `en` | `Every key is a way into your wallet` | founder, 2026-09-16 |
| `zh` | `每一把钥匙，都是一个独立的钱包入口` | founder, 2026-09-16 |

Replaces *Wherever your keys live, that place becomes part of your wallet's
security.*, which was written for the six-paragraph body that had just been cut.
The new title says what the two-paragraph body now says: each key is a way in.

A stray leading/trailing space in the submission was trimmed — invisible on the
page, noise in the file.

**The other two titles lost their full stops**, founder's call, 2026-09-16: this
one arrived without one, and three `<h3>`s in one list cannot be punctuated two
ways. Dropping rather than adding was also the better typographic answer — a
heading takes no terminal stop in any of the fifteen, `。` and `.` alike. So:

| # | `en` title |
|---|---|
| 0 | `Every transaction pays an extra relay service fee` |
| 1 | `Every key is a way into your wallet` |
| 2 | `You are relying on audited Safe contracts — and an audit is not a guarantee` |

`zh` already had no stops on any of the three, so it did not move. The rule for
any future item: **trade-off titles take no terminal punctuation, in any
locale.**

## `home.tradeoffs.items[2].body` — the audit trade-off

Six paragraphs became two (founder, 2026-09-16). `en`:

> Safe v1.4.1 and the WebAuthn module have both undergone third-party audits. We
> use the publicly released versions without modifying the contract code.
>
> Audits can help identify security issues, but they are not a guarantee of
> security and do not mean that every potential vulnerability has been found.

`zh`:

> Safe v1.4.1 和 WebAuthn 模块均经过第三方审计。我们使用其公开发布的版本，未修改合约代码。
>
> 审计可以帮助发现安全问题，但并非安全保证，也不代表所有潜在漏洞都已被发现。

**The claim that must never soften (A02 FR-2/FR-3)** is the second paragraph. Not
a guarantee; not every vulnerability found. A locale that renders it as "audited,
so it is safe" has made a false statement about custody in a language nobody on
the team reads. All fifteen keep both halves.

What the cut removed, and where it now lives — recorded so nobody assumes it was
lost by accident:

| removed from this item | still said elsewhere? |
|---|---|
| "The Vela app code … is written by us and published for anyone to inspect" | yes — `home.faq` "Has the code been audited?" says our app code has **not** been independently audited and none is scheduled |
| the `<a href="/docs/security-audits">Audits &amp; known issues</a>` link | yes — that page is still linked twice from this catalog (FAQ, and the account-contract row) |
| "there is no support desk that can reverse … an on-chain transaction" | **no longer on the home page** |
| "The software is provided as is, without warranty. The risk … is ultimately yours." | **no longer on the home page** |

The last two are flagged, not restored: they were the founder's to cut.

Realigned in all thirteen the same day
([reviews/single-string.md](reviews/single-string.md)).

## `home.compare.rows[1]` — the signing-key cells

| locale group | string | approved |
|---|---|---|
| `en` and the nine locales that keep the English word | `Passkey` | founder, 2026-09-16 |
| `zh` | `通行密钥` | founder, 2026-09-16 |
| `zh-TW` · `zh-HK` · `ja` · `ko` · `tr` | their own term: 密碼金鑰 · 通行密鑰 · パスキー · 패스키 · Geçiş anahtarı | — |

`Passkey or security key` in both the Vela and the Base column became `Passkey`.
The two cells have always been identical, so the row is a tie before and after.

**Recorded:** the table now mentions hardware security keys nowhere, and no other
row covers them. Defensible — a credential on a USB/NFC key *is* a passkey — and
`hero.facts[1]` plus the FAQ still say so in prose. Flagged in case the table was
meant to carry it.

## `home.compare.desc`, `home.compare.note` and `home.faq.desc` — deleted

Founder, 2026-09-16: neither the standfirst above *How Vela compares* nor the
five-paragraph note below the table needs to render. The table makes its own case
row by row; a summary next to it reads as filler.

The FAQ's standfirst went with them — *Roughly in the order people ask them…* —
for the same reason: eleven visible questions do not need their running order
explained.

**Standing rule for both sections: heading and content, no prose around it.** If
a row or a question is unclear, fix the row or the question. Every such paragraph
is also fifteen translations to keep true as the content changes.

`.section-desc` was removed from the stylesheet with them: these were its only
two users.

The key is gone from `en.ts` and from all fourteen translation files — the second
half is not optional, since a translation holding a key the English no longer has
fails `messages.test.ts` as a stale key. The `<p>` and the compare section's
spacing were adjusted with it.

**If it ever comes back** it is a new string in fifteen languages, and it has to
stay true as rows are added — which is the argument for leaving the table to
speak for itself.

## `home.networks.heading` / `home.networks.body`

| key | `en` | approved |
|---|---|---|
| `heading` | `12 networks built in. Add your own` | founder, 2026-09-16 |
| `body` | `Your wallet has the same address on every network. When you add another EVM chain, Vela checks for the required RIP-7212, Safe, and ERC-4337 support. If the required contracts are missing, you can deploy them with the <a …>chain setup tool</a>.` | founder, 2026-09-16 |

`zh`: `内置 12 条网络，也可以自己添加` / `所有网络使用同一个钱包地址。添加其他 EVM 链时，Vela 会检查所需的
RIP-7212、Safe 和 ERC-4337 支持；缺少合约时，可以用<a …>链部署工具</a>部署。`

- **The one-address claim moved, it did not go.** The heading used to carry it
  ("One address on all of them"); the body's first sentence carries it now, and
  the heading spends its second half on what the reader can *do*.
- **`RIP-7212` replaces `the RIP-7212 P-256 precompile`.** The spec number is the
  checkable thing; `P-256 precompile` was the explanation of it. No locale should
  put the explanation back.
- **Dropped: "including your own local testnet."** The tool still does it — it is
  simply no longer advertised on the home page. Recorded, not restored.
- The three spec names stay verbatim in all fifteen. The link text is now
  sentence-case (`chain setup tool`), and each locale keeps the tool name it had.
- `networks.link` was not in scope and is unchanged.

## `home.hero.headline` in all fifteen — the T058 record

FR-006: **a locale keeps the English headline until its own line is approved.**
None of these is approved by a native reader yet, so none of them is
`founder-approved` except the two at the top of this file. They are what the site
serves today, recorded so that "approved" and "shipped" cannot be confused.

| locale | headline as shipped | state |
|---|---|---|
| `en` | An Ethereum wallet you actually own | **founder-approved, 2026-09-15** |
| `zh` | 真正属于你的以太坊钱包 | drafted — R7 pass, no native reader yet |
| `zh-TW` | 真正屬於你的以太坊錢包 | drafted — R7 pass, no native reader yet |
| `zh-HK` | 真正屬於你嘅以太坊錢包 | drafted — R7 pass, no native reader yet |
| `ja` | 本当にあなたのものになるイーサリアムウォレット | drafted — R7 pass, no native reader yet |
| `ko` | 진짜 내 것이 되는 이더리움 지갑 | drafted — R7 pass, no native reader yet |
| `vi` | Ví Ethereum thực sự là của bạn | drafted — R7 pass, no native reader yet |
| `id` | Dompet Ethereum yang benar-benar milik Anda | drafted — R7 pass, no native reader yet |
| `tr` | Gerçekten size ait bir Ethereum cüzdanı | drafted — R7 pass, no native reader yet |
| `es-MX` | Una wallet de Ethereum que de verdad es tuya | drafted — R7 pass, no native reader yet |
| `pt-BR` | Uma carteira Ethereum que é realmente sua | drafted — R7 pass, no native reader yet |
| `fr` | Un portefeuille Ethereum qui vous appartient vraiment | drafted — R7 pass, no native reader yet |
| `de` | Eine Ethereum-Wallet, die wirklich dir gehört | drafted — R7 pass, no native reader yet |
| `ru` | Кошелёк Ethereum, который действительно ваш | drafted — R7 pass, no native reader yet |
| `it` | Un wallet Ethereum davvero tuo | drafted — R7 pass, no native reader yet |

The R7 pass on these lines is in
[reviews/single-string.md](./reviews/single-string.md); the one open item is
`ja`, whose 本当にあなたのものになる ("becomes yours") is a shade weaker than
"you actually own" and was already logged here as a draft pending native review.

Every locale in this table renders its own headline rather than falling back,
which is the state FR-006 permits but does not require: a locale may serve the
English line instead, and none currently does.

## `home.signing` — cut in half in the morning, cut entirely by lunch

2026-09-16. The block opened `A passkey signs a hash, not a screen.` and ran to
twenty paragraphs: the Bybit case, the gap, what today's decoding buys, what it
cannot buy, the self-hosted signing page, and a three-line aside on when to start
using it. It was first cut to eight paragraphs, and then **removed from the home
page entirely** — founder's call, both times the same reason: it is written out
at greater length in the docs, and a landing page is not where anybody reads an
essay.

Removed in all fifteen, along with its markup and CSS: `home.signing` no longer
exists as a key. Nothing was reworded on the way out, so if the block is ever
restored, git history holds the approved wording of every locale verbatim.

Where each thing it said now lives:

| the block said | still said where |
|---|---|
| a passkey signs a hash, not the sentence on screen | `home.hero.facts[2]` — *What you see on screen isn't necessarily what gets signed.* — whose whole row links to `/docs/bybit-attack` |
| Bybit, February 2025, ~$1.5B | `/docs/bybit-attack`, in full |
| ERC-7730 decoding, the unlimited-approval rewrite, the blind-sign warning | `/docs/clear-signing` |
| the self-hosted page and extension, what it does before it signs, when to use it | `/docs/clear-signing-self-host` |
| "What it cannot stop is Vela itself" — the trust boundary | `/docs/clear-signing-self-host`, *Which copy can sign for your wallet* |

**What the removal costs, flagged not fixed:** the home page no longer links to
`/docs/clear-signing` or `/docs/clear-signing-self-host` from anywhere. Both are
still in the docs sidebar, and the compare table still has a *Decoded before
signing* row and an *An extra check* row that could carry the links in their Vela
cells the way the account-contract row already does. That is a markup change, not
a copy change, and was not made.

## `home.pricing.cards[*].body` — the three paragraphs under the prices

Removed in all fifteen on 2026-09-16, same pass, same reason. A card is a title
and a price; the paragraph under it mostly restated the price — *"Open it, prove
who you are, and that's it"* under **Free**, *"Same code, open source. Build it
and install it yourself"* under **Free from source**.

Two claims went with them and were **not** put back anywhere:

| removed | still said where |
|---|---|
| the link to `github.com/mondaylabsltd/vela-wallet/releases` | nowhere. The hero's *view the code* button goes to the repo root, and the compare table links the org's repositories; neither points at the builds |
| "Never a subscription. This is how an independent team funds building Vela in the open." | nowhere on the site, docs included |

Both are flagged for the founder rather than restored: if they matter, the place
for them is a price line (`One-time purchase, never a subscription`) or a docs
page — not a paragraph added back under the card.

## `home.faq.items` — eleven questions down to seven

Founder, 2026-09-16, English and `zh` both written by him and approved verbatim.
The section had eleven questions, several of which answered something the page
had already answered above them, and two of which were no longer true.

**The rule the new list follows:** a reader arrives at the FAQ having read the
hero, the trade-offs, the comparison table, the pricing cards and the networks
section. The questions that belong here are the ones none of those answered.

| # | question | why it is here |
|---|---|---|
| 1 | What do I need to create a wallet? | the only irreversible decision — the key set is fixed at creation — lands where a reader is about to make it |
| 2 | Can I use Vela with dApps? | nothing above the FAQ says how a dApp reaches the wallet |
| 3 | What if I lose my phone? | the device fails |
| 4 | What if I delete my passkey? | the owner's own hand fails |
| 5 | What if my Apple or Google account is compromised? | the platform account behind it fails |
| 6 | What can Vela do to my money, and what does it know about me? | us, as a party that could misbehave |
| 7 | What if Vela shuts down or getvela.app goes offline? | us, as a party that could vanish |

The order widens by scope — my hand, my device, my platform account, the company,
the company's absence — and the two questions about Vela sit together at the end,
where a reader who has decided they want it asks who they are trusting.

**Cut, and where each answer already lived:**

| cut question | already answered by |
|---|---|
| Do I need a seed phrase, an extension, or a hardware wallet? | the hero and `home.hero.facts` |
| Can I add a second key — another device, or a YubiKey? | `home.hero.facts[1]` and `tradeoffs.items[1]`; the "chosen at creation" rule is now inside FAQ 1, 3 and 5 |
| What does it cost to use? | `tradeoffs.items[0]` and the pricing cards |
| Which chains and tokens does it hold? | the networks section, directly above the FAQ |
| Has the code been audited? | `tradeoffs.items[2]`, which links `/docs/security-audits` |

**Two answers were not cut but wrong, and that is why they are gone:**

- *Can I use it with dApps?* told the reader to pair with the **WalletPair**
  extension. WalletPair is not in the codebase any more — it survives only in
  `specs/` history and in the site's footer and roadmap, both of which are now
  stale too and are flagged here, not fixed. What actually ships: the browser
  extension injects an EIP-1193 / EIP-6963 provider into the page
  (`app-web/vela-wallet/extension/`), and the desktop, iOS and Android apps open
  dApps in a browser of their own.
- *What happens if Vela shuts down?* offered "the open-source recovery extension"
  as the way to use an existing passkey elsewhere. That extension was deleted in
  spec [039](../039-retire-expo-tree/spec.md). The replacement is real and is
  documented: `app-web/clearsigning` loaded as a Chrome extension, whose relying
  party stays `getvela.app`, so an existing key signs in code the owner read.

**Structural notes:**

- Every answer is **two beats separated by a blank line** except 4, which is one.
  The component splits on `\n\n` like `tradeoffs`; `messages.test.ts` holds every
  locale to the same paragraph count.
- **No links at all.** The old answers carried seven. Each of these is a complete
  answer rather than a doorway, and the docs they used to point at are all in the
  docs sidebar. If a link comes back it is fifteen files, so it should come back
  for a reason.
- The `freeze` claim survives here in its accurate form: Vela cannot freeze your
  funds, **and** a token issuer can still blocklist an address. The old answer
  said the first half only.

**`zh` says 通行密钥, not `passkey`.** The founder's own draft wrote `passkey`
bare; asked about it the same day, he chose the file's existing term, so FAQ 3, 4
and 5 read 通行密钥 like every other `zh` string. The English keeps `passkey`,
which is the word English-language readers use.

## `home.faq.items[6]` — the way out, corrected

Founder, 2026-09-16, after the first version shipped. The answer had claimed that
**existing passkeys keep working in the iOS, Android and desktop apps** if the
domain goes dark. An installed app does keep working, but Apple and Google fetch
the app–domain association from `getvela.app`, so a fresh install on a new device
after the domain is permanently gone is not something we can promise. The
promise moved to the two routes that do not depend on it:

| route | why it survives the domain |
|---|---|
| the Vela browser extension | a Chrome extension asserts `getvela.app` as its relying party through `host_permissions`, not by fetching anything from the domain |
| `app-web/clearsigning` as an extension | same rule, and the folder is dependency-free, so what you load is what you read |

And the key itself need not be the platform passkey on the machine in front of
you: a **USB/NFC security key**, or a **nearby phone reached by QR code** (the
WebAuthn hybrid transport), signs in either extension. That is the sentence's
real payload — the escape hatch does not run through an Apple or Google account.

`en`: *If getvela.app goes offline, sign from a browser: the Vela extension, or
the dependency-free clear-signing extension. Your existing keys work in both —
the passkey on this device, a USB security key, or a nearby phone reached by QR
code.*

Each locale names the clear-signing extension with its own docs-sidebar term
(`de` Klartext-Signatur, `fr` signature lisible, `ja` クリア署名, `zh` 清晰签名 …)
rather than transliterating the English.

## WalletPair — removed from the site

Founder, 2026-09-16: **the wallet does not support WalletPair any more**, so the
site stops pointing at it. Three places carried it:

| where | now |
|---|---|
| `home.faq` — "pair your wallet with the WalletPair extension" | replaced by the truthful dApp answer in the seven-question list |
| `chrome` footer, *Infrastructure* column | link deleted (a proper noun in the component, so no catalog key changed) |
| `roadmap.shipped[3]` — *WalletPair dApp connect* | rewritten in fifteen locales as **dApp connect in the browser**: *The wallet appears in the page like any other browser wallet — through the Vela extension, and through the browser built into the desktop, iOS and Android apps.* |

The roadmap entry was rewritten rather than deleted because the thing it records
did ship — it simply shipped as provider injection, not as pairing. Deleting the
row would have left the site with no mention of dApp connect at all.
