---
title: Ospita tu la pagina di firma
description: La pagina senza dipendenze e l'estensione Chrome che decodificano da sole una transazione e la firmano con la tua passkey — come far girare una tua copia, e quale copia può firmare per il tuo wallet.
---

# Ospita tu la pagina di firma

Vela decodifica ogni transazione prima che tu la approvi, ed è lavoro vero — ma è
lavoro fatto dalla stessa app che ha costruito la transazione. Se l'app, o il modo
in cui ti arriva, viene manomessa, può mostrarti una cosa e firmarne un'altra. È
esattamente ciò che è successo a [Bybit](/it/docs/bybit-attack).

La pagina di firma esiste per spezzare tutto questo in due: la transazione arriva
da un posto, il controllo e la firma avvengono da un'altra parte, che controlli tu.

## Cos'è

Una cartella — `app-web/trusted-signer` nel repository — che è insieme una pagina web
e un'estensione Chrome. Solo HTML, CSS e JavaScript: nessun framework, nessun
bundler, nessuno step di build, nessuna dipendenza e nessuna richiesta di rete per
conto proprio.

Quando riceve una richiesta di firma, non si fida del riassunto che l'accompagna.
Decodifica da sola la calldata grezza, calcola il proprio digest, ti mostra cosa
autorizzerà davvero la firma e solo allora chiede la tua passkey.

Poiché non c'è uno step di build, i file che leggi sono i file che girano. Puoi
confrontare la cartella con il repository e sapere cosa stai servendo.

## Quale copia può firmare per il tuo wallet

Una passkey è legata al dominio su cui è stata creata. Le tue chiavi Vela sono
registrate sotto `getvela.app`, e un browser le offrirà solo a una pagina la cui
relying party è `getvela.app`. Questa sola regola decide quale modo di far girare
una tua copia ti serve davvero.

**Come estensione Chrome — è questa da usare con il wallet che già hai.** La
relying party dell'estensione è `getvela.app` indipendentemente da dove arrivi la
cartella, quindi le tue chiavi esistenti possono firmare lì dentro, mentre il codice
è la cartella che hai caricato e ispezionato.

1. Apri `chrome://extensions` e attiva la **Modalità sviluppatore**.
2. **Carica estensione non pacchettizzata** e scegli la cartella
   `app-web/trusted-signer`.
3. L'icona nella barra degli strumenti apre la pagina in una scheda.

**Come pagina su un tuo dominio, o su localhost.** Servita via HTTP(S), la relying
party della pagina è il suo stesso hostname: può quindi firmare con chiavi
registrate sotto *quell'*hostname, non con chiavi registrate sotto `getvela.app`.
È il modo giusto per provare l'intera cerimonia da capo a fondo, per far girare il
flusso desktop e per firmare per un wallet la cui chiave è stata creata sul tuo
dominio. Non è un modo per firmare per un wallet `getvela.app` esistente.

```sh
cd app-web/trusted-signer
python3 -m http.server 8080   # → http://localhost:8080
```

Tutti i percorsi nell'app sono relativi, quindi funziona anche una sottocartella su
un host esistente; aprire `index.html` direttamente da disco (`file://`) va bene per
dare un'occhiata — senza origine non c'è relying party e non si può firmare nulla.

## Cosa fa prima di firmare

- **Decodifica la transazione da sola.** Cosa fa la chiamata, a chi e per quanto,
  dalla calldata — comprese le chiamate annidate dentro un batch.
- **Firma solo un digest che ha calcolato lei.** I digest EIP-191, EIP-712, SafeOp
  e SafeMessage sono calcolati nella pagina e confrontati con `vela-core`, lo stesso
  codice usato dal wallet. Un digest che non riesce a calcolare è un rifiuto, non
  una firma.
- **Verifica che la transazione sia quella richiesta.** La chiamata chiesta dal
  sito deve trovarsi davvero dentro l'operazione che viene firmata.
- **Rifiuta un'approvazione illimitata.** Non un avviso — un rifiuto, con
  l'indicazione di cosa fare invece.
- **Dice quando non riesce a leggere qualcosa,** invece di mostrare un riassunto
  gentile di cui non può rispondere.
- **Mostra indirizzo e identicon dell'account** e non mostra un nome del
  destinatario fornito da chi ha chiesto la firma. Tutto ciò che il richiedente
  controlla viene tolto oppure etichettato come suo.

## Cosa non ha, di proposito

- **Nessun editor.** La richiesta è fissata all'arrivo: o la firmi o no. Un
  selettore di commissioni o un editor di allowance riscriverebbero la calldata, che
  è esattamente la malattia che questa pagina esiste per prevenire.
- **Nessuna creazione di chiavi.** La pagina di firma non può creare una passkey.
  Crearne una significherebbe creare un account diverso.
- **Nessuna richiesta di rete.** Se non c'è nulla da scaricare, non c'è nulla da
  intercettare.

## Come le arriva una richiesta

| Richiedente | Canale |
| -------------------------------------------- | -------------------------------------------------------------------------- |
| Una pagina nello stesso browser | `postMessage` |
| Una pagina dello stesso browser, verso l'estensione | Porta dell'estensione |
| Un'app desktop sulla stessa macchina | Frammento URL + callback in loopback |
| Un telefono o un altro computer | Bluetooth LE (protocollo implementato; la radio non è ancora testata su hardware reale) |

Il formato di scambio, i digest e una tabella su da dove viene ogni elemento a
schermo sono in `PROTOCOL.md`, accanto al codice.

## Quando usarla

Dal giorno in cui l'account tiene soldi che ti dispiacerebbe perdere — e da lì in
poi, per ogni firma. Non solo per importi grandi: una piccola approvazione può
cedere abbastanza da svuotare un account. Un'abitudine di firma tenuta per le
occasioni speciali non è in piedi il giorno in cui serve.
