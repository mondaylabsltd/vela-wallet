---
title: Domande frequenti
description: Domande comuni su Vela — custodia, passkey, account intelligenti, recupero, reti supportate, commissioni e privacy.
---

# Domande frequenti

## Vela è auto-custodito?

Sì. Il tuo wallet è un account intelligente controllato da una chiave che solo tu
puoi usare, custodita dal sistema operativo del dispositivo e mai vista da Vela.
Vela non può spostare, congelare o recuperare i tuoi fondi.

## Il mio wallet è un account normale o un contratto?

È un **account intelligente Safe** (un contratto), operato con l'astrazione
dell'account ERC-4337. È questo che ti permette di firmare con una passkey, di
leggere ogni transazione prima di approvarla e di usare lo stesso indirizzo su ogni
rete. L'architettura è nel [whitepaper](/it/docs/whitepaper).

## Davvero non c'è nessuna frase seed?

Davvero. La tua chiave di firma è una passkey custodita dal sistema operativo del
dispositivo, e Vela non la vede mai. Non ci sono dodici parole da annotare,
perdere o farsi rubare col phishing. Perché è sicuro:
[come funzionano le passkey](/it/docs/passkeys).

## Cosa succede se perdo il telefono?

Se la tua passkey è sincronizzata tramite Portachiavi iCloud o Gestore delle
password di Google, accedi su un nuovo dispositivo con lo stesso account e il
wallet torna. Il modello completo e i suoi limiti sono in
[recupero e accesso](/it/docs/recovery).

## Quali reti e quali token sono supportati?

Vela include **12 reti EVM** — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
Base, Avalanche, Gnosis, Unichain, Tempo, Monad e World Chain — più le reti
personalizzate, con token nativi ed ERC-20. Il tuo indirizzo è lo stesso su tutte.
Vedi [reti e commissioni](/it/docs/networks-and-fees).

## Quanto costa usarlo?

Il wallet è gratuito e Vela non ha **alcun token**. Paghi il **gas** di rete dal
tuo saldo, più una commissione del relay. Il prezzo lo indica il relay e appare
**prima che tu firmi**, suddiviso in _commissione di rete / commissione del relay /
totale_: il costo esatto di ogni transazione è nella schermata di conferma, e
l'importo indicato fa parte di ciò che firmi, quindi non può cambiare dopo. Le
transazioni molto economiche possono incontrare una piccola commissione minima. Su
Tempo, che non ha una moneta nativa, il gas è regolato in stablecoin USD. Ogni rete
richiede inoltre un piccolo **deposito non rimborsabile per attivare il suo account
relay del gas** (Vela può coprirlo per i nuovi utenti); poiché quell'account può
esaurirsi, potresti doverlo ricaricare più avanti — quindi non è propriamente un
costo una tantum. Dettagli in [reti e commissioni](/it/docs/networks-and-fees).

## Cosa può vedere o fare Vela (l'azienda)?

Vela conserva la chiave **pubblica** della tua passkey e il **nome** che hai
scelto, per abilitare l'accesso multi-dispositivo. Non vede la tua chiave privata,
i saldi vengono letti da chain pubbliche e non esiste alcuna registrazione via
e-mail. La versione che fa fede è la [privacy policy](/privacy).

## Vela è open source?

Sì: il wallet e i suoi quattro servizi backend (dati di chain, indice passkey,
relay, tassi di cambio) sono
[pubblici su GitHub](https://github.com/mondaylabsltd/vela-wallet) con licenza MIT,
e puoi ospitarli tu.

## Ho una domanda che qui non c'è.

Apri una issue su [GitHub](https://github.com/mondaylabsltd/vela-wallet) oppure
scrivici su [X](https://x.com/realvelawallet) o
[Telegram](https://t.me/velawallet).
