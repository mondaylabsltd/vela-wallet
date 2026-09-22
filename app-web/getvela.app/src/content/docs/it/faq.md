---
title: Domande frequenti
description: "Risposte brevi su custodia, chiavi, recupero, reti, commissioni, cosa può vedere Vela, open source e cosa succede se Vela non c'è più."
source: 2d38e6a0b6d9
---

# Domande frequenti

## Vela è in autocustodia?

Sì. Il tuo wallet è uno smart account Safe controllato solo dalle tue chiavi, che
restano nei tuoi dispositivi, nel tuo gestore di password o nelle tue chiavi di
sicurezza. Vela non ha alcuna chiave né alcun ruolo sul wallet, quindi non può
spostare, congelare o recuperare i tuoi fondi per conto proprio. Però scrive il
software che chiede alle tue chiavi di firmare — vedi il
[modello delle minacce](/it/docs/whitepaper).

## Davvero non c'è nessuna seed phrase?

Davvero. Le tue chiavi sono passkey, e una passkey non ha alcun segreto che tu
possa scrivere su un foglio o digitare. Vedi
[come funzionano le passkey](/it/docs/passkeys).

## Cosa mi serve per creare un wallet?

Un dispositivo che supporti le passkey (un telefono o un computer recente con Face
ID, impronta o Windows Hello), oppure due chiavi di sicurezza hardware. Nessuna
email, nessun account, nessun saldo iniziale. Puoi creare il wallet con un massimo
di sette chiavi; dopo non se ne possono aggiungere altre. Vedi
[crea il tuo wallet](/it/docs/create-wallet).

## Cosa succede se perdo il telefono?

Accedi da un nuovo dispositivo con un'altra chiave qualsiasi: la stessa passkey
sincronizzata tramite il Portachiavi iCloud o il Gestore delle password di Google,
un altro telefono o la tua chiave di sicurezza. Se il telefono conteneva la tua
unica chiave e quella chiave non era sincronizzata, il wallet non si può
recuperare. Vedi [recupero e accesso](/it/docs/recovery).

## Quali reti e quali token sono supportati?

24 reti EVM integrate, tra cui Ethereum, Base, Arbitrum, Optimism, Polygon, BNB
Chain, Gnosis e Avalanche, più qualsiasi rete EVM che aggiungi tu e che soddisfi i
requisiti. Monete native e token ERC-20. L'indirizzo è lo stesso su ogni rete.
Vedi [reti e commissioni](/it/docs/networks-and-fees).

## Quanto costa?

- **Le app:** il wallet web, l'estensione del browser e le app desktop sono
  gratuiti. Le app iOS e Android saranno un acquisto una tantum negli store; puoi
  anche compilare gratis qualsiasi app dal codice sorgente.
- **Ogni transazione:** una commissione pagata dal tuo wallet al relay che la
  invia — quello di Vela, a meno che tu non indirizzi il wallet verso un altro relay
  o ne gestisca uno tuo. Copre il gas più il margine del relay, con un minimo di
  circa 0,01 dollari. L'importo esatto è nella schermata di conferma prima che tu
  firmi e fa parte di ciò che firmi. Non ci sono depositi né abbonamenti.
  [Come si calcola la commissione](/it/docs/networks-and-fees#fee).
- **Nessun token.** Vela non ne ha e non ne prevede.

## Posso usare Vela con le dApp?

Sì, tramite l'estensione Vela per il browser (Chrome, Edge, Brave) e il browser
integrato nelle app desktop (macOS, Windows), iOS e Android. Il wallet web su
wallet.getvela.app non si collega alle dApp. Vedi
[installare Vela](/it/docs/install#dapps).

## Cosa può vedere o fare Vela?

Vela non può leggere le tue chiavi né spostare i tuoi fondi per conto proprio. I
suoi servizi vedono il tuo indirizzo IP e ciò che l'app chiede loro: l'indice vede
le tue chiavi pubbliche e il nome del wallet quando registra un nuovo wallet, e gli
indirizzi che cerchi; il relay vede il tuo indirizzo, le operazioni che invii e
l'endpoint RPC usato dalla tua app; il servizio dei dati delle chain vede quali
token e contratti chiede la tua app. Ciò che diventa pubblico on-chain è elencato
in [crea il tuo wallet](/it/docs/create-wallet#what-is-public).
L'[informativa sulla privacy](/privacy) è la versione completa, e quella che fa
fede.

## Vela è open source?

Le app del wallet, il relay e il servizio di tassi di cambio hanno licenza MIT su
[GitHub](https://github.com/orgs/mondaylabsltd/repositories); anche l'archivio dei
dati delle chain è MIT. L'indice delle chiavi pubbliche è pubblico, ma non ha
ancora un file di licenza. Ogni servizio lo puoi gestire tu — vedi la
[guida al self-hosting](/it/docs/self-hosting).

## Vela ha avuto un audit?

I contratti in cui stanno i tuoi soldi — Safe e i suoi moduli, e l'EntryPoint
ERC-4337 — hanno avuto un audit. Il codice di Vela no, e non ci sono audit in
programma. Vedi [audit e problemi noti](/it/docs/security-audits).

## E se Vela chiude?

I tuoi fondi restano nel tuo Safe, on-chain. Per un wallet esistente, l'estensione
Vela per il browser e le app che compili tu continuano a funzionare senza
getvela.app, e ogni servizio è open source perché qualcun altro possa gestirlo. La
[guida al self-hosting](/it/docs/self-hosting#if-getvela-app-disappears) elenca le
strade e i loro limiti.

## Ho una domanda che qui non c'è.

Apri una issue su [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues),
oppure scrivici su [X](https://x.com/realvelawallet) o
[Telegram](https://t.me/velawallet).
