---
title: Introduzione
description: Cos'è Vela, per chi è pensato e le idee dietro un wallet intelligente auto-custodito senza frase seed.
---

# Introduzione

Vela è un **wallet intelligente auto-custodito** per le reti EVM. Le chiavi sono
tue, ma non c'è nessuna frase seed da annotare: firmi con una passkey, usando il
volto o l'impronta.

Questa documentazione spiega come iniziare, creare un wallet, spostare token e
capire il modello di sicurezza che c'è sotto.

## La versione breve

- **Auto-custodito.** I tuoi fondi sono controllati da una chiave che solo tu puoi
  usare. Vela (l'azienda) non può spostare, congelare o recuperare i tuoi soldi.
- **Nessuna frase seed.** La tua chiave di firma è una passkey custodita
  nell'hardware sicuro del dispositivo. Non esistono dodici parole da perdere o da
  farsi rubare con il phishing.
- **Un account intelligente Safe.** Ogni wallet è un contratto
  [Safe](https://github.com/safe-fndn/safe-smart-account), operato con
  l'astrazione dell'account ERC-4337: è proprio questo che ti permette di firmare
  con una passkey e di leggere ogni transazione prima di approvarla.
- **12 reti, un solo indirizzo.** Ethereum, BNB Chain, Polygon, Arbitrum,
  Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad e World Chain — più le
  reti che aggiungi tu — tutte allo stesso indirizzo.
- **Firma leggibile.** Dove esiste un descrittore, la transazione viene tradotta in
  un'intenzione leggibile (ERC-7730); dove non c'è, Vela ripiega su una decodifica
  best-effort e ti avvisa. Le chiamate che non riesce a leggere vengono segnalate,
  non nascoste.
- **Open source.** Il wallet e tutti i suoi servizi sono
  [pubblici su GitHub](https://github.com/mondaylabsltd/vela-wallet), così chiunque
  può verificare cosa fanno.
- **Software in alpha.** Vela funziona e contiene soldi veri, ma non ha anni di
  rodaggio in produzione alle spalle. Inizia con importi piccoli. Il
  [post sull'alpha](/blog/vela-is-in-alpha) spiega cosa significa.

## Per chi è pensato

Vela è per chi vuole auto-custodia vera senza la trappola della gestione della
frase seed — e per chi ci si è già scottato. Se sai sbloccare il telefono, sai
usare Vela.

## Dove andare adesso

- [Installare Vela](/it/docs/install) — gira nel browser, niente da scaricare.
- [Crea il tuo wallet](/it/docs/create-wallet) — il primo wallet in circa un
  minuto.
- [Come funzionano le passkey](/it/docs/passkeys) — il modello di sicurezza,
  spiegato chiaro.
- [Whitepaper](/it/docs/whitepaper) — l'architettura completa e il modello di
  fiducia.

Se ti interessa più il *perché* che il *come*, il [blog](/blog) racconta come
Vela viene costruito.
