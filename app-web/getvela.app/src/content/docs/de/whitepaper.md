---
title: Whitepaper
description: Wie Vela funktioniert und was du — und was du nicht — vertrauen musst, um es zu benutzen. Architektur, Sicherheitsmodell, Wiederherstellung und wie du das alles selbst prüfst.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Whitepaper

<Callout type="info" title="Status: Alpha · v0.1">
Diese Seite beschreibt, wie Vela heute funktioniert und was du vertrauen musst und
was nicht. Sie bevorzugt Ehrlichkeit vor Marketing. Vela ist in der
<a href="/blog/vela-is-in-alpha">Alpha</a> — fang mit kleinen Beträgen an. Vela
hat keinen Token. Alles hier lässt sich am offenen Quellcode überprüfen.
</Callout>

## Zusammenfassung

Vela ist eine **selbstverwahrte Smart-Contract-Wallet** für EVM-Netzwerke. Jede
Wallet ist ein [Safe](https://github.com/safe-fndn/safe-smart-account) Smart
Account, kontrolliert von einem **Passkey** — einem WebAuthn-Credential (P-256),
das das Betriebssystem deines Geräts Ende-zu-Ende verschlüsselt hält und das mit
Face ID, Touch ID oder Fingerabdruck entsperrt wird. Es gibt keine Seed-Phrasen
und keine privaten Schlüssel, die du kopieren, aufbewahren oder verlieren müsstest.

Vela, die Firma, hält weder deine Schlüssel noch dein Geld und **kann es weder
bewegen noch einfrieren noch beschlagnahmen**. Die App, das Transaktions-Relay und
die begleitenden Dienste sind Open Source und selbst hostbar. Was du vertraust,
reduziert sich auf auditierte Smart Contracts, den Passkey-Tresor deines
Betriebssystems und — nur für die Erreichbarkeit — ein Relay, das du ersetzen oder
selbst betreiben kannst.

## Warum es Vela gibt

Die meisten Wallets erzwingen einen Kompromiss:

- **Seed-Phrase-Wallets** stellen jedem Nutzer ein Geheimnis aus 12–24 Wörtern vor
  die Nase. Es ist der Single Point of Failure und ein dauerhaftes Phishing-Ziel.
- **Verwahrende Wallets** nehmen die Seed-Phrase weg, übernehmen aber die
  Verwahrung deines Geldes — und holen damit das Gegenparteirisiko zurück, das
  Krypto beseitigen sollte.
- **Blindes Signieren** — undurchsichtige Hex-Zeichen freigeben, die du nicht
  lesen kannst — ist im gesamten Ökosystem normal geworden und steckt hinter einem
  großen Teil der leergeräumten Wallets.

Vela will so einfach sein wie eine verwahrende App und dich dabei vollständig
selbstverwahrt lassen: keine Seed-Phrase, keine Verwahrung und keine Transaktion,
die du vor dem Signieren nicht lesen kannst.

## Designprinzipien

1. **Selbstverwahrung, ohne Ausnahmen.** Schlüssel entstehen auf deinem Gerät und
   liegen beim Passkey-Dienst deines Betriebssystems, Ende-zu-Ende verschlüsselt.
   Velas Server sehen nur öffentliche Daten.
2. **Prüfen, nicht vertrauen.** Der gesamte Stack — App und alle vier
   Backend-Dienste — ist Open Source unter der MIT-Lizenz.
3. **Kein blindes Signieren.** Transaktionen werden in lesbare Absicht übersetzt,
   wo ein Deskriptor existiert; unbekannte Aufrufe werden markiert, nicht versteckt.
4. **Weniger tun.** Die Wallet hält ETH und ERC-20 und verbindet sich mit dApps,
   die du wählst. Weniger Code zum Vertrauen, kleinere Angriffsfläche.

## Architektur

```text
Vela-App (iOS / Android / Web, eine Codebasis)
  • Passkey (WebAuthn P-256, Passkey-Dienst des Betriebssystems)
  • UserOperation bauen und signieren
  • Klartext-Signatur-Oberfläche (ERC-7730)
        │  signierte UserOperation
        ▼
Vela-Relay (ERC-4337, selbst hostbar)
  • reicht handleOps beim EntryPoint ein
  • kann deine Transaktion weder ändern noch fälschen
        ▼
EVM-Chain
  EntryPoint v0.7 → Safe Smart Account
  WebAuthn-Signer prüft P-256 on-chain
```

### Account-Modell

Deine Wallet ist ein **Safe v1.4.1** Smart Account (ein Proxy-Vertrag), betrieben
über **ERC-4337** Account Abstraction (EntryPoint v0.7) mit dem **Safe
4337 Module** und einem **WebAuthn-Signer** als Eigentümer des Accounts.

Die Adresse ist **deterministisch** und **kontrafaktisch**: Sie wird per `CREATE2`
aus dem öffentlichen Schlüssel deines Passkeys berechnet, bevor eine Transaktion
gesendet wird — du kannst also empfangen, bevor sie je aufgesetzt wurde. Der
Account setzt sich bei deiner ersten Transaktion selbst auf, bezahlt aus seinem
eigenen Guthaben.

### Schlüssel und Authentifizierung

Die Authentifizierung nutzt **WebAuthn-Passkeys** auf der **P-256**-Kurve. Der
private Schlüssel entsteht auf deinem Gerät und liegt Ende-zu-Ende verschlüsselt
beim Passkey-Dienst deines Betriebssystems (iCloud-Schlüsselbund oder Google
Passwortmanager), der ihn über deine Geräte synchronisiert. **Velas Server sehen
immer nur deinen öffentlichen Schlüssel.** Jedes Signieren verlangt eine frische
biometrische Prüfung — es gibt keinen langlebigen Session-Key. Alle Details in
[So funktionieren Passkeys](/de/docs/passkeys).

### Signatur- und Transaktionsablauf

1. Eine ERC-4337-`UserOperation` für deinen Safe **bauen** und Gas schätzen.
2. Den Aufruf in lesbare Absicht **dekodieren** und zur Prüfung anzeigen.
3. **Signieren** — dein Gerät erzeugt nach der biometrischen Prüfung eine
   WebAuthn-Assertion über den Operationshash.
4. Die Assertion als **EIP-1271**-Vertragssignatur **kodieren**.
5. Die signierte Operation an das Relay **weiterreichen**, das sie beim EntryPoint
   einreicht.
6. **On-chain prüfen** — der Safe verifiziert die P-256-Signatur vor der
   Ausführung über die RIP-7212-Precompile. Die Precompile ist harte
   Voraussetzung: Es gibt keinen Fallback-Verifier, und Vela verweigert Netzwerke,
   denen sie fehlt.

Das Relay bekommt eine **bereits signierte** Operation. Es kann Empfänger, Betrag
oder irgendein anderes Feld nicht ändern, ohne die Signatur ungültig zu machen.

### Relay- und Gas-Modell

- Gas wird **aus dem Guthaben deiner eigenen Wallet** bezahlt — standardmäßig im
  nativen Token des Netzwerks, oder in einem unterstützten Stablecoin, wo das
  Relay einen anbietet. Tempo, ohne native Coin, rechnet Gas immer in
  USD-Stablecoins ab. Es gibt **keinen Paymaster** und keinen Dritten, der deine
  Transaktionen sponsert — oder blockiert.
- Das **Relay ist die einzige Quelle der Wahrheit für den Gaspreis.** Es leitet
  ihn aus den aktuellen Chain-Bedingungen ab; die Wallet zeigt dieses Angebot und
  signiert genau das, was sie zeigt.
- Velas Relay-Gebühr ist bewusst schlicht: Die Summe sind die **Netzwerkkosten
  plus die Servicegebühr des Relays**, mit einer kleinen Mindestgebühr bei sehr
  günstigen Transaktionen. Ein Teil geht an die Validatoren der Chain, der Rest an
  das Relay, das die Infrastruktur betreibt und dein Gas-Konto gefüllt hält.
- Die Wallet **zeigt die geschätzte Gebühr vor dem Bestätigen** — im
  Gebühren-Asset und in deiner Anzeigewährung — und der genannte Betrag samt
  Empfänger ist Teil dessen, was du signierst, das Relay bekommt also genau das
  Angezeigte. Kein versteckter Aufschlag.
- Jeder Safe hat pro Chain ein **eigenes Relay-Konto** (Gas-Konto), aktiviert
  durch eine **nicht erstattungsfähige** Einzahlung. Es kann sich mit der Zeit
  leeren und später eine **erneute Aktivierung** brauchen — es ist also nicht
  streng genommen eine einmalige Einzahlung.

Das Relay ist eine Abhängigkeit für die **Erreichbarkeit**, nicht für die
**Verwahrung**: Es kann verzögern oder ablehnen, aber niemals ändern, fälschen
oder stehlen. Es ist Open Source und du kannst dein eigenes betreiben — und weil
der Preis **genannt und angezeigt** statt versteckt wird, ist selbst die Gebühr
eines selbst gehosteten oder fremden Relays vor dem Signieren immer sichtbar.
Siehe [Netzwerke und Gebühren](/de/docs/networks-and-fees).

### Klartext-Signatur (ERC-7730)

Vela dekodiert Calldata und EIP-712-Typdaten mit **ERC-7730**-Deskriptoren und
zeigt die **Absicht** (Tauschen, Senden, Genehmigen …), die **Substanz** (Beträge,
Adressen) und auf Abruf die **Details** (Nonce, Frist, rohe Calldata), farblich
nach Risiko markiert. Passt kein Deskriptor, zeigt Vela eine ausdrückliche
Blindsignatur-Warnung, statt so zu tun, als verstünde es den Aufruf.

### Netzwerke

Vela unterstützt 12 EVM-Netzwerke — Ethereum, BNB Chain, Polygon, Arbitrum,
Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad und World Chain — plus
eigene Netzwerke. Ein eigenes Netzwerk lässt sich nur hinzufügen, wenn es die
Verträge, auf die Vela baut (EntryPoint, Safe-Verträge, WebAuthn-Signer), und die
RIP-7212-P-256-Precompile bereits hostet; Vela prüft das vor der Aktivierung.

## Sicherheitsmodell

**Was Vela nicht kann:**

- Dein Geld bewegen, ausgeben oder überweisen — nur dein Passkey kann den Safe
  autorisieren.
- Deinen Account einfrieren oder beschlagnahmen — der Safe ist dein Vertrag
  on-chain; Vela hat darauf keine privilegierte Rolle.
- In deinem Namen signieren — jede Transaktion braucht eine frische biometrische
  Assertion.
- Deinen privaten Schlüssel sehen — er erreicht Vela nie; nur dein Gerät kann
  damit signieren.
- Eine Transaktion nach dem Signieren ändern — jede Änderung macht die Signatur
  ungültig.

**Was „kann nicht einfrieren“ nicht abdeckt: den *Token*.** Ein Stablecoin mit
Berechtigungen — USDC, USDT und die meisten fiat-gedeckten Token — trägt eine
Blacklist-Funktion, die sein Emittent gegen jede Adresse aufrufen kann, auch
deine. Diese Macht gehört dem Emittenten und existiert unabhängig davon, in
welcher Wallet du den Token hältst; keine selbstverwahrte Wallet, Vela
eingeschlossen, kann sie wegnehmen. Was Selbstverwahrung dir gibt, ist, dass
**wir** nicht die zweite Partei sind, die es könnte.

**Was du vertraust:**

- Den **Safe-Verträgen** (auditiert, weit verbreitet) und dem WebAuthn-Signer, der
  deinen P-256-Schlüssel prüft.
- Deinem **Passkey-Dienst** (Apple / Google), dass er dein Credential schützt und
  synchronisiert.
- Den **RPC-Anbietern**, die du abfragst (Vela nutzt einen Pool aus mehreren
  Quellen mit Failover; du kannst eigene setzen).
- Dem **Relay**, nur für die Erreichbarkeit — und du kannst es selbst hosten.

**Betrachtete Bedrohungen:**

- **Verlorenes oder gestohlenes Gerät** — ein Dieb braucht trotzdem deine
  Biometrie oder deine PIN, um zu signieren.
- **Phishing / bösartige dApp** — dagegen steht die Klartext-Signatur.
- **Kompromittierter Vela-Server** — bringt keine Signaturfähigkeit; der
  Schadensradius ist eingeschränkter Dienst, nicht Geldverlust.
- **Lieferkettenrisiko** — abgefedert durch Open Source und Selbst-Hosting.

## Wiederherstellung

Dein Passkey wird vom Dienst deines Betriebssystems gesichert; auf einem neuen
Gerät stellst du ihn mit der Anmeldung im selben Apple- oder Google-Konto wieder
her, und deine Wallet ist wieder da.

<Callout type="warning" title="Das Passkey-Backup deiner Plattform ist deine Wiederherstellung">
Velas Wiederherstellung ist dein Passkey, synchronisiert über iCloud-Schlüsselbund
oder Google Passwortmanager. By design gibt es keine Seed-Phrase, keine Social
Recovery und keine Guardians — nichts, was Vela verlieren, leaken oder zu dessen
Nutzung man uns zwingen könnte. Die Kehrseite ist real: Verlierst du
<strong>sowohl</strong> dein Gerät <strong>als auch</strong> den
cloud-synchronisierten Passkey und hast keine weitere Kopie, ist der Account nicht
wiederherstellbar. Lass das Passkey-Backup deiner Plattform aktiv und sichere
dieses Konto.
</Callout>

Das vollständige Wiederherstellungsmodell samt ehrlicher Grenzen steht in
[Wiederherstellung und Anmeldung](/de/docs/recovery).

## Wenn Vela verschwindet

Selbstverwahrung heißt, dass deine Schlüssel und dein Geld nicht davon abhängen,
dass Vela online ist. Das Geld liegt in **deinem Safe-Vertrag on-chain**, und das
Relay ist Open Source und ersetzbar.

Eine ehrliche Einschränkung: WebAuthn bindet einen Passkey an eine
Relying-Party-Domain (`getvela.app`). Ginge diese Domain dauerhaft verloren,
bräuchten daran gebundene Passkeys Hilfe, um anderswo zu funktionieren — ein
Werkzeug, das dem Authenticator die ursprüngliche Relying Party präsentieren kann.
Vela hat dafür früher eine Browser-Erweiterung auf Entwicklerniveau ausgeliefert
und sie im September 2026 zurückgezogen; ein verbrauchertauglicher
Wiederherstellungspfad für den Domainverlust ist weiterhin offene Arbeit, und wir
sagen das, statt zu suggerieren, es gäbe ihn. Unabhängiger On-Chain-Zugriff hängt
außerdem an der P-256-Unterstützung (RIP-7212) der Ziel-Chain, die sich über die
Chains hinweg verbessert.

## Privatsphäre

Keine Konten, keine E-Mail, kein KYC, keine Seed-Phrase zum Einsammeln. Server
speichern nur deinen **öffentlichen Schlüssel** und einen gewählten Account-Namen
(für die geräteübergreifende Wiederherstellung), by design on-chain
veröffentlicht. Transaktionsinhalte werden nicht protokolliert. Die Website nutzt
cookiefreie, selbst gehostete Analytik. Siehe die
[Datenschutzerklärung](/privacy).

## Überprüfbarkeit und Open Source

Alles ist **MIT-lizenziert und Open Source** — die App und alle vier
Backend-Dienste (Chain-Daten, Passkey-Index, Relay, Wechselkurse), die du
**selbst hosten** kannst (Einstellungen → Erweitert → Service-Endpunkte). Der Code
liegt auf
[github.com/mondaylabsltd/vela-wallet](https://github.com/mondaylabsltd/vela-wallet).

## Kein Token

Vela hat **keinen Token** und plant auch keinen. Es gibt nichts zu kaufen, zu
farmen oder zu spekulieren. Gas wird im nativen Asset des jeweiligen Netzwerks
bezahlt.

## Audit-Status und Grenzen

Die **Safe-Verträge** im Kern jedes Vela-Accounts sind unabhängig auditiert und
im Feld erprobt. Velas **eigene Integration** darum herum hatte **kein
unabhängiges Audit durch Dritte**, und derzeit ist keines terminiert — ein
professionelles Audit ist ein Ziel für den Zeitpunkt, an dem das Projekt eines
finanzieren kann, keine Zusage mit Datum. Bis dahin ist die Prüfung informell: Der
Code ist Open Source, und es hängt daran, dass fähige, interessierte Leute aus der
Community ihn lesen, sowie an KI-gestützter Durchsicht. Das hilft, ist aber einem
professionellen Audit nicht gleichwertig. Behandle Vela als Alpha-Software und
benutze Beträge, die du in etwas so Junges zu stecken bereit bist.

## Referenzen

- ERC-4337 — Account Abstraction über den EntryPoint
- EIP-1271 — Standard für die Signaturprüfung durch Verträge
- ERC-7730 — Klartext-Signatur / Deskriptoren für strukturierte Daten
- EIP-5792 — Batching von Wallet-Aufrufen
- RIP-7212 — Precompile für die secp256r1-Signaturprüfung (P-256)
- WebAuthn / FIDO2 — Passkey-Authentifizierung
- [Safe Smart Account v1.4.1](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
