---
title: Installare Vela
description: "Tutti i modi per usare Vela — web, estensione del browser, desktop e telefono — quanto costa ciascuno, cosa può fare e cosa serve al tuo dispositivo."
source: fa80f5cfdb95
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Installare Vela

Lo stesso wallet gira in più posti, e tutti aprono lo stesso indirizzo con le
stesse chiavi. Scegli in base a ciò che ti serve; puoi usarne più di uno. I
download sono su [Ottieni Vela](/it/get-started).

| | Cos'è | Costo | Stato |
| --- | --- | --- | --- |
| **Web** | [wallet.getvela.app](https://wallet.getvela.app/) in qualsiasi browser recente | Gratuito | Disponibile |
| **Estensione del browser** | Il wallet nella barra degli strumenti; si collega alle dApp | Gratuito | Da scaricare e caricare a mano; non ancora sul Chrome Web Store |
| **Desktop** | App nativa per macOS, Windows e Linux | Gratuito | Da scaricare da Ottieni Vela o da GitHub |
| **iPhone, Android** | App native | Acquisto una tantum negli store | Non ancora negli store; puoi compilarle dal codice sorgente |

<a href="https://wallet.getvela.app/" target="_blank" rel="noopener" style="display:inline-block;margin:4px 0 8px;padding:11px 22px;border-radius:10px;background:#e8572a;color:#fff;font-weight:600;text-decoration:none;">Apri il wallet web →</a>

## Web

Niente da installare. Apri [wallet.getvela.app](https://wallet.getvela.app/), crea
un wallet o accedi, ed eccolo. L'elenco dei tuoi account resta in questo browser;
su un altro dispositivo ti basta accedere di nuovo con una delle tue chiavi.

## Estensione del browser

Funziona nei browser Chromium: Chrome, Edge e Brave (Chrome 116 o successivo).
Mette il wallet nella barra degli strumenti e permette alle dApp di collegarsi
direttamente.
Finché non è sul Chrome Web Store:

1. Scarica l'estensione da [Ottieni Vela](/it/get-started) e decomprimila in una
   cartella che terrai: il browser la esegue da lì.
2. Apri `chrome://extensions` e attiva la **Modalità sviluppatore**.
3. Fai clic su **Carica estensione non pacchettizzata** e scegli quella cartella.

È lo stesso wallet: l'estensione e il wallet web usano le stesse passkey di
`getvela.app`, quindi le stesse chiavi aprono lo stesso indirizzo.

## Desktop

Un'app nativa, non una pagina web dentro una finestra: **Windows** 10 e 11 (x64 e
ARM), **macOS** 11 o successivo e **Linux** (.deb, .rpm o Flatpak, x64 e ARM).

- **Windows** avviserà di aver «protetto il PC», perché il programma di
  installazione non ha ancora una firma del codice. Scegli **Ulteriori
  informazioni**, poi **Esegui comunque**.
- Le build per **macOS** vengono firmate e notarizzate da Apple in un passaggio
  separato, quindi possono arrivare dopo quelle delle altre piattaforme. Quando il
  pulsante Mac dice «A breve», la build Mac notarizzata più recente è sulla pagina
  delle release di GitHub.
- **Linux**: per usare una chiave di sicurezza USB, il sistema deve concedere
  all'app l'accesso al dispositivo — i pacchetti .deb e .rpm installano la regola
  per te.

Su macOS e Windows l'app desktop ha un browser integrato per le dApp. I checksum
di ogni pacchetto sono sulla
[pagina delle release di GitHub](https://github.com/mondaylabsltd/vela-wallet/releases) —
e puoi controllare più di un checksum, vedi qui sotto.

## iPhone e Android

App native per iOS 17.4 o successivo e Android 10 o successivo. Saranno vendute
come acquisto una tantum su App Store e Google Play; **non sono ancora negli
store**. Il codice è aperto, quindi puoi compilarle tu gratis — con una
differenza: una build firmata da te non può usare le passkey del telefono stesso
per i wallet getvela.app, mentre la scansione con un altro telefono e le chiavi di
sicurezza USB funzionano. Vedi [compilare le app da te](/it/docs/self-hosting#web-app).

## Verifica quello che hai scaricato

Un checksum ti dice che due file sono identici. Non può dirti chi ha prodotto il
file — e l'elenco dei checksum sta sulla stessa pagina del download. Per questo
ogni pacchetto che alleghiamo a una release è anche **attestato**: l'esecuzione
del workflow che l'ha compilato firma una dichiarazione che nomina il file, il
commit e l'esecuzione stessa, e GitHub la conserva. Controllarla richiede un solo
comando con la [CLI di GitHub](https://cli.github.com) (accedi una volta con
`gh auth login`; il controllo è gratuito):

```bash
gh attestation verify vela-wallet_0.9.4_amd64.deb --repo mondaylabsltd/vela-wallet
```

Stampa chi ha compilato il file e da quale commit, oppure fallisce. Per quella
risposta nulla sul tuo computer deve fidarsi di noi: la firma è di GitHub, fatta
al momento della compilazione, e non può produrla chi si limita a ricaricare un
file da qualche parte.

Le immagini per Mac sono firmate con il nostro Developer ID e notarizzate da
Apple, cosa che macOS controlla per te quando ne apri una. Per chiederlo tu
stesso:

```bash
xcrun stapler validate VelaWallet-0.9.4-macos-arm64.dmg
spctl -a -t open --context context:primary-signature -v VelaWallet-0.9.4-macos-arm64.dmg
```

<Callout type="warning" title="L'avviso di Windows resta">
L'attestazione non è una firma del codice. Il programma di installazione per
Windows non ha una firma del codice, quindi SmartScreen lo ferma comunque una
volta con «Windows ha protetto il PC» — scegli <strong>Ulteriori
informazioni</strong>, poi <strong>Esegui comunque</strong>. Verificare
l'attestazione è il controllo che ti dice che il file è davvero nostro; l'avviso
riguarda un certificato che non abbiamo comprato.
</Callout>

I pacchetti pubblicati prima che questo venisse attivato hanno solo i loro
checksum.

## Usare Vela con le dApp

<span id="dapps"></span>

Le dApp si collegano a Vela come a qualsiasi wallet del browser (EIP-1193 ed
EIP-6963):

- in un browser desktop, tramite l'**estensione Vela per il browser**;
- dentro l'**app desktop** (macOS, Windows), l'**app iPhone** e l'**app
  Android**, tramite il loro browser integrato.

Il wallet web su wallet.getvela.app non si collega alle dApp, e WalletConnect non
è supportato. Ogni richiesta di una dApp viene decodificata e ti viene mostrata
prima che tu firmi — vedi [firma leggibile](/it/docs/clear-signing).

## Cosa serve al tuo dispositivo

Vela firma con le **passkey**, supportate da quasi tutti i dispositivi degli
ultimi anni:

| Dispositivo | Supporto |
| --- | --- |
| iPhone, iPad, Mac | iOS / iPadOS 16+, macOS con Safari o Chrome recenti |
| Android | Un Android recente con i servizi Google Play, o una chiave di sicurezza USB |
| Windows | Windows Hello con Chrome o Edge, o una chiave di sicurezza |
| Linux | Una chiave di sicurezza, o un telefono vicino (scansiona il codice QR) |

Se il tuo dispositivo non può custodire una passkey, usa un altro telefono o una
chiave di sicurezza hardware. [Firmatari e chiavi di sicurezza](/it/docs/signers)
elenca quali tipi di chiave supporta ogni app.

## Gli unici indirizzi ufficiali

- **getvela.app** — questo sito, e i download
- **wallet.getvela.app** — il wallet web
- **github.com/mondaylabsltd** — il codice e i pacchetti delle release

<Callout type="warning" title="Controlla prima di installare">
Se qualcosa ti manda altrove per «installare Vela» o per «verificare il tuo
wallet», fermati. Vela non chiede mai una seed phrase — non ne ha una.
</Callout>

Poi: [crea il tuo wallet](/it/docs/create-wallet).
