---
title: Introduzione
description: "Cos'è Vela in sei righe, e le domande con cui arriva di solito chi legge — ognuna collegata direttamente alla sua risposta."
source: f1879437a0b5
---

# Documentazione di Vela

Vela è un **wallet open source per Ethereum e le altre reti EVM, che puoi ospitare
in proprio**, senza seed phrase. Il tuo wallet è uno smart account
[Safe](/it/docs/account-contract) non modificato, e firmi con le **passkey**: sul
tuo telefono o computer, su un altro telefono o su una chiave di sicurezza
hardware.

- **Funziona senza di noi.** Le app e ogni servizio su cui si appoggiano — il
  relay, l'indice delle chiavi pubbliche, i dati delle chain e i tassi di cambio —
  hanno licenza MIT. Compilali e gestiscili tu con la
  [guida al self-hosting](/it/docs/self-hosting), che ne elenca anche i limiti.
- **Le tue chiavi, fino a sette.** Le scegli quando crei il wallet; una qualsiasi
  può firmare. Vela non le ha mai e non ha alcun ruolo sul tuo wallet.
- **24 reti, un solo indirizzo.** Più qualsiasi rete EVM che aggiungi tu e che
  soddisfi i requisiti.
- **Leggi prima di firmare.** Le transazioni vengono decodificate in linguaggio
  chiaro; ciò che non si riesce a decodificare viene segnalato.
- **Alpha.** Funziona e contiene fondi veri, ma è giovane: parti con importi
  piccoli. [Cosa significa alpha qui](/blog/vela-is-in-alpha).

## Trova una risposta

| Voglio sapere… | Dove guardare |
| --- | --- |
| Come gestire tutto in proprio | [Guida al self-hosting](/it/docs/self-hosting) |
| Come gestire un mio relay, e a chi va la commissione | [Guida al self-hosting → relay](/it/docs/self-hosting#relay) · [Reti e commissioni → la commissione](/it/docs/networks-and-fees#fee) |
| Cosa succede se getvela.app va offline | [Guida al self-hosting → senza getvela.app](/it/docs/self-hosting#if-getvela-app-disappears) |
| Perché una transazione costa quello che costa | [Reti e commissioni](/it/docs/networks-and-fees) |
| Se la mia chain è supportata, o come aggiungerne una | [Reti e commissioni](/it/docs/networks-and-fees) · [Configurazione della chain](/it/chain-setup) |
| Se Vela ha avuto un audit | [Audit e problemi noti](/it/docs/security-audits) |
| Come controllare cosa sto firmando davvero | [Firma leggibile](/it/docs/clear-signing) · [L'attacco a Bybit](/it/docs/bybit-attack) |
| Quale app installare, e quanto costa | [Installare Vela](/it/docs/install) · [Ottieni Vela](/it/get-started) |
| Come creare un wallet, e quali chiavi usare | [Crea il tuo wallet](/it/docs/create-wallet) · [Firmatari e chiavi di sicurezza](/it/docs/signers) |
| Cosa fare se perdo il telefono o cancello una passkey | [Recupero e accesso](/it/docs/recovery) |
| Se più avanti posso aggiungere o cambiare chiavi | [Firmatari e chiavi di sicurezza](/it/docs/signers) |
| Come usare Vela con una dApp | [Installare Vela → dApp](/it/docs/install#dapps) |
| Cosa c'è di pubblico sul mio wallet | [Crea il tuo wallet → cosa è pubblico](/it/docs/create-wallet#what-is-public) · [Informativa sulla privacy](/privacy) |

## Per approfondire

- [Perché abbiamo creato Vela](/it/docs/why-vela) — la storia e il compromesso che
  abbiamo scelto.
- [Whitepaper](/it/docs/whitepaper) — l'architettura, e di cosa ti fidi
  esattamente.
- [Il contratto dell'account](/it/docs/account-contract) — quali contratti
  custodiscono i tuoi soldi.

Il [blog](/blog) racconta come viene costruito Vela.
