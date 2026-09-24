---
title: Crea il tuo wallet
description: "Crea un wallet Vela con da una a sette chiavi — cosa fa ogni passaggio, perché le chiavi sono fissate alla creazione, cosa diventa pubblico e cos'è davvero il tuo wallet."
source: a2edda21a075
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Crea il tuo wallet

Creare un wallet richiede un paio di minuti. Apri il wallet web su
[wallet.getvela.app](https://wallet.getvela.app/) — oppure l'estensione, l'app
desktop o quella per telefono — e scegli **Crea wallet**.

## Passaggi

1. **Dai un nome al wallet.** Il nome ti aiuta a riconoscerlo, e viene scritto in
   un registro pubblico insieme alle tue chiavi: consideralo pubblico e non
   metterci niente di privato.
2. **Conferma cosa succede.** Spunti che le tue chiavi pubbliche e il nome del
   wallet vengono scritti on-chain, che le tue chiavi private restano nei tuoi
   dispositivi o nelle tue chiavi di sicurezza, e che accetti i [termini](/terms) e
   l'[informativa sulla privacy](/privacy).
3. **Crea la prima chiave.** Scegli come: **questo dispositivo** (Face ID, Touch
   ID, impronta, Windows Hello), **un telefono o un tablet** (scansioni un codice
   QR e la crei lì, dove l'app lo prevede) o una **chiave di sicurezza USB**. Il
   dispositivo crea la passkey e poi firma una volta con essa, così l'app sa che
   la chiave funziona davvero prima di andare avanti.
4. **Aggiungi altre chiavi, se vuoi.** Fino a sette in tutto, di qualsiasi tipo.
   Ognuna potrà firmare da sola. Se la tua unica chiave non è sincronizzata da
   nessuna parte — una chiave di sicurezza, o Windows Hello — l'app te ne chiede
   una seconda, perché con una sola chiave non sincronizzata basta perdere un
   dispositivo per perdere il wallet.
5. **Crea.** L'app calcola l'indirizzo del wallet dall'insieme completo delle
   chiavi e pubblica l'insieme nel registro pubblico su Gnosis Chain. Quando quel
   record è on-chain, il wallet si apre.

<Callout type="warning" title="Scegli adesso le tue chiavi">
Il tuo indirizzo è calcolato dalle chiavi con cui concludi, quindi più avanti le
chiavi non si possono aggiungere, rimuovere o sostituire.
[Firmatari e chiavi di sicurezza](/it/docs/signers) spiega perché, e come
sceglierle.
</Callout>

## Cos'è il tuo wallet

Il tuo wallet è uno **smart account Safe**: un contratto, non un semplice account
con una sola chiave privata. Le tue chiavi ne sono i proprietari, e una qualsiasi
di esse può autorizzare una transazione.
[Il contratto dell'account](/it/docs/account-contract) elenca tutti i contratti
coinvolti.

L'indirizzo è **lo stesso su ogni rete**, ed è **controfattuale**: viene calcolato
prima di qualsiasi deploy, quindi puoi ricevere fondi su qualsiasi rete fin da
subito. Il contratto viene deployato automaticamente la prima volta che invii da
una rete, e la commissione di quella prima transazione comprende il deploy. Creare
il wallet non ti costa nulla.

## Cosa è pubblico

<span id="what-is-public"></span>

Creare un wallet scrive un record permanente in un contratto di registro pubblico
su Gnosis Chain, leggibile da chiunque e impossibile da modificare o cancellare:

- la **chiave pubblica** di ogni chiave (mai la chiave privata) e il suo **ID
  della credenziale**;
- il **modello di autenticatore** di ogni chiave (quale gestore di password o
  chiave di sicurezza l'ha creata) e dei flag che indicano se è avvenuta la
  verifica dell'utente e se la chiave è sincronizzata;
- il **nome del wallet** e un'**etichetta per ogni chiave**;
- l'**indirizzo del wallet** e la data di creazione;
- i **dati di registrazione firmati** stessi.

L'indice delle chiavi pubbliche di Vela invia il record e ne paga il gas, quindi è
il primo a vederlo. Niente di quanto contiene può spostare i tuoi fondi; è ciò che
permette a una qualsiasi delle tue chiavi di ritrovare il wallet su un nuovo
dispositivo ([recupero](/it/docs/recovery)). L'[informativa sulla privacy](/privacy)
riporta l'elenco completo, e la [pagina del registro](/registry) mostra tutti i
record.

## Prossimi passi

- [Ricevere i primi token](/it/docs/send-and-receive)
- [Capire reti e commissioni](/it/docs/networks-and-fees)
- [Cosa fare se perdi un dispositivo](/it/docs/recovery)
