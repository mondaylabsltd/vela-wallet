---
title: Inviare e ricevere
description: "Come ricevere e inviare con Vela — un solo indirizzo su ogni rete, invii a una o a più persone, come vengono trovati i nomi dei destinatari, cosa confermi e come il relay sposta i tuoi fondi."
source: 9e280dfc853b
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Inviare e ricevere

## Ricevere

1. Apri il wallet e tocca **Ricevi**.
2. Condividi il tuo indirizzo: copialo o mostra il codice QR. Il wallet web può
   anche creare una richiesta di pagamento che include un importo.
3. Quando il trasferimento è confermato on-chain, compare nel tuo saldo.

- Il tuo indirizzo è **lo stesso su ogni rete**, quindi ne dai uno solo — ma chi
  invia deve comunque usare una rete supportata da Vela, o una che hai aggiunto tu.
- Puoi **ricevere prima che il wallet sia deployato** su una rete. Il deploy
  avviene da solo al tuo primo invio da lì.

## Inviare

1. Tocca **Invia** e scegli il **token**.
2. Inserisci l'**importo** (nel token o nella tua valuta di visualizzazione) e
   l'**indirizzo del destinatario**, incollandolo, scansionando un codice QR o
   scegliendo un contatto.
3. **Controlla.** Vela mostra cosa succederà, la commissione e il nome che ha
   trovato per il destinatario, se c'è.
4. **Conferma** con una delle tue chiavi — Face ID, impronta, PIN, oppure un tocco
   e il PIN sulla tua chiave di sicurezza.

### Inviare a più persone, o raccogliere

- **Dividi** — invia un token a più persone con un'unica transazione. Puoi
  incollare un elenco o importare un foglio di calcolo, e inserire gli importi
  nella tua valuta.
- **Raccogli** — invia più token a un solo indirizzo con un'unica transazione.

In entrambi i casi firmi una volta sola, e la transazione paga una sola
commissione.

### Nomi per gli indirizzi

Quando inserisci un indirizzo, Vela cerca un nome da associargli: prima nel
proprio registro (il nome di un altro wallet Vela), poi nei record inversi `.bnb`,
`.arb`, `.g`, Basename ed ENS, letti direttamente da ciascuna chain. Funziona in
una sola direzione: dà un nome a un indirizzo che hai inserito. Se scrivi un nome
come `alice.eth`, Vela non cerca l'indirizzo corrispondente. Anche i tuoi
**contatti** salvati mostrano il loro nome.

Un nome che viene da quei record inversi si mostra solo se **risolve in avanti
verso lo stesso indirizzo**. Chiunque può impostare il proprio record inverso su
una stringa qualsiasi, quindi il record da solo non dimostra nulla; il wallet
chiede al servizio dei nomi a quale indirizzo punta quel nome, e mostra il nome
solo quando i due coincidono. Se il controllo non si può fare — un endpoint che
non risponde, un resolver che fallisce — vedi l'indirizzo e nessun nome, mai uno
non controllato.

### Moneta della commissione e velocità

La schermata di conferma mostra la commissione nella moneta con cui la paghi e
nella tua valuta. Puoi pagare nella moneta della rete oppure, dove il relay lo
accetta, in una stablecoin in dollari, e scegliere una velocità (predefinita:
rapida). Quando invii il **massimo** di una moneta nativa, Vela tiene da parte
quanto basta per la commissione.
[Come si calcola la commissione](/it/docs/networks-and-fees).

### Cosa succede quando confermi

1. Vela costruisce una **UserOperation** ERC-4337 per il tuo Safe, che include il
   pagamento della commissione al relay.
2. La tua chiave, dopo averti verificato, la firma con un'asserzione **WebAuthn
   (P-256)**.
3. L'operazione firmata va al **relay**, che la invia all'EntryPoint; il tuo Safe
   controlla on-chain la firma P-256 ed esegue.

<Callout type="info" title="Il relay non può cambiare la tua transazione">
Il relay riceve un'operazione già firmata. Non può cambiare il destinatario,
l'importo o la commissione: qualsiasi modifica invalida la tua firma. Può
ritardarla o rifiutarla, e decide quando finisce on-chain. È open source, e puoi
[gestirne uno tuo](/it/docs/self-hosting#relay).
</Callout>

Prima che tu firmi, Vela decodifica cosa fa la transazione e ti avvisa di ciò che
non riesce a decodificare; vedi [firma leggibile](/it/docs/clear-signing).

## Prima di premere Invia

- **Controlla l'inizio e la fine dell'indirizzo.** I malware che sostituiscono gli
  indirizzi esistono davvero, e lo stesso vale per gli indirizzi somiglianti
  infilati nella tua cronologia.
- **Conferma la rete.** Inviare sulla rete sbagliata è un errore comune, e costoso.
- **Con un destinatario nuovo, parti in piccolo.** Un trasferimento di prova
  minuscolo è un'assicurazione che costa poco.

Le transazioni sono irreversibili. Nessuno può recuperare un invio all'indirizzo
sbagliato: è la natura dell'autocustodia.

## La tua attività

La tua attività unisce ciò che hai inviato da questo dispositivo ai trasferimenti
di token letti dai log di ciascuna chain. Un semplice trasferimento di moneta
nativa che ti arriva tramite un altro contratto (per esempio alcuni prelievi da
exchange) su alcune reti può non generare un log, quindi può comparire nel saldo
senza comparire nell'attività. I saldi vengono letti in tempo reale tramite un
pool di endpoint RPC con failover automatico; una rotellina significa «sto ancora
leggendo», non «i fondi sono spariti».

Poi: [reti e commissioni](/it/docs/networks-and-fees).
