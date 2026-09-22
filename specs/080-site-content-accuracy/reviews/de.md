# de — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (native-German localizer agent) · Method: single-string locale review, whole pages

Scope: `messages/de.json` (every key listed in the translation brief, plus defects
found in untouched keys) and all 17 docs under `content/docs/de/`, with
`self-hosting.md` newly created. Every page was rewritten from the current English
and Chinese pair; the old German was mined only for phrasing whose meaning still
holds (most of `why-vela.md` and the Bybit narrative survived that way).

## Terminology and register

| Concept | German term | Note |
| --- | --- | --- |
| key (umbrella) | **Schlüssel** | passkey, phone or security key; "Signaturschlüssel" where the page is about signers |
| passkey | **Passkey** (der, Pl. Passkeys) | per 059; what German tech press and the OS settings say |
| security key | **Sicherheitsschlüssel** / **Hardware-Sicherheitsschlüssel** | per 059; matches the wallet UI ("USB-Sicherheitsschlüssel") |
| relay | **Relay** (das) | one term only; the old text mixed "Relay" and "Relayer" for the same thing. "Relayer" now appears only for the relay's own funded sender addresses (self-hosting), as in the English |
| registry | **Register** / **Registervertrag** | matches the wallet UI ("Das Register ist nicht erreichbar"); the public ERC-7730 descriptor registry is called **Verzeichnis** to keep the two apart |
| public-key index | **Public-Key-Index** | the Settings field is named by its UI label **Passkey-Index** where the page tells you which field to fill |
| self-hosting | **selbst hosten** / **Selbsthosten**; guide = **Anleitung zum Selbsthosten** | chrome draft kept |
| signing page | **Signaturseite** | |
| clear signing | **Klartext-Signatur** (Clear Signing on first mention) | kept from 059 (sidebar, FAQ); *im Klartext* = in plain words |
| blind signing | **Blindsignieren**; warning = **Blindsignatur-Warnung** | |
| relying party | **Relying Party** | as German WebAuthn documentation writes it |
| deploy | **bereitstellen** / **Bereitstellung** | the wallet UI's verb ("Contract bereitstellen") |
| owner / threshold | **Eigentümer** / **Schwellenwert** | the wallet UI's "Eigentümer" |
| approval | **Freigabe**; intent label *Genehmigen* | wallet UI ("ERC-20-Freigabe", intent "Genehmigen") |
| best effort (decoder label) | **ohne Gewähr** | the idiomatic German label for "no guarantee" information |
| treasury | **Treasury** | the wallet UI's own modal title |
| coin | **der Coin** (masc.) | follows the wallet UI ("den nativen Coin"); the old site had feminine "die Coin" |
| Safe | **das Safe** (neuter) | as the existing hero line ("ein unverändertes Safe") and the wallet UI ("über dein Safe") |
| phone | **Handy** | the site's and the wallet UI's word ("Handy oder Tablet") |

UI paths use the German wallet labels: **Einstellungen → Erweitert →
Dienst-Endpunkte**, **Einstellungen → Netzwerke**, fields **Chain-Daten-Index /
Passkey-Index / Vela Relay / Fiat-Kurse**, button **Auf Standard zurücksetzen**, key
methods **Dieses Gerät / Handy oder Tablet / USB-Sicherheitsschlüssel**, speeds
*Langsam / Standard / Schnell* (so the default is written "voreingestellt ist
*Schnell*", never "Standard: schnell", which would name the wrong speed). Chrome
and Windows strings are the German originals (**Entwicklermodus**, **Entpackte
Erweiterung laden**, „Der Computer wurde durch Windows geschützt“, **Weitere
Informationen**, **Trotzdem ausführen**).

Register: informal **du**, unchanged from 059. Typography: German quotes „…“,
spaced en dash " – " as the Gedankenstrich (applied across `de.json` too; the em
dash survives only as the empty-cell placeholder in tables), decimal comma and
point grouping (0,01 US-Dollar, 140.000–170.000 Gas, 1,1 Mio. Gas), German dates
(21. Februar 2025). Source titles in the Bybit reference list stay in English, as
in zh.

Departures from 059: none in the terms themselves. The catalog's "Seed Phrase"
(open compound) is now **Seed-Phrase** everywhere, which is German orthography for
an English compound and what the docs already wrote.

## Findings

Severity is judged against the finalized reference and the claim ledger. Most High
items are facts that were true of an older English and are false now; they are
listed because a German reader was being told them today.

| # | File / key | Before (old translation) | Type | Severity | Why | After |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | `home.networks.heading`; docs `introduction`, `faq`, `whitepaper`, `networks-and-fees` | „12 Netzwerke eingebaut“, „Vela bringt **12 EVM-Netzwerke** mit“ | mistranslation (stale fact) | High | C-net-1: 24 built-in networks; also failed `networks.test.ts` | „24 Netzwerke eingebaut. Füge eigene hinzu“; full list of 24 in the docs |
| 2 | `home.meta.description`, `home.meta.organization` | „keine Seed Phrase, kein Hardware-Schlüssel, keine Bindung an uns“ | mistranslation (stale fact) | High | Hardware security keys are supported (C-keys-2); the description denied it | „Signiert wird mit Passkeys oder Sicherheitsschlüsseln – ohne Seed-Phrase …“ |
| 3 | docs `faq` (costs), `networks-and-fees` („Das Gas-Konto aktivieren“), `whitepaper` (Gebühren) | „nicht erstattungsfähige Einzahlung, um sein Gas-Relay-Konto zu aktivieren … erneute Aktivierung“ | mistranslation (stale fact) | High | C-fee-2: no per-network deposit or gas account; described a payment flow that does not exist | Treasury-empty behaviour: optional, non-refundable top-up that does not pay for your transaction; „Es gibt kein Gas-Konto pro Wallet und keine Aktivierungseinzahlung“ |
| 4 | `home.tradeoffs.items[0].body`; `whitepaper` („Kein versteckter Aufschlag“) | „Die Gebühr enthält die On-Chain-Kosten und ein Entgelt für den Relay-Betrieb“ | mistranslation (stale fact) | High | C-fee-1: fee = 3 × reserved gas at the higher price, often ≥10× the real cost; the old framing understated what the reader pays | Mechanism, the „oft das Zehnfache … oder mehr“, the 0,01-$ minimum, and „kann sich danach nicht mehr ändern“ |
| 5 | docs `networks-and-fees` | „Es gibt keine Geschwindigkeitsauswahl“ | mistranslation (stale fact) | High | C-fee-3: speed can be chosen, default fast | *Langsam / Standard / Schnell*, voreingestellt *Schnell* |
| 6 | docs `faq` (Open Source), `whitepaper` (Open Source) | „die Wallet und ihre vier Backend-Dienste … sind unter der MIT-Lizenz“ | mistranslation (stale fact) | High | C-lic-1: the public-key index has no licence file | „Der Public-Key-Index ist öffentlich, hat aber noch keine Lizenzdatei“; compare row „MIT, außer dem Index, dessen Lizenz noch aussteht“ |
| 7 | `roadmap.upcoming[4]` | „… und ein unabhängiges Sicherheitsaudit von Velas Safe- und WebAuthn-Integration“ as an upcoming item | cultural/claim risk | High | Invariant: never write that an audit is planned or coming (A02 FR-2) | Array replaced by the 5 new items; no audit promise anywhere in `de` |
| 8 | `roadmap.upcoming[4]`, `roadmap.upcoming[3]` | „samt eines Signaturwegs für Chains ohne P-256-Precompile“; „Auf iOS und Android folgen dir Konten und Netzwerke bereits über das Backup“ | mistranslation (stale fact) | High | C-p256-1 (no such path is possible); C-sync-1 (account lists are not synced) | Replaced with the new `upcoming` (5) and `shipped` (10) arrays |
| 9 | `home.tradeoffs.items[2]` | Title „Du vertraust auditierten Safe-Verträgen — und ein Audit ist keine Garantie“; body without Vela's own audit status | mistranslation (omission) | High | C-audit-1: trade-off #3 must also say Vela's own code has had no third-party audit and none is scheduled | „Die Verträge sind auditiert, Velas eigener Code nicht“ + „Velas eigene Apps und Dienste hatten kein Audit durch Dritte, und derzeit ist auch keines angesetzt“ |
| 10 | `home.tradeoffs.items[1].body` | „Beim Anlegen der Wallet kannst du mehrere Schlüssel hinterlegen“ | mistranslation (omission) | High | C-keys-1: up to seven, chosen at creation, cannot be changed later | „Beim Erstellen der Wallet wählst du bis zu sieben Schlüssel; danach lassen sie sich nicht mehr ändern.“ |
| 11 | `home.compare.rows` (Vela: extra check, self-hosting) | „Über eine unabhängige, selbst gehostete Seite oder Erweiterung“; „App, Relay und Backend-Dienste lassen sich alle selbst betreiben“ | mistranslation (stale fact) | High | C-signpage-1 (built, not connected, not published); self-hosting has documented gaps and passkeys stay tied to getvela.app (C-rp-1) | „Eine unabhängige Signaturseite, fertig gebaut, aber noch nicht an die Apps angebunden“; „… mit Lücken, die die Anleitung zum Selbsthosten auflistet; Passkeys bleiben an getvela.app gebunden“ |
| 12 | `home.compare.rows` (structure) | 12 rows; no "Adding a key later"; „Umfang des Open Source“ | technical | High | English has 13 rows; `de.home` failed "is not a fragment", so the whole home page fell back to English | 13 rows incl. „Später einen Schlüssel hinzufügen“ and „Quellcode“ |
| 13 | `home.faq.items[0].a` | „Ein Gerät mit Face ID oder Fingerabdruck, oder einen USB-/NFC-Sicherheitsschlüssel“ | mistranslation (stale fact) | High | A wallet cannot be created on one unsynced security key; the app asks for two (C-keys-2, create-wallet) | „… oder Hardware-Sicherheitsschlüssel – zwei, wenn du nur Sicherheitsschlüssel verwendest“ |
| 14 | `home.faq.items[4].a` | „Wer an deine synchronisierten Passkeys kommt, kommt womöglich auch an deine Wallet … nimm stattdessen einen USB-/NFC-Sicherheitsschlüssel“ | mistranslation (omission) | High | Missing the action that protects the money: a key can't be removed, so move funds to a new wallet | „… und ein Schlüssel lässt sich nicht entfernen. Verschieb dein Guthaben in eine neue Wallet …“ |
| 15 | `home.faq.items[6].a` | offline signing via „der abhängigkeitsfreien Klartext-Signatur-Erweiterung“ | mistranslation (stale fact) | High | The signing page is not a way in on its own — no app sends it requests (self-hosting, C-signpage-1); the relay also needs a code change | Extension + self-built apps only; relay code-change caveat |
| 16 | `getStarted.fundingNote` | „du kannst es also auch selbst bauen — dieselbe App, ohne Kosten“ | mistranslation (stale fact) | High | A self-built phone app cannot use the phone's own passkey for getvela.app wallets (C-rp-1) | „Eine selbst gebaute Handy-App signiert allerdings mit einem anderen Handy oder einem Sicherheitsschlüssel …“ |
| 17 | docs `install` | „es gibt nichts herunterzuladen und keinen App Store“ | mistranslation (stale fact) | High | C-plat-1: extension and desktop are downloads; phone apps come to the stores | Platform table with cost and status for all five |
| 18 | docs `account-contract` | „Es gibt keinen Vela-Vertrag.“; gas „etwa dem 1,5- bis 3-Fachen … einer einfachen EOA-Überweisung“ | mistranslation (stale fact) | High | Vela's registry contracts exist (unaudited, not in the funds path); measured 140.000–170.000 Gas vs 21.000 | Own-contracts paragraph; measured gas figures |
| 19 | docs `bybit-attack` | „Kein Vertrag, den wir upgraden können“, „das Upgrade-Primitiv zu entfernen, auf das der Angriff baute“; no self-call warning | mistranslation (stale fact) | High | The owner-signed `delegatecall` still exists in every Safe; a dApp can request `enableModule`/`addOwnerWithThreshold`/… — the reader must reject calls to their own address | „Keine Admin-Rolle, die man uns abnehmen könnte“ + what that does *not* remove + the self-call limit |
| 20 | docs `clear-signing` | Callout „Unbegrenzte Genehmigungen werden blockiert“ (no permit caveat) | mistranslation (stale fact) | High | C-approve-1: signed permits and large finite approvals are not capped | New callout title: „Unbegrenzte“ On-Chain-Freigaben lassen sich nicht absenden — plus what it does not stop |
| 21 | docs `create-wallet` | single-passkey flow; „Vela leitet deine Adresse aus dem öffentlichen Schlüssel deines Passkeys ab“ | mistranslation (stale fact) | High | C-addr-1 (address from all keys), C-keys-1, C-reg-1 (full public record) | Five-step flow with 1–7 keys; `what-is-public` list |
| 22 | docs `recovery` | sign-in only „mit demselben iCloud- oder Google-Konto“; the synced passkey as the whole model | mistranslation (stale fact) | High | Any one key signs in; lookup index → Gnosis → Ethereum; C-sync-1 | Rewritten from the reference |
| 23 | `home.why.p1` | Base Account link to `account.base.app`; „wenn der Dienst verschwindet, verschwindet die Wallet mit ihm“ | technical + mistranslation | Medium | href differs from English (failed the markup gate); the claim is stronger than the reference („no published way for your passkey to reach your account“) | New href; „… gibt es keinen veröffentlichten Weg, auf dem dein Passkey noch an dein Konto kommt“ |
| 24 | `home.tradeoffs.items[0].body` | relay link to the GitHub repo; „Den Relayer kannst du in den Einstellungen wechseln“ | technical + terminology | Medium | href now `/docs/self-hosting#relay` (failed the markup gate); "Relayer" vs "Relay" | „Du kannst die Wallet auf ein anderes Relay umstellen oder <a>ein eigenes betreiben</a>“ |
| 25 | `home.networks.body` | link to biubiu.tools „Chain-Setup-Tool … deployen“ | technical | Medium | href now `/chain-setup` (failed the markup gate) | „… zeigt die <a>Chain-Einrichtung</a>, welche, und stellt die bereit, die jeder bereitstellen kann“ |
| 26 | `home.hero.facts[2].link`, `[3].term`, `[3].link` | „welchen Weg wir schließen“; „Auch wenn Vela offline geht, kommst du weiter an deine Wallet.“; „Hoste die App, das Relay und jeden Backend-Dienst selbst“ | mistranslation | Medium | Overclaims: the attack class is mitigated, not closed; the fact now names what still depends on Vela | „… und was Vela dagegen tut“; „Alles, was Vela für deine Wallet betreibt, ist Open Source …“; „… und was noch von uns abhängt“ |
| 27 | `home.seal.label` | „Wallets on-chain erstellt“ | mistranslation | Medium | Registration ≠ creation on-chain (the contract deploys on first send) | „on-chain registrierte Wallets“ |
| 28 | `home.compare.rows` (other cells) | MetaMask „EOA“; Vela „Passkey“; MetaMask gas sponsorship „Nicht angeboten“; batched „Hängt von den Fähigkeiten des Kontos ab“; MetaMask source „Der Wallet-Client“ | mistranslation (stale) | Medium | Each contradicts the current row (EIP-7702 default, security keys + seven, "on some networks", "via its EIP-7702 account", non-commercial licence) | Current rows |
| 29 | `home.compare.rows` (gas) | „ERC-4337-Aufschlag“ | terminology | Medium | *Aufschlag* means a surcharge, i.e. a fee; the row is about gas overhead, with the fee listed separately | „ERC-4337-Mehraufwand“ |
| 30 | `home.pricing.cards` | „Desktop- und Mobile-Apps — Aus dem Quellcode kostenlos“; „Aus den App-Stores — Einmaliger Kauf“ | mistranslation (stale) | Medium | Desktop is a free download; store apps are not available yet | Three current cards; „Einmalkauf · demnächst“ |
| 31 | `home.faq.items[1].a`, `[5].a` | dApps „in der iOS-, der Android- und der Desktop-App“ (no Linux caveat, no web-wallet caveat); privacy answer listing only key, address and name | mistranslation (omission) | Medium | C-dapp-1; the public record also holds authenticator kind and key labels, and services see IP and transactions | Current answers |
| 32 | `about.lede`, `about.values[1]` | „Keine gesichtslose Firma dahinter“ (no company named); passkey = „dein Gesicht oder deinen Fingerabdruck“ | mistranslation | Medium | Reference names MONDAY LABS LTD (UK); C-auth-1 (PIN, security key) | Current lede; „… nach Gesicht, Fingerabdruck oder PIN …“ |
| 33 | `getStarted.meta.description`, `lede`, `platforms.web.blurb`, `platforms.desktop.stores` | desktop/extension „entstehen aus demselben Code“; web blurb without the dApp caveat; „Mac App Store · Microsoft Store“ | mistranslation (stale) | Medium | Desktop and extension are downloadable now; the web wallet does not connect to dApps; no Mac App Store listing | Current strings |
| 34 | `getStarted.downloads.yourSystem`, `downloads.extension.steps[0]` | „Ihr System“; „den Sie behalten“ | cultural risk (register) | Medium | Formal *Sie* on a *du* site; „Ihr System“ also reads as "their system" | „Dein System“; „den du behältst“ |
| 35 | `chrome.docs.ui.browse` (draft) | „Dokumentation durchsuchen“ | UI fit | Medium | *durchsuchen* = to search; the control opens the docs sidebar on phones | „Dokumentation einblenden“ (pairs with „… ausblenden“) |
| 36 | docs `passkeys` | „benutzt wird er nur mit deinem Gesicht oder deinem Fingerabdruck“ | mistranslation | Medium | C-auth-1: device PIN, or touch and PIN on a security key | Four confirmation routes named |
| 37 | docs `why-vela` | „den allerersten Schlüssel zu einem Hardware-Sicherheitsschlüssel machen“; callout link to `security-audits` | mistranslation + technical | Medium | Following it fails at creation (a single unsynced key is refused); the link text "Safe Smart Account" pointed at the audits page | „… ausschließlich Hardware-Sicherheitsschlüssel verwenden – zwei davon …“; link to `account-contract` |
| 38 | docs `bybit-attack` | „Jede Signatur, die du je in einer Web-Wallet erzeugt hast, ruhte auf dieser Annahme“ | mistranslation | Medium | Overgeneralisation; reference says "most signatures" | „Die meisten Signaturen, die in einer Web-Wallet entstehen …“ |
| 39 | `home.compare.heading` | „Wie sich Vela schlägt“ | cultural risk (tone) | Low | A contest idiom ("how Vela holds its own"); the section is a neutral table | „Vela im Vergleich“ |
| 40 | docs `why-vela` | „du bist ein gestohlenes Handy vom Ärger entfernt“; „das zukünftige Du“; „Was dir das nicht kauft“ | unnatural | Low | English calques ("X away from", "future-you", "buy you") | „schon ein gestohlenes Handy kann Ärger bedeuten“; „dein zukünftiges Ich“; „Was dir das nicht bringt“ |
| 41 | `chrome.footer.tagline`, `about.values[1].title`, catalog vs docs | „Seed Phrase“ / „Seed Phrases“ beside „Seed-Phrase“ | terminology | Low | One spelling per locale; German hyphenates English compounds | „Seed-Phrase“, „Seed-Phrasen“ |
| 42 | `getStarted.downloads.extension.action` | „Erweiterung laden“ | UI fit | Low | Collides with Chrome's „Entpackte Erweiterung laden“ two lines below | „Erweiterung herunterladen“ |
| 43 | all of `de.json` | em dash „ — “ as Gedankenstrich | technical (typography) | Low | German uses the spaced en dash; the file already mixed both | „ – “ throughout (em dash kept only as the empty-cell placeholder) |
| 44 | docs, catalog | "der/die Coin", "Relay/Relayer", "Kasse/Treasury" | terminology | Low | Mixed within the locale | Masculine *Coin*, *Relay*, *Treasury* (wallet UI terms) |

Counts: **High 22 · Medium 16 · Low 6** (items 1–22, 23–38, 39–44), all fixed.

## Open items

- The wallet apps' German corpus addresses the user formally (*Sie*) while the site
  uses *du* (059 register). Not something the site can fix; recorded so nobody
  "harmonises" the site towards the app.
- The apps have no German labels for the Split and Sweep send modes; the docs say
  **Aufteilen** and **Zusammenführen** descriptively. If the apps add labels, align
  `send-and-receive.md` and `roadmap.shipped[6]` to them.
- Grammatical gender of *Safe*: neuter is kept to match the existing hero fact and
  the wallet UI. Masculine (*der Safe*) is equally correct German; if the founder
  prefers it, it is a mechanical change across the catalog and docs.

## Result

reviewed — no open High or Medium findings

## Update 2026-09-22

Carried the en + zh revision of the same day (fee wording, configurable relay chain
directory, hero subtitle, facts #3 and #4) into `de.json` and four docs. Terms
unchanged from the table above: *Relay*, *Chain-Verzeichnis*, *Chain-Daten*,
*bereitstellen*, *Passkey* (der). A running relay deployment is *vela-relay-Instanz*
(*Bereitstellung* stays reserved for contract deployment).

| String / section | Change | Severity of anything fixed beyond the brief | Note |
| --- | --- | --- | --- |
| `home.hero.subtitle` | „Der private Schlüssel deines Passkeys gelangt nie zu Vela.“ | – | first sentence unchanged, already "signing is done on your device" |
| `home.hero.facts[2]` | „Was du siehst, signierst du: Vela dekodiert vor deiner Freigabe genau die Transaktion, die signiert wird.“ + new link | – | German has no settled native term for WYSIWYS (the literature quotes the English); the inverted form mirrors the fee callout „Was du siehst, zahlst du“ |
| `home.hero.facts[3]` | 059 string restored; new link „Wie du deine Wallet weiter nutzt, wenn Vela verschwindet“ | – | **059 string kept**: „Auch wenn Vela offline geht, kommst du weiter an deine Wallet.“ matches en ("doesn't depend on Vela staying online") and zh (即使 Vela 停止服务…) and reads naturally; link echoes the whitepaper heading „Wenn Vela verschwindet“ |
| `home.tradeoffs.items[0].body` | middle paragraph rewritten (one fee, to the relay, Vela's by default, formula + `#fee` link, relay keeps the rest) | – | both hrefs verbatim; closing paragraph left as it was (en only swapped a dash for ", and") |
| `home.faq.items[6].a` | relay code-change clause replaced by the list of services you can run | – | |
| `roadmap.upcoming[1].body` | relay clause dropped | – | |
| docs `networks-and-fees` | `<span id="fee">`; "ten times" paragraph replaced; „**Wer sie bekommt.**“ added | – | bold on the old claim dropped, as in en |
| docs `faq` | fee bullet (relay choice, `#fee` link); shutdown answer without the parenthesis | – | |
| docs `whitepaper` | intro clause, fee bullet + new bullet on who gets the fee, „Wenn Vela verschwindet“ clause | – | |
| docs `self-hosting` | intro limit removed; two code comments; „Gut zu wissen“ bullet; chain-data paragraph; relay line removed from „Was danach noch auf Vela zeigt“ | – | Docker comment's verb moved into the lead („in .env setzen: …“) so the added line stays grammatical |

No High or Medium findings open.
