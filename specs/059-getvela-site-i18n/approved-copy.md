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
