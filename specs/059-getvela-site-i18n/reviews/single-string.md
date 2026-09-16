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

---

## Sixth pass — `home.tradeoffs.items[0].body`

Date: 2026-09-16 · 15 locales · the first trade-off (relay fee). Four paragraphs
in, three out, one inline link, and the paragraph gate live throughout.

The English lost its third paragraph — "The full fee is shown before you sign and
is part of the transaction you actually sign. What you signed is what you pay; it
does not change afterwards." — to four words in the second: **`and is fixed when
you sign`**. A trade-off that spends a quarter of itself defending the thing it
is admitting stops reading as an admission.

**The gate did its job on the way through.** With the English rewritten and the
translations untouched, `messages.test.ts` failed fourteen times with
`items[0].body: 3 paragraphs in English, 4 translated`. That is the drift this
test was added for on 2026-09-15, caught the same day it was created rather than
a week later.

### What every locale had to keep

Three things cost the reader money, and all three survive in all fifteen:

1. the gas is higher than an ordinary transfer, and why (on-chain passkey
   verification + ERC-4337);
2. the fee is the on-chain cost **plus** a service fee — a locale that merges
   them into "the fee" has turned a trade-off into a feature;
3. you can leave: switch relayers, or run one.

What must NOT come back is "shown before you sign". The source stopped saying it;
a translator filling the sentence out to feel complete would be restoring a
claim the founder cut.

### Per-locale notes

| locale | note |
|---|---|
| `ja` `ko` | the link now sits mid-sentence (`自分で動かす` / `직접 돌릴`) because both languages end on the verb; the anchor wraps the verb phrase, and the sentence closes outside it |
| `tr` | same shape: `kendiniz çalıştırabilirsiniz` is the whole predicate, so the link carries it |
| `de` | `sie steht fest, sobald du signierst` — German says a price *stands fixed*; `fixiert` would read as jargon |
| `ru` | `фиксируется в момент подписи` — reflexive, which is how Russian states a rule rather than an action someone performs |
| `it` | `trasferimento`, not `bonifico`: the old text called an on-chain transfer a bank transfer |
| `zh-TW` | keeps 中繼 where `zh` has 中繼器 — this locale's existing term, not a slip |
| `zh-HK` | 「喺你簽名嗰陣就定咗」 for *fixed when you sign*, which is the spoken form; 定咗 not 確定 |
| `vi` | `được chốt ngay khi bạn ký` — `chốt` is the word Vietnamese uses for a number being locked in |
| `id` | `besarnya sudah tetap begitu Anda menandatangani` — `begitu` for *the moment that*, which is tighter than `ketika` |

Technical: exactly 3 paragraphs in all fifteen; one `<a>` per locale with the
`vela-relay` href byte-identical; `prettier` reformatted the English string over
two lines and the JSON files were already clean; `i18n:stamp` re-fingerprinted
`home` in all fourteen. 189 tests pass.

---

## Seventh pass — `home.tradeoffs.items[1].body`

Date: 2026-09-16 · 15 locales · the second trade-off (many keys). Six paragraphs
in, two out.

What the English dropped: the passkey-sync explanation, the "which means you have
to trust the devices, the accounts and the hardware" line, and the two recipes
("Worried about losing a device… Worried about depending on a single
provider…"). What it kept is the only part that is actually a trade-off:

> Vela is 1-of-n: any authorized key can spend from the wallet on its own.
> Adding another key gives you another way to recover access — and another entry
> point you need to protect.

**The test for every locale was the second half of that sentence.** Another key
is another way in — not only another way back. A translation that keeps the
recovery half and softens the spend half has quietly turned the section's second
admission into a feature, and it would read perfectly well. All thirteen keep
both: `ein Zugang mehr, den du schützen musst`, `otra entrada que tienes que
proteger`, `une entrée de plus à protéger`, `satu pintu lagi yang harus Anda
jaga`, `korumanız gereken bir kapı daha`, `thêm một cửa bạn phải giữ`,
`守るべき入り口も一つ増えます`, `지켜야 할 입구도 하나 늘어납니다`, `ещё один вход, который придётся
защищать`, `多一個需要保護的入口`, `多一個要你睇實嘅入口`.

Other calls:

- **The first paragraph has no full stop** in English or Chinese — it is a
  lead-in. Every locale follows, including `ja` (`…登録できます`) and `ko`
  (`…등록할 수 있습니다`), where a bare `ます`/`습니다` without `。`/`.` is unusual in
  prose but correct for a line that heads a paragraph.
- **`1-of-n` is left untranslated everywhere.** It already was, in all fourteen.
- `zh-TW` and `zh-HK` reuse their own earlier image for the recovery half —
  `多一條把錢包拿回來的路` / `多一條攞得返個錢包嘅路` — rather than calquing 恢复方式 from `zh`.
- `de` keeps `ein Zugang mehr`, which this locale had already coined for the
  paragraph now deleted; the phrase survives its source.

Recorded, not changed: the item's `title` still describes the cut body. Flagged
in `approved-copy.md`.

Technical: exactly 2 paragraphs in all fifteen, no links in this string, no
placeholders. `prettier` clean, 189 tests, `i18n:stamp` re-fingerprinted `home`.

---

## Eighth pass — `home.tradeoffs.items[2].body` and `items[1].title`

Date: 2026-09-16 · 15 locales × 2 strings. The audit trade-off goes from six
paragraphs to two; the many-keys title is replaced to match the body cut in the
seventh pass.

### The title

`Wherever your keys live, that place becomes part of your wallet's security.` →
`Every key is a way into your wallet`

The old title described the body that had just been deleted. The new one states
the same thing the body now states, so the row reads as one claim rather than
two. Locale notes: `de` `ein Weg in deine Wallet` and `it` `un ingresso` reuse
the exact nouns their own bodies use two lines below, so the row rhymes with
itself; `tr` `cüzdanınıza açılan bir kapı` (a door that opens onto your wallet)
is the Turkish figure, where a literal *yol* would read as a route; `ja`
「鍵はどれも、ウォレットへの入り口です」 puts `どれも` on the key rather than a count, which is
how Japanese says *every*.

No full stop, in any locale — the source has none. That leaves this list with two
punctuated titles and one not; flagged in `approved-copy.md` rather than fixed,
because the string is founder-approved verbatim.

### The body — the one string on this site where a good translation can lie

Everything else in this pass is craft. This paragraph is liability:

> Audits can help identify security issues, but they are not a guarantee of
> security and do not mean that every potential vulnerability has been found.

Both halves have to survive in all fifteen, and the failure mode is not a
mistranslation — it is a *tidier* sentence. "Audited by third parties, so your
funds are protected" would read better than what the English says, which is
precisely why `messages.test.ts` has a posture check and why this axis is read
first. Checked, and all fifteen hold: `non è una garanzia di sicurezza`,
`keine Garantie für Sicherheit`, `не гарантия безопасности`, `güvenlik garantisi
değildir`, `bukan jaminan keamanan`, `không phải là bảo đảm an toàn`,
`안전을 보증하지는 않으며`, `安全性の保証ではなく`, `並非安全保证`, `唔等於安全保證` — and
each keeps the second clause too (*not every vulnerability has been found*),
which is the half a translator drops as redundant.

Terminology: each locale keeps its own audit word, unchanged from the rest of its
file — `zh` 审计, `zh-TW` **稽核** (14×, and it has no 審計 anywhere), `zh-HK` 審計,
`ja` 監査, `ko` 감사, `tr` denetim, `ru` аудит, the Latin-script locales `audit`.

### What the cut removed

The link to `/docs/security-audits` is gone from this item, along with the "as
is, without warranty" sentence and "no support desk can reverse an on-chain
transaction". The audits page is still linked twice elsewhere in the catalog, and
the FAQ still says our own app code is not independently audited — so the
honesty posture survives the cut. The two disclaimer sentences do not appear
anywhere else on the home page. Recorded in `approved-copy.md`; not restored.

Technical: this string now has **no** `<a>` in any locale, which is what the tag
gate compares against the English; exactly 2 paragraphs everywhere; prettier
clean, 189 tests, `i18n:stamp` re-fingerprinted `home`.

### Addendum — the trade-off titles lose their full stops

Founder's call, same day. `items[1]` arrived without one, and three `<h3>`s in
one `<ol>` cannot be punctuated two ways. Dropping was the right direction rather
than adding: a heading takes no terminal stop in any of the fifteen — Japanese
and Chinese headings drop `。` by the same convention that English headings drop
`.`, so `ja` 「…手数料がかかります」 and `ko` 「…수수료를 냅니다」 now read as headings
instead of as sentences that wandered into a heading slot.

Stripped from `items[0].title` and `items[2].title` in all fifteen. `zh` had
never carried them and did not move — the one locale that had been right about
this all along. Standing rule: trade-off titles take no terminal punctuation.

## `home.signing` in all fifteen — cut, then removed, 2026-09-16

Two passes in one day, and the second one ate the first.

**The cut.** Twenty paragraphs to eight, by deleting paragraphs and joining the
survivors — no sentence rewritten, so every locale kept its approved wording.
Paragraphs merged with a space in the eleven Latin- and Cyrillic-script locales
and in `ko`, with nothing in `zh`, `zh-TW`, `zh-HK` and `ja`, whose sentences end
in `。` and take no following space. One deixis had to move: `lede` said *the
plain language **above it***, pointing at a hash sentence the cut had removed, so
fourteen locales were changed to say *on screen* in their own idiom.

**The removal.** The founder then cut the section itself. `home.signing` is gone
from `en.ts` and from all fourteen translation files, along with its markup and
its CSS; the record of what it said and where that content now lives is in
[approved-copy.md](../approved-copy.md).

What is worth keeping from the reading pass, because it is the general lesson and
not about this block: **a deletion pass needs its own review in every language.**
What breaks is not the translation but the seam — a pronoun or a demonstrative
whose referent was in a paragraph that is no longer there, which reads perfectly
well in English review because the reviewer remembers the deleted sentence. One
was found here (*above it* / `darüber` / 上面), in fourteen files at once.

Technical, at the point of removal: prettier clean, 448 tests, `svelte-check` 0
errors, `i18n:stamp` re-fingerprinted `home` in all fourteen, `i18n:status`
reports no stale namespace.

---

## Ninth pass — `home.compare.rows[1].vela` and `.base`

Date: 2026-09-16 · 15 locales × 2 cells, both the same string.

`Passkey or security key` → `Passkey`, in the Vela and the Base column alike
(they have always held the same value — the row is a tie and stays one).

A table cell is a label, not a sentence, so each locale gives the bare term it
already uses everywhere else in its own file, capitalised as that locale
capitalises a label: `Passkey` for the ten that keep the English word, `パスキー`,
`패스키`, `通行密钥` / `密碼金鑰` / `通行密鑰` for the three Chinese locales that each
have their own, and `Geçiş anahtarı` for Turkish — capitalised here because it
stands alone, where the running text has it lowercase mid-sentence.

Nothing to get wrong in the translation. What is worth recording is what the
shorter cell no longer says: **the comparison table no longer mentions hardware
security keys at all.** There is no other row that does. It is defensible — a
credential on a USB/NFC key is a passkey, device-bound rather than synced — and
`hero.facts[1]` still links to "Up to seven keys, set at creation — hardware keys
included", with the FAQ naming YubiKey. But if the table is meant to show that
Vela signs with hardware keys, this row was the only place it showed.

---

## Tenth pass — three standfirsts are deleted

Date: 2026-09-16 · founder's call: the paragraph under "How Vela compares" does
not need to render. It was `Three kinds of wallet: a plain EOA, a vendor smart
account, and Vela. We use all three, and none of them is better at everything.`

Deleting a key is not the same job as rewriting one, so, for the record, what it
touched:

1. **`en.ts`** — the key is gone and a comment stands where it was, saying why,
   so the next person does not "restore" it as an oversight.
2. **All fourteen translation files** — mandatory, not tidiness.
   `messages.test.ts` fails a locale that holds a key English does not have
   (a *stale key*: the English it translated is gone, so it can never render).
   Leaving them would have broken the build for fourteen locales.
3. **The markup** — `<p class="section-desc">` removed from the compare section.
   `.section-desc` itself stays: the FAQ still uses it.
4. **The spacing** — `.compare h2` had `margin-bottom: 12px` and relied on the
   paragraph's own `48px` to stand off the table. The heading now carries that
   gap itself, or the table would have come up to meet it.

Verified with a production build, not just tests: `npm run build` prerenders all
fifteen locales clean.

**`home.compare.note` went the same way, minutes later** — five paragraphs under
the table saying what each of the three wallets is good at and what Vela insists
on. The founder's reason is the right one and worth keeping: a reader who has
just read twelve rows does not need to be told what they say, and prose that
summarises a table it is sitting next to reads as filler however carefully it is
written. It cost 15 × 5 paragraphs to maintain and said nothing the rows did not.

Same four steps, plus its CSS: `.compare-note` and its three descendant rules
went with the markup, and `.section-desc` stayed because the FAQ still uses it.
The section is now a heading and twelve rows.

**`home.faq.desc` went too** — "Roughly in the order people ask them — starting
with what it is, ending with what happens if we're gone." Same objection, and
the same answer: a reader looking at eleven questions can see the order for
himself; being told about it is being managed.

That one took the CSS with it. `.section-desc` had exactly two users, the
comparison table and the FAQ; with both gone the class was dead, so the base
rule, its later `margin-bottom` override and the FAQ centring rule were removed
rather than left as a class no element carries. Both headings — `.compare h2`
and `.faq h2` — now hold the 48px their paragraphs used to leave above the table
and the questions.

Three strings, one instinct: **the home page stopped narrating itself.** Each
was a paragraph telling the reader what the thing next to it was about. The
table, the questions and the rows say it themselves, and none of the three now
needs maintaining in fifteen languages.

Nothing was translated in this pass. It is recorded because a deleted string is
the one change that looks like nothing happened — and because the argument
against these three applies to the next one somebody proposes.

---

## Eleventh pass — `home.networks.heading` and `home.networks.body`

Date: 2026-09-16 · 15 locales × 2 strings, one inline link in the body.

The heading traded a claim for an invitation — `One address on all of them` →
`Add your own` — and the claim reappears as the body's first sentence, so nothing
was lost in the move. The body also stopped explaining RIP-7212 (`the RIP-7212
P-256 precompile` → `RIP-7212`) and stopped mentioning local testnets.

Locale notes:

- **Headings are short and unpunctuated at the end**, which is now this page's
  rule. `ko` `12개 네트워크 기본 탑재. 직접 추가도 가능` ends on a noun phrase, the
  natural Korean heading shape; `ja` 「自分でも追加できます」 keeps `ます` because the
  heading is a statement in Japanese, where Korean prefers the nominal.
- **`de` `Füge deine eigenen hinzu`, `es-MX` `Agrega la tuya`, `ru` `Добавьте
  свою`** — each language picks its own number and gender for "your own"; the
  English is ambiguous between one and many and every locale had to choose.
  Singular reads better in all of them: you add a network, not networks.
- **The three spec names are verbatim everywhere** — `RIP-7212`, `Safe`,
  `ERC-4337`. `zh`/`zh-TW`/`zh-HK` use the ideographic comma `、` for that list,
  which is the correct enumeration mark and not the `,` a careless pass leaves.
- **The link text is sentence-case now**, and that changed one locale's grammar:
  `ru`'s tool name was sentence-initial (`Инструмент настройки сети`) and is now
  mid-sentence in the instrumental — `развернуть инструментом настройки сети`.
  A locale that only lower-cased the first letter would have left it in the
  nominative, reading as a title dropped into the middle of a clause.
- `fr` `avec l'<a>outil…` and `it` `con lo <a>strumento…` keep the elision and
  the article form *outside* the anchor, where they belong: the link text is the
  tool's name, not the preposition in front of it.

Technical: one paragraph, exactly one `biubiu.tools` anchor per locale, href
byte-identical; `networks.link` untouched. 189 tests, prettier clean, production
build prerenders all fifteen, no STALE.

---

## Pass — `home.faq.items`, all seven, in fifteen locales

Date: 2026-09-16 · same method · reference frame **English + `zh`**, both written
by the founder that day and treated as authoritative: every prior translation of
the FAQ is superseded, not reconciled.

The English list went from eleven questions to seven and the survivors were
rewritten shorter, so this is not a wording review of existing strings — thirteen
locales were written fresh against the new pair. What follows is the call made in
each, on the axes the method names.

### Accuracy — two claims that must not drift back

Both of the answers this pass replaces were **factually wrong**, not merely long
(see [approved-copy.md](../approved-copy.md)). A translator working from the old
files would have carried both forward, so they are stated here as prohibitions:

- No locale may mention **WalletPair**. The dApp answer is: the wallet is injected
  into the page, and that works in the iOS, Android and desktop apps and in the
  Vela browser extension.
- No locale may offer a **recovery extension**. The shutdown answer names the
  dependency-free Chrome extension, nothing else.

`fr`/`it`/`es-MX`/`pt-BR` also had to resist "extension de récupération"-shaped
phrasings that read naturally and would have reintroduced the deleted product.

### Naturalness — the verb for *injects*

English *injects the wallet directly into dApps* is developer register, and a
literal calque is clumsy or faintly medical in most of these languages. Each
locale took the verb its own wallet ecosystem would use:

| locale | verb | why not the literal one |
|---|---|---|
| `de` | `bindet die Wallet direkt in die dApp ein` | *injiziert* is chemistry; *einbinden* is what software does |
| `fr` | `place le portefeuille directement dans la dApp` | *injecte* exists in dev French but reads as jargon in a FAQ |
| `it` | `mette il wallet direttamente dentro la dApp` | same |
| `es-MX` | `pone la wallet directamente dentro de la dApp` | *inyecta* is understood but technical |
| `pt-BR` | `coloca a carteira direto dentro da dApp` | same |
| `tr` | `cüzdanı doğrudan dApp'in içine yerleştirir` | *enjekte* is a loan verb nobody writes here |
| `ja` | `ウォレットを dApp に直接組み込みます` | 注入 is literally injection of fluid |
| `ko` | `지갑을 dApp 안에 바로 넣어 줍니다` | 주입 reads chemical |
| `vi` | `đưa ví thẳng vào dApp` | plainer than *tiêm* |
| `id` | `menaruh dompet langsung di dalam dApp` | *menyuntikkan* is injection-with-a-needle |
| `ru` | `встраивает кошелёк прямо в dApp` | *внедряет* is the dev word, but *встраивает* is what a reader expects |
| `zh` / `zh-TW` / `zh-HK` | 注入 / 注入 / 注入 | kept — the founder's own `zh` uses it, and Chinese wallet copy does say 注入 |

### Consistency — each locale's own vocabulary, not each other's

Terminology was taken from each file, not from English:

- **passkey**: `ja` パスキー · `ko` 패스키 · `tr` geçiş anahtarı · `zh` 通行密钥 ·
  `zh-TW` 密碼金鑰 · `zh-HK` 通行密鑰 · everyone else keeps `passkey`. `zh` was the
  one open question of this pass — the founder's draft wrote `passkey` bare — and
  he closed it the same day in favour of the term the rest of the file uses.
- **security key**: `de` Sicherheitsschlüssel · `fr` clé de sécurité · `es-MX`
  llave de seguridad · `pt-BR` chave de segurança · `it` chiave di sicurezza ·
  `ru` ключ безопасности · `tr` güvenlik anahtarı · `id` kunci keamanan · `vi`
  khóa bảo mật · `zh` 安全钥匙 (the founder's word; 安全密钥 elsewhere in the file) ·
  `zh-TW`/`zh-HK` 安全金鑰.
- **device**: `pt-BR` aparelho, `ja` 端末, `ko` 기기, `zh-HK` 機 — each the word its
  own file already uses, so FAQ 1 and FAQ 3 read like the rest of the page.
- `zh-HK` is written in spoken Cantonese throughout, as the rest of that file is:
  加唔到, 攞返出嚟, 入唔返個錢包, 執笠. It is not a transcription of `zh`, and neither
  is `zh-TW`, which uses 建立/存取權/位址 where `zh` uses 创建/访问权/地址.

### Register — unchanged per locale

`de`/`it`/`es-MX` informal, `fr`/`ru`/`tr`/`id`/`pt-BR` formal, `ja`/`ko`
です・ます / 합니다, `vi` `bạn`, the three Chinese files plain 你. FAQ questions are
first-person in every locale ("what do **I** need"), answers second-person, which
is the shape the English pair set.

### UI fit

Seven `<summary>` rows on a phone: the longest question is `id`'s *Apa yang bisa
Vela lakukan terhadap uang saya, dan apa yang Vela tahu tentang saya?* at 88
characters, which wraps to three lines on a 360 px screen and is still the
shortest faithful form — Indonesian has no shorter possessive. No locale was
padded to match English length.

### Technical

No locale carries markup: the new English has **no links and no tags**, so
`href`/tag parity is trivially satisfied and every file was checked for leftovers
from the old answers. Paragraph counts match the English exactly — 2 2 2 **1** 2
2 2 — which is the check that catches a translator merging the two beats.

448 unit tests pass, `svelte-check` clean, prettier clean, `i18n:stamp` re-run so
`home` is fingerprinted in all fourteen (honest here: every one of them was
retranslated in this pass).


### Second cut — item 7 and the WalletPair removal

Same day, after the founder read it back. Two changes, both accuracy:

- **Item 7's second beat was rewritten in all fifteen.** The old line promised
  that existing passkeys keep working in the native apps; the association file
  those platforms check is fetched from `getvela.app`, so that promise cannot
  cover a reinstall after the domain is gone. It now names the two routes that
  survive the domain — the Vela extension and the dependency-free clear-signing
  extension — and says the key may be a USB security key or a phone reached by QR
  code rather than the platform passkey. Every locale took the clear-signing name
  from its own docs sidebar (`ru` понятная подпись, `vi` ký minh bạch, `id` clear
  signing, and so on), and "reached by QR code" was written as a connection in
  each language — `ru` подключённый по QR-коду, `tr` QR kodla bağlanan — never as
  "a phone you scan", which reverses who scans what.
- **WalletPair is gone from the site**: the footer link, and `roadmap.shipped[3]`,
  rewritten in fifteen as *dApp connect in the browser*. `zh-HK` keeps its own
  register there (`個錢包會好似其他瀏覽器錢包噉出現喺網頁入面`), and `ja`/`ko` keep
  the middle dot for the platform list (デスクトップ・iOS・Android).

448 tests, `svelte-check` clean, prettier clean on every touched file, dev-server
render checked in `en`, `zh` and `zh-TW`, and no `walletpair` string left in
`src/`.
