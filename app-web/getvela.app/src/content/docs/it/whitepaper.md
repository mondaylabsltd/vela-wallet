---
title: Whitepaper
description: Come funziona Vela e cosa devi — e cosa non devi — dare per buono per usarlo. Architettura, modello di sicurezza, recupero e come verificare tutto da solo.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Stato: alpha · v0.1">
Questa pagina descrive come funziona Vela oggi e cosa devi o non devi dare per
buono per usarlo. Preferisce l'onestà al marketing. Vela è in
<a href="/blog/vela-is-in-alpha">alpha</a>: parti con importi piccoli. Vela non ha
token. Tutto ciò che c'è qui è verificabile sul codice open source.
</Callout>

## Sintesi

Vela è un **wallet a smart contract auto-custodito** per le reti EVM. Ogni wallet è
un account intelligente [Safe](https://github.com/safe-fndn/safe-smart-account)
controllato da una **passkey**: una credenziale WebAuthn (P-256) custodita dal
sistema operativo del dispositivo, cifrata end-to-end e sbloccata con Face ID,
Touch ID o l'impronta. Non ci sono frasi seed né chiavi private da copiare,
conservare o perdere.

Vela, l'azienda, non detiene mai le tue chiavi né i tuoi fondi e **non può
spostarli, congelarli o sequestrarli**. L'app, il relay delle transazioni e i
servizi di supporto sono tutti open source e ospitabili in proprio. Ciò di cui devi
fidarti si riduce a smart contract auditati, alla cassaforte di passkey del tuo
sistema operativo e — solo per la disponibilità — a un relay che puoi sostituire o
far girare tu.

## Perché Vela esiste

La maggior parte dei wallet impone un compromesso:

- **I wallet con frase seed** mettono davanti a ogni utente un segreto di 12–24
  parole. È il punto singolo di rottura e un bersaglio costante del phishing.
- **I wallet custoditi** tolgono la frase seed ma prendono in custodia i tuoi
  fondi, reintroducendo il rischio di controparte che le criptovalute dovevano
  eliminare.
- **La firma al buio** — approvare esadecimale illeggibile — si è normalizzata in
  tutto l'ecosistema ed è dietro a una bella fetta dei wallet svuotati.

Vela punta a essere semplice come un'app custodita restando completamente
auto-custodito: niente frase seed, niente custodia da parte di terzi e nessuna
transazione che non puoi leggere prima di firmarla.

## Principi di progettazione

1. **Auto-custodia, senza eccezioni.** Le chiavi nascono sul tuo dispositivo e sono
   custodite dal servizio passkey del sistema, cifrate end-to-end. I server di Vela
   vedono solo dati pubblici.
2. **Verifica, non fidarti.** L'intero stack — app e quattro servizi backend — è
   open source con licenza MIT.
3. **Niente firma al buio.** Le transazioni vengono tradotte in intenzione
   leggibile ovunque esista un descrittore; le chiamate sconosciute vengono
   segnalate, non nascoste.
4. **Fare meno.** Il wallet tiene ETH ed ERC-20 e si collega alle dApp che scegli
   tu. Meno codice di cui fidarsi, superficie d'attacco più piccola.

## Architettura

```text
App Vela (iOS / Android / Web, un'unica base di codice)
  • Passkey (WebAuthn P-256, servizio passkey del sistema)
  • Costruzione e firma della UserOperation
  • Interfaccia di firma leggibile (ERC-7730)
        │  UserOperation firmata
        ▼
Relay Vela (ERC-4337, ospitabile in proprio)
  • invia handleOps all'EntryPoint
  • non può alterare né falsificare la tua transazione
        ▼
Chain EVM
  EntryPoint v0.7 → account intelligente Safe
  Il firmatario WebAuthn verifica P-256 on-chain
```

### Modello dell'account

Il tuo wallet è un account intelligente **Safe v1.4.1** (un contratto proxy)
operato tramite l'astrazione dell'account **ERC-4337** (EntryPoint v0.7) con il
**Safe 4337 Module** e un **firmatario WebAuthn** come proprietario dell'account.

L'indirizzo è **deterministico** e **controfattuale**: viene calcolato dalla chiave
pubblica della passkey con `CREATE2` prima che venga inviata qualsiasi transazione,
quindi puoi ricevere fondi prima ancora che sia distribuito. L'account si
distribuisce da solo, pagando dal proprio saldo, alla tua prima transazione.

### Chiavi e autenticazione

L'autenticazione usa **passkey WebAuthn** sulla curva **P-256**. La chiave privata è
generata sul tuo dispositivo e custodita, cifrata end-to-end, dal servizio passkey
del sistema (Portachiavi iCloud o Gestore delle password di Google), che la
sincronizza tra i tuoi dispositivi. **I server di Vela vedono soltanto la tua chiave
pubblica.** Ogni firma richiede una nuova verifica biometrica: non esiste una chiave
di sessione a lunga vita. Tutti i dettagli in
[come funzionano le passkey](/it/docs/passkeys).

### Firma e flusso della transazione

1. **Costruire** una `UserOperation` ERC-4337 per il tuo Safe e stimare il gas.
2. **Decodificare** la chiamata in intenzione leggibile e mostrarla per la revisione.
3. **Firmare** — dopo la verifica biometrica il dispositivo produce un'asserzione
   WebAuthn sull'hash dell'operazione.
4. **Codificare** l'asserzione come firma di contratto **EIP-1271**.
5. **Inoltrare** l'operazione firmata al relay, che la sottopone all'EntryPoint.
6. **Verificare on-chain** — il Safe verifica la firma P-256 on-chain tramite il
   precompilato RIP-7212 prima di eseguire. Il precompilato è un requisito rigido:
   non c'è un verificatore di riserva, e Vela rifiuta di abilitare una rete che non
   lo abbia.

Il relay riceve un'operazione **già firmata**. Non può cambiare destinatario,
importo o qualsiasi altro campo senza invalidare la firma.

### Relay e modello del gas

- Il gas si paga **dal saldo del tuo wallet** — nel token nativo della rete per
  impostazione predefinita, o in una stablecoin supportata dove il relay ne offre
  una. Tempo, che non ha moneta nativa, regola sempre il gas in stablecoin USD. Non
  c'è **alcun paymaster** né terze parti che sponsorizzino — o condizionino — le
  tue transazioni.
- **Il relay è l'unica fonte di verità per il prezzo del gas.** Lo quota dalle
  condizioni reali della chain; il wallet mostra quella quotazione e firma
  esattamente ciò che mostra.
- La tariffa del relay di Vela è volutamente semplice: il totale è il **costo di
  rete più la commissione di servizio del relay**, con un piccolo minimo sulle
  transazioni molto economiche. Una parte va ai validatori della chain; il resto
  paga il relay che gestisce l'infrastruttura e tiene rifornito il tuo account di
  gas.
- Il wallet **mostra la commissione stimata prima che tu confermi** — nell'asset
  delle commissioni e nella tua valuta di visualizzazione — e l'importo quotato con
  il suo destinatario fa parte di ciò che firmi, quindi il relay riceve esattamente
  quanto mostrato. Nessun ricarico nascosto.
- Ogni Safe ha un **account di relay dedicato** (account di gas) per chain, attivato
  da un deposito **non rimborsabile**. Può esaurirsi col tempo e richiedere una
  **riattivazione** più avanti: non è propriamente un deposito una tantum.

Il relay è una dipendenza di **disponibilità**, non di **custodia**: può ritardare o
rifiutare, ma non può mai alterare, falsificare o rubare. È open source e puoi farne
girare uno tuo — e poiché il prezzo viene **quotato e mostrato** invece che
nascosto, anche la commissione di un relay auto-ospitato o di terze parti ti è
sempre visibile prima di firmare. Vedi
[reti e commissioni](/it/docs/networks-and-fees).

### Firma leggibile (ERC-7730)

Vela decodifica calldata e dati tipizzati EIP-712 con i descrittori **ERC-7730** e
mostra l'**intenzione** (Scambia, Invia, Approva…), la **sostanza** (importi,
indirizzi) e, su richiesta, i **dettagli** (nonce, scadenza, calldata grezza),
colorati per rischio. Quando nessun descrittore corrisponde, Vela mostra un avviso
esplicito di firma al buio invece di fingere di aver capito la chiamata.

### Reti

Vela supporta 12 reti EVM — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base,
Avalanche, Gnosis, Unichain, Tempo, Monad e World Chain — più le reti
personalizzate. Una rete personalizzata si può aggiungere solo se ospita già i
contratti su cui Vela si appoggia (l'EntryPoint, i contratti Safe, il firmatario
WebAuthn) e il precompilato P-256 RIP-7212; Vela lo verifica prima di abilitarla.

## Modello di sicurezza

**Cosa Vela non può fare:**

- Spostare, spendere o trasferire i tuoi fondi: solo la tua passkey può autorizzare
  il Safe.
- Congelare o sequestrare il tuo account: il Safe è il tuo contratto on-chain; Vela
  non vi ha alcun ruolo privilegiato.
- Firmare al posto tuo: ogni transazione richiede una nuova asserzione biometrica.
- Vedere la tua chiave privata: non arriva mai a Vela; solo il tuo dispositivo può
  usarla per firmare.
- Modificare una transazione dopo la firma: qualsiasi modifica la invalida.

**Cosa «non può congelare» non copre: il *token*.** Una stablecoin con permessi —
USDC, USDT e la maggior parte dei token garantiti da valuta — porta con sé una
funzione di blacklist che il suo emittente può invocare contro qualsiasi indirizzo,
compreso il tuo. Quel potere appartiene all'emittente ed esiste in qualunque wallet
tu tenga il token; nessun wallet auto-custodito, Vela incluso, può toglierlo. Ciò
che l'auto-custodia ti dà è che **noi** non siamo una seconda parte che può farlo.

**Di cosa ti fidi:**

- Dei **contratti Safe** (auditati, molto usati) e del firmatario WebAuthn che
  verifica la tua chiave P-256.
- Del **servizio passkey del tuo sistema** (Apple / Google), perché protegga e
  sincronizzi la tua credenziale.
- Dei **provider RPC** che interroghi (Vela usa un pool multi-sorgente con failover;
  puoi impostare i tuoi).
- Del **relay**, solo per la disponibilità — e puoi ospitarlo tu.

**Minacce considerate:**

- **Dispositivo perso o rubato** — un ladro ha comunque bisogno della tua biometria
  o del tuo PIN per firmare.
- **Phishing / dApp malevola** — affrontati dalla firma leggibile.
- **Server Vela compromesso** — non dà alcuna capacità di firma; il raggio del danno
  è un servizio degradato, non la perdita dei fondi.
- **Rischio di catena di fornitura** — mitigato da open source e auto-hosting.

## Recupero

La tua passkey è salvata dal servizio del sistema operativo; su un nuovo dispositivo
accedere con lo stesso account Apple o Google la ripristina, e il wallet ricompare.

<Callout type="warning" title="Il backup delle passkey della piattaforma è il tuo recupero">
Il recupero di Vela è la tua passkey, sincronizzata da Portachiavi iCloud o dal
Gestore delle password di Google. Per progettazione non ci sono frase seed, recupero
sociale o guardiani: nulla che Vela possa perdere, far trapelare o essere costretta
a usare. Il rovescio è reale: se perdi <strong>sia</strong> il dispositivo
<strong>sia</strong> la passkey sincronizzata nel cloud, senza altre copie,
l'account non è recuperabile. Tieni attivo il backup delle passkey della tua
piattaforma e metti in sicurezza quell'account.
</Callout>

Il modello di recupero completo, limiti onesti compresi, è in
[recupero e accesso](/it/docs/recovery).

## Se Vela sparisce

Auto-custodia significa che le tue chiavi e i tuoi fondi non dipendono dal fatto che
Vela sia online. I fondi vivono nel **tuo contratto Safe on-chain**, e il relay è
open source e sostituibile.

Un'avvertenza onesta: WebAuthn lega una passkey a un dominio di relying party
(`getvela.app`). Se quel dominio andasse perso in modo definitivo, le passkey legate
a esso avrebbero bisogno di aiuto per funzionare altrove: uno strumento capace di
presentare all'autenticatore la relying party originale. Vela distribuiva un tempo
un'estensione per browser di livello sviluppatore per questo caso e l'ha ritirata a
settembre 2026; un percorso di recupero per la perdita del dominio adatto al grande
pubblico resta lavoro aperto, e lo diciamo invece di lasciar intendere che esista.
L'accesso on-chain indipendente dipende anche dal supporto P-256 (RIP-7212) della
chain di destinazione, che sta migliorando su più chain.

## Privacy

Nessun account, nessuna e-mail, nessun KYC, nessuna frase seed da raccogliere. I
server conservano solo la tua **chiave pubblica** e un nome dell'account scelto da
te (per il recupero multi-dispositivo), pubblicati on-chain per progetto. I contenuti
delle transazioni non vengono registrati. Il sito usa analytics auto-ospitate e senza
cookie. Vedi la [privacy policy](/privacy).

## Verificabilità e open source

Tutto è **con licenza MIT e open source** — l'app e i quattro servizi backend (dati
di chain, indice passkey, relay, tassi di cambio), che puoi **ospitare tu**
(Impostazioni → Avanzate → Endpoint dei servizi). Leggi il codice su
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet).

## Nessun token

Vela **non ha token** e non ne prevede. Non c'è niente da comprare, da farmare o su
cui speculare. Il gas si paga nell'asset nativo di ogni rete.

## Stato degli audit e limiti

I **contratti Safe** al cuore di ogni account Vela sono auditati in modo
indipendente e collaudati sul campo. L'**integrazione di Vela** attorno a essi
**non ha avuto un audit indipendente di terze parti**, e al momento non ne è
programmato nessuno: un audit professionale è un obiettivo per quando il progetto
potrà finanziarlo, non un impegno con una data. Fino ad allora la revisione è
informale: il codice è open source e si appoggia a membri della comunità capaci e
interessati che lo leggono, oltre che a revisioni assistite dall'IA. Aiuta, ma non
equivale a un audit professionale. Tratta Vela come software in alpha e usa importi
che ti senti di mettere in qualcosa di così giovane.

## Riferimenti

- ERC-4337 — astrazione dell'account tramite EntryPoint
- EIP-1271 — standard di validazione delle firme per i contratti
- ERC-7730 — firma leggibile / descrittori di dati strutturati
- EIP-5792 — batching delle chiamate del wallet
- RIP-7212 — precompilato per la verifica delle firme secp256r1 (P-256)
- WebAuthn / FIDO2 — autenticazione con passkey
- [Account intelligente Safe v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
