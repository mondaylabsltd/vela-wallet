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

---

## Third pass — `home.facts.lede`

Date: 2026-09-16 · 15 locales · the caption over the four fact rows
(0.85rem, `--text-tertiary`, above an `<ol>`).

**The English changed**, and this one is a rewrite rather than a rewording:

| | before | after |
|---|---|---|
| `en` | `Four things worth checking before a wallet holds your money.` | `Before you put assets in Vela` |
| `zh` | `在把钱交给一个钱包之前，值得核对的四件事。` | `把资产放进 Vela 之前` |

Gone: the count (the `<ol>` numbers the rows already), the hedge "a wallet" (the
four claims are about *this* wallet, not wallets in general), "money" → assets,
and the full stop. What is left is a moment — before the deposit, while walking
away is still free — so every locale had to become a fragment too. A locale that
closes it back into a sentence, or keeps "four", has changed what the line is.

### Realignment — all thirteen

| locale | string | the call |
|---|---|---|
| `zh-TW` | `把資產放進 Vela 之前` | differs from `zh` only in script here, and that is the honest answer: 資產 and 放進 are what Taiwan writes too. Manufacturing a difference would be worse than sharing one |
| `zh-HK` | `放資產入 Vela 之前` | Cantonese `放…入`, the verb frame this file already uses (`放錢入去`) — not the Mandarin 放進 |
| `ja` | `Vela に資産を預ける前に` | `預ける` (entrust/deposit) is the verb Japanese uses for putting money anywhere that holds it; `資産` matches `faq.items[2].a` |
| `ko` | `Vela에 자산을 넣기 전에` | `넣다` for *put in*, where the old line's `맡기다` (entrust) was carrying "holds your money" |
| `vi` | `Trước khi nạp tài sản vào Vela` | `nạp` is the word Vietnamese crypto users actually use for funding a wallet; `bỏ`/`đưa` would both be understood and neither is the register |
| `id` | `Sebelum menaruh aset di Vela` | `menaruh` — plain "put". No `Anda`: the English has no possessive and Indonesian does not need one to be polite here |
| `tr` | `Varlıklarınızı Vela'ya koymadan önce` | the possessive is not an addition — bare `varlık koymadan önce` is not Turkish. `-ınız` also keeps the locale's formal register |
| `es-MX` | `Antes de poner tus activos en Vela` | `activos` is the financial term, as in `faq.items[2].a`; tú |
| `pt-BR` | `Antes de colocar seus ativos na Vela` | `na Vela` with the article, as this locale writes the brand |
| `fr` | `Avant de mettre vos actifs dans Vela` | `actifs`; vous |
| `de` | `Bevor du Vermögenswerte in Vela verwahrst` | **deliberate departure.** This file translates "assets" as `Werte` (`faq.items[2].a`), which works inside "gleiche Adresse, gleiche Werte, gleiche Chains" and does not work alone: a standalone `Werte` reads as *values* — and this site has a values page. `Vermögenswerte` is the same word made unambiguous, and `verwahren` is the custody verb German crypto actually uses |
| `ru` | `Прежде чем положить активы в Vela` | `положить … в` is how Russian puts money somewhere; perfective, as `прежде чем` + infinitive wants |
| `it` | `Prima di mettere i tuoi asset in Vela` | `asset` untranslated, as Italian finance writes it and as `faq.items[2].a` already does |

None of the fourteen carries a count, a full stop, or a generic "a wallet".
Terminology for *assets* is each locale's own existing one, checked against
`home.faq.items[2].a`, with the German exception argued above.

Technical: no placeholders, no markup, no plurals. Longest is `de` at 41
characters — a caption at 0.85rem in a 1000px column, so length is not a risk in
any locale. `i18n:stamp` re-fingerprinted `home` in all fourteen; every one of
them was retranslated for this string in this pass.

---

## Fourth pass — `home.hero.facts[0..3].term`

Date: 2026-09-16 · 15 locales × 4 strings · the four numbered claim rows
(`link` siblings untouched — out of scope).

**All four English terms were replaced**, and the shape of the change is the
same every time: from performance to statement.

| # | before | after |
|---|---|---|
| 0 | `An unmodified Safe v1.4.1.` | `Your account is an unmodified Safe v1.4.1.` |
| 1 | `Don't bet the wallet on one device.` | `Your wallet can have more than one signing key.` |
| 2 | `One transaction on the screen. Another one signed.` | `What you see on screen isn’t necessarily what gets signed.` |
| 3 | `If Vela disappears, your wallet does not.` | `Access to your wallet doesn’t depend on Vela staying online.` |

Three of them changed more than their wording:

- **#1 stopped giving advice.** It was an imperative telling the reader what not
  to do; it is now a capability — a wallet *can* have several signing keys. No
  locale may keep the imperative, and none may promise the wallet already has
  more than one.
- **#2 gained the hedge it always needed.** "One transaction on the screen.
  Another one signed." asserts as normal what is in fact the attack. `isn't
  necessarily` / `不一定` is the true claim. This is the one place in this pass
  where the wrong translation is also the more impressive one, so it is the one
  to check: every locale had to land on its own *not necessarily* —
  `とは限りません`, `반드시 … 것은 아닙니다`, `belum tentu`, `chưa chắc`, `未必`,
  `pas forcément`, `nicht unbedingt`, `не обязательно`, `olmayabilir`, `nem
  sempre`, `no siempre`, `non sempre`.
- **#3 is now about access, not survival.** Old: the wallet outlives the company.
  New: reaching it does not depend on us being online. Weaker on paper, and the
  one we can actually prove with the self-hosting page it links to.

### Realignment — the calls worth recording

| locale | note |
|---|---|
| `zh-HK` | keeps the locale's `screen` loanword in #2, which this file already uses in `signing.heading`, and `未必`/`入到` for the hedge and for access — spoken Cantonese, not Mandarin transposed |
| `zh-TW` | 畫面 for screen, 金鑰 for key, 存取 for access — three places where Taiwan and the mainland genuinely diverge, so three places this locale is not a script conversion of `zh` |
| `ja` | #2 on `とは限りません`, the standard Japanese hedge; #1 restructured to `一つのウォレットに…複数持てます` because a literal "more than one" does not exist as a Japanese phrase |
| `ko` | #2 on `반드시 … 것은 아닙니다`; `서명 키` for signing key |
| `ru` | #1 uses `подписывающих ключей`, the term this file already uses in the FAQ for a signer — not a new coinage |
| `de` | #1 `mehrere Signaturschlüssel`, since German says *several* where English says *more than one*; #3 `kommst du weiter an deine Wallet`, which is how German says you can still get at a thing |
| `fr` | #1 `plusieurs clés de signature` for the same reason — `plus d'une` is a calque |
| `tr` | #0 drops the `-dir` copula: `Safe v1.4.1'dir` is unreadable with a version number, and UI Turkish drops it anyway |
| `vi` `id` `it` `es-MX` `pt-BR` | statements in the locale's own register; `nguyên bản`, `tidak diubah sama sekali`, `non modificato`, `sin modificar`, `sem modificações` all say unmodified without sounding like a spec sheet |

Technical: no placeholders, no markup, no plurals; `{#each … (fact.term)}` is
keyed on these strings, and all sixty are distinct. The curly apostrophes in the
English (`isn’t`, `doesn’t`) are the founder's and were kept byte-for-byte.
`i18n:stamp` re-fingerprinted `home` in all fourteen.

---

## Fifth pass — `home.why.p1` and `home.why.p2`

Date: 2026-09-16 · 15 locales × 2 strings. `p1` carries the one inline link on
this page (`account.base.app`), so the markup gate is live here.

### `p2` — rewritten everywhere

`Vela is the version we were willing to keep money in.` + **`Same passkey
sign-in. None of the lock-in.`** → **`We kept passkey signing, but made sure the
wallet doesn’t depend on us.`**

Two fragments became one sentence, and the claim moved from a result ("none of
the lock-in") to the work that produced it. Every locale had carried the
fragments faithfully — `Dieselbe Passkey-Anmeldung. Nichts davon, was einen
festhält.`, `Lo stesso accesso con passkey. Senza nessun vincolo.`, `같은 패스키
로그인, 잠금은 하나도 없이.` — so every locale was rewritten. The `but` is the
load-bearing word: kept one thing, changed the other. A locale that drops it
back into two clauses loses the concession and reads as a feature list again.

Calls worth recording: `ja` 「残したまま」 and `ko` 「그대로 두되」 both carry *kept …
but* in one clause, which is how those languages join this pair; `pt-BR` `fizemos
questão de que` (we made a point of it) is the natural Brazilian form of "made
sure"; `de` `dafür gesorgt, dass` likewise; `zh-HK` 「通行密鑰我哋照用，但唔想個錢包要
靠住 Vela」 keeps the spoken-Cantonese fronting this file uses throughout.

### `p1` — the English did not change, and mostly neither did the locales

Only the Chinese was rewritten by the founder: the 撞上那几堵墙 metaphor is gone and
the three complaints are three short clauses. `zh-TW` and `zh-HK` follow it in
their own idiom (`繞不過去` / `始終避唔開`, TW 產生/自訂網路, HK 第日…冇咗).

The other eleven keep the English paragraph's detail, including *you just have to
trust* and *nothing you can host yourself*. That asymmetry was flagged for the
founder rather than guessed at — and he closed it the same day: **put the trust
clause back into the Chinese.** It is the load-bearing half of that complaint.
`恢复密钥由浏览器生成` on its own describes a mechanism; what makes it an objection is
that you cannot check what came out of the browser, only believe it. So:

| locale | first complaint, as it now reads |
|---|---|
| `zh` | `恢复密钥由浏览器生成，你只能选择相信` |
| `zh-TW` | `復原金鑰由瀏覽器產生，你只能選擇相信` |
| `zh-HK` | `復原金鑰係瀏覽器度生成嘅，你唯有信佢` — `唯有` is how Cantonese says *you have no other option* |

All fifteen carry the clause now. The remaining Chinese compression — `服务也不能
自己部署` where the English says *nothing you can host yourself* — was left as the
founder wrote it: it is the same complaint about the same product, in fewer
words.

**What was repaired: `Then we hit the walls`.** It is an English idiom, and seven
locales had it word for word — the exact failure this second methodology exists
to catch, since a per-locale read sees a grammatical sentence and moves on.

| locale | was | now | why |
|---|---|---|---|
| `de` | `Dann kamen die Wände:` | `Dann stießen wir an die Grenzen:` | "then the walls came" is not German; `an Grenzen stoßen` is the idiom |
| `fr` | `Puis nous avons buté sur les murs :` | `Puis nous nous sommes heurtés aux limites :` | `se heurter à` is what French runs up against |
| `es-MX` | `Luego chocamos con los muros:` | `Luego nos topamos con los límites:` | `toparse con` is the Mexican everyday form |
| `pt-BR` | `Aí batemos nas paredes:` | `Aí esbarramos nos limites:` | `esbarrar em` |
| `it` | `contro i muri` | `contro il muro` | Italian does say `sbattere contro un muro` — singular. The plural was the calque |
| `ru` | `упёрлись в стены` | `упёрлись в стену` | same: the idiom is singular |
| `tr` | `duvarlara tosladık` | `duvara tosladık` | same |
| `vi` | `Rồi đụng tường:` | `Rồi chúng tôi vấp phải những giới hạn:` | `đụng tường` is not a Vietnamese figure of speech; `vấp phải giới hạn` is |
| `id` | `Lalu kami menabrak dindingnya:` | `Lalu kami mentok:` | `mentok` is exactly this — hitting the point past which a thing will not go |

`ja` 「壁に突き当たりました」 and `ko` 「벽에 부딪혔습니다」 were left alone: both
languages have that wall as a native figure of speech, so they were never calques.

Technical: the `<a href="https://account.base.app" target="_blank"
rel="noopener">Base Account</a>` anchor is byte-identical in all fourteen — the
href/tag gate in `messages.test.ts` covers it, and it passes. `i18n:stamp`
re-fingerprinted `home` in all fourteen.
