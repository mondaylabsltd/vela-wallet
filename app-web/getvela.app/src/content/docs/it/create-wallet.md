---
title: Crea il tuo wallet
description: Crea un wallet Vela auto-custodito in circa un minuto con una passkey — senza frase seed. Il tuo wallet è un account intelligente Safe con lo stesso indirizzo su ogni rete.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Crea il tuo wallet

Creare un wallet richiede circa un minuto e una sola verifica biometrica. Apri il
wallet web su [wallet.getvela.app](https://wallet.getvela.app/) e scegli **Crea un
wallet**.

## I passaggi

1. **Dai un nome al wallet.** Scegli un nome che ti faccia riconoscere l'account
   più avanti, anche quando accedi da un altro dispositivo. Viene salvato accanto
   alla tua chiave pubblica: trattalo come pubblico e non metterci nulla di
   privato.
2. **Conferma le basi.** Una breve checklist conferma che hai capito che Vela è
   auto-custodito e ancora in alpha, con i link alla
   [privacy policy](/privacy) e ai [termini](/terms).
3. **Crea la tua passkey.** Quando appare la richiesta, autenticati con **Face ID,
   Touch ID o l'impronta**. Così nasce una passkey WebAuthn (P-256) che il tuo
   dispositivo custodisce e che Vela non vede mai. Non c'è il passaggio «annota la
   frase seed», perché non c'è nessuna frase seed.
4. **Fatto.** Vela mostra l'indirizzo del wallet e sei dentro. Puoi verificarlo e
   poi accedere per entrare nel tuo wallet.

## Cos'è davvero il tuo wallet

È la parte che la maggior parte dei wallet non spiega — e che decide come funziona
Vela.

Il tuo wallet Vela è un **account intelligente Safe** (un contratto), non un comune
«account posseduto esternamente». La tua passkey è il proprietario dell'account; un
impianto ERC-4337 ti permette di usarlo con il solo volto o la sola impronta.

<Callout type="info" title="Il tuo indirizzo è lo stesso su ogni rete">
Vela deriva il tuo indirizzo dalla chiave pubblica della passkey, quindi è identico
su Ethereum, Base, Arbitrum, Gnosis e su ogni altra rete supportata. Dai un solo
indirizzo, ovunque.
</Callout>

Una conseguenza utile: l'indirizzo è **controfattuale**. Viene calcolato prima che
on-chain esista alcunché, quindi **puoi ricevere fondi prima ancora che il
contratto del wallet esista**. Il contratto si distribuisce da solo — pagando dal
proprio saldo — alla tua prima transazione su una data rete.

## Cos'è appena successo alle tue chiavi

- Il dispositivo ha generato una **coppia di chiavi passkey**.
- La **chiave privata** è custodita dal servizio passkey del sistema operativo
  (Portachiavi iCloud o Gestore delle password di Google), cifrata end-to-end e
  sincronizzata tra i tuoi dispositivi: nessuna app, Vela inclusa, la vede mai.
- La **chiave pubblica e il nome scelto** vengono pubblicati nell'indice passkey di
  Vela, che scrive anche la chiave in un registro leggibile pubblicamente su Gnosis
  Chain, così il tuo account può essere ritrovato da un nuovo dispositivo. Vedi
  [recupero e accesso](/it/docs/recovery).

## Prossimi passi

- [Ricevere i primi token](/it/docs/send-and-receive)
- [Capire reti e commissioni](/it/docs/networks-and-fees)
- [Leggere perché le passkey rendono tutto questo sicuro](/it/docs/passkeys)
