# ru — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), ru localizer · Method: single-string locale review, whole pages

Scope: every key the brief lists in `ru.json`, plus the defects found in keys it
did not list; all 17 docs rewritten from the current English and Chinese
(`self-hosting.md` created). The old ru docs (drafted 2026-09-16, never reviewed —
see 059 `reviews/ru.md`) were translated from an English that predates spec 080,
so most of what follows is accuracy: a Russian reader was being told about twelve
networks, a gas-account deposit, an audit on the roadmap and a signing page that
does not ship.

## Terminology and register

| Concept | ru | Note |
| --- | --- | --- |
| key (any signing credential) | ключ | umbrella term, as the wallet UI (`Ключи`, `Ключ {{n}}`) uses it |
| signer (a key that is a Safe owner) | подписывающий ключ | the 059 term (`подписывающих ключей`); owners = *владельцы*; a signer **contract** = *контракт-подписант* |
| passkey | passkey (indeclinable, no plural form) | 059 choice. Never the bare object of *подписывать*: *подписать с помощью passkey*, *подпись passkey* (genitive), *passkey на этом устройстве* — the RU-1 trap in 059 `single-string.md` |
| security key / hardware security key | ключ безопасности / аппаратный ключ безопасности | the term the Russian OS dialogs and the wallet corpus use (`Ключ безопасности`, `USB-ключ безопасности`, `аппаратном ключе безопасности`). The old files mixed it with bare *аппаратный ключ*; now *аппаратный* appears only where English says "hardware". The sidebar title for `signers` changed from «Подписывающие и аппаратные ключи» to «Подписывающие ключи и ключи безопасности» to match |
| relay | релей | kept (30 uses in the old files; matches `vela-relay` and the MEV usage). *релеер*, which the old trade-off mixed in, is gone; the relay's own sender EOAs in the self-hosting guide are *адреса-отправители (relayer)* |
| bundler / paymaster / treasury | бандлер / paymaster / казна | *казна* as in the wallet's treasury sheet |
| registry | реестр / контракт-реестр | the on-chain contract; UI already says *Реестр недоступен* |
| public-key index | индекс публичных ключей | the service; the Settings field is quoted by its UI label «Индекс passkey» |
| chain data / exchange rates | данные сетей / курсы валют | Settings fields quoted as the UI shows them: «Индекс данных сетей», «Курсы валют» |
| self-hosting | самостоятельное развёртывание / развернуть (запустить) у себя | footer «Руководство по самостоятельному развёртыванию»; sidebar and page title shortened to «Самостоятельное развёртывание» (the full form is 46 characters in a sidebar row); group «Запустить у себя» kept |
| signing page | страница подписи | 059/old docs term, kept; sidebar «Своя страница подписи» kept |
| clear signing / blind signing | понятная подпись / слепая подпись | 059 choice, kept; matches the wallet corpus (*Слепая подпись*) |
| seed phrase / recovery phrase | сид-фраза / фраза восстановления | **unified**: the catalog said *seed-фраза* in the tagline, meta, FAQ and a values title but *сид-фраза* in the comparison table, and the docs said *сид-фраза* throughout. One spelling now, the Cyrillic one. *Фраза восстановления* only for Base Account's phrase |
| relying party | relying party (проверяющая сторона) | English term kept, glossed at first mention on each page |
| iCloud Keychain / Google Password Manager | Связка ключей iCloud / Менеджер паролей Google | Apple's and Google's own Russian names, capitalised (the catalog FAQ had them lower-case) |
| precompile / deploy / counterfactual | прекомпайл / развернуть / контрфактический | old-file terms, kept |

Register: formal *вы*, lower-case, as recorded in 059; imperatives in the
вы-form. **Vela is feminine** (*Vela сделала*, *она не может*), as the reviewed
catalog already had it (*Почему мы её сделали*, *Vela выпустила*); the unreviewed
docs had drifted to masculine (*Vela сделан*, *у него*). Russian typography:
«…» quotes, spaced em dash, $0,01 / $1,5 млрд, 140 000. UI labels are quoted as
the Russian builds show them: the wallet corpus («Создать кошелёк», «Это
устройство», «Телефон или планшет», «USB-ключ безопасности», «Настройки →
Дополнительно → Сервисные эндпоинты», «Сбросить к стандартным», «Медленно» /
«Стандартно» / «Быстро», «Получить» / «Отправить», «Проверено»), Chrome («Режим
разработчика», «Загрузить распакованное расширение») and Windows SmartScreen
(«Система Windows защитила ваш компьютер», «Подробнее», «Выполнить в любом
случае»). Contract names, EIP/ERC numbers, commands, file paths and addresses stay
as in English; comments inside code blocks and the whitepaper's architecture
diagram are translated, as `zh` does.

## Findings

Findings in the old text, all fixed. "C-…" ids refer to `claim-ledger.md`.

| # | File / key | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `home.meta.description`, `.organization` | «…Подпись через passkey — без seed-фразы, без аппаратного ключа, без привязки к нам» | mistranslation (stale) | High | Says there is no hardware key; security keys are a first-class key type (C-keys-2) | «Подпись с помощью passkey или ключей безопасности — без сид-фразы. Ваш аккаунт — немодифицированный Safe…» |
| 2 | `home.hero.facts[3]` | «Даже если Vela перестанет работать, доступ к кошельку останется у вас» / «Разверните у себя приложение, релей и каждый бэкенд-сервис» | mistranslation (stale) | High | An unconditional promise the domain limit makes false (C-rp-1) | «Всё, что Vela держит для вашего кошелька, открыто — и это можно запустить самому.» / «Что запускать, что работает без getvela.app и что пока зависит от нас» |
| 3 | `home.hero.facts[2].link` | «…и какой путь мы закрываем» | mistranslation | Medium | Claims the path is closed; the reference says what Vela *does about it* — mitigated, not eliminated | «Как Bybit потеряла $1,5 млрд и что с этим делает Vela» |
| 4 | `home.why.p1` | `href="https://account.base.app"`; «…и если сервис исчезнет, кошелёк исчезнет вместе с ним» | technical + mistranslation | High | href differed from the English value (FR-031 test failed); the reason is now a signing service whose code isn't public | new href; «…и сервис подписи с закрытым кодом — если он исчезнет, не останется никакого опубликованного способа добраться до аккаунта с помощью вашего passkey» |
| 5 | `home.tradeoffs.items[0]` | «По умолчанию газ платит релеер… Комиссия… фиксируется в момент подписи. Релеер можно сменить в настройках или поднять свой» | mistranslation (stale) | High | No trace of the 3× reserve priced at the higher gas reading, or of "often ten times or more" (C-fee-1); link pointed at the repo instead of `/docs/self-hosting#relay`; *релей* and *релеер* mixed | Retranslated: «…берёт плату за трёхкратный резерв по более высокой из двух цен… Часто это в десять и более раз больше реальной стоимости в сети…» |
| 6 | `home.tradeoffs.items[1].body` | «При создании кошелька можно задать несколько ключей» (no full stop) | mistranslation | Medium | Lost "up to seven" and "can't be changed afterwards" (C-keys-1) | «При создании кошелька вы выбираете до семи ключей, и потом их уже не изменить.» |
| 7 | `home.tradeoffs.items[2]` | «Вы доверяете проверенным аудитом контрактам Safe — а аудит не гарантия» | mistranslation (stale) | High | Omitted that Vela's own code is unaudited and none is scheduled — the sentence C-audit-1 requires on this card | «Контракты прошли аудит. Собственный код Vela — нет.» … «…стороннего аудита не проходили, и он не запланирован» |
| 8 | `home.compare.rows` | 12 rows; Vela signing key «Passkey»; «Через независимую страницу или расширение, которые вы держите сами»; «Приложение, релей и бэкенд-сервисы — всё можно поднять самому»; «Что открыто» | mistranslation + technical | High | Missing the 13th row "Adding a key later" (array length); security keys absent (C-keys-2); the unshipped signing page presented as usable (C-signpage-1); self-hosting overclaimed; licence caveat missing (C-lic-1) | 13 rows retranslated, incl. «Добавить ключ позже — Нельзя — ключи фиксируются при создании» and «Исходный код — …MIT, кроме индекса — для него лицензия ещё не оформлена» |
| 9 | `home.pricing.cards` | «Приложения для компьютера и телефона — Из исходников — бесплатно»; «Из магазинов приложений — Разовая покупка» | mistranslation (stale) | High | Desktop is a free download; the store apps do not exist yet (C-plat-1) | «Веб, расширение для браузера и десктоп — Бесплатно» … «iPhone и Android — из магазинов приложений — Разовая покупка · скоро» |
| 10 | `home.networks.heading`, `.body` | «12 встроенных сетей»; link to `biubiu.tools/…chain-setup` | mistranslation + technical | High | 24 networks (C-net-1, networks test failed); href differed from English `/chain-setup` | «24 встроенные сети. Добавьте свою»; `<a href="/chain-setup">настройки сети</a>` |
| 11 | `home.faq.items[0].a` | «Устройство с Face ID или отпечатком либо ключ безопасности USB/NFC» | mistranslation | Medium | Dropped "two, if you use only security keys" and "up to seven" | «…либо аппаратные ключи безопасности — два, если вы используете только ключи безопасности… Выберите все ключи, до семи…» |
| 12 | `home.faq.items[1].a` | «Vela встраивает кошелёк прямо в dApp… в приложениях для iOS, Android и десктопа» | mistranslation (stale) | Medium | Implied Linux desktop and omitted that the web wallet does not connect (C-dapp-1) | «…во встроенном браузере приложений для десктопа (macOS, Windows), iPhone и Android. Веб-кошелёк к dApp не подключается.» |
| 13 | `home.faq.items[4].a` | «Тот, кто доберётся до ваших синхронизированных passkey, скорее всего доберётся и до кошелька» | mistranslation | Medium | Missing "a key can't be removed — move your funds to a new wallet" | «…а удалить ключ нельзя. Переведите средства в новый кошелёк…» |
| 14 | `home.faq.items[5].a` | «Кошельком управляют только ваши ключи… Ваш публичный ключ, адрес кошелька и его название открыто записаны» | mistranslation | Medium | Dropped that Vela writes the software that asks keys to sign; public record incomplete (C-reg-1); what services see omitted | «Но программу, которая просит ваши ключи подписать, пишет именно Vela…» + full list |
| 15 | `home.faq.items[6].a` | «…не зависит от серверов Vela»; «подписывайте из браузера: расширение Vela или расширение понятной подписи без зависимостей» | mistranslation (stale) | High | Presented the unpublished signing page as a working fallback (C-signpage-1); omitted the relay code change | «…только в релее придётся изменить код…»; fallback = extension + self-built apps |
| 16 | `about.lede`, `about.team.bio` | «кошелёк, смарт-контракты и сам этот сайт… нет безликой компании» | mistranslation (stale) | Medium | Vela writes no contract in the funds path (C-acct-1); the company is now named | «Vela делает MONDAY LABS LTD — небольшая компания из Великобритании…» |
| 17 | `about.values[1].body` | «Мы заменили её на passkey: ваше лицо или отпечаток» | mistranslation | Medium | A passkey is a key, not a biometric; PIN and security keys missing (C-auth-1) | «…ключи хранятся на ваших устройствах или на ключе безопасности и используются после проверки лица, отпечатка или PIN-кода» |
| 18 | `roadmap.upcoming` | «…и независимый аудит безопасности интеграции Safe + WebAuthn у Vela»; «включая путь подписи для сетей без прекомпайла P-256»; «аккаунты и сети уже переезжают с вами через резервную копию платформы» | mistranslation (stale) | High | Announced an audit (forbidden, A02 FR-2); promised a path C-p256-1 rules out; claimed sync C-sync-1 denies | new five items |
| 19 | `roadmap.shipped` | 7 items | technical | High | English has 10, so `ru.roadmap` was a fragment and the page fell back to English | new ten items |
| 20 | `getStarted.meta.description`, `.lede` | «…версии для десктопа, телефона и расширение собираются из того же кода»; «…на компьютер, в телефон или на панель браузера» | mistranslation (stale) | Medium | Phone apps are not available yet (C-plat-1) | «…приложения для телефона на подходе»; «…а скоро и на телефон» |
| 21 | `getStarted.platforms.web.blurb` | «…пройдите аутентификацию своим passkey» | mistranslation | Medium | Any key signs in; the dApp caveat (use the extension) was missing | «Подтвердите одним из своих ключей… Чтобы подключаться к dApp, используйте расширение.» |
| 22 | `getStarted.platforms.desktop.stores` | «Mac App Store · Microsoft Store» | mistranslation | Medium | English lists only the Microsoft Store | «Microsoft Store» |
| 23 | `getStarted.fundingNote` | «…собрать сами — то же приложение, без оплаты» | mistranslation | Medium | A self-built phone app cannot use the phone's own passkey (C-rp-1) | «…только самостоятельно собранное приложение для телефона подписывает с помощью другого телефона или ключа безопасности, а не passkey самого телефона» |
| 24 | `docs/introduction.md` | «12 сетей, один адрес»; «вы подписываете с помощью passkey, лицом или отпечатком пальца» | mistranslation (stale) | High | C-net-1; C-auth-1 / C-keys-2 | rewritten |
| 25 | `docs/install.md` | «скачивать нечего и проходить через магазин приложений не нужно»; «Нативные мобильные приложения скоро выйдут» | mistranslation (stale) | High | Extension and desktop downloads exist; phone apps are a paid store product (C-plat-1) | rewritten with the four-way table |
| 26 | `docs/install.md` | no `<span id="dapps">`, no dApp section | technical | High | Anchor linked from intro/FAQ missing (anchor test failed) | section and anchor added |
| 27 | `docs/create-wallet.md` | «Vela выводит ваш адрес из публичного ключа passkey» | mistranslation (stale) | High | The address comes from all founding keys (C-addr-1) | «Приложение вычисляет адрес кошелька по всему набору ключей» |
| 28 | `docs/create-wallet.md` | «Публичный ключ и выбранное имя публикуются в индексе passkey»; no `what-is-public` anchor | mistranslation + technical | High | Public record incomplete (C-reg-1); linked anchor missing | «Что становится публичным» with the full list + anchor |
| 29 | `docs/why-vela.md` | «Кошелёк можно создать с **семью** подписывающими ключами»; «самым первым ключом можно сделать аппаратный»; «чтобы ваш кошелёк никогда не зависел от того, остаётся ли наша компания онлайн» | mistranslation | Medium | "up to" dropped; a single unsynced key can't be created (two security keys); the domain limit (C-rp-1) | rewritten |
| 30 | `docs/send-and-receive.md` | «Vela переписывает его на конечную сумму и отказывается отправлять…»; «запрашивает ваш passkey (Face ID / Touch ID / отпечаток)» | mistranslation | Medium | Signed permits are cautioned, not capped (C-approve-1); C-auth-1 | rewritten |
| 31 | `docs/networks-and-fees.md` | «12 сетей EVM»; «Активация газового аккаунта… невозвратный депозит»; «Выбора скорости нет» | mistranslation (stale) | High | C-net-1, C-fee-2, C-fee-3; the fee formula (C-fee-1) absent | rewritten |
| 32 | `docs/passkeys.md` | «используется он только после проверки лицом или отпечатком»; «та же технология, что защищает Apple Pay» | mistranslation | Medium | C-auth-1; other phones and security keys missing (C-keys-2) | rewritten with the where-the-key-lives table |
| 33 | `docs/signers.md` | «зарегистрируйте YubiKey при создании и подписывайте им»; no compromised-key section | mistranslation | Medium | The app asks for a second key when the only key is unsynced; missing "a key can't be removed — move your funds" | rewritten |
| 34 | `docs/recovery.md` | «Войдите в тот же аккаунт iCloud или Google…»; «Вход с разных устройств опирается на синхронизацию passkey» | mistranslation (stale) | High | Sign-in works with any one key via the index/registry (C-sync-1, C-keys-2) | rewritten |
| 35 | `docs/clear-signing.md` | callout «Неограниченные разрешения блокируются»; «Vela никогда не предполагает 18» | mistranslation (stale) | High | Large finite approvals and signed permits are not blocked (C-approve-1); unconfirmed decimals are shown as 18 and flagged; "verified" is not cryptographic (C-clear-1) | rewritten |
| 36 | `docs/bybit-attack.md` | «…убрать примитив обновления, на который опиралась атака» | mistranslation (stale) | High | The owner-signed `delegatecall` primitive still exists in every Safe, Vela's included — the old text claimed the opposite | rewritten, incl. the self-call warning |
| 37 | `docs/bybit-attack.md` | «Статус: сделана и протестирована, ещё не развёрнута. Когда выйдет — по желанию» | mistranslation | Medium | Not published, and no app sends it requests (C-signpage-1) | rewritten |
| 38 | `docs/account-contract.md` | «управлять им может любой Safe-совместимый интерфейс»; «примерно на 1,5–3 раза больше газа» | mistranslation (stale) | High | Reading ≠ signing (C-safeui-1); measured 140–170k gas vs 21k | rewritten |
| 39 | `docs/security-audits.md` | `0xdd93…` listed as the current «Индекс публичных ключей passkey» | mistranslation (stale) | High | The current registry is `0x94fD…`; `0xdd93…` is read-only history | rewritten |
| 40 | `docs/clear-signing-self-host.md` | no status; «Когда ею пользоваться — …для каждой подписи»; «ни одного собственного сетевого запроса» | mistranslation (stale) | High | C-signpage-1; token logos are fetched | rewritten |
| 41 | `docs/whitepaper.md` | «Vela поддерживает 12 сетей»; «газовый аккаунт… активируется невозвратным депозитом»; «Всё открыто под лицензией MIT — приложение и все четыре бэкенд-сервиса» | mistranslation (stale) | High | C-net-1, C-fee-2, C-lic-1 | rewritten |
| 42 | `docs/faq.md` | «12 сетей EVM»; «невозвратный депозит для активации её газового аккаунта»; «кошелёк и четыре его бэкенд-сервиса… под лицензией MIT» | mistranslation (stale) | High | C-net-1, C-fee-2, C-lic-1 | rewritten |
| 43 | `docs/self-hosting.md` | (absent) | technical | High | The page every "without getvela.app" link points at did not exist in ru | created |
| 44 | `chrome.footer.tagline`, `about.values[1].title` | «Без seed-фразы», «Без seed-фраз» | terminology | Low | Two spellings in one locale (see table) | «Без сид-фразы», «Без сид-фраз» |
| 45 | `chrome.docs.titles.signers` | «Подписывающие и аппаратные ключи» | terminology | Low | Aligns the sidebar with the security-key term | «Подписывающие ключи и ключи безопасности» |
| 46 | `chrome.docs.ui.browse` / `.hide` (draft) | «Открыть документацию» / «Скрыть документацию» | UI fit | Low | The phone toggle opens the sidebar of a docs page the reader is already on | «Показать разделы» / «Скрыть разделы» |
| 47 | `home.seal.label` | «кошельков создано в блокчейне» | mistranslation | Low | "registered", not "created" | «кошельков зарегистрировано в блокчейне» |
| 48 | `home.faq.items[2].a` | «со связкой ключей iCloud или менеджером паролей Google» | terminology | Low | Product names are capitalised everywhere else | «со Связкой ключей iCloud или Менеджером паролей Google» |
| 49 | `docs/why-vela.md` | callout link to `/ru/docs/security-audits` | technical | Low | English links the account-contract page there | `/ru/docs/account-contract` |

High fixed: 26 · Medium fixed: 17.

## Open items

- **Split / Sweep labels.** The wallet's Russian corpus has no label for these
  two send modes that I could find, so `send-and-receive.md` names them
  descriptively («Разделить», «Собрать»). If the apps ship Russian names for
  them, the doc should quote those.
- **"Self-hosting" in the app.** The wallet's Settings link reads «Руководство по
  self-hosting →»; the site uses «самостоятельное развёртывание». The app string
  is outside this scope; one of the two should follow the other.
- **`chainSetup`** is absent from `ru.json` (the chain-setup page falls back to
  English, which is a legal state). Not in this brief.
- This pass was made by an AI localizer, not a human native speaker; `review.json`
  was not touched (not in scope).

## Result
reviewed — no open High or Medium findings
