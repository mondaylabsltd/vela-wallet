---
title: Trusted Signer
description: "Una pagina in un solo file su sign.getvela.app che decodifica una richiesta e la firma con la tua passkey per conto proprio — cosa controlla, quali app la usano e come ricostruirla o gestirne una tua copia."
source: 43a1af6ffdcd
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Trusted Signer

Vela decodifica ogni transazione prima che tu la approvi, ed è un lavoro fatto
onestamente — ma è fatto dalla stessa app che ha costruito la transazione. Se
l'app, o il modo in cui ti arriva, viene manomessa, può mostrarti una cosa e
firmarne un'altra. È esattamente ciò che è successo a [Bybit](/it/docs/bybit-attack).

Il Trusted Signer esiste per dividere tutto questo in due: l'app consegna soltanto
la richiesta, e il controllo e la firma avvengono su una pagina separata — una
pagina che puoi leggere da cima a fondo, ricostruire byte per byte o gestire tu.

## Dove funziona

La pagina ufficiale è servita da **sign.getvela.app**. L'app desktop (macOS,
Windows, Linux) e le app per iPhone e Android possono inviarle una richiesta: l'app
apre la pagina in una scheda del browser con la richiesta nel link, tu la controlli
e firmi lì con la tua passkey, e la pagina restituisce la firma all'app tramite un
link `velawallet://`. Il wallet web non può usarlo.

È attivo solo se lo scegli tu: lo scegli come modo di firmare quando crei un wallet
o accedi, e da quel momento ogni firma per quel wallet su quel dispositivo passa da
lì. Può anche creare le chiavi del wallet. Su `sign.getvela.app` usa le stesse
passkey di `getvela.app` usate dalle app.

<Callout type="info" title="Cosa è stato testato finora">
Esecuzioni complete registrate con la pagina pubblicata: Android e Windows 11
(creazione di un wallet e accesso). Le app per macOS, Linux e iPhone usano lo
stesso collegamento; per nessuna di loro c'è ancora un'esecuzione completa
registrata.
</Callout>

## Cosa fa prima di firmare

- **Decodifica da solo la richiesta.** Cosa fa la chiamata, a chi e per quanto,
  a partire dalla calldata — comprese le chiamate annidate dentro un batch.
- **Firma solo un digest che ha calcolato lui.** I digest EIP-191, EIP-712, SafeOp
  e SafeMessage vengono calcolati nella pagina, mai presi dal richiedente; i test
  confrontano i digest SafeOp e SafeMessage con `vela-core`, il codice usato dal
  wallet, e l'app rifiuta una firma su qualsiasi digest diverso da quello che ha
  calcolato lei stessa.
- **Verifica che la transazione sia quella richiesta.** La chiamata chiesta dal
  sito deve trovarsi davvero dentro l'operazione che viene firmata, altrimenti la
  pagina rifiuta.
- **Dice quando un'approvazione è illimitata.** Non può cambiare un importo — firma
  i byte arrivati o niente — quindi un'approvazione o un permit illimitati (2^128 o
  più su questa pagina) vengono mostrati in rosso con questo motivo e si possono
  firmare così come sono; un limite on-chain si sceglie nella schermata di
  approvazione del wallet stesso, prima che la richiesta arrivi qui.
  Un'approvazione per un'intera collezione NFT viene rifiutata.
- **Rifiuta ciò di cui non può rispondere:** `eth_sign`, un metodo che non conosce,
  un token inviato al contratto del token stesso, un'operazione che non riesce a
  leggere e un accesso la cui challenge è stata fornita dal richiedente.
- **Rifiuta ciò che consegnerebbe il tuo account,** con la stessa regola delle app:
  una chiamata del tuo account a una delle sue funzioni per owner, moduli, guard o
  fallback, anche dentro un batch; un `delegatecall`, tranne verso il contratto
  MultiSend di Safe, che raggruppa le chiamate di un'operazione; e una firma
  `SafeTx`. Controlla ogni chiamata dell'operazione assemblata dall'app, non solo
  quelle chieste dal sito.
- **Mostra l'indirizzo dell'account e un identicon calcolato nella pagina.**
  Destinatari e contratti non vengono mai nominati in base alla richiesta — solo la
  tabella della pagina stessa, sottoposta a revisione, può dare un nome a un
  contratto. Il nome dell'account, che l'app invia perché tu possa scegliere la
  passkey giusta, compare accanto al suo indirizzo.
- **Chiede la verifica dell'utente** (impronta digitale, volto o PIN) a ogni firma.

## Cosa non ha, di proposito

- **Nessun editor.** La richiesta è fissata al suo arrivo: o la firmi o no. Un
  selettore della commissione o un editor dell'allowance riscriverebbero la
  calldata, che è proprio il male che questa pagina esiste per prevenire.
- **Nessun accesso alla rete.** La pagina è un unico file la cui content security
  policy (`default-src 'none'`) sta dentro i suoi stessi byte, quindi non può
  scaricare nulla, aprire una connessione o caricare un'immagine. L'unica cosa che
  ne esce è la sua risposta, quando segue il link di callback contenuto nella
  richiesta (`velawallet://` quando a chiedere è stata un'app Vela). I loghi dei
  token sono disegnati come lettere.

## Cosa controlla l'app in cambio

Nemmeno l'app si fida della pagina. Accetta una firma solo se la challenge firmata
è il digest **calcolato dall'app**, la verifica dell'utente è stata eseguita, la
chiave è una di quelle del tuo wallet e la firma P-256 è valida per quella chiave.

## Ogni versione pubblicata, verificabile

Ogni versione viene compilata da `app-web/trusted-signer/src/` in un unico file, in
modo riproducibile — Bun e Node producono gli stessi byte — e pubblicata al proprio
indirizzo, `sign.getvela.app/b/<sha256>/sign.html`, accanto a tutte le versioni
precedenti. L'elenco è su `sign.getvela.app/index.json`.

```sh
cd app-web/trusted-signer
node samples/build-single.mjs --check   # rebuilds a version listed in dist/
curl -sL https://sign.getvela.app/b/<sha256>/sign.html | shasum -a 256
```

All'avvio, l'app desktop scarica la versione pubblicata che aprirà, ne calcola
l'hash e lo confronta con le versioni incluse nell'app. Il risultato viene soltanto
registrato nel log, e una pagina che non corrisponde viene aperta comunque. Le app
per telefono non fanno ancora questo controllo.

## Gestire una tua copia

Nelle Impostazioni c'è l'indirizzo della pagina che le tue app aprono, così puoi
farlo puntare a un tuo deployment: qualsiasi indirizzo HTTPS, oppure `localhost`
per i test. Compilala con `bun samples/build-single.mjs` (o `node`) e copia `dist/`
sul tuo host.

Una copia su un tuo dominio firma con passkey create per **quel** dominio, non con
le passkey di `getvela.app` — quindi è un modo per creare e usare un wallet le cui
chiavi stanno sotto il tuo dominio, non un modo per firmare per un wallet
`getvela.app` esistente. Tutte le chiavi di un wallet condividono un solo dominio.

Il codice, e gli script che lo compilano e lo verificano, sono in
`app-web/trusted-signer/`.
