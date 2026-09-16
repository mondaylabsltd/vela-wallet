---
title: Perché abbiamo creato Vela
description: La versione lunga — dove dovresti tenere dodici parole, cosa hanno cambiato le passkey, cosa non potevamo accettare nei wallet che già usavamo, e il compromesso che abbiamo scelto al suo posto.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Perché abbiamo creato Vela

Non volevamo fare l'ennesimo wallet. Siamo partiti da una domanda a cui non siamo
mai riusciti a rispondere in modo pulito:

> Dove dovresti tenere dodici parole?

## La risposta onesta è uno screenshot

Mettile nelle Note e sei a un telefono rubato dai guai. Scrivile su carta, e adesso
stai pensando a incendi, acqua, traslochi, coinquilini, sacchi della spazzatura, e
se il te del futuro si ricorderà dov'era «il posto sicuro».

Per molti la risposta onesta è uno screenshot nel rullino. Tutti sanno che è
sbagliato. Lo fanno lo stesso — perché la risposta «giusta» è troppo pesante da
sostenere nella vita di tutti i giorni.

Una frase seed è un segreto che deve sopravvivere a decenni di vita ordinaria senza
mai essere copiato, fotografato, digitato nella casella sbagliata o letto ad alta
voce a qualcuno di gentile al telefono. Non è un problema difficile per una persona
attentissima. È un problema difficile per una persona.

## Poi le passkey hanno cambiato cosa può essere un wallet

Usavamo [Base Account](https://account.base.app) tutti i giorni, e firmare con Face
ID sembrava ovvio in un modo in cui le frasi seed non lo erano mai state: meno
maneggiare materiale pericoloso, più usare il resto di internet.

Ma più lo usavamo, più incontravamo bordi che non si potevano ignorare:

- una **chiave di recupero generata nel browser** di cui ti dovevi semplicemente
  fidare,
- **niente reti personalizzate**,
- **nessun modo di ospitarlo da soli**,
- e il problema silenzioso che era il più grande: **se il servizio sparisce, il
  wallet sparisce con lui.**

Così abbiamo costruito la versione da cui volevamo dipendere.

## Cos'è davvero Vela

Vela è **un wallet a passkey che puoi possedere per intero.**

La tua passkey resta dove il dispositivo la protegge già — Portachiavi iCloud,
Gestore delle password di Google, o una chiave di sicurezza hardware che tieni tu.
Quando firmi una transazione, Vela manda una sfida al dispositivo; il dispositivo
la firma e restituisce solo la firma. Vela non vede mai la chiave.

La maggior parte dei wallet ha ancora un momento pericoloso, anche se breve: parole
su uno schermo, una frase seed in memoria, una chiave di recupero appoggiata in una
scheda del browser. Vela è progettato perché quel momento non esista.

<Callout type="info" title="Non una promessa — un'architettura">
Non possiamo accedere alle tue chiavi. Non «promettiamo di non farlo»: in Vela non
esiste un percorso di codice che possa farlo. Il wallet è un
<a href="/it/docs/security-audits">account intelligente Safe</a> azionato da una
firma che il tuo dispositivo produce e che noi ci limitiamo a ricevere.
</Callout>

Abbiamo reso Vela **open source** perché tu possa verificarlo da solo, e
**ospitabile in proprio** perché il tuo wallet non dipenda mai dal fatto che la
nostra azienda resti online. E l'abbiamo costruito su
[contratti Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
non modificati, perché la strada noiosa e collaudata è quella giusta quando ci sono
di mezzo i soldi delle persone: gli stessi contratti che già custodiscono miliardi
on-chain.

## Il compromesso che abbiamo scelto

Un compromesso resta, e seppellirlo sarebbe disonesto.

Con Vela il tuo account Apple o Google conta, perché è lì che vive una passkey
sincronizzata. Perdi quell'account, o cancelli la passkey, e non c'è frase seed,
né reset dell'assistenza, né porta sul retro.

Ma ogni wallet auto-custodito ti chiede di scegliere con quale rischio preferisci
convivere. Una frase seed può essere copiata, fotografata, rubata col phishing o
digitata nel sito sbagliato all'una di notte. Una passkey è diversa: non ci sono
parole da rivelare, nessun segreto da incollare e nessun sito falso capace di
farsela consegnare. Il tuo dispositivo firma per il dominio vero, oppure non firma.

E la scelta non è binaria. Un wallet può essere creato con **fino a sette
firmatari**, ognuno capace di firmare da solo: passkey su dispositivi diversi, un
telefono vicino che scansioni, o una chiave di sicurezza USB/NFC. Se preferisci che
il wallet non dipenda affatto da un account di piattaforma, puoi rendere la primissima
chiave una chiave di sicurezza hardware. L'unica condizione è il momento: il tuo
indirizzo è derivato dall'insieme completo delle chiavi, quindi si scelgono quando
crei il wallet.

<Callout type="warning" title="Cosa non ti compra">
I firmatari in più sono una via di rientro, non un secondo lucchetto. Poiché una
singola chiave basta a firmare, aggiungere una chiave hardware ti protegge dal
<em>perdere</em> l'accesso: non ferma chi ha già preso il controllo di una delle
tue chiavi. È questa la forma onesta dell'1-of-n.
</Callout>

## Ecco perché Vela esiste

Un wallet senza frase seed da nascondere, senza chiave di recupero di cui fidarsi e
senza un'azienda di cui devi sperare che resti in giro per sempre.

Se vuoi verificare le affermazioni invece di prenderle per buone: il
[whitepaper](/it/docs/whitepaper) contiene l'architettura,
[audit e problemi noti](/it/docs/security-audits) elenca ogni contratto da cui
dipendiamo e cosa è stato auditato o no, e tutto il codice è
[su GitHub](https://github.com/mondaylabsltd/vela-wallet).

Poi: [installare Vela](/it/docs/install).
