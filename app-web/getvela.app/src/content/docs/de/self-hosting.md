---
title: Anleitung zum Selbsthosten
description: "Alles, was Vela für dich betreibt, was jedes Teil tut und wie du es durch dein eigenes ersetzt – das Relay, den Public-Key-Index, Chain-Daten, Wechselkurse und die Apps –, dazu das eine, was sich nicht ersetzen lässt, und wie du ohne getvela.app auskommst."
source: de484cb33065
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Anleitung zum Selbsthosten

Dein Geld liegt in einem Safe-Vertrag on-chain und wird von deinen Schlüsseln
gesteuert. Nichts, was Vela betreibt, kann es bewegen. Was Vela betreibt, ist die
Technik, die die Wallet bequem macht: ein Relay, das deine Transaktionen einreicht, ein
Index, der einem neuen Gerät hilft, deine Wallet zu finden, ein Verzeichnis mit
Chain-Daten, ein Wechselkurs-Feed und die Apps selbst.

Diese Seite listet jedes dieser Teile auf, was ohne es nicht mehr geht und wie du dein
eigenes betreibst. Sie behandelt auch das eine Teil, das du nicht ersetzen kannst – die
Domain, zu der deine Passkeys gehören –, und was zu tun ist, wenn getvela.app
verschwindet.

<Callout type="info" title="Für wen diese Seite ist">
Du solltest mit einem Terminal, mit Docker oder Cloudflare Workers und mit dem
Aufladen einer Adresse auf einer Chain vertraut sein. Für die alltägliche Nutzung von
Vela brauchst du nichts davon.
</Callout>

## Die Übersicht

| Teil | Was es tut | Velas Standard | Ersetzbar? | Ohne es |
| --- | --- | --- | --- | --- |
| **Relay** | Nimmt deine signierte Operation entgegen, zahlt das Gas, reicht sie ein und kassiert die Gebühr, die du signiert hast | `vela-relay-cf.getvela.app` | Ja – betreibe [vela-relay](#relay) und stelle die Wallet darauf um | Du kannst nicht senden |
| **Public-Key-Index** | Registriert die Schlüssel einer neuen Wallet on-chain; beantwortet die Frage „Zu welcher Wallet gehört dieser Schlüssel?“ | `p256-index-v2.getvela.app` | Ja – betreibe [p256-index](#index) | Neue Wallets lassen sich nicht erstellen; die Anmeldung liest ersatzweise direkt von der Chain |
| **Registervertrag** | Der dauerhafte öffentliche Eintrag der Schlüssel jeder Wallet | `0x94fD…1EA9` auf Gnosis | Nicht nötig – er gehört niemandem; die Wallet liest ihn direkt | — |
| **Chain-Daten** | Netzwerkdetails, Token-Listen, Logos, Deskriptoren für die Klartext-Signatur | `ethereum-data.getvela.app` | Ja – betreibe [ethereum-data](#chain-data) | Keine Token-Listen oder Logos; weniger Transaktionen dekodiert; Netzwerke hinzufügen scheitert |
| **Wechselkurse** | Fiat-Werte in deiner Anzeigewährung | `vela-currency.getvela.app` | Ja – betreibe [vela-currency](#exchange-rates) oder eine andere Frankfurter-kompatible Quelle | Die Apps weichen, wo möglich, auf Chainlink-Kurse on-chain aus (der Desktop zeigt USD) |
| **RPC-Knoten** | Guthaben lesen, Transaktionen simulieren | Öffentliche Endpunkte pro Netzwerk | Ja – pro Netzwerk, unter Einstellungen → Netzwerke | Vela wechselt zwischen Endpunkten |
| **Die Apps** | Die Wallet selbst | wallet.getvela.app, Release-Builds | Ja – [selbst bauen](#web-app) | — |
| **getvela.app** | Die Domain, zu der deine Passkeys gehören | — | **Nein** – siehe [unten](#if-getvela-app-disappears) | — |

Außerdem werden einige Dienste Dritter kontaktiert, die nicht zu Vela gehören: die
öffentlichen Funktionsselektor-Datenbanken (sourcify, openchain, 4byte), die beim
Dekodieren einer Transaktion als letzter Ausweg dienen, das Authentifikator-Verzeichnis,
das das Modell deines Sicherheitsschlüssels benennt, und die Tunnel-Server von Apple
und Google, wenn du mit einem Handy per QR-Code signierst.

## Das eine, was du nicht ersetzen kannst: die Domain des Passkeys

<span id="if-getvela-app-disappears"></span>

Ein Passkey gehört zu der Website, für die er erstellt wurde. Velas Schlüssel werden
für `getvela.app` erstellt. Browser bieten sie nur Seiten auf getvela.app oder ihren
Subdomains an (oder Origins, die getvela.app als verwandt deklariert), und die
eingebauten Passkeys eines Handys funktionieren nur in Apps, für die getvela.app
bürgt. Außerhalb des Browsers ist die Regel lockerer: Chrome lässt eine Erweiterung mit
Berechtigung für getvela.app sie nutzen, und ein Programm auf deinem Computer kann einen
Sicherheitsschlüssel oder ein Handy direkt um eine getvela.app-Signatur bitten – so
funktionieren selbst gebaute Apps, und deshalb kommt es darauf an, welche Software du
ausführst. Daraus folgen zwei Dinge.

**Eine Kopie der Web-Wallet auf deiner eigenen Domain ist eine andere Wallet.** Unter
`wallet.example.com` ausgeliefert, erstellt derselbe Code Passkeys für
`wallet.example.com` – neue Schlüssel und damit eine neue Adresse. Sie kann nicht für
eine Wallet signieren, die auf wallet.getvela.app erstellt wurde. Nützlich ist die
Kopie trotzdem: für eine Wallet, die du dort erstellst, oder um den ganzen Stack von
Grund auf selbst zu betreiben.

**Für eine bestehende Wallet funktionieren diese Wege weiter, wenn getvela.app offline
oder verschwunden ist:**

| Zugang | Schlüssel, die er nutzen kann | Woher du ihn bekommst |
| --- | --- | --- |
| Die **Vela-Browser-Erweiterung** (Chromium-Browser: Chrome, Edge, Brave) | Jeden Schlüssel, den der Browser erreicht: den Passkey dieses Geräts, einen USB-Sicherheitsschlüssel (NFC, wo der Computer es unterstützt), ein Handy per QR | Ein Release-ZIP von [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases) oder [selbst bauen](#web-app) |
| Eine **selbst gebaute Desktop- oder Handy-App** | Ein Handy per QR und USB-Sicherheitsschlüssel | [Selbst bauen](#web-app) |
| Die **Store-Apps und die notarisierten Desktop-Apps** | Ein Handy per QR und Sicherheitsschlüssel immer; Passkeys auf „diesem Gerät“ nur, solange das Betriebssystem die App noch gegen getvela.app prüfen kann | GitHub-Releases (Stores später) |

Die Erweiterung kann `getvela.app`-Schlüssel nutzen, weil Chrome einer Erweiterung mit
Berechtigung für eine Website erlaubt, die Passkeys dieser Website zu verwenden. Der
Browser prüft diese Berechtigung lokal; wir haben nachgemessen, dass das funktioniert,
allerdings noch nicht bei einer Domain, die tatsächlich offline ist. Eine selbst
gebaute App kann ein Handy oder einen Sicherheitsschlüssel nutzen, weil Vela direkt mit
ihnen spricht; der eigene Passkey des Handys („Dieses Gerät“) setzt voraus, dass die
App von Vela signiert ist, und deine ist es nicht.

Die [Signaturseite](/de/docs/clear-signing-self-host) ist für sich allein kein
Zugang: Sie signiert Anfragen, die ihr ein anderes Programm schickt, und noch keine
Vela-App schickt welche.

<Callout type="warning" title="Wer die Domain kontrolliert, kann eine Signatur anfordern">
Jede Seite, die von getvela.app oder einer ihrer Subdomains ausgeliefert wird – oder
künftig von wem auch immer, der die Domain kontrolliert –, kann deine Schlüssel um eine
Signatur bitten, und die Systemabfrage zeigt „getvela.app“, nicht die Transaktion. So
funktionieren Passkeys überall. Aus diesem Grund verbietet die Website von Vela ihren
eigenen Seiten, Passkeys zu nutzen. Deshalb sind auch die Erweiterung und selbst
gebaute Apps wichtig: Sie bringen ihren eigenen Code mit, rufen standardmäßig aber
trotzdem Deskriptoren ab und nutzen Dienste unter getvela.app.
</Callout>

## Die Wallet auf deine Dienste umstellen

Jede App hat unter **Einstellungen → Erweitert → Dienst-Endpunkte** (auf dem Desktop
**Einstellungen → Dienst-Endpunkte**) vier Felder: Chain-Daten-Index, Passkey-Index,
Vela Relay und Fiat-Kurse. Jedes Feld zeigt Velas Standardwert, bis du es änderst;
**Auf Standard zurücksetzen** stellt alle vier wieder her. Für Relay, Index und
Chain-Daten ruft die Wallet `/api/health` auf und zeigt eine Statusanzeige, die nur
grün wird, wenn der Endpunkt den richtigen Dienst nennt und `status: "ok"` meldet.
Gespeichert wird, was du eingibst, in jedem Fall – warte also, bis die Anzeige grün
ist.

| Dienst | `service` in `/api/health` |
| --- | --- |
| Relay | `vela-relay` |
| Public-Key-Index | `webauthn-p256-publickey-registry` |
| Chain-Daten | `ethereum-data` |
| Wechselkurse | nicht nach Namen geprüft – muss eine USD-basierte Kursliste liefern |

Wie gut jede App diese Einstellungen heute berücksichtigt:

| App | Dienst-Endpunkte | RPC pro Netzwerk |
| --- | --- | --- |
| Web und Erweiterung | Chain-Daten, Relay und Fiat-Kurse. Der Passkey-Index wird zum Nachschlagen von Namen genutzt, aber das Erstellen einer Wallet und die Anmeldung nutzen weiterhin Velas Index | Ja |
| Desktop | Alle vier; ein neuer Passkey-Index greift nach einem Neustart oder Abmelden | Ja |
| Android | Alle vier, nur fragt das Nachschlagen von Namen für Adressen weiterhin Velas Index | Ja |
| iOS | **Noch nicht**: Die Seite zeigt Platzhalterwerte und speichert nicht. Der Passkey-Index lässt sich auf dem Anmeldebildschirm ändern, wenn der Standard nicht erreichbar ist | Nur lesend |

Diese Lücken sind Bugs, und sie sind erfasst.

## Ein eigenes Relay betreiben

<span id="relay"></span>

Das Relay ist [vela-relay](https://github.com/mondaylabsltd/vela-relay) (Rust, MIT).
Eine Bereitstellung bedient alle Chains: Die Wallet ruft
`https://your-relay/<chainId>` auf. Es muss vela-relay sein – die Wallet fragt das
Gebührenangebot über eine Vela-spezifische Methode ab, die allgemeine ERC-4337-Bundler
nicht implementieren.

**Was du brauchst**

- Entweder Docker plus einen Redis- und einen [Iggy](https://iggy.apache.org)-Server,
  die du bereits betreibst, oder ein Cloudflare-Konto mit **Workers Paid** sowie
  Node.js und eine Rust-Toolchain (mit dem Target `wasm32-unknown-unknown`) auf deinem
  Rechner.
- Ein `OPERATOR_SECRET` (hex, mindestens 32 Byte). Daraus werden eine Treasury-Adresse
  und ein Pool von Relayer-Adressen abgeleitet, auf jeder Chain dieselben. Halte es
  geheim: Es kontrolliert das Geld des Relays.
- Gas auf jeder Chain, die du bedienen willst: Schick den Coin der Chain (auf Tempo
  pathUSD) an deine Treasury-Adresse. Die Treasury füllt die Relayer auf.

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# in .env setzen: VELA_RELAY_IGGY_URL, VELA_RELAY_REDIS_URL, OPERATOR_SECRET,
# VELA_RELAY_CHAIN_DIRECTORY_URL, falls du eigene Chain-Daten betreibst,
# und VELA_RELAY_IMAGE auf ein Release-Image, dem du vertraust (siehe docs/docker.md)
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

Nimm lieber das veröffentlichte Image: Aus dem Quellcode mit
`docker compose up --build` zu bauen, kann mit dem aktuellen Dockerfile fehlschlagen.
Ohne Docker startet `cargo run --release --bin vela-relay` es direkt.

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# eigene Chain-Daten: "VELA_RELAY_CHAIN_DIRECTORY_URL" unter "vars" in wrangler.jsonc eintragen
npx wrangler deploy
```

**Prüfen**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # deine Treasury-Adresse auf Gnosis und ob sie Gas braucht
```

Trag dann `https://your-relay` in das Feld **Vela Relay** ein.

**Gut zu wissen**

- Die Gebühr, die die Wallet zahlt, geht an deine Treasury. Die Wallet berechnet sie
  auf dieselbe Weise, egal welches Relay du nutzt (siehe
  [Netzwerke und Gebühren](/de/docs/networks-and-fees)).
- Ein eigenes Netzwerk, das du vor dem Wechsel des Relays hinzugefügt hast, behält die
  Relay-Adresse, mit der es hinzugefügt wurde.
- Das Relay liest die Details jeder Chain und die Stablecoins, die es akzeptiert, aus
  einem Chain-Verzeichnis: `ethereum-data.getvela.app`, sofern du
  `VELA_RELAY_CHAIN_DIRECTORY_URL` nicht auf [dein eigenes](#chain-data) setzt. Die
  Einstellung gibt es seit September 2026; ein älterer Relay-Build liest immer Velas
  Kopie.

## Einen eigenen Public-Key-Index betreiben

<span id="index"></span>

Der Index ist [p256-index](https://github.com/mondaylabsltd/p256-index) (Rust). Wenn
eine Wallet erstellt wird, prüft er den Nachweis jedes Schlüssels, schreibt die Gruppe
dann in den **Registervertrag** auf Gnosis und zahlt das Gas. Nutze weiter das
bestehende Register unter `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`: Es hat keinen
Eigentümer, jede Adresse mit Guthaben kann hineinschreiben, und jede Vela-App liest es
direkt. Ein eigenes Register wäre für sie unsichtbar.

**Was du brauchst**

- Docker mit Redis und Iggy (der Server) oder ein Cloudflare-Konto (die
  Worker-Version, in deren eigener README steht, dass ihr Schreiben on-chain noch nicht
  durchgängig getestet ist).
- Einen privaten Gnosis-Schlüssel mit xDAI. Eine Wallet zu registrieren kostet mit
  einem Schlüssel etwa 1,1 Mio. Gas und mit sieben etwa 3,6 Mio.
- Diese Einstellungen:

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

`P256_INDEX_DOMAIN_REGISTRY` ist wichtig, auch wenn die Beispieldatei des Servers es
weglässt: Ohne diese Einstellung verteilt der Server Challenges, die der Vertrag
ablehnt, und jede Registrierung schlägt fehl.

**Starten und prüfen**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

Der Server lauscht auf einfachem HTTP (standardmäßig Port 11256); setz einen
TLS-Proxy davor, denn die Wallet akzeptiert nur `https://`-Endpunkte. Das Dockerfile im
Quellcode lässt sich zum Zeitpunkt dieses Textes möglicherweise nicht bauen; mit Cargo
zu bauen funktioniert. Das Repository hat noch keine Lizenzdatei.

**Wenn überhaupt kein Index antwortet**, funktionieren bestehende Wallets trotzdem: Bei
der Anmeldung liest die App den Registervertrag auf Gnosis (dann auf Ethereum) über
deine RPC-Knoten. Eine Wallet mit einem einzigen Schlüssel lässt sich sogar ganz ohne
Register aus zwei Signaturen neu aufbauen. Für das Erstellen einer neuen Wallet braucht
es dagegen einen Index, denn irgendjemand muss die Registrierung bezahlen.

## Eigene Chain-Daten betreiben

<span id="chain-data"></span>

Die Chain-Daten sind [ethereum-data](https://github.com/atshelchin/ethereum-data)
(MIT): statisches JSON und Bilder für rund 2.600 Netzwerke und ihre Token, dazu die
ERC-7730-Deskriptoren, mit denen Vela Transaktionen erklärt.

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

Die README beschreibt auch den Bau aus dem Quellcode und die Bereitstellung auf
Cloudflare. Liefere die Daten über HTTPS aus und trag die Adresse in das Feld
**Chain-Daten-Index** ein.

Auch das Relay liest diese Dateien, darunter ein Vela-spezifisches Feld (die Liste
`stables` entscheidet, welche Stablecoins Gebühren bezahlen können). Mit
`VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data` stellst du es auf deine Kopie
um; den Eintrag jedes Netzwerks speichert es eine Stunde lang zwischen.

## Eigene Wechselkurse betreiben

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency) (MIT) veröffentlicht die
täglichen Kurse der Europäischen Zentralbank neu. Es braucht keine Schlüssel.

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

Trag `https://your-host/v2/rates?base=USD` in das Feld **Fiat-Kurse** ein. Jeder
Frankfurter-kompatible Dienst funktioniert ebenfalls. Behalte `?base=USD` bei: Jede
Umrechnung setzt es voraus.

## Die Apps selbst bauen

<span id="web-app"></span>

Alle Apps liegen in [einem Repository](https://github.com/mondaylabsltd/vela-wallet)
(MIT). Die README nennt die Build-Schritte jeder App; hier die Kurzfassung:

| App | Bauen | Signiert für deine bestehende getvela.app-Wallet? |
| --- | --- | --- |
| Browser-Erweiterung | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`, dann `extension/dist` unter `chrome://extensions` als entpackte Erweiterung laden | Ja, mit jedem Schlüssel |
| Web-Wallet | `cd app-web/vela-wallet && pnpm install && pnpm build`; wird als Cloudflare Worker bereitgestellt | Nein – auf deiner Domain ist es eine andere Wallet (siehe oben) |
| Desktop | `cd app-desktop/vela-wallet && cargo run` (Paketierungsskripte in der README) | Ja, mit einem Handy per QR oder einem USB-Sicherheitsschlüssel |
| Android | Die Core-Bindings erzeugen, dann `./gradlew :app:installDebug` | Ja, mit einem Handy per QR oder einem USB-Sicherheitsschlüssel |
| iOS | `./rust/scripts/build-ios-xcframework.sh`, dann in Xcode mit deinem eigenen Team bauen | Ja, mit einem Handy per QR oder einem YubiKey mit USB-C / Lightning (Firmware 5.8 oder neuer) |

Der Passkey „Dieses Gerät“ funktioniert in einer selbst gebauten App nicht für
getvela.app-Wallets: Apple und Google lassen nur von Vela signierte Apps
`getvela.app`-Passkeys nutzen.

## Ein Netzwerk hinzufügen, das Vela nicht mitbringt

Vela läuft auf jeder EVM-Chain, die das P-256-Precompile und die Standardverträge hat,
auf die es prüft. Die [Chain-Einrichtung](/de/chain-setup) sagt dir, was einer Chain
fehlt, und stellt bereit, was jeder bereitstellen kann;
[Netzwerke und Gebühren](/de/docs/networks-and-fees) erklärt die Anforderungen. Eine
Lücke: Eine Wallet mit mehr als einem Schlüssel braucht auf dieser Chain außerdem die
Passkey-Signer-Factory von Safe, nach der die Prüfung noch nicht sucht – ohne sie kann
dort nur der erste Schlüssel signieren.

## Was danach noch auf Vela zeigt

Wenn du alles oben Genannte ersetzt, bleibt Folgendes:

- **Das Authentifikator-Verzeichnis**, das Modelle von Sicherheitsschlüsseln benennt –
  rein kosmetisch; die Apps weichen auf einen allgemeinen Namen aus.
- **Die Zuordnungsdateien von getvela.app**, die die Store-Apps für Passkeys auf
  „diesem Gerät“ brauchen. Ein Handy oder ein Sicherheitsschlüssel braucht sie nicht.

Und diese gehören nicht zu Vela: öffentliche Selektor-Datenbanken, die Tunnel von Apple
und Google für die Anmeldung per Handy und die RPC-Anbieter, die du wählst.

Weiter: [die Signaturseite, die du selbst betreiben kannst](/de/docs/clear-signing-self-host).
