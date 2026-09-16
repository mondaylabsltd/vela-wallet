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

---

## Second pass — `home.hero.subtitle` only

Date: 2026-09-16 · 15 locales · same method, one string.

**The English changed.** The founder replaced the line that day:

| | before | after |
|---|---|---|
| `en` | `Sign with a passkey Vela never sees.` | `Signing happens on your device. Vela never receives your passkey.` |
| `zh` | `用通行密钥签名，Vela 永远看不到你的密钥。` | `签名发生在你的设备上，Vela 拿不到你的通行密钥。` |

Both halves moved. *Sees* → *receives* turns a claim about our conduct into a
claim about where the key is — the version A02 FR-1 actually licenses, since
"we don't look" is a promise and "it never arrives" is an architecture. And the
device is now said out loud instead of being left as an inference.

So this pass has two parts: findings against the OLD line, which were repaired
first, and then the realignment of all thirteen remaining locales to the new one.
`approved-copy.md` carries the new row; `i18n:stamp` re-fingerprinted `home` in
all fourteen, which is only honest because every one of them was retranslated
for this string in the same pass.

### Findings against the old line — fixed, then superseded

**JA-2 / KO-2 — naturalness — Medium.** `Vela が決して見ることのないパスキーで署名します。` /
`Vela가 결코 볼 수 없는 패스키로 서명합니다.` Both languages front the relative clause, so
both sentences opened with a marked subject `Vela が` / `Vela가` while the main
clause had no subject at all. The first parse available to a reader was
**"Vela signs"**, undone only on reaching `ない` / `없는` — the wrong momentary
reading for the one line whose job is to say the signing is yours. The new
copy's two-beat shape removes the structure entirely.

**RU-3 — naturalness — Medium.** `Подписывайте с помощью passkey, …` The RU-1 fix
above was right about the bug and reached for the heaviest instrument: `с помощью`
+ a relative clause is officialese in a standfirst, where the file's own
`подписывайте … своим passkey` (`home.faq.items[8].a`) was available. Also
superseded — the new line is not an imperative at all.

### The realignment — what each locale says now, and why

| locale | string | the call |
|---|---|---|
| `zh-TW` | `簽署都在你的裝置上完成，Vela 拿不到你的密碼金鑰。` | not a transcription of `zh`: 裝置 not 设备, 簽署 not 签名, 密碼金鑰 not 通行密钥 — the file's own three TW terms |
| `zh-HK` | `簽名喺你部機度做，Vela 攞唔到你嘅通行密鑰。` | spoken Cantonese, as the rest of this file is. 攞唔到, not the Mandarin 拿不到; 部機 is the word this file already uses for the device |
| `ja` | `署名はあなたの端末で行われます。パスキーが Vela に渡ることはありません。` | `渡る` — "never reaches Vela" — is how Japanese says *never receives* without naming a recipient's intent. `端末` matches `signing.lede` |
| `ko` | `서명은 당신의 기기에서 이뤄집니다. 패스키는 Vela에 전달되지 않습니다.` | `전달되지 않습니다` (is never transmitted) over a verb of receiving; `당신` keeps the register this file already uses |
| `vi` | `Việc ký diễn ra ngay trên thiết bị của bạn. Vela không bao giờ nhận được passkey đó.` | `ngay trên` carries the emphasis "on your device" is doing; second `của bạn` dropped for `đó`, which is what Vietnamese does instead of repeating the possessive |
| `id` | `Penandatanganan dilakukan di perangkat Anda. Vela tidak pernah menerima passkey Anda.` | `dilakukan` reads better than `terjadi` here — signing is done, not merely occurring |
| `tr` | `İmzalama cihazınızda gerçekleşir. Geçiş anahtarınız Vela'ya asla ulaşmaz.` | flipped to `anahtarınız … ulaşmaz` (your key never reaches Vela): Turkish puts the possessed thing first, and it is shorter than a receive-verb. `kendi` dropped — `cihazınızda` already says whose |
| `es-MX` | `La firma ocurre en tu dispositivo. Vela nunca recibe tu passkey.` | tú, as the rest of the locale |
| `pt-BR` | `A assinatura acontece no seu aparelho. A Vela nunca recebe sua passkey.` | `aparelho`, the word `signing.lede` already uses; `a Vela` with the article, as this locale does |
| `fr` | `La signature a lieu sur votre appareil. Vela ne reçoit jamais votre passkey.` | vous |
| `de` | `Signiert wird auf deinem Gerät. Dein Passkey erreicht Vela nie.` | fronted passive `Signiert wird …` is how German says this; `erreicht Vela nie` over `erhält … nie`, which would put the sentence's weight on us rather than on the key |
| `ru` | `Подпись создаётся на вашем устройстве. Vela никогда не получает ваш passkey.` | `passkey` stays indeclinable in the nominative here, so the RU-1 case trap cannot recur |
| `it` | `La firma avviene sul tuo dispositivo. Vela non riceve mai la tua passkey.` | `la passkey` feminine, as `signing.heading` already has it |

Not changed, recorded instead: `fr` renders passkey as `passkey` (31×) where
Apple and Google French say `clé d'accès`. If that is wrong it is wrong in
thirty-one places — a file-wide terminology decision, not a single-string fix.

Technical: no placeholders, no markup, no plurals in this string; the paragraph
gate counts `\n\n`, so two sentences in one paragraph is what every locale has.
Longest line is `vi` at 84 characters — two lines in a 460px standfirst, three on
a phone, which is what the English does too.
