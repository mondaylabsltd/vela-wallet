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
