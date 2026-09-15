---
title: Audit e problemi noti
description: Ogni contratto on-chain da cui Vela dipende, chi l'ha auditato, se la versione auditata coincide con quella davvero distribuita, e cosa non è auditato affatto.
---

«Auditato» è un'affermazione su una versione precisa di un codice preciso, quindi
questa pagina non si limita ad agitare la parola: cita i report esatti, gli
indirizzi di deployment esatti e le differenze tra versione auditata e distribuita.
Elenca anche ciò che _non_ è auditato, perché quella lista pesa quanto la prima.

Ultima revisione: agosto 2026. Se trovi un errore qui, diccelo e lo correggiamo.

## Il percorso dei fondi

Quattro livelli di contratti possono toccare i tuoi soldi. Tutti e quattro sono
contratti di terze parti con audit pubblicati, e in ogni caso l'indirizzo
distribuito è il deployment canonico ufficiale.

### Safe v1.4.1 — l'account stesso

Il tuo wallet è un proxy
[Safe](https://github.com/safe-global/safe-smart-account): singleton SafeL2, proxy
factory, fallback handler di compatibilità e MultiSend per i batch.

[Ackee Blockchain ha auditato Safe v1.4.0](https://github.com/safe-global/safe-smart-account/blob/main/docs/audit_1_4_0.md)
(report finale marzo 2023): 11 rilievi, nessuno critico o alto. La v1.4.1 che
distribuiamo differisce dalla v1.4.0 auditata per una correzione di compatibilità
ERC-4337 di una sola riga
([PR #572](https://github.com/safe-global/safe-smart-account/pull/572)). La logica
di MultiSend è invariata dalla
[v1.3.0 auditata da G0 Group](https://github.com/safe-global/safe-smart-account/tree/main/docs).
Tutti gli indirizzi coincidono con i deployment canonici di
[safe-deployments](https://github.com/safe-global/safe-deployments), e i contratti
rientrano nel
[bug bounty della Safe Foundation](https://docs.safefoundation.org/security/bug-bounty)
(fino a 1.000.000 $ per rilievi critici).

Una cosa che un audit non copre: l'incidente Bybit del 2025. Quell'attacco ha
compromesso la pipeline di build del frontend web ufficiale di Safe, non i
contratti — la
[conclusione forense ufficiale](https://safefoundation.org/blog/safe-ecosystem-foundation-statement)
non ha trovato vulnerabilità negli smart contract di Safe. Noi la leggiamo come una
lezione sul livello web e operativo, che è poi il livello su cui dovresti esaminare
anche noi.

### Safe4337Module v0.3.0 — l'adattatore ERC-4337

Distribuito a `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226`, l'indirizzo canonico
della v0.3.0 (corrispondenza esatta su Sourcify: il bytecode on-chain è il codice
auditato).
[Auditato da Ackee Blockchain](https://github.com/safe-global/safe-modules/blob/main/modules/4337/docs/v0.3.0/audit.md)
(report finale marzo 2024), senza rilievi irrisolti sopra il livello informativo.
La combinazione v0.3.0 + EntryPoint v0.7 + Safe ≥ 1.4.1 che usiamo è esattamente la
configurazione descritta dall'audit e dalle note di rilascio.

La storia del modulo include un problema divulgato: la v0.1.0 (2023) non firmava
`initCode` e `paymasterAndData`, un vettore di griefing sul gas. È stato
[corretto nella v0.2.0](https://safefoundation.org/blog/strengthening-security-addressing-the-incident-of-the-canonical-4337-module)
e la v0.1.0 non ha mai lasciato le testnet. Noi usiamo la v0.3.0, che eredita la
correzione.

### SafeWebAuthnSharedSigner v0.2.1 — il firmatario delle passkey

Distribuito a `0x94a4F6affBd8975951142c3999aEAB7ecee555c2`, l'indirizzo canonico
della v0.2.1 (lo stesso su ogni chain tramite la singleton factory di Safe).

Cosa vuol dire «shared» — e cosa non vuol dire: a essere condiviso è il
_deployment del contratto_, come lo è il singleton di Safe. La tua chiave no. Ogni
Safe chiama `configure()` via delegatecall e conserva la propria chiave pubblica
P-256 nel proprio storage. Un'istanza del firmatario rappresenta esattamente una
passkey per Safe, e il Safe di qualcun altro non può usare la tua.

Qui la versione conta. L'audit della v0.2.0
[affermava esplicitamente](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.0/audit.md)
che il firmatario condiviso era fuori perimetro: il contratto non esisteva ancora.
Gli audit che coprono ciò che distribuiamo sono quelli della v0.2.1: una
[competizione di audit di Hats Finance](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit-competition-report-hats.md)
(giugno–luglio 2024: zero alti, zero medi, tre rilievi bassi — tutti risolti) più
una
[revisione Certora del commit di rilascio](https://github.com/safe-global/safe-modules/blob/main/modules/passkey/docs/v0.2.1/audit.md)
senza nuovi rilievi. Dal rilascio non è stata divulgata alcuna vulnerabilità a
livello di contratto; i contratti delle passkey rientrano nel bounty della Safe
Foundation.

La documentazione di Safe consiglia di affiancare alla proprietà tramite passkey un
percorso di recupero, invece di trattare una singola credenziale come l'unica chiave
dell'account. Come lo gestisce Vela è documentato in
[recupero e accesso](/it/docs/recovery).

La verifica P-256 on-chain usa direttamente il precompilato RIP-7212, senza
verificatore Solidity di riserva. Prima di abilitare una rete, l'app sonda il
precompilato con una firma reale e rifiuta la rete se la verifica fallisce. Due
avvertenze oneste: la specifica RIP-7212 originale ha difetti su casi limite che
l'[EIP-7951](https://eips.ethereum.org/EIPS/eip-7951) è stato scritto per correggere
(non riguardano firme WebAuthn ben formate), e una sonda non può intercettare ogni
modo in cui l'implementazione di una chain potrebbe divergere in contesti di
esecuzione insoliti.

### EntryPoint v0.7 — il punto d'ingresso ERC-4337

Distribuito a `0x0000000071727De22E5E9d8BAf0edAc6f37da032`, il
[deployment canonico della v0.7.0](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0).
[Auditato da OpenZeppelin](https://www.openzeppelin.com/news/erc-4337-account-abstraction-incremental-audit)
(su commissione della Ethereum Foundation, gennaio 2024): zero critici, zero alti,
cinque rilievi medi, tutti risolti — e il commit auditato è la release distribuita.
EntryPoint v0.7.0 rientra nel
[bug bounty ERC-4337](https://docs.erc4337.io/community/bug-bounty) della Ethereum
Foundation (fino a 250.000 $).

## Problemi noti che stiamo seguendo

### Il vettore di griefing dell'EntryPoint

A febbraio 2026 i ricercatori di sicurezza di Trust Security hanno
[divulgato](https://erc4337.substack.com/p/improving-useroperation-execution)
un vettore di griefing e censura che riguarda ogni EntryPoint precedente alla v0.9,
inclusa la v0.7 che usiamo. Un attaccante che intercetta una UserOperation firmata
prima che venga inclusa può eseguirla dentro un frame di chiamata che controlla lui
e forzare il fallimento dell'esecuzione interna: l'operazione fallisce, ma il gas
viene comunque addebitato. La Ethereum Foundation ha pagato ai ricercatori un bounty
da 50.000 $; ha classificato il problema come vettore di censura/griefing, non di
furto di fondi, e non è mai stato sfruttato.

Cosa può fare: sprecare una commissione e ritardare una transazione. Cosa non può
fare: rubare fondi o falsificare una firma. L'esposizione di Vela è stretta perché
le UserOperation vanno dritte a un relay invece che attraverso una mempool
pubblica — quindi c'è poca occasione di intercettarne una — e il caso peggiore è
limitato dalla commissione che hai già accettato. La correzione esiste solo in
EntryPoint v0.9 (novembre 2025); la v0.7 in sé non è patchabile. Contiamo di
migrare man mano che lo stack attorno — in particolare la linea del modulo 4337 di
Safe — supporterà la v0.9, e lo annoteremo qui.

## Cosa non è auditato

- **I contratti di Vela.** Due piccoli contratti scritti da noi e distribuiti su
  Gnosis: l'
  [indice delle chiavi pubbliche delle passkey](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools)
  (un registro in sola aggiunta che aiuta i tuoi dispositivi a trovare la tua chiave
  pubblica) e il suo helper per i batch. Non sono auditati. Per costruzione non
  detengono fondi, non hanno un proprietario e non possono essere aggiornati: sono
  un livello di scoperta, non di autorizzazione. Il potere di spesa arriva sempre
  dalla passkey configurata dentro il tuo Safe. Il peggior guasto realistico è il
  griefing (qualcuno occupa una voce dell'indice), che può rendere il recupero meno
  comodo ma non può spostare soldi. Un contratto splitter per il regolamento del gas,
  figlio di un vecchio design delle commissioni, non fa più parte del flusso di
  transazione.
- **Multicall3.** Il suo stesso README lo
  [dice chiaramente](https://github.com/mds1/multicall3): «This contract is
  unaudited.» Lo usiamo esattamente nel modo che i suoi autori descrivono come
  sicuro: chiamate di sola lettura in batch per saldi, metadati dei token e
  quotazioni. Vela non gli concede mai approvazioni e non gli fa mai detenere fondi.
  Il caso peggiore di un bug è una lettura sbagliata.
- **Il deployer CREATE2.** Il
  [proxy di deployment deterministico di Arachnid](https://github.com/Arachnid/deterministic-deployment-proxy)
  è il deployer stateless standard dell'ecosistema; non ha un audit formale. I
  nostri controlli di rete falliscono in modo conservativo se manca o è alterato su
  una chain.
- **Tempo e pathUSD.** Tempo, una delle nostre dodici reti integrate, non ha moneta
  nativa; lì il gas è regolato nella stablecoin pathUSD. Ad agosto 2026 né il
  protocollo di base di Tempo né pathUSD hanno un audit di sicurezza pubblicato o un
  bug bounty, e una
  [valutazione indipendente del collaterale di DefiLlama](https://artifacts.llama.fi/md-exports/pathusd-collateral-assessment-april2026-1776332825042.md)
  (aprile 2026) ha classificato pathUSD ad alto rischio. È un rischio a livello di
  chain che nessun wallet può mitigare: i fondi che tieni su Tempo, e il regolamento
  del gas lì, lo ereditano. Tratta Tempo come la chain più nuova e meno collaudata
  della lista e dimensiona i saldi di conseguenza. Aggiorneremo questa sezione man
  mano che usciranno audit.
- **Vela stesso.** La nostra app e i servizi backend non hanno avuto un audit di
  terze parti. È la maggiore riserva di questa pagina, lo scriviamo
  nell'intestazione del sito, e i dettagli onesti sono in
  [Vela è in alpha](/blog/vela-is-in-alpha). Parti con importi piccoli. Leggi il
  codice.

## Verifica tu stesso

Ogni indirizzo qui sopra è un deployment pubblico canonico che puoi confrontare con
i registri ufficiali —
[safe-deployments](https://github.com/safe-global/safe-deployments),
[safe-modules-deployments](https://github.com/safe-global/safe-modules-deployments)
e le
[note di rilascio dell'EntryPoint](https://github.com/eth-infinitism/account-abstraction/releases/tag/v0.7.0):

| Contratto | Indirizzo |
| ----------------------------------- | -------------------------------------------- |
| SafeL2 singleton v1.4.1 | `0x29fcB43b46531BcA003ddC8FCB67FFE91900C762` |
| SafeProxyFactory v1.4.1 | `0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67` |
| CompatibilityFallbackHandler v1.4.1 | `0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99` |
| MultiSend v1.4.1 | `0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526` |
| SafeModuleSetup v0.3.0 | `0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47` |
| Safe4337Module v0.3.0 | `0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226` |
| SafeWebAuthnSharedSigner v0.2.1 | `0x94a4F6affBd8975951142c3999aEAB7ecee555c2` |
| EntryPoint v0.7 | `0x0000000071727De22E5E9d8BAf0edAc6f37da032` |
| Multicall3 | `0xcA11bde05977b3631167028862bE2a173976CA11` |
| Indice delle chiavi pubbliche delle passkey (Gnosis) | `0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3` |
