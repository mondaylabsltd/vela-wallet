---
title: Häufige Fragen
description: Häufige Fragen zu Vela — Verwahrung, Passkeys, Smart Accounts, Wiederherstellung, unterstützte Netzwerke, Gebühren und Privatsphäre.
---

# Häufige Fragen

## Ist Vela selbstverwahrt?

Ja. Deine Wallet ist ein Smart Account, gesteuert von einem Schlüssel, den nur du
benutzen kannst. Das Betriebssystem deines Geräts hält ihn, Vela sieht ihn nie.
Vela kann dein Geld weder bewegen noch einfrieren noch zurückholen.

## Ist meine Wallet ein normales Konto oder ein Vertrag?

Sie ist ein **Safe Smart Account** (ein Smart Contract), betrieben mit ERC-4337
Account Abstraction. Genau das erlaubt dir, mit einem Passkey zu signieren, jede
Transaktion vor der Freigabe zu lesen und in jedem Netzwerk dieselbe Adresse zu
nutzen. Die Architektur steht im [Whitepaper](/de/docs/whitepaper).

## Gibt es wirklich keine Seed-Phrase?

Wirklich nicht. Dein Signaturschlüssel ist ein Passkey, den das Betriebssystem
deines Geräts hält und den Vela nie sieht. Es gibt keine zwölf Wörter zum
Aufschreiben, Verlieren oder Abfischen. Warum das sicher ist, steht in
[So funktionieren Passkeys](/de/docs/passkeys).

## Was, wenn ich mein Handy verliere?

Wird dein Passkey über iCloud-Schlüsselbund oder Google Passwortmanager
synchronisiert, meldest du dich auf einem neuen Gerät mit demselben Konto an und
deine Wallet ist wieder da. Das vollständige Modell samt Grenzen steht in
[Wiederherstellung und Anmeldung](/de/docs/recovery).

## Welche Netzwerke und Token werden unterstützt?

Vela bringt **12 EVM-Netzwerke** mit — Ethereum, BNB Chain, Polygon, Arbitrum,
Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad und World Chain — dazu
eigene Netzwerke, mit nativen Token und ERC-20. Deine Adresse ist überall
dieselbe. Siehe [Netzwerke und Gebühren](/de/docs/networks-and-fees).

## Was kostet die Nutzung?

Die Wallet ist kostenlos und Vela hat **keinen Token**. Du zahlst das
Netzwerk-**Gas** aus deinem eigenen Guthaben, dazu eine Relay-Gebühr. Den Preis
stellt das Relay, und er erscheint **vor dem Signieren** aufgeschlüsselt als
_Netzwerkgebühr / Relay-Gebühr / Summe_ — die exakten Kosten jeder Transaktion
stehen auf dem Bestätigungsbildschirm, und der genannte Betrag ist Teil dessen,
was du signierst, kann sich also nicht nachträglich ändern. Sehr günstige
Transaktionen können eine kleine Mindestgebühr treffen. Auf Tempo, das keine
native Coin hat, wird Gas in USD-Stablecoins abgerechnet. Jedes Netzwerk braucht
außerdem eine kleine, **nicht erstattungsfähige Einzahlung, um sein
Gas-Relay-Konto zu aktivieren** (für neue Nutzer übernimmt Vela das mitunter);
weil dieses Konto sich leeren kann, kann es später erneut nötig werden — es ist
also nicht streng genommen einmalig. Details in
[Netzwerke und Gebühren](/de/docs/networks-and-fees).

## Was kann Vela (die Firma) sehen oder tun?

Vela speichert den **öffentlichen** Schlüssel deines Passkeys und den **Namen**,
den du gewählt hast, um die geräteübergreifende Anmeldung zu ermöglichen. Deinen
privaten Schlüssel sieht Vela nicht, Guthaben werden von öffentlichen Chains
gelesen, und eine E-Mail-Anmeldung gibt es nicht. Maßgeblich ist die
[Datenschutzerklärung](/privacy).

## Ist Vela Open Source?

Ja — die Wallet und ihre vier Backend-Dienste (Chain-Daten, Passkey-Index, Relay,
Wechselkurse) sind unter der MIT-Lizenz
[öffentlich auf GitHub](https://github.com/mondaylabsltd/vela-wallet), und du
kannst sie selbst hosten.

## Meine Frage steht hier nicht.

Öffne ein Issue auf [GitHub](https://github.com/mondaylabsltd/vela-wallet) oder
schreib uns auf [X](https://x.com/realvelawallet) oder
[Telegram](https://t.me/velawallet).
