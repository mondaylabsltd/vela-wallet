# it — review against the finalized en + zh (spec 080)

Date: 2026-09-22 · Reviewer: Claude (Opus 5), native-Italian localizer pass · Method: single-string locale review, whole pages

Scope: `messages/it.json` (every key listed in the translation brief, plus the
real defects below in keys the brief did not list) and all 17 docs in
`content/docs/it/` (`self-hosting.md` created). Every page was rewritten from the
current English and Chinese, not patched: the old Italian docs were translated
from an English that predates spec 080, and almost every page carried facts the
claim ledger has since corrected.

## Terminology and register

| Concept | Italian, everywhere | Note |
|---|---|---|
| key (umbrella) | **chiave** / chiavi | a passkey, a phone, a security key |
| passkey | **passkey** (f., invariable: la passkey, le passkey) | as 059 (`la passkey`, `signing.heading`) |
| security key | **chiave di sicurezza**; *chiave di sicurezza hardware* where the English says hardware | replaces the old meta's «chiavetta hardware» |
| relay | **relay** (m.) | «relayer» only for the relay's own sending addresses (self-hosting); the old trade-off used «relayer» for the service |
| registry | **registro**, *contratto di registro* | the on-chain contract |
| public-key index | **indice delle chiavi pubbliche** (short: l'indice) | the Settings field is named with the app's own label, «Indice passkey»; the old docs used «indice passkey» for both the service and the on-chain registry |
| self-hosting | **self-hosting** (noun, as in the footer «Guida al self-hosting»); verbs *gestire in proprio*, *ospitare in proprio* | sidebar group «Gestiscilo tu» |
| signing page | **pagina di firma** | |
| clear signing / blind signing | **firma leggibile** / **firma alla cieca** | «alla cieca» is the Italian app's own wording; the old docs said «firma al buio» while the roadmap said «alla cieca» |
| seed phrase / recovery phrase | **seed phrase** / **frase di recupero** | seed phrase as the site has used since 059 (footer, hero); frase di recupero only for Base Account's recovery phrase, which the English distinguishes |
| smart account | **smart account** (m.) | the old docs said «account intelligente», the catalog «smart account» |
| self-custodial | **in autocustodia**, *autocustodia* | replaces the calque «auto-custodito» and the mixed «Self-custody, davvero» |
| deploy | **deploy** (n.), **deployare** (v.) | replaces «distribuire», which in Italian first reads as *distribute* |

UI labels are the Italian app's and the OS vendors' own: Impostazioni → Reti;
Impostazioni → Avanzate → Endpoint dei servizi; «Ripristina i valori
predefiniti»; the fields «Indice dati delle reti», «Indice passkey», «Vela Relay»,
«Tassi fiat»; speeds lenta / standard / rapida; «Crea wallet», «Ricevi», «Invia»;
Chrome «Modalità sviluppatore», «Carica estensione non pacchettizzata»; Windows
SmartScreen «Ulteriori informazioni», «Esegui comunque»; Portachiavi iCloud,
Gestore delle password di Google.

Register: informal **tu**, as in 059 and the rest of the file. Quotes «…», as
the existing catalog. Straight apostrophes, as the file already uses. Vela takes
masculine agreement where Italian forces one (*il wallet Vela*); where Vela is
the company, sentences are built to avoid agreement («per conto proprio»), so the
brand never switches gender between pages. Numbers in Italian format: 0,01
dollari, 1,5 miliardi, 300.000 gas.

Departures from 059: none in register or in the passkey term. One heading follows
the standing 059 rule over the new English: `tradeoffs.items[2].title` carries
full stops in English («…audited. Vela’s own code is not.») and none in Italian,
because trade-off titles take no terminal punctuation (059, eighth pass addendum)
and zh has none either.

## Findings

Before = the old Italian text. Where one root defect recurred across keys or
pages it is one row.

### Catalog (`messages/it.json`)

| # | File / key | Before (old translation) | Type | Severity | Why | After |
|---|---|---|---|---|---|---|
| C1 | `home.meta.description`, `ogDescription`, `organization` | «Firma con una passkey: niente seed phrase, **niente chiavetta hardware**, nessun vincolo» | mistranslation (stale fact) | High | Vela signs with security keys (C-keys-2); the meta told search engines the opposite | «Firmi con passkey o chiavi di sicurezza, senza seed phrase…» |
| C2 | `home.hero.facts[3].term` / `.link` | «Anche se Vela smette di funzionare, continui ad accedere al tuo wallet.» / «Ospita tu l'app, il relay e ogni servizio di backend» | mistranslation | High | a survival promise the ledger does not license (C-rp-1: only named paths survive getvela.app); the link implied the passkeys' domain is self-hostable | «Tutto quello che Vela gestisce per il tuo wallet è open source, e puoi gestirlo tu.» / «Cosa gestire, cosa funziona anche senza getvela.app e cosa dipende ancora da noi» |
| C3 | `home.why.p1` | «una chiave di recupero generata in un browser… e se il servizio sparisce, il wallet se ne va con lui» (href `account.base.app`) | mistranslation (stale) | High | the English corrected both claims and the link target; the new objection is the unpublished signing service | new href, «(oggi parte di Coinbase Wallet)», «frase di recupero creata su un sito…», «non esiste un modo documentato perché la tua passkey arrivi al tuo account» |
| C4 | `home.tradeoffs.items[0]` | «in genere consumano più gas» · «fissata nel momento in cui firmi» · title «…che tiene in piedi il relay» · link to the GitHub repo | mistranslation | High | the fee was understated: no 3× reserve, no «often ten times or more» (C-fee-1); the title added a justification the source never made; «relayer» for the service | full fee rule, «spesso dieci volte o più il costo reale on-chain», link `/docs/self-hosting#relay` |
| C5 | `home.tradeoffs.items[1].body` | «Puoi impostare più chiavi quando crei il wallet» (no full stop) | mistranslation | Medium | dropped «up to seven» and «can’t be changed afterwards» (C-keys-1) | «Scegli fino a sette chiavi quando crei il wallet, e dopo non si possono più cambiare.» |
| C6 | `home.tradeoffs.items[2]` | «Ti stai fidando di contratti Safe **verificati**…» + body with no word on Vela’s own code | mistranslation | High | «verificati» reads as *verified*; the second half of C-audit-1 (Vela’s code not audited, none scheduled) was missing from the trade-off that exists to say it | «I contratti sono sottoposti ad audit, il codice di Vela no» + «…non hanno avuto un audit di terze parti, e non ce n’è nessuno in programma» |
| C7 | `home.compare.rows` (length) | 12 rows; no «Adding a key later» | technical | High | array shorter than English → `it.home` was a fragment (test failed) and the one row that states the fixed key set was missing | 13 rows; new row «Aggiungere una chiave dopo» |
| C8 | `home.compare.rows[9].vela` | «Con una pagina o estensione indipendente che ospiti tu» | mistranslation | High | presented the signing page as usable today; it is built but not connected (C-signpage-1) | «Una pagina di firma indipendente, pronta ma non ancora collegata alle app» |
| C9 | `home.compare.rows[10]` | «Autogestione completa · App, relay e servizi backend, tutti eseguibili da te» | mistranslation | High | omitted the gaps and that passkeys stay tied to getvela.app (C-rp-1) | «Self-hosting completo · …con le lacune elencate nella guida al self-hosting; le passkey restano legate a getvela.app» |
| C10 | `home.compare.rows[1]`, `[3]`, `[11]` | «Passkey» (Vela) · MetaMask sponsored gas «Non offerto» · «Ambito open source» with no licence caveat | mistranslation (stale) | Medium | security keys and the seven-key cap missing; MetaMask sponsors on some networks; C-lic-1 (index has no licence yet) | new rows 1, 3, 11 |
| C11 | `home.pricing.cards` | «Portafoglio web ed estensione Chrome» · «App desktop e mobile — Gratis dai sorgenti» · «Dagli app store — Acquisto una tantum» | mistranslation (stale) | Medium | desktop downloads are free, not source-only; store apps are not out yet (C-plat-1) | three new cards, «Acquisto una tantum · in arrivo» |
| C12 | `home.networks.heading` / `.body` | «12 reti integrate…» · link to an external biubiu.tools setup tool | mistranslation (stale) | High | 24 networks (C-net-1); the tool is now the site's own `/chain-setup` | «24 reti integrate. Aggiungi la tua» + `/chain-setup` |
| C13 | `home.faq.items[0].a` | «Un dispositivo che si sblocca con Face ID o con l'impronta, oppure una chiave di sicurezza USB/NFC… Aggiungi **tutte le chiavi che vuoi**» | mistranslation | High | no seven-key cap; one security key alone cannot create a wallet (the app asks for a second) | «…oppure chiavi di sicurezza hardware — due, se usi solo chiavi di sicurezza… fino a sette» |
| C14 | `home.faq.items[1].a` | «Funziona nelle app iOS, Android e desktop…» | mistranslation | Medium | desktop dApp browser is macOS/Windows only, and the web wallet does not connect (C-dapp-1) | «…app desktop (macOS, Windows), iPhone e Android. Il wallet web non si collega alle dApp.» |
| C15 | `home.faq.items[4].a` | «Chi arriva alle tue passkey sincronizzate può probabilmente arrivare anche al tuo wallet… usa invece una chiave di sicurezza USB/NFC» | mistranslation | Medium | no «a key can’t be removed / move your funds»; singular security key contradicts the two-key rule | new answer, «crea il wallet con chiavi di sicurezza hardware» |
| C16 | `home.faq.items[5].a` | «Vela non può spostare né congelare i tuoi fondi… La tua chiave pubblica, l'indirizzo del wallet e il nome…» | mistranslation | High | dropped «by itself» and «writes the software that asks your keys to sign»; public list incomplete (C-reg-1); services' view of IP and transactions missing | full answer, «per conto proprio», authenticator kind and labels, IP |
| C17 | `home.faq.items[6].a` | «…oppure l'estensione di firma leggibile senza dipendenze. Le chiavi che hai già funzionano in entrambe» | mistranslation | High | the signing page is not a way in on its own (not published, no app sends it requests); the relay's code change was missing | extension + self-built apps only; relay caveat |
| C18 | `about.meta.description`, `roadmap.meta.description`, `roadmap.lede`, `getStarted.fundingNote` | «costruito **allo scoperto**» / «rilascia… allo scoperto» | cultural risk | Medium | on a finance site «allo scoperto» first reads as *short / uncovered*; `roadmap.meta` and `lede` were not in the brief's list and were fixed as real defects | «in pubblico» |
| C19 | `about.lede`, `about.team.bio` | «Dietro non c'è un'azienda senza volto… il wallet, gli smart contract» | mistranslation (stale) | Medium | names no company (now MONDAY LABS LTD) and credits Vela with smart contracts it did not write | new lede and bio |
| C20 | `about.values[0..2].body` | «una passkey: il tuo volto o la tua impronta» · «La causa principale di cripto perse…» · «Il wallet è pubblico su GitHub» | mistranslation (stale) | Medium | a passkey is not a face (C-auth-1); an unsupported causal claim; only the wallet was said to be public | new bodies (PIN and security keys named; apps and services) |
| C21 | `roadmap.upcoming` | «…ti seguono già tramite il backup della piattaforma» · «una via di firma per le chain senza il precompilato P-256» · «**un audit di sicurezza indipendente** dell'integrazione» | mistranslation (stale) | High | three false or forbidden claims: account sync (C-sync-1), a P-256 fallback (C-p256-1), and an audit presented as upcoming (never allowed) | the five new items |
| C22 | `roadmap.shipped` (length) | 7 items | technical | High | English has 10 → `it.roadmap` was a fragment (test failed) | the ten new items |
| C23 | `getStarted.meta.description`, `lede` | «…con desktop, mobile e un'estensione costruiti dallo stesso codice» · «…sul computer, sul telefono…» | mistranslation (stale) | Medium | phone apps presented as available (C-plat-1) | «…e app per telefono in arrivo» · «…e presto sul telefono» |
| C24 | `getStarted.platforms.web.blurb`, `desktop.stores` | «Aprilo, autenticati con la tua passkey…» · «Mac App Store · Microsoft Store» | mistranslation (stale) | Medium | no «use the extension for dApps» (C-dapp-1); no Mac App Store | new blurb; «Microsoft Store» |
| C25 | `getStarted.fundingNote` | «…puoi sempre compilartelo da solo — stessa app, nessun costo» | mistranslation | Medium | a self-built phone app cannot use the phone's own passkey (C-rp-1) — «stessa app» promised otherwise | «…firma con un altro telefono o con una chiave di sicurezza, invece che con la passkey del telefono stesso» |
| C26 | `home.faq.items[2].a` | «Gestore password di Google» | terminology | Low | Google's Italian name is «Gestore delle password di Google», as the docs say | fixed |
| C27 | `home.compare.heading` | «Vela, messa a confronto» | terminology | Low | feminine Vela, masculine everywhere else | «Come si confronta Vela» |
| C28 | `about.values[0].title`, `chrome.docs.groups.selfHost` | «Self-custody, davvero» · «Ospitalo tu» | terminology | Low | half-English title; «ospitare» is narrower than *run* (the group covers running services too) | «Autocustodia, davvero» · «Gestiscilo tu» |
| C29 | `chrome.docs.ui.*`, `chrome.docs.groups.keys`, `titles["self-hosting"]`, `footer.links.selfHosting` | draft | — | — | reviewed: natural and consistent («Successiva →» agrees with *pagina*); kept | unchanged |

### Docs (`content/docs/it/`)

| # | File | Before (old translation) | Type | Severity | Why | After |
|---|---|---|---|---|---|---|
| D1 | introduction, faq, networks-and-fees, whitepaper, security-audits | «**12 reti**» / «dodici reti integrate» | mistranslation (stale) | High | 24 built-in networks (C-net-1) | 24, with the full list where the English has it |
| D2 | networks-and-fees, faq, whitepaper | «attivare l'account di gas… deposito **non rimborsabile**… riattivazione» | mistranslation (stale) | High | there is no gas account or activation deposit (C-fee-2) | treasury section: optional, non-refundable top-up that does not pay your transaction; «Non esiste un account di gas per wallet né un deposito di attivazione» |
| D3 | networks-and-fees, whitepaper | «**Il relay è l'unica fonte di verità** per il prezzo del gas… Non c'è un **selettore di velocità**» | mistranslation (stale) | High | the price is the higher of the wallet's reading and the relay's; a speed can be chosen, default fast (C-fee-1, C-fee-3) | the fee rule, speeds lenta / standard / rapida |
| D4 | whitepaper, faq | «Nessun ricarico nascosto» · «costo di rete più la commissione di servizio» | mistranslation (stale) | Medium | framing the ledger check forbids; the fee is usually several times the on-chain cost | «spesso dieci volte o più… il relay tiene la differenza» |
| D5 | create-wallet, whitepaper | «Vela deriva il tuo indirizzo **dalla chiave pubblica della passkey**» | mistranslation (stale) | High | the address comes from all founding keys (C-addr-1) | «dall'insieme completo delle chiavi» |
| D6 | introduction, passkeys, whitepaper, send-and-receive | «firmi con una passkey, **usando il volto o l'impronta**» · «custodita dal sistema operativo» | mistranslation (stale) | High | the only kinds named were face/fingerprint and the OS keychain (C-auth-1, C-keys-2); another phone and security keys were absent from the page that explains keys | kinds table; «Face ID, l'impronta, il PIN del dispositivo, oppure un tocco e il PIN su una chiave di sicurezza» |
| D7 | signers, why-vela | «registra una YubiKey alla creazione e firma con quella» · «puoi rendere la primissima chiave una chiave di sicurezza hardware» | mistranslation | High | a wallet whose only key is unsynced cannot be created — hardware-only needs two keys | «crealo con due chiavi di sicurezza» |
| D8 | signers | owner changes «un'operazione Safe che Vela oggi non espone»; no section on a compromised key | mistranslation (omission) | Medium | the reason (owner sets drifting across chains) and «move everything to a new wallet» were missing | both restored |
| D9 | recovery | recovery = «Accedi allo stesso account iCloud o Google» · «Vela può ricostruire la tua chiave pubblica… da due firme» (unqualified) | mistranslation (stale) | High | any one key signs in, the registry is read on Gnosis then Ethereum, and two-signature rebuild works only for single-key wallets | the new sign-in steps and the single-key callout |
| D10 | send-and-receive, clear-signing | «Vela **la riscrive su un importo finito**» · callout «Le approvazioni illimitate vengono bloccate» | mistranslation (stale) | High | the user must change it; signed permits, large finite approvals and `setApprovalForAll` are not blocked (C-approve-1) | «non la invia finché non la trasformi…»; the «what it does not stop» sentence |
| D11 | clear-signing | «verificata» with no caveat; no lookup order | mistranslation (omission) | High | fetched descriptors are not cryptographically authenticated (C-clear-1) | «“Verificata” significa che è stato trovato un descrittore…, non che sia stato controllato crittograficamente» |
| D12 | bybit-attack | «togliere la primitiva di upgrade su cui l'attacco si è appoggiato» · no self-call paragraph | mistranslation | High | the owner-signed `delegatecall` still exists in every Safe, Vela's included; the warning to reject self-targeted requests was missing | the «Due limiti» paragraph and the corrected admin-role paragraph |
| D13 | bybit-attack, clear-signing-self-host | «Vela sta costruendo una pagina di firma… Quando uscirà sarà opzionale» · no status line | mistranslation (stale) | High | built and tested, not published, no app sends it requests (C-signpage-1) | status lines as in the reference |
| D14 | bybit-attack | «L'auto-hosting… è l'unica risposta… che non richiede di fidarsi di qualcuno» | mistranslation | Medium | overclaim: you still trust the code you build | «resta il codice che compili, di cui ti fidi comunque, quindi leggilo» |
| D15 | account-contract | «qualsiasi interfaccia compatibile con Safe può guidarlo» · «Non esiste un contratto Vela» | mistranslation (stale) | High | Safe tools can read but not sign for a getvela.app key (C-safeui-1); Vela does have registry contracts, outside the funds path | corrected paragraph; «I contratti di Vela non sono in questo elenco…» |
| D16 | account-contract | «Aspettati circa 1,5–3 volte il gas di un semplice trasferimento EOA» | mistranslation (stale) | High | measured 140,000–170,000 gas vs 21,000 | the measured figures |
| D17 | account-contract, security-audits | «Il codice applicativo di Vela non ha avuto un audit» · «Vela stesso… non hanno avuto un audit» (no «none scheduled», no registry) | mistranslation (omission) | Medium | C-audit-1 names apps, backend services and the registry contract, and «none is scheduled» | full sentence |
| D18 | security-audits | «l'operazione fallisce, ma il gas viene comunque addebitato» · «c'è poca occasione di intercettarne una» | mistranslation (stale) | High | with the in-band fee the relay, not you, absorbs the gas; `handleOps` is still visible in the public mempool | corrected interception section |
| D19 | security-audits | no Certora M-01, no «Gaps in Vela's own defences» | mistranslation (omission) | High | the page's known issues were missing the medium finding and six tracked gaps | both sections |
| D20 | faq, whitepaper | «il wallet e i suoi quattro servizi backend… con licenza MIT» | mistranslation (stale) | High | the public-key index has no licence file yet (C-lic-1) | the licence sentence from the reference |
| D21 | whitepaper | «Ciò di cui devi fidarti si riduce a smart contract auditati, alla cassaforte di passkey… e — solo per la disponibilità — a un relay» | mistranslation (stale) | High | omitted the app code, the domain and the backend services as trust points | the «Di cosa ti fidi» list |
| D22 | install | «niente da scaricare e nessun app store… Le app mobile native arrivano presto»; no `dapps` anchor | mistranslation (stale) / technical | High | extension and desktop exist; the anchor other pages link to was missing (test failed) | the four-platform table, `<span id="dapps">` |
| D23 | create-wallet | no key count, no «fixed at creation», no `what-is-public` section | mistranslation (omission) / technical | High | C-keys-1 and C-reg-1; the anchor was missing (test failed) | full steps, callout, «Cosa è pubblico» |
| D24 | why-vela | callout link «account intelligente Safe» → `/it/docs/security-audits` | technical | Medium | wrong target; the English links the account-contract page | `/it/docs/account-contract` |
| D25 | why-vela | «ospitabile in proprio perché il tuo wallet non dipenda mai dal fatto che la nostra azienda resti online» | mistranslation (stale) | High | overclaim; the domain limit is the reference's point (C-rp-1) | «…con un limite, il dominio a cui appartengono le tue passkey…» |
| D26 | self-hosting | page missing | technical | High | linked from home, the sidebar and six docs | created, all six anchors |
| D27 | clear-signing-self-host | «nessuna richiesta di rete per conto proprio» | mistranslation (stale) | Medium | it loads token logos from the chain-data server | «l'unica richiesta… è per i loghi decorativi dei token» |
| D28 | all old docs | «account intelligente», «frase seed», «auto-custodito», «firma al buio», «distribuito» for deployed, «indice passkey» for the registry | terminology | Medium | each disagreed with the catalog or the app, and «distribuito» / «indice passkey» were ambiguous | the table above |
| D29 | passkeys | «È la stessa tecnologia che protegge Apple Pay» | mistranslation (stale) | Low | unsupported, and gone from the reference | removed |

## Open items

- **seed phrase vs frase di recupero.** The Italian *app* says «frase di
  recupero» for seed phrase; the site has said «seed phrase» since 059 (footer,
  hero, FAQ), and I kept that to avoid changing a founder-approved tagline. Both
  are understood by Italian crypto readers; aligning app and site is a
  product-wide call, not a page fix.
- **«deployare».** Chosen over «distribuire» (ambiguous) and «installare»
  (wrong). It is the word Italian developers use, but it is jargon in the few
  user-facing places it appears (create-wallet, networks-and-fees). Left as is;
  «viene creato on-chain» would be the plainer alternative if the founder prefers.

## Result
reviewed — no open High or Medium findings
