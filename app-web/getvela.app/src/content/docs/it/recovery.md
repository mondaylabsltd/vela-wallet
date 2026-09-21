---
title: Recupero e accesso
description: "Come rientri nel tuo wallet su un nuovo dispositivo con una qualsiasi delle tue chiavi, dove viene cercato il wallet e i limiti onesti del recupero senza seed phrase."
source: 77cf3f24c7c7
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Recupero e accesso

Senza seed phrase, il recupero si basa su due cose: **una chiave che hai ancora**
e **un record pubblico di quali chiavi appartengono al tuo wallet**.

## Cosa viene registrato quando crei un wallet

L'indirizzo del tuo wallet è calcolato da tutte le chiavi con cui lo crei.
Perché una qualsiasi di quelle chiavi possa ritrovare il wallet in seguito, la
creazione scrive un record in un **contratto di registro** pubblico su Gnosis
Chain: la chiave pubblica di ogni chiave, l'indirizzo del wallet, il suo nome e i
dati di registrazione firmati. Il registro non ha un proprietario e non si può
modificare né cancellare. (L'elenco completo di ciò che è pubblico è in
[crea il tuo wallet](/it/docs/create-wallet#what-is-public).)

Il servizio di indice delle chiavi pubbliche di Vela invia quel record e ne paga
il gas; il record in sé vive on-chain, e qualsiasi app può leggerlo direttamente.

## Accedere da un nuovo dispositivo

1. Apri Vela e scegli di accedere.
2. Usa **una qualsiasi** delle tue chiavi: una passkey sincronizzata su questo
   dispositivo, un telefono vicino (scansiona il codice QR) o la tua chiave di
   sicurezza.
3. Vela ricava dalla firma la chiave pubblica di quella chiave, la cerca — prima
   nell'indice di Vela, poi, se l'indice non risponde, nel contratto di registro su
   Gnosis e quindi su Ethereum — e ricostruisce il wallet. Prima di mostrartelo,
   controlla che le chiavi trovate producano davvero l'indirizzo registrato.

L'elenco degli account non viene sincronizzato tra dispositivi; accedendo lo
ricostruisci.

<Callout type="info" title="Se nessun indice o registro risponde">
Un wallet con una <strong>sola chiave</strong> si può ricostruire sul dispositivo
senza alcun server: due firme di quella chiave bastano per ricavarne la chiave
pubblica e ricalcolare l'indirizzo. Un wallet con più chiavi ha bisogno del record
nel registro, perché una chiave non può dire all'app quali erano le altre.
</Callout>

## Copie del record

Il registro su Gnosis è quello che le app leggono per primo. Dalle
**Impostazioni** puoi anche copiare il record del tuo wallet nello stesso contratto
di registro su **Ethereum**, pagando tu il gas, così il record esiste su una
seconda chain. Chiunque può fare una copia del genere; non contiene nulla che
possa spostare fondi.

## I limiti, detti onestamente

<Callout type="warning" title="Una chiave persa è persa">
Se tutte le chiavi con cui hai creato il wallet non ci sono più — le passkey
sincronizzate, i telefoni, le chiavi di sicurezza — nessuno può recuperare il
wallet: né Vela, né Apple o Google, né chiunque altro. Non c'è seed phrase, né
reset da parte dell'assistenza, né porta sul retro.
</Callout>

Quello che lo rende improbabile è avere più di una via d'accesso:

- **Tieni attiva la sincronizzazione delle passkey** se usi la passkey di questo
  dispositivo. È ciò che porta la chiave su un nuovo telefono o computer.
- **Proteggi l'account che c'è dietro.** Chi controlla il tuo account Apple o
  Google potrebbe riuscire a usare una passkey sincronizzata; dagli una password
  robusta e metodi di recupero propri.
- **Crea il wallet con più di una chiave**, per esempio la passkey del telefono e
  una chiave di sicurezza hardware tenuta in un posto sicuro. Le chiavi si possono
  aggiungere solo quando crei il wallet ([perché](/it/docs/signers)). Ricorda che
  una qualsiasi chiave può firmare da sola — e non si può rimuovere, quindi se una
  viene compromessa sposta i fondi in un nuovo wallet
  ([cosa fare](/it/docs/signers)).

## Cosa può e cosa non può fare Vela

- **Può:** tenere attivo l'indice, così il tuo wallet viene trovato rapidamente su
  un nuovo dispositivo.
- **Non può:** spostare i tuoi fondi, congelare il wallet, aggiungere o rimuovere
  chiavi, o recuperare una chiave che hai perso. Vela non ha mai le tue chiavi.

Poi: [firma leggibile](/it/docs/clear-signing).
