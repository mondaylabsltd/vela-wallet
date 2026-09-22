# fr — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), fr localizer · Method: single-string locale review, whole pages

Scope: every key the brief lists in `fr.json`, plus the defects found in keys it did
not list; all 17 docs rewritten from the current English and Chinese
(`self-hosting.md` created). The old fr docs were translated from an English that
predates spec 080 (12 networks, a per-network gas account with an activation
deposit, an address derived from one passkey synced by iCloud or Google, "nothing to
download"), so most findings below are accuracy: a French reader was being told
things the product no longer does, or never did. Where the old French was still
right and well written (much of `why-vela`, parts of `bybit-attack` and
`clear-signing`), its phrasing was kept.

## Terminology and register

| Concept | fr | Note |
| --- | --- | --- |
| key (any signing credential) | clé | umbrella term |
| passkey | passkey (feminine: *une passkey*) | 059 choice, kept. The wallet's own fr corpus sometimes says *clé d'accès*; the site does not (see Open items) |
| security key / hardware security key | clé de sécurité / clé de sécurité matérielle | as the wallet onboarding says (*Clé de sécurité USB*) |
| signer (a Safe owner) | signataire; owners = propriétaires | |
| relay | relais (masculine) | **unified** — the old catalog mixed *relais* and *relayeur*; *relayeurs* now only for the relay's own sender addresses in the self-hosting guide; the app's field label *Vela Relay* is quoted as is |
| registry | registre / contrat de registre | |
| public-key index | index des clés publiques | the Settings field is quoted by its fr UI label, **Index des clés d'accès** |
| self-hosting | auto-hébergement / héberger soi-même; run a service = faire tourner / exploiter | matches the wallet's *Guide d'auto-hébergement →* link |
| signing page | page de signature | |
| clear signing / blind signing | signature lisible / signature à l'aveugle | **changed** *signature aveugle* → *signature à l'aveugle*, the wallet UI's term (*Signature à l'aveugle*) |
| seed phrase / recovery phrase | phrase de récupération | one term for both, as the file already did; Base Account's browser-generated secret in `why-vela` stays *clé de récupération* (the English says "recovery key" there) |
| self-custodial | en auto-conservation (noun: auto-conservation) | **changed** from the adjective *auto-conservé*, which reads as "self-preserved"; the noun was already the file's term (*L'auto-conservation, pour de vrai*) |
| smart account | compte intelligent | **unified** — the catalog said *smart account*, the docs *compte intelligent* |
| token / native coin | jeton / monnaie native | **unified** — the old docs used *tokens*, the catalog *jetons* |
| relying party | partie de confiance (relying party) | English term in parentheses at first mention on each page |
| treasury / payload / digest | trésorerie / charge utile / condensat | |
| Settings paths | Réglages → Réseaux; Réglages → Avancé → Points d'accès des services | the fr wallet corpus labels (`settings.sections.advanced`, `settings.advanced.endpointsTitle`) |

Register: *vous*, as recorded in 059, with *vous*-form imperatives. Neutral French
technical-documentation voice, no hype. Vela is masculine when the product is the
subject (*il ne peut pas…*); the one place the English says "Vela, the company"
reads *L'entreprise Vela … elle*. `home.faq.items[5].q` was aligned (*que sait-il*).

Typography: a space before `:` `;` `?` `!` and inside « », as the existing fr files
write it (plain spaces, consistent with every other fr string); straight
apostrophes as in the rest of the file. Non-breaking spaces only inside digit groups
and before `$` (*2 000 000*, *0,01 $*), so a number never wraps. French number
format (*1,8 ×*, *1,1 million*, *0,01 $*, *1,5 milliard de dollars*). UI labels
quoted as the fr builds show them: the wallet corpus (*Créer un portefeuille*,
*Recevoir* / *Envoyer*, *Réglages*, *Points d'accès des services*, *Index des
données de chaîne*, *Index des clés d'accès*, *Vela Relay*, *Taux fiat*,
*Réinitialiser par défaut*, *Rapide*, *Bientôt*), Chrome (*Mode développeur*,
*Charger l'extension non empaquetée*) and Windows SmartScreen (*Windows a protégé
votre ordinateur*, *Informations complémentaires*, *Exécuter quand même*). The
desktop platform is *Bureau* everywhere, matching the Get Vela card. Contract names,
EIP/ERC numbers, commands, file paths and addresses stay as in English; comments in
code blocks and the prose of the whitepaper's text diagram are translated, as `zh`
does.

## Findings

| # | File / key | Before (old translation) | Type | Severity | Why | After |
|---|---|---|---|---|---|---|
| F-1 | `home.meta.description` | « …Signature par passkey — pas de phrase de récupération, **pas de clé matérielle**, aucun enfermement. » | mistranslation | High | contradicts C-keys-2: security keys are a first-class key kind | « …Signez avec des passkeys ou des clés de sécurité — sans phrase de récupération. Votre compte est un Safe non modifié… » |
| F-2 | `home.meta.organization` | same « pas de clé matérielle » | mistranslation | High | same as F-1, in the structured data search engines quote | « …en auto-conservation, bâti sur un Safe non modifié, qui signe avec des passkeys ou des clés de sécurité… » |
| F-3 | `home.meta.ogDescription` | « …auto-hébergeable… Compilez-le vous-même si vous le souhaitez. » | mistranslation | Medium | old claim set; drops the unmodified Safe and security keys | « Portefeuille Ethereum open source bâti sur un Safe non modifié. Passkeys ou clés de sécurité… » |
| F-4 | `home.hero.facts[2].link` | « …et la voie que nous fermons » | cultural risk | Medium | overclaims; the reference says "what Vela does about it", and the page says *mitigated, not eliminated* | « …et ce que fait Vela contre ce risque » |
| F-5 | `home.hero.facts[3].term` | « Même si Vela cesse de fonctionner, vous gardez l'accès à votre portefeuille. » | mistranslation | High | unconditional promise the reference walked back; C-rp-1 (passkeys stay bound to getvela.app) | « Tout ce que Vela fait tourner pour votre portefeuille est open source, et vous pouvez le faire tourner vous-même. » |
| F-6 | `home.hero.facts[3].link` | « Hébergez vous-même l'app, le relais et chaque service backend » | mistranslation | Medium | promises complete self-hosting; the guide now lists what still depends on Vela | « Ce qu'il faut faire tourner, ce qui marche encore sans getvela.app, et ce qui dépend encore de nous » |
| F-7 | `home.seal.label` / `.verify` | « portefeuilles créés on-chain » / « Chaque portefeuille est on-chain » | technical | Medium | the counter counts registry records; a wallet contract deploys only on its first send | « portefeuilles enregistrés on-chain » / « …est enregistré on-chain — voir le registre » |
| F-8 | `home.why.p1` | href `account.base.app`; « une clé de récupération générée dans un navigateur… si le service disparaît, le portefeuille disparaît avec lui » | technical + mistranslation | High | href no longer matched English (test failed); overstated the competitor's failure mode | new href; « …une phrase de récupération générée sur un site web… un service de signature dont le code n'est pas public — s'il disparaît, aucune méthode publiée ne permet à votre passkey d'atteindre votre compte » |
| F-9 | `home.tradeoffs.items[0]` | « Par défaut, le relayeur paie le gas… Les frais comprennent le coût on-chain et une commission… Vous pouvez changer de relayeur dans les réglages » (link to GitHub) | mistranslation | High | contradicts C-fee-1 (3× the reserved gas, often ≥10× the real cost, $0.01 minimum) and C-relay-1; href changed to `/docs/self-hosting#relay` | rewritten to the formula; « faire pointer le portefeuille vers un autre relais ou faire tourner le vôtre » |
| F-10 | `home.tradeoffs.items[1].body` | « Vous pouvez définir plusieurs clés à la création de votre portefeuille » (no full stop) | mistranslation | High | C-keys-1: must say *up to seven* and *can't be changed afterwards* | « Vous choisissez jusqu'à sept clés à la création du portefeuille, et elles ne peuvent plus être modifiées ensuite. » |
| F-11 | `home.tradeoffs.items[2]` | « Vous faites confiance à des contrats Safe audités — et un audit n'est pas une garantie » | mistranslation | High | C-audit-1: trade-off #3 must say Vela's own code is unaudited and none is scheduled — it didn't | « Les contrats sont audités. Le code de Vela, lui, ne l'est pas. » + « …n'ont fait l'objet d'aucun audit tiers, et aucun n'est programmé. » |
| F-12 | `home.compare.rows` | 12 rows | technical | High | English has 13 (new "Adding a key later"); "Open source" became "Source code" | 13 rows; « Ajouter une clé plus tard » inserted after « Si vous perdez une clé » |
| F-13 | `compare.rows[1]` (Signing key) | vela « Passkey », base « Passkey » | mistranslation | High | omits security keys and the seven-key limit (C-keys-1/2); Base also has a recovery phrase | « Passkeys ou clés de sécurité, jusqu'à sept » / « Passkey ou phrase de récupération » |
| F-14 | `compare.rows[3]` (Sponsored gas) | « Gas pris en charge » / metamask « Non proposé » | UI fit + mistranslation | Medium | *pris en charge* also means "supported" in every other row of the same table; MetaMask now sponsors on some networks | « Gas sponsorisé » / « Sur certains réseaux » / « Quand l'app le sponsorise » |
| F-15 | `compare.rows[9]` (Extra check) | « Via une page ou une extension indépendante que vous hébergez » | mistranslation | High | presents the signing page as usable today; C-signpage-1 (built, not published, not connected) | « Une page de signature indépendante, prête mais pas encore reliée aux apps » |
| F-16 | `compare.rows[10]` (Full self-hosting) | « App, relais et services backend, tous exécutables par vous » | mistranslation | High | omits the passkey domain limit (C-rp-1) and the listed gaps | « …avec les lacunes listées dans le guide d'auto-hébergement ; les passkeys restent liées à getvela.app » |
| F-17 | `compare.rows[11]` (Source code) | « Périmètre open source » ; metamask « Le client du portefeuille » | mistranslation | Medium | C-lic-1 licence gap missing; MetaMask's licence is non-commercial, which "open source client" hid | « Code source » ; « …MIT, sauf l'index, dont la licence est en attente » / « Public, sous une licence qui n'autorise qu'un usage non commercial » |
| F-18 | `home.pricing.cards` | « Applications bureau et mobile — Gratuit depuis les sources » ; « Depuis les stores — Achat unique » | mistranslation | High | C-plat-1: desktop is a free download, phone apps are not in the stores yet | « Web, extension de navigateur et bureau — Gratuit » ; « iPhone et Android, compilés vous-même — Gratuit » ; « …depuis les stores — Achat unique · bientôt disponible » |
| F-19 | `home.networks.heading` | « 12 réseaux intégrés » | mistranslation | High | C-net-1: 24 (networks test failed) | « 24 réseaux intégrés. Ajoutez le vôtre » |
| F-20 | `home.networks.body` | link to biubiu.tools; « vous pouvez les déployer » | technical + mistranslation | High | href no longer matched English; not every missing contract can be deployed by anyone | link to `/chain-setup`; « …indique lesquels et déploie ceux que tout le monde peut déployer » |
| F-21 | `home.faq.items[0].a` | « Un appareil déverrouillé par Face ID ou par empreinte, ou une clé de sécurité USB/NFC… Ajoutez toutes les clés que vous voulez » | mistranslation | High | no seven-key limit (C-keys-1), no "two if only security keys", face/fingerprint as the only way (C-auth-1) | « Un téléphone ou un ordinateur compatible avec les passkeys, ou des clés de sécurité matérielles — deux, si… Choisissez toutes vos clés, jusqu'à sept… » |
| F-22 | `home.faq.items[1].a` | « Cela fonctionne dans les apps iOS, Android et bureau… » | mistranslation | Medium | C-dapp-1: desktop means macOS and Windows; the web wallet doesn't connect to dApps | « …apps bureau (macOS, Windows), iPhone et Android. Le portefeuille web, lui, ne se connecte pas aux dApps. » |
| F-23 | `home.faq.items[4].a` | « Qui accède à vos passkeys synchronisées peut sans doute accéder à votre portefeuille. » | mistranslation | High | omits that a key can't be removed and that the only remedy is moving the funds (C-keys-1) | « …et une clé ne peut pas être retirée. Transférez vos fonds vers un nouveau portefeuille… » |
| F-24 | `home.faq.items[5].a` | « …Seules vos clés commandent le portefeuille… Votre clé publique, l'adresse du portefeuille et le nom… sont publics » | mistranslation | High | understates what is public (C-reg-1: authenticator type, key labels) and what services see (IP); drops that Vela writes the signing software | full list and the software disclosure |
| F-25 | `home.faq.items[6].a` | « …signez depuis un navigateur : l'extension Vela, ou l'extension de signature lisible sans dépendances » | mistranslation | High | presents the unpublished signing page as a fallback (C-signpage-1); omits the relay's code-change caveat | extension + self-built apps only; relay caveat added |
| F-26 | `about.lede` / `about.team.bio` / `about.meta.description` | « le portefeuille, les contrats intelligents et ce site » ; « Aucune entreprise sans visage derrière » | mistranslation | Medium | Vela wrote no contract in the funds path (C-acct-1); the company is now named | « Vela est conçu par MONDAY LABS LTD… les apps, les services backend et ce site… » |
| F-27 | `about.values[0].body` / `[1].body` | « Vos clés, vos pièces… » ; « une passkey : votre visage ou votre empreinte » | mistranslation | Medium | drops "what we control is the software you sign with"; face/fingerprint as the only check (C-auth-1) | reference meaning restored; « …après vérification de votre visage, de votre empreinte ou de votre code PIN » |
| F-28 | `roadmap.upcoming` | « …une voie de signature pour les chaînes sans précompilé P-256… et un audit de sécurité indépendant… » ; « vos comptes et réseaux vous suivent déjà via la sauvegarde de votre plateforme » | mistranslation | High | contradicts C-p256-1 and C-sync-1, and lists an audit as coming (A02 FR-2) | replaced with the 5 new items |
| F-29 | `roadmap.shipped` | 7 items | technical | High | English has 10 new items | replaced with the 10 new items |
| F-30 | `getStarted.meta.description` / `.lede` | « …des versions bureau, mobile et une extension… » ; « emportez… votre téléphone » | mistranslation | Medium | implies phone apps are available (C-plat-1) | « …des apps mobiles en préparation » ; « …et bientôt sur votre téléphone » |
| F-31 | `getStarted.platforms.web.blurb` | « Rien à installer et rien à mettre à jour… authentifiez-vous avec votre passkey » | mistranslation | Medium | any one of your keys; the web wallet can't connect to dApps | « Confirmez avec l'une de vos clés… Pour vous connecter aux dApps, utilisez l'extension. » |
| F-32 | `getStarted.platforms.desktop.stores` | « Mac App Store · Microsoft Store » | mistranslation | Medium | the Mac App Store is not a channel | « Microsoft Store » |
| F-33 | `getStarted.fundingNote` | « …même application, sans frais » | mistranslation | High | a self-built phone app can't use the phone's own passkey (C-rp-1) | « …une app mobile compilée vous-même signe avec un autre téléphone ou une clé de sécurité, et non avec la passkey du téléphone lui-même » |
| F-34 | `docs/fr/introduction.md` | « 12 réseaux… » ; « par le visage ou l'empreinte » ; « ça tourne dans votre navigateur, rien à télécharger » | mistranslation | High | C-net-1, C-auth-1, C-plat-1 | rewritten; 24 networks, up to seven keys, answer table |
| F-35 | `docs/fr/install.md` | « il n'y a rien à télécharger et aucun store » ; « Android 9+ » ; no `dapps` anchor | mistranslation + technical | High | C-plat-1; wrong OS minimum; anchor test failed | rewritten with all four platforms, device table, `<span id="dapps">` |
| F-36 | `docs/fr/create-wallet.md` | « Vela dérive votre adresse de la clé publique de votre passkey » ; public data = key + name; no `what-is-public` anchor | mistranslation + technical | High | C-addr-1 (all founding keys), C-reg-1 (full public record); anchor test failed | rewritten; full list under `<span id="what-is-public">` |
| F-37 | `docs/fr/why-vela.md` | « faire de la toute première clé une clé de sécurité matérielle » ; « auto-hébergeable pour que votre portefeuille ne dépende jamais du fait que notre entreprise reste en ligne » | mistranslation | High | a single unsynced key is refused (two needed); the domain limit (C-rp-1) was missing | « …n'utiliser que des clés de sécurité matérielles — deux… » ; « …avec une limite, le domaine auquel appartiennent vos passkeys… » |
| F-38 | `docs/fr/networks-and-fees.md`, `faq.md`, `whitepaper.md` | « activation du compte de gas », « dépôt non remboursable pour activer son compte relayeur de gas », « réactivation » | mistranslation | High | C-fee-2: there is no per-network deposit or gas account | rewritten: relay treasury, optional non-refundable top-up that does not pay your transaction |
| F-39 | `docs/fr/networks-and-fees.md`, `faq.md`, `security-audits.md`, `whitepaper.md` | « 12 réseaux EVM », « l'un de nos douze réseaux » | mistranslation | High | C-net-1 | 24, with the full table |
| F-40 | `docs/fr/recovery.md`, `passkeys.md` | « Connectez-vous au même compte iCloud ou Google… » ; « utilisée uniquement avec votre visage ou votre empreinte » | mistranslation | High | recovery works with any one key of any kind (C-keys-2, C-sync-1); C-auth-1 | rewritten: any one key, index → Gnosis → Ethereum lookup, single-key rebuild |
| F-41 | `docs/fr/signers.md` | no "if a key may be compromised" section; « Ajouter un huitième signataire » | mistranslation | High | C-keys-1: keys can't be removed — the move-your-funds instruction was missing | section added; « Ajouter un signataire plus tard… » |
| F-42 | `docs/fr/clear-signing.md`, `send-and-receive.md` | « Les approbations illimitées sont bloquées… il réécrit la demande en un montant fini » ; « Vela ne suppose jamais 18 » | mistranslation | High | C-approve-1: only on-chain "unlimited" approvals are held, signed permits and large finite approvals are not; the app does fall back to 18 and marks it unverified | reference wording, with the permit caveat |
| F-43 | `docs/fr/account-contract.md`, `whitepaper.md` | « n'importe quelle interface compatible Safe peut le piloter » ; MIT for all backend services | mistranslation | High | C-safeui-1 (Safe tools read; signing needs a getvela.app signature); C-lic-1 | rewritten |
| F-44 | `docs/fr/bybit-attack.md` | « Aucun contrat que nous puissions mettre à niveau… retirer la primitive de mise à niveau sur laquelle l'attaque s'est appuyée » | mistranslation | High | the owner-signed `delegatecall` primitive still exists in every Safe, Vela's included; the self-call gap was missing | « Aucun rôle d'administrateur que l'on pourrait nous voler » + what that does *not* remove + the self-call warning |
| F-45 | `docs/fr/clear-signing-self-host.md` | no status paragraph; « aucune requête réseau de son propre chef » | mistranslation | High | C-signpage-1 (not published, no app sends requests); it does fetch token logos | status paragraph added; « sa seule requête concerne des logos de jetons décoratifs » |
| F-46 | `docs/fr/self-hosting.md` | (missing) | technical | High | new page; six stable anchors required | created, all anchors present |
| F-47 | `home.why.heading` | « Pourquoi nous l'avons fait » (nav: « Pourquoi nous l'avons créé ») | terminology | Low | the nav link and the section it jumps to should say the same | « Pourquoi nous l'avons créé » |
| F-48 | `home.faq.items[5].q` | « …et que sait-elle de moi ? » | terminology | Low | Vela is masculine everywhere else in the fr site | « …et que sait-il de moi ? » |
| F-49 | docs, all | *tokens* / *jetons*, *smart account* / *compte intelligent*, *signature aveugle*, *auto-conservé* | terminology | Low | one term per concept (see table above) | unified |

Chrome drafts (`chrome.docs.groups.keys`, `.selfHost`, `titles["self-hosting"]`,
`footer.links.selfHosting`, `chrome.docs.ui.*`) were reviewed and kept: they read
naturally, match the wallet's *Guide d'auto-hébergement* and fit the sidebar.

## Open items

- The wallet's own fr corpus (vela-core `i18n/locales/fr`, not in scope here) mixes
  *passkey* and *clé d'accès* — e.g. the Settings field is labelled **Index des clés
  d'accès**. The site keeps *passkey* (059) and quotes that field by its UI label
  once, in the self-hosting guide, so a reader can find it. Aligning the app is a
  product decision.
- None blocking.

## Result

reviewed — no open High or Medium findings

## Update 2026-09-22

Carried the en + zh revision of the same day (fee wording, configurable relay chain
directory, hero subtitle, facts #3 and #4) into fr. Each changed string checked on
the five single-string axes; no High or Medium left open.

| String / section | Change | Severity of anything fixed beyond the brief | Note |
| --- | --- | --- | --- |
| `home.hero.subtitle` | « La signature se fait sur votre appareil. La clé privée de votre passkey n'est jamais transmise à Vela. » | — | *clé privée*, as `passkeys.md` already says |
| `home.hero.facts[2]` | « Ce que vous voyez est ce que vous signez : Vela décode la transaction exacte avant que vous ne l'approuviez. » / « …et comment Vela vous montre ce que vous signez » | — | the usual French rendering of WYSIWYS; mirrors the existing callout « Ce que vous voyez est ce que vous payez » |
| `home.hero.facts[3]` | term = 059 string, kept; link « Comment continuer à utiliser votre portefeuille si Vela disparaît » | — | 059 « Même si Vela cesse de fonctionner, vous gardez l'accès à votre portefeuille. » **kept**: same meaning as en ("doesn't depend on Vela staying online") and zh (即使 Vela 停止服务…), idiomatic French. The link reuses the whitepaper heading « Si Vela disparaît » |
| `home.tradeoffs.items[0].body` | paragraph 2 rewritten (« Vous ne payez qu'un seul montant : les frais du relais… » — *frais* is plural, so "one fee" is carried as one amount; formula + `#fee` link; relay keeps the rest); paragraph 3 tail aligned | — | both hrefs identical to en; non-breaking space before `$` kept |
| `home.faq.items[6].a` | code-change clause replaced by the four services you can run | — | second paragraph untouched |
| `roadmap.upcoming[1].body` | chain-data clause removed | — | |
| docs `networks-and-fees` | `<span id="fee">`; "ten times" paragraph replaced; **Qui les reçoit.** paragraph | — | |
| docs `faq` | cost bullet (relay choice, `#fee` link); shutdown answer without the code-change parenthesis | — | |
| docs `whitepaper` | intro clause dropped; Fees bullet + new "who gets the fee" bullet; « Si Vela disparaît » clause dropped | — | |
| docs `self-hosting` | intro limit dropped; `VELA_RELAY_CHAIN_DIRECTORY_URL` comment lines in both code blocks; « À savoir » bullet; chain-data paragraph; relay line removed from the final list | — | « Cette variable existe depuis septembre 2026 » rather than *réglage*, which the locale reserves for the app's Réglages |

Nothing fixed beyond the brief.

## Update 2026-09-22 (audience)

Carried the audience pass (commit 66a3c789: copy for the reader who self-hosts and
builds from source; p256-index now MIT) into fr. Each changed string checked on the
five single-string axes; `messages.test.ts -t fr` passes.

| String / section | Change | Severity fixed beyond the brief | Note |
| --- | --- | --- | --- |
| `home.meta.description` / `ogDescription` / `organization` | build-and-host meaning; « auto-hébergeable » replaces « en auto-conservation » in `organization` | — | "key index" rendered as the locale's *index des clés publiques* |
| `home.hero.ctaSelfHost` (new, after `ctaCode`) | « Ou hébergez vous-même toute la stack » | — | *la stack* is what French developers say; *pile logicielle* reads as a textbook term on a hero link |
| `home.why.more` | « …ce que nous ne pouvions pas accepter dans d'autres portefeuilles, et le compromis que nous avons choisi » | — | *compromis* = the locale's "trade-off" |
| `home.compare.rows` | reordered to the new en order (mapped from the old indices); Source code `vela` cell « …</a>, tous sous licence MIT » | — | `<a>` unchanged |
| `home.faq.items` | new 7-item order; three new answers | — | reused old wording (« trousseau iCloud », « une clé ne peut pas être retirée », « Transférez vos fonds… »); "put its address in the wallet" → « saisissez son URL dans le portefeuille », matching the guide's « Saisissez `https://your-relay` »; `1-of-n` kept as in `signers.md` |
| `getStarted.lede` | + « Chaque app se compile aussi depuis les sources. » | — | |
| docs `introduction` | opening « portefeuille open source et auto-hébergeable »; "Fonctionne sans nous" first and rewritten; table reordered + relay/fee row | — | |
| docs `faq`, `whitepaper`, `self-hosting` | all MIT incl. the index; « (Rust, MIT) »; no-licence sentence deleted | — | no "licence pending" line left in fr |

Nothing fixed beyond the brief.
