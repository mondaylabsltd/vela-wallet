---
title: Ospita tu la pagina di firma
description: "La pagina senza dipendenze e l'estensione Chrome che decodificano da sole una transazione e la firmano con la tua passkey — come gestire una tua copia, e quale copia può firmare per il tuo wallet."
source: c81389ef7aa2
---

# Ospita tu la pagina di firma

Vela decodifica ogni transazione prima che tu la approvi, ed è un lavoro fatto
onestamente — ma è fatto dalla stessa app che ha costruito la transazione. Se
l'app, o il modo in cui ti arriva, viene manomessa, può mostrarti una cosa e
firmarne un'altra. È esattamente ciò che è successo a [Bybit](/it/docs/bybit-attack).

La pagina di firma esiste per dividere tutto questo in due: la transazione arriva
da un posto, e il controllo e la firma avvengono da un'altra parte, che controlli
tu.


Quando riceve una richiesta di firma, non si fida del riepilogo che l'accompagna.
Decodifica da sola la calldata grezza, calcola il proprio digest, ti mostra cosa
autorizzerà davvero la firma e solo allora chiede la tua passkey.

Poiché non c'è uno step di build, i file che leggi sono i file che girano. Puoi
confrontare la cartella con il repository e sapere cosa stai servendo.

## Quale copia può firmare per il tuo wallet

Una passkey è legata al dominio su cui è stata creata. Le tue chiavi Vela sono
registrate sotto `getvela.app`, e un browser le offre solo a una pagina la cui
relying party è `getvela.app`. Questa sola regola decide quale modo di gestire una
tua copia ti è utile.

**Come pagina su un tuo dominio, o su localhost.** Servita in HTTPS (o da
localhost), la relying party della pagina è il suo stesso hostname: può quindi
firmare con chiavi registrate sotto _quell'_ hostname, non con chiavi registrate
sotto `getvela.app`. È il modo giusto per provare l'intera procedura da capo a
fondo, per far girare il flusso desktop e per firmare per un wallet la cui chiave è
stata creata sul tuo dominio. Non è un modo per firmare per un wallet
`getvela.app` esistente.

```sh
cd app-web/trusted-signer
python3 -m http.server 8080   # → http://localhost:8080
```

Tutti i percorsi nell'app sono relativi, quindi funziona anche una sottocartella
su un host esistente; aprire `index.html` direttamente da disco (`file://`) va
bene per dare un'occhiata — senza origine non c'è relying party e non si può
firmare nulla.

## Cosa fa prima di firmare

- **Decodifica da sola la transazione.** Cosa fa la chiamata, a chi e per quanto,
  a partire dalla calldata — comprese le chiamate annidate dentro un batch.
- **Firma solo un digest che ha calcolato lei.** I digest EIP-191, EIP-712, SafeOp
  e SafeMessage vengono calcolati nella pagina e confrontati con `vela-core`, lo
  stesso codice usato dal wallet. Un digest che non riesce a calcolare è un
  rifiuto, non una firma.
- **Verifica che la transazione sia quella richiesta.** La chiamata chiesta dal
  sito deve trovarsi davvero dentro l'operazione che viene firmata.
- **Dice quando un'approvazione è illimitata.** Non può cambiare un importo — firma
  i byte arrivati o niente — quindi un'approvazione o un permit illimitati (2^128 o
  più su questa pagina) vengono mostrati in rosso con questo motivo e si possono
  firmare così come sono; un limite on-chain si sceglie nella schermata di
  approvazione del wallet stesso, prima che la richiesta arrivi qui.
  Un'approvazione per un'intera collezione NFT viene rifiutata.
- **Dice quando non riesce a leggere qualcosa,** invece di mostrare un riepilogo
  rassicurante di cui non può rispondere.
- **Mostra l'indirizzo e l'identicon dell'account,** e non mostra un nome del
  destinatario fornito da chi ha chiesto la firma. Tutto ciò che il richiedente
  controlla viene tolto oppure etichettato come suo.

## Cosa non ha, di proposito

- **Nessun editor.** La richiesta è fissata al suo arrivo: o la firmi o no. Un
  selettore della commissione o un editor dell'allowance riscriverebbero la
  calldata, che è proprio il male che questa pagina esiste per prevenire.
- **Nessuna creazione di chiavi.** La pagina di firma non può creare una passkey.
  Crearne una vorrebbe dire creare un account diverso.
- **Nessun dato dalla rete.** Niente di ciò che mostra o firma viene scaricato.
  L'unica cosa che carica sono i loghi dei token, come immagini, dal server dei
  dati delle chain di Vela; se non arrivano, al loro posto compare una lettera.

## Come le arriva una richiesta

| Richiedente                                        | Canale                                                                     |
| -------------------------------------------------- | -------------------------------------------------------------------------- |
| Una pagina nello stesso browser                    | `postMessage`                                                              |
| Una pagina nello stesso browser, verso l'estensione | Porta dell'estensione                                                      |
| Un'app desktop sulla stessa macchina               | Frammento dell'URL + callback in loopback (demo in `samples/`; l'app desktop di Vela non lo usa ancora) |
| Un telefono o un altro computer                    | Bluetooth LE (protocollo implementato; radio non ancora testata su hardware reale) |

Il formato dei messaggi, i digest e una tabella con l'origine di ogni elemento
mostrato a schermo sono in `PROTOCOL.md`, accanto al codice.

## Dove si colloca

Quando le app potranno passarle le loro richieste, l'uso previsto è semplice: dal
giorno in cui l'account contiene soldi che ti dispiacerebbe perdere, ogni firma
passa per una pagina il cui codice hai caricato tu. Non solo per gli importi
grandi — una piccola approvazione può concedere abbastanza da svuotare un account.
Fino ad allora, la pagina è un modo per leggere e provare esattamente come
funzionerà quella seconda opinione.
