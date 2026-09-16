---
title: Inviare e ricevere
description: Come ricevere e inviare token in Vela — un solo indirizzo su tutte le reti, transazioni firmate in modo leggibile, e come l'astrazione dell'account sposta davvero i tuoi fondi.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Inviare e ricevere

## Ricevere

1. Apri il wallet e tocca **Ricevi**.
2. Condividi il tuo indirizzo: copialo, oppure fai scansionare il QR code a chi
   invia.
3. Quando il trasferimento è confermato on-chain, il saldo compare nel wallet.

Due cose che vale la pena sapere:

- Il tuo indirizzo è **lo stesso su ogni rete supportata**, quindi condividi un
  solo indirizzo ovunque: assicurati soltanto che chi invia usi la rete giusta.
- Puoi **ricevere prima che il wallet sia distribuito**. Gli account Vela sono
  account intelligenti controfattuali: i fondi possono arrivare al tuo indirizzo
  prima che il contratto esista su una data chain; si distribuisce da solo al tuo
  primo invio lì.

## Inviare

1. Tocca **Invia** e scegli il **token**.
2. Inserisci l'**importo** (puoi passare tra token e valuta di visualizzazione) e
   il **destinatario**. Dove può, Vela risolve i destinatari noti in un nome: un
   account Vela, un nome ENS, un Basename e così via.
3. **Controlla e conferma.** Vela mostra il trasferimento, poi chiede la passkey
   (Face ID / Touch ID / impronta).

### Cosa succede quando confermi

Vela non si limita a «trasmettere» una transazione. Sotto il cofano:

1. Costruisce una **UserOperation** ERC-4337 per il tuo account Safe.
2. Il dispositivo la firma con un'asserzione **WebAuthn (P-256)** dopo il controllo
   biometrico.
3. L'operazione firmata va al **relay**, che la sottopone all'EntryPoint; il tuo
   Safe verifica la firma P-256 **on-chain** ed esegue.

<Callout type="info" title="Il relay non può manomettere la tua transazione">
Il relay riceve una UserOperation <strong>già firmata</strong>. Può ritardare o
rifiutare di inoltrarla, ma non può cambiare destinatario, importo o qualsiasi
altro campo: ogni modifica invalida la tua firma. È un aiuto alla disponibilità,
non un custode, ed è open source, quindi puoi farne girare uno tuo.
</Callout>

### Firma leggibile — niente approvazioni al buio

Prima che tu firmi, Vela decodifica la transazione con i descrittori **ERC-7730** e
mostra l'**intenzione** (Invia, Approva, Scambia…), gli **importi e gli
indirizzi** e un'indicazione di rischio — non esadecimale opaco. Quando non riesce
a decodificare del tutto una chiamata, mostra un **avviso esplicito di firma al
buio** invece di fingere di capirla. Un'approvazione di token illimitata non viene
solo segnalata: Vela la riscrive su un importo finito e rifiuta di inviare
un'approvazione che resterebbe illimitata.

## Prima di premere invia

- **Controlla i primi e gli ultimi caratteri dell'indirizzo.** I malware che
  sostituiscono gli indirizzi esistono davvero.
- **Conferma la rete.** Inviare sulla rete sbagliata è l'errore costoso più comune.
  Vedi [reti e commissioni](/it/docs/networks-and-fees).
- **Con un destinatario nuovo, parti piccolo.** Un trasferimento di prova minuscolo
  è un'assicurazione a basso costo.

Le transazioni sono irreversibili. Non c'è nessun servizio clienti che possa
recuperare un invio all'indirizzo sbagliato: è la natura dell'auto-custodia.

## Leggere la cronologia

Saldi e cronologia vengono letti in tempo reale da un pool di endpoint RPC
pubblici, con failover automatico. Se la rete è lenta, la cronologia può metterci
un momento: una rotellina significa «sto ancora scaricando», non «i fondi sono
spariti».
