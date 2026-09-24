---
title: Häufige Fragen
description: "Kurze Antworten zu Verwahrung, Schlüsseln, Wiederherstellung, Netzwerken, Gebühren, dazu, was Vela sehen kann, zu Open Source und dazu, was passiert, wenn es Vela nicht mehr gibt."
source: 7446e22f990d
---

# Häufige Fragen

## Ist Vela eine Wallet zur Selbstverwahrung?

Ja. Deine Wallet ist ein Safe Smart Account, der nur von deinen Schlüsseln gesteuert
wird, und die bleiben auf deinen Geräten, in deinem Passwortmanager oder auf deinen
Sicherheitsschlüsseln. Vela hat keinen Schlüssel und keine Rolle darin und kann dein
Guthaben deshalb nicht von sich aus bewegen, einfrieren oder wiederherstellen. Die
Software, die deine Schlüssel um eine Signatur bittet, schreibt allerdings Vela – siehe
das [Bedrohungsmodell](/de/docs/whitepaper).

## Gibt es wirklich keine Seed-Phrase?

Wirklich nicht. Deine Schlüssel sind Passkeys, und ein Passkey hat kein Geheimnis, das
du aufschreiben oder eintippen könntest. Siehe
[So funktionieren Passkeys](/de/docs/passkeys).

## Was brauche ich, um eine Wallet zu erstellen?

Ein Gerät, das Passkeys unterstützt (ein aktuelles Handy oder einen aktuellen Computer
mit Face ID, Fingerabdruck oder Windows Hello), oder zwei
Hardware-Sicherheitsschlüssel. Keine E-Mail, kein Konto, kein Startguthaben. Du kannst
die Wallet mit bis zu sieben Schlüsseln erstellen; später lassen sich keine hinzufügen.
Siehe [Wallet erstellen](/de/docs/create-wallet).

## Was passiert, wenn ich mein Handy verliere?

Melde dich auf einem neuen Gerät mit einem anderen Schlüssel an: mit demselben Passkey,
synchronisiert über iCloud-Schlüsselbund oder Google Passwortmanager, mit einem anderen
Handy oder mit deinem Sicherheitsschlüssel. Lag auf dem Handy dein einziger Schlüssel
und wurde er nicht synchronisiert, lässt sich die Wallet nicht wiederherstellen. Siehe
[Wiederherstellung und Anmeldung](/de/docs/recovery).

## Welche Netzwerke und Token werden unterstützt?

24 eingebaute EVM-Netzwerke, darunter Ethereum, Base, Arbitrum, Optimism, Polygon, BNB
Chain, Gnosis und Avalanche, dazu jedes EVM-Netzwerk, das du hinzufügst und das die
Anforderungen erfüllt. Native Coins und ERC-20-Token. Die Adresse ist in jedem Netzwerk
dieselbe. Siehe [Netzwerke und Gebühren](/de/docs/networks-and-fees).

## Was kostet das?

- **Die Apps:** Die Web-Wallet, die Browser-Erweiterung und die Desktop-Apps sind
  kostenlos. Die iOS- und Android-Apps werden als Einmalkauf in den Stores angeboten;
  du kannst außerdem jede App kostenlos aus dem Quellcode bauen.
- **Jede Transaktion:** eine Gebühr, die aus deiner Wallet an das Relay geht, das sie
  einreicht – an das von Vela, es sei denn, du stellst die Wallet auf ein anderes Relay
  um oder betreibst ein eigenes. Sie deckt das Gas plus die Marge des Relays und
  beträgt mindestens etwa 0,01 US-Dollar. Der genaue Betrag steht vor dem Signieren auf
  dem Bestätigungsbildschirm und ist Teil dessen, was du signierst. Es gibt keine
  Einzahlung und kein Abo.
  [Wie die Gebühr berechnet wird](/de/docs/networks-and-fees#fee).
- **Kein Token.** Vela hat keinen und plant keinen.

## Kann ich Vela mit dApps nutzen?

Ja, über die Vela-Browser-Erweiterung (Chrome, Edge, Brave) und über den eingebauten
Browser der Desktop-App (macOS, Windows) sowie der iOS- und der Android-App. Die
Web-Wallet unter wallet.getvela.app verbindet sich nicht mit dApps. Siehe
[Vela installieren](/de/docs/install#dapps).

## Was kann Vela sehen oder tun?

Vela kann deine Schlüssel nicht lesen und dein Guthaben nicht von sich aus bewegen.
Velas Dienste sehen deine IP-Adresse und das, was die App sie fragt: Der Index sieht
deine öffentlichen Schlüssel und den Wallet-Namen, wenn er eine neue Wallet
registriert, sowie die Adressen, die du nachschlägst; das Relay sieht deine Adresse,
die Operationen, die du einreichst, und den RPC-Endpunkt, den deine App nutzt; der
Chain-Daten-Dienst sieht, nach welchen Token und Verträgen deine App fragt. Was
on-chain öffentlich wird, steht unter
[Wallet erstellen](/de/docs/create-wallet#what-is-public). Die
[Datenschutzerklärung](/privacy) ist die vollständige, maßgebliche Fassung.

## Ist Vela Open Source?

Ja, komplett, unter der MIT-Lizenz: die Wallet-Apps und der Kern, das Relay, der
Public-Key-Index, der Wechselkursdienst und das Chain-Daten-Verzeichnis, auf
[GitHub](https://github.com/orgs/mondaylabsltd/repositories). Jeden Dienst kannst du
selbst betreiben – siehe die
[Anleitung zum Selbsthosten](/de/docs/self-hosting).

## Ist Vela auditiert?

Die Verträge, in denen dein Geld liegt – Safe und seine Module sowie der
ERC-4337-EntryPoint –, sind auditiert. Velas eigener Code nicht, und ein Audit ist
derzeit nicht angesetzt. Siehe [Audits und bekannte Probleme](/de/docs/security-audits).

## Was, wenn Vela dichtmacht?

Dein Guthaben bleibt on-chain in deinem Safe. Für eine bestehende Wallet funktionieren
die Vela-Browser-Erweiterung und selbst gebaute Apps auch ohne getvela.app weiter, und
jeder Dienst ist Open Source und kann von anderen betrieben werden. Die
[Anleitung zum Selbsthosten](/de/docs/self-hosting#if-getvela-app-disappears) listet
die Wege und ihre Grenzen auf.

## Meine Frage steht hier nicht.

Öffne ein Issue auf [GitHub](https://github.com/mondaylabsltd/vela-wallet/issues) oder
schreib uns auf [X](https://x.com/realvelawallet) oder
[Telegram](https://t.me/velawallet).
