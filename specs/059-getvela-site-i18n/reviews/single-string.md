# Single-string review — the three strings every visitor reads

Method: <https://shelchin.com/prompts/single-string-i18n-locale-review/>.
Reference frame: **English and Chinese** (founder, 2026-09-15) — the two the
founder reads, so a locale is judged against what those two say, not against a
back-translation into English.

Strings: `home.hero.headline`, `home.hero.subtitle`, `home.hero.ctaCreate`.
Date: 2026-09-15 · 15 locales.

## Findings

### RU-1 — `home.hero.subtitle` — mistranslation — **High** — fixed

`Подписывайте passkey, который Vela никогда не видит.`

`passkey` is indeclinable in Russian, so with no preposition it lands in the
accusative and the sentence reads **"sign the passkey"** — you are signing the
key rather than signing *with* it. It is the one string on the page that states
the product's central mechanism, and it stated the opposite of it.

The same file already writes `с помощью Face ID` elsewhere, so this was also
internally inconsistent.

Fixed: `Подписывайте с помощью passkey, который Vela никогда не видит.`

### Register — no findings

Each locale is internally consistent between the headline and the body of its
own page, which is what the axis asks:

| locale | register | headline agrees |
|---|---|---|
| de, it, es-MX | informal (du / tu / tú) | yes — `dir gehört`, `davvero tuo`, `es tuya` |
| fr, ru, tr, id, pt-BR | formal (vous / вы / siz / Anda / você) | yes |
| ja, ko | です・ます / 합니다 | yes |
| vi | neutral `bạn` | yes |
| zh, zh-TW, zh-HK | plain 你 / 你 / 你 | yes |

### Terminology for "passkey" — differentiated correctly

`zh` 通行密钥 · `zh-TW` 密碼金鑰 · `zh-HK` 通行密鑰 · `ja` パスキー · `ko` 패스키 ·
`tr` geçiş anahtarı · everyone else keeps `passkey`, which is what those markets'
own tech press uses. This is the R7 rule that regional variants are not each
other's copies, and it holds.

### CTA — no findings

English `Getting started` is a gerund; every locale renders it as an imperative
(`Empezar`, `Başlayın`, `시작하기`, `开始使用`). That is better UI copy in those
languages than a literal gerund would be, and it is the same promise. Not a
finding.

### JA-1 — `home.hero.headline` — **Low** — open

`本当にあなたのものになるイーサリアムウォレット` is the draft recorded in
[approved-copy.md](../approved-copy.md) as *pending native review*. `になる`
("becomes yours") is a shade weaker than "you actually own". Left as is: it is
the founder-facing record's own open item, not a defect introduced here.
