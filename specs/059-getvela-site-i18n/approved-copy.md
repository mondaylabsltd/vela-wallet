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
