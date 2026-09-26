---
title: Whitepaper
description: "Wie Vela funktioniert und worauf du vertrauen musst – und worauf nicht –, um es zu nutzen: das Konto, die Schlüssel, die Gebühr, das Bedrohungsmodell, die Wiederherstellung und was passiert, wenn Vela verschwindet."
source: d0be29438317
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Stand: Alpha · zuletzt überarbeitet im September 2026">
Diese Seite beschreibt, wie Vela heute funktioniert und worauf du vertrauen musst und
worauf nicht, um es zu nutzen. Vela ist in der
<a href="/blog/vela-is-in-alpha">Alpha-Phase</a> – fang mit kleinen Beträgen an. Vela
hat keinen Token. Alles hier lässt sich am Open-Source-Code überprüfen; wo sich der
Code und diese Seite widersprechen, hat der Code recht, und die Seite hat einen Bug.
</Callout>

## Zusammenfassung

Vela ist eine **Smart-Contract-Wallet zur Selbstverwahrung** für Ethereum und andere
EVM-Netzwerke. Jede Wallet ist ein unverändertes **Safe-v1.4.1**-Konto, betrieben über
**ERC-4337** und gesteuert von bis zu sieben **Passkeys** – WebAuthn-P-256-Schlüsseln,
die deine Geräte, dein Passwortmanager oder Hardware-Sicherheitsschlüssel verwahren.
Es gibt keine Seed-Phrase.

Vela, die Firma, hält nie deine Schlüssel und hat keine Rolle in deinem Safe. Sie kann
dein Guthaben deshalb **nicht von sich aus bewegen, einfrieren oder beschlagnahmen**.
Die Software, die deine Schlüssel um eine Signatur bittet, schreibt und liefert Vela
allerdings – deshalb ist das Bedrohungsmodell unten wichtig. Die Apps, das Relay, das
Transaktionen einreicht, und die unterstützenden Dienste sind Open Source, und du
kannst von jedem eine eigene Kopie betreiben. Worauf du vertraust, kurz gesagt: auf
die Verträge, auf die Authentifikatoren, die deine Schlüssel halten,
auf den Code der App, mit der du signierst, auf die Domain, zu der deine Passkeys
gehören, und auf die Dienste, auf die du die App einstellst.

## Warum es Vela gibt

- **Seed-Phrase-Wallets** stellen jedem Nutzer ein Geheimnis aus 12–24 Wörtern in den
  Weg: ein einziger Punkt, an dem alles scheitern kann, und ein dauerhaftes Ziel für
  Phishing.
- **Verwahrende Wallets** schaffen die Seed-Phrase ab, indem sie das Geld selbst in
  Verwahrung nehmen.
- **Passkey-Wallets**, die von den Servern und dem geschlossenen Code einer einzigen
  Firma abhängen, schaffen die Seed-Phrase ab, lassen dich aber im Stich, wenn die
  Firma verschwindet.
- **Blindsignieren** – undurchsichtige Daten freigeben, die du nicht lesen kannst – ist
  immer noch verbreitet und einer der Wege, auf denen Wallets leergeräumt werden.

Vela will die Bequemlichkeit eines Passkeys ohne jede dieser Abhängigkeiten: ein
Standardkonto, offenen Code, austauschbare Dienste und Transaktionen, die du lesen
kannst, bevor du signierst.

## Gestaltungsprinzipien

1. **Selbstverwahrung, ohne Ausnahme.** Schlüssel werden von deinen Authentifikatoren
   erzeugt und gehalten. Velas Dienste sehen sie nie; was sie sehen, steht unter
   „Datenschutz“.
2. **Standardverträge, unverändert.** Kein Vertrag auf dem Weg zu deinem Geld wurde von
   Vela geschrieben.
3. **Prüfen statt vertrauen.** Die Apps und Dienste sind öffentlich; die Dienste lassen
   sich selbst hosten.
4. **Vor dem Signieren dekodieren.** Was sich nicht dekodieren lässt, trägt eine
   ausdrückliche Blindsignatur-Warnung.
5. **Weniger tun.** Die Wallet sendet, empfängt und signiert für dApps, die du
   auswählst.

## Architektur

```text
Vela-Apps – Web, Browser-Erweiterung, Desktop (macOS/Windows/Linux), iOS, Android
  ein gemeinsamer Rust-Kern (Regeln, Krypto, ABI, Klartext-Signatur) + je eine native Hülle
  • baut die UserOperation und zeigt, was sie tut
  • bittet deinen Schlüssel um eine WebAuthn-Assertion
        │  signierte UserOperation (Gebühr inklusive)
        ▼
Relay (vela-relay, selbst hostbar)
  • nennt die Gebühr, streckt das Gas vor, reicht handleOps ein
  • kann die Operation nicht ändern
        ▼
EVM-Chain
  EntryPoint v0.7 → dein Safe v1.4.1 → Safe-4337-Modul
  Passkey-Modul von Safe prüft P-256 über das EIP-7951/RIP-7212-Precompile
```

Unterstützende Dienste, alle Open Source: ein **Public-Key-Index**, der neue Wallets in
einem On-Chain-Register einträgt und Abfragen beantwortet, ein
**Chain-Daten**-Verzeichnis und ein **Wechselkurs**-Feed. Siehe die
[Anleitung zum Selbsthosten](/de/docs/self-hosting).

### Konto

Deine Wallet ist ein **Safe-v1.4.1**-Proxy (SafeL2-Singleton) mit dem **4337-Modul
v0.3.0** von Safe als aktiviertem Modul und Fallback-Handler, betrieben über den
**EntryPoint v0.7**. Seine Eigentümer sind Passkey-Signer aus dem **Passkey-Modul
v0.2.1** von Safe: Der erste Schlüssel wird vom Shared Signer geprüft, jeder weitere
Schlüssel von einem eigenen Signer-Vertrag, den die Factory von Safe erstellt. Der
Schwellenwert ist **1**.

Die Adresse ist **deterministisch und kontrafaktisch**: Sie wird mit `CREATE2` aus den
Setup-Daten des Safe berechnet, die jeden der ursprünglichen Schlüssel enthalten, bevor
irgendetwas bereitgestellt ist. Sie ist in jedem Netzwerk dieselbe. Du kannst sofort
daran empfangen; deine erste Transaktion in jedem Netzwerk stellt die Wallet bereit und
bezahlt das mit der Gebühr dieser Transaktion.

### Schlüssel

Eine Wallet hat **einen bis sieben Schlüssel**, festgelegt beim Erstellen. Jeder
einzelne kann allein signieren (1-of-n). Ein Schlüssel kann sein:

- ein Passkey auf dem Gerät, das du nutzt – synchronisiert über iCloud-Schlüsselbund,
  Google Passwortmanager oder einen anderen Passwortmanager, wenn du das erlaubst;
- ein anderes Handy, verbunden per QR-Code (der Hybrid-Transport von WebAuthn);
- ein Hardware-Sicherheitsschlüssel per USB oder NFC, der nirgends synchronisiert
  wird.

Jede Signatur braucht die eigene Nutzerverifizierung des Authentifikators – Biometrie
oder Geräte-PIN oder PIN und Berührung eines Sicherheitsschlüssels. Es gibt keinen
Sitzungsschlüssel. Schlüssel lassen sich später weder hinzufügen noch entfernen noch
ersetzen: Auf jeder Chain, auf der die Wallet noch nicht bereitgestellt ist, steht die
Adresse weiterhin für den ursprünglichen Schlüsselsatz, und ein Eigentümerwechsel auf
einer Chain würde das Konto von Chain zu Chain unterschiedlich machen.

Passkeys gehören zu einer Relying Party – die von Vela werden für **`getvela.app`**
erstellt. Browser bieten sie nur Seiten auf getvela.app oder ihren Subdomains an, was
sie resistent gegen Phishing macht; es ist aber auch eine Abhängigkeit, auf die dieses
Papier weiter unten zurückkommt.

### Ablauf einer Signatur

1. **Bauen:** Die App baut eine UserOperation für dein Safe – einschließlich einer
   Überweisung, die das Relay bezahlt – und simuliert sie.
2. **Dekodieren:** Sie übersetzt die Operation in menschenlesbare Absicht und zeigt sie
   dir.
3. **Signieren:** Dein Authentifikator erzeugt über den Hash der Operation eine
   WebAuthn-Assertion, nachdem er dich verifiziert hat.
4. **Kodieren:** Die Assertion wird als die Safe-Signatur kodiert, die das
   Passkey-Modul erwartet.
5. **Einreichen:** Die signierte Operation geht an das Relay, das den EntryPoint
   aufruft.
6. **On-chain prüfen:** Das Passkey-Modul prüft die P-256-Signatur mit dem
   EIP-7951/RIP-7212-Precompile, bevor das Safe irgendetwas ausführt. Es gibt keinen
   Ersatz-Verifizierer; ein Netzwerk ohne das Precompile lässt sich nicht hinzufügen.

### Gebühren

- Das Relay wird **in-band** bezahlt: Die Operation deklariert null EntryPoint-Gebühren
  und enthält eine Überweisung von deinem Safe an die Adresse des Relays. Betrag und
  Empfänger sind Teil dessen, was du signierst, du zahlst also genau, was der
  Bestätigungsbildschirm angezeigt hat.
- Die Gebühr beträgt **das Dreifache des Gases, das die Wallet für die Operation
  reserviert** (die simulierten Schätzungen um die Hälfte erhöht, mit Mindestwerten),
  **bewertet zum höheren Wert aus dem Gaspreis, den die Wallet selbst abliest, und dem
  Preis des Relays für die gewählte Geschwindigkeit**, mindestens etwa 0,01 US-Dollar.
  Auf Tempo ist der Faktor zwei. Durch den Puffer und den Spielraum im Preis liegt die
  Gebühr über den tatsächlichen On-Chain-Kosten der Operation, bei der ersten
  Transaktion in einem Netzwerk noch deutlicher; die Differenz behält das Relay. Der
  genaue Betrag steht vor dem Signieren auf dem Bestätigungsbildschirm.
- Die Gebühr geht an das Relay, auf das die Wallet eingestellt ist: standardmäßig an
  das von Vela, sonst an eine beliebige vela-relay-Instanz, auch an eine, die du selbst
  betreibst.
- Die Gebühr wird im Coin des Netzwerks oder in einem USD-Stablecoin bezahlt, den das
  Relay akzeptiert (pathUSD auf Tempo, das keinen nativen Coin hat). Es gibt **keinen
  Paymaster**: Niemand sponsert Gas, und niemand kann Transaktionen über eine
  Sponsoring-Richtlinie filtern.
- Ist die eigene Gas-Treasury eines Relays in einem Netzwerk leer, sagt die Wallet das,
  bevor du signierst. Es gibt keine Einzahlung pro Nutzer.

Details: [Netzwerke und Gebühren](/de/docs/networks-and-fees).

### Klartext-Signatur

Aufrufe und EIP-712-Nachrichten werden mit **ERC-7730**-Deskriptoren dekodiert – für
gängige Verträge in die App eingebaut, vom Chain-Daten-Dienst abgerufen oder
Standardformen von Token zugeordnet –, dann, als letzter Ausweg, über eine öffentliche
Selektor-Datenbank, gekennzeichnet als „ohne Gewähr“. Was übrig bleibt, bekommt eine
ausdrückliche Blindsignatur-Warnung. Ein abgerufener Deskriptor wird nie als
verifiziert gekennzeichnet – dieses Wort verdient nur einer, der in die App eingebaut
ist, oder ein abgerufener, der mit ihm identisch ist. Eine On-Chain-Freigabe in
„unbegrenzter“ Höhe (2^200 oder mehr) wird rot angezeigt, mit einer Obergrenze zur
Auswahl; begrenzt du sie nicht, wird sie genau so gesendet, wie die dApp sie gebaut
hat. Eine große, aber begrenzte Freigabe wird mit einem Vorsichtshinweis angezeigt;
signierte Permits lassen sich nicht begrenzen, deshalb werden sie wie angefragt
signiert oder abgelehnt.
Details: [Klartext-Signatur](/de/docs/clear-signing).

### Netzwerke

Vela hat 24 eingebaute Netzwerke – Ethereum, BNB Chain, Polygon, Arbitrum, Optimism,
Base, Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain, Arc, X Layer, Stable,
Soneium, MegaETH, Robinhood Chain, Mantle, Kaia, Celo, Ink, Plume und XRPL EVM – und
akzeptiert jedes EVM-Netzwerk, das die zwölf Verträge hat, auf die es prüft, sowie das
EIP-7951/RIP-7212-Precompile. Zwei der zwölf sind die Passkey-Signer-Factory von Safe
und der Signer-Code, den sie bereitstellt; die braucht nur eine Wallet mit mehr als
einem Schlüssel, und die Prüfung weist sie gesondert aus.

## Sicherheitsmodell

**Was Vela nicht kann**

- Dein Guthaben von sich aus bewegen, ausgeben oder einfrieren – nur deine Schlüssel
  autorisieren dein Safe, und Vela hat darin keine Rolle. (Was Vela kann: Software
  ausliefern, die dich um eine Signatur bittet; siehe die Bedrohungen unten.)
- Eine Transaktion ändern, nachdem du sie signiert hast – jede Änderung macht die
  Signatur ungültig.
- Deine privaten Schlüssel lesen – sie bleiben in deinen Authentifikatoren.
- Deiner Wallet einen Schlüssel hinzufügen oder einen entfernen.

**Was „kann nicht einfrieren“ nicht abdeckt: den Token.** USDC, USDT und die meisten
fiatgedeckten Token erlauben ihrem Emittenten, jede Adresse zu sperren, auch deine.
Diese Macht gehört dem Emittenten und besteht unabhängig davon, welche Wallet du nutzt.
Was dir Selbstverwahrung gibt: Vela ist keine zweite Partei, die das kann.

**Worauf du vertraust**

- Die **Verträge**: Safe, sein 4337- und sein Passkey-Modul, EntryPoint v0.7 und das
  EIP-7951/RIP-7212-Precompile der Chain.
- Die **Domain**: Jede Seite, die von getvela.app oder einer ihrer Subdomains
  ausgeliefert wird, kann deine Schlüssel um eine Signatur bitten.
- Die **Authentifikatoren**, die deine Schlüssel halten, und – bei synchronisierten
  Passkeys – das Apple-, Google- oder Passwortmanager-Konto dahinter.
- **Den Code der App, mit der du signierst.** Er baut die Transaktion und zeigt dir,
  was sie tut. Eine kompromittierte App kann dir das eine zeigen und dich das andere
  signieren lassen; die Abfrage des Authentifikators verrät dir den Unterschied nicht.
- Die **RPC-Endpunkte**, von denen du liest: Ein lügender Knoten kann falsche Guthaben
  oder eine falsche Simulationsvorschau zeigen. Du kannst eigene festlegen.
- Die **Chain-Daten- und Wechselkursdienste**: Sie liefern Token-Listen, Deskriptoren,
  die Liste der Gebühren-Token und die Kurse, mit denen ein Fiat-Betrag in einen
  Token-Betrag umgerechnet wird.
- Das **Relay**: Es kann nicht ändern, was du signiert hast, aber es kann die Operation
  verzögern oder ablehnen, entscheiden, wann sie on-chain landet (und dir so bei einem
  Swap innerhalb deiner Slippage zuvorkommen), und den Gaspreis festlegen, auf dem
  deine Gebühr beruht – bis zum Dreifachen dessen, was die Wallet selbst abliest.

**Berücksichtigte Bedrohungen**

- **Verlorenes oder gestohlenes Gerät** – ein Dieb muss trotzdem die Prüfung des
  Authentifikators bestehen; ein anderer Schlüssel stellt den Zugang wieder her. Aber
  ein Schlüssel lässt sich nicht entfernen: Könnte einer in fremden Händen sein, zieh
  dein Guthaben in eine neue Wallet um, denn über die alte Adresse kann dieser
  Schlüssel in jedem Netzwerk weiter verfügen.
- **Phishing** – ein Passkey lässt sich nicht auf einer gefälschten Seite eintippen,
  und Browser bieten ihn nur Seiten auf getvela.app und ihren Subdomains an.
- **Bösartige dApp** – abgedeckt durch Klartext-Signatur, Freigabesperre und eine
  Ablehnung: Eine Anfrage nach einem Aufruf von deinem Safe an sich selbst –
  `enableModule`, `addOwnerWithThreshold`, `swapOwner`, `setFallbackHandler`,
  `setGuard` und der Rest dieser Familie – wird blockiert, auch innerhalb eines Batches
  oder eines `MultiSend`, ebenso jeder Teilaufruf, der einen `delegatecall` trägt, und
  eine `SafeTx`-Signatur über typisierte Daten. Jeder davon würde, einmal signiert, das
  Konto so vollständig übergeben wie die Bybit-Payload, deshalb bietet die Wallet sie
  gar nicht erst zum Signieren an.
- **Kompromittierter Backend-Dienst** (Relay, Index, Chain-Daten, Wechselkurse) – keine
  Macht zu signieren, aber echter Einfluss: Dienstverweigerung, irreführende
  Deskriptoren oder Token-Listen, falsche Wechselkurse, die ändern, wie viel ein
  Fiat-Betrag tatsächlich sendet, und (beim Relay) Zeitpunkt und Gaspreis wie oben.
  Eine vom Chain-Daten-Dienst abgerufene Beschreibung wird nie verifiziert genannt –
  dieses Wort verdient nur eine in die App eingebaute oder eine abgerufene, die mit ihr
  identisch ist; alle übrigen werden mit einer Zeile angezeigt, die sagt, dass nichts
  sie authentifiziert hat. Jeder Dienst lässt sich ersetzen.
- **Kompromittierte App-Auslieferung** – eine manipulierte Web-Bereitstellung, ein
  manipuliertes Erweiterungs-Update oder ein manipulierter App-Build könnte dir eine
  bösartige Transaktion zum Signieren vorlegen. Das ist die Angriffsklasse von
  [Bybit](/de/docs/bybit-attack). Die Gegenmaßnahmen sind heute begrenzt: die
  Dekodierung und die Freigabesperre in der App selbst, notarisierte macOS-Builds und
  das eigene Bauen der Erweiterung oder der Apps aus dem Quellcode (Release-Pakete
  haben SHA-256-Prüfsummen und GitHub-Build-Provenance-Attestierungen, die Commit und
  Workflow-Lauf benennen; das Windows-Installationsprogramm ist weiterhin nicht
  codesigniert). Eine unabhängige Signaturseite, die den Code der App nicht teilt, ist
  gebaut, aber noch nicht angebunden.
- **Alles, was von der Domain ausgeliefert wird** – jede Seite auf getvela.app oder
  ihren Subdomains, einschließlich eines Skripts, das sie lädt, könnte Signaturen von
  Vela-Passkeys anfordern, und die Abfrage zeigt nur „getvela.app“. Die Website
  verbietet ihren eigenen Seiten deshalb, Passkeys zu nutzen, und hält ihr
  Analyse-Skript von der Seite fern, auf der ein Schlüssel liegt. Wechselte die Domain
  den Besitzer, würde der neue Eigentümer auch kontrollieren, welche Apps die Passkeys
  nutzen dürfen. Die Erweiterung und selbst gebaute Apps bringen ihren eigenen Code
  mit, rufen standardmäßig aber trotzdem Deskriptoren ab und nutzen Dienste unter
  getvela.app.

## Wiederherstellung

Beim Erstellen einer Wallet werden ihre öffentlichen Schlüssel und ihre Adresse in
einem öffentlichen **Registervertrag** auf Gnosis veröffentlicht (auf Ethereum
kopierbar). Auf einem neuen Gerät meldest du dich mit **einem beliebigen** Schlüssel
an; die App findet die Wallet über den Index oder, falls das nicht klappt, direkt über
das Register und prüft, dass die Schlüssel die eingetragene Adresse ergeben. Eine
Wallet mit einem einzigen Schlüssel lässt sich außerdem ganz ohne Register aus zwei
Signaturen neu aufbauen.

<Callout type="warning" title="Deine Schlüssel sind deine Wiederherstellung">
Es gibt keine Seed-Phrase, keine soziale Wiederherstellung und keine Vertrauenspersonen
(Guardians) – nichts, was Vela verlieren, preisgeben oder unter Zwang benutzen könnte.
Gehen alle ursprünglichen Schlüssel verloren, lässt sich die Wallet nicht
wiederherstellen. Erstelle die Wallet mit mehr als einem Schlüssel, lass die
Passkey-Synchronisierung an, wenn du dich darauf verlässt, und sichere das Konto
dahinter.
</Callout>

Details: [Wiederherstellung und Anmeldung](/de/docs/recovery).

## Wenn Vela verschwindet

Dein Guthaben bleibt on-chain in deinem Safe. Die Verträge hängen nicht von Vela ab,
und jeder Dienst, den Vela betreibt, ist Open Source und kann von anderen betrieben
werden. Das Einzige, was sich nicht verlegen lässt, ist die Relying Party
der Passkeys, `getvela.app`: Eine Kopie der Web-Wallet auf einer anderen Domain erstellt
eine andere Wallet. Für bestehende Wallets funktionieren die Vela-Browser-Erweiterung
(die `getvela.app`-Passkeys mit einer Berechtigung nutzen kann) und selbst gebaute Apps
(mit einem Handy oder Sicherheitsschlüssel) auch ohne getvela.app weiter. Die
[Anleitung zum Selbsthosten](/de/docs/self-hosting#if-getvela-app-disappears)
beschreibt jeden Weg und seine Grenzen. Unabhängiger Zugang auf einer Chain setzt
außerdem voraus, dass diese Chain EIP-7951 / RIP-7212 unterstützt.

## Datenschutz

Kein Konto, keine E-Mail, kein KYC. Öffentlich wird, was beim Erstellen einer Wallet ins
Register geschrieben wird: für jeden Schlüssel der öffentliche Schlüssel und die
Credential-ID, das Authentifikator-Modell, dein Wallet-Name und deine
Schlüsselbezeichnungen, die Adresse und die signierten Registrierungsdaten. Velas
Index sieht diesen Eintrag, bevor er ihn einreicht, und die Adressen, für die du Namen
nachschlägst; Velas Relay sieht deine Adresse, die Operationen, die du einreichst, und
den RPC-Endpunkt, den deine App nutzt (einschließlich eines API-Schlüssels in dessen
URL), und bewahrt Operationen für begrenzte Zeit auf, um sie erneut zu versuchen und
Fehler zu analysieren. Jeder Dienst sieht deine IP-Adresse. Die Website nutzt eine
Analyse ohne Cookies. Maßgeblich ist die [Datenschutzerklärung](/privacy).

## Open Source

Alles steht unter der MIT-Lizenz: die Wallet (alle Apps und der Kern), das Relay,
der Public-Key-Index, der Wechselkursdienst und das Chain-Daten-Verzeichnis. Code:
[github.com/mondaylabsltd](https://github.com/orgs/mondaylabsltd/repositories).

## Kein Token

Vela hat keinen Token und plant auch keinen. Es gibt nichts, was man kaufen, farmen
oder womit man spekulieren könnte. Gebühren werden im Coin des jeweiligen Netzwerks
oder in einem Stablecoin bezahlt.

## Audit-Status und Grenzen

Die Verträge von Safe, das 4337- und das Passkey-Modul sowie EntryPoint v0.7 sind
unabhängig auditiert und weit verbreitet. **Velas eigener Code – die Apps, die
Backend-Dienste und der Registervertrag – hatte kein unabhängiges Audit durch Dritte,
und derzeit ist auch keines angesetzt**; ein professionelles Audit ist ein Ziel für den
Zeitpunkt, an dem das Projekt eines finanzieren kann, keine Zusage mit Datum. Bis dahin
ist die Prüfung informell: Der Code ist offen, fähige Mitglieder der Community lesen
ihn, und er wird mit KI-Werkzeugen geprüft. Das hilft; einem professionellen Audit ist
es nicht gleichwertig. Behandle Vela als Alpha-Software. Details:
[Audits und bekannte Probleme](/de/docs/security-audits).

## Referenzen

- ERC-4337 – Account Abstraction über den EntryPoint
- EIP-1271 – Signaturprüfung für Verträge
- ERC-7730 – Deskriptoren für die Klartext-Signatur
- EIP-5792 – Bündeln von Wallet-Aufrufen (`wallet_sendCalls`)
- EIP-7951 / RIP-7212 – Precompile zur Prüfung von P-256-Signaturen
- WebAuthn / FIDO2 – Passkeys
- [Safe Smart Account v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/v1.4.1)
