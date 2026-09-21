---
title: Perché abbiamo creato Vela
description: "La versione lunga — dove dovresti tenere dodici parole, cosa hanno cambiato le passkey, cosa non potevamo accettare nei wallet che già usavamo e il compromesso che abbiamo scelto al loro posto."
source: 06307425f631
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Perché abbiamo creato Vela

Non volevamo fare l'ennesimo wallet. Siamo partiti da una domanda a cui non siamo
mai riusciti a rispondere in modo pulito:

> Dove dovresti tenere dodici parole?

## La risposta onesta è uno screenshot

Mettile nelle Note e basta un telefono rubato per finire nei guai. Scrivile su
carta, e adesso stai pensando a incendi, acqua, traslochi, coinquilini, sacchi
della spazzatura, e a se il te del futuro si ricorderà dov'era «il posto sicuro».

Per molti la risposta onesta è uno screenshot nel rullino. Tutti sanno che è
sbagliato. Lo fanno lo stesso — perché la risposta «giusta» è troppo pesante da
sostenere nella vita di tutti i giorni.

Una seed phrase è un segreto che deve sopravvivere a decenni di vita ordinaria
senza mai essere copiato, fotografato, digitato nella casella sbagliata o letto ad
alta voce a qualcuno molto disponibile al telefono. Non è un problema difficile
per una persona attentissima. È un problema difficile per una persona.

## Poi le passkey hanno cambiato l'esperienza di un wallet

Usavamo [Base Account](https://account.base.app) tutti i giorni, e firmare con
Face ID sembrava ovvio in un modo in cui le seed phrase non lo erano mai state:
meno come maneggiare materiale pericoloso, più come usare il resto di internet.

Ma più lo usavamo, più incontravamo limiti che non si potevano ignorare:

- una **chiave di recupero generata nel browser** di cui dovevi semplicemente
  fidarti,
- **niente reti personalizzate**,
- **nessun modo di ospitarlo in proprio**,
- e il problema silenzioso, il più grande di tutti: **se il servizio spariva, il
  wallet spariva con lui.**

Così abbiamo costruito la versione da cui volevamo dipendere.

## Cos'è davvero Vela

Vela è **un wallet a passkey che puoi possedere per intero.**

La tua passkey resta dove il dispositivo la protegge già — Portachiavi iCloud,
Gestore delle password di Google o una chiave di sicurezza hardware che tieni tu.
Quando firmi una transazione, Vela chiede al dispositivo di firmarla; il
dispositivo firma e restituisce solo la firma. Vela non vede mai la chiave.

La maggior parte dei wallet ha ancora un momento pericoloso, anche se breve:
parole su uno schermo, una seed phrase in memoria, una chiave di recupero
parcheggiata in una scheda del browser. Vela è progettato perché quel momento non
esista.

<Callout type="info" title="Non una promessa — un'architettura">
Non possiamo accedere alle tue chiavi. Non «promettiamo di non farlo»: in Vela non
esiste un percorso di codice che possa farlo, e WebAuthn non lo permette. Il
wallet è uno <a href="/it/docs/account-contract">smart account Safe</a> azionato da
una firma che il tuo dispositivo produce e che noi ci limitiamo a ricevere. Ciò che
l'app con cui firmi decide davvero è <em>che cosa</em> viene chiesto di firmare
alla tua chiave — ed è per questo che il
<a href="/it/docs/whitepaper">modello delle minacce</a> vi dedica tanto spazio.
</Callout>

Abbiamo reso Vela **open source** perché tu possa verificarlo da solo, e
**ospitabile in proprio** perché un wallet esistente continui a funzionare senza i
server della nostra azienda — con un limite, il dominio a cui appartengono le tue
passkey, che la [guida al self-hosting](/it/docs/self-hosting) spiega insieme ai
modi per aggirarlo. E l'abbiamo costruito su
[contratti Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
non modificati, perché la strada noiosa e collaudata è quella giusta quando ci
sono di mezzo i soldi delle persone: sono gli stessi contratti che già proteggono
miliardi on-chain.

## Il compromesso che abbiamo scelto

Un compromesso resta, e nasconderlo sarebbe disonesto.

Con Vela il tuo account Apple o Google conta, perché è lì che vive una passkey
sincronizzata. Perdi quell'account, o cancelli la passkey, e non c'è seed phrase,
né reset da parte dell'assistenza, né porta sul retro.

Ma ogni wallet in autocustodia ti chiede di scegliere con quale rischio preferisci
convivere. Una seed phrase può essere copiata, fotografata, rubata col phishing o
digitata nel sito sbagliato all'una di notte. Una passkey è diversa: non ci sono
parole da rivelare, nessun segreto da incollare e nessun sito falso capace di
fartela consegnare. Il browser la offre solo alle pagine del dominio vero.

E la scelta non è binaria. Un wallet può essere creato con **fino a sette
firmatari**, ognuno capace di firmare da solo: passkey su dispositivi diversi, un
telefono vicino che colleghi con un codice QR o una chiave di sicurezza USB/NFC. Se
preferisci che il wallet non dipenda affatto da un account di piattaforma, puoi
usare solo chiavi di sicurezza hardware — due, perché un wallet non può reggersi
su una sola chiave che non si sincronizza da nessuna parte. L'unica condizione è
il momento: il tuo indirizzo è derivato dall'insieme completo delle chiavi, quindi
le scegli quando crei il wallet.

<Callout type="warning" title="Cosa non ti garantisce">
I firmatari in più sono una via per rientrare, non un secondo lucchetto. Poiché una
singola chiave basta a firmare, aggiungere una chiave hardware ti protegge dal
<em>perdere</em> l'accesso: non ferma chi ha già preso il controllo di una delle
tue chiavi. È questa la forma onesta dell'1-of-n.
</Callout>

## Ecco perché esiste Vela

Un wallet senza seed phrase da nascondere, senza chiave di recupero di cui fidarsi
e senza un'azienda di cui devi sperare che resti in giro per sempre.

Se vuoi verificare le affermazioni invece di prenderle per buone: il
[whitepaper](/it/docs/whitepaper) descrive l'architettura,
[Audit e problemi noti](/it/docs/security-audits) elenca ogni contratto da cui
dipendiamo e cosa ha avuto un audit e cosa no, e tutto il codice è
[su GitHub](https://github.com/mondaylabsltd/vela-wallet).

Poi: [installare Vela](/it/docs/install).
