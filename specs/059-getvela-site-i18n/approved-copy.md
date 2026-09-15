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
