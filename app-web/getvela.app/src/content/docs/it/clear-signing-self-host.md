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

**Stato:** costruita e testata in locale; **non pubblicata**, e **nessuna app Vela
le invia ancora richieste**. Oggi è qualcosa da leggere, eseguire e provare con i
richiedenti di esempio nella sua cartella `samples/`. Per usarla per firme vere
serve che le app le inoltrino le loro richieste, e questa parte è ancora da
costruire.

## Cos'è

Una cartella — `app-web/clearsigning` nel repository — che è insieme una pagina
web e un'estensione Chrome. Solo HTML, CSS e JavaScript: nessun framework, nessun
bundler, nessuno step di build, nessuna dipendenza e nessun dato scaricato da un
server — l'unica richiesta che fa è per i loghi decorativi dei token.

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

**Come estensione Chrome — il modo per usarla con le chiavi che hai già.** La
relying party dell'estensione è `getvela.app` indipendentemente da dove arrivi la
cartella, quindi le tue chiavi esistenti possono firmare lì dentro, mentre il
codice è la cartella che hai caricato e ispezionato.

1. Scarica la cartella: `git clone https://github.com/mondaylabsltd/vela-wallet`
   (dentro c'è `app-web/clearsigning`).
2. Apri `chrome://extensions` e attiva la **Modalità sviluppatore**.
3. **Carica estensione non pacchettizzata** e scegli la cartella
   `app-web/clearsigning`.
4. L'icona nella barra degli strumenti apre la pagina in una scheda.

**Come pagina su un tuo dominio, o su localhost.** Servita in HTTPS (o da
localhost), la relying party della pagina è il suo stesso hostname: può quindi
firmare con chiavi registrate sotto _quell'_ hostname, non con chiavi registrate
sotto `getvela.app`. È il modo giusto per provare l'intera procedura da capo a
fondo, per far girare il flusso desktop e per firmare per un wallet la cui chiave è
stata creata sul tuo dominio. Non è un modo per firmare per un wallet
`getvela.app` esistente.

```sh
cd app-web/clearsigning
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
- **Rifiuta un'approvazione di livello «illimitato».** Non un avviso — un rifiuto,
  con l'indicazione di cosa fare invece.
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
