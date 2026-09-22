---
title: Audit e problemi noti
description: "Ogni contratto da cui Vela dipende, chi ha sottoposto ad audit quale versione, se la versione controllata è quella deployata, i rilievi aperti che teniamo d'occhio e ciò che non ha avuto alcun audit."
source: c4c50ad89f2f
---

«Ha avuto un audit» è un'affermazione su un codice specifico in una versione
specifica, quindi questa pagina cita i report, i commit e gli indirizzi deployati
— ed elenca ciò che **non** ha avuto un audit, che conta altrettanto.

Ultima revisione: 22 settembre 2026. Se trovi un errore, diccelo e lo correggiamo.

## La strada dei fondi

Ogni contratto che può toccare i tuoi soldi è un deploy canonico di codice di terze
parti con revisioni pubbliche.

### Safe v1.4.1 — l'account stesso

Il tuo wallet è un proxy [Safe](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
che usa il singleton SafeL2 e SafeProxyFactory. I batch passano per MultiSend.

[Ackee Blockchain ha sottoposto ad audit Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(report finale del 16 marzo 2023, verifica delle correzioni del 28 marzo): 11
rilievi, nessuno critico o alto; i due rilievi di gravità media sono stati
riconosciuti, senza modifiche al codice. Il perimetro comprendeva SafeL2, SafeProxyFactory,
CompatibilityFallbackHandler, MultiSendCallOnly e SignMessageLib. La v1.4.1
differisce dalla v1.4.0 per una sola riga funzionale, una correzione di
compatibilità con ERC-4337 nel setup dei moduli
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)); Safe ha
consultato Ackee e ha concluso che non serviva un nuovo audit. La logica di
MultiSend è invariata dalla v1.3.0, che è stata
[sottoposta ad audit da G0 Group](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Tutti gli indirizzi corrispondono a
[safe-deployments](https://github.com/safe-global/safe-deployments). I contratti
core rientrano nel
[bug bounty della Safe Foundation](https://docs.safefoundation.org/security/bug-bounty),
la cui fascia più alta paga fino a 1.000.000 di dollari.

L'incidente di Bybit del 2025 non è un rilievo sui contratti: gli attaccanti hanno
manomesso il JavaScript servito all'interfaccia web di Safe, e la
[dichiarazione forense](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
di Safe non ha trovato vulnerabilità nei contratti.
[La nostra pagina sull'attacco](/it/docs/bybit-attack) spiega perché la stessa
classe di attacchi riguarda ogni interfaccia di wallet, compresa la nostra.

### Safe4337Module v0.3.0 — l'adattatore ERC-4337

Deployato a `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` (corrispondenza esatta su
Sourcify), e impostato anche come fallback handler del tuo Safe. Esaminato tre
volte —
[report qui](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md):

- **Ackee Blockchain**, report finale di marzo 2024: un avviso (uso
  dell'ottimizzatore del compilatore) riconosciuto, niente di più grave aperto.
- **Certora**, agosto 2026: un rilievo di gravità **media**, riconosciuto e
  **non corretto** nella v0.3.0 — *le modifiche alle autorizzazioni non
  invalidano le UserOperation successive già validate nello stesso bundle*. Vedi
  «Problemi noti» più sotto.
- **Nethermind**, agosto 2026: nessun rilievo.

SafeModuleSetup v0.3.0 (`0x2dd6…5b47`), che abilita il modulo quando un wallet
viene deployato, rientrava nelle revisioni di Certora e Nethermind.

La storia del modulo ha un solo problema reso pubblico: la v0.1.0 non firmava
`initCode` e `paymasterAndData`, un vettore di gas griefing
[corretto nella v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module);
Safe riferisce che la v0.1.0 non è stata usata fuori dalle testnet. Vela usa la
v0.3.0 con l'EntryPoint v0.7 e Safe 1.4.1, la configurazione descritta dalla
release del modulo.

### Modulo passkey di Safe v0.2.1 — i firmatari

La tua prima chiave viene verificata da **SafeWebAuthnSharedSigner** a
`0x94a4F6affBd8975951142c3999aEAB7ecee555c2`. «Shared» significa che è condiviso
il deploy del contratto, come per il singleton di Safe; la tua chiave no. Ogni
Safe conserva la propria chiave pubblica P-256 nel proprio storage.

Ogni chiave aggiuntiva ha un proprio contratto firmatario, creato da
**SafeWebAuthnSignerFactory** a `0x1d31F259eE307358a26dFb23EB365939E8641195`
come proxy verso il **singleton SafeWebAuthnSigner** a
`0x4E27b51350e6c2083EE19011120F50DAfEc5CA50`.

Le revisioni che coprono questi contratti nella v0.2.1
([report](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)):

- Una [audit competition di Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
  (giugno–luglio 2024): nessun rilievo alto o medio; tre bassi, tutti corretti.
- La revisione di **Certora** del commit di release: nessun nuovo rilievo.
  (L'audit precedente della v0.2.0 annota che il firmatario condiviso non era
  ancora stato sottoposto ad audit: è stato aggiunto dopo quell'audit.)
- **Nethermind**, agosto 2026: nessun rilievo.

Dalla release non è stata resa pubblica alcuna vulnerabilità a livello di
contratto, e i contratti passkey rientrano nel bounty della Safe Foundation.

Le firme delle passkey vengono verificate dal precompilato **EIP-7951 / RIP-7212** della
chain, senza verificatore di riserva. Prima di abilitare una rete, l'app controlla
il precompilato con una firma vera. Due avvertenze: la specifica originale di
RIP-7212 ha difetti nei casi limite che [EIP-7951](https://eips.ethereum.org/EIPS/eip-7951)
corregge (riguardano solo input che dovrebbero comunque fallire, non firme
WebAuthn ben formate), e un controllo di prova non può individuare ogni modo in
cui l'implementazione di una chain potrebbe discostarsi.

### EntryPoint v0.7 — esegue la tua operazione

Deployato a `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, la
[release canonica v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Sottoposto ad audit da OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
per la Ethereum Foundation (gennaio 2024): nessun rilievo critico o alto, cinque
medi, tutti i 24 rilievi risolti; il commit della verifica delle correzioni
corrisponde alla release. Rientra nel
[bug bounty ERC-4337](https://docs.erc4337.io/community/bug-bounty) della
Ethereum Foundation (fino a 250.000 dollari).

## Problemi noti che teniamo d'occhio

### Modifiche alle autorizzazioni dentro un bundle (Safe4337Module, Certora M-01)

L'EntryPoint valida tutte le operazioni di un bundle prima di eseguirne una
qualsiasi. Quindi, se un'operazione rimuove un proprietario, un'operazione firmata
da quel proprietario e posizionata più avanti nello stesso bundle supera comunque
la validazione e viene eseguita. Safe ha riconosciuto il problema e non ha
modificato la v0.3.0.

Le app di Vela non costruiscono mai modifiche ai proprietari, quindi Vela in sé
non lo innesca mai. Conta comunque: una dApp può chiedere al tuo wallet di
cambiare i propri proprietari (vedi «Lacune» più sotto), e chi rimuovesse una
chiave compromessa tramite altri strumenti Safe non potrebbe contare sul fatto che
venga esclusa già nello stesso bundle.

### Intercettazione di un'operazione firmata (EntryPoint precedenti alla v0.9)

A febbraio 2026 alcuni ricercatori hanno
[reso pubblico](https://erc4337.substack.com/p/improving-useroperation-execution)
un vettore di griefing e censura che riguarda tutti gli EntryPoint precedenti alla
v0.9, compresa la v0.7. Chi ottiene un'operazione firmata prima che venga minata
può eseguirla dentro una chiamata che controlla e forzare il revert
dell'esecuzione interna: l'operazione fallisce e va firmata di nuovo. (Con la
commissione in banda di Vela anche il trasferimento della commissione va in
revert, quindi il gas lo assorbe il relay e non tu.) Riguarda le operazioni che
chiamano contratti protetti dalla reentrancy o che possono essere mandate in
revert da uno stato temporaneo; i semplici trasferimenti non ne sono toccati.
Usato ripetutamente contro dei flussi di prelievo, potrebbe rendere i fondi
indisponibili per un certo tempo. Non può falsificare una firma né dirottare i
fondi.

Il relay di Vela invia le operazioni direttamente invece che tramite una mempool
condivisa, ma una transazione `handleOps` in attesa resta comunque visibile nella
mempool pubblica, quindi questo riduce l'esposizione senza eliminarla. La
correzione esiste solo nell'EntryPoint v0.9 (novembre 2025); la v0.7 non si può
correggere. La migrazione dipende dal supporto della v0.9 da parte del modulo 4337
di Safe, e questa pagina dirà quando avverrà.

### Lacune nelle difese di Vela

Non sono rilievi sui contratti, ma punti in cui il wallet ti protegge meno di
quanto potresti pensare. Per ognuno è prevista una correzione:

- **Le chiamate dal tuo wallet verso se stesso non vengono bloccate.** Una dApp può
  chiedere `enableModule`, `addOwnerWithThreshold`, `setFallbackHandler` o
  `setGuard` sul tuo Safe; una qualsiasi di queste, firmata una sola volta,
  consegna l'account. Vela decodifica queste chiamate ma non le ferma. Rifiuta
  qualsiasi richiesta il cui destinatario sia il tuo stesso indirizzo.
- **Il controllo sulle approvazioni ferma solo gli importi «illimitati»** (2^200 o
  più; 2^152 per Permit2). Un'approvazione finita ma elevata, un permit firmato o
  un `setApprovalForAll` degli NFT ricevono un avviso, non un blocco.
- **I descrittori scaricati non sono autenticati.** Un descrittore dal server dei
  dati delle chain viene mostrato come «verificato» se corrisponde al contratto;
  è affidabile solo quanto quel server.
- **La pagina di firma indipendente non è collegata** ancora a nessuna app.
- **Il controllo della rete non verifica la factory dei firmatari passkey di
  Safe**, che serve alle chiavi dalla seconda alla settima; su una rete aggiunta
  senza di essa, può firmare solo la prima chiave.
- **Il sito web carica uno script di analytics di terze parti** sullo stesso
  dominio delle passkey. Il sito vieta alle proprie pagine di usare le passkey
  (con un header Permissions-Policy) e tiene lo script lontano dalla pagina che
  custodisce una chiave.

## Cosa non ha avuto un audit

- **I contratti di Vela.** Il
  [registro delle chiavi pubbliche](https://github.com/mondaylabsltd/p256-index/tree/main/contracts)
  a `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` (Gnosis; lo stesso indirizzo su
  Ethereum e Base), il deploy originale del registro a
  `0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf` (il suo indirizzo fa parte del
  dominio di firma di ogni registrazione) e il vecchio indice che hanno sostituito
  (`0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3`, storico in sola lettura). Non
  hanno avuto un audit. Non custodiscono fondi, non hanno un proprietario e non
  sono aggiornabili; sono un livello di ricerca, non di autorizzazione. Il potere
  di spesa viene solo dalle chiavi configurate nel tuo Safe. Il peggior guasto
  realistico è che un wallet diventi più difficile da trovare su un nuovo
  dispositivo, non che si spostino dei soldi.
- **Multicall3.** Il suo README
  [dice](https://github.com/mds1/multicall3) «This contract is unaudited» (questo
  contratto non ha avuto un audit). Vela lo usa solo per letture in batch — saldi,
  dettagli dei token, quotazioni di prezzo — mai con approvazioni o fondi.
- **I deployer deterministici** (il proxy CREATE2 di Arachnid e la singleton
  factory di Safe) — standard dell'ecosistema e senza stato, senza audit formali.
  Il controllo di rete di Vela fallisce in modo sicuro se mancano; verifica che
  all'indirizzo ci sia del codice, non che corrisponda byte per byte.
- **Tempo.** Una delle 24 reti integrate, senza moneta nativa; lì Vela paga il gas
  nella stablecoin pathUSD. A settembre 2026, la
  [security policy](https://github.com/tempoxyz/.github/blob/main/SECURITY.md) di
  Tempo dice che il protocollo è ancora sotto audit e non ha un bug bounty attivo.
  I fondi tenuti su Tempo, e il gas pagato lì, portano con sé quel rischio a
  livello di chain; considerala la chain più nuova e meno collaudata dell'elenco.
- **Vela stesso.** Le app, i servizi di backend e i contratti qui sopra non hanno
  avuto un audit di terze parti, e non ce n'è nessuno in programma. È l'avvertenza
  più importante di questa pagina. I dettagli sono in
  [Vela is in alpha](/blog/vela-is-in-alpha). Parti con importi piccoli, e leggi il
  codice.

## Verifica tu stesso

Ogni indirizzo qui sotto è un deploy pubblico canonico. Verificali su
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
e nella [release dell'EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Contratto                                           | Indirizzo                                    |
| --------------------------------------------------- | -------------------------------------------- |
| Singleton SafeL2 v1.4.1                             | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1                             | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| MultiSend v1.4.1                                    | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| CompatibilityFallbackHandler v1.4.1 ¹               | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| SafeModuleSetup v0.3.0                              | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0                               | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1                     | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| SafeWebAuthnSignerFactory v0.2.1                    | `0x1d31F259eE307358a26dFb23EB365939E8641195` |
| Singleton SafeWebAuthnSigner v0.2.1                 | `0x4E27b51350e6c2083EE19011120F50DAfEc5CA50` |
| EntryPoint v0.7                                     | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3                                          | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Registro delle chiavi pubbliche (Vela, senza audit) | `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` |

¹ Controllato quando si aggiunge una rete; come fallback handler il tuo Safe usa
invece il modulo 4337.
