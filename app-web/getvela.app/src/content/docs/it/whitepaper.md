---
title: Whitepaper
description: "Come funziona Vela e di cosa devi — e non devi — fidarti per usarlo: l'account, le chiavi, la commissione, il modello delle minacce, il recupero e cosa succede se Vela sparisce."
source: 60d297b650ac
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Stato: alpha · ultima revisione settembre 2026">
Questa pagina descrive come funziona Vela oggi e di cosa devi o non devi fidarti
per usarlo. Vela è in <a href="/blog/vela-is-in-alpha">alpha</a>: parti con importi
piccoli. Vela non ha un token. Tutto ciò che c'è qui si può verificare sul codice
open source; dove il codice e questa pagina non concordano, ha ragione il codice e
questa pagina è un bug.
</Callout>

## Sintesi

Vela è un **wallet a smart contract in autocustodia** per Ethereum e le altre reti
EVM. Ogni wallet è un account **Safe v1.4.1** non modificato, operato tramite
**ERC-4337** e controllato da un massimo di sette **passkey** — chiavi WebAuthn
P-256 custodite dai tuoi dispositivi, dal tuo gestore di password o da chiavi di
sicurezza hardware. Non c'è alcuna seed phrase.

Vela, l'azienda, non detiene mai le tue chiavi e non ha alcun ruolo sul tuo Safe,
quindi **non può spostare, congelare o sequestrare i tuoi fondi** per conto
proprio. Scrive e distribuisce però il software che chiede alle tue chiavi di
firmare — ed è per questo che il modello delle minacce qui sotto conta. Le app, il relay che invia le
transazioni e i servizi di supporto sono open source, e puoi gestire una tua copia
di ciascuno. Di cosa ti fidi, in breve: dei contratti,
degli autenticatori che custodiscono le tue chiavi, del codice dell'app con cui
firmi, del dominio a cui appartengono le tue passkey e dei servizi verso cui
indirizzi l'app.

## Perché esiste Vela

- **I wallet con seed phrase** mettono davanti a ogni utente un segreto di 12–24
  parole: un unico punto di rottura e un bersaglio costante per il phishing.
- **I wallet custodial** eliminano la seed phrase prendendo in custodia i fondi.
- **I wallet a passkey** che dipendono dai server e dal codice chiuso di una sola
  azienda eliminano la seed phrase, ma ti lasciano a piedi se l'azienda sparisce.
- **La firma alla cieca** — approvare dati opachi che non sai leggere — è ancora
  diffusa, ed è uno dei modi in cui i wallet vengono svuotati.

Vela punta alla comodità di una passkey senza nessuna di queste dipendenze: un
account standard, codice aperto, servizi sostituibili e transazioni che puoi
leggere prima di firmare.

## Principi di progettazione

1. **Autocustodia, senza eccezioni.** Le chiavi vengono create e custodite dai
   tuoi autenticatori. I servizi di Vela non le vedono mai; ciò che vedono è
   elencato alla voce Privacy.
2. **Contratti standard, non modificati.** Nessun contratto sulla strada verso i
   tuoi fondi è stato scritto da Vela.
3. **Verifica, non fidarti.** Le app e i servizi sono pubblici; i servizi si
   possono ospitare in proprio.
4. **Decodificare prima di firmare.** Ciò che non si può decodificare porta un
   avviso esplicito di firma alla cieca.
5. **Fare meno.** Il wallet invia, riceve e firma per le dApp che scegli tu.

## Architettura

```text
App Vela — web, estensione del browser, desktop (macOS/Windows/Linux), iOS, Android
  un unico core in Rust condiviso (regole, crittografia, ABI, firma leggibile) + una shell nativa per ciascuna
  • costruisce la UserOperation e mostra cosa fa
  • chiede alla tua chiave un'asserzione WebAuthn
        │  UserOperation firmata (commissione inclusa)
        ▼
Relay (vela-relay, ospitabile in proprio)
  • quota la commissione, anticipa il gas, invia handleOps
  • non può modificare l'operazione
        ▼
Chain EVM
  EntryPoint v0.7 → il tuo Safe v1.4.1 → modulo 4337 di Safe
  il modulo passkey di Safe verifica P-256 tramite il precompilato EIP-7951 / RIP-7212
```

Servizi di supporto, tutti open source: un **indice delle chiavi pubbliche** che
registra i nuovi wallet in un registro on-chain e risponde alle ricerche, un
archivio di **dati delle chain** e un servizio di **tassi di cambio**. Vedi la
[guida al self-hosting](/it/docs/self-hosting).

### Account

Il tuo wallet è un proxy **Safe v1.4.1** (singleton SafeL2), con il **modulo 4337
v0.3.0** di Safe abilitato come modulo e come fallback handler, operato tramite
l'**EntryPoint v0.7**. I suoi proprietari sono firmatari passkey del **modulo
passkey v0.2.1** di Safe: la prima chiave viene verificata dal firmatario
condiviso, e ogni chiave aggiuntiva dal proprio contratto firmatario creato dalla
factory di Safe. La soglia è **1**.

L'indirizzo è **deterministico e controfattuale**: viene calcolato con `CREATE2`
dai dati di setup del Safe, che includono ogni chiave fondatrice, prima di
qualsiasi deploy. È lo stesso su ogni rete. Puoi ricevere subito; la tua prima
transazione su ogni rete deploya il wallet e ne paga il costo dentro la
commissione di quella transazione.

### Chiavi

Un wallet ha **da una a sette chiavi**, fissate quando lo crei. Una qualsiasi di
esse può firmare da sola (1-of-n). Una chiave può essere:

- una passkey sul dispositivo che stai usando — sincronizzata dal Portachiavi
  iCloud, dal Gestore delle password di Google o da un altro gestore di password,
  se lo consenti;
- un altro telefono, collegato scansionando un codice QR (il trasporto ibrido di
  WebAuthn);
- una chiave di sicurezza hardware via USB o NFC, che non si sincronizza da
  nessuna parte.

Ogni firma richiede la verifica dell'utente da parte dell'autenticatore stesso —
un dato biometrico o il PIN del dispositivo, oppure il PIN e il tocco di una chiave
di sicurezza. Non esiste una chiave di sessione. Le chiavi non si possono
aggiungere, rimuovere o sostituire in seguito: su ogni chain dove il wallet non è
ancora deployato, l'indirizzo corrisponde ancora all'insieme fondatore, quindi
cambiare i proprietari su una chain renderebbe l'account diverso da una chain
all'altra.

Le passkey appartengono a una relying party — quelle di Vela sono create per
**`getvela.app`**. I browser le offrono solo alle pagine di getvela.app o dei suoi
sottodomini, ed è ciò che le rende resistenti al phishing; è anche una
dipendenza su cui questo documento torna più avanti.

### Flusso di firma

1. **Costruire** una UserOperation per il tuo Safe — compreso un trasferimento che
   paga il relay — e simularla.
2. **Decodificarla** in un intento leggibile e mostrartela.
3. **Firmare**: dopo averti verificato, il tuo autenticatore produce
   un'asserzione WebAuthn sull'hash dell'operazione.
4. **Codificare** l'asserzione come la firma Safe che il modulo passkey si
   aspetta.
5. **Inviare** l'operazione firmata al relay, che chiama l'EntryPoint.
6. **Verificare on-chain**: il modulo passkey controlla la firma P-256 con il
   precompilato EIP-7951 / RIP-7212 prima che il Safe esegua qualsiasi cosa. Non c'è un
   verificatore di riserva; una rete senza il precompilato non si può aggiungere.

### Commissioni

- Il relay viene pagato **in banda**: l'operazione dichiara commissioni
  EntryPoint pari a zero e include un trasferimento dal tuo Safe all'indirizzo del
  relay. Importo e destinatario fanno parte di ciò che firmi, quindi paghi
  esattamente quanto mostrato nella schermata di conferma.
- La commissione è **il triplo del gas che il wallet riserva per l'operazione**
  (le stime simulate aumentate della metà, con dei minimi), **al prezzo più alto
  tra la lettura del prezzo del gas fatta dal wallet e il prezzo del relay per la
  velocità scelta**, con un minimo di circa 0,01 dollari. Su Tempo il
  moltiplicatore è due. Il margine sulla riserva e sul prezzo rende la commissione
  più alta del costo reale on-chain dell'operazione, e ancora di più per la prima
  transazione su una rete; il relay tiene la differenza. L'importo esatto è nella
  schermata di conferma prima che tu firmi.
- La commissione va al relay impostato nel wallet: quello di Vela per impostazione
  predefinita, oppure qualsiasi istanza di vela-relay, compresa una che gestisci tu.
- La commissione si paga nella moneta della rete o in una stablecoin in dollari
  accettata dal relay (pathUSD su Tempo, che non ha moneta nativa). **Non c'è
  alcun paymaster**: nessuno sponsorizza il gas, e nessuno può filtrare le
  transazioni con una politica di sponsorizzazione.
- Se la tesoreria di gas di un relay su una rete è vuota, il wallet te lo dice
  prima che tu firmi. Non esiste un deposito per utente.

Dettagli: [reti e commissioni](/it/docs/networks-and-fees).

### Firma leggibile

Le chiamate e i messaggi EIP-712 vengono decodificati con i descrittori
**ERC-7730** — integrati nell'app per i contratti più comuni, scaricati dal
servizio dei dati delle chain o abbinati a forme standard dei token — e poi, come
ultima risorsa, con un database pubblico di selettori, etichettato come best
effort. Tutto il resto riceve un avviso esplicito di firma alla cieca. Un
descrittore scaricato non viene mai etichettato come verificato: quella parola se
la guadagna solo uno integrato nell'app, o uno scaricato identico a quello.
Un'approvazione on-chain di livello «illimitato» (2^200 o più) non si può inviare
finché non la riduci; un'approvazione finita ma elevata e i permit firmati vengono
mostrati con un avviso, ma non bloccati. Dettagli:
[firma leggibile](/it/docs/clear-signing).

### Reti

Vela integra 24 reti — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable,
Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume e XRPL EVM — e
accetta qualsiasi rete EVM che abbia i dodici contratti che controlla e il
precompilato EIP-7951 / RIP-7212. Due dei dodici sono la factory dei firmatari
passkey di Safe e il codice del firmatario che distribuisce, che servono solo a un
wallet con più di una chiave; il controllo li riporta separatamente.

## Modello di sicurezza

**Cosa Vela non può fare**

- Spostare, spendere o congelare i tuoi fondi per conto proprio — solo le tue chiavi
  autorizzano il tuo Safe, e Vela non vi ha alcun ruolo. (Ciò che Vela può fare è
  distribuire software che ti chiede di firmare; vedi le minacce qui sotto.)
- Modificare una transazione dopo che l'hai firmata — qualsiasi modifica invalida
  la firma.
- Leggere le tue chiavi private — restano nei tuoi autenticatori.
- Aggiungere una chiave al tuo wallet, o rimuoverne una.

**Cosa «non può congelare» non copre: il token.** USDC, USDT e la maggior parte dei
token garantiti da valuta fiat permettono al loro emittente di mettere in
blacklist qualsiasi indirizzo, compreso il tuo. Quel potere appartiene
all'emittente ed esiste qualunque wallet tu usi. Ciò che l'autocustodia ti dà è
che Vela non è una seconda parte in grado di farlo.

**Di cosa ti fidi**

- Dei **contratti**: Safe, i suoi moduli 4337 e passkey, l'EntryPoint v0.7 e il
  precompilato EIP-7951 / RIP-7212 della chain.
- Del **dominio**: qualsiasi pagina servita da getvela.app o da uno dei suoi
  sottodomini può chiedere una firma alle tue chiavi.
- Degli **autenticatori** che custodiscono le tue chiavi e — per le passkey
  sincronizzate — dell'account Apple, Google o del gestore di password che c'è
  dietro.
- **Del codice dell'app con cui firmi.** Costruisce la transazione e ti mostra
  cosa fa. Un'app compromessa può mostrarti una cosa e chiederti di firmarne
  un'altra; la richiesta dell'autenticatore non ti farà vedere la differenza.
- Degli **endpoint RPC** da cui leggi: un nodo che mente può mostrare saldi
  sbagliati o un'anteprima di simulazione sbagliata. Puoi impostare i tuoi.
- Dei **servizi di dati delle chain e di tassi di cambio**: forniscono elenchi di
  token, descrittori, l'elenco dei token per le commissioni e i tassi usati per
  convertire un importo in valuta fiat in un importo in token.
- Del **relay**: non può cambiare ciò che hai firmato, ma può ritardarlo o
  rifiutarlo, scegliere quando finisce on-chain (quindi potrebbe fare front-running
  su uno swap entro il tuo slippage) e fissare il prezzo del gas su cui si basa la
  tua commissione, fino al triplo della lettura del wallet.

**Minacce considerate**

- **Dispositivo perso o rubato** — un ladro deve comunque superare il controllo
  dell'autenticatore; un'altra chiave ti restituisce l'accesso. Ma una chiave non
  si può rimuovere: se una potrebbe essere nelle mani di qualcun altro, sposta i
  fondi in un nuovo wallet, perché il vecchio indirizzo resta spendibile da quella
  chiave su ogni rete.
- **Phishing** — una passkey non si può digitare in un sito falso, e i browser la
  offrono solo alle pagine di getvela.app e dei suoi sottodomini.
- **dApp malevola** — affrontata dalla firma leggibile, dal controllo sulle
  approvazioni e da un rifiuto: una richiesta di chiamata dal tuo Safe verso se
  stesso — `enableModule`, `addOwnerWithThreshold`, `swapOwner`,
  `setFallbackHandler`, `setGuard` e il resto di quella famiglia — viene bloccata,
  anche dentro un batch o un `MultiSend`, e così pure qualsiasi ramo che porti un
  `delegatecall` e una firma di dati tipizzati `SafeTx`. Una qualsiasi di queste,
  firmata una sola volta, consegnerebbe l'account in modo completo quanto il
  payload di Bybit, perciò il wallet non te le propone proprio da firmare.
- **Servizio di backend compromesso** (relay, indice, dati delle chain, tassi di
  cambio) — nessun potere di firma, ma un'influenza reale: rifiuto del servizio,
  descrittori o elenchi di token fuorvianti, tassi di cambio sbagliati che cambiano
  quanto invia un importo in valuta fiat e (per il relay) i tempi e il prezzo del
  gas visti sopra. Una descrizione scaricata dal servizio dei dati delle chain non
  viene mai definita verificata: quella parola se la guadagna solo una descrizione
  integrata nell'app, o una scaricata identica a quella, e le altre vengono
  mostrate con una riga che dice che nulla le ha autenticate. Ogni servizio si può
  sostituire.
- **Distribuzione dell'app compromessa** — un deploy web, un aggiornamento
  dell'estensione o una build dell'app manomessi potrebbero presentarti una
  transazione malevola da firmare. È la classe di attacchi di
  [Bybit](/it/docs/bybit-attack). Le mitigazioni oggi sono limitate: la decodifica e
  il controllo sulle approvazioni nell'app stessa, le build macOS notarizzate, e la
  possibilità di compilare da te l'estensione o le app dal codice sorgente (i
  pacchetti di release hanno checksum SHA-256 e attestazioni di provenienza della
  build GitHub che nominano il commit e l'esecuzione del workflow; il programma di
  installazione per Windows continua a non avere una firma del codice). Una pagina
  di firma indipendente che non condivide il codice dell'app è pronta ma non ancora
  collegata.
- **Qualsiasi cosa servita dal dominio** — qualsiasi pagina su getvela.app o sui
  suoi sottodomini, compreso uno script che carica, potrebbe chiedere firme alle
  passkey di Vela, e la richiesta mostra solo «getvela.app». Per questo il sito
  vieta alle proprie pagine di usare le passkey, e tiene il suo script di analytics
  lontano dalla pagina che custodisce una chiave. Se il dominio cambiasse
  proprietario, il nuovo proprietario controllerebbe anche quali app possono usare
  le passkey. L'estensione e le app compilate da te portano con sé il proprio
  codice, anche se per impostazione predefinita scaricano comunque i descrittori e
  usano i servizi sotto getvela.app.

## Recupero

Creare un wallet pubblica le sue chiavi pubbliche e il suo indirizzo in un
**contratto di registro** pubblico su Gnosis (copiabile su Ethereum). Su un nuovo
dispositivo accedi con **una qualsiasi** chiave; l'app trova il wallet tramite
l'indice o, se non risponde, direttamente dal registro, e controlla che le chiavi
producano di nuovo l'indirizzo registrato. Un wallet con una sola chiave si può
anche ricostruire da due firme, senza alcun registro.

<Callout type="warning" title="Le tue chiavi sono il tuo recupero">
Non c'è seed phrase, né recupero social, né guardian — niente che Vela possa
perdere o divulgare, né che qualcuno possa costringere Vela a usare. Se tutte le chiavi fondatrici vanno
perse, il wallet non si può recuperare. Crea il wallet con più di una chiave, tieni
attiva la sincronizzazione delle passkey se ci fai affidamento e proteggi
l'account che c'è dietro.
</Callout>

Dettagli: [recupero e accesso](/it/docs/recovery).

## Se Vela sparisce

I tuoi fondi restano nel tuo Safe, on-chain. I contratti non dipendono da Vela, e
ogni servizio che Vela gestisce è open source perché qualcun altro possa gestirlo.
L'unica cosa che non si può spostare è la relying party delle
passkey, `getvela.app`: una copia del wallet web su un altro dominio crea un
wallet diverso. Per i wallet esistenti, l'estensione Vela per il browser (che può
usare le passkey di `getvela.app` grazie a un permesso) e le app che compili tu
(con un telefono o una chiave di sicurezza) continuano a funzionare senza
getvela.app. La
[guida al self-hosting](/it/docs/self-hosting#if-getvela-app-disappears) descrive
ogni strada e i suoi limiti. Un accesso indipendente a una chain richiede anche
che quella chain supporti EIP-7951 / RIP-7212.

## Privacy

Nessun account, nessuna email, nessun KYC. Ciò che diventa pubblico viene scritto
nel registro quando crei un wallet: la chiave pubblica e l'ID della credenziale di
ogni chiave, il modello di autenticatore, il nome del wallet e le etichette delle
chiavi, l'indirizzo e i dati di registrazione firmati. L'indice di Vela vede quel
record prima di inviarlo, e gli indirizzi di cui cerchi il nome; il relay di Vela
vede il tuo indirizzo, le operazioni che invii e l'endpoint RPC usato dalla tua app
(compresa un'eventuale chiave API nel suo URL), e conserva le operazioni per un
tempo limitato per ritentarle e diagnosticarle. Ogni servizio vede il tuo
indirizzo IP. Il sito web usa analytics senza cookie.
L'[informativa sulla privacy](/privacy) è l'elenco che fa fede.

## Open source

Tutto ha licenza MIT: il wallet (tutte le app e il core), il relay, l'indice delle
chiavi pubbliche, il servizio di tassi di cambio e l'archivio dei dati delle chain.
Codice:
[github.com/mondaylabsltd](https://github.com/orgs/mondaylabsltd/repositories).

## Nessun token

Vela non ha un token e non prevede di averne uno. Non c'è niente da comprare, da
farmare o su cui speculare. Le commissioni si pagano nella moneta di ogni rete o in
una stablecoin.

## Stato degli audit e limiti

I contratti di Safe, i suoi moduli 4337 e passkey e l'EntryPoint v0.7 hanno avuto
audit indipendenti e sono molto usati. **Il codice di Vela — le app, i servizi di
backend e il contratto di registro — non ha avuto un audit indipendente di terze
parti, e non ce n'è nessuno in programma**; un audit professionale è un obiettivo
per quando il progetto potrà finanziarlo, non un impegno con una data. Fino ad
allora la revisione è informale: il codice è aperto, membri capaci della community
lo leggono, e viene revisionato con strumenti di IA. Questo aiuta, ma non equivale
a un audit professionale. Considera Vela un software in alpha. Dettagli:
[audit e problemi noti](/it/docs/security-audits).

## Riferimenti

- ERC-4337 — Account abstraction tramite l'EntryPoint
- EIP-1271 — Validazione delle firme per i contratti
- ERC-7730 — Descrittori per la firma leggibile
- EIP-5792 — Batch di chiamate del wallet (`wallet_sendCalls`)
- EIP-7951 / RIP-7212 — Precompilato per la verifica delle firme P-256
- WebAuthn / FIDO2 — Passkey
- [Safe smart account v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
