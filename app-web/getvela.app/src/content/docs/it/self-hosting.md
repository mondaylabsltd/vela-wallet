---
title: Guida al self-hosting
description: "Tutto ciò che Vela gestisce per te, a cosa serve ogni parte e come sostituirla con la tua — il relay, l'indice delle chiavi pubbliche, i dati delle chain, i tassi di cambio e le app — più l'unica cosa che non puoi sostituire e come fare a meno di getvela.app."
source: de484cb33065
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Guida al self-hosting

I tuoi soldi sono in un contratto Safe on-chain, controllato dalle tue chiavi.
Niente di ciò che Vela gestisce può spostarli. Quello che Vela gestisce è il
meccanismo che rende comodo il wallet: un relay che invia le tue transazioni, un
indice che aiuta un nuovo dispositivo a trovare il tuo wallet, un archivio di dati
delle chain, un servizio di tassi di cambio e le app stesse.

Questa pagina elenca ognuna di queste parti, cosa smette di funzionare senza di
essa e come gestirne una tua. Parla anche dell'unica parte che non puoi sostituire
— il dominio a cui appartengono le tue passkey — e di cosa fare se getvela.app
sparisce.

<Callout type="info" title="A chi è rivolta questa pagina">
Dovresti saper usare un terminale, Docker o Cloudflare Workers, e sapere come
finanziare un indirizzo su una chain. Niente di tutto questo serve per usare Vela
ogni giorno.
</Callout>

## La mappa

| Parte | Cosa fa | Predefinito di Vela | Puoi sostituirla? | Senza di essa |
| --- | --- | --- | --- | --- |
| **Relay** | Riceve la tua operazione firmata, paga il gas, la invia e incassa la commissione che hai firmato | `vela-relay-cf.getvela.app` | Sì — gestisci [vela-relay](#relay) e indirizza lì il wallet | Non puoi inviare |
| **Indice delle chiavi pubbliche** | Registra on-chain le chiavi di un nuovo wallet; risponde a «di quale wallet fa parte questa chiave?» | `p256-index-v2.getvela.app` | Sì — gestisci [p256-index](#index) | Non si possono creare nuovi wallet; l'accesso ripiega sulla lettura della chain |
| **Contratto di registro** | Il record pubblico e permanente delle chiavi di ogni wallet | `0x94fD…1EA9` su Gnosis | Non serve — nessuno ne è proprietario; il wallet lo legge direttamente | — |
| **Dati delle chain** | Dettagli delle reti, elenchi di token, loghi, descrittori per la firma leggibile | `ethereum-data.getvela.app` | Sì — gestisci [ethereum-data](#chain-data) | Niente elenchi di token né loghi; meno transazioni decodificate; l'aggiunta di reti non funziona |
| **Tassi di cambio** | Valori in valuta fiat nella tua valuta di visualizzazione | `vela-currency.getvela.app` | Sì — gestisci [vela-currency](#exchange-rates) o qualsiasi fonte compatibile con Frankfurter | Le app ripiegano, dove possono, sui tassi on-chain di Chainlink (il desktop mostra USD) |
| **Nodi RPC** | Lettura dei saldi, simulazione delle transazioni | Endpoint pubblici per ogni rete | Sì — rete per rete, in Impostazioni → Reti | Vela passa da un endpoint all'altro |
| **Le app** | Il wallet stesso | wallet.getvela.app, build di release | Sì — [compilale](#web-app) | — |
| **getvela.app** | Il dominio a cui appartengono le tue passkey | — | **No** — vedi [sotto](#if-getvela-app-disappears) | — |

Vengono contattati anche alcuni servizi di terze parti che non sono di Vela: i
database pubblici di selettori di funzione (sourcify, openchain, 4byte), usati
come ultima risorsa per decodificare una transazione, l'elenco di autenticatori
che dà il nome al modello della tua chiave di sicurezza, e i server tunnel di
Apple e Google quando firmi con un telefono scansionando un codice QR.

## L'unica cosa che non puoi sostituire: il dominio della passkey

<span id="if-getvela-app-disappears"></span>

Una passkey appartiene al sito web per cui è stata creata. Le chiavi di Vela sono
create per `getvela.app`. I browser le offrono solo alle pagine di getvela.app o
dei suoi sottodomini (o alle origini che getvela.app dichiara correlate), e le
passkey integrate di un telefono funzionano solo nelle app garantite da
getvela.app. Fuori dal browser la regola è meno rigida: Chrome permette di usarle
a un'estensione che ha il permesso per getvela.app, e un programma sul tuo
computer può chiedere una firma getvela.app direttamente a una chiave di sicurezza
o a un telefono — ed è così che funzionano le app compilate da te, e il motivo per
cui conta quale software usi. Ne seguono due cose.

**Una copia del wallet web sul tuo dominio è un wallet diverso.** Servito da
`wallet.example.com`, lo stesso codice crea passkey per `wallet.example.com` —
chiavi nuove, e quindi un indirizzo nuovo. Non può firmare per un wallet creato su
wallet.getvela.app. Quella copia resta utile: per un wallet che crei lì, o per
gestire in proprio tutto lo stack partendo da zero.

**Per un wallet esistente, queste strade funzionano anche se getvela.app è offline
o non esiste più:**

| Strada | Chiavi che può usare | Dove trovarla |
| --- | --- | --- |
| L'**estensione Vela per il browser** (browser Chromium: Chrome, Edge, Brave) | Qualsiasi chiave raggiungibile dal browser: la passkey di questo dispositivo, una chiave di sicurezza USB (NFC dove il computer lo supporta), un telefono tramite QR | Uno zip di release da [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases), oppure [compilala](#web-app) |
| Un'**app desktop o per telefono compilata da te** | Un telefono tramite QR e le chiavi di sicurezza USB | [Compilala](#web-app) |
| Le **app degli store e le app desktop notarizzate** | Sempre un telefono tramite QR e le chiavi di sicurezza; le passkey di «questo dispositivo» solo finché il sistema operativo riesce ancora a verificare l'app rispetto a getvela.app | Release su GitHub (più avanti gli store) |

L'estensione può usare le chiavi di `getvela.app` perché Chrome permette a
un'estensione con il permesso per un sito di usare le passkey di quel sito. Il
browser controlla quel permesso in locale; lo abbiamo verificato in funzione, ma
non ancora con il dominio realmente offline. Un'app compilata da te può usare un
telefono o una chiave di sicurezza perché Vela comunica con loro direttamente; la
passkey del telefono stesso («questo dispositivo») richiede che l'app sia firmata
da Vela, e la tua non lo è.

La [pagina di firma](/it/docs/clear-signing-self-host) non è di per sé una strada
d'accesso: firma le richieste che le invia un altro programma, e nessuna app Vela
gliele invia ancora.

<Callout type="warning" title="Chi controlla il dominio può chiedere una firma">
Qualsiasi pagina servita da getvela.app o da uno dei suoi sottodomini — o da chi
controllerà il dominio in futuro — può chiedere una firma alle tue chiavi, e la
richiesta di sistema mostra «getvela.app», non la transazione. È così che
funzionano le passkey ovunque. Per questo il sito di Vela vieta alle proprie pagine
di usare le passkey. Ed è anche per questo che contano l'estensione e le app
compilate da te: portano con sé il proprio codice, anche se per impostazione
predefinita scaricano comunque i descrittori e usano i servizi sotto getvela.app.
</Callout>

## Indirizza il wallet verso i tuoi servizi

Ogni app ha quattro campi in **Impostazioni → Avanzate → Endpoint dei servizi**
(sul desktop, **Impostazioni → Endpoint dei servizi**): indice dati delle reti,
indice passkey, Vela Relay e tassi fiat. Ogni campo mostra il valore predefinito di
Vela finché non lo cambi; **Ripristina i valori predefiniti** li riporta tutti e
quattro all'origine. Per relay, indice e dati delle chain, il wallet chiama
`/api/health` e mostra un indicatore, verde solo quando l'endpoint dichiara il
servizio giusto e risponde `status: "ok"`. Salva quello che scrivi in ogni caso —
aspetta il verde.

| Servizio | `service` in `/api/health` |
| --- | --- |
| Relay | `vela-relay` |
| Indice delle chiavi pubbliche | `webauthn-p256-publickey-registry` |
| Dati delle chain | `ethereum-data` |
| Tassi di cambio | non verificato per nome — deve restituire un elenco di tassi con base USD |

Quanto ogni app rispetta oggi queste impostazioni:

| App | Endpoint dei servizi | RPC per rete |
| --- | --- | --- |
| Web ed estensione | Dati delle chain, relay e tassi fiat. L'indice passkey si usa per cercare i nomi, ma la creazione del wallet e l'accesso usano ancora l'indice di Vela | Sì |
| Desktop | Tutti e quattro; un nuovo indice passkey ha effetto dopo il riavvio o dopo essere uscito dall'account | Sì |
| Android | Tutti e quattro, tranne la ricerca dei nomi degli indirizzi, che interroga ancora l'indice di Vela | Sì |
| iOS | **Non ancora**: la pagina mostra valori segnaposto e non salva. L'indice passkey si può cambiare nella schermata di accesso quando quello predefinito non è raggiungibile | Sola lettura |

Queste lacune sono bug, e sono tracciate.

## Gestisci il tuo relay

<span id="relay"></span>

Il relay è [vela-relay](https://github.com/mondaylabsltd/vela-relay) (Rust, MIT).
Un solo deploy serve tutte le chain: il wallet chiama
`https://your-relay/<chainId>`. Deve essere vela-relay: il wallet chiede la
quotazione della commissione con un metodo specifico di Vela che i bundler
ERC-4337 generici non implementano.

**Cosa ti serve**

- Docker più un server Redis e un server [Iggy](https://iggy.apache.org) che già
  gestisci, oppure un account Cloudflare con il piano **Workers Paid** e, sulla tua
  macchina, Node.js e una toolchain Rust (con il target
  `wasm32-unknown-unknown`).
- Un `OPERATOR_SECRET` (esadecimale, almeno 32 byte). Da esso derivano un
  indirizzo di tesoreria e un pool di indirizzi relayer, uguali su ogni chain.
  Tienilo segreto: controlla i fondi del relay.
- Gas su ogni chain che vuoi servire: invia la moneta della chain (pathUSD su
  Tempo) al tuo indirizzo di tesoreria. La tesoreria ricarica i relayer.

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# nel file .env: VELA_RELAY_IGGY_URL, VELA_RELAY_REDIS_URL, OPERATOR_SECRET,
# VELA_RELAY_CHAIN_DIRECTORY_URL se gestisci i tuoi dati delle chain,
# e VELA_RELAY_IMAGE impostato su un'immagine di release di cui ti fidi (vedi docs/docker.md)
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

Meglio usare l'immagine pubblicata: compilare dal codice sorgente con
`docker compose up --build` può fallire con il Dockerfile attuale. Senza Docker,
`cargo run --release --bin vela-relay` lo esegue direttamente.

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# se gestisci i tuoi dati delle chain: aggiungi "VELA_RELAY_CHAIN_DIRECTORY_URL" sotto "vars" in wrangler.jsonc
npx wrangler deploy
```

**Verifica**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # il tuo indirizzo di tesoreria su Gnosis, e se ha bisogno di gas
```

Poi inserisci `https://your-relay` nel campo **Vela Relay**.

**Da sapere**

- La commissione pagata dal wallet va alla tua tesoreria. Il wallet la calcola nello
  stesso modo qualunque relay tu usi (vedi [reti e commissioni](/it/docs/networks-and-fees)).
- Una rete personalizzata aggiunta prima di cambiare relay mantiene l'indirizzo del
  relay con cui è stata aggiunta.
- Il relay legge i dettagli di ogni chain e le stablecoin che accetta da un archivio
  delle chain: `ethereum-data.getvela.app`, a meno che tu non imposti
  `VELA_RELAY_CHAIN_DIRECTORY_URL` su [un archivio tuo](#chain-data). L'impostazione
  è arrivata a settembre 2026; le build precedenti del relay leggono sempre la copia
  di Vela.

## Gestisci il tuo indice delle chiavi pubbliche

<span id="index"></span>

L'indice è [p256-index](https://github.com/mondaylabsltd/p256-index) (Rust).
Quando viene creato un wallet, controlla la prova di ogni chiave, poi scrive il
gruppo nel **contratto di registro** su Gnosis e paga il gas. Continua a usare il
registro esistente a `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`: non ha un
proprietario, qualsiasi indirizzo con fondi può scriverci e ogni app Vela lo legge
direttamente. Un registro tutto tuo sarebbe invisibile alle app.

**Cosa ti serve**

- Docker con Redis e Iggy (il server), oppure un account Cloudflare (la versione
  Worker, il cui README avverte che la scrittura on-chain non è ancora stata
  testata da capo a fondo).
- Una chiave privata Gnosis con dello xDAI. Registrare un wallet costa circa 1,1
  milioni di gas con una chiave e circa 3,6 milioni con sette.
- Queste impostazioni:

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

`P256_INDEX_DOMAIN_REGISTRY` è indispensabile anche se il file di esempio del
server non lo riporta: senza, il server emette challenge che il contratto rifiuta,
e ogni registrazione fallisce.

**Avvio e verifica**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

Il server ascolta in HTTP semplice (porta 11256 per impostazione predefinita);
mettigli davanti un proxy TLS, perché il wallet accetta solo endpoint `https://`.
Il Dockerfile nel codice sorgente, al momento in cui scriviamo, potrebbe non
compilare; con Cargo la compilazione funziona. Il repository non ha ancora un file
di licenza.

**Se non risponde nessun indice**, i wallet esistenti funzionano comunque: in fase
di accesso l'app legge il contratto di registro su Gnosis (poi su Ethereum)
tramite i tuoi nodi RPC. Un wallet con una sola chiave si può perfino ricostruire
da due firme, senza alcun registro. Per creare un nuovo wallet invece serve un
indice, perché qualcuno deve pagare la registrazione.

## Gestisci i tuoi dati delle chain

<span id="chain-data"></span>

I dati delle chain sono [ethereum-data](https://github.com/atshelchin/ethereum-data)
(MIT): JSON statici e immagini per circa 2.600 reti e i loro token, più i
descrittori ERC-7730 che Vela usa per spiegare le transazioni.

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

Il suo README spiega anche come compilarlo dal codice sorgente e come fare il
deploy su Cloudflare. Servilo in HTTPS e inserisci l'indirizzo nel campo
**Indice dati delle reti**.

Anche il relay legge questi file, compreso un campo specifico di Vela (l'elenco
`stables` decide quali stablecoin possono pagare le commissioni). Indirizzalo verso
la tua copia con `VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data`; mette in
cache la voce di ogni rete per un'ora.

## Gestisci i tuoi tassi di cambio

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency) (MIT) ripubblica
i tassi giornalieri della Banca centrale europea. Non richiede chiavi.

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

Inserisci `https://your-host/v2/rates?base=USD` nel campo **Tassi fiat**. Va bene
anche qualsiasi servizio compatibile con Frankfurter. Mantieni `?base=USD`: ogni
conversione lo dà per scontato.

## Compila le app da te

<span id="web-app"></span>

Tutte le app sono in [un unico repository](https://github.com/mondaylabsltd/vela-wallet)
(MIT). Il README elenca i passaggi di compilazione di ogni app; in breve:

| App | Compilazione | Firma per il tuo wallet getvela.app esistente? |
| --- | --- | --- |
| Estensione del browser | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`, poi carica `extension/dist` come estensione non pacchettizzata in `chrome://extensions` | Sì, con qualsiasi chiave |
| Wallet web | `cd app-web/vela-wallet && pnpm install && pnpm build`; si pubblica come Cloudflare Worker | No — sul tuo dominio è un wallet diverso (vedi sopra) |
| Desktop | `cd app-desktop/vela-wallet && cargo run` (script di packaging nel suo README) | Sì, con un telefono tramite QR o una chiave di sicurezza USB |
| Android | Genera i binding del core, poi `./gradlew :app:installDebug` | Sì, con un telefono tramite QR o una chiave di sicurezza USB |
| iOS | `./rust/scripts/build-ios-xcframework.sh`, poi compila in Xcode con il tuo team | Sì, con un telefono tramite QR o una YubiKey USB-C / Lightning (firmware 5.8 o successivo) |

Con un'app compilata da te, la passkey di «questo dispositivo» non funziona per i
wallet getvela.app: Apple e Google permettono di usare le passkey di `getvela.app`
solo alle app firmate da Vela.

## Aggiungi una rete che Vela non include

Vela gira su qualsiasi chain EVM che abbia il precompilato P-256 e i contratti
standard che controlla. La [configurazione della chain](/it/chain-setup) ti dice
cosa manca a una chain e fa il deploy di ciò che chiunque può deployare;
[reti e commissioni](/it/docs/networks-and-fees) spiega i requisiti. Una lacuna:
un wallet con più di una chiave ha bisogno anche della factory dei firmatari
passkey di Safe su quella chain, che il controllo non verifica ancora — senza di
essa, lì può firmare solo la prima chiave.

## Cosa punta ancora a Vela dopo tutto questo

Se sostituisci tutto quanto sopra, restano:

- **L'elenco di autenticatori** che dà il nome ai modelli di chiave di sicurezza —
  solo estetico; le app ripiegano su un nome generico.
- **I file di associazione di getvela.app**, che servono alle app degli store per
  le passkey di «questo dispositivo». A un telefono o a una chiave di sicurezza non
  servono.

E questi non sono di Vela: i database pubblici di selettori, i tunnel di Apple e
Google per l'accesso tramite telefono, e i provider RPC che scegli.

Poi: [la pagina di firma che puoi gestire tu](/it/docs/clear-signing-self-host).
